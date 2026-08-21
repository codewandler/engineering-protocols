#!/usr/bin/env bash
# task:agent-eval-decomposer-stage — S1 … S8.
#
# S1 and S2 are claims about a run that reaches the API. They are asserted against the recording
# `contracts/evidence-manifest.txt` names, never skipped: a live-only row that disappears from the
# table is the failure mode `A vacuous check is a failed check` describes.
#
# S3, S4 and S5 are the rows that matter, and none of them costs anything — they replay one saved
# store state through the stage's own assertions and require exactly one row to move.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

PROMPT="$EVAL_DIR/prompt-decomposer.md"
EVIDENCE="$EVAL_DIR/evidence/run-1-live.txt"

declare_row S1 "one live stage-1 run printed all nine rows, each named D1…D9, each with a verdict"
declare_row S2 "on that run every row was green and the stage exited 0"
declare_row S3 "replaying with one created story's status hand-moved turns D5 red and nothing else"
declare_row S4 "replaying with one baseline artifact's file byte-changed turns D6 red and nothing else"
declare_row S5 "replaying with created empty turns D1 red, and D2, D3, D4 do not report green"
declare_row S6 "D3's expected status comes from protocol artifact lifecycle story, not the literal draft"
declare_row S7 "prompt-decomposer.md names no status, no move, no approval, and no id but epic:passkey-sign-in"
declare_row S8 "the stage reads no file under integrations/claude-code/agents/"

# ---- S1, S2: the recorded live run --------------------------------------------------------------
IDS=()
while IFS=$'\t' read -r stage id _; do
  [ "$stage" = "decomposer" ] && IDS+=("$id")
done < <(contract_lines verdict-rows.txt)

if [ ! -f "$EVIDENCE" ]; then
  row S1 1; why "no recording at ${EVIDENCE#"$REPO"/} (contracts/evidence-manifest.txt)"
  row S2 1; why "no recording at ${EVIDENCE#"$REPO"/}"
else
  R=0
  for id in "${IDS[@]}"; do
    table_has_row "$EVIDENCE" "$id" || { R=1; why "the recorded table has no row $id"; }
  done
  row S1 "$R"
  R=0
  for id in "${IDS[@]}"; do
    v="$(table_verdict "$EVIDENCE" "$id")"
    [ "$v" = "pass" ] || { R=1; why "$id is '${v:-absent}' in the recorded run"; }
  done
  grep -q '^exit: 0$' "$EVIDENCE" || { R=1; why "the recording does not end in \`exit: 0\`"; }
  row S2 "$R"
fi

# ---- S3, S4, S5: discrimination, by replay ------------------------------------------------------
if ! runner_present; then
  red_all "$RUNNER does not exist"
  finish; exit
fi

# replay_verdicts <store-dir> -> a verdict table on stdout
replay_verdicts() {
  EVAL_REPLAY_STORE_DECOMPOSER="$1" \
  EVAL_REPLAY_TRANSCRIPT_DECOMPOSER="$TRANSCRIPTS/decomposer-clean.jsonl" \
  EVAL_REPLAY_STORE_REVIEWER="$1" \
  EVAL_REPLAY_TRANSCRIPT_REVIEWER="$TRANSCRIPTS/plan-reviewer-clean.jsonl" \
    runner 2>&1
}

# only_these_moved <baseline-table> <mutated-table> <expected-red…>
#
# "turns D5 red **and nothing else**" is two assertions, and the second is the one that catches an
# assertion written so loosely that any mutation reddens it.
only_these_moved() {
  local base="$1" mutated="$2"; shift 2
  local expected=" $* " id bv mv bad=0
  for id in "${IDS[@]}"; do
    bv="$(table_verdict "$base" "$id")"; mv="$(table_verdict "$mutated" "$id")"
    if [[ "$expected" == *" $id "* ]]; then
      [ "$mv" = "fail" ] || { why "$id was expected red and is '${mv:-absent}'"; bad=1; }
    elif [ "$bv" != "$mv" ]; then
      why "$id moved from '${bv:-absent}' to '${mv:-absent}' and should not have"
      bad=1
    fi
  done
  return "$bad"
}

WORK="$(scratch)"
BUILD="$(runner --build-fixture-only 2>&1)"
FIXTURE="$(sed -n 's/^fixture: //p' <<< "$BUILD" | head -1)"
if [ -z "$FIXTURE" ] || [ ! -d "$FIXTURE" ]; then
  for id in S3 S4 S5; do row "$id" 1; why "--build-fixture-only produced no usable fixture"; done
