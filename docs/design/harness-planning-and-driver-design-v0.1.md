# Harness — a planning store, and a reference driver — Design v0.1

> **Repository:** `codewandler/engineering-protocols`
> **Status:** **Phase 1 accepted for implementation** by
> [`docs/plan/harness-wave-1-planning-plugin.md`](../plan/harness-wave-1-planning-plugin.md), 2026-08-21.
> **Phase 2 is decided and designed, and is not accepted for build** by that page or by any other:
> the vision narrowing it depends on is recorded (V-5 in
> [`control-document-updates.md`](../plan/control-document-updates.md)), and the build waits behind
> its own feasibility review.
> **Wave-2 update, 2026-08-21:** § 4.7 takes the six decisions § 4.5 named, § 4.8 adds the
> enforcement mapping and § 4.9 the adapter surface, driven by
> [`harness-wave-2-driver-decision.md`](../plan/harness-wave-2-driver-decision.md). §§ 4.1–4.6 are
> unchanged, deliberately: the corrections the update makes to them are recorded *in* the update, so
> that a reader sees what was found rather than a document that was always right. **Still not
> accepted for build.**
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
  **The `@1` spelling in this line is wrong and is corrected in § 4.7 D1** — this repository's
  versioned reference is `adp/default/1` — and the correction is recorded there rather than applied
  here, so that a reader can see a mistake was found rather than find a document that was always
  right.
* **Per state, an ordered list of steps.** Order inside a state is the map author's; order *between*
  states is the workflow's and the driver never overrides it.
  **The list is what the transition waits for.** A state's steps run to exhaustion and *then* the
  engine is asked to move — that is what makes an ordered list mean anything, and it is what
  `establish_verifiers`'s two steps in the shipped map rely on: the model writes the failing tests
  and the driver runs the suite, and only the pair of them buys the transition. § 4.4's loop diagram
  reads the other way and **is corrected there** (built 2026-08-21;
  `crates/aep-driver/src/route.rs:30-35`).
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

> **The diagram's `transition` is in the wrong place, and the correction is written beside it rather
> than drawn over it** (built 2026-08-21, and the third of the three findings the driver build
> returned). As drawn, a transition is attempted after **every** step. That is not what a step map
> means: **a state's steps are an ordered list, and the engine is asked to move when the list is
> exhausted.**
>
> The router says so and is the only thing that decides it —
> `NextStep::Run { index }` while `cursor.step < map.steps_for(state).len()`, and `NextStep::Transition`
> otherwise (`crates/aep-driver/src/route.rs:50-63`). `Transition` is also the answer for a state the
> map says nothing about, because a state whose transition is unguarded needs no work done in it and
> a map that had to say so for every such state would be noise.
>
> Why the difference is load-bearing rather than cosmetic. `establish_verifiers` in the shipped map
> has two steps — the model writes the failing tests, then the driver runs the suite — and the guard
> out of that state reads `test.exists`. Transitioning after the first of them would ask the engine
> to move on evidence the second step had not produced yet, get `Blocked`, and route the run back
> into a state it had never finished. The evaluate-and-route arrow is unaffected: it is consulted
> once per iteration either way, and what it selects is a *step*, never a transition.
>
> Everything else in the diagram stands, including the ordering of `submit evidence` before
> `transition`: a `command` step's evidence is submitted the moment it is produced, so a state's last
> step is the one whose evidence the transition is evaluated against.

* **restore-or-init** — `engine.restore(task, artifacts, snapshot)` if a run exists at this
  execution id, `initialize` if not. The plan is re-resolved either way; a snapshot never carries
  one, so a run resumed after the documents changed is governed by the documents as they are now.
* **evaluate → route** — the routing branches on `Truth`, which is the third of the guide's three
  rules made structural: `False` selects a *fix* step, `Unknown` selects an *observe* step, `True`
  selects nothing. Collapsing the two would produce a driver that tries to fix code nobody has
  tested.
* **persist** — after every step, a snapshot **and** a driver cursor go to
  `.engineering/runs/<execution-id>/`. **Corrected in § 4.7 D6 (review F20): the path is the
  driver's own run id, not the execution id**, because the execution id is not unique across
  invocations. The snapshot is the engine's (`Execution::snapshot()`); the
  cursor is the driver's own — which step of which state it is on, and its visit budgets. Two
  documents because they have two owners, and a driver that stored its cursor inside the engine's
  snapshot would be a driver that had quietly forked the snapshot format.
* **back-edges are legal, with a budget.** `adp/default` has a deliberate `verify → implement`
  back-edge, and the workflow's own comment explains why: *"a workflow that can only go forwards is
  a lie about how engineering works"*. A driver must therefore be able to go round again — and must
  not go round forever. Each state carries a **visit budget**; exceeding it stops the run and
  reports which state it was cycling in, rather than burning a token budget in silence.

### 4.5 Hard problems, unsolved

**All six are taken in § 4.7 (wave-2 update, 2026-08-21). This section is left as written**, because
the agenda a review was given is part of the record: a decision list that quietly becomes a decision
table is one nobody can check the decisions against.

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

### 4.7 Wave-2 update — the six decisions, taken

> **Added 2026-08-21 by [`harness-wave-2-driver-decision.md`](../plan/harness-wave-2-driver-decision.md).**
> Sections 4.1–4.6 are unchanged; this section, § 4.8 and § 4.9 are the wave-2 addition. Nothing
> here is accepted for build either: it is what the feasibility review has to attack, and its job is
> to leave that review with claims specific enough to be wrong.
>
> **W2.3 applied, 2026-08-21.** The review is
> [`2026-08-21-driver-feasibility-review.md`](../reviews/2026-08-21-driver-feasibility-review.md) —
> **23 CONFIRMED · 14 NEEDS-CHANGE · 3 INFEASIBLE · 0 UNRESOLVED**, three open cells resolved. Every
> NEEDS-CHANGE and INFEASIBLE item is applied below and **cites the finding that forced it** (F1,
> F2, …). Nothing is corrected silently, and no reviewed sentence is deleted where a correction can
> stand beside it — the wave-4/5 precedent, and the same rule this section already applies to § 4.2.

Each decision states what was taken, the mechanism in the types and functions that already exist,
what was rejected, and what wave 3 has to test. The mechanism column is the point: a decision whose
mechanism is a sentence is a decision nobody can implement or refute.

**Where the review changed a decision, and where it did not.**

| finding | verdict | what changed here |
|---|---|---|
| **F1** | INFEASIBLE | D1's `drivers/` loading and W3.1's single crate are a dependency cycle. `aep-driver` splits into a leaf **`aep-driver-spec`** and the router **`aep-driver`** |
| **F2** | INFEASIBLE | D6's lock lived inside the directory it was allocating. One fixed path, `.engineering/runs/lock.json`, taken **before** any run-id allocation |
| **F3** | NEEDS-CHANGE | the tool set derives from `CapabilityPolicy::decide`, never from `.allow` membership |
| **F5** | NEEDS-CHANGE | D1's verifier check exempts `Verifier::ExternalTool` instead of refusing every external tool |
| **F6** | NEEDS-CHANGE | the mandatory pin becomes a type, `PinnedWorkflowRef`, so the published schema is not looser than the validator |
| **F7** | NEEDS-CHANGE | D2 stops on `StoreReport::is_clean()` too — `graph()` returns `Ok` for a file that did not parse |
| **F8** | NEEDS-CHANGE | D2's per-iteration cost restated honestly, and the registry/store asymmetry decided rather than left accidental |
| **F9** | NEEDS-CHANGE | D3's reachability walk gains `Transition::requires` and recurses through nested conditionals |
| **F10** | correction | the `ExecutionId` counter is per **`Engine` value**, not per process — a stronger form of the same hazard |
| **F12** | INFEASIBLE today | § 4.8 row 3's audit needs a 50th expectation kind, `env.tool_available`; it becomes a named wave-3 build item |
| **F13** | NEEDS-CHANGE | `permission.denied` is a whole-run count, and `0` is ambiguous; the audit cell says so |
| **F14** | NEEDS-CHANGE | a hook cannot call `Engine::authorize`; the channel is decided — an append-only decision log the driver folds in |
| **F15** | NEEDS-CHANGE | `--bare` is forbidden by name, and the hook trust model is named as an unverified assumption |
| **F16** | NEEDS-CHANGE | the write-guard matcher gains `NotebookEdit`, and the `Bash` hole is closed by a stated property rather than by the matcher |
| **F17** | overturned in our favour | a removable lockfile is **not** an invariant-16 breach; two adjacent rules adopted anyway |
| **F19** | NEEDS-CHANGE | `aep-driver`'s determinism claim gains a scan, and the pid-liveness probe is placed |
| **F20** | three small | § 4.4's path pointer, the cursor's engine-version field, `ProtocolRef` as a type-level precedent |
| **F4** | CONFIRMED | all three self-declared corrections stand; two citation errors fixed in place |
| **F11, F18** | CONFIRMED | the boundary claim holds in both halves; the wave-3 sequence is buildable after F1 and F12 |

**Nothing was re-argued.** Every NEEDS-CHANGE and INFEASIBLE item is applied as the review asked, or
— where the review offered options (F6, F14) — at its stated preference, with the option taken named
so the choice is auditable.

#### D1 — step-map versioning: the pin is mandatory, and a major bump orphans the map at load

**Taken.** A step map names its workflow with the repository's existing versioned-reference
spelling, and the pin is **required** rather than optional:

```yaml
format: aep.driver-steps/1
id: development/default
workflow: adp/default/1     # WorkflowRef — id `adp/default`, major 1
```

**The spelling is `adp/default/1`, not `adp/default@1`.** `WorkflowRef` is declared by the
`versioned_ref!` macro (`crates/aep-domain/src/version.rs:174`) and declared at
`crates/aep-domain/src/version.rs:290-297` — **corrected per review F4**, which found `:131-138` is
`ProtocolRef`, not `WorkflowRef` — its published pattern is
`^(workflow:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*(/[1-9][0-9]*)?$`, its `Display` writes `{id}/{major}`
(`version.rs:214-221`), and `split_version` (`version.rs:93-102`) takes the trailing all-digit
segment as the major, so `adp/default/1` is id `adp/default` at major 1. § 4.2's `@` is a second
spelling of a version pin, and a second spelling is a second parser.

