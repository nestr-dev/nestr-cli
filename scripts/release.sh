#!/usr/bin/env bash
# Release nestr-cli through a pull request.
#
#   scripts/release.sh [--dry-run] <patch|minor|major|X.Y.Z>
#
# main is protected (required checks, an approving review, enforced for admins),
# so the version bump can't be pushed straight to main. Instead this script:
#
#   1. checks preconditions: clean tree, on an up-to-date main, gh logged in,
#      auto-merge allowed on the repo, the tag and release branch are unused and
#      the new version is greater than the current one;
#   2. bumps the version in Cargo.toml and Cargo.lock on a release/vX.Y.Z branch;
#   3. opens a PR with a changelog since the last tag and arms squash auto-merge;
#   4. waits for the PR to merge, stopping early on a failed required check,
#      requested changes or a closed PR;
#   5. tags the squash commit that landed on main and pushes the tag, which
#      triggers .github/workflows/release.yml.
#
# --dry-run runs the read-only checks, prints the bump diff, the PR body and the
# commands it would run, and changes nothing locally or on GitHub.
#
# Environment: WAIT_TIMEOUT_SECONDS (default 3600), POLL_INTERVAL_SECONDS
# (default 15), MAX_POLL_FAILURES (default 8, consecutive failed gh polls).

set -euo pipefail

WAIT_TIMEOUT_SECONDS=${WAIT_TIMEOUT_SECONDS:-3600}
POLL_INTERVAL_SECONDS=${POLL_INTERVAL_SECONDS:-15}
MAX_POLL_FAILURES=${MAX_POLL_FAILURES:-8}

die() {
  echo "Error: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: scripts/release.sh [--dry-run] <patch|minor|major|X.Y.Z>

Opens a release/vX.Y.Z PR that bumps Cargo.toml and Cargo.lock, waits for it
to merge, then tags the merge commit on main and pushes the tag.

  --dry-run   check preconditions and show what would happen; change nothing
EOF
}

# --- version logic -----------------------------------------------------------

is_semver() {
  [[ "$1" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]
}

# next_version CURRENT patch|minor|major|X.Y.Z
next_version() {
  local current=$1 spec=$2 major minor patch
  is_semver "$current" || die "current version '$current' is not X.Y.Z"
  IFS=. read -r major minor patch <<<"$current"
  case "$spec" in
    patch) echo "$major.$minor.$((patch + 1))" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    major) echo "$((major + 1)).0.0" ;;
    v*) die "pass the version without the leading 'v' (got '$spec')" ;;
    *)
      is_semver "$spec" || die "'$spec' is not patch, minor, major or a X.Y.Z version"
      echo "$spec"
      ;;
  esac
}

# version_gt A B: true when A > B (both X.Y.Z)
version_gt() {
  local a1 a2 a3 b1 b2 b3
  IFS=. read -r a1 a2 a3 <<<"$1"
  IFS=. read -r b1 b2 b3 <<<"$2"
  ((a1 != b1)) && { ((a1 > b1)); return; }
  ((a2 != b2)) && { ((a2 > b2)); return; }
  ((a3 > b3))
}

# The [package] version in Cargo.toml (dependency tables also have version keys).
cargo_toml_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[package]") }
    in_pkg && /^version[[:space:]]*=/ {
      sub(/^version[[:space:]]*=[[:space:]]*"/, ""); sub(/".*/, ""); print; exit
    }
  ' "$1"
}

