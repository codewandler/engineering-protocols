# Harness — a planning store, and a reference driver — Design v0.1

> **Repository:** `codewandler/engineering-protocols`
> **Status:** **Phase 1 accepted for implementation** by
> [`docs/plan/harness-wave-1-planning-plugin.md`](../plan/harness-wave-1-planning-plugin.md), 2026-08-21.
> **Phase 2 is decided and designed, and is not accepted for build** by that page or by any other:
> the vision narrowing it depends on is recorded (V-5 in
> [`control-document-updates.md`](../plan/control-document-updates.md)), and the build waits behind
> its own feasibility review.
> **Audience:** an implementor who has read [`docs/guide/harness.md`](../guide/harness.md) and knows
> the seven calls; and anyone deciding whether the driver should be built at all.
> **Relationship to existing design:** additive. It replaces nothing in
> [`consolidated-design-v0.2.md`](consolidated-design-v0.2.md) or
> [`reconciliation-v0.2.md`](reconciliation-v0.2.md), and it changes one line of
> [`docs/VISION.md`](../VISION.md), deliberately and with the argument written down.

**Why one document for two phases.** The Phase 1 store is the Phase 2 driver's artifact source: the
driver walks a workflow over planning artifacts, and if those artifacts live nowhere, the driver has
nothing to walk. Splitting this into two documents would let the store be built as a filing cabinet
and the driver be designed against an imagined one. The dependency is the point, so it is on one
page — and the acceptance boundary runs *inside* the page rather than between two of them, which is
why the status note above is as emphatic as it is.

---

## 1. Motivation and the contract gap

This repository publishes two contracts to the outside world.

| Contract | Where it is written | Who implements it here |
|---|---|---|
| **Storage** — commands, queries, audit | `crates/aep-contract`, [`docs/guide/backend.md`](../guide/backend.md) | `aep-backend-memory`, held to sixteen conformance suites and checked against a deliberately faulty backend |
| **Harness** — seven calls, three rules | [`docs/guide/harness.md`](../guide/harness.md), and `docs/design/consolidated-design-v0.2.md:4443` (*"The harness executes."*) | **nothing** |

The asymmetry is the defect. `AGENTS.md` § *Invariants* states the standard this repository holds
itself to — *"a rule nothing checks is a rule that has already drifted somewhere"*, and *"do not
write an enforcement here that you cannot point at"*. A published contract with zero implementations
is the same class of claim: a shape nobody has been forced to fit. The seven calls are documented in
prose and in one `fn main` inside a guide, and no program in this workspace makes them in order,
persists between them, or discovers what it gets wrong by doing so.

Two smaller gaps sit under the same heading, and they are the reason Phase 1 comes first.

**`adp-domain`'s command vocabulary has zero consumers.** `crates/adp-domain/src/command.rs:47`
declares four development commands — `adp.story.start/v1`, `adp.test-plan.record/v1`,
`adp.specification.satisfy/v1`, `adp.story.complete/v1` — each revision-guarded, each naming what
makes it true. No crate in the workspace depends on `adp_domain`: `grep -rl adp_domain crates/
xtask/` returns nothing outside the crate itself. The types are correct and unspoken. They are
unspoken because the entities they act on — stories, test plans — exist nowhere a program can
address.

**Planning kinds have vocabulary and lifecycles but no store.**
`ArtifactKind::is_planning` (`crates/aep-domain/src/artifact.rs:555`) names six kinds — vision,
product-requirements, initiative, epic, story, task — with the comment *"AEP models these but does
not own them: they usually live in a planning system."* One of the six has a lifecycle document
(`artifacts/lifecycles/story.yaml`); the other five do not. An artifact manifest can *reference* a
story, and the reference resolves to a row somebody typed into `artifacts.yaml` by hand.

### 1.1 What this design moves, and what it does not

**"Not owned" was a claim about where planning data lives, and this design moves that boundary
deliberately rather than by accretion.** The repository gains a store for planning artifacts. Three
reasons, in order of weight:

1. The driver needs artifacts to walk over, and an artifact source that is a hand-maintained YAML
   manifest is not one — it has no lifecycle, no validation of its own, and no way to record that
   a story moved.
2. This repository plans its own work. Waves are planned in markdown by hand today, and nothing
   validates that a wave page's claims about status are legal moves. The protocol's own methodology
   is not applied to the protocol's own backlog, which is the least defensible gap on this list.
3. A markdown store is the cheapest thing that is honestly durable. Files in git are versioned,
   diffable, reviewable in a pull request and readable without this repository's binaries. Every
   alternative starts by asking someone to run a database.

What does **not** change: AEP still does not require anyone to use this store. An organisation whose
stories live in Jira keeps them there and references them from an artifact manifest exactly as it
does now. `is_planning` keeps its meaning — these kinds are intent decomposition, not engineering
output — and its comment gains a sentence saying that this repository now ships one place to put
them.

### 1.2 The two phases

