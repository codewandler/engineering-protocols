#!/usr/bin/env bash
# task:agent-eval-reviewer-stage — V1 … V8.
#
# The stage whose green means least on its own: a reviewer that died in its first turn also leaves
# the tree clean. V6 is therefore the load-bearing row here — it replays three transcripts, each
# broken in exactly one way, and requires P5, P6 and P7 to notice.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

PROMPT="$EVAL_DIR/prompt-plan-reviewer.md"
EVIDENCE="$EVAL_DIR/evidence/run-1-live.txt"

declare_row V1 "one live stage-2 run printed all seven rows, each named P1…P7, each with a verdict"
declare_row V2 "on that run every row was green and the stage exited 0"
declare_row V3 "the inter-stage commit is in the fixture's git log as the runner's, tree clean after it"
declare_row V4 "replaying against a tree with one file touched turns P1 red and nothing else"
declare_row V5 "replaying against a store with one status moved turns P3 red"
declare_row V6 "an empty final text turns P5 red; no Bash read verb turns P6 red; no subagent turns P7 red"
declare_row V7 "neither the prompt nor the stage reads a file under integrations/claude-code/agents/"
declare_row V8 "the fixture .gitignore adds only commented single-path lines, and hides nothing in the store"

IDS=()
while IFS=$'\t' read -r stage id _; do
  [ "$stage" = "plan-reviewer" ] && IDS+=("$id")
done < <(contract_lines verdict-rows.txt)

# ---- V1, V2: the recorded live run --------------------------------------------------------------
if [ ! -f "$EVIDENCE" ]; then
  row V1 1; why "no recording at ${EVIDENCE#"$REPO"/} (contracts/evidence-manifest.txt)"
  row V2 1; why "no recording at ${EVIDENCE#"$REPO"/}"
else
  R=0
  for id in "${IDS[@]}"; do
    table_has_row "$EVIDENCE" "$id" || { R=1; why "the recorded table has no row $id"; }
  done
  row V1 "$R"
  R=0
  for id in "${IDS[@]}"; do
    v="$(table_verdict "$EVIDENCE" "$id")"
    [ "$v" = "pass" ] || { R=1; why "$id is '${v:-absent}' in the recorded run"; }
  done
  row V2 "$R"
fi

runner_present || { red_all "$RUNNER does not exist"; finish; exit; }
have git      || { red_all "\`git\` is not on PATH"; finish; exit; }

WORK="$(scratch)"
BUILD="$(runner --build-fixture-only 2>&1)"
FIXTURE="$(sed -n 's/^fixture: //p' <<< "$BUILD" | head -1)"
if [ -z "$FIXTURE" ] || [ ! -d "$FIXTURE" ]; then
  red_all "--build-fixture-only produced no usable fixture"
  finish; exit
fi

# The post-stage-1 state, built rather than paid for: the seed store plus two created stories.
POST1="$WORK/post-stage-1"
cp -R "$FIXTURE" "$POST1"
STORE="$POST1/.engineering/planning"
for slug in device-binding cross-device-sign-in; do
  (cd "$POST1" && protocol artifact new story "$slug" --store "$STORE" \
    --title "Replay story $slug" --relate decomposes:epic:passkey-sign-in) >/dev/null 2>&1
done

replay() { # replay <store-dir> <reviewer-transcript>
  EVAL_REPLAY_STORE_DECOMPOSER="$POST1" \
  EVAL_REPLAY_TRANSCRIPT_DECOMPOSER="$TRANSCRIPTS/decomposer-clean.jsonl" \
  EVAL_REPLAY_STORE_REVIEWER="$1" \
  EVAL_REPLAY_TRANSCRIPT_REVIEWER="$2" \
    runner 2>&1
}

BASE_TABLE="$WORK/base.txt"
replay "$POST1" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$BASE_TABLE"

only_these_moved() { # <base> <mutated> <expected-red…>
  local base="$1" mutated="$2"; shift 2
  local expected=" $* " id bv mv bad=0
  for id in "${IDS[@]}"; do
    bv="$(table_verdict "$base" "$id")"; mv="$(table_verdict "$mutated" "$id")"
    if [[ "$expected" == *" $id "* ]]; then
      [ "$mv" = "fail" ] || { why "$id was expected red and is '${mv:-absent}'"; bad=1; }
    elif [ "$bv" != "$mv" ]; then
      why "$id moved from '${bv:-absent}' to '${mv:-absent}' and should not have"; bad=1
    fi
  done
  return "$bad"
}

# ---- V3 -----------------------------------------------------------------------------------------
# The commit between the stages is **the runner's, not an agent's** — R8 — and that is what makes P1
# a claim about stage 2 alone. Asserted on the fixture after a replayed run: one more commit than
# the fixture's single seed commit, its message saying whose it is, and a clean tree behind it.
R=0
LOG="$(git -C "$POST1" log --oneline 2>/dev/null)"
COMMITS="$(wc -l <<< "$LOG")"
if [ "$COMMITS" -lt 2 ]; then
  R=1; why "the fixture has $COMMITS commit(s) after stage 1 — no inter-stage commit was made"