# bump_files DIR OLD NEW: rewrite the nestr-cli version in DIR/Cargo.toml and
# DIR/Cargo.lock. Only the [package] entry and nestr-cli's lock entry change;
# other crates that happen to share the version are left alone.
bump_files() {
  local dir=$1 old=$2 new=$3 tmp
  tmp=$(mktemp)
  awk -v old="$old" -v new="$new" '
    /^\[/ { in_pkg = ($0 == "[package]") }
    in_pkg && !done && $0 == "version = \"" old "\"" { $0 = "version = \"" new "\""; done = 1 }
    { print }
    END { exit !done }
  ' "$dir/Cargo.toml" >"$tmp" || { rm -f "$tmp"; die "no [package] version \"$old\" in Cargo.toml"; }
  mv "$tmp" "$dir/Cargo.toml"

  tmp=$(mktemp)
  awk -v old="$old" -v new="$new" '
    /^\[\[package\]\]$/ { in_pkg = 0 }
    $0 == "name = \"nestr-cli\"" { in_pkg = 1 }
    in_pkg && !done && $0 == "version = \"" old "\"" { $0 = "version = \"" new "\""; done = 1 }
    { print }
    END { exit !done }
  ' "$dir/Cargo.lock" >"$tmp" || { rm -f "$tmp"; die "no nestr-cli $old entry in Cargo.lock"; }
  mv "$tmp" "$dir/Cargo.lock"
}

# changelog SINCE_REF UNTIL_REF: one markdown bullet per commit.
changelog() {
  local range
  if [ -n "$1" ]; then range="$1..$2"; else range=$2; fi
  git log --no-merges --format='- %s (%h)' "$range"
}

# --- preconditions -----------------------------------------------------------

require_clean_tree() {
  [ -z "$(git status --porcelain)" ] || die "the working tree is not clean. Commit or stash changes first."
}

require_up_to_date_main() {
  # Not --quiet: a diverged tag's rejection line is the usual reason this fails.
  git fetch origin main --tags ||
    die "could not fetch origin. If a tag diverged ('would clobber existing tag'), run: git fetch origin main --tags --force"
  [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || die "run releases from main (you are on $(git rev-parse --abbrev-ref HEAD))."
  [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] ||
    die "local main differs from origin/main. Run 'git pull --ff-only' first."
}

require_gh() {
  command -v gh >/dev/null 2>&1 || die "the GitHub CLI (gh) is required."
  gh auth status >/dev/null 2>&1 || die "gh is not logged in. Run 'gh auth login'."
}

require_auto_merge() {
  local allowed
  allowed=$(gh api 'repos/{owner}/{repo}' --jq .allow_auto_merge) ||
    die "could not read the repository's auto-merge setting."
  if [ "$allowed" != true ]; then
    die "auto-merge is disabled on this repository, and the release PR relies on it.
A repo admin can enable it under Settings > General > Pull Requests > 'Allow auto-merge', or with:
  gh api -X PATCH 'repos/{owner}/{repo}' -F allow_auto_merge=true"
  fi
}

require_unused() {
  local tag=$1 branch=$2
  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    die "tag $tag already exists locally. Pass a higher version."
  fi
  [ -z "$(git ls-remote --tags origin "refs/tags/$tag")" ] || die "tag $tag already exists on origin. Pass a higher version."
  if git rev-parse -q --verify "refs/heads/$branch" >/dev/null; then
    die "branch $branch already exists locally, probably from an earlier attempt. Delete it or pick another version."
  fi
  [ -z "$(git ls-remote --heads origin "refs/heads/$branch")" ] ||
    die "branch $branch already exists on origin. Close its PR and delete it, or pick another version."
}

# --- waiting for the merge ---------------------------------------------------

finish_hint() {
  echo "Once it merges, finish the release with:" >&2
  echo "  git fetch origin main && git tag -a $1 -m $1 <merge-commit-sha> && git push origin $1" >&2
}

# required_failures PR_URL: how many required checks have failed, or nothing if
# that can't be read right now. gh exits non-zero while checks are pending or
# failing, so the count is read from stdout whatever the exit status.
required_failures() {
  local out
  out=$(gh pr checks "$1" --required --json bucket --jq '[.[] | select(.bucket == "fail")] | length' 2>/dev/null) || true
  if [[ "$out" =~ ^[0-9]+$ ]]; then echo "$out"; fi
}

# wait_for_merge PR_URL TAG: prints the merge commit SHA once the PR has merged.
wait_for_merge() {
  local pr=$1 tag=$2 deadline failures=0 pr_status state decision merge_sha failed
  deadline=$((SECONDS + WAIT_TIMEOUT_SECONDS))
  echo "Waiting for an approving review, green required checks and the merge (timeout ${WAIT_TIMEOUT_SECONDS}s)..." >&2
  while true; do
    # Captured into a variable, not read from a process substitution: set -e
    # ignores a failure inside <(...), which left every field empty and spun the
    # loop silently. A failed poll is retried, but not forever.
    if pr_status=$(gh pr view "$pr" --json state,reviewDecision,mergeCommit \
      --jq '[.state, (.reviewDecision // "" | if . == "" then "NONE" else . end), (.mergeCommit.oid // "-")] | @tsv'); then
      failures=0
    else
      failures=$((failures + 1))
      echo "Warning: could not read $pr (attempt $failures of $MAX_POLL_FAILURES); retrying." >&2
      if [ "$failures" -ge "$MAX_POLL_FAILURES" ]; then
        echo "Error: gave up reading $pr after $failures failed attempts. Auto-merge stays armed." >&2
        finish_hint "$tag"
        return 1
      fi
      pr_status=""
    fi
    state="" decision="" merge_sha=""
    IFS=$'\t' read -r state decision merge_sha <<<"$pr_status" || true

    # mergeCommit can lag behind state on the first poll after the merge.
    if [ "$state" = MERGED ] && [ -n "$merge_sha" ] && [ "$merge_sha" != - ]; then
      echo "$merge_sha"
      return 0
    fi
    if [ "$state" = CLOSED ]; then
      echo "Error: $pr was closed without merging. Nothing was tagged." >&2
      return 1
    fi
    if [ "$decision" = CHANGES_REQUESTED ]; then
      echo "Error: changes were requested on $pr. Nothing was tagged." >&2
      return 1
    fi
    # Only required checks: GitHub merges past a failing optional one (such as
    # the advisories audit), so the release should too.
    failed=$(required_failures "$pr")
    if [ -n "$failed" ] && [ "$failed" != 0 ]; then
      echo "Error: a required check failed on $pr. Fix it there; auto-merge stays armed." >&2
      finish_hint "$tag"
      return 1
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      echo "Error: timed out waiting for $pr to merge (state: ${state:-unknown}, review: ${decision:-unknown})." >&2
      finish_hint "$tag"
      return 1
    fi
    sleep "$POLL_INTERVAL_SECONDS"
  done
}

# --- main --------------------------------------------------------------------

main() {
  local dry_run=false spec=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) dry_run=true ;;
      -h | --help)
        usage
        return 0
        ;;
      -*) die "unknown option $1 (see --help)" ;;
      *)
        [ -z "$spec" ] || die "expected one version argument (see --help)"
        spec=$1
        ;;
    esac
    shift
  done
  [ -n "$spec" ] || {
    usage >&2
    exit 2
  }

  cd "$(git rev-parse --show-toplevel)"

  require_clean_tree
  require_gh
  command -v cargo >/dev/null 2>&1 || die "cargo is required to check Cargo.lock after the bump."
  require_up_to_date_main

  local current new tag branch last_tag notes body title
  current=$(cargo_toml_version Cargo.toml)
  [ -n "$current" ] || die "could not read the [package] version from Cargo.toml."
  new=$(next_version "$current" "$spec")
  version_gt "$new" "$current" || die "$new is not greater than the current version $current."
  tag="v$new"
  branch="release/$tag"
  require_unused "$tag" "$branch"
  require_auto_merge

  last_tag=$(git describe --tags --abbrev=0 --match 'v[0-9]*' origin/main 2>/dev/null || true)
  notes=$(changelog "$last_tag" origin/main)
  [ -n "$notes" ] || notes="- No changes since ${last_tag:-the first commit}."
  title="release: bump version to $new"
  body="Release $tag. Bumps the version in \`Cargo.toml\` and \`Cargo.lock\` from $current to $new.

