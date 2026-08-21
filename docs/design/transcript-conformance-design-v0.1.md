# Transcript conformance — a typed specification over an agent run — Design v0.1

> **Repository:** `codewandler/engineering-protocols`
> **Status:** **proposed, not accepted.** Nothing here is a work order. No plan page has taken it up,
> and per [`AGENTS.md`](../../AGENTS.md) § *Which documents are normative* a proposal is not a work
> order however recent it is. The milestones in § 9 are unsequenced on purpose.
> **Audience:** whoever reviews this for acceptance, and whoever would build it afterwards.
> **Relationship to existing design:** additive, and structurally derivative. It is `infra-spec/1`
> pointed at a third observation domain, and it reuses that family's shape deliberately rather than
> inventing a parallel one.
> **Cross-reference:**
> [`harness-planning-and-driver-design-v0.1.md`](harness-planning-and-driver-design-v0.1.md) § 4 —
> § 5 below is what makes that design's `llm` step checkable.

---

## 1. Motivation

`integrations/claude-code/eval/run.sh` runs a headless agent and then decides whether it behaved.
Its assertions have grown in two directions since the first draft of this document, and both
directions are the argument for a specification language rather than against one.

**It started with a grep**, and one is still there — assertion 3.4, at `run.sh:121`:

```bash
# 3.4 the agent used the CLI to create artifacts, not hand-written frontmatter
R=1; grep -q 'protocol artifact new' "$WORK/result.jsonl" && R=0; check "transcript shows protocol artifact new" "$R"
```

**Then it grew structure.** Assertions 3.5 and 3.6 (`run.sh:123-131`) do the right thing and reach
into the event stream properly, with `jq`:

```bash
jq -e 'select(.tool_use_result.commandName=="engineering-protocols:planning")
       | select(.tool_use_result.success==true)' "$WORK/result.jsonl"
```

**And then it grew a metrics block** — sixty-five lines of `jq` at `run.sh:168-232` that print the
environment, the plugin list, turns and api requests and iterations, tokens, cache hit ratio,
latency and rate-limit state. Every one of those numbers is deliberately **informational, never
asserted**, with a comment saying why: they vary run to run.

That is the honest starting state, and it sharpens the argument rather than blunting it. What this
design replaces is not a naive grep; it is **inline `jq` that has started to become a query
language**, plus a block of facts the script can compute and cannot express an opinion about.
Concretely:

* **The grep is untyped.** `protocol artifact new` appearing anywhere in 86KB of JSON satisfies it,
  including inside the skill text the model was shown, inside a tool *result*, or inside a sentence
  where the model explains that it will not use the CLI. The claim is about a `tool_use` event with
  a particular name and a particular input; the grep cannot express any of that.
* **The `jq` is typed and still unversioned.** It encodes the transcript's field names in a bash
  string. When the format changes, the assertion does not fail — it silently stops matching, and a
  check that quietly stops checking reads exactly like a check that passed. This repository has a
  written position on that (`AGENTS.md` § *Gate*: a skipped check reads exactly like a passing one)
  and applies it everywhere except here. Worse, both assertions carry a **`grep` fallback for when
  `jq` is absent**, which is a second, weaker definition of the same claim in the same file.
* **The metrics block is a specification with no verdicts.** It computes exactly the quantities
  § 3.3 gives expectation kinds to, and can say nothing about any of them, because a bash script has
  nowhere to put a bound and no third value to report when a field is missing. It even detects a
  real defect — *"$EXTRA non-eval plugin(s) leaked in from the user environment — the run is not
  hermetic"* — and prints it as a note, because `check` would have to be pass-or-fail and the honest
  answer is neither.
* **None of it composes or accumulates.** A second eval wants the same assertions plus three more;
  the only mechanism available is copy-paste. Every other family of claims in this repository —
  documents, specifications, infrastructure — went through exactly this stage and out the other side
  into a typed document with a checker.
* **None of it can be evidence.** A shell exit status is not something the protocol can reason
  about. It cannot be minted as an evidence record, cannot carry provenance, and cannot satisfy an
  independence requirement.

The thesis of this repository is that a checkable claim deserves a typed document rather than a
prose instruction or a shell idiom. *"The skill was loaded"*, *"the CLI was called before any file
was edited"*, *"no `Edit` touched a frontmatter path"*, *"only our plugin was loaded"*, *"the run
cost under a dollar"* are all checkable claims. They are currently a grep, some `jq`, a printed note,
or absent.

### 1.1 Why it belongs here, and not in a test-harness project

This is the same pattern the repository has now built twice, and this would be its third instance:

| | ESS | Infra | **Trace** |
|---|---|---|---|
| observation | — (the model is authored) | a cluster scan (`infra-scout`, out of process) | **an agent-run transcript** |
| normalized IR | `EssIr` | `infra-ir/1`, content-addressed | **`trace-ir/1`, content-addressed** |
| authored expectations | the specification itself | `infra-spec/1`, twelve kinds | **`trace-spec/1`** |
| verdicts | pass / fail / unsupported | `ok` / `gap` / `unk` | **`ok` / `gap` / `unk`** |
| the third value means | the scenario could not be executed | the snapshot cannot decide | **the adapter did not understand the event** |
| evidence | `ess conform evidence` | — | **`trace evidence`** |

The pattern is not being copied for tidiness. It is being copied because the third value is the
whole point in each case, and getting it wrong in a new domain is how a checker starts lying. An
event kind the adapter does not understand must yield `unk` — never a pass, and never a fail. A
transcript from a harness version that renamed a field must produce *"this run could not be judged
on that expectation"*, not *"the agent did not load the skill"*.

---

## 2. Observation → `trace-ir/1`

### 2.1 The shape

An **adapter** per harness format reads a raw transcript and normalizes it into a harness-neutral
event IR. The IR is content-addressed by a digest over the raw transcript bytes, so a report can
name exactly which run it judged, and two reports about "the same run" that disagree are
distinguishable from two reports about two runs.

```text
result.jsonl  ──adapter──▶  trace-ir/1  ──check(spec)──▶  report  ──▶  evidence
   (harness)                (neutral)                    (verdicts)     (AEP)
```

Neutral means the expectation kinds in § 3 are phrased against the IR and never against
`stream-json`. A second adapter — for another harness, or for a future native eval format — is a
second adapter and not a second specification language. That is the same seam `ess-synth` draws
between a plan and its emitters, for the same reason.

### 2.2 The first adapter: Claude Code `stream-json`

Grounded in a real transcript, read and re-read while writing this: 46 events, 86KB, at
`/home/timo/.cache/claude-tmp/plugin-eval.3Rgmwv/result.jsonl`, produced by the plugin eval on
2026-08-21. The full type census of that run:

| count | event | normalizes to |
|---:|---|---|
| 1 | `system` / `init` | `SessionStart` — a whole observation family of its own (§ 2.3) |
| 21 | `assistant` | one `ToolCall` per `tool_use` block, one `AssistantText` per text block, plus per-request usage |
| 14 | `user` | `ToolResult` correlated to its call — **typed per tool** (§ 2.4) — or a synthetic injection (§ 2.8) |
| 8 | `system` / `thinking_tokens` | `ThinkingEstimate` — `estimated_tokens` and a delta |
| 1 | `rate_limit_event` | `RateLimit` — the account's state at the moment the run started |
| 1 | `result` / `success` | `RunOutcome` — the terminal record, and the source of every resource fact in § 3.4 |
| **46** | | |

Nothing in that census is discarded. The first draft of this document treated `thinking_tokens` and
`rate_limit_event` as opaque; both turn out to carry facts somebody would want to assert on, and
both are typed below.

**A skill invocation is a tool call.** There is no distinct event kind for it; the observed form is