**The pin is mandatory where a profile's is optional**, and the asymmetry is deliberate. A profile
that does not pin is saying *"whatever this workflow becomes, I still mean it"*, which is a
reasonable thing for a policy document to say. A step map is an instruction sheet for a specific
state graph — it names states and orders steps inside them — so an unpinned one is an instruction
sheet for whatever happens to be in the tree. `ProtocolRef` already refuses the same shape for a
task, with the version non-optional in the type (`version.rs:104-113`).

**Corrected per review F6: the mandatory pin is a *type*, not a validator rule.** As written, the
decision was stricter than the schema it would publish. `WorkflowRef::major` is
`Option<MajorVersion>` (`version.rs:200-202`), `accepts` returns `true` for an unpinned reference
(`:206-208`), and the published pattern makes the version group optional (`version.rs:296`) — which
the `JsonSchema` impl writes verbatim into the generated document (`version.rs:325`). So
`schemas/generated/driver-steps.schema.json` would have accepted `workflow: adp/default` while the
loader refused it: an editor telling an author their map is fine, and a loader disagreeing. That is
invariant 1 inverted.

So the step-map crate declares **`PinnedWorkflowRef`**: a newtype over `WorkflowRef` whose
`TryFrom` refuses `major() == None` with its own accumulating `ValidationCode`, and whose
`JsonSchema` publishes the same pattern with the group made **required** —
`^(workflow:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*/[1-9][0-9]*$`. `ProtocolRef` is the model, and the
review is right that D1 was citing it rhetorically when it is a type-level precedent: it holds a
non-optional `MajorVersion` (`version.rs:108-112`) and publishes a pattern with no optional group
(`version.rs:132`). Copy the type, not just the argument. The alternative — keep `WorkflowRef`,
refuse an unpinned one in `TryFrom`, and write one line saying the schema is deliberately looser —
is acceptable and is **not** taken: a published schema that is looser than its validator is a
drift check that has been told to look the other way.

**Mechanism — a major bump orphans the map with no new code.** `Registry::workflow`
(`crates/aep-engine/src/registry.rs:118-123`) looks the workflow up by id and then filters on
`WorkflowRef::accepts`, which is equality against the pin (`version.rs:206-208`). A map pinned to
`/1` against a registry holding `version: 2` therefore resolves to `None`, and the step-map loader
turns that `None` into an accumulating validation error naming the map, the pin and the version
present. Not a warning, not a fallback: the map is refused.

**Cross-validation runs in two phases, because the two halves are knowable at different times.**

| phase | what it checks | against |
|---|---|---|
| **at load** — registry only | every state the map names is a key of `workflow.states`; every **named** verifier a step names can produce the kind it claims | `Workflow` (`crates/aep-domain/src/workflow.rs:153`, `states` at `:166` — citation corrected per **F4**), and `aep_engine::engine::kinds_for_verifier` (`crates/aep-engine/src/engine.rs:499-505`) |
| **at run start** — plan resolved | every evidence kind a step declares is declared by the protocol in force; the resolved workflow's id and version equal the map's pin | `Protocol::declares_evidence` (`crates/aep-domain/src/protocol.rs:103-105`) |

**Corrected per review F5: phase 1 checks only *named* verifiers.** As written, the check refused
every external tool. `kinds_for_verifier` filters `EvidenceKind::ALL` on `default_verifiers()`
(`engine.rs:499-505`), and `default_verifiers` enumerates only the thirteen named classes
(`crates/aep-domain/src/evidence.rs:1317-1334`, the list being `Verifier::NAMED`,
`crates/aep-domain/src/verification.rs:72-86`). `Verifier::ExternalTool(ToolRef)` — what
`Verifier::parse` falls through to for anything unrecognised (`verification.rs:110-117`) — is in no
row, so a step reading `evidence: { kind: static_analysis, verifier: ruff }` would have been refused
at load for a defect that is not one. The rule is therefore:

> A step naming a verifier in `Verifier::NAMED` is refused when `kinds_for_verifier` does not
> contain the kind it declares. A step naming a `Verifier::ExternalTool` is **not checked here** —
> the protocol has nothing to check it against — and its kind is still checked at run start against
> `Protocol::declares_evidence` (`crates/aep-domain/src/protocol.rs:103-105`).

The review's second point is the one worth keeping visible: `default_verifiers` is a table of
**defaults**, not of constraints — `Diff` defaults to `[Compiler, StaticAnalyzer]`
(`evidence.rs:1326`), so a `diff` produced by `git` would also have been refused. Using a default
table as a hard gate promotes a default into a rule, in a repository whose register asks for the
opposite.

The second phase is not a duplicate of the first and cannot be folded into it. The protocol in force
comes from the **task** (`Task::protocol`, `crates/aep-domain/src/task.rs:338`), which no document
loader has seen; a loader that guessed would let a map validate and then fail at
`ProtocolError::EvidenceRejected` (`engine.rs:321-332`) at the transition that needed the evidence —
the exact failure § 4.2 says the check exists to prevent. Both phases accumulate (invariant 3).

**A run in flight is the driver's problem, because the engine cannot see it.** `Snapshot`
(`crates/aep-engine/src/execution.rs:56-74`) carries the execution id, the task id, the state, the
states entered, the evidence, the events and the actor — and **no workflow id and no version**.
`Execution::restore` (`execution.rs:257-293`) checks two things: that the snapshot's task matches the
plan, and that the snapshot's *state name* still exists in the workflow. A workflow that renamed
nothing and rewrote every guard therefore restores cleanly and silently re-governs the run.

So the **cursor** closes it. At run start the cursor records the resolved `workflow: <id>/<major>`,
the step map's id and the canonical digest of the validated map. `protocol drive --resume` compares
all three and **refuses when any moved**, printing both values. Fail closed, and the routes out are
named in the refusal: `--restart` (a new run id, evidence re-observed) or revert the document.

**Per review F20, the cursor records the engine's version too.** `Snapshot` carries
`#[serde(deny_unknown_fields)]` (`execution.rs:54`) and a serde default for `actor` (`:72`), so a
field a future engine adds makes an *older* driver refuse a *newer* snapshot — as a deserialisation
error, at the least informative possible moment. One field in the cursor turns that into *"this
snapshot was written by engine X and this driver links engine Y"*, which is the same class of
refusal D1 already makes for a moved workflow and should read the same way.

**Rejected.** *An unpinned map, resolving to whatever loads* — that is the drift the pin exists to
prevent. *Migration* — a mechanical rewrite across a major would have to guess which new state a
removed one became, and a major exists precisely because the author could not express the change
additively. *Refusing only at the transition that needs it* — that is failing at the most expensive
possible moment, halfway through a run that has already spent a token budget.

**Where the step-map types live — corrected per review F1, and it is the finding that moved the
wave.** D1 loads step maps through `load_tree`'s fixed table (`crates/aep-engine/src/load.rs:22-28`),
which is design open-decision D4's whole argument for `drivers/`. But `load_tree` lives in
`aep-engine`, its table's values are `aep_schema::parse::DocumentKind`
(`crates/aep-schema/src/parse.rs:32-51`), and `aep-engine` depends on `aep-schema`. A sixth row means
`aep-schema` must see `RawStepMap`; the router must see `Evaluation`
(`crates/aep-engine/src/evaluate.rs:124`) and `TransitionResult` (`engine.rs:104`). Putting both in
one `aep-driver` is the cycle `aep-schema → aep-driver → aep-engine → aep-schema`, and `cargo`
refuses it.

So the crate is **two**:

| crate | holds | depends on |
|---|---|---|
| **`aep-driver-spec`** — a leaf | `RawStepMap` → `StepMap`, `PinnedWorkflowRef`, the cursor types, `ToolConfig`, both cross-validation phases | `aep-domain` only |
| **`aep-driver`** | the three-valued router, `LlmStepExecutor`, `tool_config` | `aep-domain`, `aep-engine`, `aep-driver-spec` |

`aep-schema` then depends on `aep-driver-spec` — the same sideways edge it already carries to
`aep-backend-markdown`, `ess-domain` and `trace-domain`, and for the same reason: all four are
leaves. `drivers/` must be the **last** row of `TREE`, because phase 1 reads `workflow.states` out
of a registry the earlier rows fill. The cheaper alternative — `StepMap` in `aep-domain` — is one
fewer manifest and is refused: a step map is explicitly *"a harness's business"* (§ 4.2), and
`aep-domain` is the protocol's vocabulary. The split costs one `Cargo.toml`; finding the cycle at
`cargo build` after W3.1 lands costs a type move with the schema index pointing at the old path.

**Wave 3 must test.** A map naming a state the workflow does not have is refused at load, naming the
state and listing the workflow's; a map pinned to `/1` against `version: 2` is refused with a code
naming both, **accumulating with** the state errors rather than short-circuiting; a step claiming
`contract_result` from `verifier: test-runner` is refused at load; **a step naming
`verifier: some-external-tool` loads** (F5); **an unpinned `workflow: adp/default` is refused by
`PinnedWorkflowRef`, and the generated schema refuses the same document** — one assertion per side,
because the defect F6 names is the two disagreeing (F6); a step declaring a kind the task's protocol
does not declare is refused at **run start, before the first step executes**; and a snapshot resumed
after the workflow's major moved is refused by the cursor check — with the test asserting that
`Engine::restore` *would* have accepted it, which is what makes the cursor check load-bearing rather
than decorative.

#### D2 — store → facts: rebuild the graph every iteration, and cache nothing

**Taken.** At the top of every loop iteration the driver rebuilds the `ArtifactGraph` from the
planning store and hands it to the engine through `restore`. There is no incremental fact cache and
no dirty-tracking.

**Mechanism, and it is entirely existing code.** `MarkdownStore::load()` returns a `StoreReport`
(`crates/aep-backend-markdown/src/store.rs:85`); `StoreReport::graph()` builds the graph through
`ArtifactGraph::build`, which is where duplicate ids and edges pointing at nothing are caught
(`store.rs:329-330`). `Engine::restore(task, artifacts, snapshot)`
(`crates/aep-engine/src/engine.rs:250-259`) takes the graph *as an argument*, and
`Execution::restore` ends by calling `refresh_facts()` (`execution.rs:291`), which rebuilds the fact
store from the plan's facts, `self.artifacts.facts()` and the evidence log in that order
(`execution.rs:297-319`). "Restore with fresh artifacts" is therefore not a new engine capability —
it is what `restore` already does with whatever graph it is handed.