| Phase | What it is | Status |
|---|---|---|
| **1a** | `aep-backend-markdown` + `protocol artifact …` — the planning store | accepted, harness wave 1 |
| **1b** | `integrations/claude-code/` — one skill, two agents, a marketplace entry | accepted, harness wave 1 |
| **2** | `aep-driver` + `protocol drive` — the reference driver | decided, designed, **not accepted for build** |

---

## 2. Phase 1a — the planning store

A durable store of planning artifacts as markdown files, owned by a new crate
`aep-backend-markdown` and exposed by a new CLI verb family `protocol artifact`.

### 2.1 Layout

```text
.engineering/planning/
├── epic/
│   └── passkey-login.md
├── story/
│   ├── credential-store.md
│   └── registration-ceremony.md
└── task/
    └── ceremony-fixtures.md
```

One directory per kind, one file per artifact, no nesting below the kind directory. The root
defaults to `.engineering/planning/` — beside `.engineering/project.yaml`, which the CLI already
discovers — and is overridable, because a repository may hold more than one store and a fixture
directory is one.

The directory *is* the index. There is no manifest file listing the artifacts, and no lock file.
Two reasons: a second copy of the membership list is a second thing that can disagree with the
first, and a store whose index is a file cannot be updated by two branches without a merge
conflict on every addition.

### 2.2 The frontmatter format is private to the backend

Every file is YAML frontmatter followed by markdown body. The frontmatter format is named
`aep.planning-md/1` and its fields are:

| Field | Meaning | Owner |
|---|---|---|
| `format` | `aep.planning-md/1` — the format's own name and version. **Optional, and defaulted**: a file that omits it is read as `aep.planning-md/1`, because the store has exactly one format and refusing a file for not naming it would make hand-writing an artifact harder for no gain. It is written on creation and preserved on every write, so a second version can be told apart from the first without a migration that has to guess | machine |
| `id` | `<kind>:<slug>`, agreeing with the path | machine |
| `kind` | the `ArtifactKind` | machine |
| `status` | the `ArtifactStatus`, only ever written by a lifecycle-validated move | machine |
| `title` | one line | descriptive |
| `summary` | one or two sentences, optional | descriptive |
| `owner` | who holds it, optional | descriptive |
| `tags` | a flat list of strings, optional | descriptive |
| `relations` | `{relation, target}` pairs — the artifact graph's edges | machine |
| `revision` | how many times the machine-owned half has been written | machine |

Unknown keys are **preserved**, not stripped and not refused. A store that silently deleted a field
somebody's other tool wrote would be a store nobody could adopt incrementally, and refusing the file
outright would make this format the only one allowed to touch it. Preservation is what makes
`aep.planning-md/1` a format rather than a claim of exclusivity.

**`aep-domain` gains zero types for this.** The frontmatter is a `Raw*` type inside
`aep-backend-markdown`, validated through `TryFrom` into the domain types that already exist —
`ArtifactKind`, `ArtifactStatus`, `RelationKind`, `ArtifactRelation`, `EntityRevision` — exactly as
invariant 2 requires of every document this repository reads. What the backend does not do is
publish that raw type as protocol vocabulary.

"Private" here means **owned by the backend crate**, not hidden. `aep-domain` declares nothing for
it, no other crate constructs it, and no second backend is obliged to store anything this way —
`aep-backend-memory` does not, and a future SQL backend will not. What it is *not* is undescribed:
`RawPlanningFrontmatter` derives `JsonSchema` and is published as
`schemas/generated/planning-document.schema.json` like every other document type
(**D1**, decided).

The argument for publishing it is invariant 1, read at its own scope. *Rust is the source of truth;
schemas are generated* governs the documents somebody **authors**, and these files are authored: a
person or an agent opens one in an editor and types into it. That is the property that decides the
question, not where the parser lives. A schema that is generated is a schema `schema-check` holds to
the type, so the published description of the format cannot drift from the code that refuses a bad
one; a `Raw*` type that derives `JsonSchema` and is never published is outside that check for no
gain. The memory backend's `BTreeMap`s are the honest contrast — nobody edits those in an editor,
and nothing describes them.

What publishing does **not** do is make the format an interface a second backend must match. The
seam between backends is the `aep-contract` traits (§2.8); a schema beside them describes one
backend's document format for the people and tools that write it, and obliges nobody else.

### 2.3 Identity: declared, path-checked, and never allocated

An artifact's id is `<kind>:<slug>`. It is declared in the frontmatter, and the file lives at
`<kind>/<slug>.md`. The two must agree; `validate` reports every disagreement.

**There is no counter and no allocator.** No `STORY-0018`, no `next-id` file, no sequence. The
argument is a merge, not a preference:

> Two branches each create a story. Each asks the allocator for the next number. Each gets `18`.
> Both branches merge cleanly — different files, no textual conflict — and the store now holds two
> `story:STORY-0018`, every relation pointing at one of them ambiguously. Nothing in git can catch
> this, because nothing in git was in conflict.