```json
{"type":"tool_use","name":"Skill","input":{"skill":"engineering-protocols:planning","args":"…"}}
```

at event 5, followed by twelve `Bash`/`Read`/`Edit` calls — which is exactly the claim the grep was
reaching for and could not state.

**Every event keeps its index.** A verdict cites the indices of the events that produced it, which
is what makes a report checkable by a human against the transcript it names.

### 2.3 `system`/`init` is an observation family, not a preamble

The single `init` event is the most under-used object in the transcript, and it is where a whole
class of eval defects is visible *before the first turn is spent*. Observed keys and values:

| key | observed | what an expectation over it catches |
|---|---|---|
| `model` | `"claude-sonnet-5"` | the alias `sonnet` was passed on the command line; this is what it **resolved to**. A run judged against a model nobody meant to use |
| `permissionMode` | `"dontAsk"` | a run that silently asked for permissions, or one that was more permissive than the eval intended |
| `apiKeySource` | `"none"` | **the one that has already bitten.** In an earlier failing run this was non-`none`: an exported `ANTHROPIC_API_KEY` took precedence over the claude.ai login and billed an account with no credits. `run.sh:75-79` now unsets the variable, and the comment there is the scar. An `env.api_key_source` expectation would have caught it in the first event, before a turn was spent |
| `claude_code_version` | `"2.1.238"` | the version a green run was green on — the thing you want in the record when the next version turns it red |
| `output_style` | `"Operator Report"` | **leaked from the user's own configuration** — see below |
| `tools` | 33 | a tool surface wider than `--allowedTools` implies |
| `slash_commands` | 66 | as above |
| `skills` | 27, including `engineering-protocols:planning` | the plugin's skill is **available**, which is a different fact from it being invoked |
| `agents` | 11, including `engineering-protocols:decomposer` and `:plan-reviewer` | the plugin's agents loaded under the names the plan page's W1.3 acceptance refers to |
| `plugins` | 6 objects, each `{name, version, path, source}`; ours is `engineering-protocols@inline` v0.1.0 from the scratch directory | which plugin was actually loaded, at which version, from where |

Six expectation kinds follow directly, and they are cheap because the facts are all in one event:
`env.model`, `env.permission_mode`, `env.api_key_source`, `env.plugin_loaded` (by name, optionally
version and source), `env.skill_available` / `env.agent_available`, and one more that deserves its
own paragraph.

**`env.exclusive` — hermeticity, prevented *and* asserted.** The transcript at the path above is
**not** hermetic, and it is the motivating case:

```text
plugins loaded:  engineering-protocols@inline 0.1.0   ← the one under test
                 rust-analyzer-lsp@claude-plugins-official 1.0.0
                 gopls-lsp@claude-plugins-official 1.0.0
                 typescript-lsp@claude-plugins-official 1.0.0
                 track@agentplugins 0.5.0
                 flux-agent@…
skills:          27, of which 1 is the plugin's
output_style:    "Operator Report"   ← the operator's own
```

Five foreign plugins, twenty-six foreign skills and the user's output style all leaked from
`~/.claude` into a run that was supposed to be a copy of the plugin in a scratch directory. An
earlier draft of this section concluded that only *detection* was achievable, on the reasoning that
authentication lives in the config directory and reading it brings everything else along.

**That was wrong, and the fix is now in the eval.** The config directory is redirectable, and the
credentials are one file inside it, so the two can be separated (`run.sh:64-73`):

```bash
# A scratch config home, so the operator's own plugins, skills and output style cannot leak into
# the run … Only the login credentials are carried over — auth is the one thing the run
# must share with the operator, and the only thing it does.
mkdir -p "$WORK/claude-home"
cp "$HOME/.claude/.credentials.json" "$WORK/claude-home/.credentials.json"
…
CLAUDE_CONFIG_DIR="$WORK/claude-home" claude -p "$PROMPT" --plugin-dir "$WORK/plugin" …
```

Probed after the change: **plugins 0 foreign** (the eval's is loaded by `--plugin-dir`, not from the
config home), **skills 27 → 16**, all of them built-ins, **output style back to the default**,
**`apiKeySource: none`**, and the run still authenticated and still billed to the login. Isolation
holds and auth survives it.

So the honest statement is that in *this* harness the leak is **preventable**, and the eval now
prevents it. The expectation kind survives that, for two reasons, and the second is the one that
matters:

1. **Prevention is harness-specific; detection is not.** `CLAUDE_CONFIG_DIR` is a Claude Code
   affordance. A second adapter, a hosted runner, or a CI image that cannot redirect a config
   directory has no equivalent, and for those the assertion is the only control available. A
   specification language that only worked where the environment could be sealed would be a
   specification language for the easy case.
2. **A guard is verified by what it refuses.** `AGENTS.md` § *Conventions* makes this a house rule —
   *verify a guard by breaking it* — and it applies exactly here. Isolation that silently stops
   working is indistinguishable from isolation that works: the run goes green either way, and the
   contamination shows up months later as an unreproducible result. `env.exclusive` is what turns a
   broken seal into a `gap` on the next run. `run.sh:144-153` now asserts precisely this — the init
   event's plugin list must be exactly `["engineering-protocols"]` — and `run.sh:155-166` asserts
   `apiKeySource == "none"` unless `EVAL_USE_API_KEY=1` opts into key billing.

That pair — prevent it in the runner, assert it in the specification — is the same arrangement this
repository uses everywhere else. The type makes the bad state hard to reach; the test makes it
impossible to reach silently.

### 2.4 Tool results are typed per tool

The `tool_use_result` on a `user` event has a different shape for every tool, and each shape carries
an assertion somebody wants. Observed across the run's twelve results:

| tool | `tool_use_result` keys | the assertion it enables |
|---|---|---|
| `Skill` | `commandName`, `success` | **`success == true`** — the skill ran to completion, structurally |
| `Bash` | `stdout`, `stderr`, `interrupted`, `isImage`, `noOutputExpected` | a regex on `stdout` (what the CLI actually printed), and **`interrupted == false`** — a command that was killed is not a command that ran |
| `Edit` | `filePath`, `oldString`, `newString`, `originalFile`, `replaceAll`, `structuredPatch`, `userModified` | **`userModified == false`** — nobody's hands were in the file. In a headless run this must always hold, and an assertion that it did is a headless-integrity check the workspace cannot provide |
| `Read` | `type`, `file` | which file was actually read, as a fact rather than as an argument the model supplied |

This is a distinct expectation kind, `tool.result`: `tool.called` matches the **request**, and
`tool.result` matches what came back. The two are different claims — a `Bash` call whose command
matched and whose `interrupted` is `true` satisfies the first and should fail the second.

### 2.5 Tool traffic is measurable, and it is a context-budget fact

A `tool_use` and its correlated `tool_result` are a *pair*, and the pair has a size. Measured on the
hermetic run (`/home/timo/.cache/claude-tmp/plugin-eval.VYjb60/result.jsonl`, 37 events):

| tool | calls | errors | input bytes | result bytes | ≈ tokens |
|---|---:|---:|---:|---:|---:|
| `Bash` | 4 | 0 | 854 | 2 035 | 509 |
| `Read` | 3 | 0 | 378 | 4 668 | 1 167 |
| `Write` | 3 | 0 | **4 029** | 645 | 161 |
| `Skill` | 1 | 0 | 300 | 47 | 11 |
| **total** | **11** | **0** | 5 561 | **7 395** | **1 848** |

At roughly four bytes to the token, that run pushed about **1 848 tokens of tool output into the
context window** — and it did so invisibly, because no aggregate in the `result` event separates
tool results from anything else.

