# Harness wave 1 — the planning store and the Claude Code plugin

> **Accepted for implementation, 2026-08-21.** Design:
> [`harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md),
> **Phase 1 sections only**; the Phase 2 driver is decided (vision item V-5) and explicitly *not*
> accepted for build by this page. The review is the operator's in-session plan approval, recorded
> here and in [`control-document-updates.md`](control-document-updates.md); the driver build gets a
> full feasibility review before any build wave opens.

**Goal: an operator and Claude, with the plugin enabled and the `protocol` CLI on `PATH`, plan real
work in `.engineering/planning/` — and every status move is lifecycle-validated.**

This is the first wave of a new family. `harness` is not ESS and not infra: it is the layer this
repository has published a contract for and never implemented, and wave 1 builds the part of it that
is useful on its own — a place to put planning artifacts, and something that knows how to use it.

## What this wave is, in one sentence each way

For the person adopting this: your backlog stops being a wiki page and a tool that has never heard
of your lifecycles, and becomes markdown in your repository that refuses an illegal move and tells
you which moves are legal instead.

For the machinery: the third of `is_planning`'s six kinds gets a store, the four `adp-domain`
commands get entities they could eventually address, and the driver of harness wave 3 gets the
artifact source it cannot be built without.

## Decisions, taken

| decision | taken as | why |
|---|---|---|
| where the store lives | `.engineering/planning/<kind>/<slug>.md`, one directory per kind, no index file | `.engineering/` is already the project's directory (`project.yaml` is discovered there). The directory *is* the index: a second copy of the membership list is a second thing that can disagree, and an index file conflicts on every branch that adds an artifact |
| how deep the backend goes | a **plain store**, not a contract implementation. `CommandService`/`QueryService` come at **P3** | building the envelope surface first means building the journal first, and the store would not exist for three milestones. The cost is recorded as deviation **D-P1**, with the mitigation that every write funnels through two functions P3 reroutes |
| domain purity | the frontmatter format `aep.planning-md/1` is **owned by `aep-backend-markdown`**; `aep-domain` gains zero types for it | the format is one backend's, and no other backend is obliged to store anything this way. It is owned, not hidden: the file is *authored* — somebody types into it — and every authored document here has a generated schema, so `RawPlanningFrontmatter` is published as `schemas/generated/planning-document.schema.json` and held to its type by `schema-check`. The seam between backends stays the contract traits (design open decision **D1**) |
| the id scheme | declared `id` = `<kind>:<slug>`, checked against the path; **no counters, no allocator** | two branches both allocate `18`, both merge cleanly, and the store holds two artifacts with one id — a corruption git cannot see because nothing was in conflict. A slug collides only when two people meant the same thing, and then git conflicts on the path. External ticket names (`story:dev-399`) stay legal because nothing parses an id |
| timestamps | none in the file | git carries authorship and time and cannot be edited by writing a number into a file; a stale `updated:` reads as an observation; and `SEED_AT: Timestamp::EPOCH` (`crates/protocol-cli/src/main.rs:51`) is the standing precedent — a wall clock in diffable output is diff noise. The cost, "how long has this been in draft", is answered by `git log` until P3's journal |
| new lifecycles | `epic`, `task`, `initiative`, mirroring `story.yaml`'s ladder | every word is an existing `ArtifactStatus` variant, so it is four documents and zero domain changes. One ladder across the four planning kinds means an operator learns it once |
| where the plugin lives | `integrations/claude-code/`, plus `.claude-plugin/marketplace.json` at the repository root | a consumer-named deliverable, the shape `website/` already has: built beside the specification, consuming only its public surface. Plural `integrations/` because the first one should not sit at a path that moves when the second arrives |
| one skill, not three | exactly one skill, `planning` | a skill is a decision about *when instructions load*, and there is one moment: the operator is planning. Three skills would triple the trigger surface and let a session load the one missing the guardrail it was about to break |
| discover, do not memorise | the skill inlines **rules only** — `kinds`, `relations`, `lifecycle`, `list` answer every vocabulary question at use time | lifecycles and relations are validated, versioned documents; a prose copy in a skill file is neither. It goes stale the first time a kind gains a status, and the failure is confident and silent. A prose copy of a validated document is drift with a nice font |
| no hooks | the plugin ships no hooks, on purpose | deterministic interception is the driver's job. A hook layer would be a *second, weaker driver* — one that sees tool calls rather than workflow states and cannot ask the engine anything — and it would have to be deleted or reconciled when the real one lands |
| no `commands/` | the CLI is the command surface | a slash command wrapping `protocol artifact new` is a second spelling of one verb, and two spellings drift |
| V-5 lands in this wave | the vision narrowing is applied now, before the driver is built | the decision was taken in session, and a decision recorded three months after it was taken is a decision nobody can audit. The VISION text says plainly that the driver is *designed, not yet built* |
| the fixture is a contrast, not a copy | `examples/planning-passkeys/` — the same feature `examples/development-passkeys/` governs, planned rather than executed | the two directories side by side are the argument: one shows a task being held to a protocol, the other shows the work being decomposed before there is a task. Reusing the passkey subject means the reader compares mechanisms, not domains |

## W1.1 — the store, the verbs, the lifecycles

`crates/aep-backend-markdown`: the `aep.planning-md/1` frontmatter as a `Raw*` type validated through
`TryFrom` into the domain types that already exist, unknown keys preserved, and every write funnelled
through `create` and `update` so there is exactly one place inside the crate where validation,
revision bumping and serialisation happen.

`protocol artifact` in `protocol-cli`: `new`, `move`, `relate`, `list`, `board`, `graph`, `validate`,
`kinds`, `relations`, `lifecycle`. `move` is validated against the kind's lifecycle document; a
refusal names the legal set. `new` writes its body from `artifacts/templates/` where the kind has a
template.

`artifacts/lifecycles/` gains `epic.yaml`, `task.yaml` and `initiative.yaml`.

**Acceptance:**

* a round trip holds — an artifact written by `new`, read back, and written again is byte-identical,
  and an unknown frontmatter key survives both passes;
* an illegal move is refused **naming every legal target from the current status**, and the store is
  unchanged afterwards;
* `validate` accumulates: a store with four broken relation targets reports four, not one;
* `task check` is green — all nine steps, including `schema-check` and `clippy -D warnings` on the
  new crate.

## W1.2 — the plugin: skill and scaffold

`integrations/claude-code/` with `.claude-plugin/plugin.json`, the `planning` skill, and its
`references/store-conventions.md`. Repository-root `.claude-plugin/marketplace.json` so the plugin
is installable by name.

The skill carries the four guardrails — status via the CLI only, the body is free, validate and relay
verbatim, a refusal is the answer — and no vocabulary at all.

**Acceptance** (repeatable, and deliberately outside `task check`):

a fresh Claude Code session with the plugin enabled and the CLI on `PATH` creates one epic and two
stories derived from it, performs one legal status move **through `protocol artifact move` and by no
other means**, and relays an illegal-move refusal to the operator verbatim rather than routing around
it.

`integrations/claude-code/eval/run.sh` is that check, scripted: a headless `claude -p` run in a
scratch directory, followed by **mechanical inspection of the store it left behind** — the artifacts
that exist, their statuses, their edges, and whether `protocol artifact validate` passes. The
assertions are about files, not about wording, because the behaviour under test is a model's and an
assertion on its prose would be an assertion on a sentence that is allowed to vary.

It is **not a step of `task check`**, and that is a rule rather than a convenience: the gate reaches
no network (`AGENTS.md` § *Dependencies*), and this one calls a model. A gate that went red because
an API was slow would be a gate people learn to ignore. It is run deliberately, and its output is
recorded with the wave.

## W1.3 — the two agents

`decomposer` and `plan-reviewer`, each with a charter that is also its bound.

**Acceptance:**

* `decomposer` run against an epic produces **only draft stories, each linked to the epic**, moves
  nothing, and leaves a store that `protocol artifact validate` passes;
* `plan-reviewer` run against the same store **changes zero files** — asserted by `git status` being
  clean after the run, not by reading the agent's definition.

Both are cases in `eval/run.sh`, and both are checked the same way W1.2 is: by inspecting the scratch
store afterwards. "The decomposer moved nothing" is a statement about statuses in files, and
"the reviewer changed nothing" is a statement about a clean tree — neither needs the transcript read.

## W1.4 — the install path, the documents, the changelog

The install path written down where a person will find it, and followed once from a clean checkout:
install the plugin from the marketplace entry, build the CLI, run `protocol artifact new`.

Documents: the design doc's row in `README.md` § *Documents* and in `AGENTS.md`'s acceptance table;
the plugin and the `protocol artifact` verbs mentioned where the README lists what works; the vision
narrowing applied; `docs/plan/gap-register.md` carrying the two rows this wave opens;
`CHANGELOG.md` under `## [Unreleased]`.

**Acceptance:** a clean checkout can follow the written path end to end without asking anyone a
question. Nothing in the path refers to a file that is not in the repository, and no step assumes a
directory the reader has not been told to create. The path names `eval/run.sh` and says plainly what
it needs — a `claude` binary, credentials and a network — so that a reader who cannot run it knows
that is why, rather than concluding it is broken.

## Harness wave 2 — driver design-and-decide (not a build wave)

**This wave produces decisions and a review, not a crate.** It exists because
`harness-planning-and-driver-design-v0.1.md` § 4 is architecture with six named holes in it, and
because this repository's ordering rule — do not build from an unreviewed design — is the rule that
kept wave 4 from generating code against an oracle nobody had watched fail.

The six hard problems, which are wave 2's agenda:

1. **step-map versioning against workflow versions** — what a map pinned to `adp/default@1` does when
   the workflow reaches `@2`, and what happens to a run in flight;
2. **store → facts** — rebuilding an `ArtifactGraph` between steps so the next evaluation sees the
   store as it now is, and what that costs per step;
3. **`require_approval` under a headless run** — stop, queue, or refuse to start a run whose path
   crosses an approval;
4. **session granularity** — one model session per state or per step, and the resume semantics that
   fall out of the answer;
5. **failure taxonomy** — a crashed run is `Unknown`, a failing suite is `False`, and every ambiguous
   failure has to land on one side; plus retry budgets, where a retried success must not erase the
   first attempt's failure;
6. **concurrency and locking** — one execution per store behind a lockfile, and what a stale lock
   from a crashed run does to the next operator.

**Wave 2's exit condition is a feasibility review of § 4 against the code**, in
`docs/reviews/`, of the kind ESS waves 4 and 6 went through. A decision not to build the driver is a
legitimate outcome of that review and closes the gap-register row just as building it would.

**Harness wave 3 — the driver build — is unsequenced and sits behind wave 2.** It is named here so
that the register has something to point at, and for no other reason.

## What is deliberately not in this wave

* **The driver**, in any part. No `aep-driver`, no `protocol drive`, no `drivers/` documents, and no
  `.engineering/runs/` writer — the directory name is reserved and nothing writes to it.
* **Contract conformance for the store.** The sixteen `aep-conformance` suites are not run against
  `aep-backend-markdown`, because it does not implement the contract yet. That is P3, and the
  gap-register row says so.
* **Hooks, slash commands, or a second skill.** Each has an argument against it in the decisions
  table above; adding one later means overturning the argument, not filling a blank.
* **Enforcing `relations.yaml` pairings.** `validate` checks that a target resolves; it does not
  refuse an unusual pairing. Turning that document's advisory lists into refusals changes the meaning
  of a shared document for every consumer of the artifact graph, and it gets its own decision
  (design open decision **D2**).
* **A `protocol entity --planning` bridge.** The two surfaces answer different questions until the
  store implements the contract, and at P3 the bridge is not a bridge — it is the store answering as
  a backend (design open decision **D3**).
