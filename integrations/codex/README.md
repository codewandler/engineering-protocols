# Engineering Protocols — Codex variant

The same instructions as the [Claude Code plugin](../claude-code/), in the form Codex reads them.
Plan engineering work in a markdown artifact store governed by this repository's `protocol` CLI:
epics, stories, tasks and initiatives as files under `.engineering/planning/`, with kinds, statuses
and legal moves supplied by validated documents rather than by convention.

It carries rules and **no vocabulary** — no list of kinds, no status ladder, no relation names.
Those are read at use time from `protocol artifact kinds`, `protocol artifact relations` and
`protocol artifact lifecycle <kind>`, because a prose copy of a validated document is a copy that
goes stale, and drift is the thing this project exists to refuse. That rule is the same one on both
harnesses, and it is the reason a second harness is a port of instructions rather than a rewrite of
them.

**This is not the second adapter, and it is not
[wave 4's W4.4](../../docs/plan/harness-wave-4-governed-dogfood.md).** It is the instruction surface
only. `trace-spec` still reads exactly one transcript format, so a Codex run cannot yet be judged by
`protocol trace check` — [`eval/README.md`](./eval/README.md) says what that costs and what closes
it.

## What is in it

| Component | What it does |
|---|---|
| [`skills/planning/SKILL.md`](./skills/planning/SKILL.md) | the port of the Claude Code skill: the model, the four guardrails, the discovery rule and a worked decomposition. Loaded on demand |
| [`AGENTS.planning.md`](./AGENTS.planning.md) | the same guardrails and the same discovery rule as an `AGENTS.md` fragment, which Codex reads on every turn without anything being invoked |
| [`.codex-plugin/plugin.json`](./.codex-plugin/plugin.json) | the plugin manifest, for the marketplace install route |
| [`eval/`](./eval/) | one check that costs nothing: the manifest and the skill are well-formed, and the instructions reach the model. No live run — see below |

**Two files carry the four guardrails and nothing else is duplicated.** The skill is loaded when
Codex decides the task is a planning task; the `AGENTS.md` text is in context whether it decides that
or not, and a rule that only holds once a skill is opened does not hold in the turn where the model
decides not to open it. Everything past the guardrails — the model, the discovery table's reasoning,
the worked example, the on-disk format — lives once, in the skill.

## Prerequisite: `protocol` on `PATH`

The variant is a set of instructions for driving a CLI; without the CLI it does nothing. From a
checkout of this repository:

```console
$ cargo install --path crates/protocol-cli
$ protocol --version
```

The crate is `protocol-cli`; the binary it installs is `protocol`.

## Install

**As a repository skill** — the route this repository has actually exercised. From your project's
root:

```console
$ mkdir -p .agents/skills
$ cp -R <checkout>/integrations/codex/skills/planning .agents/skills/planning
$ cp <checkout>/integrations/claude-code/skills/planning/references/store-conventions.md \
     .agents/skills/planning/references/store-conventions.md
$ cat <checkout>/integrations/codex/AGENTS.planning.md >> AGENTS.md
```

The second `cp` is the on-disk format reference, which is one file in this repository and is
deliberately not duplicated under `integrations/codex/` — a second copy of a document is a document
that goes stale, which is the skill's own § 2 argument applied to this tree. `.codex/skills/` and
`$CODEX_HOME/skills/` work as roots too; a bare `skills/` does not.

**As a plugin** — the manifest is here and validates, but this route is **unverified**: installing a
plugin writes to the operator's marketplace file and Codex configuration, and a check that mutates
the machine it is auditing is not a check. The vendor's own material is the reference, and the two
paths it names are `~/.agents/plugins/marketplace.json` for a personal plugin and
`<repo-root>/.agents/plugins/marketplace.json` for a repository one. This repository ships **no**
marketplace file for Codex; its `.claude-plugin/marketplace.json` is Claude Code's.

## What is the same, and what cannot be

| | Claude Code plugin | Codex variant |
|---|---|---|
| the instructions | `skills/planning/SKILL.md` | ported, one file, same four guardrails and same discovery rule |
| always-on instructions | none shipped — the plugin is a skill and two hooks | `AGENTS.planning.md`, because Codex reads a project instruction file on every turn and the port has somewhere to put the guardrails that does not depend on the model choosing to open a skill |
| the vocabulary | read from the CLI at use time | **identical**, and this is the point: the rule is about the CLI, not about the harness |
| a command surface | none, deliberately — the CLI is the command surface | none, same argument |
| enforcement hooks | two `PreToolUse` hooks, shipped | **not shipped.** The mechanism exists on Codex; the rules do not port. See below |
| sub-agents | `agents/decomposer.md`, `agents/plan-reviewer.md` | **not ported** — no verified equivalent file format |
| a live eval | `eval/run.sh`, judged by `protocol trace check` | **not built** — no Codex adapter in `trace-spec` |
| a free check | `eval/checks/` | `eval/check-instruction-surface.sh` |

## Every Codex mechanism this file asserts, and how it is known

Verified here means: run against **codex-cli 0.145.0** on 2026-08-22, with the command in the row.
Vendor doc means: read out of the material the CLI itself ships. Unverified means nobody here has
seen it work, and it is written down as unverified rather than left to read like a fact.

| claim | how known |
|---|---|
| the version everything below was checked against is `codex-cli 0.145.0` | verified — `codex --version` |
| a project-root `AGENTS.md` is injected into the model-visible prompt, as `# AGENTS.md instructions for <path>` wrapped in `<INSTRUCTIONS>` | verified — `codex debug prompt-input` in a scratch fixture; asserted by `eval/check-instruction-surface.sh` |
| the walk is root-to-cwd, one file per directory, `AGENTS.override.md` wins, concatenated root-first | vendor doc, via [the research record](../../docs/reviews/2026-08-21-codex-harness-research.md). **Only the root file is verified here** |
| a skill is a directory holding `SKILL.md` with YAML frontmatter carrying a non-empty `name` and `description` | verified — the vendor's validator enforces exactly those two, and `codex debug prompt-input` lists the skill by them |
| only the **description** enters context; the body is read on demand from the path Codex prints beside it | verified — the `<skills_instructions>` block lists `name: description (file: …/SKILL.md)` and nothing else |
| skill roots: `<repo>/.agents/skills/`, `<repo>/.codex/skills/`, `$CODEX_HOME/skills/`. A bare `skills/` is not one | verified — one fixture per root |
| a scratch `CODEX_HOME` keeps the operator's own skills out of a run, the way `CLAUDE_CONFIG_DIR` does. Codex's own `.system` skills still appear | verified — with it, one skill is offered; without it, six more |
| a plugin is a directory with `.codex-plugin/plugin.json`, optionally `skills/`, `hooks/`, `scripts/`, `assets/`, `.mcp.json`, `.app.json`; `skills` and `hooks` paths *supplement* default discovery rather than replacing it | vendor doc — the `plugin-creator` skill shipped in the CLI |
| the manifest requires `name`, strict-semver `version`, `description`, `author.name`, and an `interface` block with `displayName`, `shortDescription`, `longDescription`, `developerName`, `category`, `capabilities` and `defaultPrompt` | verified — `validate_plugin.py`, which is what this variant's manifest is checked by |
| **the vendor's material contradicts itself about `hooks` in the manifest**: the shipped spec lists `hooks` as a top-level field, and the shipped validator rejects it as "not accepted by plugin validation" | verified — both read, both shipped in the same CLI. Not resolved here; this manifest omits `hooks`, which satisfies both readings |
| Codex has a `PreToolUse` hook, stable and enabled by default | verified — `codex features list` prints `hooks stable true` |
| the hook **output** wire is Claude Code's shape: `hookSpecificOutput.{hookEventName, permissionDecision, permissionDecisionReason, updatedInput}` beside `continue`, `stopReason`, `suppressOutput`, `systemMessage` | verified — the `pre-tool-use.command.output` JSON Schema embedded in the binary |
| a `deny` **must** carry a non-empty `permissionDecisionReason`; `permissionDecision: ask` is refused for `PreToolUse` | verified — the binary's own refusal strings |
| the hook **input** carries `cwd`, `hook_event_name`, `model`, `permission_mode`, `session_id`, `tool_input`, `tool_name`, `tool_use_id`, `transcript_path`, `turn_id`, all required; `tool_input` is unconstrained | verified — the `pre-tool-use.command.input` schema embedded in the binary |
| hook config lives in `~/.codex/hooks.json`, `<repo>/.codex/hooks.json` or `config.toml`, and a plugin bundles `hooks/hooks.json`; entries carry `matcher`, `command`, `timeout` and friends | vendor doc plus binary strings; **not exercised** |
| a non-managed hook needs explicit trust, and `--dangerously-bypass-hook-trust` exists for automation | verified — `codex --help` |
| Codex's file-writing tool is `apply_patch` and its shell tool is the exec/unified-exec family. There is no native `Write`, `Edit` or `NotebookEdit` | verified — binary strings, including *"apply_patch was requested via … instead of exec_command"*. This is the fact that stops the hooks porting |
| the transcript worth adapting is the session rollout JSONL under `$CODEX_HOME/sessions/`, not `codex exec --json` stdout, which carries no timestamps, no durations and no cost | vendor doc plus a local corpus — [the research record](../../docs/reviews/2026-08-21-codex-harness-research.md), 2,437 rollout files |
| the rollout format has no documented stability guarantee, and drift is already observable across one install's own history | vendor-adjacent observation, same record. It is why an adapter must version-gate on `session_meta.cli_version` and treat an unknown shape as opaque |
| `-p/--profile <name>` layers `$CODEX_HOME/<name>.config.toml`; the older `[profiles.…]` tables in `config.toml` are a **legacy selector** Codex refuses to write | verified — `codex --help` and the binary's own refusal string. Anything describing Codex profiles as `[profiles.x]` blocks in `config.toml` is describing an older release |
| the binary carries `.claude-plugin/plugin.json` and `.cursor-plugin/plugin.json` beside its own `.codex-plugin/plugin.json`, and it carries `CLAUDE_PLUGIN_ROOT` / `CLAUDE_PLUGIN_DATA` with no `CODEX_PLUGIN_ROOT` anywhere | observed in the binary's strings, **unverified as behaviour**. It suggests a compatibility path for a Claude Code plugin directory, and if one exists it would matter for a hook port — `../claude-code/hooks/hooks.json` already spells its command as `${CLAUDE_PLUGIN_ROOT}/hooks/…`. Nothing here tested it, and the install route above does not rely on it |

Nothing about Codex's sub-agent definition format, its slash-command surface or its approval-event
wire shape is asserted anywhere in this variant, because none of it was verified. The last of those
is an open question the research record names explicitly.

## What is deliberately absent

**No hooks — and the reason is a tool name, not a missing mechanism.** Codex's `PreToolUse` hook is
stable, enabled by default, and speaks the same decision wire as Claude Code's; the deny path would
port line for line. What does not port is *what the hooks read*. Every rule in
`../claude-code/hooks/` is a rule about a tool **argument**: `store-integrity.sh` reads
`tool_input.file_path` and inspects `old_string` and `new_string`, and `driven-surface.sh` reads
`tool_input.command`. Codex writes files through `apply_patch`, whose input is a patch envelope with
none of those keys. A port would therefore look at every store write, find no `file_path`, and pass
it through — a guard that has silently stopped guarding, which is the defect this repository writes
registers about. Closing it needs one recorded `apply_patch` hook invocation to fix the shape of
`tool_input`; nothing else is missing.

`driven-surface.sh` is blocked twice over: it is inert outside a `protocol drive` run, and it finds
that run through `AEP_DRIVE_STEP_CONTEXT`, which the driver exports onto a `claude` child. There is
no Codex `LlmStepExecutor`, so there is no child to export it onto.

**No agents.** `agents/decomposer.md` and `agents/plan-reviewer.md` are Claude Code sub-agents:
frontmatter carrying a name, a description and a **tool allowlist**, which is what makes the
plan-reviewer read-only. Codex 0.145.0 does have a multi-agent surface, and a skill may carry an
`agents/openai.yaml` — but that file is presentation metadata (`display_name`, icons,
`default_prompt`), not a persona with a tool allowlist, and no file format for the latter was
verified here. A "read-only reviewer" whose read-only-ness is a sentence rather than a grant is a
different artifact from the one on the Claude side, and shipping it under the same name would
overstate what holds.

**No `commands/` and no slash commands.** The same argument the Claude plugin makes: the CLI is the
command surface, and a wrapper is a second place for a verb to live and the first place drift starts.

**No copy of `store-conventions.md`.** One file, in `../claude-code/skills/planning/references/`,
harness-neutral, referenced from both skills. The install step above copies it.

**No live eval.** [`eval/README.md`](./eval/README.md) § *What is not built, and why*.

## What would have to change outside this directory

Named because none of it is here, and a reader should not have to infer it from silence.

* **A second adapter in `crates/trace-spec/`** — a `read_transcript` over the session rollout JSONL,
  declaring its own `AdapterRef` (`codex/rollout-jsonl`, written against `0.145.0`) so a report says
  which reader produced a verdict. That is W4.4's *partial* tier, and it is what turns a Codex run
  into something `protocol trace check` can decide against the **same** specification file.
* **A Codex `LlmStepExecutor`** — the driver's other adapter point, for a driven run under Codex.
  `tool_config` is deliberately not re-implemented: § 4.9 keeps it a pure function so a second
  harness cannot quietly re-decide what a capability admits.
* **The design's § 4.8 portability claim** — the plan page records that Codex's hook contract
  strengthens it from *"three adapter points"* to *"three adapter points plus one hook contract that
  holds on both harnesses"*, and says the strengthening is owed to the design document, which the
  plan page does not own.
