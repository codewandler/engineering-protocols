#!/usr/bin/env bash
# Guardrail 1, made mechanical: the planning store's frontmatter is the CLI's, and nothing else
# writes it.
#
# `PreToolUse`, matcher `Edit|Write|NotebookEdit`. `NotebookEdit` is in the matcher because it
# writes files and is in the offered set — leaving it out would have made the matcher look
# exhaustive while a second file-writing tool walked past it (review finding F16).
#
# ## The scope, decided rather than assumed — and why it is narrower than "deny the path"
#
# The skill's guardrail 2 says the **body is edited directly, and only the body**: prose is not the
# CLI's business and there is no verb for it. A hook that denied every write under
# `.engineering/planning/**` would therefore forbid the one kind of edit the plugin exists to ask
# for. So the rule is split by what the tool can promise:
#
# | tool | decision | why |
# |---|---|---|
# | `Write`, `NotebookEdit` | **denied** under the store, always | both replace a whole file. Re-typing frontmatter by hand is exactly the failure guardrail 2 names, and a faithfully-copied frontmatter is indistinguishable from a silently-altered one until something downstream breaks |
# | `Edit` | allowed **only** when neither `old_string` nor `new_string` contains a `---` fence line or a machine-owned key at the start of a line | a targeted edit below the closing fence cannot reach frontmatter, and the payload carries both strings, so this is decidable from `tool_input` alone |
#
# The machine-owned keys are the store conventions' own list — `id`, `kind`, `status`, `revision`,
# `relations`, `format` — not a guess. `title` and `summary` are descriptive and are not guarded:
# correcting a typo by hand is harmless, and the conventions say so.
#
# **What this deliberately does not claim.** Content inspection from `tool_input` is exact for
# `Edit` (both strings are present) and impossible for `Write` (the file's current frontmatter is
# not in the payload, so "would this change it?" is not answerable) — which is why `Write` is denied
# by path rather than by content. That asymmetry is the honest scope, and the audit that does not
# depend on it is `protocol artifact validate`, which catches an illegal status whether or not this
# hook ever fired.
#
# ## Always on, and why that is not the hook layer § 3.6 refused
#
# § 3.6 refused hooks in the planning plugin because a hook layer would be *a second, weaker
# driver* — one that sees tool calls rather than workflow states and can ask the engine nothing.
# This guard is not that: it reads no workflow state, asks nothing, and holds one path rule that is
# true in every state of every workflow. It therefore runs with or without a driven run, unlike its
# neighbour `driven-surface.sh`, which is inert outside one precisely because it *is* per-state.
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
. "$DIR/lib.sh"

AEP_PAYLOAD="$(cat)"
HOOK=store-integrity
CAPABILITY=planning.write

# The pre-filter, and the reason it is sound: `tool_input.file_path` is a substring of the payload,
# so a call that touches the store cannot fail to mention it. Everything else leaves without
# parsing anything, which is what bounds the cost of having no parser to the calls this guard is
# for.
case "$AEP_PAYLOAD" in
  *.engineering/planning*) ;;
  *) aep_pass ;;
esac

aep_load_context || true

if [ -z "$(aep_parser)" ]; then
  aep_deny "$HOOK" "$CAPABILITY" \
    "this write mentions the planning store and neither \`jq\` nor \`python3\` is on PATH, so the store-integrity guard cannot read the tool's arguments. It refuses rather than passing the call through unchecked. Install either, or use \`protocol artifact\` verbs, which need no guard."
fi

TOOL="$(aep_field tool_name)"
case "$TOOL" in
  NotebookEdit) TARGET="$(aep_field tool_input.notebook_path)" ;;
  *) TARGET="$(aep_field tool_input.file_path)" ;;
esac

# The mention was somewhere else in the payload — a prompt, a description, another argument.
case "$TARGET" in
  *.engineering/planning/*) ;;
  *) aep_pass ;;
esac

case "$TOOL" in
  Write|NotebookEdit)
    aep_deny "$HOOK" "$CAPABILITY" \
      "\`$TOOL\` replaces the whole of $TARGET, and the planning store's frontmatter is owned by the \`protocol\` CLI. Write the body with a targeted \`Edit\` below the closing \`---\`, and change frontmatter through \`protocol artifact\` — \`new\`, \`move\`, \`relate\`. A hand-retyped frontmatter is indistinguishable from a silently-altered one."
    ;;
esac

FENCE='^[[:space:]]*---[[:space:]]*$'
OWNED='^[[:space:]]*(id|kind|status|revision|relations|format)[[:space:]]*:'

for FIELD in old_string new_string; do
  VALUE="$(aep_field "tool_input.$FIELD")"
  [ -n "$VALUE" ] || continue
  if printf '%s\n' "$VALUE" | grep -Eq "$FENCE"; then
    aep_deny "$HOOK" "$CAPABILITY" \
      "the edit's \`$FIELD\` crosses the \`---\` frontmatter fence of $TARGET. Edit only below the closing fence; the frontmatter is the CLI's."
  fi
  if printf '%s\n' "$VALUE" | grep -Eq "$OWNED"; then
    KEY="$(printf '%s\n' "$VALUE" | grep -Eom1 "$OWNED" | tr -d ' \t:')"
    aep_deny "$HOOK" "$CAPABILITY" \
      "the edit's \`$FIELD\` writes the machine-owned field \`$KEY\` of $TARGET. \`status\` moves only through \`protocol artifact move\`, which validates the move against the kind's lifecycle; \`id\`, \`kind\`, \`revision\`, \`relations\` and \`format\` are written by \`protocol artifact new\` and \`protocol artifact relate\`. A hand-edited status is an unvalidated one."
  fi
done

aep_allow "$HOOK" "$CAPABILITY" "a targeted body edit of $TARGET, clear of the frontmatter fence and of every machine-owned field"
