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
| `eval/` | a repeatable check that the plugin works: `eval/run.sh` drops a headless agent into a scratch project with the plugin, runs a fixed dummy planning task, and mechanically inspects what it created. Costs API money; never part of `task check`. See [eval/README.md](./eval/README.md) |

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

**No hooks.** A hook is deterministic interception — it fires whether or not a model cooperates,
which is exactly the right mechanism for enforcing that a status never gets hand-edited. It is also
the workflow driver's job, and the driver is not built yet. Shipping a hook now would put enforcement
in the harness and the workflow in the library, and the two would disagree the first time either
moved. Until the driver exists, guardrail 1 in the skill is a rule the model follows and
`protocol artifact validate` is the thing that catches it when it does not.

**No `commands/` directory.** The CLI is the command surface. A `/plan-new` slash command wrapping
`protocol artifact new` would be a second place for the verb to live, and the second place is where
the drift starts: a flag gets added to the CLI, the command does not learn about it, and the model
now has two spellings of the same operation with different capabilities. The skill teaches the CLI;
the CLI stays the only definition of what the CLI does.

## Deferred, on purpose

Phase 1 covers planning only — writing down what is to be done. Two pieces arrive with the workflow
driver, and not before, because both need an engine to be correct rather than plausible:

* **a `governed-task` skill** — the evaluate/advance loop: what is owed in the current state, what
  evidence satisfies it, when a transition is legal. That loop is answered by `aep-engine`, and a
  skill that approximates it from prose would produce an agent that believes it finished tasks
  nothing verified.
* **`implementor` and `verifier` agents** — an implementor needs the capability policy in force to
  know which tools it may use, and a verifier's output is only worth anything if it lands as evidence
  with a `Producer` the engine can distinguish from the agent's own. Both are engine facts. Until the
  driver exposes them, the honest surface is the one here: plan the work, and let a person run it.
