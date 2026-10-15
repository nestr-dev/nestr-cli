#!/usr/bin/env bash
# Tests for scripts/release.sh. Plain bash, no framework:
#
#   scripts/release_test.sh
#
# Each scenario runs the real script in a throwaway clone whose origin is a
# local bare repo, with a fake `gh` on PATH. The fake answers `gh pr view` polls
# from a per-test script and, on a MERGE step, squash-merges the release branch
# into the bare origin's main, the way GitHub would.

# shellcheck source-path=SCRIPTDIR
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SCRIPT="$REPO_ROOT/scripts/release.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

PASSED=0
FAILED=0

fail() {
  echo "    $*" >&2
  return 1
}

assert_contains() {
  [[ "$1" == *"$2"* ]] || fail "expected output to contain: $2"$'\n'"--- output ---"$'\n'"$1"
}

assert_not_contains() {
  [[ "$1" != *"$2"* ]] || fail "expected output not to contain: $2"
}

run_test() {
  local name=$1 rc
  # Not `if (...)`: bash ignores set -e anywhere inside an if condition, even in
  # a subshell, so every assertion but the last would be silently skipped.
  set +e
  (
    set -e
    "$name"
  ) >"$WORK/test.log" 2>&1
  rc=$?
  set -e
  if [ "$rc" = 0 ]; then
    PASSED=$((PASSED + 1))
    echo "ok   $name"
  else
    FAILED=$((FAILED + 1))
    echo "FAIL $name"
    sed 's/^/    /' "$WORK/test.log"
  fi
}

# --- fixtures ----------------------------------------------------------------

# A fake gh. State lives in $FAKE_GH_DIR:
#   auth          exit status for `gh auth status` (default 0)
#   auto_merge    value of allow_auto_merge (default true)
#   polls         one `gh pr view` answer per line: a JSON object, FAIL (exit 1)
#                 or MERGE (squash-merge the release branch and report MERGED).
#                 The last line repeats once the others are used up.
#   calls         every invocation, appended
#   pr_body       the --body passed to `gh pr create`
write_fake_gh() {
  mkdir -p "$WORK/bin"
  cat >"$WORK/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
d=$FAKE_GH_DIR
echo "gh $*" >>"$d/calls"
jq_filter=""
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  [ "${args[i]}" = --jq ] && jq_filter=${args[i + 1]}
done
case "$1 ${2:-}" in
  "auth status") exit "$(cat "$d/auth" 2>/dev/null || echo 0)" ;;
  "api repos/{owner}/{repo}")
    echo "{\"allow_auto_merge\": $(cat "$d/auto_merge" 2>/dev/null || echo true)}" | jq -r "$jq_filter"
    ;;
  "pr create")
    for ((i = 0; i < ${#args[@]}; i++)); do
      [ "${args[i]}" = --body ] && printf '%s' "${args[i + 1]}" >"$d/pr_body"
      [ "${args[i]}" = --head ] && echo "${args[i + 1]}" >"$d/head"
    done
    echo "https://github.com/nestr-dev/nestr-cli/pull/99"
    ;;
  "pr merge") ;;
  "pr view")
    n=$(cat "$d/poll_n" 2>/dev/null || echo 0)
    total=$(wc -l <"$d/polls")
    line=$(sed -n "$((n < total ? n + 1 : total))p" "$d/polls")
    echo $((n + 1)) >"$d/poll_n"
    case "$line" in
      FAIL) echo "HTTP 502" >&2; exit 1 ;;
      MERGE)
        if [ ! -f "$d/merged_sha" ]; then
          m=$(mktemp -d)
          git clone -q "$ORIGIN" "$m"
          git -C "$m" merge -q --squash "origin/$(cat "$d/head")"
          git -C "$m" commit -q -m "release: squashed (#99)"
          git -C "$m" push -q origin main
          git -C "$m" push -q origin --delete "$(cat "$d/head")"
          git -C "$m" rev-parse HEAD >"$d/merged_sha"
          rm -rf "$m"
        fi
        line="{\"state\":\"MERGED\",\"reviewDecision\":\"APPROVED\",\"mergeCommit\":{\"oid\":\"$(cat "$d/merged_sha")\"},\"statusCheckRollup\":[]}"
        ;;
    esac
    echo "$line" | jq -r "$jq_filter"
    ;;
  *) echo "fake gh: unexpected call: $*" >&2; exit 64 ;;
