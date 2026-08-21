#!/usr/bin/env bash
# task:agent-eval-scratch-fixture — F1 … F9.
#
# The fixture is what every later assertion is a difference against, so it is checked first and
# checked without an API call: `run-agents.sh --build-fixture-only` is the handle
# `contracts/interface.md` fixes for exactly this.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

declare_row F1 "two builds produce two directories, both outside /tmp, each path printed"
declare_row F2 "the fixture's store lists 7 artifacts, each at its committed source status"
declare_row F3 "protocol artifact validate exits 0 inside the fixture"
declare_row F4 "git status --porcelain is empty, and the build fails loudly when it is not"
declare_row F5 "git log --oneline shows exactly one commit"
declare_row F6 "the baseline has one row per artifact, and one mutated file changes exactly one row"
declare_row F7 "no seed artifact holds the story lifecycle's initial status, and a seed that does fails the build"
declare_row F8 "CLAUDE_CONFIG_DIR is inside the scratch directory and the plugin copy has no eval/"
declare_row F9 "the fixture .gitignore matches no path under .engineering/planning/"

runner_present || { red_all "$RUNNER does not exist"; finish; exit; }
have protocol   || { red_all "the \`protocol\` CLI is not on PATH"; finish; exit; }
have git        || { red_all "\`git\` is not on PATH"; finish; exit; }

WORK_F6_SAVE="$(scratch)/f6-original.md"

build() { # build [env assignments already exported by the caller] -> prints the three lines
  runner --build-fixture-only 2>&1
}
field() { sed -n "s/^$1: //p" <<< "$2" | head -1; }

FIRST="$(build)"
SECOND="$(build)"
S1_SCRATCH="$(field scratch "$FIRST")"; S1_FIXTURE="$(field fixture "$FIRST")"
S1_BASELINE="$(field baseline "$FIRST")"; S2_FIXTURE="$(field fixture "$SECOND")"

if [ -z "$S1_FIXTURE" ] || [ -z "$S2_FIXTURE" ]; then
  red_all "--build-fixture-only printed no \`fixture:\` line (see contracts/interface.md)"
  finish; exit
fi

# ---- F1 -----------------------------------------------------------------------------------------
# Two builds, two directories, neither under /tmp, and the paths printed — which is the last of the
# three, because a scratch directory that survives a run and is never named is one nobody inspects.
R=0
[ "$S1_FIXTURE" = "$S2_FIXTURE" ] && { R=1; why "both builds printed the same path"; }
[ -d "$S1_FIXTURE" ] && [ -d "$S2_FIXTURE" ] || { R=1; why "a printed path is not a directory"; }
under_allowed_base "$S1_FIXTURE" && under_allowed_base "$S2_FIXTURE" \
  || { R=1; why "a fixture is outside \$TMPDIR and \$HOME/.cache/claude-tmp"; }
row F1 "$R"

# ---- F2 -----------------------------------------------------------------------------------------
# Field by field, not counted: seven files at seven statuses is the fixture's whole point, and a
# count is green against seven artifacts that all moved.
STORE="$S1_FIXTURE/.engineering/planning"
LISTED="$(cd "$S1_FIXTURE" && protocol artifact list --store "$STORE" --format json 2>/dev/null)"
R=0
if ! have jq; then
  R=1; why "jq is not on PATH — F2 compares statuses field by field and will not guess"
else
  N="$(jq 'length' <<< "$LISTED" 2>/dev/null)"
  [ "${N:-0}" -eq 7 ] || { R=1; why "the fixture store lists ${N:-0} artifact(s), not 7"; }
  while IFS= read -r src; do
    id="$(sed -n 's/^id: //p' "$src" | head -1)"
    want="$(sed -n 's/^status: //p' "$src" | head -1)"
    got="$(jq -r --arg id "$id" '.[] | select(.id == $id) | .status' <<< "$LISTED" 2>/dev/null)"
    if [ "$got" != "$want" ]; then
      R=1; why "$id is '$got' in the fixture, '$want' in $src"
    fi
  done < <(find "$FIXTURE_SRC/.engineering/planning" -name '*.md' 2>/dev/null | sort)
