# Shared machinery for this plugin's `PreToolUse` hooks. Sourced, never executed.
#
# Three jobs, and nothing else: read one field out of the hook payload, find the driven run this
# session belongs to (if any), and emit a decision — to Claude Code on stdout, and to the run's
# append-only decision log on disk.
#
# ## Why a decision log at all
#
# A `PreToolUse` hook is a separate process with JSON on stdin. It cannot call `Engine::authorize`,
# which takes `&mut Execution` — an in-memory value inside the driver's process, whose mutation is
# the point. A hook that shelled out to a `protocol authorize` would build a *different* execution,
# emit its events into that one, and drop them on exit; the driver's snapshot would never see them.
# So the hook writes its decision to `<run>/hook-decisions.jsonl` and the driver folds each line in
# after the step's process exits. Decisions land a moment late and they land in the real trail.
# (Design § 4.8, "How a hook's decision reaches the audit trail — decided, per F14".)
#
# ## Why a JSON parser is required, and what happens when there is none
#
# Every rule here is a rule about a tool *argument* — a path, a command string — so the payload has
# to be parsed. `jq` is preferred and `python3` is the fallback. When neither exists the hook
# **denies** rather than passing the call through: a guard that silently stops guarding is the
# defect this repository writes registers about. The blast radius is bounded by each hook's own
# cheap pre-filter, which decides *without parsing* whether the call is one this layer adjudicates
# at all — so a machine with neither parser loses nothing except the calls the guard exists for.

# The payload, read once. Every helper below reads this variable, never stdin.
AEP_PAYLOAD=""

# Which JSON reader is available: `jq`, `python3`, or nothing.
aep_parser() {
  if command -v jq >/dev/null 2>&1; then printf 'jq'
  elif command -v python3 >/dev/null 2>&1; then printf 'python3'
  else printf ''
  fi
}

# One field out of a JSON document, by dot path. `aep_read <json> <a.b.c>`.
#
# Prints the value as a bare string (a JSON `null` or a missing key prints nothing) and returns 0,
# or returns 1 when there is no parser. Scalars only: nothing here reads an array.
aep_read() {
  local document="$1" path="$2" parser
  parser="$(aep_parser)"
  case "$parser" in
    jq)
      printf '%s' "$document" | jq -r --arg p "$path" \
        'try (getpath($p | split(".")) | if . == null then "" else tostring end) catch ""' 2>/dev/null
      ;;
    python3)
      printf '%s' "$document" | AEP_PATH="$path" python3 -c '
import json, os, sys
try:
    node = json.load(sys.stdin)
except Exception:
    sys.exit(0)
for key in os.environ["AEP_PATH"].split("."):
    if isinstance(node, dict) and key in node:
        node = node[key]
    else:
        node = None
        break
if node is None:
    pass
elif isinstance(node, bool):
    sys.stdout.write("true" if node else "false")
elif isinstance(node, (str, int, float)):
    sys.stdout.write(str(node))
else:
    sys.stdout.write(json.dumps(node))
' 2>/dev/null
      ;;
    *) return 1 ;;
  esac
}

# A field of the hook payload.
aep_field() { aep_read "$AEP_PAYLOAD" "$1"; }

# One string, escaped as a JSON string literal *including* its quotes.
aep_json_string() {
  local parser
  parser="$(aep_parser)"
  case "$parser" in
    jq) printf '%s' "$1" | jq -Rs . ;;
    python3) printf '%s' "$1" | python3 -c 'import json,sys; sys.stdout.write(json.dumps(sys.stdin.read()))' ;;
    *) printf '"<unencodable>"' ;;
  esac
}

# ---------------------------------------------------------------------------------------------
# The driven run this session belongs to, if it belongs to one
# ---------------------------------------------------------------------------------------------

