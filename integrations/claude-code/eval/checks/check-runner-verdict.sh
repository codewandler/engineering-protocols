#!/usr/bin/env bash
# task:agent-eval-runner-verdict — R1 … R9.
#
# Ids collide with the specification's requirement numbers on purpose: these are the task's own
# acceptance rows, `R1`–`R9`, and renaming them here would break the one thing a verdict table is
# for. Where this file means a specification requirement it says so in words.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

EVIDENCE="$EVAL_DIR/evidence/run-1-live.txt"
DEC_DOC="$EVAL_DIR/expectations.decomposer.trace.yaml"
REV_DOC="$EVAL_DIR/expectations.plan-reviewer.trace.yaml"

declare_row R1 "one live run printed one table with D1–D9, P1–P7 and a row per trace expectation"
declare_row R2 "exit 0 when every gating row is green, non-zero when one is red — shown both ways"
declare_row R3 "a stage failing early still prints the full verdict table — shown for each stage"
declare_row R4 "a trace document yielding zero rows fails the run, naming that stage"
declare_row R5 "a gating unk fails; an advisory row of any verdict does not"
declare_row R6 "each stage's document is checked against that stage's own transcript — paths differ"
declare_row R7 "the output ends with the run cost and the scratch directory path"
declare_row R8 "EVAL_MODEL and EVAL_MAX_TURNS are read with defaults and change the invocation"
declare_row R9 "run.sh and run-driven.sh are byte-identical to their pre-task state; no Taskfile target"

ALL_IDS=()
while IFS=$'\t' read -r _ id _; do ALL_IDS+=("$id"); done < <(contract_lines verdict-rows.txt)

# ---- R1: the recorded live run ------------------------------------------------------------------
if [ ! -f "$EVIDENCE" ]; then
  row R1 1; why "no recording at ${EVIDENCE#"$REPO"/} (contracts/evidence-manifest.txt)"
else
  R=0
  for id in "${ALL_IDS[@]}"; do
    table_has_row "$EVIDENCE" "$id" || { R=1; why "the recorded table has no row $id"; }
  done
  # One row per expectation in both documents, by id — R15's expansion rule, seen in the output.
  while IFS=$'\t' read -r _ id _ _ _ _; do
    grep -qw -- "$id" "$EVIDENCE" || { R=1; why "the recorded table carries no row for expectation $id"; }
  done < <(contract_lines trace-expectations.txt)
  row R1 "$R"
fi

runner_present || { red_all "$RUNNER does not exist"; finish; exit; }

WORK="$(scratch)"
BUILD="$(runner --build-fixture-only 2>&1)"
FIXTURE="$(sed -n 's/^fixture: //p' <<< "$BUILD" | head -1)"

hermetic() { # hermetic <decomposer-transcript> <reviewer-transcript> [extra env already exported]
  EVAL_REPLAY_STORE_DECOMPOSER="$GREEN" \
  EVAL_REPLAY_TRANSCRIPT_DECOMPOSER="$1" \
  EVAL_REPLAY_STORE_REVIEWER="$GREEN" \
  EVAL_REPLAY_TRANSCRIPT_REVIEWER="$2" \
    runner 2>&1
}

if [ -z "$FIXTURE" ] || [ ! -d "$FIXTURE" ]; then
  red_all "--build-fixture-only produced no usable fixture"
  finish; exit
fi

GREEN="$WORK/green"; cp -R "$FIXTURE" "$GREEN"
for slug in device-binding cross-device-sign-in; do
  (cd "$GREEN" && protocol artifact new story "$slug" --store "$GREEN/.engineering/planning" \
    --title "Replay story $slug" --relate decomposes:epic:passkey-sign-in) >/dev/null 2>&1
done

