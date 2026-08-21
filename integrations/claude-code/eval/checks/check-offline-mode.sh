#!/usr/bin/env bash
# task:agent-eval-offline-mode — O1 … O8.
#
# The only check in this set whose subject is fully hermetic by design, so every row here is a real
# assertion today and none of them is a recording. `--offline` is the mode that holds the transcript
# bounds between live runs; if it is dishonest about what it covered, nothing else notices.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

FIXTURES="$EVAL_DIR/fixtures"

declare_row O1 "--offline with both fixtures present exits 0 and prints a row per expectation in both documents"
declare_row O2 "it makes no network call — no credential, no network, still exit 0"
declare_row O3 "it creates no scratch project and runs no headless session"
declare_row O4 "with fixtures/ removed it exits non-zero, naming the missing file by path"
declare_row O5 "with exactly one fixture removed it names that one and does not pass on the other"
declare_row O6 "its output names D1–D7 and P1–P3 explicitly as the rows it did not cover"
declare_row O7 "a fixture transcript that violates a bound makes --offline exit non-zero with that row red"
declare_row O8 "the word skip appears in no verdict the mode can print"

runner_present || { red_all "$RUNNER does not exist"; finish; exit; }

WORK="$(scratch)"

# The two fixtures the mode replays. A missing one is O4's and O5's subject, not a reason to stop:
# every row below still reports.
DEC_FIX="$(find "$FIXTURES" -name '*decompos*.jsonl' 2>/dev/null | head -1)"
REV_FIX="$(find "$FIXTURES" -name '*reviewer*.jsonl' 2>/dev/null | head -1)"

# ---- O1 -----------------------------------------------------------------------------------------
R=0
OUT="$WORK/o1.txt"
runner --offline > "$OUT" 2>&1; O1_EXIT=$?
if [ ! -d "$FIXTURES" ]; then
  R=1; why "no fixtures at ${FIXTURES#"$REPO"/} — O1's premise does not hold yet"
else
  [ "$O1_EXIT" -eq 0 ] || { R=1; why "--offline exited $O1_EXIT with both fixtures present"; }
  while IFS=$'\t' read -r _ id _ _ _ _; do
    grep -qw -- "$id" "$OUT" || { R=1; why "no verdict row for expectation $id"; }
  done < <(contract_lines trace-expectations.txt)
fi
row O1 "$R"

# ---- O2 -----------------------------------------------------------------------------------------
# "No API call" asserted by removing every way to make one, not by reading the script. A mode that
# still exits 0 with no credential and no resolver is a mode that did not reach the network.
R=0
OUT="$WORK/o2.txt"
env -u ANTHROPIC_API_KEY -u CLAUDE_CODE_OAUTH_TOKEN \
  CLAUDE_CONFIG_DIR="$WORK/empty-home" \
  http_proxy=http://127.0.0.1:1 https_proxy=http://127.0.0.1:1 \
  HTTP_PROXY=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 \
  bash "$RUNNER" --offline > "$OUT" 2>&1
O2_EXIT=$?
if [ ! -d "$FIXTURES" ]; then
  R=1; why "no fixtures at ${FIXTURES#"$REPO"/} — O2's premise does not hold yet"
else
  [ "$O2_EXIT" -eq 0 ] || { R=1; why "--offline exited $O2_EXIT with no credential and no reachable network"; }
fi
row O2 "$R"

# ---- O3 -----------------------------------------------------------------------------------------
# No scratch project, shown by counting the directories the base gains — not by trusting the mode's
# own account of itself.
R=0
BASE="${TMPDIR:-$HOME/.cache/claude-tmp}"
BEFORE="$(find "$BASE" -maxdepth 1 -type d 2>/dev/null | sort)"
runner --offline > "$WORK/o3.txt" 2>&1
AFTER="$(find "$BASE" -maxdepth 1 -type d 2>/dev/null | sort)"
NEW="$(comm -13 <(printf '%s\n' "$BEFORE") <(printf '%s\n' "$AFTER") | grep -v 'agent-eval-check' )"
[ -z "$NEW" ] || { R=1; why "--offline created: $(tr '\n' ' ' <<< "$NEW")"; }
grep -qi 'claude -p\|running claude' "$WORK/o3.txt" && { R=1; why "--offline reports running a headless session"; }
row O3 "$R"