Once this merges, \`scripts/release.sh\` tags the merge commit on main as \`$tag\`, which runs the release workflow.

## Changes since ${last_tag:-the first commit}

$notes"

  echo "Releasing $current -> $new ($tag)"

  if $dry_run; then
    local scratch
    scratch=$(mktemp -d)
    cp Cargo.toml Cargo.lock "$scratch/"
    bump_files "$scratch" "$current" "$new"
    echo
    echo "--- Version bump (not applied) ---"
    diff -u Cargo.toml "$scratch/Cargo.toml" || true
    diff -u Cargo.lock "$scratch/Cargo.lock" || true
    rm -rf "$scratch"
    echo
    echo "--- PR: $title ---"
    echo "$body"
    echo
    echo "--- Would run ---"
    echo "  git switch -c $branch, apply the bump above, cargo metadata --locked"
    echo "  git commit -m '$title' Cargo.toml Cargo.lock && git push -u origin $branch"
    echo "  gh pr create --base main --head $branch --title '$title' --body <above>"
    echo "  gh pr merge <pr> --auto --squash"
    echo "  (wait for an approving review, green required checks and the merge)"
    echo "  git tag -a $tag -m $tag <merge-commit-sha> && git push origin $tag"
    echo
    echo "Dry run: nothing was changed. Next step: scripts/release.sh $spec"
    return 0
  fi

  git switch -c "$branch"
  bump_files . "$current" "$new"
  # --locked fails if the lock still disagrees with Cargo.toml, which would
  # break the release workflow's --locked builds.
  cargo metadata --locked --format-version 1 >/dev/null ||
    die "Cargo.lock is not consistent with Cargo.toml after the bump. Branch $branch is left for inspection."
  git add Cargo.toml Cargo.lock
  git commit -m "$title"
  git push -u origin "$branch"

  local pr merge_sha
  pr=$(gh pr create --base main --head "$branch" --title "$title" --body "$body")
  echo "Opened $pr"
  # Squash so main gets one commit per release; the repo deletes the branch on merge.
  gh pr merge "$pr" --auto --squash
  git switch main

  merge_sha=$(wait_for_merge "$pr" "$tag")

  # Tag exactly the commit that landed on main, and only if it carries the bump.
  git fetch origin main
  git merge-base --is-ancestor "$merge_sha" origin/main || die "merge commit $merge_sha is not on origin/main. Nothing was tagged."
  [ "$(git show "$merge_sha:Cargo.toml" | cargo_toml_version /dev/stdin)" = "$new" ] ||
    die "Cargo.toml at $merge_sha is not $new. Nothing was tagged."
  git merge --ff-only origin/main
  git tag -a "$tag" -m "$tag" "$merge_sha"
  git push origin "$tag" ||
    die "could not push $tag. v* tags are protected by a ruleset; only users allowed to bypass it can push them. The bump is merged; ask one of them to run: git push origin $tag"
  git branch -D "$branch" >/dev/null 2>&1 || true

  echo "Released $tag at $merge_sha. The release workflow is building it now:"
  echo "  gh run list --workflow release.yml --limit 1"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