A slug is chosen by the person or agent creating the artifact and describes the work. Two branches
collide only when both chose the same words, which means they meant the same thing, and then git
*does* conflict — on the same path — which is the correct outcome.

This also keeps **external ticket names legal**. `story:dev-399` is a valid id, so a team whose
tickets come from elsewhere can name the artifact after the ticket without a translation table. The
store never parses an id for meaning; it splits on the first `:` to find the kind directory and
treats the rest as opaque, which is invariant 13's discipline applied one layer out.

Renaming is therefore not a `mv`. Moving a file by hand breaks every relation naming the old id and
leaves `validate` to find the wreckage. The store's answer is the one the protocol already gives:
create the new artifact, re-point the relations, archive the old one through its lifecycle. Nothing
is deleted (invariant 16).

### 2.4 No timestamps in the file

No `created`, no `updated`, no `moved_at`. Three reasons, and the third is the one that settles it:

* **Git already knows.** `git log --follow <file>` carries authorship and time for every change,
  signed where the team signs commits, and it cannot be edited by writing a different number into a
  file.
* **A hand-editable timestamp is a lie with a format.** The body of these files is edited by people
  and by agents. A `updated: 2026-08-21` line that an editor forgot to bump is worse than no line,
  because it reads as an observation.
* **Determinism.** Every committed output in this repository is byte-stable across runs, and the CLI
  already refuses to put a wall clock into anything diffable:
  `crates/protocol-cli/src/main.rs:51` fixes `SEED_AT: Timestamp = Timestamp::EPOCH` with the reason
  written beside it — *"a wall clock here would make every `--format json` diff noise"*. A planning
  store is diffed on every pull request.

The honest cost: the store cannot answer *"how long has this been in draft"* without shelling out
to git. That is a real question and it has no answer inside the store. It gets one at **P3**, where
the journal records when a write happened as a fact rather than as a field. Until then, `git log` is
the answer and the store says so.

### 2.5 The verbs

`protocol artifact <verb>`, ten of them, split into three groups by what they do.

| Verb | What it does |
|---|---|
| `new <kind> <slug>` | write a new artifact in the lifecycle's initial status, from the kind's template |
| `move <id> --to <status>` | change status, **validated against the kind's lifecycle** |
| `relate <source> <relation> <target>` | add an edge |
| `list` | what is in the store, filterable by kind and status |
| `board` | the same, grouped by status |
| `graph` | the artifact graph |
| `validate` | every problem in the store, in one run |
| `kinds` | which kinds exist, and what each is for |
| `relations` | the relation vocabulary and its pairings |
| `lifecycle <kind>` | the statuses a kind may hold, and every legal move |

The last three exist for one reason: **a consumer should ask rather than remember.** They are the
CLI half of the plugin's discover-don't-memorise rule (§3.2), and they cost almost nothing because
the answers are already validated documents in `artifacts/`.

**A status move is lifecycle-validated, and a refusal names the legal targets.**

```console
$ protocol artifact move story:credential-store --to implemented
error: `story:credential-store` is in `draft`; `implemented` is not reachable from there
       legal targets from `draft`: proposed, archived
```

The refusal is the design decision, not the validation. A validator that says *"illegal transition"*
sends the caller — human or model — to go and read a lifecycle file, and an agent that cannot find
it will guess. Naming the legal set turns the refusal into the answer to the question the caller was
actually asking, which is the same reason `CompletionExplanation` is one line per requirement rather
than the word "blocked", and the same reason `CommandKind::parse` prints the vocabulary it refused
against.

**`validate` accumulates.** A store with four broken relation targets reports four, not the first
(invariant 3). It checks: id/path agreement; the kind directory matches the declared kind; status is
legal for the kind; every relation target resolves; relation pairings against `artifacts/relations/`
where those are enforced (see **D2**); and frontmatter that does not parse. Exit 1 if anything
remains, exit 0 and a count if nothing does.

**Templates come from `artifacts/templates/`.** `new` writes a body from the kind's template where
one exists, and an empty body where none does. The templates are already in the repository and
already reviewed; a second set of body skeletons inside the backend crate would be a second thing to
keep in step with the first.

### 2.6 Three new lifecycles

`artifacts/lifecycles/` gains `epic.yaml`, `task.yaml` and `initiative.yaml`, mirroring the ladder
`story.yaml` already declares:

```text
draft → proposed → active → implemented → archived
        proposed → rejected → archived
        draft → archived,  active → archived
```

Every word is an existing `ArtifactStatus` variant (`crates/aep-domain/src/artifact.rs:637`), so
this is four documents and zero domain changes. One ladder across the four planning kinds is
deliberate: an operator and an agent learn it once, and a decomposition that produces an epic and
three stories does not need four mental models to move any of them. Where a kind eventually needs a
different ladder it gets one — the ladder is a document, which is the whole point of it being a
document.

### 2.7 Deviation register

Numbered, because these are decisions and not oversights, and because the honest list is short
enough to read. Each says what is deviated from, why the deviation is taken now, and what closes it.