The loop is: read snapshot → `store.load().graph()` → `engine.restore(task, graph, snapshot)` →
`evaluate`. Once, at the top, and nowhere else. A step never mutates a live execution's graph
because there is no API to: the graph is an input the engine reads and never writes
(`engine.rs:198-200`).

**The boundary — which family comes from where.**

| fact family | source | produced by |
|---|---|---|
| `artifact.**` | the planning store | `ArtifactGraph::facts()` (`crates/aep-domain/src/artifact.rs:1830-1872`): `artifact.total`, and per kind and per kind *lineage* `exists`, `count`, `approved`, `approved.count`, `<status>.count` |
| `evidence.**`, `required_evidence.**` | submissions to this execution | `Execution::derived_facts` (`execution.rs:322-398`): `evidence.count.<kind>`, `evidence.first_seq.<kind>`, `evidence.last_seq.<kind>`, `evidence.missing` |
| `state.**`, `workflow.terminal`, `approvals.granted`, `test.first_result` | the engine | the same function |
| `tests.*`, `diff.*`, `static_analysis.*`, `review.*`, `trace_conformance.*` | evidence payloads | `EvidenceRecord::facts()`, folded in by `refresh_facts` (`execution.rs:309-311`) |

**The driver writes into no fact family at all.** The tempting one — projecting *"the step map says
this state's steps are done"* into a fact — is refused by name. A fact the driver minted is a gate
the driver evaluated, and § 4.1 says gates are evaluated only by the engine.

**The cost, stated rather than argued — and understated in the first draft. Corrected per review
F8.** It is more than a directory walk. Per iteration:

* **a full read and parse of every planning document.** `collect_documents` walks (`store.rs:371`),
  then every path gets `fs::read_to_string` and `PlanningDocument::parse` (`store.rs:105-120`). It
  is O(bytes of the store), not O(directory entries);
* **a full plan re-resolution.** `Engine::restore` calls `resolve(&task, &self.registry)` before it
  touches the snapshot (`engine.rs:250-258`), so profile extension, principle selection, capability
  composition and obligation resolution all run again, and `refresh_facts` then rebuilds the whole
  fact store from plan facts, graph facts and the entire evidence log (`execution.rs:297-319`).

**The conclusion does not move, and F8 agrees it should not.** Both are pure CPU over local files
with no clock and no network, both are linear, and D2's own argument extends to the re-resolve: a
workflow or profile edited mid-run is re-validated rather than assumed. Because the build *is* the
store's integrity check, the rebuild also re-runs `validate`'s structural half every iteration for
free — which is the answer to "is it an unacceptable cost or the correct semantics": it is both, and
the correct semantics is what is bought. A store large enough for this to hurt is a store of
thousands of markdown files, and the answer there is P4's SQLite backend, not a cache in the driver.
The number to produce before reaching for one is a measurement, and nobody has it: no
`.engineering/planning/` store exists in this tree to measure, which F8 labels as inferred and so
does this line. **A cache is refused for § 2.1's reason**: a cached membership list is a second copy
of the membership list, and a second copy is a second thing that can disagree with the first.

**Decided per F8, because it was accidental rather than chosen: the registry is loaded once per
`protocol drive` invocation; the store is rebuilt per iteration.** `Engine` holds its registry
(`engine.rs:172`) and `restore` re-resolves against whatever registry the engine was built with, so
a mid-run edit to `workflows/` is *not* picked up while a mid-run edit to `.engineering/planning/`
*is*. The asymmetry is right — D1's cursor pins the workflow for the life of the run precisely so a
governing document cannot move under it — but a defensible behaviour that nobody decided is one
nobody can be held to.

**A store that breaks mid-run stops the run — and the stop condition was wrong. Corrected per review
F7, the most dangerous finding in the set.** The first draft said `graph()` returns
`ValidationErrors`. It does, for **graph** problems — duplicate ids, edges pointing at nothing,
cycles. A document that fails to parse, or whose declared id disagrees with its path, never reaches
`graph()`: it lands in `report.failures` (`store.rs:100-120`), and `graph()` returns `Ok` for a
graph of what did load. The crate says so in its own doc comment (`store.rs:319-328`), and it is
deliberate — *"a listing of nine artifacts is more useful than a refusal because the tenth file has
a typo"* — which is right for reading and wrong for gating.

The consequence is the silent-corruption class. A driver checking only `graph()` gets `Ok` from a
store that has quietly lost a document; `artifact.story.count` drops by one,
`artifact.design.approved` flips from `true` to `false`
(`crates/aep-domain/src/artifact.rs:1830-1872`), and the engine evaluates a **completion gate**
against a fact base that shrank because of a typo — reporting a requirement unmet, or in the
`at_most` direction, met. A fact base that shrinks silently produces a verdict nobody can attribute.
So:

> The driver stops when **`report.is_clean()` is false** (`StoreReport::is_clean`,
> `crates/aep-backend-markdown/src/store.rs:314`) **or** `graph()` returns errors, and it consults
> `is_clean` **first**, because a file that did not parse is not in the graph to be wrong about.

The driver does not carry on with the last good graph — that is a run evaluating against a store that no longer
exists. It persists the snapshot and the cursor, prints the accumulated errors **verbatim** (the
plugin's guardrail 3, for its reason) and exits. It does **not** report `Blocked`: `Blocked` is the
engine's word for *the protocol says no*, and a broken store is not that.

**Rejected.** *A dirty-flag cache keyed on file mtimes* — a store a human edits in an editor is
exactly where mtime-based invalidation is wrong, and D-P2 already says an out-of-band edit is
indistinguishable in the file. *Rebuilding once per state instead of once per step* — a `command`
step can create an artifact, and the next step in the same state would then evaluate against a store
one write behind.

**Wave 3 must test.** An artifact created by a step changes `artifact.<kind>.count` in the next
evaluation, asserted **on the fact store**; a status move moves `artifact.story.<status>.count` and
flips `artifact.story.approved` exactly where `ArtifactStatus::is_approved`
(`crates/aep-domain/src/artifact.rs:709`) says it should; a mutation to the store is *not* observable
through a previously built graph, which is what proves the rebuild happens; a store with a broken
relation target stops the run with the store's own errors while leaving a run directory that
resumes; and — **the twin F7 asked for** — **a store with one unparseable file stops the run**,
asserted by the fact store being *unchanged* rather than silently shrunk, because that is precisely
the case a `graph()`-only check waves through.

#### D3 — `require_approval` headless: no tool, no auto-approve, and a static pre-flight refusal

The obvious first answer — *refuse a headless run whose `approval_required` set is non-empty* —
**refuses every run**, and this is worth stating before the decision because it is where a reader's
intuition goes. `principles/governance/least-privilege.yaml:19-22` has no `applies_when` (its first
line says a privilege rule with exceptions is not a privilege rule) and puts `production.write`,
`deployment.create` and `network.write` behind approval for every task under every profile. The test
has to be about what a run can **reach**, not about a set being non-empty.

**Taken, in three parts.**

**(a) An approval-gated capability is never a tool — and the derivation was wrong. Corrected per
review F3.** The first draft derived the `llm` step's tool set from `CapabilityPolicy::allow`
**only**, and called that "invariant 6's ordering". It is not. `allow`, `approval_required` and
`deny` are three independent `BTreeSet`s (`crates/aep-domain/src/capability.rs:485`, `:493`, `:497`);
nothing removes a capability from `allow` when a principle adds it to `deny` — `grant` extends all
three (`capability.rs:619-624`) and `restrict` extends two (`:630-634`) — and `AGENTS.md:176-179`
records that the invariant's own enforcing test *asserts its fixture holds one capability in all
three sets* before asserting the outcome. A capability in all three sets is a state the model is
built to represent.

Worse, membership is not equality. `find` matches on `covers` (`capability.rs:612-614`), and
`Capability::covers` widens across environments (`:240-246`) with `Environment::Any` covering
everything (`:83-85`). So an `allow` entry of unscoped `deployment.create` — which parses to
`Deploy(Environment::Any)` — **covers** `deployment.create:production`, the exact grant
`approval-gates.yaml:38` puts behind approval and the protocol floor gates independently
(`policy.rs:98-106`). Iterating `.allow` hands out the tool. No shipped profile has that pairing
today, and `profiles/release-progressive.yaml:29-31` avoids it **in a comment** — which is the class
of rule this repository writes registers about.

The ordering lives in exactly one function, and the derivation calls it:

```rust
// crates/aep-domain/src/capability.rs:588-599
pub fn decide(&self, capability: &Capability) -> CapabilityDecision {
    if Self::covered_by(&self.deny, capability) { return CapabilityDecision::Denied; }
    if Self::covered_by(&self.approval_required, capability) { return CapabilityDecision::RequiresApproval; }
    if Self::covered_by(&self.allow, capability) { return CapabilityDecision::Allowed; }
    CapabilityDecision::NotGranted
}
```

> **A capability is offered as a tool iff `policy.decide(&capability) == CapabilityDecision::Allowed`.**
> `RequiresApproval`, `Denied` and `NotGranted` all map to *no tool*.

That is invariant 6's ordering expressed at the one layer that can enforce it — by calling the
function that owns it rather than by reading one of its three inputs — and a model cannot request a
tool it was never given. It is also why the common case needs no approval machinery at all: a
development run that will never touch production cannot reach the gated capability, because no step
of it has a tool that would.

**(b) An approval the plan actually owes becomes an `operator` step, which is a pause and an exit.**
Interactive mode: the driver prints `CompletionExplanation` verbatim
(`Engine::explain_completion`, `engine.rs:267-269`), persists the snapshot and the cursor, releases
the lock, and exits 0 with the resume line. There is no waiting process and no queue — a driver
holding a terminal open for a person is a driver that loses the run when the terminal closes, and the
snapshot is already a queue that survives a reboot. What comes back is an operator-authored
`Evidence::Approval` with `Producer::Human` (`crates/aep-domain/src/evidence.rs:1093-1103`), which is
what `approval_recorded` (`crates/aep-engine/src/policy.rs:135-151`) reads to turn `RequiresApproval`
into allowed.

**(c) Headless refuses to start, and the test is static, decidable and run before the first step.**
The driver walks the resolved plan and asks whether any approval is *reachable*:

* `plan.completion` and every `plan.obligations[].requires` (`crates/aep-domain/src/plan.rs:90-113`
  and `:37-49`), every `workflow.states[].requires` (`crates/aep-domain/src/workflow.rs:98`), and —
  **added per review F9** — every `workflow.transitions[].requires`
  (`crates/aep-domain/src/workflow.rs:127-138`). A transition's requirement set is a first-class one
  that the evaluator reads beside the current and target states' (`crates/aep-engine/src/evaluate.rs:215`,
  against `:203` and `:226`), so a `human: true` approval on a transition is genuinely owed. No
  shipped workflow uses it today — `workflows/development/default.yaml`'s one `requires:` is on a
  *state*, at `:60` — which is exactly why the omission would have been found by a user rather than
  by the gate;