else
  SUBJECT="$(git -C "$POST1" log -1 --format=%s 2>/dev/null)"
  grep -qiE 'run-agents|runner|stage 1|eval' <<< "$SUBJECT" \
    || { R=1; why "the inter-stage commit's message does not identify it as the runner's: '$SUBJECT'"; }
  DIRT="$(git -C "$POST1" status --porcelain 2>&1)"
  [ -z "$DIRT" ] || { R=1; why "the tree is dirty immediately after the inter-stage commit: $DIRT"; }
fi
row V3 "$R"

# ---- V4 -----------------------------------------------------------------------------------------
# One file touched. Deliberately a file **outside** `.engineering/planning/`, so P1 is the only row
# that can see it — a change inside the store would move P2 or P3 as well and the "nothing else"
# half of the assertion would prove nothing.
V4_STORE="$WORK/v4"; cp -R "$POST1" "$V4_STORE"
printf '\n' >> "$V4_STORE/README.md" 2>/dev/null || printf 'v4\n' > "$V4_STORE/v4-probe.txt"
replay "$V4_STORE" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$WORK/v4.txt"
only_these_moved "$BASE_TABLE" "$WORK/v4.txt" P1; row V4 $?

# ---- V5 -----------------------------------------------------------------------------------------
# P3's reference is the **post-stage-1** status set, not the fixture baseline — so the status this
# moves is one of the two stories stage 1 created.
V5_STORE="$WORK/v5"; cp -R "$POST1" "$V5_STORE"
(cd "$V5_STORE" && protocol artifact move story:device-binding \
  --store "$V5_STORE/.engineering/planning" --to proposed) >/dev/null 2>&1
git -C "$V5_STORE" add -A >/dev/null 2>&1
git -C "$V5_STORE" -c user.email=check@localhost -c user.name=check commit -q -m "v5 probe" >/dev/null 2>&1
replay "$V5_STORE" "$TRANSCRIPTS/plan-reviewer-clean.jsonl" > "$WORK/v5.txt"
R=0
V5_P3="$(table_verdict "$WORK/v5.txt" P3)"
[ "$V5_P3" = "fail" ] || { R=1; why "P3 is '${V5_P3:-absent}' against a moved status — the row does not discriminate"; }
row V5 "$R"

# ---- V6 -----------------------------------------------------------------------------------------
# The three rows that make the green mean something, each against a transcript broken in exactly one
# way. Shown per row, because "one of them went red" is not what the task asks for.
R=0
for pair in "P5:plan-reviewer-empty-final-text" "P6:plan-reviewer-no-read-verb" "P7:plan-reviewer-no-subagent"; do
  id="${pair%%:*}"; t="$TRANSCRIPTS/${pair#*:}.jsonl"
  if [ ! -f "$t" ]; then R=1; why "no input transcript at ${t#"$REPO"/}"; continue; fi
  replay "$POST1" "$t" > "$WORK/v6-$id.txt"
  v="$(table_verdict "$WORK/v6-$id.txt" "$id")"
  if [ "$v" = "fail" ]; then why "$id red against ${t##*/}"
  else R=1; why "$id is '${v:-absent}' against ${t##*/} — the row does not discriminate"; fi
done
row V6 "$R"

# ---- V7 -----------------------------------------------------------------------------------------
R=0
SRC=("$RUNNER")
[ -f "$EVAL_DIR/stage-plan-reviewer.sh" ] && SRC+=("$EVAL_DIR/stage-plan-reviewer.sh")
[ -f "$PROMPT" ] && SRC+=("$PROMPT")
[ -f "$PROMPT" ] || { R=1; why "no prompt at ${PROMPT#"$REPO"/}"; }
HIT="$(grep -n 'agents/' "${SRC[@]}" 2>/dev/null)"
[ -z "$HIT" ] || { R=1; while IFS= read -r l; do why "$l"; done <<< "$HIT"; }
row V7 "$R"

# ---- V8 -----------------------------------------------------------------------------------------
# R10's rule, at the one place it can go wrong: a pattern broad enough to hide a write under
# `.engineering/planning/` turns P1 from a bound into a decoration.
R=0
git -C "$FIXTURE" check-ignore -q ".engineering/planning/story/v8-probe.md" \
  && { R=1; why ".gitignore hides a path under .engineering/planning/"; }
IGNORE="$FIXTURE/.gitignore"
if [ -f "$IGNORE" ]; then
  while IFS= read -r line; do
    case "$line" in ''|'#'*) continue ;; esac
    case "$line" in *'*'*|*'**'*) R=1; why "a .gitignore pattern is not a single path: $line" ;; esac
  done < "$IGNORE"
  UNCOMMENTED="$(awk '
    /^[[:space:]]*#/ { commented = 1; next }
    /^[[:space:]]*$/ { commented = 0; next }
    { if (!commented) print; commented = 0 }' "$IGNORE")"
  [ -z "$UNCOMMENTED" ] || { R=1; why "a pattern carries no comment saying what writes it: $UNCOMMENTED"; }
fi
row V8 "$R"

finish