# Where the step context is, or nothing when this session is not part of a driven run.
#
# Three routes, most-specific first. The environment variable is what the driver exports onto the
# `claude` child, and hook processes inherit the environment of the process that launched them. The
# two file routes exist because that inheritance is the only undocumented link in the chain: a
# session that lost the variable can still find its own run through the store-level `current`
# pointer the driver writes, which is a fact on disk rather than an assumption about process trees.
aep_context_path() {
  if [ -n "${AEP_DRIVE_STEP_CONTEXT:-}" ] && [ -r "${AEP_DRIVE_STEP_CONTEXT}" ]; then
    printf '%s' "$AEP_DRIVE_STEP_CONTEXT"
    return 0
  fi
  local root current
  for root in "${CLAUDE_PROJECT_DIR:-}" "$(aep_field cwd)" "$PWD"; do
    [ -n "$root" ] || continue
    current="$root/.engineering/runs/current"
    [ -r "$current" ] || continue
    local run
    run="$(tr -d '\n' <"$current")"
    [ -n "$run" ] || continue
    if [ -r "$root/.engineering/runs/$run/step-context.json" ]; then
      printf '%s' "$root/.engineering/runs/$run/step-context.json"
      return 0
    fi
  done
  printf ''
}

# The step context document, or nothing.
AEP_CONTEXT=""
aep_load_context() {
  local path
  path="$(aep_context_path)"
  [ -n "$path" ] || { AEP_CONTEXT=""; return 1; }
  AEP_CONTEXT="$(cat "$path" 2>/dev/null || printf '')"
  [ -n "$AEP_CONTEXT" ]
}

# A field of the step context.
aep_context_field() { [ -n "$AEP_CONTEXT" ] && aep_read "$AEP_CONTEXT" "$1"; }

# ---------------------------------------------------------------------------------------------
# Deciding
# ---------------------------------------------------------------------------------------------

# Appends one decision to the run's log, when there is a run to append it to.
#
# `aep_log <hook> <decision> <capability> <reason>`. Silent and best-effort by design: a hook that
# failed to write its own audit line must still return its decision, because the decision is the
# enforcement and the line is only the record of it.
aep_log() {
  local directory
  directory="$(aep_context_field run_directory)"
  [ -n "$directory" ] && [ -d "$directory" ] || return 0
  printf '{"format":"aep.hook-decision/1","hook":%s,"event":"PreToolUse","tool":%s,"tool_use_id":%s,"decision":%s,"capability":%s,"reason":%s,"state":%s,"step":%s,"attempt":%s,"session":%s}\n' \
    "$(aep_json_string "$1")" \
    "$(aep_json_string "$(aep_field tool_name)")" \
    "$(aep_json_string "$(aep_field tool_use_id)")" \
    "$(aep_json_string "$2")" \
    "$(aep_json_string "$3")" \
    "$(aep_json_string "$4")" \
    "$(aep_json_string "$(aep_context_field state)")" \
    "$(aep_json_string "$(aep_context_field step_index)")" \
    "$(aep_json_string "$(aep_context_field attempt)")" \
    "$(aep_json_string "$(aep_field session_id)")" \
    >>"$directory/hook-decisions.jsonl" 2>/dev/null || true
}

# Refuses the call, with the reason fed back to the model.
#
# The JSON form rather than exit 2: both deny deterministically, and only this one carries a reason
# the model is told, which is the difference between a wall and an instruction.
aep_deny() {
  local hook="$1" capability="$2" reason="$3"
  aep_log "$hook" deny "$capability" "$reason"
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":%s}}\n' \
    "$(aep_json_string "$reason")"
  exit 0
}

# Lets the call through, recording that this layer looked at it.
#
# A hook can deny and can never grant, so this emits no `permissionDecision` at all: saying `allow`
# here would claim an authority the layer does not have and would override a stricter rule
# elsewhere. Silence is the correct shape of "I have no objection".
aep_allow() {
  aep_log "$1" allow "$2" "$3"
  exit 0
}

# Passes the call through without adjudicating it and without a log line.
aep_pass() { exit 0; }