* in each: an `approvals[]` entry with `human: true`
  (`crates/aep-domain/src/requirement.rs:794-799`), a `reviews[]` entry with `human: true`
  (`requirement.rs:607-619`), or an `evidence[]` entry whose `verifier` satisfies
  `Verifier::is_human` (`crates/aep-domain/src/verification.rs:120-122`);
* plus any capability the **`command` steps in the map** would exercise for which
  `policy.decide(&capability) == RequiresApproval` — an `llm` step cannot reach one, by (a).
* A conditional counts as reachable unless its `when` evaluates `False` against the plan's facts,
  **and the walk recurses**: `ConditionalRequirement.require` is a `Box<RequirementSet>`
  (`requirement.rs:895-900`), so conditionals nest, and the `when == False` skip is applied at every
  level. **Corrected per F9** — the precedent this rule was borrowed from, `count_missing_evidence`
  (`execution.rs:400-412`, applied at `:421-423`), descends exactly one level *by design*, because
  it is counting rather than proving absence. A reachability scan that stops at one level
  under-reports, and under-reporting here means starting a headless run that will wedge. An unknown
  guard is treated as in force, never as absent.

**Two consequences the first draft did not state, both supplied by review F4(2), and the first is
load-bearing.** `development.standard` includes `approval-gates`
(`profiles/development-standard.yaml:18-22`), whose `before_completion` obligation carries a
`human: true` approval (`principles/governance/approval-gates.yaml:22-26`). Read naively, *that*
also refuses every standard run. It does not — but only because the obligation is conditional and
its guard is deliberately two-valued:

```yaml
# principles/governance/approval-gates.yaml:16-22
      # Guarded with `defined(...)` on purpose: the guard has to be two-valued.
      - when: defined(deployment.production.status)
```

```rust
// crates/aep-domain/src/predicate.rs:402
Self::Defined(path) => Truth::from_bool(facts.fact(path).is_some()),
```

With no deployment fact at pre-flight, `defined(...)` is `False`, the conditional is skipped, and
the run starts. **D3's headline wave-3 test is green for a reason outside D3**, and saying so is the
point: a future principle author who writes the bare `deployment.production.status == succeeded`
instead of `defined(...)` silently turns every headless development run into a refusal, and nothing
in the driver would explain why.

The second consequence is a surprise that should not be one:
`profiles/development-critical.yaml:46-52` carries an **unconditional**
`reviews: [{subject_kind: design, result: approved, human: true, fresh: true}]`. Under the corrected
rule, **a headless run under `development.critical` refuses to start** unless the design review
already exists in the store. That is the right behaviour — a profile chosen for work whose failure
is silent should not run unattended past a human review — and it is written here so that the first
person to meet it reads a decision rather than a bug.

If anything is reachable and the run is headless, `protocol drive` **refuses to start**, printing
every reachable approval with the document that asked for it. The route out is
`--pause-on-approval`, which converts the run to *run until the first approval, then persist and
exit 0*. It is opt-in because it changes what a green exit means: without it exit 0 means finished,
with it exit 0 means finished **or** waiting, and a caller has to choose to be told that.

**There is no auto-approve, under any flag, ever — and there is a specific reason it has to be the
driver's refusal.** `approval_recorded` (`policy.rs:135-151`) matches an `Evidence::Approval` on its
subject or approval id and on `ApprovalDecision::Granted`. It does **not** check who granted it.
`ApprovalRequirement::evaluate` does — it skips a record whose approver is not human when
`human: true` (`requirement.rs:839-874`) — but the *capability* gate does not. So nothing below the
driver would stop a harness from writing its own approval and unlocking a capability with it. The
driver therefore **never constructs an `Evidence::Approval`**: the only route into a run is a
document a person wrote, and anything the driver mints for itself carries
`Producer::Harness { id }`, which satisfies neither `independent: true` nor `human: true`.

**Rejected.** *Queue* — a queued approval is a run that is neither finished nor failed and whose
state lives in a process. *Auto-approve behind a flag* — a gate a caller's own flag can satisfy is
not a gate, the same argument trace wave 1 makes about `--advisory`
(`docs/plan/trace-wave-1-transcript-checker.md:39`). *Refuse on a non-empty `approval_required`* —
refuses every run, as above; named because it is the obvious answer and it is wrong.

**Wave 3 must test.** A headless run under `development.standard` with no production path **starts** —
the regression that catches the naive rule, and the test comment names the `defined(...)` guard it
depends on; a headless run whose plan owes a `human: true` approval refuses to start with a non-zero
exit naming the principle and the obligation; **a headless run under `development.critical` refuses
to start** with the unconditional design review named (F4(2)); **a workflow whose `verify → complete`
transition carries a `human: true` approval refuses a headless start, naming the transition** (F9);
the same run under `--pause-on-approval` reaches the approval, persists and exits 0, and resumes;
and a source scan finds no construction of `Evidence::Approval` and no `Producer::Human` anywhere in
`aep-driver` — the shape `crates/aep-engine/tests/evidence_scan.rs` already uses for invariant 7.

**The tool-set test is the mutation F3 named, not the weaker one the first draft wrote.** *"The
allowlist contains no tool for any capability in `approval_required` or `deny`"* passes for an
implementation that iterates `.allow`, because it never constructs the case where the two sets
overlap by `covers`. The test is instead: **a fixture policy whose `allow` holds unscoped
`deployment.create` and whose `approval_required` holds `deployment.create:production`, asserting no
deploy tool is offered.** That fails on `.allow` membership and passes on `decide`.

#### D4 — session granularity: one model session per `llm` step

**Taken: per step, not per state.** Each `llm` step is one `claude -p` invocation carrying that
step's prompt and named skills and a tool set derived from the state's `capabilities()`. The process
exits when the step does. Nothing is passed by `--resume` or `--continue`.

**Context is carried by the store and the prompt, never by session memory.** The prompt is assembled
from the step map's `prompt`, the state's requirement lines (`ProtocolEngine::requirements`,
`engine.rs:146` and `:277-279`, each of which names the document that asked for it) and the artifacts
the task references (`Task::artifacts`, `task.rs:347-349`). Everything an `llm` step knows is either
in a file or in that prompt, which is the property that makes the next point true.

**Why per-step beats per-state.**

* **Replayability is the whole claim, and it is only checkable if each step's input is a function of
  persisted state.** § 4.1 narrows replay to *the same snapshot and the same evidence yield the same
  routing*. A long-lived session makes step two's input depend on step one's hidden context, which is
  neither snapshotted nor diffable, and the narrow claim quietly stops being true.
* **The allowlist is per state, and a session that outlives a transition outlives its allowlist.**
  `effective_policy` grants the state's capabilities on top of the plan's
  (`crates/aep-engine/src/policy.rs:84-92`), so the legal tool set genuinely changes at every
  `Moved`. A session held across the boundary either keeps the old tools or re-registers mid-session,
  and the first is a hole in exactly the enforcement § 4.8 is about.
* **Failure is per step, and so is the retry budget** (D5). A retried step inside a shared session
  retries with the failed attempt still in context, which is the opposite of a retry.
* **It is the only granularity at which a launch-time flag can express a per-state tool set** —
  **added per the review's resolution of open cell (b)**, and it is mechanical rather than
  argumentative. `--allowedTools` is fixed at session launch; there is no mid-session list swap
  (hooks reference). § 4.8 row 3 wants the set re-rendered at every `Moved`, and a step never spans
  a transition, so every session is launched with the tool set for the state it runs in. **Per-state
  sessions would already be wrong here, and one session per run would be unimplementable.** The
  review found this while filling a cell about hooks; it is the strongest of the four reasons and it
  was not one of the three the first draft gave.

**The honest cost, and where it is measured.** Per-step means re-sending the preamble, so a state
with four `llm` steps pays four of them. That cost is *observable* rather than arguable: the trace
family already measures the per-step `gen`/`exec` split and the token census
(`docs/plan/trace-wave-1-transcript-checker.md:78-79`, `:117-119`). If it dominates, the fix is
fewer and larger `llm` steps in the map — a document change, reviewable in a diff — not a stateful
session that trades the replay claim for tokens.

**One property this buys that belongs here rather than in § 4.8: the hook-versus-engine TOCTOU
window is zero by construction.** A `PreToolUse` hook that consults the driver races the engine only
if the engine can advance while the session is live. It cannot: the driver runs the step to
completion, *then* submits evidence, *then* calls `transition()` (§ 4.4). The review confirms the
window closed and records where it re-opens — **the moment anybody proposes a longer-lived session**
— so the note lives with D4, which is the decision that would have to be overturned first.

**The driver never passes `--bare`, and the hook configuration is passed with `--settings`.**
**Added per review F15**, which is the quietest hole in the set: `--bare` skips hooks (hooks
reference), and nothing in the first draft constrained the command line the driver builds. A future
implementer reaching for `--bare` to get a clean reproducible environment — a reasonable instinct in
a repository this deterministic — would **silently delete the driver's own enforcement arm**, and
every § 4.8 row whose layer is *plugin hook* would become a claim with nothing behind it. The
failure would be partial (the `--allowedTools` layer survives), silent, and exactly the shape this
repository writes registers about. It is a rule with a test, not a note: **wave 3 asserts over the
constructed argv** that it contains no `--bare` and does contain the settings path.

**Resume after a driver restart is a new session with the same inputs**, and the cursor says so. A
resumed run's transcript is a different transcript and its digest differs, which is correct rather
than unfortunate: the step did run twice, and a record claiming otherwise would be the record lying.

**Rejected.** *Per-state sessions* — above. *One session for the whole run* — all of the above, plus
a context that outgrows the point at which a clean per-state tool set can be applied to it.
*`--resume` for retries* — see D5; a retry that can read the failure is not an independent attempt.

