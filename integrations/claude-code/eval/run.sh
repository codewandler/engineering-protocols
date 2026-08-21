#!/usr/bin/env bash
# A repeatable evaluation of the planning plugin.
#
# What one run does, end to end:
#   1. builds `protocol` and puts it on PATH;
#   2. creates a scratch directory (under $TMPDIR, never /tmp) holding a copy of the
#      `examples/planning-passkeys` project scaffold with an EMPTY planning store, and a copy of
#      this plugin — the scratch directory is self-contained and survives the run for inspection;
#   3. drops a headless Claude agent (its own process, `claude -p`) into that project with the
#      plugin loaded and a fixed dummy task (eval/prompt.md);
#   4. mechanically inspects what the agent created, and prints a verdict table.
#
# This eval talks to the Claude API: it costs money and needs network, which is why it is not —
# and must never be — part of `task check`. Run it with `task plugin-eval` or directly.
#
# Environment overrides: EVAL_MODEL (default sonnet), EVAL_MAX_TURNS (default 30).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PLUGIN_SRC="$REPO/integrations/claude-code"
FIXTURE_SRC="$REPO/examples/planning-passkeys"
MODEL="${EVAL_MODEL:-sonnet}"
MAX_TURNS="${EVAL_MAX_TURNS:-30}"

say() { printf '%s\n' "$*"; }

# ---- 0. preconditions -------------------------------------------------------------------------
command -v claude >/dev/null || { say "FAIL: \`claude\` is not on PATH"; exit 1; }
[ -d "$FIXTURE_SRC/.engineering" ] || {
  say "FAIL: $FIXTURE_SRC/.engineering does not exist (the fixture the scaffold is copied from)"
  exit 1
}

say "building protocol-cli …"
(cd "$REPO" && cargo build -p protocol-cli --quiet)
export PATH="$REPO/target/debug:$PATH"
command -v protocol >/dev/null || { say "FAIL: protocol binary missing after build"; exit 1; }

# ---- 1. scratch directory ---------------------------------------------------------------------
# Never /tmp: this machine's tmpfs drops writes under pressure; TMPDIR points at a safe cache.
SCRATCH_BASE="${TMPDIR:-$HOME/.cache/claude-tmp}"
mkdir -p "$SCRATCH_BASE"
WORK="$(mktemp -d "$SCRATCH_BASE/plugin-eval.XXXXXX")"
say "scratch directory: $WORK"

# The project the agent works in: the fixture's scaffold (project.yaml, vendored document tree)
# with the planning store emptied — the agent must create the artifacts, not find them.
PROJECT="$WORK/project"
mkdir -p "$PROJECT"
cp -R "$FIXTURE_SRC/." "$PROJECT/"
rm -rf "$PROJECT/.engineering/planning"
mkdir -p "$PROJECT/.engineering/planning"
# The document tree the CLI validates against (lifecycles for status moves, templates for `new`).
mkdir -p "$PROJECT/artifacts"
cp -R "$REPO/artifacts/lifecycles" "$PROJECT/artifacts/lifecycles"
cp -R "$REPO/artifacts/templates" "$PROJECT/artifacts/templates"

# The plugin, copied in as a local plugin so the scratch directory is the whole experiment.
# (eval/ excluded: the eval must not carry itself into its own fixture.)
mkdir -p "$WORK/plugin"
(cd "$PLUGIN_SRC" && tar --exclude=./eval -cf - .) | (cd "$WORK/plugin" && tar -xf -)

# ---- 2. the agent, headless -------------------------------------------------------------------
# An exported ANTHROPIC_API_KEY takes precedence over the claude.ai login and may point at an
# account with no credits; the eval bills the logged-in account unless EVAL_USE_API_KEY=1 says
# otherwise.
if [ "${EVAL_USE_API_KEY:-0}" != "1" ]; then unset ANTHROPIC_API_KEY; fi
PROMPT="$(cat "$SCRIPT_DIR/prompt.md")"
say "running claude -p (model $MODEL, max $MAX_TURNS turns) …"
AGENT_EXIT=0
(cd "$PROJECT" && claude -p "$PROMPT" \
  --plugin-dir "$WORK/plugin" \
  --model "$MODEL" \
  --max-turns "$MAX_TURNS" \
  --permission-mode dontAsk \
  --allowedTools "Bash,Read,Write,Edit,Glob,Grep,Skill,Task,TodoWrite" \
  --output-format stream-json --verbose \
  > "$WORK/result.jsonl" 2> "$WORK/stderr.log") || AGENT_EXIT=$?
say "agent exit: $AGENT_EXIT (transcript: $WORK/result.jsonl)"

# ---- 3. mechanical inspection -----------------------------------------------------------------
STORE="$PROJECT/.engineering/planning"
PASS=0
FAIL=0
declare -a ROWS

check() { # check <name> <0-for-pass>  (never aborts the script: the report must always print)
  if [ "$2" -eq 0 ]; then PASS=$((PASS + 1)); ROWS+=("PASS  $1"); else FAIL=$((FAIL + 1)); ROWS+=("FAIL  $1"); fi
}

# 3.1 the store validates (lifecycle-legal statuses, graph builds, frontmatter parses)
VALIDATE_OUT="$(cd "$PROJECT" && protocol artifact validate --store "$STORE" 2>&1)" && V=0 || V=$?
check "protocol artifact validate exits 0" "$V"

# 3.2 at least one epic, at least two stories
EPICS=$(find "$STORE/epic" -name '*.md' 2>/dev/null | wc -l)
STORIES=$(find "$STORE/story" -name '*.md' 2>/dev/null | wc -l)
R=1; [ "$EPICS" -ge 1 ] && R=0; check "≥1 epic created ($EPICS found)" "$R"
R=1; [ "$STORIES" -ge 2 ] && R=0; check "≥2 stories created ($STORIES found)" "$R"

# 3.3 every story relates back to an epic (derived_from / decomposes edge in its frontmatter)
UNLINKED=0
while IFS= read -r f; do
  grep -Eq '^[[:space:]]*-[[:space:]]*(derived_from|decomposes):[[:space:]]*epic:' "$f" || UNLINKED=$((UNLINKED + 1))
done < <(find "$STORE/story" -name '*.md' 2>/dev/null)
R=1; [ "$UNLINKED" -eq 0 ] && R=0; check "every story carries an epic relation ($UNLINKED unlinked)" "$R"

# 3.4 the agent used the CLI to create artifacts, not hand-written frontmatter
R=1; grep -q 'protocol artifact new' "$WORK/result.jsonl" && R=0; check "transcript shows protocol artifact new" "$R"

# ---- 4. report --------------------------------------------------------------------------------
say ""
say "== verdict ($PASS pass, $FAIL fail) =="
for row in "${ROWS[@]}"; do say "  $row"; done
say ""
say "== created files =="
(cd "$PROJECT" && find .engineering/planning -name '*.md' | sort)
say ""
say "== protocol artifact list =="
(cd "$PROJECT" && protocol artifact list --store "$STORE" 2>&1) || true
say ""
say "== validate output =="
say "$VALIDATE_OUT"
say ""
COST="$(grep '"type":"result"' "$WORK/result.jsonl" | tail -1 | grep -o '"total_cost_usd":[0-9.]*' | cut -d: -f2 || true)"
[ -n "$COST" ] && COST="$(printf '%.2f' "$COST")"
say "cost: \$${COST:-unknown}   transcript: $WORK/result.jsonl"
say "inspect the run yourself: $WORK"
[ "$FAIL" -eq 0 ]
