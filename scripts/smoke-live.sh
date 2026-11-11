#!/usr/bin/env bash
# Pre-release smoke test against a live Nestr server. Not run in CI.
#
#   scripts/smoke-live.sh [path-to-nestr-binary]
#
# Runs read-only commands with whatever profile is configured (NESTR_PROFILE,
# NESTR_API_KEY etc. are honoured as usual) and prints PASS or FAIL per command.
# NESTR_READ_ONLY=1 is forced, so a write can't slip through.
#
# Each command lists the clean refusals it may meet: "needs a newer Nestr server" for a
# feature the server lacks, and the server's own refusals (Pro plan, the Insights app
# off, admin-only) where they apply. A raw parse error, a printed web page, a panic or
# any other failure is a FAIL. Exits 1 on any FAIL.
set -uo pipefail

bin="${1:-}"
if [[ -z "$bin" ]]; then
  bin="$(command -v nestr || true)"
fi
if [[ -z "$bin" || ! -x "$bin" ]]; then
  echo "usage: $0 [path-to-nestr-binary]  (no nestr binary found)" >&2
  exit 2
fi
export NESTR_READ_ONLY=1
export NO_COLOR=1

# The first --to check sits inside the last four weeks, which every server takes.
if recent="$(date -u -d '10 days ago' +%F 2>/dev/null)"; then :; else
  recent="$(date -u -v-10d +%F)"
fi

fails=0
out_file="$(mktemp)"
trap 'rm -f "$out_file"' EXIT

# Signs of a broken reply, whatever the exit code.
broken='expected value at line|panicked|<!DOCTYPE|<html|unexpected non-JSON'
# Clean refusals, per kind of command.
newer='needs a newer Nestr server'
insights='plan required|Insights app is not enabled'

# run <allowed-refusals-regex or -> <label> <args...>
run() {
  local allowed="$1" label="$2"
  shift 2
  "$bin" "$@" >"$out_file" 2>&1
  local code=$?
  local first
  first="$(grep -v '^[[:space:]]*$' "$out_file" | head -n 1 | cut -c1-160)"
  if grep -Eq "$broken" "$out_file"; then
    echo "FAIL  $label  (exit $code: $first)"
    fails=$((fails + 1))
  elif [[ $code -eq 0 ]]; then
    echo "PASS  $label"
  elif [[ "$allowed" != - ]] && grep -Eq "$allowed" "$out_file"; then
    echo "PASS  $label  (refused cleanly: $first)"
  else
    echo "FAIL  $label  (exit $code: $first)"
    fails=$((fails + 1))
  fi
}

echo "nestr: $("$bin" --version 2>&1)"

run - "auth status" auth status
run - "me" me
run - "search" search "a" --limit 3
run - "inbox list" inbox list
run - "labels list" labels list
run - "agents list" agents list
run "$insights" "insights list" insights list
run "$insights" "insights list --to $recent" insights list --to "$recent"
run "$newer|$insights" "insights list --to 2025-01-01" insights list --to 2025-01-01
run "$insights" "insights list --user me" insights list --user me
run "$newer|$insights" "insights compare tactical_completed" insights compare tactical_completed

me_id="$("$bin" -o json me 2>/dev/null | grep -o '"_id"[[:space:]]*:[[:space:]]*"[^"]*"' | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/')"
if [[ -n "$me_id" ]]; then
  run "$newer|permission denied" "users labels activity <me>" users labels activity "$me_id" --limit 5
else
  echo "FAIL  users labels activity  (could not read your user id from \`nestr -o json me\`)"
  fails=$((fails + 1))
fi

if [[ $fails -gt 0 ]]; then
  echo "$fails command(s) failed"
  exit 1
fi
echo "all passed"