**Wave 3 must test.** Two `llm` steps in one state produce two transcripts with two distinct session
ids; a step's prompt is byte-identical whether or not the previous step ran in the same process; a
step's tool set equals `tool_config` over `effective_policy` for the state it ran in — decided by
`CapabilityPolicy::decide`, not by `.allow` (F3) — asserted against the policy rather than a
hard-coded list; **the constructed `claude -p` argv contains no `--bare` and does contain the
settings path** (F15); and a resumed run's prompt at a given cursor position is byte-identical to
the pre-restart one.

#### D5 — failure taxonomy: the driver never converts an absence into a `False`

**Taken.** Three-valued routing, bound to `Truth` (`crates/aep-domain/src/predicate.rs:56`) and to
`TransitionResult` (`engine.rs:104-126`).

| what happened | what it means | what the driver does |
|---|---|---|
| the step could not run or produced no verdict — missing executable, killed process, model-call error, timeout, unreadable transcript | **nothing was observed** | submit **no evidence at all**; retry within the step kind's budget; on exhaustion, snapshot and exit |
| the step ran and a verifier said no — a suite with failures, a linter with errors | **observed, and wrong** | submit the evidence that says so, then `transition()` and let the workflow route |
| the step ran and a verifier said yes | observed, and right | submit, then `transition()` |

**The load-bearing sentence: `Unknown` is spelled "submit nothing".** The engine has no `Unknown`
value to submit — absence is modelled as the fact simply not being in the store (`refresh_facts`,
`execution.rs:297-319`). A crashed `cargo test` is therefore *not* `tests.unit.failed > 0`;
submitting a failing `TestResult` for a suite that never ran would fabricate an observation, which is
invariant 7's failure one layer above the engine.

**This is why the routing must be the engine's.** `workflows/development/default.yaml:117-127` guards
`verify → implement` on `any: [tests.unit.failed > 0, tests.contract.failed > 0,
static_analysis.errors > 0]` and `verify → adversarial_verify` on the `all:` of the same three at
zero (`:106-116`). With no test evidence at all **both** guards are `Unknown`, so `transition()`
returns `Blocked` (`engine.rs:397-415`) — which is exactly right, and is the thing the driver retries
against. A driver that collapsed the crash into `False` would take the back-edge and send an agent to
fix code nobody ran, which is the guide's own named failure (`docs/guide/harness.md` § 3).

**The ambiguous cases are decided by one question: did a verifier produce a verdict?**

| case | verdict? | routed as |
|---|---|---|
| timeout mid-suite | no — a partial suite is not a failing suite | Unknown |
| OOM, killed process, missing binary | no | Unknown |
| network error inside a step that reaches the network | no | Unknown |
| suite ran to completion and reported failures | yes | False |
| `protocol trace check` exit 3 — nobody found out (`docs/plan/trace-wave-1-transcript-checker.md:147-149`) | no verdict, **but a record exists** | Unknown, *and* the record is submitted |

The last row is the one exception and it is deliberate. `trace evidence` writes a record whose own
status is `inconclusive` and whose `trace_conformance.passed` is false
(`crates/trace-spec/src/evidence.rs:43-54`). That is a **recorded** absence rather than a silent one,
and recording it is strictly better than dropping it: a requirement stays owed either way, and the
run report can say which check could not be read.

**Retry budgets.**

* **Per step kind, not per step.** `command` steps retry (a process died); `llm` steps retry once (a
  model call errored); `operator` steps never retry, because a person is not a flaky dependency.
* **The budget is spent, not reset.** A retried step that succeeds does not erase the first attempt.
  There is no evidence to erase — the failed attempt produced none — but the cursor records the
  attempt count and the run report names it, so *"green on the third try"* stays visible.
* **The step budget and § 4.4's per-state visit budget are different bounds.** The first bounds *this
  step keeps crashing*; the second bounds *this loop keeps going round*. Collapsing them would make a
  legitimate `verify → implement` cycle indistinguishable from a wedged command.
* **Exhaustion is reported in the engine's words plus one of the driver's own.** The driver prints
  `TransitionResult::Blocked`'s `reasons` and `CompletionExplanation` verbatim
  (`engine.rs:410-414`, `:267-269`), then adds exactly one line naming the budget and the step. It
  does not invent a verdict, and it does not summarise the engine's.

**Rejected.** *Treating a non-zero exit code as `False` uniformly* — exit codes mean different things
per tool, and `protocol trace check` ships three of them with `3` explicitly not a softer `1`.
*Retrying an `operator` step* — re-prompting a person who did not answer is the driver deciding a
human is a transient fault. *Unbounded retry with backoff* — a bound nobody can state is a token
budget nobody can state.

**Wave 3 must test.** A `command` step whose executable does not exist submits **zero** evidence and
leaves the evaluation unchanged; a suite that runs and fails submits a `TestResult` with failures and
the next `transition()` returns `Moved { from: verify, to: implement }`; a step exhausting its budget
leaves a resumable snapshot and a `Blocked` report; the cursor's attempt count survives the retry
that succeeded; and a source scan finds no `Producer::Human` and no `Evidence::Approval` in
`aep-driver`, with `Producer::Verifier` constructed in exactly one place — the command-step evidence
builder, which fills it from the verifier the step map named.

#### D6 — concurrency: one execution per store, and the lock is the allocator

**Taken, and corrected per review F2 — the first draft was circular and F2 is right to call it
INFEASIBLE.** It said the lock lives at `.engineering/runs/<run-id>/lock.json` *and* that the run id
is allocated **after** taking the lock. There is no order in which those two execute, and the
failure mode is the one D6 exists to prevent: two invocations count the existing directories, get
`3` and `4`, and **both `create_new` succeed**, because they are different paths. Two live runs, one
store, D2 rebuilding the graph under both — D6's own rejected option, *"no lock, last writer wins"*,
reached by accident.

**One fixed path per store, taken before anything is allocated:**

```text
.engineering/runs/lock.json     ← create_new; the mutex. Carries pid, host, driver version,
                                   and the run id it granted.
.engineering/runs/current       ← the store-level pointer
.engineering/runs/<run-id>/     ← snapshot.json, cursor.json, transcripts. No lock file.
```

Everything else in D6 survives verbatim, and one thing improves: the holder's run id is now *inside*
the lock rather than around it, so a refusal prints it without opening a second file.

**The run id is the driver's, not the engine's, and this is a correctness point rather than a naming
preference.** `ExecutionId` is `format!("{}.{ordinal}", plan.task.id)` where `ordinal` comes from an
`AtomicU64` initialised to zero **in each `Engine` value** — **corrected per review F10**, which
found the first draft's *"in each process"* understates it: the counter is a field
(`engine.rs:173`), initialised at construction (`:186-190`) and read at `:210-213`, so two `Engine`s
in **one** process collide too, which is precisely the shape a test harness builds. Two
`protocol drive` invocations against the same task therefore both mint `<task>.1`, and a run
directory keyed on the execution id alone would have one run overwrite the other's snapshot. The
hazard is confined to `initialize`: `Execution::restore` preserves the snapshot's id
(`execution.rs:277`), which is worth saying because it bounds the fix. The driver allocates
`<task-id>/<n>` **after taking the lock**, by counting the directories that already exist; the
engine's `ExecutionId` is recorded *inside* the cursor so the two can be joined later, and is never
the path.

**The lock is a file created with `create_new`.** One syscall, atomic on every filesystem that
matters, and it needs no advisory-locking support. `lock.json` carries the pid, the hostname, the run
id and the driver's version. **It carries no timestamp** — § 2.4's rule holds here too, and staleness
is decided by liveness rather than by a number somebody wrote into a file.

**Stale-lock policy, in order, with no age threshold.**

| condition | verdict |
|---|---|
| pid alive | held — refuse |
| same host, pid not alive | **stale, and still refused** without `--take-lock`; the refusal names the pid and the run |
| different host | never stale, whatever the local pid table says |

**There is deliberately no age threshold**, and this is the decision inside the decision. Any
threshold has to exceed the longest legitimate step, and the longest legitimate step is *an operator
step waiting for a person*, which has no bound (D3). A driver that broke a lock after two hours would
break exactly the runs that paused correctly. Requiring `--take-lock` makes stealing a lock something
a person did, which is the same shape as § 3.4's rule that a claim about the state of the world is the
operator's to make.

**A second `protocol drive` refuses and prints the holder** — run id, pid, host, and the state the
cursor says it is in — exits non-zero, and names the two routes: `--resume` that run, or
`--take-lock` if the holder is provably dead. Refusing while naming what to do instead is the same
choice `protocol artifact move` makes for an illegal transition (§ 2.5), for the same reason.

**The lock is released on every exit path the driver controls, including the approval pause and
budget exhaustion.** A paused run does not hold a lock, because the pause has no bound. What a paused
run keeps is `current`, so resuming is one word. **Added per F2:** because a paused run holds
nothing, **`--resume` must re-take the store lock before it writes, and must refuse if another run
now holds it.** The first draft said a pause releases and never said a resume re-acquires, which
left the reader to assume the obvious — and the obvious assumption is the one that produces two live
runs.

**Where the lock lives, decided per review F19.** § 4.1 puts the three things that touch the world
in `protocol-cli`, and D6 never said which side of that line it sits on. It sits on the impure side:
**the lock file, the pid-liveness probe and the run directory are `protocol-cli`'s (W3.3); the
router in `aep-driver` is handed a `LockState` and never probes anything.** A liveness probe reads
ambient OS state and would slip past a banned-token scan — it uses neither `SystemTime::now` nor
`rand` — so placement is the only thing keeping `aep-driver`'s purity claim true. It also makes the
lock testable without spawning a second process.

**A removable lockfile is not an invariant-16 breach — the review was asked and overturned the worry
(F17).** Invariant 16's subject is the **entity command vocabulary**: *"`ArchiveEntity` and
`SupersedeEntity` are the vocabulary"*, enforced by there being no delete variant to call
(`AGENTS.md:239-242`). A lock file is not an entity, `--take-lock` is not a `CommandKind`, and
removing `lock.json` is no more a breach than removing a build artifact. Two adjacent rules are
adopted anyway, in the invariant's spirit, because they cost nothing:

* **a run directory is never deleted and never reused.** `--restart` allocates a new run id;
* **`--take-lock` supersedes rather than erases.** The stolen lock's contents go into the new run's
  cursor, so *"this run took the lock from pid 4711 of run `<task>/2`"* is in the record. One field,
  and it is the difference between a run whose history explains itself and one that does not.