**D-P1 — the CLI writes through the store, not through `CommandService`.**
Invariant 14 says every mutation is a command, and there is exactly one write path. The
`protocol artifact` verbs do not construct a `CommandEnvelope`; they call the store crate directly.
This is a real deviation and it is taken for one wave. The mitigation is structural rather than
promissory: *all* writes in the crate funnel through two functions, `create` and `update`, so there
is one place inside the store where validation, revision bumping and serialisation happen, and no
verb writes a file by any other route. **P3 reroutes those two functions through command envelopes**
and the deviation ends. Building the envelope surface first would mean building the journal first,
and the store would not exist until three milestones from now.

**D-P2 — an out-of-band file edit is not tracked.**
Somebody opens `story/credential-store.md` in an editor and changes `status: draft` to
`status: active`. The store finds out the next time something reads the file, and cannot tell that
edit from a legal move. This is a **permanent property of a file store**, not a wave-1 shortcut: any
system whose state is files a human can edit has it, and a lock file or a checksum sidecar would
convert it from an unnoticed edit into a refused file, which is worse for a format whose whole
argument is that it stays readable and editable. The compensating control is `validate`: it is the
thing that catches an illegal status, a broken edge or a mismatched id whenever it runs, and the
plugin's guardrails and the CI step are what make it run.

**D-P3 — there is no history and no audit; `revision` counts writes.**
`revision` is incremented when the machine-owned half of a file changes. It is not a version
history: the store cannot say what the artifact looked like at revision 2, who moved it, or why.
Git is the history — `git log -p` on the file answers all three, better than a sidecar would. What
git does not give is a *queryable* trail joined with the protocol's own audit records, and that is
what the journal at **P3** is for. Recording this here so the `revision` field is not mistaken for
an audit trail by anything downstream: it is a stale-write guard and nothing else.

**D-P4 — `rm` deletes an artifact, and nothing prevents it.**
Invariant 16 says nothing is physically deleted, and the CLI keeps that line: there is no
`protocol artifact delete`, `archived` is a status, and archiving is how an artifact leaves. But the
artifacts are files, and `rm` is a program. This is inherent to a file store and is not closed by
anything at this layer; it is closed, to the extent it can be, by the files being in git.

**D-P5 — `describe_type` still reports no lifecycle.**
`QueryService::describe_type` (`crates/aep-contract/src/query.rs:345`) exists so a harness can ask
what a design *is* rather than hard-coding it, and `TypeDescriptor` has a `lifecycle` field. The
in-memory backend never populates it (`crates/aep-backend-memory/src/query.rs:189`), so a harness
asking the contract which statuses a story may hold gets `None` while
`artifacts/lifecycles/story.yaml` sits in the tree with the answer. This is **pre-existing and not
owed by this wave** — the new store answers the question through `protocol artifact lifecycle`, and
the contract-level gap is noted here so that P3, which is where the store meets the contract, finds
it already written down rather than discovering it.

### 2.8 Backend roadmap

The store is the first of several, and the sequence is decided now so that each milestone is a
narrowing rather than a redesign.

| | Milestone | What it delivers |
|---|---|---|
| **P1** | `aep-backend-markdown` (this wave) | a plain durable store, CLI-owned, no contract implementation |
| **P3** | journal-backed `CommandService`/`QueryService` for the markdown store | writes become commands; the journal is the history D-P3 does not have; the store runs the sixteen conformance suites |
| **P4** | `aep-backend-sqlite` | the first database backend; one file, no server, the obvious next durability step |
| **P5** | `aep-backend-postgres` | the backend an organisation actually runs |
| **P6** | `aep-backend-hybrid` | a composite: write to the primary database first, project to markdown second, compensate on partial failure |

P2 is deliberately absent from this table: it is the plugin (§3), which ships in the same wave as P1
and is numbered in the plugin's own sequence rather than the backend's.

**P6 is the interesting one and the least settled.** The shape is a composite backend that holds a
primary (SQL) and a projection (markdown): the write goes to the primary first, because that is the
one with transactions, and the markdown projection follows. A projection that fails after a
committed primary write leaves the two disagreeing, so the composite runs a **compensating
rollback** — the primary write is reversed by its own inverse command, which the protocol's
vocabulary already supplies, rather than by a `DELETE`. What *exact* atomicity that buys is an open
P6 design question and is not answered here: the honest options run from "eventually consistent with
a repair verb" to "two-phase with a durable intent log", and choosing between them without the
conformance suites of P3 to test against would be guessing.

**The multi-backend seam is the existing `aep-contract` traits. No new store trait is added.**
This is the roadmap's load-bearing decision, so the argument goes here rather than in a commit
message:

* The contract already *is* the seam. `CommandService`, `QueryService` and the audit surface are
  storage-independent by construction, and `aep-conformance`'s sixteen suites are a black-box
  definition of what implementing them means — checked against a `FaultyBackend` whose injected
  defects the suites are proven to catch. Anything a new `PlanningStore` trait could say about a
  store, these already say, and these come with an executable acceptance test.
