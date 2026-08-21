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
| `hooks/` | two `PreToolUse` hooks — the deterministic half of the guardrails. `store-integrity.sh` keeps the planning store's frontmatter the CLI's; `driven-surface.sh` holds a driven run's shell to the `protocol` verbs. See below |
| `eval/` | two repeatable checks that the plugin works: `eval/run.sh` drops a headless agent into a scratch project and inspects what it created; `eval/run-driven.sh` does the same for a whole `protocol drive` run, hooks included. Both cost API money and neither is part of `task check`. See [eval/README.md](./eval/README.md) |

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

## The hooks, and what changed about "no hooks"

Earlier versions of this file said the plugin ships **no hooks**, and gave a good reason: a hook
layer would be *a second, weaker driver* — one that sees tool calls rather than workflow states and
cannot ask the engine anything, because it has no execution to ask about. Two mechanisms both
claiming to enforce the same thing is worse than one.

**That reasoning is unchanged. What changed is that the driver exists.** A hook is no longer a
second driver: it is the driver's enforcement arm, configured *by* the driver, per state, for a run
that is holding an execution. `--allowedTools` governs which tools a session is *offered* and is
fixed at launch; a hook is the only layer that sees a call's **arguments**. Two layers with
different failure modes, which is this project's enforce-and-verify argument applied one level down
rather than belt-and-braces.

| hook | matcher | what it does | active when |
|---|---|---|---|
| `hooks/store-integrity.sh` | `Edit\|Write\|NotebookEdit` | denies `Write` and `NotebookEdit` anywhere under `.engineering/planning/**`, and denies an `Edit` whose `old_string` or `new_string` crosses the `---` fence or writes a machine-owned field (`id`, `kind`, `status`, `revision`, `relations`, `format`). A targeted body edit is allowed — guardrail 2 says the body is yours | **always** |
| `hooks/driven-surface.sh` | `Bash` | denies any shell command that is not one simple invocation of `protocol artifact …` or `protocol trace …` — no pipes, no redirection, no `&&`, no command substitution | **only inside a `protocol drive` run** |

The second one is inert outside a driven run on purpose: a per-state rule with no state to read
would be exactly the second, weaker driver the paragraph above refuses to ship. The first one is
not a per-state rule at all — it reads no workflow state and asks nothing — so it holds everywhere.

**Both hooks refuse rather than pass a call through unread.** Every rule they hold is about a tool
*argument*, so they need `jq` or `python3`; with neither, a call they would have adjudicated is
denied with a reason naming the missing dependency. Each hook's own header carries its full
reasoning, including what it deliberately does not claim.

**Every decision goes to `<run>/hook-decisions.jsonl`** when there is a run to write to — one JSON
line per adjudicated call, allow and deny alike. A `PreToolUse` hook is a separate process and
cannot call `Engine::authorize`, which mutates an in-memory execution inside the driver; the log is
the channel by which its decisions reach the run's record. It is also the only record that can tell
*denied* from *never attempted*, which the transcript's whole-run denial counter cannot.

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