**The asymmetry is the interesting part, and it is worth stating plainly: a tool call's input and
its result are spent from different budgets.** A tool *input* is model output — the model wrote
those bytes, and they cost output tokens at output prices. A tool *result* is injected into the
*next* request, where it costs input tokens and then sits in the context for the rest of the run.
The two tools at the extremes of the observed run make the point:

* `Write` is **output-heavy**: 4 029 bytes of input for 645 bytes of result. The model composed
  three whole files.
* `Read` is **injection-heavy**: 378 bytes of input for 4 668 bytes of result — twelve times as much
  came back as went out, and all of it stays in context.

A run that reads six large files has spent its context window before it has done anything, and
nothing in the terminal record says so. This is the observation the expectation kinds in § 3.3 are
built on, and the eval's metrics block already computes every number in the table above
(`run.sh:191-208`), including the per-tool error counts and the count of identical call groups.

### 2.6 Where the wall clock actually goes, per step

The timestamps on `assistant` and `user` events support two derived durations per tool call, and
`run.sh:209-231` now computes both:

* **`gen`** — the interval **ending** at the event that carries the `tool_use`, attributed to
  producing that call. It is the model thinking and emitting.
* **`exec`** — from the call being issued to its result coming back. It is the tool doing the work.

Measured on run `plugin-eval.7hTYjT` (11 steps, `duration_ms: 42 167`):

| step | tool | gen | exec |
|---:|---|---:|---:|
| 1 | `Skill` | 1 486 | 35 |
| 2–4 | `Bash` (discovery, two `artifact new`) | 1 290 / 1 088 / 3 205 | 187 / 21 / 38 |
| 5–7 | `Read` | 555 / 560 / 305 | 36 / 6 / 16 |
| 8–10 | `Edit` (the three story bodies) | **8 742 / 5 968 / 4 482** | 26 / 28 / 9 |
| 11 | `Bash` (`artifact validate`) | 80 | 13 |
| | **total** | **27 761 ms** | **415 ms** |

**The wall clock is about 98.5% model.** Every CLI execution in the run finished in ≤ 187 ms, and
the three body-writing `Edit`s cost between 4.5 and 8.7 seconds *each* — in generation, before the
edit was applied. Any optimisation aimed at the tooling is aimed at 1.5% of the run; the interesting
number is which steps make the model think hardest, and it is the ones where it is composing prose.

Four expectation kinds follow: **`step.gen_time`** and **`step.exec_time`**, scoped by tool name or
argument matcher, and the run-level **`time.inference_total`** and **`time.tool_exec_total`**.
`step.exec_time` scoped to `Bash` matching `protocol artifact` is a genuine regression guard on this
repository's own CLI — a verb that got slow shows up as a step, not as a percentage of a total that
is dominated by inference.

**Derived, not measured, and `unk` when it cannot be derived.** Both durations come from timestamps
the harness recorded; the checker reads no clock (invariant 9). Events without a `timestamp` are
skipped — § 2.3 shows that the first four events of a run carry none — so a call whose neighbours
lack timestamps yields `?`, and the expectation over it is **`unk`**. That is the same posture as
`ttft` in § 3.6: read what was recorded, never subtract what was not.

### 2.7 Per-request usage is on every assistant event

Each `assistant` event carries `message.usage`, and the run's ramp is legible: `cache_read` climbs
monotonically `25032 → 38732 → 41357 → 42288 → 42711 → 43068 → 43501 → 45865 → 47137 → 48175 →
49251`, while `cache_creation` is front-loaded — `13700` on the first request, then `2625`, `931`,
`423`, `357`, and hundreds thereafter.

v0.1 asserts on **aggregates only** (§ 3.6). The per-request series is preserved in the IR and given
no expectation kind, and the deferral is named rather than silent: *shape* assertions over the
series — the cache-read ramp is monotone, cache creation is front-loaded, no single request exceeds
a share of the total — are a real and useful family, and they need a vocabulary for talking about
sequences that § 3.4's single-field matchers do not have. Adding them later costs nothing, because
the data is already in the IR; adding them now would mean designing that vocabulary under a deadline
set by a different feature.

### 2.8 One more thing the transcript makes visible

Event 7 is a `user` event with `isSynthetic: true` whose text begins:

```text
Base directory for this skill: /home/timo/.cache/claude-tmp/plugin-eval.3Rgmwv/plugin/skills/planning

# Planning in a governed artifact store
…
```

That is the skill's own content being injected into the conversation — direct evidence that the
skill *entered the model's context*, from the scratch directory, which is a third and stronger fact
than "available" or "invoked". It is recorded here as **observable with no expectation kind in
v0.1**: the three levels in § 3.2 cover what people actually want to assert, and a fourth kind whose
matcher is "a synthetic event containing the skill's text" would be a wording assertion wearing a
structural costume.

### 2.9 Unknown shapes are preserved opaque

An event the adapter does not recognise is retained in the IR as an opaque record — its index, its
raw bytes' digest, and its `type`/`subtype` if present — and is never dropped. Two consequences,
both deliberate:

* the IR's digest covers everything, so a transcript cannot be silently reinterpreted by an adapter
  upgrade that starts understanding a field;
* an expectation whose truth would depend on an opaque event is `unk`, with the reason naming the
  event index and its unrecognised type.

Dropping unknown events would produce the failure mode this design exists to prevent: a checker that
reports *"the tool was never called"* when what happened is that it stopped being able to see tool
calls.

---

## 3. Specification → `trace-spec/1`

Typed YAML, `Raw*` → validated through `TryFrom` (invariant 2), accumulating (invariant 3),
schema-generated (invariant 1). One document states what a run must have looked like:

```yaml
format: trace-spec/1
id: planning-plugin/eval
title: The planning plugin behaves as its skill says it does

expectations:
  # --- the environment the run actually got -------------------------------
  - id: our-plugin-loaded
    expect: env.plugin_loaded
    plugin: engineering-protocols
    version: "0.1.0"
    source: engineering-protocols@inline

  - id: nothing-else-loaded
    expect: env.exclusive
    plugins: [engineering-protocols]        # observed run: gap, 5 foreign plugins

  - id: billed-to-the-session
    expect: env.api_key_source
    equals: none                            # an exported API key is a billing misfire

  # --- what the agent did -------------------------------------------------
  - id: skill-completed
    expect: skill.completed
    skill: engineering-protocols:planning
    count: {at_least: 1}

  - id: created-through-the-cli
    expect: tool.called
    tool: Bash
    args: {command: {contains: "protocol artifact new"}}
    count: {at_least: 1}

  - id: no-hand-edited-frontmatter
    expect: tool.absent
    tool: Edit
    args: {file_path: {regex: "\\.engineering/planning/.*\\.md$"}}

  - id: no-edit-was-touched-by-a-human
    expect: tool.result
    tool: Edit
    result: {userModified: {equals: false}}

  - id: asked-before-writing
    expect: order
    first: {tool.called: {tool: Bash, args: {command: {contains: "protocol artifact"}}}}
    before: {tool.called: {tool: Edit}}

  # --- what it cost -------------------------------------------------------
  - id: within-budget
    expect: cost.total
    at_most_usd: 1.00

  - id: not-paid-from-overage
    expect: rate_limit.overage
    equals: false
```

### 3.1 Environment expectations

Evaluated against the single `init` event (§ 2.3). They are first in the document because they are
first in the transcript, and because a run whose environment was wrong should be reported as such
rather than as a behavioural failure downstream of it.