else
  # The state a green stage 1 leaves: the seed store plus two created stories, both in the story
  # lifecycle's initial status, both decomposing the epic. Built here rather than paid for.
  GREEN="$WORK/green"
  cp -R "$FIXTURE" "$GREEN"
  STORE="$GREEN/.engineering/planning"
  for slug in device-binding cross-device-sign-in; do
    (cd "$GREEN" && protocol artifact new story "$slug" --store "$STORE" \
      --title "Replay story $slug" --relate decomposes:epic:passkey-sign-in) >/dev/null 2>&1
  done
  BASE_TABLE="$WORK/base.txt"; replay_verdicts "$GREEN" > "$BASE_TABLE"

  # S3 — one created story's status hand-moved. Through the CLI, because a hand-edited status is
  # not what D5 is about: D5 is about the status *changing*, however legally.
  S3_STORE="$WORK/s3"; cp -R "$GREEN" "$S3_STORE"
  (cd "$S3_STORE" && protocol artifact move story:device-binding \
    --store "$S3_STORE/.engineering/planning" --to proposed) >/dev/null 2>&1
  replay_verdicts "$S3_STORE" > "$WORK/s3.txt"
  only_these_moved "$BASE_TABLE" "$WORK/s3.txt" D5; row S3 $?

  # S4 — one *baseline* artifact's bytes changed. The epic, which the decomposer must never touch.
  S4_STORE="$WORK/s4"; cp -R "$GREEN" "$S4_STORE"
  printf '\n<!-- S4 probe -->\n' >> "$S4_STORE/.engineering/planning/epic/passkey-sign-in.md"
  replay_verdicts "$S4_STORE" > "$WORK/s4.txt"
  only_these_moved "$BASE_TABLE" "$WORK/s4.txt" D6; row S4 $?

  # S5 — `created` empty. D1 must go red, and D2, D3 and D4 must **not** report green: they are
  # universally quantified over the empty set and vacuously true unless the stage says otherwise.
  S5_STORE="$WORK/s5"; cp -R "$FIXTURE" "$S5_STORE"
  replay_verdicts "$S5_STORE" > "$WORK/s5.txt"
  R=0
  [ "$(table_verdict "$WORK/s5.txt" D1)" = "fail" ] || { R=1; why "D1 did not go red against an empty created set"; }
  for id in D2 D3 D4; do
    v="$(table_verdict "$WORK/s5.txt" "$id")"
    [ "$v" = "pass" ] && { R=1; why "$id reported green over the empty set — vacuously true, and said so"; }
    [ -z "$v" ] && { R=1; why "$id produced no row at all"; }
  done
  row S5 "$R"
fi

# ---- S6, S7, S8: what the source may and may not say ---------------------------------------------
# Grep over the stage's source, as the task specifies. Weak as evidence about behaviour, exact as
# evidence about a literal — which is what all three of these rows are.
STAGE_SRC=("$RUNNER")
[ -f "$EVAL_DIR/stage-decomposer.sh" ] && STAGE_SRC+=("$EVAL_DIR/stage-decomposer.sh")

R=0
if ! grep -q 'protocol artifact lifecycle story' "${STAGE_SRC[@]}"; then
  R=1; why "the stage never reads \`protocol artifact lifecycle story\`"
fi
# The literal, used as D3's expected value. `draft` inside a comment or a message is not the defect;
# `draft` on the right-hand side of the comparison is.
HARDCODED="$(grep -nE '(D3|expected|initial)[^#]*=[[:space:]]*"?draft"?' "${STAGE_SRC[@]}")"
[ -z "$HARDCODED" ] || { R=1; why "the literal draft is used as an expected value: $HARDCODED"; }
row S6 "$R"

R=0
if [ ! -f "$PROMPT" ]; then
  R=1; why "no prompt at ${PROMPT#"$REPO"/}"
else
  # Status names come from the lifecycle, not from a list written here — the same discipline D3 is
  # held to. Any of them appearing in the prompt makes the stage a test of obedience.
  STATUSES="$(protocol artifact lifecycle story 2>/dev/null \
    | tr ' ,->' '\n\n\n\n' | grep -E '^[a-z][a-z-]+$' | sort -u)"
  while IFS= read -r s; do
    [ -z "$s" ] && continue
    grep -qiw "$s" "$PROMPT" && { R=1; why "the prompt names the status '$s'"; }
  done <<< "$STATUSES"
  grep -qiwE 'move|approve|approval|approved' "$PROMPT" \
    && { R=1; why "the prompt names a move or an approval"; }
  OTHER_IDS="$(grep -oE '\b(epic|story|task|initiative|specification):[a-z0-9-]+' "$PROMPT" \
    | sort -u | grep -v '^epic:passkey-sign-in$')"
  [ -z "$OTHER_IDS" ] || { R=1; why "the prompt names other artifact id(s): $(tr '\n' ' ' <<< "$OTHER_IDS")"; }
fi
row S7 "$R"

R=0
HIT="$(grep -n 'agents/' "${STAGE_SRC[@]}" "$PROMPT" 2>/dev/null)"
[ -z "$HIT" ] || { R=1; while IFS= read -r l; do why "$l"; done <<< "$HIT"; }
row S8 "$R"

finish