**Rejected.** *A lock per task* — the store is the shared thing, and D2's per-iteration rebuild makes
two tasks writing one store a live race rather than a stale read. *`flock`* — semantics differ across
the filesystems people keep repositories on, NFS in particular. *An in-process mutex* — does not
cross processes, which is the entire case. *No lock, last writer wins* — two runs interleaving
snapshots into one directory produce an execution history that never happened, which is worse than
either run failing.

**Wave 3 must test.** A second `drive` against a locked store exits non-zero, prints the holder's run
id and pid, and **writes nothing** — asserted by an unchanged run directory and a clean tree; a lock
whose pid is dead is reported stale and still refused without `--take-lock`; a lock naming another
host is refused even when its pid is not alive locally; two executions initialised in one process do
not collide on a run directory — **two `Engine` values in one process**, which is the sharper form
F10 found; the lock is absent after an approval pause while `current` still points at the run; and
**`--resume` against a store whose lock is held by another run refuses** (F2), which is the test the
first draft's silence about re-acquisition would have left unwritten.

### 4.8 Enforcement mapping — every rule class, and the mechanism that holds it

The target this design is for, stated once: **current agent evaluation is prose-based steering plus
post-hoc verification; the target is a specified workflow that runs strictly.** The driver holds the
loop, the engine decides, and the rules are enforced deterministically — not asked for politely in a
skill file. This section is the mapping, and it is first-class rather than an appendix because a rule
whose mechanism is unnamed is the thing `AGENTS.md` § *Invariants* says has already drifted.

**The marked cells are filled. Updated per the feasibility review, 2026-08-21.** The first draft
marked every Claude-Code-specific cell **(per hooks reference)** because it was written from the
shape of the mechanism rather than from its documentation. The review filled all three open cells
against the hooks reference, and the table below now carries the answers rather than the marks —
except where the answer is *"this is not documented anywhere"*, which is named as an assumption
below rather than dressed up as a fact. Three of the table's audit columns were wrong or
unimplementable and are corrected here: **F12** (row 3 needs an expectation kind that does not
exist), **F13** (row 1's kind is a whole-run count), **F14** (a hook cannot reach `Engine::authorize`).

Facts taken from the hooks reference and used below: `PreToolUse` denies deterministically by exit
code **2** or by `{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"…"}}`;
the reason is fed back to the model; the hook sees the full `tool_input`, which is what makes a path
check possible; `matcher` selects by tool name and `if` takes permission-rule syntax; a hook can
**deny but never grant**; hooks fire identically under `claude -p` (`PermissionRequest` hooks do
not, which is why `PreToolUse` is the one used); a plugin ships them in `hooks/hooks.json` at its
root; multiple sources run in parallel and the most restrictive wins; and `--allowedTools` is fixed
at session launch, with no mid-session swap.

| rule class | where the rule is stated | mechanism that enforces it | layer | what audits that it held |
|---|---|---|---|---|
| **capability denied** | `decide()` returns `Denied` or `NotGranted` (`capability.rs:588-599`) | the tool is not in the derived set at all; **and** a `PreToolUse` hook denies it if it is reached by another route — exit 2 or `permissionDecision: deny`, with the reason fed back | driver + plugin hook | **F13, corrected — and answered by W3.6 on 2026-08-21.** The transcript's **whole-run** `permission.denied` count (`crates/trace-domain/src/spec.rs:377-378`, dispatched `check.rs:156-160`, read from one array's length at `adapter.rs:477`, `:571-574`) evidences *that* a refusal happened, never *which*, and `0` cannot distinguish enforcement holding from nothing being attempted. **The open question — does a hook's `permissionDecision: deny` reach that array at all — is now answered: yes, one-for-one.** See the F13 record below. It stays a **whole-run count** even so, so the gating record is the hook-decision log and `protocol artifact validate`, and the transcript row is advisory |
| **capability approval-gated** | `decide()` returns `RequiresApproval` (`capability.rs:588-599`); `least-privilege`, `approval-gates` | no tool headless; an `operator` step interactive; `Engine::authorize` returns `RequiresApproval` and records it (`policy.rs:95-118`, `engine.rs:285-313`) | driver + engine | the audit trail — `ActionRequested` / `ActionDenied` events (`engine.rs:288-311`) — **for decisions the driver makes**. A hook's decisions reach the same trail only through the decision log below (**F14**) |
| **per-state tool set** | the workflow's `State::capabilities` on top of the plan's (`workflow.rs:101`) | `effective_policy` (`policy.rs:84-92`) → `tool_config` (§ 4.9 point 2) → **`--allowedTools` at session launch**, which is the only place it can go, since the list is fixed at launch and a step never spans a transition (D4); **plus** a `PreToolUse` hook denying anything outside the same derived set, re-rendered per step | driver + plugin hook | **F12 built, and the audit is still open — the kind reads the wrong list.** `env.tool_available` shipped as the 50th expectation kind (`crates/trace-domain/src/spec.rs:231-232`, dispatched `crates/trace-spec/src/check.rs:113` → `:563`), before the hooks, as F12 required. Using it showed that **`SessionStart.tools` is the harness's tool *inventory*, not the session's allow rules**: the committed fixture `crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl` was launched with **nine** allowed tools and lists **thirty-two**; the driven runs pass **eight** and list **twenty-eight**. The kind is still load-bearing — it rules out *"the tool did not exist"* as an explanation for a refusal, which is what makes a refusal attributable to a layer that **chose** to refuse — but **this row has no transcript-side allowlist audit and this table no longer claims one.** What would close it is in `docs/plan/gap-register.md` |
| **evidence gates** | workflow `when:` guards and `requires.evidence` | `Engine::transition` → `TransitionResult::Blocked` with one reason per unmet requirement (`engine.rs:397-415`) | **engine — exists today** | the audit trail: `TransitionBlocked` events (`engine.rs:400-409`) |
| **ordering** (spec before decompose; test before implement; red before green) | workflow transitions; `evidence.first_seq.*` | the same transition guards, over facts the engine derives from submission order (`execution.rs:354-363`) | **engine — exists today** | `evidence.first_seq.test_result < evidence.first_seq.diff`, as the guide states it |
| **store integrity** — a status is never hand-edited | guardrail 1, § 3.3; deviation D-P2 | a `PreToolUse` hook with `matcher: "Edit\|Write\|NotebookEdit"` reading `tool_input.file_path` against `.engineering/planning/**` — **`NotebookEdit` added per F16**, it writes files and is in the offered set. The matcher is exhaustive **only given** that no development profile grants `command.execute`, so there is no `Bash` to route around it; that fact is stated below rather than assumed here | plugin hook | `protocol artifact validate`, which catches an illegal status **whether or not the hook fired** — the strongest audit in this table, and the review left it unchanged |
| **an LLM cannot mint evidence** | invariant 7; the vision's one named trust | **type-level**: the `Llm` step variant has no `evidence` field (§ 4.3); `TraceEvidence::PRODUCER` is a constant with no call site that can set it (`crates/trace-spec/src/evidence.rs:98-100`) | the type system | not needed — unrepresentable states need no audit |
| **an agent's own claim is not independent** | `EvidenceRequirement::independent` (`requirement.rs:195`) | `Producer::Agent` does not satisfy it; the engine checks at evaluation | **engine — exists today** | `CompletionExplanation`, one line per requirement |
| **an agent's *behaviour* is checkable** | the trace family | `trace check` reads the transcript the model produced; `to_evidence` mints `trace_conformance` with a producer nobody can set (`evidence.rs:169`) | verifier | itself — it *is* the audit |
| **no route around the allowlist via a subagent** | § 4.9 point 2 — `Task` maps to no `Action` | `Task` is **never offered**. A subagent runs with its own tool set that nothing in D1–D6 derives, so it would be a hole in the per-state allowlist | driver | `subagent.spawned: {count: {at_most: 0}}` — a kind that already ships (`crates/trace-domain/src/spec.rs:797`, dispatched `crates/trace-spec/src/check.rs:161-163`, read at `crates/trace-spec/src/adapter.rs:478-480`). Enforce and verify, on one object, with vocabulary that exists |
| **run bounds** — visit budget, retry budget | the step map and the cursor | the driver refuses to continue and exits with the state or step it was cycling in (§ 4.4, D5) | driver | the run report and the cursor's attempt counts |
| **one run per store** | D6 | `.engineering/runs/lock.json` — **one fixed path**, created with `create_new` **before** any run id is allocated (**F2**); `current` as the store-level pointer | driver (`protocol-cli`, per **F19**) | the refusal itself names the holder, whose run id is inside the lock |

#### `Bash` is not a function of a capability — and the property that makes it survivable

**Added per the review's resolution of open cell (c).** Every other tool in the offered set is a
function of one capability. `Bash` is not: one call can be `tests.execute` (`cargo test`),
`command.execute` (`ls`), `repository.write` (`sed -i`, `>`), `network.write` (`curl -X POST`) or
`secret.read` (`cat ~/.aws/credentials`). The guide's table is total in the `Action → Capability`
direction (`docs/guide/harness.md:131`) and says nothing about the reverse, which is the direction a
tool table needs. So the rule, stated rather than implied:

> **`Bash` is offered only when `decide(command.execute) == Allowed`, and granting `command.execute`
> is understood to grant a superset of the shell's reach.** Any narrower gating — `if: Bash(cargo
> test *)`, or a hook classifying `tool_input.command` — is **pattern-based and best-effort**, and
> this section says so rather than listing `Bash` as fully enforced.

**And here is the property that makes that far less painful than it sounds, which the review found
and which this section should have claimed from the start.** No development profile grants
`command.execute`: `development.fast` allows `repository.read`, `repository.write`, `tests.execute`,
`artifact.read`, `artifact.write` (`profiles/development-fast.yaml:30-35`), and
`development.standard` adds `review.request` and `approval.request`
(`profiles/development-standard.yaml:28-30`). Capabilities default to deny (invariant 6). Therefore:

> **Under both development profiles, an `llm` step holds no shell at all.** `cargo test` still runs
> — as a `command` **step the driver executes**, not as a tool the model holds. The model never gets
> a shell in a development run; `tests.execute` is exercised by the driver.

That is a stronger enforcement claim than anything the first draft made, and it is the fact row 6
leans on: with no `Bash`, the `.engineering/planning/**` write guard only has to cover `Edit`,
`Write` and `NotebookEdit`.

**One profile now grants it, and the premise above is replaced rather than dropped (built
2026-08-21).** The claim was true of the two profiles that existed when it was written, and building
the driven eval found what it had not costed: **the planning store has no tool surface other than the
`protocol` CLI.** Its whole vocabulary — `artifact new`, `move`, `relate`, `validate` — is shell
commands, on purpose, because the CLI owns the frontmatter precisely so that nothing else writes it.
So a driven `llm` step under `development.standard` can be told to write a specification as an
artifact and has **no way to create one**; the guard on `artifact.specification.exists` does not fail
loudly, the run simply never moves.

The narrow fix is not expressible: **`command.execute:protocol` is a parse error.** Scoping exists
for exactly one thing, an `Environment` on `deployment.create` and `deployment.rollback`, and
`Capability::parse` refuses a scope on a simple capability
(`crates/aep-domain/src/capability.rs:272-280`). Inventing a second scoping axis to serve one profile
would put a new dimension into the protocol's vocabulary to solve a harness's problem, which is the
wrong layer.

So the resolution takes the other half of this section's own shape for the same class of rule — **a
capability grant plus a hook constraint**:

* `profiles/development-driven.yaml` (`development.driven`) is `development.standard` **plus
  `command.execute`**, and says in its own header that the grant exists so the `protocol` CLI is
  reachable and for no other reason. It is for a run under `protocol drive` and for nothing else;
* `integrations/claude-code/hooks/driven-surface.sh` is the constraint: it denies any `Bash` call
  that is not **one simple invocation** of `protocol artifact …` or `protocol trace …` — no pipes,
  no redirection, no `&&`, no command substitution, because a composed command line is a second
  command wearing the first one's name.

Two properties keep row 6 standing, and both are checked rather than asserted:

* **the approval floor is untouched.** `command.execute` is not in `protocols/aep/1.yaml`'s
  `approval_floor`, whose two entries are still `production.write` and
  `deployment.create:production`. Nothing that needed a recorded approval before needs one less now;
* **row 6's matcher no longer depends on the shell being absent.** It was exhaustive over `Edit`,
  `Write` and `NotebookEdit` *given* that premise; under this profile the premise is gone and the
  surface hook replaces it, denying the `sed -i` that would otherwise have routed around the write
  guard. **Both hooks ship together for that reason**, and neither is sufficient alone.

Per this section's own standard the narrowing remains **pattern-based and best-effort**: granting
`command.execute` grants a superset of the shell's reach, and a hook narrows that reach rather than
making it a function of a capability. The two development profiles that grant no shell are unchanged
and stay the right choice for interactive work and for any harness that cannot constrain a shell to a
named surface.

**`Skill` is a named exemption.** § 4.3 has `llm` steps naming skills, and `docs/guide/harness.md:144-146`
says a tool with no `Action` to describe it is a tool the protocol cannot govern. Both are right and
they collide, so the resolution is written as a decision rather than left as an oversight: **`Skill`
loads instructions and takes no action; everything it causes is a subsequent tool call, which is
governed.** It is offered; nothing else without an `Action` is.

#### How a hook's decision reaches the audit trail — decided, per F14

Rows 1 and 2 named the audit trail, and **a `PreToolUse` hook cannot reach it.** `Engine::authorize`
takes `&mut Execution` (`crates/aep-engine/src/engine.rs:285`) — an in-memory value in the driver's
process, whose mutation is the point (`docs/guide/harness.md:23-24`: *"`authorize` takes `&mut`
because asking is itself an event"*). A hook is a separate process with JSON on stdin. It could
shell out to a `protocol authorize`, but that process would build a *different* `Execution`, emit
its events into that one and drop them on exit; the driver's snapshot would never see them.

The review offered three options and recommended the first. **Taken as (a):**

> **The hook appends its decision to `.engineering/runs/<run-id>/hook-decisions.jsonl`, and the
> driver folds each line into the execution through `Engine::authorize` after the step exits.**

One file format and one fold. The decisions land a moment late — after the step rather than during
it — and they land in the *real* trail, so the snapshot carries them and `audit_trail` sees them.
The alternatives are named because refusing them is part of the decision: **(b) a local socket the
hook queries** puts a server inside a program that is otherwise a batch job, and adds a failure mode
that can hang a run; **(c) the hook enforces without asking**, rendering the same `tool_config` the
launch flag did from a file the driver wrote — simplest, and it would require rewriting rows 1 and 2
to say *the transcript* instead of *the audit trail*. What is not defensible, and was what the first
draft did, is leaving the cell naming a mechanism the layer it is attached to cannot reach.

Folding late is safe for the same reason the TOCTOU window is zero (D4): the driver does not call
`transition()` until the step's process has exited, so no decision is folded in after the state it
was made in has been left.

**Built 2026-08-21: the channel exists, and the fold is deliberately deferred.** Splitting the
decision into its two halves is what makes the deferral reportable rather than a quiet omission:

* **the channel is in the tree.** Both hooks append a JSON line — hook, decision, tool, state, step,
  capability, reason — to `<run-dir>/hook-decisions.jsonl`. They find the run directory from a
  **step context** the driver writes (below), and the driven eval reads the log as its own gating
  evidence: on the run of 2026-08-21 it held **10 decisions, 6 allow and 4 deny**, which is the only
  record in the system that can tell *denied* from *never attempted*;
* **`Engine::authorize` does not yet ingest it.** Deferred for one reason, stated so a later wave
  does not have to rediscover it: **every decision the log has ever held is a refusal or an
  allowance of an action that then happened anyway, and neither changes engine state.** `authorize`
  exists so that *asking is itself an event* — it appends `ActionRequested` / `ActionDenied` to the
  audit trail — so folding the log in adds **provenance to the trail**, not enforcement, and it adds
  it for decisions a second process already made. Doing it wrong is worse than not doing it: a fold
  that replayed a hook's deny as the driver's own would put the driver's name on somebody else's
  refusal, and the trail's value is that it says who asked.

  **What closes it**, carried as a row in `docs/plan/gap-register.md`: an `authorize` ingestion that
  preserves the hook as the deciding party, plus the first case where a hook's decision would change
  what the engine does — which does not exist yet, because hooks deny and never grant.

#### Two things the build settled that this section had left implicit

**A step's skills reach the session in the prompt, not on the command line.** The first draft of the
executor passed the step map's `skills:` list to `--agents`, which takes a JSON object of *agent
definitions*: a usage error that fails the whole invocation. The skill is asked for instead, and the
`Skill` tool — the named exemption above, because loading instructions takes no action — is what
answers. Pinned by `a_steps_skills_are_asked_for_in_the_prompt_and_never_passed_as_agent_definitions`
(`crates/protocol-cli/src/drive.rs:1389-1400`), which asserts the flag is absent from the constructed
argv, beside the F15 test that `--bare` never is present.

**A hook is told facts about the state and never the rules.** The channel above needs the hook to
know which run and which state it is adjudicating, and that is a file the driver writes:
`<run-dir>/step-context.json`, format `aep.drive-step-context/1`
(`crates/protocol-cli/src/drive.rs:899-926`), carrying the run directory, the store, the state, the
step index, the attempt, whether a shell is offered at all, the admitted capabilities and the
harness's names for them. Two properties, both deliberate:

* **rewritten before every `llm` step, not once per run.** `effective_policy` grants the state's
  capabilities on top of the plan's, so the legal surface in `implement` is not the legal surface in
  `review`. A run-scoped context would be a per-state rule enforced with per-run facts;
* **it carries no rules.** The surface a shell is held to is declared in the hook that enforces it,
  never here. A run that could name its own allowed surface could widen it, and a widening the run
  authored is a route around the constraint rather than a check on it. It is also what makes the
  `driven-surface` hook **inert outside a driven run**: with no step context on disk it passes
  silently, which is this section's own rule that a plugin installed without the driver ships no
  per-state enforcement.

#### F13, answered — a hook deny does reach `permission_denials`

**Nothing documents it; two real driven runs on Claude Code 2.1.238 settled it, 2026-08-21.** In the
denial session of the second, the three hook refusals — `Bash`, `Edit`, `Write` — produced exactly
**three** `permission_denials` entries, each carrying the tool's name; the honest session's single
refusal produced exactly **one**. So the transcript-side audit of a hook refusal works, one-for-one.

**Row 1's transcript row stays advisory even so**, and the reason is a rule this section already
applies to itself: the row asserts a *model behaviour* — that something forbidden was attempted at
all — on top of an *undocumented harness detail* that can change without notice. A gate that goes red
because a model behaved better than the prompt asked is a gate people learn to ignore. The **gating**
evidence is on disk: the hook-decision log, and `protocol artifact validate`, which catches an
illegal status whether the guard held, was never installed, or was routed around.

One thing the first run got wrong is kept because it is the interesting part. The denial step
originally asked for a hand-edited **`status:`**, and the model did not take the bait — it read the
lifecycle and used `protocol artifact move`, which is the legal route, which the surface hook allows,
and which is exactly what the skill teaches. The prompt had induced *correct* behaviour and the store
guard was never exercised. The target is now `revision:`, which has **no CLI verb at all**, so a hand
edit is the only route to it. **A deliberate-denial case has to ask for something with no legal
alternative, or it measures the model's judgement instead of the guard.**

#### Two assumptions this section makes — one closed by a run, one still open

**Added per F15**, whose point is that this section's own preamble refuses confidently-fonted
guesses and therefore has to hold itself to that. Both were named as unfillable; **one was filled
2026-08-21 by running the thing**, which is the outcome the naming existed to make possible.

* **The trust model for plugin-supplied hooks is undocumented. Still open.** The assumption is *that
  hooks shipped by an installed plugin execute without a per-invocation consent step, and that a user
  who installed the plugin has thereby accepted them.* If that is wrong, or becomes wrong, **the hook
  layer of this table degrades to advisory and the `--allowedTools` layer carries enforcement
  alone.** The driven eval's hooks fired without a consent step, and **that does not close it**:
  running a hook successfully in one install establishes nothing about somebody else's. Naming it
  costs a sentence and not naming it is the failure the preamble describes.
* ~~**Whether a hook's `permissionDecision: deny` increments the transcript's `permission_denials`
  array is unverified.**~~ **Closed 2026-08-21 by the driven eval: yes, one-for-one** — see *F13,
  answered* above. The sentence is struck rather than deleted because an assumption that quietly
  becomes a fact is one nobody can audit the closing of. The hooks reference still says nothing about
  the `result` event, so the answer is an observation about one Claude Code version (2.1.238) and the
  row it supports is kept advisory for that reason.

**Stop hooks are deliberately not used**, and the review is right that it should stay that way: the
hooks reference notes the model overrides after 8 consecutive blocks, and a run-completion rule must
not sit on a bound like that. The driver decides completion from `TransitionResult::Completed`
(`engine.rs:388`), which has none.

#### The honest boundary

**Enforcement is complete over ACTIONS and TRANSITIONS, and over nothing else.**

An action is a tool call, and every tool call either maps to exactly one `Capability` or has no tool
— the guide's table (`docs/guide/harness.md` § 2) is total in that direction. A transition is the
engine's, and the driver has no path around it, because it never evaluates a gate (§ 4.1).

**Text is free.** What the model says, plans, reasons about or claims is not governed, cannot be
governed by a capability, and is not evidence. That is safe because of three mechanisms that are
already in place rather than because of an expectation of good behaviour: an `llm` step cannot carry
an evidence block; a `Producer::Agent` does not satisfy `independent: true`; and a claim about *how*
an agent worked is established by a deterministic checker reading the transcript, never by the agent
reporting on itself (`crates/trace-spec/src/evidence.rs:17-33`).

**Enforce and verify — neither replaces the other.** The allowlist and the hooks are enforcement:
they stop the action before it happens. `trace check` is the audit: it reads the transcript afterwards
and says whether enforcement held. An enforcement mechanism nobody audits is a claim, and an audit
with no enforcement is a report about a horse that has already left. This repository's own standard —
*a rule nothing checks is a rule that has already drifted somewhere* — is what makes running both
mandatory rather than belt-and-braces.

#### What this does to § 3.6's refusal of hooks

§ 3.6 refused hooks in the planning plugin, and the reason it gave was that a hook layer would be *a
second, weaker driver* — one that sees tool calls rather than workflow states and cannot ask the
engine anything because it has no execution to ask about.

**That reason is unchanged, and the refusal is unchanged for a plugin that ships alone.** What
changes at Phase 2 is that the hook stops being a second driver: it is the driver's enforcement arm,
configured *by* the driver from `capabilities()` at each state, and it has an execution to ask about
because the driver is holding one. The plugin's hooks therefore ship in the **driver's** wave, not
the planning plugin's, and a plugin installed without the driver still ships none. Anything else
would be two mechanisms both claiming to enforce the same thing, which is what § 3.6 was avoiding.

### 4.9 The adapter surface — three points, and no more

The driver is the **sample runner**, not the only runner. Every behavioural specification this
repository publishes is a harness-neutral document — a workflow, a profile, a step map, a trace
specification — and a second harness adopts the whole set by implementing exactly **three** adapter
points. Naming them and bounding them at three is the decision; a surface nobody bounded grows a
fourth point the first time somebody finds something awkward.

**Point 1 — invoke the agent: an `LlmStepExecutor` trait in `aep-driver`.**
One method: take a resolved `llm` step (prompt, named skills, tool config) and return a step outcome
carrying an exit status and the path to a transcript. **This trait does not exist yet** — nothing in
the workspace has this shape — and it is named as new rather than dressed up as existing. It lives in
`aep-driver` so the routing core can be exercised against a fake; the Claude Code implementation
lives in `protocol-cli` behind `protocol drive`, because § 4.1 puts the three things that touch the
world outside the pure crate. **Per review F1, `aep-driver` is now the second of two crates** — the
trait and the router live there; the step-map and cursor types live in the leaf `aep-driver-spec`,
for the dependency-cycle reason D1 sets out.

**Point 2 — capabilities → tool config: a pure function, deliberately not a trait.**
`fn tool_config(policy: &CapabilityPolicy) -> ToolConfig` in `aep-driver`: total, clock-free and
consumed by the executor. It produces a **harness-neutral** description — which capabilities are
admitted, with the path and intent constraints that go with them — and the harness adapter renders
that into its own tool names. Two layers, and the split is the point: the *decision* about which
capabilities admit which actions is shared and is the guide's table
(`docs/guide/harness.md` § 2, where every `Action` maps to exactly one `Capability`); only the
*rendering* is harness-specific.

**Its input is the decision, not one of the decision's three inputs. Corrected per review F3:**

```rust
// admits a capability iff policy.decide(&capability) == CapabilityDecision::Allowed
fn tool_config(policy: &CapabilityPolicy) -> ToolConfig
```

Reading `.allow` directly is the bug D3(a) now documents: the three sets are independent, membership
is by `covers` and not equality, and an unscoped `allow` entry covers a scoped `approval_required`
one. Calling `decide` (`crates/aep-domain/src/capability.rs:588-599`) changes nothing about the
argument for a function over a trait — it is still pure, total and clock-free — it just calls the
one function that owns invariant 6's ordering instead of re-deriving it badly.

**Three entries in the rendering table are not functions of a capability, and each is decided
rather than left to an implementer** (per the review's resolution of open cell (c); the reasoning is
in § 4.8):

| tool | decision |
|---|---|
| `Bash` | offered only when `decide(command.execute) == Allowed`; granting it grants a superset of the shell's reach, and narrower gating is best-effort. ~~**No development profile grants it**, so a development `llm` step holds no shell~~ — **true of `development.fast` and `development.standard`, and no longer true of the set: `development.driven` grants it so a driven step can reach the `protocol` CLI at all, held to that surface by a hook.** § 4.8 carries the correction and the two properties that keep it survivable |
| `Skill` | a **named exemption** — it loads instructions and takes no action; what it causes is a subsequent, governed tool call |
| `Task` and the agent-spawning family | **never offered.** A subagent's tool set is derived by nothing here, so it would be a route around the per-state allowlist. Audited by `subagent.spawned: at_most 0` |

**A trait is rejected here on purpose.** Making point 2 a trait method would let a second harness
quietly re-decide that `repository.write` admits a shell, and the protocol would have no way to
notice. A function that every adapter calls, with the naming table as its only per-harness input, is
the narrower seam and the one that keeps a second harness honest.

**Point 3 — the transcript adapter, and a correction about what the seam actually is.**
**There is no adapter trait in `trace-spec`, and this design should not claim one.** What exists is a
neutral IR plus a per-harness free function: `trace_spec::adapter::read_transcript(&[u8]) ->
Result<TraceIr, ValidationErrors>` (`crates/trace-spec/src/adapter.rs:102`), stamping
`CLAUDE_CODE_STREAM_JSON: AdapterRef` (`adapter.rs:90-93`) into `TraceIr::adapter`
(`crates/trace-domain/src/ir.rs:506`, `:516`, `:528`). Everything downstream takes `&TraceIr` and has
never heard of Claude Code: `check` (`crates/trace-spec/src/check.rs:58`), `CheckReport`
(`crates/trace-spec/src/report.rs:435`), `CheckReport::to_evidence`
(`crates/trace-spec/src/evidence.rs:169`).

So the seam is real and load-bearing — it is spelled as a **format** rather than as a trait. Trace
wave 1 says so plainly and says the claim is untested: *"No second adapter. One harness format,
versioned and declared… until there is one the claim is untested"*
(`docs/plan/trace-wave-1-transcript-checker.md:263-265`).

**Decision: do not add a trait to `trace-spec` speculatively.** A second adapter is a second free
function returning `TraceIr`, selected by the driver from the step's harness name. A trait buys
dynamic dispatch nobody needs, and designing it before there is a second implementation to design it
against is the mistake the gap register exists to catch. If wave 3's second adapter shows the
selection is awkward, the trait is a one-file change made **with** evidence rather than for symmetry.

#### The acceptance idea for wave 3: a shell-echo harness

The three points are claims, and a claim about neutrality that only one implementation has ever
tested is the shape of defect this repository writes registers about. So wave 3's acceptance for the
adapter surface is a **second, fake harness** rather than a second real one:

* a second `LlmStepExecutor` that runs a shell script instead of a model — it reads the prompt on
  stdin, writes a fixed set of files, and emits a transcript in a dialect of its own;
* a second transcript reader for that dialect, returning `TraceIr` with its own `AdapterRef`;
* the same step map, the same workflow, the same `tool_config` function, the same checker.

It proves all three points at once: the executor trait has two implementations, the tool-config
function is consumed by both, and `check` / `to_evidence` mint a `trace_conformance` record from a
transcript that no Claude Code wrote. And it does it **with no model, no network and no credential**,
so unlike the paid eval (`docs/plan/harness-wave-1-planning-plugin.md:91-94`) it can be a step of
`task check`. That is the whole value: *"this is harness-neutral"* stops being a sentence in a design
document and becomes a gate that goes red.

It also does not need Codex, or any other real second harness, to exist. When one arrives, it
replaces the fake as a third implementation rather than as the first test of the seam.

**One prerequisite the first draft did not have, added per review F12.** The per-state tool set has
no audit until the trace vocabulary can read the offered tool list. `SessionStart.tools` is already
in the IR (`crates/trace-domain/src/ir.rs:222-223`); what is missing is a **50th expectation kind,
`env.tool_available`**, mirroring the 49th almost line for line — `env.skill_available` is four
lines of dispatch (`crates/trace-spec/src/check.rs:103-107`) and the new kind is the same call
against `start.tools`, plus a `RawExpectationKind` variant, a `NAMES` entry and a name arm. Three
files, and the existing drift test that asserts the raw and validated vocabularies agree
(`crates/trace-domain/src/spec.rs:772-776`) catches a half-done job. It ships **before** the hooks,
because otherwise the allowlist ships with nothing that can audit it and § 4.8's own standard —
*an enforcement mechanism nobody audits is a claim* — is asserted rather than met.

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

**Taken as `drivers/` in the wave-2 update (§ 4.7), and the reason is mechanical rather than
aesthetic.** `load_tree` walks a fixed table of directory-to-document-kind pairs
(`crates/aep-engine/src/load.rs:22-28`), and a step map is a validated, versioned, schema-generated
document exactly like the four already in it. Putting it anywhere else would mean a fifth document
kind loaded by a second mechanism, which is the thing that table exists to prevent. The feasibility
review can still overturn this; what it cannot do is leave it undecided, because D1's load-time
cross-validation has to run somewhere.
