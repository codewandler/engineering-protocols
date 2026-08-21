# Suggested updates to the control documents

Proposals, not changes. Written after four unreviewed designs landed in `docs/design/` on
2026-08-20 and after four reviews of `3647f80`. Each item says what is wrong now, what to write
instead, and why it matters — so the decision can be taken without re-deriving the evidence.

The theme: three documents describe a repository with two halves and one horizon. There are now four
proposed designs that add a third axis and a fourth domain, and none of them is normative. Left alone,
the vision quietly grows to whatever the newest document proposes.

## Applied, 2026-08-20

All twelve. `docs/VISION.md`, `README.md` and `AGENTS.md` carry them; this page is kept as the record
of why, since each item states evidence the documents themselves do not repeat. Where the applied form
differs from what was proposed here, the difference is recorded below.

| item | applied where | difference from the proposal |
|---|---|---|
| V-1 | `docs/VISION.md` § The thesis | the corrected sentence, then the gap named as a **gap**: attested evidence is a real missing feature, and no plan document proposes it |
| V-2 | `docs/VISION.md` § Where this stands | rewritten against `git tag -n99`; one row per delivered ESS crate |
| V-3 | `docs/VISION.md` § Proposed, not accepted | sequencing settled, not left open: wave 4 first, semantic diff after it, the other two unsequenced |
| V-4 | `docs/VISION.md` § What this is deliberately not | the boundary is *generating an artifact* versus *operating a system*, not the vaguer "does not deploy" |
| R-1 | `README.md` § Documents | the reviews are **five**, not four — four at `3647f80` plus a full-repository review at `95e210f`. One row covers `docs/reviews/` rather than five rows, so the fifth is findable too |
| R-2 | `README.md` § ESS status table | the gate page's own table lists 18 gates with 14 closed; its summary line says 19 and 15. The table was taken as authoritative and the four open gates are named |
| R-3 | `README.md` § Status | one paragraph, pointing at the vision section rather than restating it |
| A-1 | `AGENTS.md` § Which documents are normative | new section; names the four proposals, and says what acceptance looked like for the ESS implementor design. **Two of the four are unreviewed, not three**: `docs/reviews/2026-08-20-next-waves-feasibility-review.md:1-7` takes the structural-synthesis design as a subject and finds a blocking model gap in it, so that one is *reviewed and unreconciled*. Semantic diff and infrastructure are named in no review at all |
| A-2 | `AGENTS.md` § Invariants | a line per invariant. Enforced by a lint or gate step 3, by a type 4, by a test 4, by a scan 2, by **nothing** 3 |
| A-3 | `AGENTS.md` § Conventions | both conventions, each pointing at the test that demonstrates it |
| A-4 | `AGENTS.md` § Dependencies | new section. Nine direct third-party crates, not seven: seven in `[workspace.dependencies]`, plus `sha2` and `jsonschema` |
| A-5 | `AGENTS.md` § Gate | six steps, listed in order, with what each one turns from a warning into a failure |

Not changed, and worth knowing: `docs/plan/ess-wave-3.5-reconciliation.md` disagrees with itself about
how many gates there are (19 in its summary, 18 in its table). That is one line to fix on a page
outside this proposal's scope.

## Applied, 2026-08-21

A second round, and a different cause. The first round was written because four unreviewed designs
had landed and the control documents did not say so. This one is written because a decision was
**taken** — in session, by the operator, on 2026-08-21 — and one of the three control documents
states the opposite of it. `docs/VISION.md` refuses *a workflow engine*, and the repository is now
going to ship a reference driver. A refusal that a decision has overtaken is worse than a missing
entry: it reads as current policy, and the next agent enforces it.

All four applied the same day they were proposed, because the decision they record was taken before
they were written. The proposals are kept below for the same reason the first round's are: each
states evidence the documents themselves do not repeat.