fi
row F2 "$R"

# ---- F3 -----------------------------------------------------------------------------------------
VALIDATE_OUT="$(cd "$S1_FIXTURE" && protocol artifact validate --store "$STORE" 2>&1)" && R=0 || R=$?
[ "$R" -eq 0 ] || why "$VALIDATE_OUT"
row F3 "$R"

# ---- F4 -----------------------------------------------------------------------------------------
# Both halves. The clean tree, and the build's own refusal to hand back an unclean one — because R3
# says the assertion is the runner's, and an assertion nobody has seen fire is an assertion nobody
# has written.
R=0
DIRT="$(git -C "$S1_FIXTURE" status --porcelain 2>&1)"
[ -z "$DIRT" ] || { R=1; why "the built fixture is already dirty: $DIRT"; }
PROBE="$(EVAL_FIXTURE_DIRTY_PROBE=1 runner --build-fixture-only 2>&1)"; PROBE_EXIT=$?
if [ "$PROBE_EXIT" -eq 0 ]; then
  R=1; why "EVAL_FIXTURE_DIRTY_PROBE=1 still exited 0 — the clean-tree assertion does not fire"
elif ! grep -qi 'unclean\|not clean\|dirty' <<< "$PROBE"; then
  R=1; why "the dirty build failed without saying the tree was unclean"
fi
row F4 "$R"

# ---- F5 -----------------------------------------------------------------------------------------
COMMITS="$(git -C "$S1_FIXTURE" log --oneline 2>/dev/null | wc -l)"
R=0; [ "$COMMITS" -eq 1 ] || { R=1; why "the fixture has $COMMITS commit(s), not 1"; }
row F5 "$R"

# ---- F6 -----------------------------------------------------------------------------------------
# One row per artifact, and the digest is load-bearing: mutate one file, and exactly one row moves.
R=0
if [ -z "$S1_BASELINE" ] || [ ! -f "$S1_BASELINE" ]; then
  R=1; why "no baseline file at '${S1_BASELINE:-<not printed>}' (see contracts/baseline-record.md)"
else
  ROWS="$(wc -l < "$S1_BASELINE")"
  [ "$ROWS" -eq 7 ] || { R=1; why "the baseline has $ROWS row(s) for 7 artifacts"; }
  BAD="$(awk -F'\t' 'NF != 3 || $1 == "" || $2 == "" || $3 !~ /^[0-9a-f]{64}$/' "$S1_BASELINE")"
  [ -z "$BAD" ] || { R=1; why "a baseline row is not <id>\\t<status>\\t<sha256>: $(head -1 <<< "$BAD")"; }

  BEFORE="$(runner --baseline "$S1_FIXTURE" 2>/dev/null)"
  VICTIM="$(find "$STORE" -name '*.md' | sort | head -1)"
  # Restored from a copy, not with `git checkout --`: this is a scratch fixture, but the habit of
  # reaching for a discarding git verb is not one to practise in a script that also runs beside a
  # real checkout.
  cp "$VICTIM" "$WORK_F6_SAVE"
  printf '\n<!-- F6 probe -->\n' >> "$VICTIM"
  AFTER="$(runner --baseline "$S1_FIXTURE" 2>/dev/null)"
  cp "$WORK_F6_SAVE" "$VICTIM"
  CHANGED="$(diff <(printf '%s\n' "$BEFORE") <(printf '%s\n' "$AFTER") | grep -c '^[<>]')"
  # One row on each side of the diff — the same row, before and after.
  [ "$CHANGED" -eq 2 ] || { R=1; why "mutating one file moved $((CHANGED / 2)) baseline row(s), not 1"; }
fi
row F6 "$R"