| kind | holds when | `unk` when |
|---|---|---|
| `env.plugin_loaded` | a plugin with this name is loaded; optionally at this `version` and from this `source` | the harness records no plugin list |
| `env.exclusive` | the loaded plugins are **exactly** the named set — nothing else leaked in | as above |
| `env.output_style` | the output style is the expected one, usually the default | no such field |
| `env.skill_available` | the named skill is in `skills` | no `skills` field |
| `env.agent_available` | the named agent is in `agents` | no `agents` field |
| `env.model` | the **resolved** model matches | no `model` field |
| `env.permission_mode` | `permissionMode` equals the expected value | no such field |
| `env.api_key_source` | `apiKeySource` equals the expected value — `none` for a run that must bill the logged-in session | no such field |

`env.exclusive` is the one worth writing a spec around, and § 2.3 has both the observed leakage
that motivates it and the config-directory isolation that now prevents it. It is the only kind here
that can fail on a correctly-behaving agent, which is the point: it reports on the *experiment*, not
on the subject — and it is what makes a silently-broken isolation visible, which prevention alone
cannot do.

### 3.2 Skill expectations, in three levels

The first draft of this document had one `skill.invoked` kind. The transcript shows three distinct
facts, and collapsing them loses the one that matters most:

| kind | reads | means |
|---|---|---|
| `skill.available` | `init.skills` contains the name | the harness offered it — an environment fact, and the alias of `env.skill_available` |
| `skill.invoked` | a `tool_use` with `name: "Skill"` and `input.skill` matching | the model **chose** it |
| `skill.completed` | the correlated `tool_use_result` has `commandName` matching **and `success: true`** | it ran to completion |

`skill.completed` is **structural, not textual**. The observed result object is
`{"commandName":"engineering-protocols:planning","success":true}` — a boolean the harness set, not a
sentence the model wrote. `run.sh:123-131` already asserts exactly this with `jq`, which is the
strongest existing assertion in the eval and the clearest demonstration that the script has outgrown
its medium.

Available-but-never-invoked is a real and interesting outcome: the plugin loaded and the model did
not reach for it. A single kind cannot report that; three can.

### 3.3 Behavioural expectations

| kind | holds when | `unk` when |
|---|---|---|
| `tool.called` | a tool call matches the name **and** the args matcher, within `count` | the adapter met an opaque event that could have been a tool call |
| `tool.absent` | no tool call matches — `count: {exactly: 0}` with a name of its own, because "this must never happen" is the assertion people get wrong when they have to spell it as a bound | as above |
| `tool.result` | a matched call's **result** satisfies a result matcher (§ 3.4) | the call matched but no result was correlated to it — a truncated transcript, which is not the same as a bad result |
| `tool.result_bytes` | the result bytes of a matched call — or of every call — stay within a bound | a call with no correlated result |
| `tool.failed` | the number of results with `is_error: true`, scoped to a tool or matcher, stays within a bound | as above |
| `tool.error_rate` | failed calls over total calls, in the same scope, stays within a bound | no calls in scope at all — a rate over zero is not zero |
| `tool.repeated` | the number of groups of **identical** `(tool, input)` calls stays within a bound | as above |
| `order` | the **first** occurrence of A precedes the **first** occurrence of B | either side never occurs — "A before B" is undecidable when there is no A, and reporting it as a failure blames the wrong thing |
| `result` | the run's terminal record matches: `terminal_reason`, `stop_reason`, `subtype`, `is_error`, and `api_error_status` absent-or-equal | no `result` event — a transcript truncated by a crash has no terminal record, and that is exactly the case that must not read as a failed assertion |
| `subagent.spawned` | `subagent_stats.spawned` within bounds | the field is absent from this harness version |
| `permission.denied` | `permission_denials` length within bounds | as above |
| `rate_limit.status` | `rate_limit_info.status` is in an allowed set | no `rate_limit_event` in the transcript |
| `rate_limit.overage` | `isUsingOverage` is `false` | as above |
| `rate_limit.utilization` | `utilization` within a bound | as above |
| `text.matches` | the final assistant text matches a regex | there is no final text |

**`order` is the same fact AEP already models.** `evidence.first_seq.test_result <
evidence.first_seq.diff` is how red-before-green is checked in the protocol
(`docs/guide/harness.md` § 1), and it is first-occurrence ordering over a submission sequence. This
is first-occurrence ordering over an event sequence, and it is spelled the same way on purpose: an
author who has met one meets no new idea in the other.

**The tool-traffic family measures two different things, and both are worth bounding.**
`tool.result_bytes` is a **context-budget guard**: § 2.5 shows a run injecting about 1 848 tokens of
tool output that no aggregate in the terminal record accounts for, and a bound on it catches the
agent that read six large files before doing any work. `tool.failed` and `tool.error_rate` measure
something else entirely — **how well the model understood the tooling**. A run that called
`protocol artifact` four times and got four usage errors did the job eventually, and did it by
groping.

There is a design nuance in the error family that must not be papered over. **A refusal this
project designed is correct behaviour, not a failure.** `protocol artifact move` exits 1 when the
move is illegal, and the plugin's fourth guardrail says a refusal is the answer — so a run in which
the model asked for an illegal move, received the refusal and relayed it behaved *exactly right*,
and it contains a failed tool call. A blanket `tool.error_rate: 0` would forbid the plugin's own
intended behaviour. The kinds are therefore **scoped** — by tool name and by argument matcher — so a
specification can say *no failed `Read`* and *no failed `Bash` whose command matches
`protocol artifact new`* while leaving the deliberate refusal alone. Every documented example uses a
scoped bound; none uses a global zero.

`tool.repeated` counts groups of byte-identical `(tool, input)` calls. Two identical `Read`s of one
file is a model that lost track; three identical `Bash` invocations is a retry loop. It is a
confusion signal rather than a correctness one, which is why it is a bound and not a prohibition —
the observed hermetic run has zero such groups, and a run with one is worth a look, not a failure.

**The rate-limit family is a billing guard, not a performance one.** Observed:
`{"status":"allowed_warning","rateLimitType":"seven_day","utilization":0.62,"isUsingOverage":false}`.
`rate_limit.overage == false` is the expectation that says *this eval run must not have been paid
for out of overage*, which is a fact about money that no other part of the record carries, and which
a CI job running an eval on every merge should absolutely be allowed to assert. `allowed_warning` at
0.62 utilization is also the shape of an early warning: an eval suite that starts reporting it is an
eval suite about to start failing for reasons that have nothing to do with the code.

**`text.matches` is the weakest kind and is marked as such in its own documentation.** The eval
deliberately asserts on files and events rather than on wording, because wording is allowed to vary
and an assertion on it is a test of a sentence. It is included because *"the refusal was relayed to
the operator"* has no other observable form today, and excluded from every example in the
documentation except the one that explains why to avoid it.

### 3.4 Argument and result matchers

Structured, not a language. A matcher applies to one named field of a tool's input, or of its
result:

| matcher | on |
|---|---|
| `exact: <string>` | the whole field |
| `contains: <string>` | substring |
| `regex: <pattern>` | the field, anchored as written |
| `equals: <bool\|number>` | a scalar field of a result — `interrupted: {equals: false}` |

`count` is `{at_least}`, `{at_most}`, `{exactly}`, or a pair — never a bare number, so
`count: 1` cannot be read as "at least once" by one author and "exactly once" by the next.

The result fields available per tool are the observed ones in § 2.4, and the two worth naming in
prose because they are assertions nobody thinks to write:

* **`interrupted: {equals: false}`** on a `Bash` result. A command that was killed still appears as
  a call with the right arguments; only the result says it never finished.
* **`userModified: {equals: false}`** on an `Edit` result. In a headless run this must always hold,
  and asserting it is a check on the *experiment's* integrity that no amount of workspace inspection
  can provide — the file afterwards looks the same either way.

This is deliberately not an expression language (see **D2**).

