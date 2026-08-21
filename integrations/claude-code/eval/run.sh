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

# A scratch config home, so the operator's own plugins, skills and output style cannot leak into
# the run (observed before this existed: 5 foreign plugins and the user's output style, visible in
# the init event). Only the login credentials are carried over — auth is the one thing the run
# must share with the operator, and the only thing it does.
mkdir -p "$WORK/claude-home"
if [ -f "$HOME/.claude/.credentials.json" ]; then
  cp "$HOME/.claude/.credentials.json" "$WORK/claude-home/.credentials.json"
else
  say "note: no ~/.claude/.credentials.json — relying on ANTHROPIC_API_KEY or another auth source"
fi

# ---- 2. the agent, headless -------------------------------------------------------------------
# An exported ANTHROPIC_API_KEY takes precedence over the claude.ai login and may point at an
# account with no credits; the eval bills the logged-in account unless EVAL_USE_API_KEY=1 says
# otherwise.
if [ "${EVAL_USE_API_KEY:-0}" != "1" ]; then unset ANTHROPIC_API_KEY; fi
PROMPT="$(cat "$SCRIPT_DIR/prompt.md")"
say "running claude -p (model $MODEL, max $MAX_TURNS turns) …"
AGENT_EXIT=0
(cd "$PROJECT" && CLAUDE_CONFIG_DIR="$WORK/claude-home" claude -p "$PROMPT" \
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

# 3.5 the skill actually completed — the Skill tool's structured result, not text matching
R=1
if command -v jq >/dev/null; then
  jq -e 'select(.tool_use_result.commandName=="engineering-protocols:planning")
         | select(.tool_use_result.success==true)' "$WORK/result.jsonl" >/dev/null 2>&1 && R=0
else
  grep -q '"commandName":"engineering-protocols:planning"' "$WORK/result.jsonl" && R=0
fi
check "planning skill completed (Skill result success=true)" "$R"

# 3.6 clean terminal record: no API error, no permission denials (the sandbox contract held)
R=1
if command -v jq >/dev/null; then
  TERM_STATE="$(jq -r 'select(.type=="result")
    | (.is_error|tostring)+" "+(.permission_denials|length|tostring)' "$WORK/result.jsonl" | tail -1)"
  [ "$TERM_STATE" = "false 0" ] && R=0
else
  grep -q '"is_error":false' "$WORK/result.jsonl" && R=0
fi
check "terminal record clean (no error, 0 permission denials)" "$R"

# 3.7 the environment is hermetic: exactly the eval's plugin loaded, nothing from the operator
R=1
if command -v jq >/dev/null; then
  PLUGINS="$(jq -r 'select(.type=="system" and .subtype=="init")
    | [.plugins[].name] | sort | join(",")' "$WORK/result.jsonl" | tail -1)"
  [ "$PLUGINS" = "engineering-protocols" ] && R=0
else
  R=0  # cannot inspect without jq; do not fail on missing tooling
fi
check "hermetic: only engineering-protocols loaded (saw: ${PLUGINS:-unknown})" "$R"

# 3.8 the run authenticated the way the eval intends (login, not a stray API key) — the check
# that would have caught the ANTHROPIC_API_KEY misfire before a single turn was spent
if [ "${EVAL_USE_API_KEY:-0}" != "1" ]; then
  R=1
  if command -v jq >/dev/null; then
    SRC="$(jq -r 'select(.type=="system" and .subtype=="init") | .apiKeySource' "$WORK/result.jsonl" | tail -1)"
    [ "$SRC" = "none" ] && R=0
  else
    R=0
  fi
  check "auth is the login (api-key-source: ${SRC:-unknown})" "$R"
fi

# ---- 4. metrics -------------------------------------------------------------------------------
# Informational, never asserted: numbers vary run to run (see README on variance); a missing field
# prints nothing rather than failing the eval. Written to a file so the adversarial reviewer
# (section 5) reads exactly what the report prints.
if command -v jq >/dev/null; then
  {
    jq -r 'select(.type=="system" and .subtype=="init") |
      "environment  model=\(.model)  claude-code=\(.claude_code_version)  api-key-source=\(.apiKeySource)  permission-mode=\(.permissionMode)",
      "plugins      \(.plugins | map(.name+"@"+.version) | join(", "))"' "$WORK/result.jsonl" || true
    API_REQS="$(jq -r 'select(.type=="assistant") | .request_id' "$WORK/result.jsonl" | sort -u | wc -l)"
    ASSISTANT_EVENTS="$(jq -r 'select(.type=="assistant") | .type' "$WORK/result.jsonl" | wc -l)"
    jq -r --arg reqs "$API_REQS" --arg aev "$ASSISTANT_EVENTS" 'select(.type=="result") |
      "turns        \(.num_turns) turns, \($reqs) api requests, \($aev) assistant events, \(.usage.iterations // [] | length) iterations",
      "tokens       in=\(.usage.input_tokens) out=\(.usage.output_tokens) thinking=\(.usage.output_tokens_details.thinking_tokens // 0)",
      "cache        read=\(.usage.cache_read_input_tokens) created=\(.usage.cache_creation_input_tokens) hit-ratio=\(
        if (.usage.cache_read_input_tokens + .usage.input_tokens) > 0
        then ((.usage.cache_read_input_tokens / (.usage.cache_read_input_tokens + .usage.input_tokens) * 100) | floor | tostring) + "%"
        else "n/a" end)",
      "latency      ttft=\(.ttft_ms // "?")ms  time-to-request=\(.time_to_request_ms // "?")ms  total=\(.duration_ms)ms  api=\(.duration_api_ms)ms"' \
      "$WORK/result.jsonl" || true
    jq -r 'select(.type=="rate_limit_event") | .rate_limit_info |
      "rate-limit   status=\(.status)  utilization=\(.utilization)  overage=\(.isUsingOverage)"' \
      "$WORK/result.jsonl" | tail -1 || true
    # Tool traffic: what each call cost the context window (result bytes land in the next request's
    # input; ~4 bytes per token), whether calls failed, and whether identical calls were repeated.
    jq -rs '
      ([.[] | select(.type=="assistant") | .message.content[]? | select(.type=="tool_use")
        | {id, name, inb: (.input|tostring|length), key: (.name+" "+(.input|tostring))}]) as $uses
      | ([.[] | select(.type=="user") | .message.content[]? | select(.type=="tool_result")
        | {id: .tool_use_id, outb: (.content|tostring|length), err: (.is_error // false)}]) as $res
      | ($uses | map(. as $u
          | ($res | map(select(.id == $u.id)) | first) as $r
          | {name: $u.name, inb: $u.inb, key: $u.key,
             outb: ($r.outb // 0), err: ($r.err // false)})) as $calls
      | ($calls | group_by(.name) | map({name: .[0].name, n: length,
           errs: (map(select(.err)) | length),
           inb: (map(.inb) | add), outb: (map(.outb) | add)})
         | .[] | "tool         \(.name): \(.n) call(s), \(.errs) error(s), in \(.inb)B, results \(.outb)B (~\((.outb/4)|floor) tokens)"),
        "tools-total  \($calls|length) calls, \($calls | map(select(.err)) | length) failed, results \($calls | map(.outb) | add // 0)B (~\((($calls | map(.outb) | add // 0)/4)|floor) tokens into context)",
        "repeated     \($calls | group_by(.key) | map(select(length > 1)) | length) identical call group(s)"
    ' "$WORK/result.jsonl" || true
    # Per-step timing, derived from recorded event timestamps (never measured here): `gen` is the
    # inference interval ending at the event that carries the tool call — the time the model took
    # to produce it — and `exec` is call-issued to result-back.
    jq -rs '
      def ts: if . == null then null
        else (sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601 * 1000)
             + ((capture("\\.(?<ms>[0-9]{1,3})")?.ms // "0" | tonumber)) end;
      ([.[] | select(.timestamp != null)
        | {t: (.timestamp | ts), type,
           uses: [.message.content[]? | select(.type=="tool_use") | {id, name}],
           rids: [.message.content[]? | select(.type? == "tool_result") | .tool_use_id]}]) as $ev
      | ([range(0; $ev|length) as $i | $ev[$i] as $e
          | ($e.uses[]? | . as $u
             | ([$ev[] | select(.rids | index($u.id))] | first) as $r
             | {name: $u.name,
                gen: (if $i > 0 and $e.t != null and $ev[$i-1].t != null then $e.t - $ev[$i-1].t else null end),
                exec: (if $r != null and $r.t != null and $e.t != null then $r.t - $e.t else null end)})]) as $steps
      | ($steps | to_entries | .[] |
          "step         \(.key + 1). \(.value.name): gen \(.value.gen // "?")ms, exec \(.value.exec // "?")ms"),
        "time-split   inference \([$steps[].gen // 0] | add)ms across \($steps|length) steps, tool-exec \([$steps[].exec // 0] | add)ms"
    ' "$WORK/result.jsonl" || true
  } > "$WORK/metrics.txt" 2>/dev/null || true
fi

# ---- 5. adversarial review (advisory — never gates the verdict) -------------------------------
# A second, independent headless session reads the task, the mechanical verdict, the metrics and
# a summarized timeline, and reviews the run adversarially. Its opinion is printed and saved,
# and deliberately never touches the exit code: an LLM's judgement is not a deterministic check,
# and this eval's authority stays with the assertions above.
if command -v jq >/dev/null && [ "${EVAL_SKIP_REVIEW:-0}" != "1" ]; then
  jq -rs '
    def ts: if . == null then null
      else (sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601 * 1000)
           + ((capture("\\.(?<ms>[0-9]{1,3})")?.ms // "0" | tonumber)) end;
    ([.[] | select(.timestamp != null)
      | {t: (.timestamp | ts), type, content: (.message.content // []),
         rids: [.message.content[]? | select(.type? == "tool_result") | .tool_use_id]}]) as $ev
    | [range(0; $ev|length) as $i | $ev[$i] as $e
       | (if $e.type == "assistant" then
            ($e.content[] |
              if .type == "text" then "AGENT: \(.text | gsub("\n"; " ") | .[0:220])"
              elif .type == "tool_use" then
                (. as $u
                 | ([$ev[] | select(.rids | index($u.id))] | first) as $r
                 | (if $i > 0 and $e.t != null and $ev[$i-1].t != null
                    then "gen \($e.t - $ev[$i-1].t)ms" else "gen ?" end) as $g
                 | (if $r != null and $r.t != null and $e.t != null
                    then ", exec \($r.t - $e.t)ms" else "" end) as $x
                 | "TOOL \($u.name) [\($g)\($x)]: \($u.input | tostring | gsub("\n"; " ") | .[0:300])")
              else empty end)
          elif $e.type == "user" then
            ($e.content[] | select(.type? == "tool_result")
             | "  -> result \(.content | tostring | length)B\(if .is_error then " ERROR" else "" end): \(.content | tostring | gsub("\n"; " ") | .[0:150])")
          else empty end)]
    | .[]' "$WORK/result.jsonl" 2>/dev/null | head -c 28000 > "$WORK/timeline.txt" || true

  {
    printf '%s\n\n' "You are an adversarial reviewer of one evaluation run of a Claude Code plugin. You get the task the agent was given, the mechanical verdict, the run metrics, and a summarized timeline. The mechanical assertions only check outcomes; your job is what they cannot see: did the agent follow the plugin's rules in spirit, not just to the letter? Were there wasted, repeated or failing tool calls, and what do failures say about how well the agent understood the tooling? Any risky idiom (for example rewriting whole files where a targeted edit was safer, or touching machine-owned frontmatter)? Be specific: every finding cites a timeline line. At most six findings, most severe first; no praise, no filler. End with exactly one line: 'ADVISORY: sound' or 'ADVISORY: concerns — <one line>'."
    printf '== the task ==\n%s\n\n' "$PROMPT"
    printf '== mechanical verdict (%s pass, %s fail) ==\n' "$PASS" "$FAIL"
    for row in "${ROWS[@]}"; do printf '%s\n' "$row"; done
    printf '\n== metrics ==\n'; cat "$WORK/metrics.txt" 2>/dev/null || true
    printf '\n== timeline ==\n'; cat "$WORK/timeline.txt" 2>/dev/null || true
    # The artifacts themselves, verbatim, so claims about their content (acceptance statements,
    # untouched frontmatter) are verifiable rather than guessed from truncated timeline excerpts.
    printf '\n== the created artifacts, verbatim ==\n'
    (cd "$PROJECT" && find .engineering/planning -name '*.md' | sort | while IFS= read -r f; do
      printf -- '--- %s ---\n' "$f"; cat "$f"; printf '\n'
    done) 2>/dev/null | head -c 16000
  } > "$WORK/review-input.md"

  say "running adversarial reviewer (model ${EVAL_REVIEW_MODEL:-sonnet}) …"
  (cd "$WORK" && CLAUDE_CONFIG_DIR="$WORK/claude-home" claude -p "$(cat "$WORK/review-input.md")" \
    --model "${EVAL_REVIEW_MODEL:-sonnet}" --max-turns 4 --permission-mode dontAsk \
    --allowedTools "" > "$WORK/review.md" 2>> "$WORK/stderr.log") || \
    say "note: reviewer run failed (advisory only — verdict unaffected); see $WORK/stderr.log"
fi

# ---- 6. report --------------------------------------------------------------------------------
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
if [ -s "$WORK/metrics.txt" ]; then
  say ""
  say "== run metrics (informational) =="
  cat "$WORK/metrics.txt"
fi
if [ -s "$WORK/review.md" ]; then
  say ""
  say "== adversarial review (advisory — does not gate) =="
  cat "$WORK/review.md"
fi

say ""
COST="$(grep '"type":"result"' "$WORK/result.jsonl" | tail -1 | grep -o '"total_cost_usd":[0-9.]*' | cut -d: -f2 || true)"
[ -n "$COST" ] && COST="$(printf '%.2f' "$COST")"
say "cost: \$${COST:-unknown} (+ reviewer)   transcript: $WORK/result.jsonl   review: $WORK/review.md"
say "inspect the run yourself: $WORK"
[ "$FAIL" -eq 0 ]
