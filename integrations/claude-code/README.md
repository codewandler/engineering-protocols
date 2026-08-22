# Engineering Protocols — Claude Code plugin

Teaches Claude Code to plan engineering work in a markdown artifact store governed by this
repository's `protocol` CLI: epics, stories, tasks and initiatives as files under
`.engineering/planning/`, with kinds, statuses and legal moves supplied by validated documents rather
than by convention.

The plugin carries rules. It deliberately carries no vocabulary — no list of kinds, no status ladder,
no relation names. Those are read at use time from `protocol artifact kinds`, `protocol artifact
relations` and `protocol artifact lifecycle <kind>`, because a prose copy of a validated document is
a copy that goes stale, and drift is the thing this project exists to refuse.

## What is in it

| Component | What it does |
|---|---|
| `skills/planning/` | the model, the four guardrails, and how to discover the store's vocabulary. Auto-triggers on planning talk or a `.engineering/planning/` directory; also invocable as `/engineering-protocols:planning` |
| `agents/decomposer.md` | takes one epic id, drafts the stories that jointly cover it, each with an acceptance statement. Creates drafts only — never moves an artifact, never touches one it did not create |
| `agents/plan-reviewer.md` | read-only semantic audit: stories that no longer cover their epic, finished epics still open, stale work, missing acceptance statements. Proposes moves, performs none |

The hooks and the eval that used to live here **migrated to the metaharness repository**
(`epic:metaharness-migration`, 2026-08-22). The hooks' policy is now Rust inside the driver —
`decide_tool` in `crates/protocol-cli/src/drive.rs`, answering the metaharness seam per call —
and the eval machinery, its recorded transcripts and its results live in metaharness under
`evals/engineering-protocols/`. See "Enforcement" below.

## Prerequisite: `protocol` on `PATH`

The plugin is a set of instructions for driving a CLI; without the CLI it does nothing. From a
checkout of this repository:

```console
$ cargo install --path crates/protocol-cli
$ protocol --version
```

The crate is `protocol-cli`; the binary it installs is `protocol`.

## Install

**Local development** — point Claude Code at the plugin directory, from the repository root:

```console
$ claude --plugin-dir ./integrations/claude-code
```

**From GitHub** — add the marketplace defined at the repository root, then install from it:

```
/plugin marketplace add codewandler/engineering-protocols
/plugin install engineering-protocols@engineering-protocols
```

The marketplace and the plugin share a name; the `@` form is `<plugin>@<marketplace>`.

## What is deliberately absent

**No `commands/` directory.** The CLI is the command surface. A `/plan-new` slash command wrapping
`protocol artifact new` would be a second place for the verb to live, and the second place is where
the drift starts: a flag gets added to the CLI, the command does not learn about it, and the model
now has two spellings of the same operation with different capabilities. The skill teaches the CLI;
the CLI stays the only definition of what the CLI does.

## Enforcement, and where the hooks went

Earlier versions of this plugin carried two `PreToolUse` shell hooks — `store-integrity.sh` and
`driven-surface.sh` — as the driver's enforcement arm. Both retired on 2026-08-22 under
`epic:metaharness-migration`, and the reasoning that once kept hooks *out* of this plugin is what
retired them: a hook process could not call `Engine::authorize` and wrote its decisions to a side
log the driver folded in late, and a session launched without the plugin ran unenforced while
looking clean (run `W4-2` paid for eight such sessions).

The same rules hold today, one level down and in one place:

| rule | where it lives now | active when |
|---|---|---|
| the planning store's frontmatter is the CLI's: `Write`/`NotebookEdit` denied under `.engineering/planning/**`, an `Edit` denied when it crosses the `---` fence or writes a machine-owned field | `store_integrity` in `crates/protocol-cli/src/drive.rs`, answering the metaharness seam at decision time | every driven `llm` step |
| a driven shell is one simple invocation of `protocol artifact …` or `protocol trace …` — no pipes, no redirection, no substitution | `driven_surface`, same place | every driven `llm` step |
| a tool no admitted capability renders to is refused naming the state's surface | `decide_tool`, same place — the allowlist that used to ride on `--allowedTools` | every driven `llm` step |

The decisions are `tool.decided` events in the run's own event stream — the transcript the driver
writes — so *denied* and *never attempted* are distinguishable from the record itself, and there
is no `hook-decisions.jsonl` to forget. Outside a driven run the store's protection is what it
always also was: `protocol artifact validate`, which catches an illegal write whether or not any
seam fired.

## Deferred, and why that is no longer a wait

The skill covers planning only — writing down what is to be done. Earlier versions of this file
deferred two further pieces until the workflow driver existed. **The driver now exists**, and its
arrival did not turn them into work items; it answered one of them and moved the other.

* **A `governed-task` skill is not coming, because `protocol drive` is that loop.** What is owed in
  the current state, what evidence satisfies it and when a transition is legal are `aep-engine`
  questions, and the driver asks them. A skill that approximated the same loop from prose would be
  a second, weaker protocol implementation with none of the conformance suites — and the first time
  the two disagreed, the untested one would be the one holding the session.
* **`implementor` and `verifier` agents are still absent, and now for a narrower reason.** The
  driver already puts the capability policy in force per state, so an implementor no longer has to
  discover what it may use. What is missing is a verifier whose output the engine can tell apart
  from the agent's own: `independent: true` is checked structurally, and nothing signs a record.
  Until that gap closes — `gap-register.md` D-3, proposed and unaccepted — a verifier agent would
  produce evidence that looks independent and is not.

The honest surface today is the one here, plus the driver: plan the work with the skill, and run it
with `protocol drive`, where the enforcement is a program rather than a paragraph.