# ---- F7 -----------------------------------------------------------------------------------------
# The specification's invariant, asserted rather than assumed. Both halves again: the fixture is
# clean of the initial status *and* the build refuses one that is not.
R=0
# `protocol artifact lifecycle story` opens with `story starts at draft`. Read, never assumed —
# which is the same discipline D3 is held to by S6.
INITIAL="$(cd "$S1_FIXTURE" && protocol artifact lifecycle story 2>/dev/null \
  | sed -n 's/^story starts at //p' | head -1)"
if [ -z "$INITIAL" ]; then
  R=1; why "could not read the story lifecycle's initial status from \`protocol artifact lifecycle story\`"
else
  OFFENDERS="$(grep -l "^status: $INITIAL\$" "$STORE"/*/*.md 2>/dev/null)"
  [ -z "$OFFENDERS" ] || { R=1; why "the fixture already holds '$INITIAL': $OFFENDERS"; }

  SEEDED="$(scratch)/seed"
  mkdir -p "$SEEDED"; cp -R "$FIXTURE_SRC/." "$SEEDED/"
  sed -i "s/^status: .*/status: $INITIAL/" \
    "$(find "$SEEDED/.engineering/planning/story" -name '*.md' | sort | head -1)"
  SEED_OUT="$(EVAL_FIXTURE_SRC="$SEEDED" runner --build-fixture-only 2>&1)"; SEED_EXIT=$?
  if [ "$SEED_EXIT" -eq 0 ]; then
    R=1; why "a source seeded with a '$INITIAL' artifact still built — D3 stops discriminating and nothing said so"
  elif ! grep -q "$INITIAL" <<< "$SEED_OUT"; then
    R=1; why "the build refused the seeded source without naming '$INITIAL' as the reason"
  fi
fi
row F7 "$R"

# ---- F8 -----------------------------------------------------------------------------------------
R=0
[ -n "$S1_SCRATCH" ] || { R=1; why "--build-fixture-only printed no \`scratch:\` line"; }
[ -d "$S1_SCRATCH/plugin" ] || { R=1; why "no plugin copy at $S1_SCRATCH/plugin"; }
[ -e "$S1_SCRATCH/plugin/eval" ] && { R=1; why "the plugin copy carries eval/ into its own fixture"; }
CMD="$(EVAL_PRINT_COMMAND=1 runner 2>&1)"
grep -q "CLAUDE_CONFIG_DIR=[^ ]*$S1_SCRATCH\|CLAUDE_CONFIG_DIR=\"\?[^ ]*/claude-home" <<< "$CMD" \
  || { R=1; why "the printed invocation does not set CLAUDE_CONFIG_DIR inside the scratch directory"; }
row F8 "$R"

# ---- F9 -----------------------------------------------------------------------------------------
# "A pattern broad enough to also hide a write under `.engineering/planning/` is a defect, not a
# workaround" — asserted against a path that does not exist yet, which is the case that matters:
# the artifacts stage 1 creates.
R=0
git -C "$S1_FIXTURE" check-ignore -q ".engineering/planning/story/f9-probe.md" \
  && { R=1; why ".gitignore matches .engineering/planning/story/f9-probe.md — a created story could hide"; }
IGNORE="$S1_FIXTURE/.gitignore"
if [ -f "$IGNORE" ]; then
  # Each pattern names one harness-written path and says what writes it (R10). "Says" is a comment
  # on the line above; anything else is a pattern whose reason nobody recorded.
  UNCOMMENTED="$(awk '
    /^[[:space:]]*#/ { commented = 1; next }
    /^[[:space:]]*$/ { commented = 0; next }
    { if (!commented) print; commented = 0 }' "$IGNORE")"
  [ -z "$UNCOMMENTED" ] || { R=1; why "a .gitignore pattern carries no comment saying what writes it: $UNCOMMENTED"; }
fi
row F9 "$R"

finish