esac
EOF
  chmod +x "$WORK/bin/gh"
}

# Fresh origin + clone at 0.3.3, tagged v0.3.3, with one commit after the tag.
setup_repo() {
  local t=$WORK/$1
  rm -rf "$t"
  mkdir -p "$t/fake"
  export FAKE_GH_DIR=$t/fake ORIGIN=$t/origin.git
  export PATH="$WORK/bin:$PATH" POLL_INTERVAL_SECONDS=0 MAX_POLL_FAILURES=3 WAIT_TIMEOUT_SECONDS=20
  export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.com
  export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.com
  : >"$FAKE_GH_DIR/calls"
  echo '{"state":"OPEN","reviewDecision":"","mergeCommit":null,"statusCheckRollup":[]}' >"$FAKE_GH_DIR/polls"

  git init -q --bare -b main "$ORIGIN"
  git clone -q "$ORIGIN" "$t/repo" 2>/dev/null
  cd "$t/repo"
  cp "$REPO_ROOT/rust-toolchain.toml" .
  cargo init -q --name nestr-cli --vcs none .
  sed -i.bak 's/^version = ".*"/version = "0.3.3"/' Cargo.toml && rm Cargo.toml.bak
  cargo generate-lockfile -q --offline
  printf 'target/\n' >.gitignore
  git add -A && git commit -q -m "release: bump version to 0.3.3"
  git tag -a v0.3.3 -m v0.3.3
  echo "// feature" >>src/main.rs
  git commit -q -am "feat: add the feature"
  git push -q origin main --tags
}

# --- unit tests (sourced functions) ------------------------------------------

test_next_version() {
  # shellcheck source=release.sh
  source "$SCRIPT"
  [ "$(next_version 0.3.3 patch)" = 0.3.4 ] || fail "patch"
  [ "$(next_version 0.3.3 minor)" = 0.4.0 ] || fail "minor"
  [ "$(next_version 0.3.3 major)" = 1.0.0 ] || fail "major"
  [ "$(next_version 0.3.3 0.10.0)" = 0.10.0 ] || fail "explicit"
  ! (next_version 0.3.3 v0.3.4) 2>/dev/null || fail "accepted a v prefix"
  ! (next_version 0.3.3 0.4) 2>/dev/null || fail "accepted 0.4"
  ! (next_version 0.3.3 0.4.0-rc1) 2>/dev/null || fail "accepted a pre-release"
  ! (next_version 0.3.3 01.2.3) 2>/dev/null || fail "accepted a leading zero"
}

test_version_gt() {
  # shellcheck source=release.sh
  source "$SCRIPT"
  version_gt 0.3.4 0.3.3 || fail "0.3.4 > 0.3.3"
  version_gt 0.10.0 0.9.9 || fail "0.10.0 > 0.9.9"
  version_gt 1.0.0 0.99.99 || fail "1.0.0 > 0.99.99"
  ! version_gt 0.3.3 0.3.3 || fail "0.3.3 > 0.3.3"
  ! version_gt 0.3.2 0.3.3 || fail "0.3.2 > 0.3.3"
  ! version_gt 0.2.9 0.3.0 || fail "0.2.9 > 0.3.0"
}

