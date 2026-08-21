#!/usr/bin/env bash
# Shared vocabulary for the W4.1 verifiers.
#
# Sourced by every `check-*.sh`. It carries no assertion of its own — what it carries is the two
# rules the specification states as invariants, made mechanical so no individual check can forget
# them:
#
#   * **A vacuous check is a failed check.** Every id a check declares must be reported. `finish`
#     fails the check if one was not, so a row that fell out of a branch is a red row and not an
#     absent one.
#   * **The verdict table prints on every path, including failure.** Nothing here sets `-e`, and no
#     helper exits early. A check that dies before its rows print is indistinguishable from a check
#     that had nothing to say.
#
# Deliberately *not* `set -e`: an assertion that aborts the script takes the report with it.
set -uo pipefail

CHECKS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EVAL_DIR="$(cd "$CHECKS_DIR/.." && pwd)"
PLUGIN_DIR="$(cd "$EVAL_DIR/.." && pwd)"
REPO="$(cd "$PLUGIN_DIR/../.." && pwd)"
CONTRACTS="$CHECKS_DIR/contracts"
TRANSCRIPTS="$CHECKS_DIR/transcripts"
RUNNER="$EVAL_DIR/run-agents.sh"
FIXTURE_SRC="$REPO/examples/planning-passkeys"

# ---- rows ---------------------------------------------------------------------------------------
# A check declares its ids and their statements up front, then reports each one exactly once.

declare -A STATEMENT=()
declare -A REPORTED=()
ROW_IDS=()
FAILED=0

# declare_row <id> <statement>
declare_row() {
  STATEMENT["$1"]="$2"
  ROW_IDS+=("$1")
}

# row <id> <exit-status>   — 0 is a pass, anything else is a failure.
row() {
  local id="$1" code="$2"
  if [ -n "${REPORTED[$id]:-}" ]; then
    printf 'FAIL  %-4s reported twice — the check is confused about its own rows\n' "$id"
    FAILED=$((FAILED + 1))
    return
  fi
  REPORTED["$id"]=1
  if [ "$code" -eq 0 ]; then
    printf 'PASS  %-4s %s\n' "$id" "${STATEMENT[$id]:-<undeclared row>}"
  else
    printf 'FAIL  %-4s %s\n' "$id" "${STATEMENT[$id]:-<undeclared row>}"
    FAILED=$((FAILED + 1))
  fi
}

# why <text…>  — the reason under the row it belongs to. Printed, never counted.
why() { printf '        ↳ %s\n' "$*"; }

# red_all <reason>  — every not-yet-reported row goes red for one shared reason.
#
# This is what a missing deliverable looks like. It is emphatically not a skip: the rows are in the
# table, they are red, and the reason is under them. A check that quietly reported nothing when its
# subject did not exist would go green in `run-checks.sh` for having no failures.
red_all() {
  local reason="$1" id
  for id in "${ROW_IDS[@]}"; do
    [ -n "${REPORTED[$id]:-}" ] && continue
    row "$id" 1
    why "$reason"
  done
}

# finish  — the check's exit status, and the last enforcement of the no-silent-row rule.
finish() {
  local id missing=0
  for id in "${ROW_IDS[@]}"; do
    if [ -z "${REPORTED[$id]:-}" ]; then
      printf 'FAIL  %-4s never reported — a row that did not run is not a row that passed\n' "$id"
      missing=$((missing + 1))
    fi
  done
  [ "$((FAILED + missing))" -eq 0 ]
}

# ---- preconditions ------------------------------------------------------------------------------

# have <command>  — is the tool on PATH.
have() { command -v "$1" >/dev/null 2>&1; }

# runner_present  — the subject of most of these checks.
runner_present() { [ -f "$RUNNER" ]; }

# runner <args…>  — invoke it through `bash`, so a missing execute bit is not a false red.
runner() { bash "$RUNNER" "$@"; }

# ---- scratch ------------------------------------------------------------------------------------
# Never `/tmp`: this machine's tmpfs drops writes under pressure. Same rule the two sibling evals
# follow, and the same fallback.

scratch() {
  local base="${TMPDIR:-$HOME/.cache/claude-tmp}"
  mkdir -p "$base" || return 1
  mktemp -d "$base/agent-eval-check.XXXXXX"
}

# under_allowed_base <path>  — is it under $TMPDIR or the documented fallback (F1's other half).
under_allowed_base() {
  local path="$1" base="${TMPDIR:-}" fallback="$HOME/.cache/claude-tmp"
  case "$path" in
    /tmp/*) return 1 ;;
  esac
  [ -n "$base" ] && case "$path" in "$base"/*) return 0 ;; esac
  case "$path" in "$fallback"/*) return 0 ;; esac
  return 1
}

# ---- contracts ----------------------------------------------------------------------------------

# contract_lines <file>  — the file's meaningful lines: no comments, no blanks.
contract_lines() {
  grep -v '^[[:space:]]*#' "$CONTRACTS/$1" 2>/dev/null | grep -v '^[[:space:]]*$'
}

# pre_task_blob <revision> <path>  — the file's bytes before W4.1 touched it.
pre_task_blob() { git -C "$REPO" cat-file blob "$1:$2" 2>/dev/null; }

# ---- reading a verdict table --------------------------------------------------------------------
# The runner's table, by row id. `interface.md` fixes the two accepted verdict words per shape; a
# row this cannot parse is *absent*, and an absent row is a failure at every call site below.

# table_verdict <file> <id>  — prints `pass`, `fail`, `note` or nothing at all.
table_verdict() {
  awk -v want="$2" '
    { verdict = $1; id = $2 }
    id != want { next }
    verdict == "PASS" || verdict == "ok"  { print "pass"; found = 1; exit }
    verdict == "FAIL" || verdict == "gap" || verdict == "unk" { print "fail"; found = 1; exit }
    verdict == "note" { print "note"; found = 1; exit }
  ' "$1" 2>/dev/null
}

# table_has_row <file> <id>
table_has_row() { [ -n "$(table_verdict "$1" "$2")" ]; }

# ---- reading a `protocol trace check` report ----------------------------------------------------
# `report_to_text` writes `  <status> <id>  <statement>`; the status is `ok`, `gap` or `unk`, with
# ` (adv)` appended for an advisory row.

# trace_verdict <report-file> <expectation-id>  — prints `ok`, `gap`, `unk` or nothing.
trace_verdict() {
  awk -v want="$2" '
    { status = $1; rest = $2 }
    status != "ok" && status != "gap" && status != "unk" { next }
    rest == "(adv)" { rest = $3 }
    rest == want { print status; exit }
  ' "$1" 2>/dev/null
}

# trace_rows <report-file>  — how many verdict rows the report carried. Zero rows is R15's failure.
trace_rows() {
  awk '$1 == "ok" || $1 == "gap" || $1 == "unk" { n++ } END { print n + 0 }' "$1" 2>/dev/null
}
