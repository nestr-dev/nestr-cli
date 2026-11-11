#!/usr/bin/env bash
# Pre-release smoke test against a live Nestr server. Not run in CI.
#
#   scripts/smoke-live.sh [path-to-nestr-binary]
#
# Runs read-only commands with whatever profile is configured (NESTR_PROFILE,
# NESTR_API_KEY etc. are honoured as usual) and prints PASS or FAIL per command.
# NESTR_READ_ONLY=1 is forced, so a write can't slip through.
#
# A command that needs a newer server than the one you point at passes when the CLI
# refuses it cleanly ("needs a newer Nestr server"), and so does a clean refusal
# from the server (Pro plan, the Insights app off, admin-only). A raw parse error,
# a printed web page, a panic or any other failure is a FAIL. Exits 1 on any FAIL.
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
# Clean refusals that count as expected for a server-gated command.
clean='needs a newer Nestr server|plan required|Insights app is not enabled|permission denied'

# run <gated|plain> <label> <args...>
run() {
  local kind="$1" label="$2"
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
  elif [[ "$kind" == gated ]] && grep -Eq "$clean" "$out_file"; then
    echo "PASS  $label  (refused cleanly: $first)"
  else
    echo "FAIL  $label  (exit $code: $first)"
    fails=$((fails + 1))
  fi
}

echo "nestr: $("$bin" --version 2>&1)"

run plain "auth status" auth status
run plain "me" me
run plain "search" search "a" --limit 3
run plain "inbox list" inbox list
run plain "labels list" labels list
run plain "agents list" agents list
run gated "insights list" insights list
run gated "insights list --to $recent" insights list --to "$recent"
run gated "insights list --to 2025-01-01" insights list --to 2025-01-01
run gated "insights list --user me" insights list --user me
run gated "insights compare tactical_completed" insights compare tactical_completed

me_id="$("$bin" -o json me 2>/dev/null | grep -o '"_id"[[:space:]]*:[[:space:]]*"[^"]*"' | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/')"
if [[ -n "$me_id" ]]; then
  run gated "users labels activity <me>" users labels activity "$me_id" --limit 5
else
  echo "FAIL  users labels activity  (could not read your user id from \`nestr -o json me\`)"
  fails=$((fails + 1))
fi

if [[ $fails -gt 0 ]]; then
  echo "$fails command(s) failed"
  exit 1
fi
echo "all passed"