# On the real manifest and lockfile: exactly one line changes in each, even
# though other crates in Cargo.lock share nestr-cli's version string.
test_bump_files_touches_only_nestr_cli() {
  # shellcheck source=release.sh
  source "$SCRIPT"
  local d=$WORK/bump current
  mkdir -p "$d"
  cp "$REPO_ROOT/Cargo.toml" "$REPO_ROOT/Cargo.lock" "$d/"
  current=$(cargo_toml_version "$d/Cargo.toml")
  bump_files "$d" "$current" 9.9.9
  [ "$(cargo_toml_version "$d/Cargo.toml")" = 9.9.9 ] || fail "Cargo.toml not bumped"
  [ "$(diff "$REPO_ROOT/Cargo.toml" "$d/Cargo.toml" | grep -c '^>')" = 1 ] || fail "Cargo.toml: more than one line changed"
  [ "$(diff "$REPO_ROOT/Cargo.lock" "$d/Cargo.lock" | grep -c '^>')" = 1 ] || fail "Cargo.lock: more than one line changed"
  grep -A1 '^name = "nestr-cli"$' "$d/Cargo.lock" | grep -qx 'version = "9.9.9"' || fail "nestr-cli lock entry not bumped"
  ! (bump_files "$d" 1.2.3 1.2.4) 2>/dev/null || fail "bumped a version that isn't there"

  # A crate listed before nestr-cli with the same version must not be touched.
  printf '[[package]]\nname = "aaa"\nversion = "1.0.0"\n\n[[package]]\nname = "nestr-cli"\nversion = "1.0.0"\ndependencies = [\n "aaa",\n]\n' >"$d/Cargo.lock"
  printf '[package]\nname = "nestr-cli"\nversion = "1.0.0"\n\n[dependencies]\naaa = { version = "1.0.0" }\n' >"$d/Cargo.toml"
  bump_files "$d" 1.0.0 1.0.1
  grep -A1 '^name = "aaa"$' "$d/Cargo.lock" | grep -qx 'version = "1.0.0"' || fail "bumped the decoy crate"
  grep -A1 '^name = "nestr-cli"$' "$d/Cargo.lock" | grep -qx 'version = "1.0.1"' || fail "decoy case: nestr-cli not bumped"
  grep -qx 'aaa = { version = "1.0.0" }' "$d/Cargo.toml" || fail "touched a dependency version"
}

# --- preconditions -----------------------------------------------------------

expect_refusal() {
  local out
  if out=$("$SCRIPT" "$@" 2>&1); then
    fail "expected the script to fail"$'\n'"$out"
  fi
  printf '%s' "$out"
}

test_refuses_dirty_tree() {
  setup_repo dirty
  echo x >>README
  assert_contains "$(expect_refusal patch)" "not clean"
}

test_refuses_branch_other_than_main() {
  setup_repo branch
  git switch -q -c topic
  assert_contains "$(expect_refusal patch)" "run releases from main"
}

test_refuses_stale_main() {
  setup_repo stale
  git reset -q --hard HEAD~1
  assert_contains "$(expect_refusal patch)" "differs from origin/main"
}

test_refuses_gh_logged_out() {
  setup_repo auth
  echo 1 >"$FAKE_GH_DIR/auth"
  assert_contains "$(expect_refusal patch)" "gh is not logged in"
}

test_refuses_version_not_greater() {
  setup_repo notgreater
  assert_contains "$(expect_refusal 0.3.3)" "not greater than the current version 0.3.3"
  assert_contains "$(expect_refusal 0.2.9)" "not greater"
}

test_refuses_existing_local_tag() {
  setup_repo localtag
  git tag v0.3.4
  assert_contains "$(expect_refusal patch)" "tag v0.3.4 already exists locally"
}

test_refuses_existing_remote_tag() {
  setup_repo remotetag
  git tag v0.3.4 && git push -q origin v0.3.4 && git tag -d v0.3.4 >/dev/null
  assert_contains "$(expect_refusal patch)" "tag v0.3.4 already exists"
}

test_refuses_existing_release_branch() {
  setup_repo relbranch
  git push -q origin HEAD:refs/heads/release/v0.3.4
  assert_contains "$(expect_refusal patch)" "branch release/v0.3.4 already exists on origin"
}

test_refuses_when_auto_merge_disabled() {
  setup_repo noautomerge
  echo false >"$FAKE_GH_DIR/auto_merge"
  local out
  out=$(expect_refusal --dry-run patch)
  assert_contains "$out" "auto-merge is disabled"
  assert_contains "$out" "allow_auto_merge=true"
}

# --- dry run -----------------------------------------------------------------