| item | applied where | difference from the proposal |
|---|---|---|
| V-5 | `docs/VISION.md` § What this is deliberately not | applied as proposed. The narrowing is written *inside* the refusal list rather than as a new section, so the reader who came to that section for the boundary finds the amended boundary and not a cross-reference to it. The argument follows as its own paragraph, and it names what did **not** move — invariant 7, engine-only gate evaluation, and the "external systems do the work" sentence |
| V-6 | `docs/VISION.md` § Proposed, not accepted | the table's preamble said *"Four design documents"* and now says five, because a count in a preamble is the first thing to go stale. Status is split rather than single-valued: **Phase 1 accepted** as harness wave 1, **driver decided and sketched, build unsequenced** — one word would have hidden the boundary the design document draws inside itself |
| R-4 | `README.md` § Status, § Documents, § What works today, § Repository layout | three touches, not one. The design doc **and its plan page** get rows, since a plan page is where a reader checks what was actually accepted; the `protocol artifact` verbs and the plugin get one bullet each where the README already lists what works; `integrations/` and the new crate get lines in the layout tree, since a directory nothing mentions is a directory nobody finds; and § Status's count of proposed designs moves four→five, the same stale-count correction V-6 makes in the vision |
| A-6 | `AGENTS.md` § Which documents are normative, § Current state | the acceptance row as proposed, **plus** the "Not built yet" line, which said *"any durable backend — the only implementation of the contract is in memory"*. Half of that is now false and half is still true, and the two halves are worth separating: a durable **store** exists; the **contract** still has exactly one implementor until P3 |

---

## `docs/VISION.md`

### V-1 — the thesis sentence is false, and it is the load-bearing one (high) — **applied**

`docs/VISION.md:97`: *"Nothing in the loop asks anyone to be trusted."*

It is not true today. Independence is a single boolean over a self-declared enum
(`crates/aep-domain/src/requirement.rs:254`); nothing binds a verifier's identity to its output, no
submitter is recorded on evidence (`crates/aep-engine/src/engine.rs:50-65` has no such field), and
`docs/guide/harness.md:112` admits it. Grep finds no attestation, no signature, no key.

This is the sentence the whole document turns on, so it should say what is actually mechanised:

> The model reasons. The protocol constrains. The specification defines. The verifiers establish
> facts. What the loop asks you to trust is narrow and named: that a producer declaring itself
> independent is. Everything downstream of that declaration is checked.

Then either accept that as the resting state or file the gap. The honest version costs one sentence;
the false version costs the reader's trust in every other claim on the page.

### V-2 — "Where this stands" is two waves stale (high) — **applied**

It is honest "as of `0.2.1`" and says **ESS — everything: specified, not built**. Since then ESS
shipped its model, its compiler and four projections across three tagged waves, and the table's stated
next milestone — "parsing and validating one small system end to end" — was wave 1. Numbers: 442 tests
there, 953 now.

Rewrite the table against `git tag -n99`, and replace the ESS row with the three delivered crates. Keep
the AEP row that says the honest milestone is *a team whose work it actually governs* — that one is
still true, and it is the most valuable sentence in the section precisely because it has not moved.

### V-3 — the vision has two halves; the backlog now proposes four (high) — **applied**

`docs/design/` holds four proposed designs, none reviewed against the code, none referenced by the
vision:

| design | what it adds to the vision |
|---|---|
| closed-loop conformance (wave 4) | the specification becomes an *oracle* — a verdict, not just a projection |
| structural synthesis and obligations (wave 5) | generated applications, and human/agent work as typed obligations |
| semantic diff and evolution | a **third axis**: change over time, impact closure, invalidation |
| infrastructure discovery and multi-cloud | a **fourth domain**: infrastructure, `InfraSpec`/`InfraIr` beside the ESS pair |

The first two are horizons the vision already implies. The last two are not: semantic diff is about the
system *changing*, which "specified once and compiled" does not cover, and infrastructure is a second
subject matter entirely.

Add a short section — *"Proposed, not accepted"* — listing all four with one line each and their status.
It costs a paragraph and it stops the vision from silently becoming whatever was dropped in last. The
alternative, absorbing them, should be a deliberate act with a reason, not the default.

### V-4 — one boundary is now contested (medium) — **applied**

*"What this is deliberately not"* says: not a deployment platform. The infrastructure design proposes
multi-cloud realization, which is adjacent enough that the boundary needs restating rather than
repeating — presumably: this project decides what an infrastructure's observed state *permits*, and
still does not deploy anything. If that is not the answer, the "deliberately not" list is out of date
and should say so.