### 3.5 Counting a run: four quantities, four kinds

The single most avoidable defect in this whole family is a bound set against the wrong quantity.
The observed run produced four different numbers that all sound like "how much did the agent do",
and **all four are first-class boundable expectation kinds** — none is derived from another, and
none is left as a number the reader has to compute:

| expectation kind | observed | reads | is not |
|---|---:|---|---|
| `turns` | **15** | `result.num_turns` | the harness's own notion of a turn, and the only one of the four the harness itself names |
| `api_requests` | **11** | distinct `request_id` across `assistant` events | fewer than the event count, because one API response arrives as several events. The closest thing to "how many times did we call the model" |
| `events.assistant` | **21** | count of `assistant` events | an artefact of streaming: text and each tool call arrive as separate events sharing one `request_id`. Bound it to catch a run that fragmented, not to bound cost |
| `iterations` | **1** | `len(result.usage.iterations)` | an **array** of per-iteration usage records, not a counter — and nothing like the other three |

Each takes the same `{at_most}` / `{at_least}` / `{exactly}` bounds as everything else, and each
carries the definition above in its own documentation. `run.sh:178-181` prints all four on one line,
which is how the discrepancy became visible in the first place.

**The warning stands even now that all four exist.** Having four kinds does not make it safe to pick
one at random: a reviewer reading `turns: {at_most: 20}` should not have to guess which quantity it
bounds, and a bound of 20 means something different against each of these four numbers. Choose the
one that matches the failure being guarded against — a runaway loop is `turns`, a cost surprise is
`api_requests` or `cost.total`, and `events.assistant` is almost never what anyone means.

### 3.6 Resource and performance expectations

Every one of these is **read from a field the transcript already recorded**, and none is measured by
the checker. That is invariant 9 applied to a domain where it would be easy to lose: a checker that
timed its own run would produce a different report for the same transcript on a loaded machine, and
a report that cannot be reproduced cannot be committed, diffed or used as evidence.

| kind | reads | notes |
|---|---|---|
| `turns` / `api_requests` / `events.assistant` / `iterations` | § 3.5 | four quantities, four kinds, four definitions |
| `tokens.input` / `tokens.output` / `tokens.total` | `result.usage.input_tokens`, `output_tokens`, and their sum | `total` is defined in the document as `input + output`, excluding cache reads, and says so |
| `tokens.thinking` | `result.usage.output_tokens_details.thinking_tokens` | the **actual** count. Observed: `276` |
| `thinking.estimated` | the last `system`/`thinking_tokens` event's `estimated_tokens` | a **different source and a different number**: eight such events, `estimated_tokens` restarting per stretch (50, 80 · 50, 113 · 100, 123 · 50, 166) with `estimated_tokens_delta` beside each. It is the harness's live estimate, not the billed figure, and the two must never be conflated in one kind — one is an estimate emitted mid-stream, the other is what the API reported |
| `cost.total` | `result.total_cost_usd` | observed: `0.3561121`. Bounds only, never equality (**D6**) |
| `duration.total` | `result.duration_ms` | observed: `55812` |
| `duration.api` | `result.duration_api_ms` | observed: `56588` — note it exceeds `duration_ms` in the real transcript, which is a good reason not to derive one from the other |
| `ttft` | `result.ttft_ms` | observed: `1797`, with `ttft_stream_ms: 1253` beside it |
| `step.gen_time` | derived per call: the inference interval ending at the `tool_use` event (§ 2.6) | scoped by tool or matcher. Observed on `7hTYjT`: `Edit` steps at 8 742 / 5 968 / 4 482 ms |
| `step.exec_time` | derived per call: `tool_use` to correlated `tool_result` (§ 2.6) | scoped the same way, and the one that is a real guard on this repository's CLI — every `protocol artifact` call in the observed run returned in ≤ 187 ms |
| `time.inference_total` | the sum of every step's `gen` | observed: `27 761` ms across 11 steps |
| `time.tool_exec_total` | the sum of every step's `exec` | observed: `415` ms — 1.5% of the two combined |
| `time_to_request` | `result.time_to_request_ms` | observed: `39` — startup overhead **before the first API request**. The one latency number that is about the harness rather than the model, which makes it the one worth bounding in CI: it catches a plugin that got slow to load |
| `cache.used` | `result.usage.cache_read_input_tokens > 0` | the simple form, and the one most specs want |
| `cache.read_tokens` | `result.usage.cache_read_input_tokens` | `at_least` bounds. Observed: `467117` |
| `cache.created_tokens` | `result.usage.cache_creation_input_tokens` | `at_most` bounds. Observed: `24457`. Worth a kind after all — a run that re-creates a large cache it should have read is a real regression, and it is invisible in cost until it is expensive |
| `cache.hit_ratio` | see below | ratio, `at_least` |
| `speed` / `service_tier` | `result.usage.speed`, `result.usage.service_tier` | equals-matchers. Both observed as `"standard"`. **Environment-dependent**: documented with `enabled: false` in every example, because a spec that pins these fails on somebody else's account rather than on the agent's behaviour |

**`cache.hit_ratio` carries its denominator in the specification, not in the reader's head:**

```text
hit_ratio = cache_read_input_tokens / (cache_read_input_tokens + input_tokens)
```

Cache *creation* tokens are excluded from the denominator: writing the cache is not a miss against
it. Observed run: `467117 / (467117 + 22) = 0.99995`. Writing the formula down is the point — a ratio
whose denominator is folklore is a number two people compute differently and then argue about.
`run.sh:182-186` computes exactly this ratio in `jq`, which is where the definition currently lives.

**Per-model scope.** `result.modelUsage` is keyed by model — the observed run used
`claude-sonnet-5` **and** `claude-haiku-4-5-20251001` — and each entry carries `inputTokens`,
`outputTokens`, `cacheReadInputTokens`, `costUSD`, `contextWindow`. A token or cost expectation may
carry an optional `model:` scope, evaluated against that entry; without one it is evaluated against
the run total. An expectation scoped to a model the run never used is **`unk`**, not `ok` — the same
rule `infra-spec` applies to a scope that selects nothing, and for the same reason: an expectation
must not be able to pass by selecting nothing.

**TTFT, and one honest caveat found while writing this.** `ttft_ms` is a recorded field on the
result event, so the expectation reads it and does not compute it. The brief for this design assumed
TTFT would have to be *derived* — first assistant event's timestamp minus the first event's — and
that fallback is worth documenting precisely because the real transcript shows it does not work:
the first four events (`system init`, `rate_limit_event`, two `thinking_tokens`) carry **no
`timestamp` field at all**, and the first timestamp in the file belongs to the first assistant
event. Deriving TTFT there would compute zero. So: read the recorded field where the harness
provides it; where it does not, the expectation is **`unk`** with the reason *"this transcript
records no time to first token"*. It is never derived from a subtraction the harness did not
authorise, and never measured.

The same rule governs the four timing kinds above, and it is the reason they are in this table
rather than in a profiler: `gen` and `exec` are **derived from recorded timestamps**, not measured
by the checker, so the same transcript yields the same numbers on any machine at any load. Where a
timestamp is missing the duration is `?` and the expectation is **`unk`** — never zero, and never a
value obtained by timing something.

**Missing field ⇒ `unk`, always.** Every row above is a field of a format that is not a stable
public schema (**D1**). An absent field means this transcript cannot answer the question, which is
the third value's entire job.

### 3.7 The boundary: the trace spec owns the transcript, and nothing else

**Workspace assertions are out of scope.** *Did an epic get created? Do the stories link to it? Does
the store validate?* — those are questions about files, and they already have answers:
`protocol artifact validate`, `protocol artifact list`, and the ordinary business of looking at a
directory. Four of the eval's five current assertions are of this kind, and they should stay exactly
where they are.