* A second trait would create a second write path, which invariant 14 exists to forbid, and
  `crates/aep-contract/tests/write_surface.rs` would fail the moment it was declared — the test
  enumerates every method of every public trait in the contract and pins the list. That test failing
  is not an obstacle to route around; it is the invariant saying no.
* The cost of the decision is visible and accepted: it is what makes P1 a **deviation** (D-P1) rather
  than a design, and it is why P3 exists as its own milestone instead of being folded into P1.

---

## 3. Phase 1b — the Claude Code plugin

`integrations/claude-code/` — a plugin that teaches Claude Code to plan work in the store, plus
`.claude-plugin/marketplace.json` at the repository root so it can be installed by name.

### 3.1 Where it lives, and why there

`integrations/claude-code/` is a **consumer-named deliverable**, the same shape as `website/`: a
thing built beside the specification, consuming only its public surface, named after who it is for.
It is not a crate, it is not in the Cargo workspace, and nothing in `crates/` knows it exists. The
dependency runs one way — the plugin calls `protocol`, and `protocol` has never heard of Claude
Code — which is what keeps a second harness from being a second-class citizen.

The directory is `integrations/` plural because there will be others, and the first one should not
be at a path that has to move when the second arrives.

### 3.2 One skill, and the rule that it discovers rather than memorises

The plugin ships exactly one skill, `planning`. Not three, and specifically not one per verb family.
A skill is a decision about *when a body of instructions loads*, and there is one moment here:
the operator is planning work. Splitting that into `planning-create`, `planning-move` and
`planning-review` would triple the trigger surface, produce three files that each need the same four
guardrails, and let a session load the one that does not carry the guardrail it was about to break.

**The skill inlines rules and no vocabulary.** It contains no list of kinds, no list of statuses, no
transition table and no relation names. Each of those has a command:

| Question | Command |
|---|---|
| What kinds can I create? | `protocol artifact kinds` |
| What edges exist? | `protocol artifact relations` |
| What statuses, and what moves where? | `protocol artifact lifecycle <kind>` |
| What is in the store? | `protocol artifact list` |

The reason is this repository's own thesis turned on its own documentation. Lifecycles and relations
are **validated, versioned documents**; a prose copy of one inside a skill file is neither. It goes
stale the first time a kind gains a status, and the failure is silent and confident — the agent
recites `draft → proposed → active` from a file written in August and proposes an illegal move in a
store that renamed one of them in September. Reading `protocol artifact lifecycle story` costs one
command and cannot be wrong. A prose copy of a validated document is drift with a nice font.

### 3.3 Four guardrails

Inlined in the skill, because unlike the vocabulary these hold whatever the store contains.

1. **A status changes only through `protocol artifact move`.** Never edit `status:` in frontmatter.
   A hand-edited status is an unvalidated one and is indistinguishable in the file from a legal one,
   which is what makes it expensive rather than merely wrong. This is D-P2's compensating control at
   the point where the breach would actually happen.
2. **The body is free.** The CLI owns frontmatter; it does not own prose. Context, acceptance and
   notes are written with the ordinary editing tools. There is no CLI verb for a paragraph and there
   should not be one — a store that made a human ask a program for permission to write a sentence
   would be a worse place to think than a text file.
3. **Run `validate` after a batch of edits, and relay its output verbatim.** It accumulates every
   problem and names each artifact and each defect. Summarising that into "validation failed" throws
   away the only part the operator can act on — the same rule `docs/guide/harness.md` states for
   `CompletionExplanation`, for the same reason.
4. **A refusal is the answer, not an obstacle.** When a move is refused, the refusal names every
   status legal from where the artifact stands. Relay that list. Do not retry with a different
   spelling, do not route around it by editing the file, and do not walk the artifact through three
   intermediate statuses nobody sanctioned to reach the one that was refused.

Guardrail 4 is the one that matters most, and it is aimed at a specific and well-documented failure
mode: a model that treats a tool error as an obstacle to be defeated. The whole value of a
lifecycle-validated move is destroyed by an agent that responds to a refusal by opening the file.

### 3.4 The LLM proposes; the operator decides

Creating a draft and writing a body need no confirmation beyond the request that prompted them — a
draft is cheap and reversible. **A status move is a claim about the state of the world**, and that
is the operator's to make. So: new artifacts land in the lifecycle's initial status and are not
immediately moved; status moves and decompositions are proposals until confirmed; a move the
operator named explicitly is already confirmed and asking again is noise, not caution; and no bulk
move is ever performed autonomously, because "archive everything still in draft" is an instruction
and inferring it from a tidy-up request is not.

This is the same division the protocol makes everywhere else — the model reasons, the protocol
decides what the facts permit — applied at the one boundary where the plugin can write.

### 3.5 Two agents, with charters