# ---- R2 -----------------------------------------------------------------------------------------
# Both ways. One direction alone is satisfied by a script that always exits 0 and by a script that
# always exits 1, and neither of those is an honest exit code.
R=0
GREEN_OUT="$WORK/green.txt"
hermetic "$TRANSCRIPTS/decomposer-clean.jsonl" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$GREEN_OUT"
GREEN_EXIT=$?
RED_OUT="$WORK/red.txt"
hermetic "$TRANSCRIPTS/decomposer-ran-a-move.jsonl" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$RED_OUT"
RED_EXIT=$?
[ "$GREEN_EXIT" -eq 0 ] || { R=1; why "a green replay exited $GREEN_EXIT"; }
[ "$RED_EXIT" -ne 0 ]   || { R=1; why "a replay with a charter violation still exited 0"; }
row R2 "$R"

# ---- R3 -----------------------------------------------------------------------------------------
# "No assertion aborts the script before the report." Shown for a failure in each stage: the table
# must be *complete* in both, not merely present — a run that dies mid-table is the same defect.
R=0
for pair in "decomposer:$TRANSCRIPTS/decomposer-ran-a-move.jsonl:$TRANSCRIPTS/plan-reviewer-clean.jsonl" \
            "plan-reviewer:$TRANSCRIPTS/decomposer-clean.jsonl:$TRANSCRIPTS/plan-reviewer-created-an-artifact.jsonl"; do
  stage="${pair%%:*}"; rest="${pair#*:}"; dt="${rest%%:*}"; rt="${rest#*:}"
  out="$WORK/r3-$stage.txt"
  hermetic "$dt" "$rt" > "$out"
  for id in "${ALL_IDS[@]}"; do
    table_has_row "$out" "$id" \
      || { R=1; why "a $stage failure lost row $id from the table"; break; }
  done
done
row R3 "$R"

# ---- R4 -----------------------------------------------------------------------------------------
# The zero-rows rule. An emptied document, and the run must fail *naming the stage* — "the run
# failed" is what a person reads when a document went missing, and it sends them to the wrong file.
R=0
EMPTIED="$WORK/emptied"
mkdir -p "$EMPTIED"
{ printf 'format: trace-spec/1\nid: emptied/decomposer\ntitle: emptied\nexpectations: []\n'; } \
  > "$EMPTIED/expectations.decomposer.trace.yaml"
OUT="$WORK/r4.txt"
EVAL_SPEC_DECOMPOSER="$EMPTIED/expectations.decomposer.trace.yaml" \
  hermetic "$TRANSCRIPTS/decomposer-clean.jsonl" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$OUT"
R4_EXIT=$?
# An environment prefix on a *function* call persists in the calling shell — a bash quirk that would
# hand R5 the emptied document and make it pass for the wrong reason.
unset EVAL_SPEC_DECOMPOSER
[ "$R4_EXIT" -ne 0 ] || { R=1; why "an emptied decomposer document still exited 0"; }
grep -qi 'decomposer' "$OUT" || { R=1; why "the failure does not name the stage whose document was empty"; }
grep -qiE 'zero row|no row|0 row' "$OUT" || { R=1; why "the failure does not say the document produced no rows"; }
row R4 "$R"

# ---- R5 -----------------------------------------------------------------------------------------
# The expansion rule, at its two edges. A gating `unk` is "nobody could tell" — and for this eval
# that is not a green run. An advisory row of any verdict is a note and moves nothing.
R=0
if [ ! -f "$DEC_DOC" ]; then
  R=1; why "no decomposer document at ${DEC_DOC#"$REPO"/} to read the rule against"
else
  UNK="$WORK/unk.jsonl"
  # A transcript whose terminal record the adapter cannot read: the gating rows over it are `unk`,
  # not `gap`. This is the distinction R15's third clause turns on.
  head -1 "$TRANSCRIPTS/decomposer-clean.jsonl" > "$UNK"
  printf '{"type":"telemetry_flush","subtype":"periodic"}\n' >> "$UNK"
  OUT="$WORK/r5.txt"
  hermetic "$UNK" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$OUT"; R5_EXIT=$?
  if grep -qE '^[[:space:]]*unk ' "$OUT"; then
    [ "$R5_EXIT" -ne 0 ] || { R=1; why "a gating unk verdict did not fail the run"; }
  else
    # Without an `unk` row the exit code proves nothing about the rule under test, and a row that
    # passes on an inconclusive probe is the vacuity this whole check set exists to refuse.
    R=1; why "the truncated transcript produced no unk row — R5's premise never held"
  fi
  ADV="$(grep -cE '\(adv\)' "$OUT")"
  if [ "$ADV" -gt 0 ]; then
    grep -qE '^note ' "$OUT" || { R=1; why "$ADV advisory row(s) appeared and none was reported as a note"; }
  fi