A run is checked by **composition**: the trace spec judges the transcript, the store's own validator
judges the store, and the eval script runs both and reports both. Folding workspace inspection into
this document would produce a second, worse artifact validator inside a transcript checker, which is
the same mistake as a driver that evaluates its own gates.

---

## 4. Checking

```console
$ protocol trace check --spec integrations/claude-code/eval/expectations.trace.yaml \
    --transcript "$WORK/result.jsonl"
planning-plugin/eval against transcript sha256:9f3c… — 7 ok, 2 gap, 1 unk
  ok   our-plugin-loaded            engineering-protocols@inline 0.1.0 at event 0
  gap  nothing-else-loaded          5 unexpected plugins at event 0: rust-analyzer-lsp,
                                    gopls-lsp, typescript-lsp, track, flux-agent
  ok   billed-to-the-session        apiKeySource = none at event 0
  ok   skill-completed              Skill(engineering-protocols:planning) at event 5,
                                    result success=true at event 6
  ok   created-through-the-cli      Bash(command ~ "protocol artifact new") at events 21, 23
  gap  no-hand-edited-frontmatter   Edit(file_path ~ …/planning/.*\.md$) at events 35, 37, 39
  ok   no-edit-was-touched-by-a-human  userModified=false at events 36, 38, 40
  ok   asked-before-writing         first Bash(protocol artifact) 11 < first Edit 35
  ok   not-paid-from-overage        isUsingOverage=false at event 1
  unk  ttft-under-2s                this transcript records no time to first token
````

* **Deterministic.** Same transcript plus same spec ⇒ same report, byte for byte. No clock is read:
  every duration and every cost comes out of the transcript. `BTreeMap` ordering throughout, per
  invariant 9.
* **Accumulating.** Every expectation is evaluated and every verdict is reported; the checker does
  not stop at the first `gap`.
* **Every verdict cites its evidence** — the event indices that produced it. A `gap` names the events
  that should not have been there, or says which required event was absent; an `unk` names the event
  it could not read, or the field the transcript does not carry. A verdict with nothing to cite is
  unrepresentable, the way `infra-spec` makes a `False` without a gap unrepresentable.
* **`--format text|json|yaml`**, the repository's standing rule: text for people, JSON for programs,
  no third rendering.

The two gaps in that report are different in kind, and the report does not rank them:
`no-hand-edited-frontmatter` is the agent doing something the skill forbids, and
`nothing-else-loaded` is the *experiment* being contaminated by the operator's own configuration.
Both are `gap`, because both are observed contradictions of a stated expectation; which one to act
on is a judgement, and the checker does not make it.

**Exit codes mirror `ess conform`, which is the existing precedent and is documented at
`crates/protocol-cli/src/main.rs:2342` as *"`0` conformant, `1` contradicted, `3` nobody found
out"*:**

| code | meaning |
|---|---|
| `0` | every expectation `ok` |
| `1` | at least one `gap` — the run contradicted the specification |
| `3` | no gaps, and at least one `unk` — nothing was contradicted and something could not be judged |

Exit 3 is not a softer exit 1. A CI job may choose to treat it as a failure; the checker refuses to
make that choice on the job's behalf, because "the agent did the wrong thing" and "the transcript
format moved under us" want different people to be woken up.

### 4.1 `--redact`, and why privacy is named rather than assumed

A transcript contains the prompt, the model's reasoning, file contents it read and commands it ran.
That is more sensitive than any other input this repository consumes, and a report is a thing people
paste into pull requests.

`--redact` produces a report that cites **event indices and content digests only** — no command
strings, no file paths, no text. Every verdict remains checkable by anyone holding the transcript,
and nothing about the run leaks to anyone who does not. The redacted report is still deterministic
and still content-addressed, so it can be committed.

The default is discussed in **D3**.

---

## 5. The evidence join

This is the part that makes the design worth building rather than scripting, and it has two
consequences.

### 5.1 A check becomes an AEP evidence record

```console
$ protocol trace evidence --spec …/expectations.trace.yaml --transcript "$WORK/result.jsonl"
```

mints an evidence record **in the same process that ran the check**, exactly as
`protocol ess conform evidence` does — the design note there (`crates/protocol-cli/src/main.rs:2183`)
is that the record is produced on the producing side so no caller can author its own verdict, and
that argument transfers unchanged.

| field | value |
|---|---|
| kind | `trace_conformance` |
| producer | `Producer::Verifier` — the checker observed a file; it did not ask an agent how it went |
| provenance | the **transcript digest** and the **spec digest**, plus command, tool and revision |
| body | the counts, and the id of every expectation that gapped |

Two honest notes for the reviewer. First, `EvidenceKind` is a closed enum
(`crates/aep-domain/src/evidence.rs`, thirteen named variants ending at `EssConformance`), so a
`TraceConformance` variant is a **domain change** — small, but a change, and it belongs in the
acceptance decision rather than being discovered during implementation. The alternative, reusing
`Verification`, is worse: it would make a claim about an agent's behaviour indistinguishable from
every other verification record, and the whole value here is that it is distinguishable. Second, the
digest pair is what makes the record mean something later: *"some agent passed some behavioural
spec"* is worthless, and *"the run with this digest satisfied the spec with that digest"* is not.

### 5.2 The consequence for the Phase-2 driver

[`harness-planning-and-driver-design-v0.1.md`](harness-planning-and-driver-design-v0.1.md) § 4.3
establishes that **an `llm` step cannot carry an evidence block, and the type makes it
unrepresentable** — an agent's own statement never satisfies an independence requirement, so a step
kind that could mint evidence from a model's output would unpick the loop. That design's answer is
that anything checkable about an LLM step is observed by a *subsequent `command` step*.

This design supplies the missing command. The subsequent step becomes:

```yaml
- kind: command
  run: [protocol, trace, evidence, --spec, drivers/expectations/implement.trace.yaml,
        --transcript, "${step.previous.transcript}"]
  evidence: {kind: trace_conformance, verifier: artifact-validator}