| Agent | Charter | Bound |
|---|---|---|
| `decomposer` | break an epic into stories | **draft-only output.** It creates artifacts in the initial status, linked to the epic it decomposed, and moves nothing. Its work is judged by `validate` passing and by the operator reading the drafts |
| `plan-reviewer` | read the store and report on it | **read-only.** It changes zero files. It reports missing acceptance statements, orphaned artifacts, edges that point at nothing, and artifacts stuck in a status |

The bounds are the charters. A decomposer that could move statuses would be an agent deciding what
work is agreed; a reviewer that could fix what it found would be an agent reviewing its own changes,
which is the thing invariant 7 exists to prevent one layer down.

### 3.6 Deliberately no hooks, and deliberately no commands

**No hooks.** A Claude Code hook is deterministic interception — a program that runs before or after
a tool call and can refuse it. That is exactly the driver's job (§4), and building it in the plugin
would produce a *second, weaker driver*: one that sees tool calls rather than workflow states, that
cannot ask the engine anything because it has no execution to ask about, and that would have to be
deleted or reconciled when the real one arrives. Two mechanisms that both claim to enforce the same
thing is the failure mode this repository writes registers to avoid.

**No `commands/`.** The CLI is the command surface. A slash command that wraps
`protocol artifact new` adds a second spelling of one verb, and the two spellings drift — the CLI
grows a flag and the slash command does not, or the slash command grows a convenience the CLI cannot
honour. The skill teaches the model to use the CLI; the operator has a terminal.

---

## 4. Phase 2 — the reference driver

**Decided by the operator on 2026-08-21, designed here, and explicitly not accepted for build.**
This section is architecture. Nothing in it is a work order, and the wave that builds it opens only
after a feasibility review of this section against the code — the same gate ESS waves 4 and 6 went
through.

### 4.1 What it is

`aep-driver`, a new crate, plus `protocol drive` in the CLI: **the first in-repository
implementation of the harness contract.** It walks a workflow by making the seven engine calls in
order, doing outside the engine only what the answers permit, and recording what it did.

The split is deliberate and it is the crate boundary:

* **`aep-driver` is a pure, deterministic routing core.** Clock-free and randomness-free, the same
  discipline `aep-domain` holds under invariant 8. It consumes `Evaluation` and `TransitionResult`
  *verbatim* — it never re-derives a verdict, never re-evaluates a gate, never decides that a
  requirement is "basically satisfied". Given the same evaluation and the same step map it returns
  the same next step, which is what makes a run replayable at all.
* **The step executors live in `protocol-cli`, behind `protocol drive`.** Running a program, calling
  a model, and pausing for a person are the three things that touch the world, and they are outside
  the crate that has to stay pure.

**Gates are evaluated only by the engine.** The driver never reads a predicate, never compares a
fact path, never decides a transition is legal. It asks, and it does what it is told. A driver that
could evaluate a gate would be a second protocol implementation with none of the conformance suites,
and the first time the two disagreed the one nobody tested would win.

**A claim of replayability is narrow, and stating it narrowly is part of the design.** What replays
is the sequence of *decisions*: the same snapshot and the same evidence yield the same routing. The
*work* does not replay — a test run, a model call and a human's answer are not reproducible, and the
driver never pretends they are. What it stores is what was decided and on what evidence, not a
recording of the world.

### 4.2 Step maps — a new document type

A workflow says what states exist and what evidence each transition needs. It deliberately does not
say *how* to obtain that evidence — that is a harness's business, and keeping it out of the workflow
is what lets one workflow govern a Rust repository and a Terraform one. The driver needs the missing
half, and it is a document rather than code:

```yaml
# drivers/development/default.yaml
format: aep.driver-steps/1
id: development/default
workflow: adp/default@1
states:
  implement:
    steps:
      - kind: llm
        skill: planning
        prompt: implement the story's acceptance statement
      - kind: command
        run: [cargo, test, --workspace]
        evidence:
          kind: test_result
          verifier: test-runner
```

Properties, each of which is a rule the type enforces:

* **`drivers/<family>/<name>.yaml`**, beside `workflows/`, `principles/` and `profiles/` — the
  document tree gains a fifth kind of document, loaded the same way.
* **It references a workflow by versioned id** (`adp/default@1`). A step map is written against a
  specific workflow revision, and saying so is what makes the cross-validation below possible.
* **Per state, an ordered list of steps.** Order inside a state is the map author's; order *between*
  states is the workflow's and the driver never overrides it.
* **Raw → validated** (invariant 2), **schema-generated** (invariant 1): `RawStepMap` deserialises,
  `StepMap` is obtained only by validating, and `cargo xtask schema` writes the schema.
* **Cross-validated against the workflow it names**, at load time, accumulating (invariant 3):
  * every state the map mentions exists in the workflow — a step map for a state that was renamed is
    refused, not silently skipped;
  * every evidence kind a step declares is declared by the protocol in force — so an unrunnable map
    fails at load rather than at the transition that needed the evidence;
  * every verifier a step names can actually produce the kind it claims, via
    `aep_engine::engine::kinds_for_verifier` — the same call `docs/guide/harness.md` § 2 tells a
    harness author to make at plan time, made mechanically.