fi
row R5 "$R"

# ---- R6 -----------------------------------------------------------------------------------------
# Two sessions, two transcripts. "Shown by the printed transcript paths differing" — so the runner
# must print them, and a run that prints one path is a run that checked one document twice.
R=0
PATHS="$(grep -oE '/[^[:space:]]*\.jsonl' "$GREEN_OUT" | sort -u)"
COUNT="$(grep -c . <<< "$PATHS")"
[ "$COUNT" -ge 2 ] || { R=1; why "the run printed $COUNT transcript path(s); two sessions means two"; }
row R6 "$R"

# ---- R7 -----------------------------------------------------------------------------------------
R=0
TAIL="$(tail -4 "$GREEN_OUT")"
grep -qiE '^cost:|cost: ' <<< "$TAIL" || { R=1; why "the output does not end with the run cost"; }
grep -qF "${FIXTURE%/project}" <<< "$TAIL" || { R=1; why "the output does not end with the scratch directory path"; }
row R7 "$R"

# ---- R8 -----------------------------------------------------------------------------------------
# Read with defaults, and *used*: a variable a script reads and never passes on is a variable that
# documents a behaviour it does not have.
R=0
DEFAULTS="$(EVAL_PRINT_COMMAND=1 runner 2>&1)"
OVERRIDDEN="$(EVAL_MODEL=opus EVAL_MAX_TURNS=7 EVAL_PRINT_COMMAND=1 runner 2>&1)"
[ -n "$DEFAULTS" ] || { R=1; why "EVAL_PRINT_COMMAND=1 printed nothing"; }
grep -q -- '--model' <<< "$DEFAULTS" || { R=1; why "the printed invocation carries no --model"; }
grep -q -- '--max-turns' <<< "$DEFAULTS" || { R=1; why "the printed invocation carries no --max-turns"; }
grep -q -- '--model opus' <<< "$OVERRIDDEN" || { R=1; why "EVAL_MODEL did not change the invocation"; }
grep -q -- '--max-turns 7' <<< "$OVERRIDDEN" || { R=1; why "EVAL_MAX_TURNS did not change the invocation"; }
[ "$DEFAULTS" = "$OVERRIDDEN" ] && { R=1; why "the overridden invocation is identical to the default one"; }
row R8 "$R"

# ---- R9 -----------------------------------------------------------------------------------------
# The surface constraint, as a diff against the pinned pre-task revision rather than against HEAD.
R=0
if ! have git; then
  R=1; why "\`git\` is not on PATH"
else
  while IFS=$'\t' read -r mode rev path; do
    [ "$mode" = "identical" ] || continue
    BEFORE="$(pre_task_blob "$rev" "$path")"
    if [ -z "$BEFORE" ]; then
      R=1; why "cannot read $path at $rev"
      continue
    fi
    if [ "$BEFORE" != "$(cat "$REPO/$path" 2>/dev/null)" ]; then
      R=1
      why "$path is not byte-identical to $rev:"
      git -C "$REPO" diff --stat "$rev" -- "$path" | while IFS= read -r l; do why "  $l"; done
    fi
  done < <(contract_lines pre-task-blobs.txt)
  grep -qi 'agent-eval' "$REPO/Taskfile.yml" 2>/dev/null \
    && { R=1; why "Taskfile.yml gained an agent-eval target; the root Taskfile is outside the surface"; }
fi
row R9 "$R"

finish