### V-5 — the refusal of "a workflow engine" is now contradicted by a decision (high) — **applied**

`docs/VISION.md` § *What this is deliberately not*, first sentence: *"Not an LLM orchestration
framework, a CI system, an incident-management product, **a workflow engine**, a message broker, or a
policy language meant to replace OPA."*

On 2026-08-21 the operator decided that the repository ships a reference driver: the first in-repo
implementation of the harness contract, designed in
[`harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md)
§ 4. The refusal as written forbids it, and an agent reading that line is right to refuse the work.
The refusal is not deleted — it is **narrowed**, and the narrowing is marked as one, so a reader can
see that a boundary moved rather than finding a boundary that quietly differs from last month's.

Replacement text for the paragraph:

> Not an LLM orchestration framework, a CI system, an incident-management product, a message broker,
> or a policy language meant to replace OPA. Not a universal ontology of software engineering, and
> not a mandate for microservices, CQRS or event sourcing. One earlier entry in this list has been
> narrowed, deliberately: this is still not *a workflow engine* in the sense refused before — a
> general-purpose orchestrator other systems are built on — but the repository now ships one
> **reference driver**, a default harness that walks its own workflows by asking the engine the seven
> questions and doing, outside the engine, only what the answers permit.

And the argument, which is the part that has to be written down, because a narrowed refusal with no
reason beside it is indistinguishable from a refusal somebody forgot:

> The harness contract — seven calls, three rules — is published in
> [`docs/guide/harness.md`](../guide/harness.md) and mandated by the normative consolidated design,
> and it is implemented by no program in this repository. A contract with zero implementations is the
> same defect as an invariant enforced by nothing. The driver is that implementation, and the line it
> does not move is the one that mattered: the engine still evaluates and never acts (invariant 7
> unchanged — an agent's own statement never satisfies an independence requirement), gates are still
> evaluated by the engine and never by the driver, and *"external systems do the work; this project
> decides what the results permit"* remains true — the driver is the first of those external systems,
> kept in-tree the way the website is: a deliverable beside the specification, consuming only its
> public surface. Decided by the operator, 2026-08-21; designed, not yet built — building it waits
> behind its own reviewed design.

The cost of not applying this: the vision refuses what the roadmap builds, and the two are read by
different people.

### V-6 — a fifth design document, and the table that exists to stop exactly this (medium) — **applied**

V-3 added § *Proposed, not accepted* so that reading the newest file in `docs/design/` could not be
mistaken for reading what the project agreed to build. A fifth design document now exists and the
table does not list it — which is the failure V-3 was written to prevent, one round later.

Add the row, with a status that says both halves, because this document is the first in the
directory whose acceptance boundary runs *inside* it:

> **Phase 1 accepted as harness wave 1; driver decided and sketched, build unsequenced.**

And correct the preamble's count from four to five.

---

## `README.md`

### R-1 — nine documents exist and none is in the table (high) — **applied**

The document table lists neither the four proposed designs, nor the four reviews, nor
`docs/plan/ess-wave-3.5-reconciliation.md`. A reader arriving at the repository cannot find the review
that says wave 4 is conditionally blocked, or the gate page that lists why.

Add them, and mark status in the row rather than in a separate section: `proposed, unreviewed` for the
four designs, `snapshot, not maintained` for the reviews. A review that looks like current guidance is
worse than one nobody reads.

### R-2 — the status table has no row for what is actually happening (medium) — **applied**

`ESS wave 3.5 — reconciliation` is where the work is: 15 of 19 gates closed, 4 model changes left, and
wave 4 does not start until they land. The table shows waves 1–5 and skips the one in progress.

### R-3 — the two-halves framing needs its third axis (low, follows V-3) — **applied**

`README.md:34-36` says "this repository has two halves". Same issue as V-3, same fix, one line.

### R-4 — the README lists what works, and the newest thing that works is not on it (medium) — **applied**

Two omissions, and they are the two things a reader arriving today would most plausibly be looking
for:

* § *Documents* has a row for every design document in `docs/design/` and none for
  `harness-planning-and-driver-design-v0.1.md`. R-1's argument applies unchanged — a design a reader
  cannot find is a design that gets re-proposed.
* § *What works today* is the list of what a person can actually run, and `protocol artifact` and the
  Claude Code plugin are runnable. A capability list that omits the newest capability is a list
  people stop trusting.

Add: the design-doc row, with its split status; one bullet for the `protocol artifact` verb family
naming the behaviour rather than the flags — lifecycle-validated moves whose refusals name the legal
set, `validate` accumulating, frontmatter private to the backend; and one bullet for the plugin —
one skill, two agents, no hooks on purpose. Keep it to a few lines: this is a status list, not the
design document.

---

## `AGENTS.md`

### A-1 — "the authoritative specification" is now ambiguous (high) — **applied**

`AGENTS.md:9-13` names `consolidated-design-v0.2.md` authoritative and says the document wins over the
code. There are now four *newer*, larger design documents in the same directory, and the rule does not
say they are proposals. An agent reading the newest file in `docs/design/` and implementing it is
following the letter of this page.

State precedence explicitly: normative documents are the consolidated design plus the reconciliation
register; everything else in `docs/design/` is proposed until a plan page accepts it. Name the four.

### A-2 — record which invariants are machine-checked (high) — **applied**

The guard-efficacy review's finding was not that a rule was wrong but that **a rule with no fixture
reaching the state where it is load-bearing is untested** — `ApprovalDecision::Denied` appeared in zero
tests, and `deny_beats_approval_which_beats_allow` never put one capability in both lists. Sixteen
invariants are stated as prose; some are now enforced mechanically (invariant 2 by a source scan,
invariant 9 by a banned-token scan in one crate only), most are not.

Add a column, or a line per invariant: *enforced by* — a scan, a test, a type, or nothing. The ones
that say "nothing" are the next mutation review's target list, and knowing which they are is most of
the value.

### A-3 — add the convention that catches this class (high) — **applied**

Under § Conventions, beside "every test asserts a reason":

> **A test must reach the state where the rule is load-bearing.** A precedence rule needs a fixture
> that populates both sides; a refusal rule needs a refusal in the fixture. A test that passes whether
> or not the rule holds is not a test of the rule, whatever its name says.
>
> **Verify a guard by breaking it.** Before trusting a new test, apply the one-line mutation it is
> meant to catch, watch it fail with a message that names the defect, and revert.

Both are practices this session used to find real defects; neither is written down.

### A-4 — the dependency policy exists but is unwritten (medium) — **applied**

There is no written rule, yet the standard is visibly high: seven third-party crates workspace-wide,
every non-workspace dependency carrying a paragraph of justification (`crates/ess-gen/Cargo.toml:20-26`
is the model — what it buys, which features are dropped, why that is safe here, why the version matches),
and two recorded cases of *avoiding* a dependency on purpose. Nothing in `task check` reaches the
network, and that is a property worth stating rather than rediscovering.

Write it down as it is already practised. An unwritten standard is one the next agent meets only by
violating it.

### A-5 — the gate is six steps (low) — **applied**

§ Gate now says five. `doc-check` was added when the rustdoc backlog reached zero.

### A-6 — the acceptance table has no row for the fifth design, and one honest limit is now half false (medium) — **applied**

Two lines, in two sections, both of which A-1 and the "Current state" convention already oblige.

**§ Which documents are normative.** The table of proposed designs and their acceptance state is what
A-1 added so that "do not implement from an unreviewed design" has something to check against. It
needs a row:

| proposed design | status |
|---|---|
| `harness-planning-and-driver-design-v0.1.md` | **Phase 1 accepted** by `docs/plan/harness-wave-1-planning-plugin.md` (harness wave 1); its Phase 2 reference driver is **decided** (vision V-5) and **not accepted for build** — a feasibility review comes first |

**§ Current state, "Not built yet".** It says: *"any durable backend — the only implementation of the
contract is in memory."* After this wave that sentence is half false, and the false half is the half
that matters to somebody choosing whether to adopt. Split it:

> a durable markdown **store** now exists (`aep-backend-markdown`, harness wave 1) — but it is a
> store, not a contract implementation, and the **contract** still has exactly one implementor,
> `aep-backend-memory`, until the journal-backed milestone (P3) makes the store answer as one.

Both halves stated is the point. "There is a durable backend now" would be a claim the conformance
suites do not support, and "no durable backend" stops being true the day the store lands.