# ---- O4 -----------------------------------------------------------------------------------------
# The reason must name the **file by path**. "A fixture is missing" sends a person looking; a path
# sends them to the file.
R=0
if [ ! -d "$FIXTURES" ]; then
  OUT="$WORK/o4.txt"; runner --offline > "$OUT" 2>&1; O4_EXIT=$?
  [ "$O4_EXIT" -ne 0 ] || { R=1; why "--offline exited 0 with no fixtures directory at all"; }
  grep -q 'fixtures/' "$OUT" || { R=1; why "the reason does not name a path under fixtures/"; }
else
  HIDDEN="$FIXTURES.hidden-o4"
  mv "$FIXTURES" "$HIDDEN"
  OUT="$WORK/o4.txt"; runner --offline > "$OUT" 2>&1; O4_EXIT=$?
  mv "$HIDDEN" "$FIXTURES"
  [ "$O4_EXIT" -ne 0 ] || { R=1; why "--offline exited 0 with fixtures/ removed"; }
  grep -q 'fixtures/' "$OUT" || { R=1; why "the reason does not name the missing file by path"; }
  grep -qi 'skip' "$OUT" && { R=1; why "it reported a skip instead of a failure"; }
fi
row O4 "$R"

# ---- O5 -----------------------------------------------------------------------------------------
# One removed, not both. The failure mode this catches is a loop that checks what it finds: with one
# fixture present it has rows, it has no red rows, and it exits 0 having audited half the case.
R=0
if [ -z "$DEC_FIX" ]; then
  R=1; why "no decomposer fixture under ${FIXTURES#"$REPO"/} to remove"
else
  mv "$DEC_FIX" "$DEC_FIX.hidden-o5"
  OUT="$WORK/o5.txt"; runner --offline > "$OUT" 2>&1; O5_EXIT=$?
  mv "$DEC_FIX.hidden-o5" "$DEC_FIX"
  [ "$O5_EXIT" -ne 0 ] || { R=1; why "--offline exited 0 with one of the two fixtures removed"; }
  grep -qF "$(basename "$DEC_FIX")" "$OUT" || { R=1; why "the reason does not name $(basename "$DEC_FIX")"; }
fi
row O5 "$R"

# ---- O6 -----------------------------------------------------------------------------------------
# Named explicitly, "not as a count or a phrase". D8, D9, P4–P7 are transcript-derived and **are**
# covered offline; listing them as gaps would be a different dishonesty in the same output.
R=0
OUT="$WORK/o6.txt"; runner --offline > "$OUT" 2>&1
UNCOVERED=(D1 D2 D3 D4 D5 D6 D7 P1 P2 P3)
COVERED=(D8 D9 P4 P5 P6 P7)
for id in "${UNCOVERED[@]}"; do
  grep -qw -- "$id" "$OUT" || { R=1; why "the output does not name $id among the rows it did not cover"; }
done
NOTCOVERED_BLOCK="$(grep -iA20 'did not cover\|not covered\|uncovered' "$OUT")"
for id in "${COVERED[@]}"; do
  grep -qw -- "$id" <<< "$NOTCOVERED_BLOCK" \
    && { R=1; why "$id is listed as uncovered; it is transcript-derived and is covered offline"; }
done
row O6 "$R"

# ---- O7 -----------------------------------------------------------------------------------------
# A hand-edited copy of a fixture, as the task says. The bound violated is the decomposer's first
# hard rule, because it is the one the whole story is about.
R=0
if [ -z "$DEC_FIX" ]; then
  R=1; why "no decomposer fixture to edit"
else
  cp "$DEC_FIX" "$WORK/o7-original.jsonl"
  cp "$TRANSCRIPTS/decomposer-ran-a-move.jsonl" "$DEC_FIX"
  OUT="$WORK/o7.txt"; runner --offline > "$OUT" 2>&1; O7_EXIT=$?
  cp "$WORK/o7-original.jsonl" "$DEC_FIX"
  [ "$O7_EXIT" -ne 0 ] || { R=1; why "--offline exited 0 against a fixture that ran \`protocol artifact move\`"; }
  [ "$(table_verdict "$OUT" never-ran-a-move)" = "fail" ] \
    || { R=1; why "never-ran-a-move did not go red — the offline mode does not evaluate that bound"; }
fi
row O7 "$R"

# ---- O8 -----------------------------------------------------------------------------------------
# Across every path the mode can take, including the failing ones above. A skipped check reads
# exactly like a passing one, which is why the word is banned rather than discouraged.
R=0
for f in "$WORK"/o*.txt; do
  [ -f "$f" ] || continue
  HIT="$(grep -inE '^(PASS|FAIL|note|ok|gap|unk)[[:space:]].*skip|^[[:space:]]*skip' "$f")"
  [ -z "$HIT" ] || { R=1; why "${f##*/}: $HIT"; }
done
row O8 "$R"

finish
