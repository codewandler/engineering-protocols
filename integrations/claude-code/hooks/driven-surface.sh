#!/usr/bin/env bash
# The per-state tool surface, enforced one level below the launch flag.
#
# `PreToolUse`, matcher `Bash`. `--allowedTools` governs which tools are *offered* and is fixed at
# session launch; this hook governs what one of them is allowed to *say*, and it is the only layer
# that sees a tool's arguments. Two layers with different failure modes, which is § 4.8's
# enforce-and-verify argument applied one level down rather than belt-and-braces.
#
# ## Inert outside a driven run, on purpose
#
# With no step context on disk this hook returns silently. § 4.8 is explicit that the plugin's
# hooks are the *driver's* enforcement arm — configured by the driver, per state, for a run that is
# holding an execution — and that a plugin installed without the driver ships none. A per-state
# rule with no state to read is exactly the "second, weaker driver" § 3.6 refused, so it declines to
# be one.
#
# ## Why a shell exists at all in a development run, and what bounds it
#
# The design's § 4.8 states, correctly for the two profiles that existed when it was written, that
# **no development profile grants `command.execute`**, so a driven `llm` step holds no shell. That
# is a strong property and it has one consequence nobody costed: the planning skill's entire surface
# is `protocol artifact …`, and every one of those verbs is a shell command. Under
# `development.fast` or `development.standard` a driven `llm` step cannot reach a single one of
# them — it can be told to write a specification as an artifact and has no way to create one.
#
# The capability grammar cannot express the narrow grant that would fix it: scoping exists only for
# `Environment` on `deployment.create` and `deployment.rollback` (`crates/aep-domain/src/capability.rs`),
# so `command.execute:protocol` is a parse error, not a capability. The resolution is therefore the
# other half of § 4.8's own shape — **a capability grant plus a hook constraint**:
#
#   * `profiles/development-driven.yaml` grants `command.execute` and says in its own text that the
#     grant exists so the `protocol` CLI is reachable and for no other reason;
#   * this hook is the constraint, and it holds the grant to that surface.
#
# The approval floor is untouched: `command.execute` is not in `protocols/aep/1.yaml`'s
# `approval_floor`, so nothing that needed an approval before needs one less now.
#
# ## The surface, and why it is declared here and not in the run context
#
# One simple invocation of the `protocol` CLI's `artifact` or `trace` verbs. No pipes, no
# redirection, no `&&`, no command substitution — a composed command line is a second command
# wearing the first one's name, and admitting one would make the whole check a suggestion.
#
# The list lives in this file and **not** in the context document the driver writes, deliberately:
# a run that could name its own allowed surface could widen it, and the widening would be a route
# around the constraint rather than a check on it. The surface is a property of what the `protocol`
# CLI is, not of any one run. Per § 4.8 this remains **pattern-based and best-effort** — granting
# `command.execute` grants a superset of the shell's reach, and this hook narrows it rather than
# making it a function of a capability.
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
. "$DIR/lib.sh"

AEP_PAYLOAD="$(cat)"
HOOK=driven-surface
CAPABILITY=command.execute

aep_load_context || aep_pass

if [ -z "$(aep_parser)" ]; then
  aep_deny "$HOOK" "$CAPABILITY" \
    "this run is driven and neither \`jq\` nor \`python3\` is on PATH, so the per-state surface check cannot read the command it is asked about. It refuses rather than passing an unread shell command through."
fi

if [ "$(aep_context_field shell_offered)" != "true" ]; then
  aep_deny "$HOOK" "$CAPABILITY" \
    "state \`$(aep_context_field state)\` does not admit \`command.execute\`, so this step holds no shell. Anything a suite must observe is run by the driver as a \`command\` step and recorded with a verifier's provenance, not with yours."
fi

COMMAND="$(aep_field tool_input.command)"

case "$COMMAND" in
  *';'*|*'&'*|*'|'*|*'`'*|*'$('*|*'>'*|*'<'*|*$'\n'*)
    aep_deny "$HOOK" "$CAPABILITY" \
      "the command composes or redirects, and this run admits one simple invocation at a time: \`$COMMAND\`. Run the \`protocol\` verbs one call per Bash tool use."
    ;;
esac

set -f
# shellcheck disable=SC2086
set -- $COMMAND
PROGRAM="${1:-}"
VERB="${2:-}"

case "${PROGRAM##*/}" in
  protocol) ;;
  *)
    aep_deny "$HOOK" "$CAPABILITY" \
      "\`${PROGRAM:-(nothing)}\` is outside the surface this state admits. A driven step's shell exists so the \`protocol\` CLI is reachable; it is not a general shell. Build, test and inspection commands are \`command\` steps the driver runs, and their records carry a verifier's provenance rather than yours."
    ;;
esac

case "$VERB" in
  artifact|trace) ;;
  *)
    aep_deny "$HOOK" "$CAPABILITY" \
      "\`protocol ${VERB:-(no verb)}\` is outside the surface this state admits: \`protocol artifact …\` and \`protocol trace …\`. Driving a run from inside a driven step, or moving the store's own governing documents, is not this step's business."
    ;;
esac

aep_allow "$HOOK" "$CAPABILITY" "protocol $VERB, one simple invocation, inside the admitted surface"