### 4.3 Three step kinds

**`command` — runs a program, and maps its output to evidence.**
A command step names an executable and how to read its result, and produces an `EvidenceSubmission`
carrying `Producer::Verifier { verifier }`. **This is how `independent: true` is honestly
satisfied.** The producer is a verifier because a verifier produced it: the driver ran `cargo test`
and read its exit status and counts. Nothing about the model's opinion of the test run enters the
record. `Provenance` — command, tool, revision, workspace, digest — is filled from what the driver
actually invoked, so the record can be re-derived by someone who does not trust the driver.

**`llm` — a headless Claude run, with a derived allowlist.**
An LLM step runs headless Claude with the plugin's skills available and a tool allowlist **derived
from `capabilities()`**, not configured. The protocol already answers "what may be done here", every
`Action` maps to exactly one `Capability`, and the guide already instructs a harness to do the
mapping once at tool-registration time. The driver does it per state, which is stricter: the tools
that exist in the `implement` state are not the tools that exist in `review`.

**An `llm` step cannot carry an evidence block, and the type makes that unrepresentable.** Not a
validation rule that could be relaxed — the `Llm` variant has no `evidence` field. An agent's own
statement never satisfies an independence requirement (invariant 7, and the vision's one named
trust), so a step kind that could mint evidence from a model's output would be the single change
that unpicks the whole loop. Anything an LLM step is supposed to have achieved that is *checkable*
is observed by a subsequent `command` step: the model writes the code, and `cargo test` says whether
it works.

**`operator` — a pause.**
The run stops, shows `CompletionExplanation` verbatim (the guide's rule, and the reason the
explanation is one line per requirement rather than a summary), and waits. What comes back is
recorded with `Producer::Human`. This is what `require_approval` becomes at the harness layer, and
it is the honest answer to the states where a person is the verifier.

### 4.4 The loop

```text
restore-or-init  →  evaluate  →  select step  →  execute step
                                                      │
                        ┌─────────────────────────────┘
                        ▼
                submit evidence  →  transition  →  persist  →  (repeat)
```

* **restore-or-init** — `engine.restore(task, artifacts, snapshot)` if a run exists at this
  execution id, `initialize` if not. The plan is re-resolved either way; a snapshot never carries
  one, so a run resumed after the documents changed is governed by the documents as they are now.
* **evaluate → route** — the routing branches on `Truth`, which is the third of the guide's three
  rules made structural: `False` selects a *fix* step, `Unknown` selects an *observe* step, `True`
  selects nothing. Collapsing the two would produce a driver that tries to fix code nobody has
  tested.
* **persist** — after every step, a snapshot **and** a driver cursor go to
  `.engineering/runs/<execution-id>/`. The snapshot is the engine's (`Execution::snapshot()`); the
  cursor is the driver's own — which step of which state it is on, and its visit budgets. Two
  documents because they have two owners, and a driver that stored its cursor inside the engine's
  snapshot would be a driver that had quietly forked the snapshot format.
* **back-edges are legal, with a budget.** `adp/default` has a deliberate `verify → implement`
  back-edge, and the workflow's own comment explains why: *"a workflow that can only go forwards is
  a lie about how engineering works"*. A driver must therefore be able to go round again — and must
  not go round forever. Each state carries a **visit budget**; exceeding it stops the run and
  reports which state it was cycling in, rather than burning a token budget in silence.

### 4.5 Hard problems, unsolved

These are the wave-2 decision list. They are named here so that the feasibility review has an
agenda, and so that nobody reads §4.1–§4.4 as a complete design.

1. **Step-map versioning against workflow versions.** A step map names `adp/default@1`. What happens
   when the workflow reaches `@2` — is the map refused, migrated, or is a map without a version
   pinned to whatever loads? And what happens to a run in flight whose map's workflow moved?
2. **Store → facts.** The engine evaluates predicates over facts. The planning store holds
   artifacts. Something has to rebuild an `ArtifactGraph` between steps so the current state of the
   store is what the next evaluation sees, and doing that on every step is either the correct
   semantics or an unacceptable cost — probably both.
3. **`require_approval` under a headless run.** The capability policy's middle value exists so a
   human can say yes. A headless run has no human at the keyboard. Does the driver stop, queue, or
   refuse to start a run whose path is known to cross an approval? None of the three is obviously
   right and the choice changes what the driver is for.
4. **Session granularity.** One model session per state, or per step? Per state is cheaper and
   carries context; per step is isolable and replayable. Resume semantics fall out of the answer,
   and so does what a resumed run knows about what the previous session was thinking.
5. **Failure taxonomy.** A crashed run is `Unknown` — nothing was observed. A failing suite is
   `False` — something was observed and it is wrong. That distinction is already the protocol's, and
   the driver has to map every way a step can fail onto it correctly, including the ambiguous ones
   (a timeout, an OOM, a network error mid-suite). Retry budgets belong to the same question: a
   retried step that succeeds must not erase the evidence that the first attempt failed.