test_dry_run_changes_nothing() {
  setup_repo dryrun
  local out head
  head=$(git rev-parse HEAD)
  out=$("$SCRIPT" --dry-run minor 2>&1)
  assert_contains "$out" "Releasing 0.3.3 -> 0.4.0 (v0.4.0)"
  assert_contains "$out" '+version = "0.4.0"'
  assert_contains "$out" "## Changes since v0.3.3"
  assert_contains "$out" "- feat: add the feature"
  assert_not_contains "$out" "- release: bump version to 0.3.3"
  assert_contains "$out" "Next step: scripts/release.sh minor"
  [ -z "$(git status --porcelain)" ] || fail "tree changed"
  [ "$(git rev-parse HEAD)" = "$head" ] || fail "HEAD moved"
  [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "switched branch"
  ! git rev-parse -q --verify refs/heads/release/v0.4.0 >/dev/null || fail "created a local branch"
  [ -z "$(git ls-remote origin 'refs/heads/release/*' 'refs/tags/v0.4.0')" ] || fail "pushed something"
  ! grep -qE 'gh pr (create|merge)' "$FAKE_GH_DIR/calls" || fail "dry run called a gh write"
}

# --- full release ------------------------------------------------------------

test_release_tags_the_merge_commit() {
  setup_repo happy
  cat >"$FAKE_GH_DIR/polls" <<'EOF'
{"state":"OPEN","reviewDecision":"REVIEW_REQUIRED","mergeCommit":null,"statusCheckRollup":[{"conclusion":null,"state":null}]}
FAIL
{"state":"MERGED","reviewDecision":"APPROVED","mergeCommit":null,"statusCheckRollup":[]}
MERGE
EOF
  local out merged
  out=$("$SCRIPT" patch 2>&1) || fail "release failed"$'\n'"$out"
  merged=$(cat "$FAKE_GH_DIR/merged_sha")
  assert_contains "$out" "could not read"
  assert_contains "$out" "Released v0.3.4 at $merged"
  [ "$(git ls-remote origin refs/heads/main | cut -f1)" = "$merged" ] || fail "origin/main is not the merge commit"
  [ "$(git -C "$ORIGIN" rev-parse 'v0.3.4^{commit}')" = "$merged" ] || fail "tag is not on the merge commit"
  [ "$(git -C "$ORIGIN" cat-file -t v0.3.4)" = tag ] || fail "tag is not annotated"
  git -C "$ORIGIN" show v0.3.4:Cargo.toml | grep -qx 'version = "0.3.4"' || fail "tagged Cargo.toml not bumped"
  git -C "$ORIGIN" show v0.3.4:Cargo.lock | grep -A1 '^name = "nestr-cli"$' | grep -qx 'version = "0.3.4"' || fail "tagged Cargo.lock not bumped"
  grep -q 'gh pr merge https://github.com/nestr-dev/nestr-cli/pull/99 --auto --squash' "$FAKE_GH_DIR/calls" || fail "auto-merge not armed"
  assert_contains "$(cat "$FAKE_GH_DIR/pr_body")" "- feat: add the feature"
  [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "not back on main"
  [ "$(git rev-parse HEAD)" = "$merged" ] || fail "local main not fast-forwarded"
}

release_stops_on() {
  setup_repo "$1"
  echo "$2" >"$FAKE_GH_DIR/polls"
  local out
  out=$(expect_refusal patch)
  assert_contains "$out" "$3"
  [ -z "$(git ls-remote --tags origin refs/tags/v0.3.4)" ] || fail "tagged anyway"
}

test_release_stops_on_failed_check() {
  release_stops_on failedcheck \
    '{"state":"OPEN","reviewDecision":"APPROVED","mergeCommit":null,"statusCheckRollup":[{"conclusion":"SUCCESS"},{"conclusion":"FAILURE"}]}' \
    "a check failed"
}

test_release_stops_on_failed_status_context() {
  release_stops_on failedstatus \
    '{"state":"OPEN","reviewDecision":"","mergeCommit":null,"statusCheckRollup":[{"state":"ERROR"}]}' \
    "a check failed"
}

test_release_stops_on_requested_changes() {
  release_stops_on changes \
    '{"state":"OPEN","reviewDecision":"CHANGES_REQUESTED","mergeCommit":null,"statusCheckRollup":[]}' \
    "changes were requested"
}

test_release_stops_on_closed_pr() {
  release_stops_on closed \
    '{"state":"CLOSED","reviewDecision":"","mergeCommit":null,"statusCheckRollup":[]}' \
    "closed without merging"
}

test_release_gives_up_after_repeated_poll_failures() {
  release_stops_on pollfail FAIL "gave up reading"
}

test_release_times_out() {
  export WAIT_TIMEOUT_SECONDS=0
  release_stops_on timeout \
    '{"state":"OPEN","reviewDecision":"REVIEW_REQUIRED","mergeCommit":null,"statusCheckRollup":[]}' \
    "timed out waiting"
}

# --- run ---------------------------------------------------------------------

write_fake_gh
for t in $(declare -F | awk '$3 ~ /^test_/ { print $3 }'); do
  run_test "$t"
done
echo
echo "$PASSED passed, $FAILED failed"
[ "$FAILED" = 0 ]