```

and the consequence is worth stating plainly, because it is the point of the whole document:
**behavioural claims about an LLM step become admissible evidence without the LLM minting
anything.** The model does not report that it consulted the CLI before editing; a deterministic
checker reads the transcript the model produced and establishes it. The independence boundary is
not weakened — it is the first time it is *satisfiable* for a claim about how an agent worked, as
opposed to a claim about what the code does afterwards.

### 5.3 The eval stops greping, and stops carrying a query language in bash

`run.sh` today asserts **eight** things, in three different idioms, and prints a ninth block it
cannot assert on at all:

| # | assertion | idiom |
|---:|---|---|
| 3.1 | the store validates | the CLI's own exit code — correct, and staying |
| 3.2 | ≥1 epic, ≥2 stories exist | `find` over the workspace — staying |
| 3.3 | every story carries an epic relation | `grep` over the workspace — staying |
| 3.4 | the transcript shows `protocol artifact new` | **`grep` over the JSONL** (`run.sh:120-121`) |
| 3.5 | the planning skill completed, `success == true` | **`jq`, with a `grep` fallback** (`:123-131`) |
| 3.6 | terminal record clean: no error, no permission denials | **`jq`, with a `grep` fallback** (`:133-142`) |
| 3.7 | hermetic: the plugin list is exactly `["engineering-protocols"]` | **`jq`, no fallback** (`:144-153`) |
| 3.8 | auth is the login: `apiKeySource == "none"` | **`jq`, conditional on `EVAL_USE_API_KEY`** (`:155-166`) |
| — | metrics: environment, plugins, **all four run quantities**, tokens, cache hit ratio, latency, rate limit, **tool traffic** and **per-step timing** — per-tool calls, errors, input and result bytes, tokens injected into context, identical-call groups, and each step's `gen`/`exec` split | `jq`, **informational, asserted on nothing** (`:168-232`) |

The first three are workspace claims and stay exactly where they are — § 3.7 says why. **The last
five plus the metrics block are this design's subject**, and the shape of the list is the argument:
five transcript assertions have accumulated in three idioms, two of them carry a second weaker
definition of the same claim for when `jq` is missing, one has no fallback and silently passes
without `jq`, and the block that computes the most interesting numbers in the run can express an
opinion about none of them.

All of it becomes `integrations/claude-code/eval/expectations.trace.yaml`: 3.4 as `tool.called`, 3.5
as `skill.completed`, 3.6 as `result`, 3.7 as `env.exclusive`, 3.8 as `env.api_key_source`, and
every line of the metrics block as a bounded kind from § 3.5 and § 3.6 — or as nothing, deliberately,
which is a decision the specification records rather than a gap in what bash could express.

The script keeps its three workspace checks and gains one call to `protocol trace check`. It loses
the `jq`-or-`grep` fork, which is the clearest single symptom of the current arrangement: two
different definitions of one assertion, in one file, selected by whether a tool happens to be
installed — and, in 3.7's case, a check that passes unconditionally when the tool is absent, which
is precisely the failure mode `AGENTS.md` § *Gate* names.

**The eval now runs in four stages, and only one of them is authoritative:**

| stage | produces | authority |
|---|---|---|
| 1. the agent | `result.jsonl` | the subject |
| 2. mechanical assertions | the pass/fail verdict and the exit code | **authoritative** |
| 3. metrics | `metrics.txt` — twelve lines of numbers | informational, asserts nothing |
| 4. adversarial review | `review.md` beside `review-input.md` | advisory, gates nothing — and it has already changed the plugin's rules without changing a verdict (§ 6.3) |

This design's subject is stages 2 and 3: it turns the five transcript assertions of stage 2 into a
document, and gives stage 3's numbers somewhere to have an opinion. **It leaves stage 4 exactly
where it is** — outside the verdict, in its own file, produced by a model that the protocol's own
type system would refuse as evidence.

And if the native `claude plugin eval` (early access) becomes available, the same specification is
the grader: it is a document about a transcript, not a feature of a particular runner.

## 6. The adversarial reviewer — attached to the run, outside the verdict

`run.sh:233-284` now runs a **second, independent headless session** after the mechanical
inspection. It is a model reviewing a run, inside an eval whose whole argument is that models do not
get to judge. That is not a contradiction, and the reason it is not is worth writing down carefully,
because the next person to read this file will reach for it as a precedent.

### 6.1 What it is

| | |
|---|---|
| **input** | the task the agent was given, the mechanical verdict (every pass/fail row), the metrics block, a **summarized timeline** — assistant text, tool calls with their `gen`/`exec` timings, and per-result sizes and error flags, capped at 28 000 characters — and **the created artifacts verbatim** (§ 6.4), capped at 16 000 |
| **prompt** | review adversarially what the assertions cannot see: was the plugin followed in spirit; were there wasted, repeated or failing calls; any risky idiom |
| **output** | at most six findings, most severe first, **each citing a timeline line**, ending in exactly one line: `ADVISORY: sound` or `ADVISORY: concerns — <one line>` |
| **tools** | `--allowedTools ""` — it reads what it was given and can touch nothing |
| **artifacts** | `review.md` beside `review-input.md`, so **what the reviewer saw is reproducible**; a finding can be checked against the exact input that produced it |

### 6.2 Why it does not violate the rule against LLM judges

§ 7 states, and this design keeps, that there is **no LLM anywhere in the checker**. Three
structural facts hold that line:

1. **It never changes the exit code.** The script's last statement is `[ "$FAIL" -eq 0 ]` —
   `$FAIL` is the count of mechanical assertions, and the reviewer cannot increment it. A reviewer
   that crashes prints a note and the eval's verdict is unaffected (`run.sh:283-284`). It can be
   switched off entirely with `EVAL_SKIP_REVIEW=1`, and nothing about the verdict moves.
2. **If it were ever minted as evidence, it would be `Producer::Agent`** — and by invariant 7 and
   `Producer`'s own semantics, an agent's statement satisfies **no** requirement marked
   `independent: true`. The protocol would refuse to let it close anything. That is not a policy this
   design adds; it is the existing type system reaching the correct answer on its own, which is the
   strongest possible form of the argument.
3. **The authoritative report is the assertion verdict**, and the review is a *different artifact*.
   The vision refuses LLM calls inside authoritative report production; it does not refuse an
   advisory opinion filed next to one. `review.md` is a separate file with a separate name and a
   separate audience, and the report it sits beside was produced without it.

The distinction to hold onto: **the reviewer is attached to the run, not folded into the verdict.**

### 6.3 The loop, observed end to end

The reviewer's justification is no longer an argument about what it might catch. Across four runs it
found a real defect, the defect was fixed in the plugin's own rules, and a later run confirmed the
fix — while the authoritative verdict never moved. Every step is on disk under
`~/.cache/claude-tmp/plugin-eval.*`.

| | run | mechanical | advisory |
|---|---|---|---|
| **1. finding** | `1huAQG` | **9 / 9 pass** | `ADVISORY: concerns` — *"agent hand-wrote/rewrote machine-generated frontmatter via Write for all 3 artifacts"* |
| **2. rule change** | — | — | `SKILL.md:56` and `decomposer.md:66` tightened: **"targeted edit below the closing `---`, never a whole-file rewrite"** |
| **3. behaviour change** | `ShRLs2`, `7hTYjT` | 9 / 9 pass | tool mix moved from `Write × 3` to **`Edit × 3`, zero `Write`** |
| **4. confirmation** | `7hTYjT` | 9 / 9 pass | `ADVISORY: sound` — *"Edits at lines 8–10 targeted only the placeholder body below the `---` block … no whole-file rewrites occurred (Edit, not Write, throughout)"* |

Step 1 is the case the mechanical layer structurally cannot reach. In `1huAQG` every assertion
passed: the agent created the epic and both stories through `protocol artifact new`, the skill
completed, the store validated. Then it filled the bodies with three whole-file `Write` calls, and
the frontmatter survived **only because the model reproduced it faithfully** — `format:
aep.planning-md/1`, `id:`, `kind:`, `status: draft`, byte for byte. It got away with it, and it
would not always.

**No assertion catches that without becoming brittle.** Forbidding `Write` under the store outright
would break the legitimate first write of a file; asserting that the frontmatter is unchanged is
what `protocol artifact validate` already does, and it *passes*, because the copy was faithful. The
reviewer said it in one line, a human agreed, two documents changed, and the next run behaved
differently.

**And the verdict never moved.** All four runs are 9 / 9. The advisory layer changed the plugin's
rules without ever having changed a pass into a fail — which is precisely the arrangement § 6.2
argues for: authority stays with the deterministic assertions, and the model's opinion is an input
to *humans changing the rules*, not to the exit code.

### 6.4 What the reviewer is shown is part of its design

Run `ShRLs2` returned `ADVISORY: concerns` on a run that had done nothing wrong. Its top finding was
that *"no visible CLI call creates the second story"* and that the artifacts' body content was
*"unverifiable from the transcript"*. Both were true statements **about the reviewer's input** and
false statements about the agent: the call existed and the bodies were correct — the timeline had
simply truncated past them.

That is a finding about the harness, not the subject, and it produced two fixes in `run.sh`:
tool-input excerpts were lengthened (180 → 300 characters, `run.sh:257`), and **the created
artifacts are now attached verbatim** to the reviewer's input (`run.sh:272-277`, capped at 16 000
characters). `7hTYjT`'s `ADVISORY: sound` cites that attachment directly — it could confirm what the
`Edit`s touched because it could read the files.

The design point generalises past this eval: **an adversarial reviewer's findings are only as good
as the completeness of what it was shown**, and an incomplete input does not produce silence, it
produces confident findings about absences that are artefacts of truncation. The control is that
`review-input.md` **exists as a file** beside `review.md` (`run.sh:278`). Every finding is auditable
against the exact bytes that produced it, so "the reviewer is wrong" and "the reviewer was
under-informed" are distinguishable — which, on `ShRLs2`, is how the fix was found. A reviewer whose
input is assembled in memory and discarded offers no way to tell those two apart.

### 6.5 Where it would live in the DSL

Not as an expectation kind. `review.attached` or `review.advisory_sound` would be an expectation
whose truth depends on a model, which is exactly the thing § 7 refuses — and an expectation that
can only be evaluated by making an API call would break determinism, the no-network rule and the
replayability claim in one line of YAML.

The right shape is a **`review` attachment slot on the run record**: a place where an advisory
artifact and its input are carried alongside the verdicts, digested so they cannot be swapped, and
never read by the checker. A reader of the report sees the verdicts and, beside them, an opinion
clearly marked as one. A program consuming the report sees the verdicts and ignores the slot.

---

## 7. What this is not

* **Not a benchmark, and not a scorer.** There is no aggregate number, no percentage, no leaderboard.
  A specification is satisfied, contradicted, or undecidable. Scores invite tuning against the score.
* **No LLM judges, anywhere in the checker.** The checker is pure: it reads a file and evaluates
  typed predicates. The eval's adversarial reviewer (§ 6) is not a counter-example: it is a separate
  artifact beside the report, it cannot move the exit code, and the protocol would classify anything
  it said as `Producer::Agent` and refuse it as independent evidence. `docs/VISION.md` refuses LLM calls inside authoritative report production, and a
  transcript checker is the single most tempting place in this repository to break that rule —
  *"ask a model whether the agent behaved reasonably"* is one function call away and would make every
  verdict unreproducible and unfalsifiable at once. `text.matches` is a regex, and it is the weakest
  kind on the list for exactly this reason.
* **Not a wording tool.** See `text.matches` above. Assertions on prose are assertions on a sentence
  that was allowed to vary.
* **Not a workspace inspector.** § 3.4.
* **Not a streaming monitor.** Batch, over a completed transcript. A live checker that could halt a
  run mid-flight is a different product with different failure modes — and it is the driver's
  territory, not a checker's. Deferred by name in **D5**.
* **Not, in v0.1, an analyser of per-request shape.** The IR keeps every `assistant` event's own
  `usage`, and the observed run has a legible cache-read ramp and front-loaded cache creation
  (§ 2.7). Assertions over that *series* — monotone, front-loaded, no request above a share of the
  total — need a vocabulary for sequences that § 3.4's single-field matchers do not have. Named as a
  deferral rather than left to be discovered, because the data is already retained and the omission
  is the vocabulary, not the observation.

---

## 8. Open decisions

Each with the default taken if nobody decides otherwise.

**D1 — the harness format is not a stable public schema.**
`stream-json` is an output format, not a contract; fields appear, get renamed, and change shape
between versions. *Default: the adapter is versioned and fails soft.* The adapter declares which
harness versions it was written against; an event shape it does not recognise becomes an opaque
record (§ 2.3) and every expectation depending on it is `unk`. The adapter never guesses, and it
never treats absence as falsity. The cost is real and accepted: a harness upgrade can turn a green
run into an exit 3, which is the correct outcome — somebody should look.

**D2 — matcher language: structured matchers only, with fact projection as the growth path.**
*Default: no expression language.* v0.1 has exact/contains/regex on named fields plus count bounds,
and nothing else — no boolean combinators, no arithmetic, no nesting beyond one field.
The growth path when that becomes insufficient is **not** a second predicate language: it is to
project trace facts into the namespace the protocol's existing three-valued predicate language
already reads — `trace.tool.bash.count`, `trace.cost.total_usd`, `trace.cache.hit_ratio`,
`trace.env.plugin_count`, `trace.api_requests` — exactly as
`infra-spec`'s `workload_predicate` projects eighteen `workload.*` facts and then uses the
protocol's own operators over them rather than inventing a second set. That precedent is the whole
argument: this repository has already met this fork once and chose projection.

**D3 — is `--redact` the default?**
*Default: no, redaction is opt-in, and the un-redacted report warns.* The report is most useful with
its evidence visible, and a checker that hides evidence by default is one people stop trusting. But
the text output carries a footer naming what the report contains — command strings and file paths
from a transcript — so that pasting it somewhere public is a decision rather than an accident. A
reviewer may reasonably invert this; it is cheap to invert and expensive to discover was wrong.

**D4 — placement.**
*Default: a new crate pair beside the infra family — `trace-domain` (the IR, the adapters, the raw→
validated pair) and `trace-spec` (the expectations and the checker) — with the CLI verb
`protocol trace check|evidence`.* Two crates rather than one because the infra family's split
between an observation model and an expectation model earned its keep, and because an adapter and a
checker have different reasons to change. It is not part of `aep-domain`: a transcript is an
observation of a harness, not a protocol concept.

**D5 — streaming.**
*Default: deferred, named.* Batch only in v0.1. A streaming checker would want incremental
evaluation, partial verdicts and a halt signal, and none of those is designable against a format
that is not yet stable (**D1**).

**D6 — resource bounds in CI.**
Cost, tokens, duration and TTFT all vary run to run with model routing, cache state, service tier
and load — the observed run alone shows two models, a 99.99% cache read ratio, and
`duration_api_ms` *exceeding* `duration_ms`. *Default: bounds only, never equality; generous bounds;
`unk` on a missing field, never a gap.* A cost expectation exists to catch a run that looped for
forty minutes, not to detect a 12% regression. Concretely: set a bound at roughly three times the
observed value, write the observation into the expectation's comment so the next reader knows what
it was calibrated against, and treat any resource expectation that fires as an invitation to look at
the transcript rather than as a defect report. A CI job that goes red because a cache was cold is a
CI job people learn to ignore, and that is a worse outcome than having no bound at all.

---

## 9. Milestones, unsequenced

**Not accepted for build.** A plan page must take these up before any of them is work, and the plan
page — not this document — decides the order and the acceptance criteria.

| | what it delivers |
|---|---|
| **T1** | `trace-ir/1`, content-addressed, and the Claude Code `stream-json` adapter — all six observed event families of § 2.2, not just the three the first draft named. Acceptance would be a round trip over a committed real transcript in which the census is reproduced exactly (1 init / 21 assistant / 14 user / 8 thinking / 1 rate-limit / 1 result), every event is either typed or opaque, and the digest is stable across two reads |
| **T2** | `trace-spec/1` — the raw→validated pair, the schema, and `protocol trace check` with the three verdicts and the 0/1/3 exits. Acceptance would be every expectation kind exercised on a committed fixture **with a negative case beside each**, which is the standard `infra-spec` set for itself |
| **T3** | `protocol trace evidence`, the `EvidenceKind` addition, and rewiring the plugin eval onto `expectations.trace.yaml`. Acceptance would be the eval's behavioural half passing as a document, and the grep deleted |

The dependency worth noting for whoever sequences this: **T3 is what the Phase-2 driver needs**
(§ 5.2), and T1 and T2 are worth having on their own. If the driver is never built, T1–T2 still
replace a grep with a document. That is a smaller prize, and it is not nothing.
