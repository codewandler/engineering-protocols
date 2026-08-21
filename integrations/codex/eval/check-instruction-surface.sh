#!/usr/bin/env bash
# Does the Codex variant's instruction surface reach the model, and is it well-formed?
#
# Three checks, none of which calls an API, spends a token or reaches the network. That is the whole
# claim: this is not the Claude Code plugin's `eval/run.sh` — there is no live run here, no agent
# behaviour is observed and nothing is judged about how well the instructions work. What is checked
# is the part that is checkable for free, and the boundary is stated rather than blurred:
#
#   1. the plugin manifest and the skill's frontmatter satisfy Codex's own plugin validator;
#   2. a project carrying this variant renders the skill and the `AGENTS.md` text into the
#      model-visible prompt — `codex debug prompt-input` builds that prompt locally and prints it;
#   3. the fixture is isolated from the operator's own Codex home, so (2) is a fact about the files
#      under test rather than about this machine.
#
# Dependencies are named and are never skipped past. A machine without `codex` fails this script
# with a row saying so, because a check that quietly passes without its subject reads exactly like a
# check that passed — the same rule `task check` holds for the Go toolchain.
#
# What it cannot say is in eval/README.md § *What this does not check*.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VARIANT="$(cd "$SCRIPT_DIR/.." && pwd)"

say() { printf '%s\n' "$*"; }

PASS=0
FAIL=0
declare -a ROWS
check() { # check <name> <0-for-pass>
  if [ "$2" -eq 0 ]; then
    PASS=$((PASS + 1))
    ROWS+=("PASS  $1")
  else
    FAIL=$((FAIL + 1))
    ROWS+=("FAIL  $1")
  fi
}

# ---- 0. dependencies, each a row rather than an early exit -------------------------------------
command -v codex >/dev/null 2>&1 && R=0 || R=1
check "\`codex\` is on PATH" "$R"
CODEX_PRESENT=$R

command -v python3 >/dev/null 2>&1 && R=0 || R=1
check "\`python3\` is on PATH" "$R"
PYTHON_PRESENT=$R

# Codex's own plugin validator, shipped inside the CLI and materialized into the Codex home as a
# system skill. It is the vendor's definition of a well-formed plugin, which is why it is used
# instead of a hand-written schema check that would be this repository's opinion of one.
VALIDATOR="${CODEX_HOME:-$HOME/.codex}/skills/.system/plugin-creator/scripts/validate_plugin.py"
[ -f "$VALIDATOR" ] && R=0 || R=1
check "Codex's plugin validator is present ($VALIDATOR)" "$R"
VALIDATOR_PRESENT=$R

# ---- 1. the manifest and the skill frontmatter --------------------------------------------------
if [ "$PYTHON_PRESENT" -eq 0 ] && [ "$VALIDATOR_PRESENT" -eq 0 ]; then
  VALIDATE_OUT="$(python3 "$VALIDATOR" "$VARIANT" 2>&1)" && R=0 || R=$?
  check "plugin validation passes on $VARIANT" "$R"
  [ "$R" -eq 0 ] || say "$VALIDATE_OUT"
else
  check "plugin validation passes (skipped: dependency missing)" 1
fi

# ---- 2. the instruction surface, rendered into a prompt ------------------------------------------
if [ "$CODEX_PRESENT" -eq 0 ]; then
  # Never /tmp: this machine's tmpfs drops writes under pressure; TMPDIR points at a safe cache.
  SCRATCH_BASE="${TMPDIR:-$HOME/.cache/claude-tmp}"
  mkdir -p "$SCRATCH_BASE"
  WORK="$(mktemp -d "$SCRATCH_BASE/codex-surface.XXXXXX")"
  PROJECT="$WORK/project"
  HOME_DIR="$WORK/codex-home"
  mkdir -p "$PROJECT/.agents/skills" "$HOME_DIR"

  # A git repository, because Codex resolves a project root and its `AGENTS.md` walk from one.
  git -C "$PROJECT" init -q .

  cp -R "$VARIANT/skills/planning" "$PROJECT/.agents/skills/planning"
  cp "$VARIANT/AGENTS.planning.md" "$PROJECT/AGENTS.md"

  # A scratch `CODEX_HOME` is the analogue of the Claude eval's scratch `CLAUDE_CONFIG_DIR`: without
  # it the operator's own skills are listed beside the one under test, and check 3 below would be
  # measuring this machine. No credential is copied in — `debug prompt-input` renders the prompt
  # locally and needs none.
  PROMPT_INPUT="$WORK/prompt-input.json"
  (cd "$PROJECT" && CODEX_HOME="$HOME_DIR" codex debug prompt-input "plan this" \
    >"$PROMPT_INPUT" 2>"$WORK/stderr.log") && R=0 || R=$?
  check "\`codex debug prompt-input\` ran in the fixture (exit $R)" "$R"

  if [ "$R" -eq 0 ]; then
    grep -q "planning: Plan engineering work in a governed markdown artifact store" "$PROMPT_INPUT" && R=0 || R=1
    check "the planning skill is offered to the model, by name and description" "$R"

    grep -q "AGENTS.md instructions for $PROJECT" "$PROMPT_INPUT" && R=0 || R=1
    check "the project's AGENTS.md is injected as instructions" "$R"

    grep -q "A status changes only through .protocol artifact move" "$PROMPT_INPUT" && R=0 || R=1
    check "guardrail 1 is in the model-visible prompt without anything being invoked" "$R"

    # ---- 3. isolation: every offered skill comes from the fixture or the scratch home -----------
    FOREIGN=0
    while IFS= read -r locator; do
      case "$locator" in
        "$PROJECT"/*|"$HOME_DIR"/*) ;;
        *) FOREIGN=$((FOREIGN + 1)); say "foreign skill locator: $locator" ;;
      esac
    done < <(grep -oE '\(file: [^)]*\)' "$PROMPT_INPUT" | sed -E 's/^\(file: //; s/\)$//')
    R=1; [ "$FOREIGN" -eq 0 ] && R=0
    check "no skill leaked in from outside the fixture ($FOREIGN foreign)" "$R"
  fi

  say "scratch directory: $WORK"
else
  check "the instruction surface reaches the model (skipped: no codex)" 1
fi

# ---- report -------------------------------------------------------------------------------------
say ""
say "── verdict ───────────────────────────────────────────────"
for row in "${ROWS[@]}"; do say "$row"; done
say "──────────────────────────────────────────────────────────"
say "$PASS pass, $FAIL fail"
[ "$FAIL" -eq 0 ] || exit 1