6. **Concurrency and locking.** One execution per store, enforced by a lockfile, is the obvious
   first answer. Whether it is the right one — and what a stale lock from a crashed run does to the
   next operator — is not decided.

### 4.6 What Phase 1 leaves in place for it

Phase 1 is not a down payment on Phase 2, and it is worth being precise about what it nevertheless
leaves standing:

* **Frontmatter sufficient to build an `ArtifactGraph`.** Kind, status and typed relations are
  exactly the fields the graph needs; nothing further is added *for* the driver, and nothing the
  driver would need is left out.
* **`.engineering/runs/` is reserved** — the name is taken and documented, so the store and the
  driver do not later argue about a directory.
* **Skills are addressable by name.** `planning` is a name an `llm` step can reference, which makes
  the plugin's skills the driver's prompt library rather than a second one.
* **The CLI verbs are stable.** A driver that shells out to `protocol artifact` is not the intended
  design — it will use the crate — but the verbs are the surface an operator learns, and they do not
  move because the driver arrived.

---

## 5. Vision impact

`docs/VISION.md` § *What this is deliberately not* refuses, among other things, *a workflow engine*.
**That refusal is narrowed by this design, and the narrowing is recorded as V-5 in
[`control-document-updates.md`](../plan/control-document-updates.md).**

What the narrowing says: this is still not a workflow engine in the sense refused before — a
general-purpose orchestrator that other systems are built on top of. What the repository now ships
is **one reference driver**: a default harness that walks *its own* workflows by asking the engine
the seven questions and doing, outside the engine, only what the answers permit.

The line that does not move is the one that mattered:

* the engine still evaluates and never acts;
* invariant 7 is untouched — an agent's own statement never satisfies an independence requirement,
  and the `llm` step kind's inability to carry evidence is that invariant expressed in a type;
* gates are evaluated by the engine and never by the driver;
* *"external systems do the work; this project decides what the results permit"* stays true. The
  driver is the first of those external systems, kept in-tree the way `website/` is — a deliverable
  beside the specification, consuming only its public surface.

**What stays refused, and is not narrowed by anything here:**

* **General-purpose orchestration for other systems.** The driver walks AEP workflows. It is not a
  runtime somebody schedules unrelated jobs on, it has no plugin surface for foreign step kinds, and
  a request to make it one is a request for a different product.
* **LLM calls inside authoritative report production.** A model may write code, propose a
  decomposition and draft prose. Nothing a model says becomes evidence, becomes a verdict, or enters
  a report the protocol treats as authoritative. The `llm` step's missing evidence field is where
  that refusal is mechanical rather than stated.

---

## 6. Open decisions

Each with the default that is taken if nobody decides otherwise.

**D1 — schema wiring for the backend-owned frontmatter format. Decided: yes, publish it.**
Should `aep.planning-md/1` be generated into `schemas/generated/` by `cargo xtask schema`?
*Taken as yes*, against this document's first draft, which defaulted to no on the grounds that a
backend's on-disk representation is not protocol vocabulary. The counter-argument won and is worth
keeping: these files are **authored** — somebody types into them — and every authored document in
this repository has a generated schema held to its type by `schema-check`. Leaving
`RawPlanningFrontmatter` unpublished would have put the one description of the format outside the
drift check for no benefit. `aep-schema` therefore depends on `aep-backend-markdown` for this one
type, and `schemas/generated/planning-document.schema.json` is committed. It obliges no other
backend: the seam between backends is the contract traits, not this file.

**D2 — enforcing `relations.yaml` source/target pairings.**
`artifacts/relations/relations.yaml` says its `source`/`target` lists are *"advisory guidance for
humans until the artifact validator reads them"*. `protocol artifact validate` is a validator that
could read them.
*Default: deferred.* Wave 1's `validate` checks that a relation's target *resolves* and that the
relation kind exists; it does not refuse an unusual pairing. Turning advisory guidance into a
refusal is a change to the shared document's meaning and affects every consumer of the artifact
graph, not just this store — it deserves its own decision rather than arriving as a side effect of a
new backend.

**D3 — the `protocol entity --planning` bridge.**
`protocol entity list|show` seeds the in-memory backend from an artifact manifest and answers about
entities. The planning store holds artifacts that are entities. Should `entity` learn to read the
store?
*Default: not in Phase 1.* The two surfaces answer different questions today —
`protocol artifact` is about a store, `protocol entity` is about the contract — and joining them
before the store implements the contract would produce a bridge whose semantics change at P3. The
bridge is P3's, and at P3 it is not a bridge: it is the store answering as a backend.

**D4 — where the driver's step maps live in the document tree.**
`drivers/` at the repository root, beside `workflows/`, is what §4.2 assumes.
*Default: `drivers/`, decided when the driver's wave opens, not now.* Nothing in Phase 1 depends on
it, and the feasibility review may have a better answer.
