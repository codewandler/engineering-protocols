#!/usr/bin/env bash
# task:decomposes-edge-examples — E1 … E4.
#
# The one task in the set with no dependency on anything else, and the only one whose subject
# already exists — which is why E1–E3 can be red for a real reason today rather than for absence.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

DECOMPOSER="$PLUGIN_DIR/agents/decomposer.md"
SKILL="$PLUGIN_DIR/skills/planning/SKILL.md"

declare_row E1 "neither file contains derived_from:epic: in a protocol artifact new example"
declare_row E2 "both files contain decomposes:epic: in the example that previously read derived_from"
declare_row E3 "the diff against the pre-task revision is the relation token and nothing else"
declare_row E4 "the corrected command creates a story carrying decomposes: epic:…, and validate exits 0"

have git || { red_all "\`git\` is not on PATH"; finish; exit; }
for f in "$DECOMPOSER" "$SKILL"; do
  [ -f "$f" ] || { red_all "$f does not exist"; finish; exit; }
done

# ---- E1 -----------------------------------------------------------------------------------------
# Scoped to a `protocol artifact new` example, not to the whole file: `derived_from` is a legitimate
# relation, and a rule that forbade the word would forbid the vocabulary.
R=0
for f in "$DECOMPOSER" "$SKILL"; do
  HIT="$(grep -n -- '--relate derived_from:epic:' "$f")"
  [ -z "$HIT" ] || { R=1; why "${f#"$REPO"/}: $HIT"; }
done
row E1 "$R"

# ---- E2 -----------------------------------------------------------------------------------------
R=0
for f in "$DECOMPOSER" "$SKILL"; do
  grep -q -- '--relate decomposes:epic:' "$f" \
    || { R=1; why "${f#"$REPO"/} teaches no \`--relate decomposes:epic:\` example"; }
done
row E2 "$R"

# ---- E3 -----------------------------------------------------------------------------------------
# "only the relation token on those lines — no surrounding prose is rewritten", made exact: undo
# the substitution on the current file and it must be byte-identical to the pre-task blob. Any other
# edit anywhere in either file survives the undo and shows up here.
R=0
while IFS=$'\t' read -r mode rev path; do
  [ "$mode" = "token-only" ] || continue
  BEFORE="$(pre_task_blob "$rev" "$path")"
  if [ -z "$BEFORE" ]; then
    R=1; why "cannot read $path at $rev — the pinned pre-task revision is unreachable"
    continue
  fi
  UNDONE="$(sed 's/decomposes:epic:/derived_from:epic:/g' "$REPO/$path")"
  if [ "$UNDONE" != "$BEFORE" ]; then
    R=1
    why "$path differs from $rev by more than the relation token:"
    diff <(printf '%s\n' "$BEFORE") <(printf '%s\n' "$UNDONE") | head -8 | while IFS= read -r l; do
      why "  $l"
    done
  fi
done < <(contract_lines pre-task-blobs.txt)
row E3 "$R"

# ---- E4 -----------------------------------------------------------------------------------------
# The token the file teaches, run for real against a scratch store. Not the whole example line: the
# example names `epic:passkey-login`, which is not the epic the passkeys fixture actually carries,
# and a check that failed on that would be reporting the example's slug rather than its edge.
R=0
if ! have protocol; then
  R=1; why "the \`protocol\` CLI is not on PATH"
else
  WORK="$(scratch)/e4"
  mkdir -p "$WORK"
  cp -R "$FIXTURE_SRC/." "$WORK/"
  mkdir -p "$WORK/artifacts"
  cp -R "$REPO/artifacts/lifecycles" "$WORK/artifacts/lifecycles"
  cp -R "$REPO/artifacts/templates" "$WORK/artifacts/templates"
  STORE="$WORK/.engineering/planning"

  TOKEN="$(grep -ho -- '--relate [a-z_]*:epic:' "$DECOMPOSER" "$SKILL" | sort -u)"
  if [ "$(wc -l <<< "$TOKEN")" -ne 1 ]; then
    R=1; why "the two files teach more than one epic edge: $(tr '\n' ' ' <<< "$TOKEN")"
  else
    REL="${TOKEN#--relate }"; REL="${REL%:epic:}"
    OUT="$(cd "$WORK" && protocol artifact new story e4-probe --store "$STORE" \
      --title "E4 probe" --relate "$REL:epic:passkey-sign-in" 2>&1)" || {
      R=1; why "the taught command was refused: $OUT"
    }
    FILE="$STORE/story/e4-probe.md"
    if [ -f "$FILE" ]; then
      grep -Eq '^[[:space:]]*-[[:space:]]*decomposes:[[:space:]]*epic:passkey-sign-in' "$FILE" \
        || { R=1; why "the created story carries no \`decomposes: epic:passkey-sign-in\` edge"; }
    else
      R=1; why "no story was created at $FILE"
    fi
    VOUT="$(cd "$WORK" && protocol artifact validate --store "$STORE" 2>&1)" \
      || { R=1; why "validate exited non-zero after the taught command: $VOUT"; }
  fi
fi
row E4 "$R"

finish
