#!/usr/bin/env bash
# The verifiers for W4.1 — one check per decomposed task, one table, an honest exit code.
#
# Written in the `establish_verifiers` state, **before** any of `run-agents.sh`, the two prompts,
# the two trace documents, `fixtures/` or the README exists. It is therefore red, and being red is
# the state's product: a test that passes before the code exists is a test of nothing.
#
# Nothing here calls the Claude API. Rows that are claims about a live run are asserted against the
# recordings named in `contracts/evidence-manifest.txt`, which is how a live-only row stays in the
# table instead of becoming a skip.
#
#   ./run-checks.sh                 every check
#   ./run-checks.sh scratch-fixture trace-documents    only those
#
set -uo pipefail

CHECKS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# The order is the tasks' dependency order, so a reader of a red table sees the root cause first.
ALL=(
  scratch-fixture
  decomposes-edge-examples
  trace-documents
  decomposer-stage
  reviewer-stage
  runner-verdict
  offline-mode
  readme
  live-evidence
)

# Which task each check decides, so a red row points at the artifact that owns it.
declare -A OWNER=(
  [scratch-fixture]="task:agent-eval-scratch-fixture"
  [decomposes-edge-examples]="task:decomposes-edge-examples"
  [trace-documents]="task:agent-eval-trace-documents"
  [decomposer-stage]="task:agent-eval-decomposer-stage"
  [reviewer-stage]="task:agent-eval-reviewer-stage"
  [runner-verdict]="task:agent-eval-runner-verdict"
  [offline-mode]="task:agent-eval-offline-mode"
  [readme]="task:agent-eval-readme"
  [live-evidence]="task:agent-eval-live-evidence"
)

SELECTED=("$@")
[ "${#SELECTED[@]}" -eq 0 ] && SELECTED=("${ALL[@]}")

OUT="$(mktemp "${TMPDIR:-$HOME/.cache/claude-tmp}/agent-eval-checks.XXXXXX")" || exit 1
trap 'rm -f "$OUT"' EXIT

TOTAL_PASS=0
TOTAL_FAIL=0
BROKEN=0

for name in "${SELECTED[@]}"; do
  script="$CHECKS_DIR/check-$name.sh"
  printf '\n== %s  (%s) ==\n' "$name" "${OWNER[$name]:-unowned}"
  if [ ! -f "$script" ]; then
    printf 'FAIL  ----  no check exists for %s\n' "$name"
    BROKEN=$((BROKEN + 1))
    continue
  fi

  # `bash "$script"`, never `"$script"`: a missing execute bit is a property of the checkout, not a
  # verdict about the task.
  bash "$script" > "$OUT" 2>&1
  status=$?
  cat "$OUT"

  pass=$(grep -c '^PASS ' "$OUT")
  fail=$(grep -c '^FAIL ' "$OUT")
  TOTAL_PASS=$((TOTAL_PASS + pass))
  TOTAL_FAIL=$((TOTAL_FAIL + fail))

  # R15's rule, applied to the harness itself: a check that produced **zero** rows fails. A table
  # with nothing in it goes green while checking nothing, and that is the one outcome no report may
  # ever produce.
  if [ "$((pass + fail))" -eq 0 ]; then
    printf 'FAIL  ----  %s produced no rows (exit %s) — a check that asserts nothing is not green\n' \
      "$name" "$status"
    BROKEN=$((BROKEN + 1))
  elif [ "$status" -eq 0 ] && [ "$fail" -gt 0 ]; then
    printf 'FAIL  ----  %s exited 0 with %s red row(s) — its exit code disagrees with its table\n' \
      "$name" "$fail"
    BROKEN=$((BROKEN + 1))
  fi
done

printf '\n== W4.1 verifiers: %s pass, %s fail, %s broken check(s) ==\n' \
  "$TOTAL_PASS" "$TOTAL_FAIL" "$BROKEN"
printf 'contracts: %s\n' "$CHECKS_DIR/contracts"
[ "$((TOTAL_FAIL + BROKEN))" -eq 0 ]
