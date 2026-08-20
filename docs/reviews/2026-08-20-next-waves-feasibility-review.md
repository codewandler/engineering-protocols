# Feasibility review — the wave 4 and wave 5 designs against the code that exists

Reviewed at `3647f80` (`main`, CI green, 916 tests). Subjects:
[`ess-closed-loop-execution-conformance-design-v0.1.md`](../design/ess-closed-loop-execution-conformance-design-v0.1.md)
(proposed wave 4, 2180 lines) and
[`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](../design/ess-structural-synthesis-obligations-realizations-design-v0.1.md)
(proposed wave 5, 1977 lines), both landed unreviewed in `3647f80`. This review answers the
reconciliation the wave-4 header asks for: *"Reconcile names with the actual local API rather than
creating duplicate abstractions."* Read-only; nothing outside this file was changed.

Section numbers prefixed **W4** and **W5** refer to the two documents. Everything else carries a
`file:line` or a command's output. Where I could not verify a claim, it is labelled.

---

## Verdict

**Both documents are buildable here in the order they propose, and neither can meet its own
acceptance criteria without a model change that appears in neither.** Nothing in the model connects a
command to a lifecycle transition — `Transition` is `{name, from, to}` with no command field
(`crates/ess-domain/src/entity.rs:157`) and `ResolvedCommand` is `{name, domain, input, outcomes,
naming}` with no entity or transition field (`crates/ess-compiler/src/ir.rs:501`). The billing
fixture proves the consequence rather than merely implying it: three declared transitions (`issue`,
`settle`, `cancel`, `examples/billing/domains/invoice.yaml:101`) and one command in the domain
(`CreateInvoice`, `:131`), and **no command drives any transition**. So every invoice a conformant
implementation can create is `Draft`; W4 §19's lifecycle scenarios have no arrow to walk, W4 §25's
`F-ILLEGAL-TRANSITION` fault has no command to attempt, the only `read_your_writes` view
(`OutstandingInvoices`, filter `state == Issued`, `:180`) is provably always empty so
`F-VIEW-RACE` is undetectable, and W5 §6's own worked example (`IssueInvoice`, `PayInvoice`,
`CancelInvoice`) names three commands that do not exist. Three of W4 §50's seven falsifiability
criteria are unmeetable as written. That is one model change, in wave-1 territory, and it blocks both
waves.

Separately, W4 reinvents five things that already exist under the same or a near name — most sharply
`ConsistencyToken` and `QueryConsistency::{Current, AtLeast}`, which sit at
`crates/aep-contract/src/consistency.rs:30` and `:76` with W4 §14's own argument already written in
the module doc — and one section (W5 §28) proposes a capability-grant path that invariant 6 exists to
forbid and `CapabilityPolicy::restrict` mechanically refuses
(`crates/aep-domain/src/capability.rs:626`). The duplicates are cheap: an hour each, and the fix is
`use` rather than `pub struct`. The good news is larger than expected in one place — W4 §33 and §49
steps 9–10 ("the first real ADP task", "a real protocol completion test") **already exist and pass**
at `crates/aep-engine/tests/end_to_end.rs:245`, so the loop's protocol half is a ~50-line conversion,
not a milestone.

On scope: wave 4 as written is **three waves**. Measured against the last three (ESS 1: 16,096
insertions / +200 tests; ESS 2: 10,661 / +135; ESS 3: 14,283 / +139 — `git show --stat`,
`git tag -n99`), each prior wave did exactly one *kind* of thing. Wave 4 does three: a projection, an
execution layer with a new dependency class, and an integration. Only the first resembles wave 3. The
seams are clean and W4 §49's own step order already falls on them.

Wave 5's ordering after wave 4 is correct and its reuse direction (§W5.9: "run the existing ESS
conformance suite unchanged") is right. But the wave-5 *document* is four waves: `docs/plan/ess-roadmap.md:199`
defines wave 5 as W5.1–W5.3, and W5 §40 already exceeds that before §41–§43 add an agent loop, a
`Realization` model and five more.

Assessed separately, against the decision to pursue `proptest`: **W4 §11's refusal-first stance is
right and is the reason to keep it, but the section is written two-valued against a three-valued
evaluator** (`Predicate::evaluate -> Truth`, `crates/aep-domain/src/predicate.rs:345`), and the
input→`FactSource` projection it presupposes exists nowhere. A seeded generator in
`[dev-dependencies]` is plainly outside invariants 8 and 9. Full assessment in P1.

---

## Findings, by severity

| # | severity | finding | proposed | real |
|---|---|---|---|---|
| C1 | **critical** | no command↔transition link; W4 lifecycle scenarios and W5 state-safe synthesis are not derivable | W4 §19, §25, §50; W5 §6, §7, §W5.3 | `entity.rs:157`, `ir.rs:501`, `invoice.yaml:101` |
| C2 | **critical** | no command↔entity link; invariant scenarios cannot say which entity changed | W4 §20 | `ir.rs:501`, `ir.rs:540` |
| C3 | **critical** | the only `read_your_writes` view is unreachable, so `F-VIEW-RACE` is undetectable | W4 §14, §25, §50 | `invoice.yaml:180` |
| C4 | high | a view has no parameters and reaches no contract; `SemanticViewRequest` has nothing to select with | W4 §14, §21, §29 | `view.rs:180`, `generated/openapi/invoice-service.yaml:24` |
| C5 | high | `Failure::Escalate` has no observable representation, and it is the fixture's only failure mode | W4 §18, §50 | `binding.rs:164`, `components.yaml:105` |
| H1 | high / 1h | `ConsistencyToken` + `QueryConsistency` already exist verbatim, with the same rationale | W4 §14 | `aep-contract/src/consistency.rs:30`, `:76` |
| H2 | high / 1h | `Fault`, `Fault::caught_by`, and the whole fault-matrix meta-test already exist | W4 §25, §26 | `faulty.rs:35`, `:88`, `tests/faults.rs:55` |
| H3 | high / 1h | `SuiteProvenance` duplicates `Provenance`; only `suite_version` is new | W4 §23, §21, §30 | `ess-gen/src/provenance.rs:13` |
| H4 | high / 1h | `ConformanceReport` collides; `CheckResult` duplicates `Check` | W4 §27, §28 | `aep-conformance/src/report.rs:204`, `:12` |
| H5 | high | the runner needs an async runtime and a clock; the workspace has neither, and §15 contradicts §40 | W4 §7, §14, §15, §28, §30, §40 | `aep-contract/src/testing.rs:22`, `aep-engine/src/clock.rs:12` |
| H6 | high | putting the runner in `ess-gen` breaks two properties that crate states about itself | W4 §35, §36, §52 | `ess-gen/src/artifact.rs:44`, `ess-gen/src/lib.rs:24` |
| H7 | high (process) | the step-list silently replaces the authoritative §18 scenario format the model was built for | W4 §21 | `view.rs:105`, `ess-roadmap.md:172` |
| H8 | high | obligation kinds granting capabilities is a second grant path invariant 6 forbids | W5 §28 | `capability.rs:626`, `:10`, `AGENTS.md:42` |
| M1 | medium | W4's own witness `amount = -1` is refused by `Money`'s invariant before the command sees it | W4 §9 | `invoice.yaml:28` |
| M2 | medium | the model has no `examples:` construct, so two of §11's four witness strategies have no source | W4 §11 | `ess-domain/src/*.rs` (absent), `ir.rs:59` |
| M3 | medium | `TestStrategy` and `AssertionStyle` already decide what §11/§12/§14 re-derive | W4 §11, §12, §14 | `command.rs:233`, `view.rs:113`, `ir.rs:488` |
| M4 | medium (good news) | §33 and §49 steps 9–10 already exist as a passing test | W4 §33, §49 | `aep-engine/tests/end_to_end.rs:245` |
| M5 | medium | `EssConformanceResult` is `deny_unknown_fields` with no digest; §31's digests are a model change | W4 §31 | `evidence.rs:673` |
| M6 | medium | invariant 7 is honoured, but the independence *claim* is unenforced and neither design closes it | W4 §32 | `end_to_end.rs:80`, `ess-conformance.yaml:35` |
| M7 | — (correct) | determinism: §37 matches invariant 9 and the mechanism already exists | W4 §37, §38 | `ir.rs:878`, `xtask/src/main.rs:9`, `ci.yml:65` |
| M8 | medium | wave-5 ordering is right; both waves blocked on C1; the wave-5 document is four waves | W5 header, §40–§43 | `ess-roadmap.md:199` |
| M9 | medium | wave 4 as written is three waves | W4 §4, §49 | `git tag -n99`, `git show --stat` |
| L1 | low | `502` for an `external` outcome will read as an infrastructure error to a naive adapter | W4 §9 | `CHANGELOG.md` §`[0.3.2-ess-wave-3]` |
| L2 | low (improvement) | `ScenarioStatus::Unsupported` is better than `SuiteReport::aborted`; record the divergence | W4 §28 | `report.rs:96` |
| L3 | low (correct) | W5 §16 reuses `EntityLocator` correctly | W5 §16 | `aep-domain/src/locate.rs` |
| L4 | low | `ArtifactKind::ImplementationObligation` is cheap but touches the schema gate; `Other` works today | W5 §22, §41 | `artifact.rs:385`, `:387` |
| L5 | low (no collision) | the CLI verbs W4 §34 suggests match what shipped | W4 §34 | `protocol-cli/src/main.rs:514` |
| P1 | **high** | §11 is written two-valued against a three-valued evaluator, and the input→`FactSource` projection it needs does not exist | W4 §11 | `predicate.rs:345`, `facts.rs:119`, `facts.rs:669` |

---

## C1 — Nothing connects a command to a lifecycle transition (critical, structural)

**Verified.** A transition carries a name and states, and nothing else:

```rust
pub struct Transition {
    pub name: String,              // "Its own name, such as `IssueInvoice`"
    pub from: BTreeSet<StateName>,
    pub to: StateName,
}
```

`crates/ess-domain/src/entity.rs:157`. And a resolved command carries no entity and no transition:
`ResolvedCommand { name, domain, input, outcomes, naming }`, `crates/ess-compiler/src/ir.rs:501`.

The fixture makes this concrete rather than theoretical. `examples/billing/domains/invoice.yaml:101`
declares three transitions — `issue` (`Draft → Issued`), `settle` (`Issued → Paid`), `cancel`
(`Draft|Issued → Cancelled`). `:131` declares the domain's only command, `CreateInvoice`. There is no
`IssueInvoice`, no `PayInvoice`, no `CancelInvoice`. **No command drives any transition**, so every
invoice a conformant implementation can produce sits in `Draft` (`lifecycle.initial: Draft`, `:98`)
forever.

What this invalidates:

| asked for | why it cannot be generated |
|---|---|
| W4 §19 legal transitions: "state A / command C → state B" | the IR has no `C` for any transition |
| W4 §19 illegal transitions: "state Paid, `CancelInvoice` → must not reach Cancelled" | `CancelInvoice` does not exist; and the IR could not say which command a state refuses |
| W4 §25 `F-ILLEGAL-TRANSITION` ("paid invoice can be cancelled") | no command to invoke, and no way to reach `Paid` |
| W4 §50 "Lifecycle transitions produce positive and negative checks" | unmeetable |
| W5 §6 typed transition APIs (`Invoice<Draft>::issue`) | the generator cannot know which command calls `issue` |
| W5 §W5.3 "legal transition APIs" | same |

The absence-is-illegality semantics are **right** and deliberately so — `entity.rs:253`: *"There is
deliberately no `forbids` counterpart. A move nothing declares is already impossible."* What is
missing is the *command* end of the arrow, not a prohibition. Both designs assume it is there; neither
proposes it.

Fix, and it is a model change of wave-1 shape: a `transition:` field on a command outcome, plus an
`entity:`/`affects:` field on a command so the transition can be validated against the right
lifecycle. That is the single highest-value change in either document, and it must land **before**
wave 4, not inside it.

## C2 — A command declares no entity, so invariant scenarios have no subject (critical)

**Verified.** Neither `ResolvedCommand` (`ir.rs:501`) nor `ResolvedEvent` (`ir.rs:540`) references an
entity. In the fixture, `InvoiceCreated.invoice_id: billing.invoice.InvoiceId` (`invoice.yaml:160`)
and `Invoice.identity {name: invoice_id, type: InvoiceId}` (`:66`) agree by **name and type only** —
a coincidence the model does not assert.

W4 §20 asks the runner to "evaluate invariants after successful state-changing commands". The IR
cannot say which command is state-changing, nor which entity it changed. And the read-your-writes
chain in §14 (command → token → view assertion) needs to know that the `invoice_id` in the event
identifies the entity the view now projects.

Severity note: this is smaller than C1 because the repository has an accepted pattern for it. Wave 3
faced the identical problem for HTTP paths and chose a **stated** convention, written into the
generated artifact (`docs/plan/ess-wave-3-projections.md:138`: *"a stated convention, not a silent
one"*). Matching an event field to an entity identity by name and type, stated in the suite's
provenance and refusing when the match is ambiguous, is acceptable by that precedent. Inventing it
silently is the named failure.

## C3 — The only read-your-writes view is unreachable (critical)

**Verified.** `examples/billing/domains/invoice.yaml:180`:

```yaml
- name: billing.invoice.OutstandingInvoices
  consistency: read_your_writes
  filter: state == Issued
```

By C1, nothing moves an invoice to `Issued`. So for any conformant implementation this view is
provably always empty — and a target that ignores the consistency token satisfies an
always-empty assertion exactly as well as one that honours it.

Consequences:

- W4 §14's read-your-writes flow ("command → token T → query view `AtLeast(T)` → assert immediately")
  has nothing to assert.
- W4 §25's `F-VIEW-RACE` ("adapter returns stale view despite `AtLeast(token)`") is **undetectable**.
- W4 §50 "Both view consistency modes produce correct runner semantics" — the `eventual` mode
  (`InvoiceById`, `:168`) works; the read-your-writes mode cannot be exercised.

Fix: grow the fixture, which is C1's fix. Cost is the fixture plus regenerating all 27 committed
artifacts and re-passing `tests/agreement.rs`.

## C4 — A view has no parameters, and reaches no contract (high, structural)

**Verified.** `ViewSpec { name, source, fields, filter, consistency, naming }` —
`crates/ess-domain/src/view.rs:180`. No arguments, no key, no selector beyond a static `filter`
predicate over the source entity.

And `InvoiceById` (`invoice.yaml:168`) has **no filter at all**. Despite the name it means "every
invoice, projecting `invoice_id` and `total`". "by id" is a word in the name and nothing else.

The generated OpenAPI confirms views have no wire surface: `generated/openapi/invoice-service.yaml:24`
declares exactly one path, `/invoices/commands/create-invoice`, and no view endpoint. Wave 3 recorded
this as next-wave work (`docs/plan/ess-wave-3-projections.md:123`: *"a view is a read model an
OpenAPI document could expose"*).

W4 §21's `QueryView(...)` / `ExpectView(...)` steps and §29's diagnostic shape read as though a view
query takes an argument. It cannot. This is **small** if the intended semantics are "the query returns
the set and the runner filters it" — implementable today, and it makes C2's name-matching convention
load-bearing. It is **structural** if the design wants `InvoiceById(id)`, which needs a view-parameter
construct the specification language lacks. The design does not say which it means.

Also: W4 §42's "contract conformance" half has nothing to check for views, since no contract mentions
them.

## C5 — `escalate` has no observable representation, and it is the fixture's only failure mode (high)

**Verified.** `crates/ess-domain/src/binding.rs:164`:

```rust
pub enum Failure {
    Retry,     // "Try again, on whatever schedule the transport provides."
    Escalate,  // "Surface it to a person."
    Drop,      // "Give up silently."
}
```

The fixture's only binding is `on_failure: escalate` (`examples/billing/components.yaml:105`).
"Surface it to a person" is not an observation a runner can make.

W4 §18 is **correct** about this and says so: *"If the ESS does not yet define an observable
representation of `escalate`, scenario synthesis must refuse that check rather than invent one."* The
consequence it does not draw is that this refusal fires for the *only* binding in the *only* fixture,
so W4 §50's "The billing binding produces an executable cross-domain scenario" covers the success path
only, and the binding-failure-semantics check ships as a refusal.

Related, and in the design's favour: `Delivery` has exactly one variant (`AtLeastOnce`,
`binding.rs:151`), so W4 §17's warning about not accidentally asserting exactly-once is right and
currently unexercisable — there is no second guarantee to distinguish it from.

## H1 — `ConsistencyToken` and `QueryConsistency` already exist, verbatim (high, one hour)

This is the exact failure the wave-4 header warns about, and the sharpest instance of it.

W4 §14 proposes:

```rust
pub struct ConsistencyToken(String);
pub enum ViewConsistency { Current, AtLeast(ConsistencyToken) }
```

`crates/aep-contract/src/consistency.rs:30` and `:76` already have:

```rust
pub struct ConsistencyToken(String);                      // :30
pub enum QueryConsistency { Current, AtLeast { token } }  // :76
```

And the module doc at `consistency.rs:1-8` is W4 §14's and §40's own argument, already written:

> A conformance suite cannot sleep. If it could, it would be testing the machine it runs on rather
> than the implementation, and the first slow CI box would turn a correct backend red. So every
> accepted mutation returns an opaque `ConsistencyToken`, and a query may demand a view no older than
> that token. An immediately consistent backend satisfies it for free; a projected one blocks until
> its projection catches up. Neither has to say which it is.

The working implementation of the wait is `crates/aep-backend-memory/src/store.rs:80`
(`Store::has_reached`), and the suite that proves it bites is
`crates/aep-conformance/src/suites/consistency.rs`, whose fault is
`Fault::AnswerStaleReads → "consistency"` (`faulty.rs:88`) — i.e. W4 §25's `F-VIEW-RACE` row, already
built and already falsified once.

Additional collision: `ViewConsistency` would be a **third** name in a two-concept space.
`ess_domain::view::Consistency { ReadYourWrites, Eventual }` already exists and means something
different — what the specification *declares*, not what a query *demands*.

Cost if unreconciled: two token types and an adapter converting between them. Cost to fix: `use
aep_contract::consistency::{ConsistencyToken, QueryConsistency};`.

## H2 — The fault matrix W4 §25/§26 proposes is already built (high, one hour)

**Verified.** `crates/aep-conformance/src/faulty.rs`:

- `pub enum Fault` with 15 variants, `#[non_exhaustive]` — `:35`
- `Fault::ALL` — `:67`
- `Fault::caught_by(self) -> &'static str` — `:88`, the fault→suite map W4 §25 calls "the important
  invariant"
- `Fault::describe(self) -> &'static str` — `:108`
- `FaultyBackend`, wrapping a working backend and perturbing only what goes in and out, "the same
  position a real backend's clients are in" (`faulty.rs:8`)

And the meta-tests W4 §26 describes as "roughly equivalent to" already exist by name in
`crates/aep-conformance/tests/faults.rs`:

| W4 §26 asks for | exists |
|---|---|
| "correct implementation → all scenarios pass" | `the_reference_backend_passes_every_level` `:22` |
| "fault X → scenario S fails" | `each_fault_is_caught_by_the_suite_that_exists_to_catch_it` `:55` |
| "unrelated core scenarios still pass" | `a_fault_does_not_simply_break_everything` `:78` |
| (not asked for, and it should be) | `every_suite_checks_something` `:37`, asserting `len() >= 4` |

W4 §25's Option A vs Option B deliberation is settled by precedent: Option B (one target, injected
fault) is what shipped, and W4 §25 already prefers it. Reuse the *pattern*; do not reuse the *name* in
the same workspace without a module qualifier, because `aep_conformance::Fault` is re-exported at the
crate root (`aep-conformance/src/lib.rs:34`).

Note also that this whole section is roadmap W4.2 and review F10
(`docs/design/ess-review-v0.1.md:176`) restated at length, not a new proposal.

## H3 — `SuiteProvenance` duplicates `Provenance` (high, one hour)

W4 §23 proposes `SuiteProvenance { spec_version, spec_digest, compiler_version, generator_version,
suite_version }`. `crates/ess-gen/src/provenance.rs:13` already has:

```rust
pub struct Provenance {
    pub system: String,
    pub specification_version: String,
    pub source_digest: String,        // digest of the resolved IR, not the source text
    pub compiler_version: &'static str,
    pub generator_version: &'static str,
}
```

derived by `Provenance::of(ir)` (`:38`) and emitted by every generator today. `spec_digest` is
`source_digest`; only `suite_version` is new. W4 §21's `SpecificationIdentity` and §30's spec/compiler/
generator fields are the same struct again.

One real caveat if the digest becomes load-bearing: `provenance.rs:113` states the digest is
"truncated to 16 hex characters: this is for telling two models apart in a comment header, not for
resisting an adversary". W4 §31 wants a spec digest inside AEP evidence a completion decision rests
on. That is a different requirement from a comment header, and widening it is a `generated/` rewrite
(the digest appears in all 27 artifacts).

## H4 — `ConformanceReport` collides; `CheckResult` duplicates `Check` (high, one hour)

`crates/aep-conformance/src/report.rs` has `Check { name, passed, detail }` `:12`, `SuiteReport` `:66`
and `pub struct ConformanceReport { level, suites }` `:204`. W4 §27's illustrative
`run_suite(...) -> ConformanceReport` collides with the last of these in the same workspace, meaning a
different thing. `EssConformanceReport` (§30) does not collide and is the right spelling.

W4 §28's four statuses are a genuine **improvement** over what exists — `SuiteReport::aborted`
(`report.rs:96`) folds "the suite could not even ask the question" into a failed `Check`, and W4 §28's
`unsupported`/`error` split plus the rule "an `unsupported` required scenario makes overall
conformance fail; do not silently skip required semantics" is better. State it as a deliberate
divergence so the two report shapes do not drift; wave 3's lesson is that "the same value,
deliberately, without importing it" is a hope until a test compares the outputs
(`docs/plan/ess-wave-3-projections.md:160`).

## H5 — The runner needs a runtime and a clock; the workspace has neither (high)

**Verified, and this is the largest unpriced item in wave 4.**

*No async runtime exists.* `crates/aep-contract/src/testing.rs:22` is the only future driver, it
busy-polls, and it **panics** rather than waits:

> Panics if the future is still pending after a bounded number of polls, which for a synchronous
> backend means it is waiting on something no waker will ever signal — a deadlock, reported rather
> than hung.

`async-trait` is not a workspace dependency (`Cargo.toml:27-46`; the list is serde, serde_json,
serde_yaml, schemars, thiserror, clap, anyhow). W4 §7's `#[async_trait] ConformanceTarget` and
§14/§15's bounded polling both need something that can genuinely wait.

*No clock exists below `aep-engine`.* `pub trait Clock` is at `crates/aep-engine/src/clock.rs:12`, and
`SystemTime::now` appears exactly once in the workspace — inside it. Invariant 8 (`AGENTS.md:45`)
forbids a clock in the domain crate, so `Clock` cannot move down to `aep-domain`. W4 §28's
`ScenarioResult.duration: Duration` and §30's `started_at`/`completed_at: Timestamp` therefore have no
source, and the ESS crates currently depend on `aep-domain` only — an `ess-conformance → aep-engine`
edge to borrow a clock is a dependency direction nobody has argued for, and a second `Clock` trait is
a duplicate abstraction.

*W4 contradicts itself here.* §15 configures `poll_interval: 25ms`; §40 makes "no sleeps" an explicit
invariant. A poll interval **is** a sleep. The existing resolution is the one §14 already reaches for
in the read-your-writes case and then abandons for the eventual case: the *target* blocks until it can
satisfy `AtLeast(token)`, and the runner never polls. `Store::has_reached`
(`aep-backend-memory/src/store.rs:80`) is that mechanism working today. Applying it to `eventual`
views as well removes the runtime requirement, the clock requirement and the contradiction in one
move — at the cost of pushing the wait into every adapter.

## H6 — Putting the runner in `ess-gen` breaks two of that crate's stated properties (high)

W4 §35 and §52 place `runner.rs` and `report.rs` under `crates/ess-gen/src/conformance/`. Two
problems, both recorded decisions rather than accidents:

**`Generator::generate` is infallible on purpose.** `crates/ess-gen/src/artifact.rs:44`:

> Infallible on purpose. A generator reaching a construct it cannot project is a gap in this crate,
> not a fault in the specification — and the specification has already been refused if it was wrong,
> because this takes an `EssIr` and there is no way to hold one that did not resolve. So there is
> nothing left for a `Result` to report.

Wave 3 recorded why refusing would have been worse
(`docs/plan/ess-wave-3-projections.md:95`): *"A specification that has already resolved is not at
fault for a hole in this crate, so failing would report the wrong thing — and it would destroy the
very pages that say what is missing."* W4 §11 and §18 both **require** refusal.

**`ess-gen` states it has no clock.** `crates/ess-gen/src/lib.rs:24`: *"Same IR in, byte-identical
bytes out. No clock, no RNG, `BTreeMap`/`BTreeSet` only."* A runner that timestamps a report violates
this.

The correct shape for the first problem is already in the other document: W5 §2's
`SynthesisDisposition::{Generated, Obligation, Refused}`. Wave 4 needs it more than wave 5 does —
carry a `Refused { scenario, reason, ess_ref }` list *in the suite*, so a refusal is a committed,
drift-checked artifact a reader can audit rather than a build failure. Then synthesis stays a
`Generator`, the infallibility decision survives, and C5's `escalate` refusal becomes a visible line
in `generated/conformance/suite.json` instead of a red build.

The second problem forces the crate split. F9 says three crates and *"split when a boundary has been
argued about twice"* (`docs/design/ess-review-v0.1.md:171`) — but that rule is about premature
abstraction, and this boundary is not argued, it is *mechanical*: the clock cannot live in `ess-gen`.
Synthesis in `ess-gen` as a fifth `Generator`; the runner in `crates/ess-conformance`.

## H7 — The step-list silently replaces the §18 format the model was built for (high, process)

`ess-implementor-design-v0.1.md` §18 "Conformance scenario format" (lines 942–976) is a declarative
`given / expect / eventually` YAML shape. `docs/plan/ess-roadmap.md:172` commits wave 4 to it: *"The
§18 scenario format, derived from the IR"*. And the model already carries a type that exists *only*
for it — `crates/ess-domain/src/view.rs:105`:

> The block a generated conformance scenario puts a view assertion in (§18).

with `AssertionStyle::as_str` returning `"expect"` / `"eventually"` — literally "the scenario key this
style writes" (`view.rs:122`).

W4 §21 replaces this with `Vec<ScenarioStep>` and ten step variants, without recording a deviation.

**The step-list is better, and the reason is concrete.** §18's declarative shape has no slot for a
negative assertion, and W4 §10 correctly makes negatives non-negotiable — *"Negative assertions are
first-class. A happy-path-only suite is non-conformant with the design."* Against the fixture,
`CreateInvoice/rejected` must assert `InvoiceCreated` **absent**, and §18 cannot express it. §18's own
worked example is also stale against the shipped fixture: it asserts `InvoiceById.where: {status:
created}`, and the real `InvoiceById` (`invoice.yaml:168`) projects `invoice_id` and `total` with no
`status` field at all.

So: adopt the step-list, and record it in `docs/design/reconciliation-v0.2.md` §5 as AGENTS.md:11
requires. Keep the step keys spelled `expect` and `eventually` so `AssertionStyle::as_str` remains the
single source of that decision rather than dead weight.

## H8 — Obligation kinds granting capabilities is a second grant path (high)

Invariant 6 (`AGENTS.md:42`): *"Capabilities default to deny, and `deny` beats `require_approval`
beats `allow`. A principle may restrict; only a profile or protocol may grant."* This is mechanical,
not aspirational — `crates/aep-domain/src/capability.rs:626`:

```rust
/// Adds `other`'s approvals and denials only, never its grants.
/// [...] a principle cannot hand out access a profile did not grant.
pub fn restrict(&mut self, other: &Self) { ... }
```

with the test `restrict_cannot_grant_but_grant_can` at `:733`, and `capability.rs:10`: *"A capability
that appears in no list is **not granted**."*

W5 §28 "Capability Derivation" proposes that obligation kinds "inform safe capability defaults":
`BusinessPolicy → repository.read, repository.write, tests.execute`; `ExternalEffect → repository.*`.
An `ImplementationObligation` is a machine-derived artifact, not a profile and not a protocol.
Deriving grants from it is a second place capabilities come from — the shape invariant 14
(`AGENTS.md:55`) exists to prevent one level down: *"There is no second write path, because a second
path is a second place to forget validation, authorisation, idempotency, provenance and audit."*

W5 §14 already has the correct answer: *"Different obligation kinds may later select different ADP
**profiles** and evidence requirements."* §14 and §28 contradict each other and §14 is right. The
design's own reason for §28 (least privilege) is served better by §14's routing, because a profile is
auditable and reviewable and an obligation-kind table is not.

Fix: delete §28's grant table, keep the profile selection. Free.

## M1 — W4 §9's own witness is refused by the type before the command sees it (medium)

`Money` declares `invariants: [amount >= 0]` (`examples/billing/domains/invoice.yaml:28`).

W4 §9's illustration is `CreateInvoice(amount = -1)` → "declared result: outcome = rejected, error =
InvalidAmount". But `-1` is **not a valid `Money`**. A conformant implementation must refuse the input
at the type boundary, and the observed result is an input-validation failure — which is exactly the
"declared domain rejection vs adapter/runtime failure" confusion §9 exists to forbid. The only witness
for `rejected` against this fixture is `amount == 0`, which §26 and §48 get right.

This is cheap to correct and it names a real trap: a witness generator must intersect the negation of
every other outcome's `when` with the *input types' own invariants*. W4 §11's four-step strategy does
not mention type invariants at all.

## M2 — The model has no `examples:` construct (medium, structural-small)

W4 §11's witness strategy is: "1. prefer explicit examples/fixtures if the ESS provides them; 2.
derive trivial witnesses for primitive predicates where deterministic; 3. allow the normative example
to provide scenario fixture values; 4. refuse when no safe witness can be produced."

**Verified absent:** no `examples` or sample-value field exists on `CommandSpec`, `Field`,
`EntitySpec`, `ViewSpec` or any other spec type in `crates/ess-domain/src/`. So strategies 1 and 3
have no source, and strategy 2 carries the whole load.

What strategy 2 must actually do for the fixture's two commands:

| outcome | strategy | what a witness needs |
|---|---|---|
| `CreateInvoice/accepted` | `ConstructInput` | `amount.amount > 0`; `currency: String` **unconstrained**; `customer_email: Email` (newtype of `String`, no invariants) — **unconstrained** |
| `CreateInvoice/rejected` | `DefaultBranch` | negate `amount.amount > 0`, intersect with `Money`'s `amount >= 0` ⇒ exactly `0` |
| `SendEmail/sent` | `ConstructInput` | both inputs unconstrained |
| `SendEmail/failed` | `InjectFault` | no input reaches it |

So most witness fields are arbitrary. Invariant 9 (`AGENTS.md:47`) and `ess-gen/src/lib.rs:24` forbid
RNG, so this must be a fixed constant table per primitive, committed and drift-checked like every
other generated artifact. Implementable; unstated in the design.

One further wrinkle the IR already flags: the witness must resolve `amount.amount`, a fact path
walking into a struct, and `crates/ess-compiler/src/ir.rs:59` records that as deliberately open — *"a
rule for the deep paths (`total.amount` walking into a struct) that `ess-domain` deliberately leaves
open — a new rejection class in a pass whose job is to resolve, not to grow rules."* Wave 4 is the
consumer that needs it.

## M3 — `TestStrategy` and `AssertionStyle` already decide what §11/§12/§14 re-derive (medium)

Two fields exist in the IR *specifically* to stop wave 4 deciding these per generator, and neither
design mentions either.

`ResolvedOutcome::test_strategy` — `crates/ess-compiler/src/ir.rs:488`:

> How a generated test reaches this branch. Computed once here, from the domain's own answer, so two
> projections cannot disagree about whether a branch is reachable by constructing an input.

backed by `TestStrategy { ConstructInput, DefaultBranch, InjectFault }`
(`crates/ess-domain/src/command.rs:233`), computed from `OutcomeCondition` at `:214`. W4 §11
re-derives the same three-way split in prose.

`ResolvedView::assertion_style` — `ir.rs:609`, with the same rationale spelled out: *"it is a
decision, and a decision made per projection is a decision made wrong eventually."* W4 §14 re-derives
`expect`-vs-`eventually` from `Consistency` without naming it.

W4 §12's `ExternalOutcomeControl { command, force_outcome }` is a **real addition** and should stay:
`TestStrategy::InjectFault` says *that* a fault must be injected, not *how* to ask a target to inject
it. Drive it from `test_strategy`, not from a fresh analysis of `condition`.

The risk is not wasted work, it is a second derivation that disagrees — the exact bug wave 3 shipped
and had to fix (`docs/plan/ess-wave-3-projections.md:146`: three projections each carrying their own
copy of the type mapping, and all 17 comparable pairs disagreeing).

## M4 — W4 §33 and §49 steps 9–10 already exist and pass (medium, good news)

**Verified.** `crates/aep-engine/tests/end_to_end.rs:245`,
`a_specification_governed_task_is_not_finished_until_something_else_says_it_conforms`, walks the whole
loop W4 §33 proposes as "the first real ADP task":

1. inserts `ArtifactKind::ExecutableSystemSpecification` into the artifact graph (`:257`);
2. asserts `ess_conformance` becomes outstanding;
3. submits `Evidence::EssConformance` `by_agent` and asserts the requirement **stays owed** — *"An
   agent's own report that it conforms is not a conformance run"*;
4. submits the same evidence `by_verifier(..., Verifier::ConformanceRunner)` and asserts it closes.

And `crates/aep-engine/tests/end_to_end.rs` also has
`a_task_without_a_specification_owes_no_conformance`, so the conditional half is covered too. The
repository states this deliberately — `docs/guide/specification.md:241`:

> Until a compiler exists, a person produces that evidence by hand. The point is that the *shape* is
> already right: when the runner arrives, nothing about the protocol side changes.

So W4 §49 step 9 is a `From<EssConformanceReport> for EssConformanceResult`, and step 10 is replacing
two hand-written literals with a real report. Roughly 50 lines and one test. W4 §50's five "Evidence"
acceptance criteria are four-fifths met at `3647f80`.

This materially changes where wave 4's risk sits: not at the protocol end, which is done, but in the
middle — the runner, the reference implementation and the fault matrix.

## M5 — `EssConformanceResult` cannot carry a digest without a model change (medium)

`crates/aep-domain/src/evidence.rs:673` is `#[serde(deny_unknown_fields)]` with fields
`specification, implementation, status, scenarios_total, scenarios_failed, suite_version,
compiler_version, generator_version, failed_scenarios`. W4 §31 says "prefer also attaching or
referencing spec digest, report digest, conformance report artifact — if the existing evidence model
permits this without expanding scope unnecessarily."

It does not permit it. Adding those fields means: the struct, the fact projection (`evidence.rs:1377`),
the regenerated `schemas/generated/aep.schema.json` (`cargo xtask schema --check` is a CI job of its
own, `.github/workflows/ci.yml:48`), and possibly a new observable in `protocols/adp/1.yaml:31`. Small,
but it touches the gate — and see H3 on the 16-hex-character digest being sized for a comment header
rather than for evidence.

## M6 — Invariant 7 is honoured; the independence *claim* is not enforced, and neither design closes it (medium)

Invariant 7 (`AGENTS.md:44`): *"The engine never manufactures evidence. It evaluates what verifiers and
humans produced."* **W4 §32 agrees with it explicitly and unprompted**, and correctly: the agent may
trigger the runner, read the report and repair failures, but must not construct the authoritative
payload by assertion. No conflict.

But the mechanism is thinner than either document assumes. Independence is a `Producer` variant on the
submission — `Producer::Verifier { verifier }` vs `Producer::Agent { id }`
(`crates/aep-engine/tests/end_to_end.rs:80`) — and the principle's requirement
(`principles/verification/ess-conformance.yaml:35`, `independent: true`, `verifier:
conformance-runner`) checks the **claim**, not its truth. Nothing structurally prevents an agent from
submitting `Producer::Verifier { verifier: ConformanceRunner }`.

Neither design says who invokes the runner, how the submission is attributed, or what makes the
`Producer::Verifier` claim trustworthy (a separate process? CI as the submitter? a signature over the
report digest?). That is the one place the entire closed loop can be short-circuited, and it sits
inside the section the design is most confident about. See "what is missing", item 2.

## M7 — Determinism: the design is right, and the mechanism already exists (no finding)

W4 §37 is precisely invariant 9 (`AGENTS.md:47`) plus wave 2's mechanism, and the split it draws —
*"Execution reports may contain timestamps. Generated definitions may not"* — is exactly right. The
machinery is built:

| §37 rule | exists |
|---|---|
| deterministic maps/sets, canonical serialization, trailing newline | `EssIr::to_canonical_json`, `crates/ess-compiler/src/ir.rs:878` |
| committed fixture regeneration checked in CI | `cargo xtask generate --check`, CI job "Projections up to date", `.github/workflows/ci.yml:65` |
| an artifact nothing generates any more is caught | orphan detection, `xtask/src/main.rs:9` and `:179` |
| no clock, no RNG in the generator | `crates/ess-gen/src/lib.rs:24` |

So W4 §38 ("commit the generated suite, CI regenerates and diffs") is a **one-line addition** to the
existing `generated/` mechanism, not new machinery — provided synthesis is a `Generator` (see H6).
This is the strongest section in either document.

## M8 — Ordering: correct, but both waves are blocked on the same missing construct (medium)

Wave 5's header ("assumes the closed-loop conformance milestone already exists and is trusted") is
right, and its acceptance criterion §W5.9 — *"Run the existing ESS conformance suite unchanged"* — is
the correct dependency direction. W4 §43/§44 states the same reuse from the other side, and W4 §44's
rule ("no new behavioural oracle for generated Rust; if Rust synthesis needs bespoke tests, decide
whether they are generator tests or evidence the ESS lacks semantics") is the right guard.

Two ordering problems:

**Nothing in wave 4 is blocked on wave 5. One thing in wave 4 is blocked on something neither
provides.** C1's command↔transition construct blocks W4 §19/§20/§25 *and* W5 §6/§7/§W5.3. It belongs
before wave 4, in `ess-domain`.

**The wave-5 document is four waves.** `docs/plan/ess-roadmap.md:199` defines wave 5 as W5.1–W5.3:
domain types/commands/events/views; component skeletons and *one* transport adapter; the generated
code passing wave 4's suite. W5 §40 already lists ten steps beyond that. §41 (W6.1–W6.8: obligations
as AEP artifacts, obligation-derived tasks, the agent repair loop), §42 (W7.1–W7.4: `Realization`) and
§43 (W8–W12) are three further waves inside a document titled wave 5. The roadmap and the document
disagree about their own numbering, and the roadmap is the one the repository follows
(`docs/plan/ess-wave-3-projections.md:194`: *"wave 1's plan page lists test synthesis under wave 3,
and the roadmap is the one to follow"*).

## M9 — Wave 4 as written is three waves (medium)

Measured, not estimated:

| wave | commits | files | insertions | tests |
|---|---|---|---|---|
| ESS 1 | `4450450`, `95e210f` | 71 | 16,096 | 442 → 642 (+200) |
| ESS 2 | `ea68f18` | 33 | 10,661 | 642 → 777 (+135) |
| ESS 3 | `bffaf71`, `05c3d04` | 54 | 14,283 | 777 → 916 (+139) |

(`git show --stat`, and the counts from `git tag -n99`.)

A wave here buys ~10–16k insertions and ~135–200 tests, and each of the last three did exactly **one
kind of thing**: the model, the compiler, the projections. Wave 4 as written does three:

1. a projection — scenario synthesis, which resembles wave 3 and reuses its machinery;
2. an execution layer — `ConformanceTarget`, runner, hand-written reference implementation, fault
   injection. A new dependency class (H5) and a new crate (H6);
3. an integration — report → evidence → ADP, which is four-fifths done (M4).

W4 §4 lists 12 deliverables, §49 lists 10 steps and §39 lists 6 test layers. Nothing about that is one
wave. **Verdict: three, and the seams are clean:**

| slice | contents | §49 steps | new deps | clock |
|---|---|---|---|---|
| **4a** the suite as an artifact | scenario IR, synthesis, provenance, `Refused` as data, committed `generated/conformance/suite.json`, drift check. A 5th `Generator` inside `ess-gen`. | 1–2 | none | none |
| **4b** the oracle that bites | `ConformanceTarget`, runner, hand-written billing reference, fault injection, per-fault matrix test, bindings, lifecycle, `eventual`. New crate `ess-conformance`. | 4–7 | H5's decision lands here | yes |
| **4c** the loop closed | `EssConformanceReport` → `EssConformanceResult`, `end_to_end.rs` rewired to a real report | 8–10 | none | — |

W4 §55 argues for exactly this order (*"Do not synthesize more software until the specification has
proven that it can independently judge software"*). §49's step sequence is already right; the document
just does not admit that step 4 crosses a wave boundary.

## L1 — `502` for an `external` outcome will read as an infrastructure error (low)

Wave 3's stated convention maps `accepted → 202`, `rejected → 422`, `external → 502`
(`CHANGELOG.md`, `[0.3.2-ess-wave-3]`: *"A status code comes from the outcome, and `external` is not
the caller's fault"*). W4 §9 requires that "conformance tests must fail if declared domain behavior is
surfaced merely as an untyped infrastructure error".

For `SendEmail/failed` an HTTP adapter returns `502` carrying a declared `Undeliverable` — a declared
outcome delivered through an infrastructure status code. A naive adapter will map it to
`TargetError::Http502` and the scenario will fail for the wrong reason. Cheap to state in the design;
easy to get wrong exactly once.

## L2 — `Unsupported` is an improvement; record the divergence (low)

See H4. `SuiteReport::aborted` (`report.rs:96`) collapses "could not be checked" into a failed check.
W4 §28's `passed / failed / error / unsupported` split is better, and its rule that an `unsupported`
required scenario makes the aggregate fail is right. Record it rather than letting two report shapes
drift.

## L3 — W5 §16 reuses `EntityLocator` correctly (low, and worth saying)

`ImplementationObligation { id, locator: EntityLocator, source, kind, contract, contract_digest,
verification }` reuses a real type — `aep_domain::entity::EntityLocator`, the `ep://` logical address
from reconciliation §2.1, implemented in `crates/aep-domain/src/locate.rs`. It also respects invariant
13 (identity is opaque; a human-readable key belongs in the locator). This is the reconciliation the
wave-4 header asks for, done right, in the other document.

## L4 — `ArtifactKind::ImplementationObligation` is cheap, and `Other` is cheaper (low)

`ArtifactKind` (`crates/aep-domain/src/artifact.rs:328`) needs a variant, a `NAMED` entry (`:392`), an
`as_str` arm (`:422`), a parse arm (`:461`) and a regenerated schema. Small, but it touches the gate.

Note `ArtifactKind::Other(String)` (`:387`) already exists as the escape hatch, so
`Other("implementation-obligation")` works today with **no code change** — the cheaper first slice if
W5 §41's W6.1 is being probed rather than committed to.

## L5 — The CLI verbs W4 §34 suggests match what shipped (low, no collision)

`EssCommand` (`crates/protocol-cli/src/main.rs:514`) already has `Validate`, `Compile`, `Inspect`,
`Generate`, `Graph`. W4 §34's `protocol ess generate <spec> --kind docs|schema|openapi|asyncapi` is
what exists; `protocol ess test generate` and `protocol ess conformance run` are new and consistent
with the shape. W4 §34's own hedge ("Do not treat these names as normative if the unpushed `ess-gen`
already established a coherent command shape") is the right instinct and the shape is coherent. Clap
derive is the repository's rule (`AGENTS.md:77`) and neither design proposes otherwise.

---

## P1 — W4 §11 against the decision to pursue `proptest` (high)

Assessed against the stated intent — `proptest` on stable, phase 1 generating adversarial
*specifications* against parse/validate/compile/project, phase 2 carrying that generator into wave 4
as input-witness synthesis — rather than in isolation. Three questions, answered in order.

### Does the refusal-first stance survive a generate-then-check generator?

**Yes, and it is the reason to keep it — but only if `Unknown` is routed to refusal, never to retry.**
This is the finding most likely to be got wrong.

`Predicate::evaluate(&self, facts: &dyn FactSource) -> Truth`
(`crates/aep-domain/src/predicate.rs:345`) returns Kleene three-valued truth
(`predicate.rs:56`: `True | False | Unknown`). W4 §11 is written two-valued — *"never generate an
arbitrary value and claim it satisfies an outcome predicate unless the generator can prove or
**evaluate** that it does"* — and says nothing about the third answer. The correct mapping:

| `evaluate` returns | the generator's move | W4 §11 as written |
|---|---|---|
| `True` | keep it: this is a witness | satisfied |
| `False` | discard and draw again — proptest's normal filter path | fine |
| `Unknown` | **refuse the scenario**, and say which predicate could not be decided | not covered |

Feeding `Unknown` into a `prop_filter` is the trap. proptest treats a filter miss as "draw another
value" against a local rejection budget, so a predicate that is *structurally* undecidable burns the
budget and surfaces as a flaky "too many local rejections" rather than as the refusal W4 §11 wants —
turning a specification defect into a test-harness defect. That collapse is exactly what invariant 5
forbids, moved into a harness: `AGENTS.md:39` — *"`Unknown` is not `False`. Predicate evaluation is
three-valued; only `True` permits a transition. **Never collapse unobserved to false.**"*

So: evaluate, classify the `Truth`, and only let `False` reach the filter. W4 §11's own sentence — *"A
refusal is better than a false test"* — is the right principle and the right place to hang this; it
just needs a third branch.

The billing fixture will **not** catch a wrong implementation of this. `amount.amount > 0` is
Number-vs-Number (`predicate.rs:385-388`) and evaluates cleanly to `True`/`False`, so the two-valued
version passes the only fixture that exists and fails on the first specification with a text ordering
or a union-valued condition. A green fixture is not evidence here.

### Is evaluation cheap and total?

**Cheap: yes. Total as a function: yes. Total as a decision procedure: no** — five distinct sources of
`Unknown`, and three of them no choice of generated value can resolve.

Cheap is not in doubt: `evaluate` is a pure fold over a small tree with `BTreeMap` lookups, no I/O and
no allocation beyond a diagnostic note. Where it stops being a decision procedure:

| # | source of `Unknown` | citation | generator-fixable? |
|---|---|---|---|
| 1 | an operand resolves to no observed fact | `predicate.rs:379`; also `Truthy` `:358`, `AnyOf` `:361`, `NoneOf` `:364` | **yes** — it means the generator left a field unpopulated, which is a generator bug and a detectable one |
| 2 | text ordering with no protocol scale containing both values | `predicate.rs:388-397`, with the note *"cannot order X against Y: no protocol scale contains both values"* | **no** — needs a declared scale (reconciliation §5.4: *"inventing lexicographic order silently would make `high < low` true"*) |
| 3 | ordering across types — Number against Text with `>` | `predicate.rs:398-404`, *"cannot order a {} against a {}"* | **no**, for every value |
| 4 | Kleene propagation: one `Unknown` leaf poisons a whole `All` | `predicate.rs:66-84` — `Unknown` dominates `True` in a conjunction | **no** — inherited from 2, 3 or 5 |
| 5 | **no fact path exists for the field at all** | `FactValue` is `Bool \| Number \| Text` (`facts.rs:119`) | **no** — a missing construct |

Source 5 is the structural one. A command input in ESS is a tree — `Money {amount: Decimal, currency:
String}`, `List<LineItem>`, `Map<String,String>`, `Optional<String>`, the tagged union `Payee`,
`Bytes`, `Duration` (all present in `examples/billing/domains/invoice.yaml:51-90`) — and a `FactValue`
is a scalar. There is no fact path for `lines` or for `payee`, so a `when` predicate over a list or a
union field is not evaluable at all, at any value.

**And the precondition nobody has built:** to evaluate a candidate input against `amount.amount > 0`
you need a `FactSource` whose facts *are* that input. `FactSource` (`facts.rs:583`) is implemented
exactly once, by `FactStore` (`facts.rs:669`), and consumed only by the engine
(`aep-engine/src/execution.rs:459`) and the requirement layer (`requirement.rs:64`). Nothing projects
an ESS command input into facts. Worse, the IR already records the rule for it as deliberately open —
`crates/ess-compiler/src/ir.rs:59`:

> a rule for the deep paths (`total.amount` walking into a struct) that `ess-domain` deliberately
> leaves open — a new rejection class in a pass whose job is to resolve, not to grow rules.

**That flattener is the real first deliverable hiding inside W4 §11, and neither design names it.**
Scope it as: `EssIr` + a candidate input value → `FactStore` of dotted scalar paths, refusing (not
guessing) at a `List`, `Map` or `Union` boundary. Sources 2, 3 and 5 then become refusal reasons with
provenance, which is the shape W5 §2 already proposes for wave 5 (see H6).

### Does a seeded generator collide with invariants 8 or 9?

**No. A `proptest` dev-dependency is plainly outside the scope of both.**

- Invariant 8 (`AGENTS.md:45`) — *"The domain crate is clock-free and randomness-free. No
  `SystemTime::now`, no RNG. The engine takes a `Clock` so an execution is replayable."* This
  constrains what the **shipped** code does. Verified: `SystemTime::now` occurs exactly once in the
  workspace, in `crates/aep-engine/src/clock.rs:23`, behind the `Clock` trait (`:12`). A generator in
  `[dev-dependencies]` puts no RNG on any shipped path.
- Invariant 9 (`AGENTS.md:47`) — *"Determinism. Same validated state plus same evidence set ⇒ same
  decision."* This is a property **of** the code that a property test *checks*. It says nothing about
  how test inputs are chosen; choosing them randomly is the strongest available way to check it.

The precedent is already in the tree and is exact: `jsonschema` is a dev-dependency of `ess-gen` with
a recorded rationale and `default-features = false` chosen to keep tests off the network
(`crates/ess-gen/Cargo.toml`, dev-dependencies comment). A generator occupies the same slot. Verified
absent today: no `proptest`, `quickcheck` or `arbitrary` anywhere in the workspace.

Three real frictions, none fatal:

1. **`clippy --workspace --all-targets -D warnings` covers test code** (`Taskfile.yml`,
   `.github/workflows/ci.yml:38`) and `clippy::pedantic` is on workspace-wide (`Cargo.toml:53`).
   proptest's macros generate code that will trip pedantic lints; expect targeted `#[allow]`s at the
   macro sites. Cheap, but it surfaces on the first run, not later.
2. **`proptest-regressions/` must be committed.** That file is what makes a property failure
   reproducible, and without it a failure found in CI cannot be reproduced locally — which *is* where a
   genuine collision with the spirit of invariant 9 would occur. Verified: `.gitignore` has four
   entries (`/target`, `**/*.rs.bk`, `*.pdb`, `.DS_Store`) and excludes nothing relevant, so this needs
   no `.gitignore` change — only the discipline of committing the file.
3. **`ess-gen` states "No clock, no RNG" about itself** (`crates/ess-gen/src/lib.rs:24`). A
   dev-dependency does not falsify it, but the sentence reads absolute and someone will file it as a
   contradiction. One clause settles it: *in the generators; the property tests seed one deliberately.*

### Where the two designs help, and where they obstruct

**Help:**

- W4 §11's refusal-first default is the **opposite** of what a naive proptest integration does, and
  that is its value. Keep it; add the `Unknown` branch.
- W4 §11 explicitly defers a constraint solver ("Possible later extension... but this is not required
  for the first closed loop"). That deferral is what makes generate-and-check the right phase-2
  technique rather than a compromise.
- `TestStrategy` (`crates/ess-domain/src/command.rs:233`) hands phase 2 free targeting: only
  `ConstructInput` and `DefaultBranch` outcomes may be generated for; `InjectFault` outcomes must not
  be attempted, and `ResolvedOutcome::test_strategy` (`ir.rs:488`) already says which are which. In the
  fixture that is 3 of 4 outcomes generable and `SendEmail/failed` correctly excluded.
- W4 §37's determinism rules give phase 2 its acceptance criterion for free: a *seeded* generator with
  a committed seed yields a byte-identical suite, drift-checked by machinery that already exists
  (`xtask/src/main.rs:9`, CI job at `ci.yml:65`).
- **Phase 1 has an unusually good target surface, and it is the repository's own doing.** `Raw*` →
  `TryFrom` → validated (invariant 2), accumulating errors (invariant 3), a stable `ValidationCode` per
  failure (invariant 4), and `compile` as the sole door to an `EssIr` (`ir.rs:9-14`). The property is
  short and strong: *for any generated document, either validation returns at least one
  `ValidationCode`, or `compile` succeeds and `EssIr::to_canonical_json` is idempotent* — with no panic
  in between. `EssIr::to_canonical_json` (`ir.rs:878`) is the oracle, and
  `compiling_the_billing_example_twice_produces_byte_identical_json`
  (`crates/ess-compiler/tests/billing.rs:256`) is the exact shape to generalise from one fixture to a
  generated population.
- W5 §39 anticipates this and routes it correctly — *"parser → property testing"* as an assurance
  level attached to an obligation kind. It simply does not connect to W4 §11.
- W5 §2's `Refused` disposition is the right data shape for phase 2's refusals (H6).

**Obstruct:**

- W4 §11 strategies 1 and 3 both point at fixtures the model does not have (M2), so phase 2 carries
  the whole load and the four-step ladder collapses to two.
- W4 §11's two-valued framing (above) is the single most likely thing to be implemented wrong, and the
  fixture will not catch it.
- W4 §11 assumes a candidate value can be evaluated against a predicate; the input→`FactSource`
  projection does not exist and the deep-path rule is explicitly open (`ir.rs:59`).
- **A phase-1 hazard worth knowing before the first run:** the handle accessors panic by design when a
  handle from one `EssIr` is used against another — `crates/ess-compiler/src/ir.rs:141`, *"a handle
  belongs to the IR that minted it"*, documented as *"a programming mistake and not a specification's
  problem"*. A generator that builds two IRs and crosses handles will hit a designed panic and report
  it as a finding. Scope phase 1's property to one IR per case.
- **A phase-1 finding to expect rather than treat as a regression:** `Operand::parse`
  (`predicate.rs:225-236`) — *"A bare word containing a dot is a fact path; everything else is a
  literal"* — means `when: currency == en.US` silently becomes a fact-path comparison that evaluates
  `Unknown` forever. Numeric literals are guarded (`parse::<f64>().is_err()`), text ones are not. Phase
  1 will find this quickly; that is the point of phase 1.
- Neither design mentions `Truth`, `FactSource` or `FactValue` once. The reconciliation the wave-4
  header asks for has its largest gap precisely here.

---

## What is missing that the designs' own logic requires

1. **A command↔transition and command↔entity construct.** Both designs assume it; neither proposes it.
   It blocks W4 §19, §20, §25's `F-ILLEGAL-TRANSITION`, C3's read-your-writes path, and W5
   §6/§7/§W5.3. Highest-value change in either document, and absent from both.
2. **What makes `Producer::Verifier` true.** W4 §32 states the policy that an agent must not
   manufacture conformance evidence; nothing enforces the producer claim (M6). The design must say who
   invokes the runner and how the submission is attributed, or the loop's load-bearing claim rests on
   an honour system.
3. **A refusal artifact for wave 4.** W4 §11 and §18 require refusal; `Generator::generate` is
   infallible by recorded decision. W5 §2 solves this (`Refused`) for wave 5 and wave 4 does not
   inherit it. Without it, "scenario synthesis refuses" means "the build fails" — which wave 3
   explicitly rejected.
4. **Where the clock and the runtime live** (H5). Not mentioned in either document, and it is the
   decision that determines whether wave 4 adds two workspace dependencies.
5. **An input→`FactSource` projection** (P1). Nothing turns a candidate command input into the facts a
   predicate reads; `FactSource` has one implementor (`facts.rs:669`) and the deep-path rule is
   deliberately open (`ir.rs:59`). This is the first task of witness synthesis and neither design names
   it. Alongside it: the rule for intersecting a negated `when` with the input types' own invariants
   (M1), and — if the generator is not seeded — a deterministic witness constant table (M2).
6. **A `Truth::Unknown` branch in W4 §11** (P1). The section is written two-valued against a
   three-valued evaluator; `Unknown` must refuse, not retry.
7. **A stated convention linking an event field to an entity identity** (C2), written into the
   generated artifact in the wave-3 style rather than assumed.
8. **W5: the selection rule when two implementations claim one obligation.** §20 proposes
   `link(plan, implementations) -> Result<LinkedSystem, UnsatisfiedObligations>` and §21 proposes
   multiple implementations per obligation. Nothing says how `link` chooses, or whether ambiguity is an
   error.
9. **W5: obligation invalidation should reuse revision-bound approval, not mirror it.** §16 says the
   `contract_digest` mechanism "mirrors revision-bound design approvals" and §29 restates it as
   caching. `ReviewResult::covers` already implements exactly this (reconciliation §1, `review.rs`) —
   *"an approval of version 3 does not cover version 7"*. Reuse it and check the name.

---

## What these designs get right

Worth stating plainly, because the findings above are all objections:

- **W4 §37 determinism.** Precisely invariant 9, with the right split between suite and report, and it
  reuses the mechanism that exists rather than proposing a new one. The best section in either
  document.
- **W4 §32.** Invariant 7, restated correctly and unprompted, including the wrong pattern spelled out
  (`agent: "all ESS tests passed" → EssConformance(passed=true)`).
- **W4 §5's architectural rule** — *"If the conformance runner needs information that does not exist
  in `EssIr`, the default response is: improve the ESS model or IR. Do not hide missing semantics
  inside the runner."* This is the rule that would have caught C1 through C5, had it been applied to
  the fixture rather than stated about it.
- **W4 §17 and §18.** Refusing to assert exactly-once from `at_least_once`, and refusing to invent an
  observable for `escalate`. Both correct, both expensive, both stated anyway — including the feedback
  loop ("test cannot express expected failure behavior → the model is semantically incomplete").
- **W4 §21/§22.** Insisting on a technology-neutral scenario IR before any Rust runner is the same
  argument that produced `EssIr`, applied one level up, and it is right for the same reason.
- **W4 §41/§42.** Separating semantic from contract conformance, and leaving the OpenAPI/AsyncAPI
  documents where wave 3 put them.
- **W4 §35.** Respects F9 and refuses to create a crate for a concept it merely named — *"Do not
  create it merely because this document names the concept."*
- **W4 §10.** "Negative assertions are first-class. A happy-path-only suite is non-conformant with the
  design." This is the criterion that makes the step-list IR necessary (H7) and it is stated before
  the shape that needs it.
- **W4 §28's status taxonomy** and the rule that an unsupported required scenario cannot be silently
  skipped. Better than what `aep-conformance` has today.
- **W5 §2's `SynthesisDisposition`** — `Generated | Obligation | Refused`. The best single idea in
  either document, and wave 4 needs it more than wave 5 does.
- **W5 §8's algebraic command outcomes.** Maps one-to-one onto `ResolvedOutcome`, and keeps
  `InfrastructureError` outside the outcome type, which is the distinction the whole outcome model
  exists to preserve.
- **W5 §16's `EntityLocator` reuse** (L3) and **§14's routing of obligation kinds to profiles** — the
  latter being the correct version of what §28 gets wrong.
- **W5 §17's generated/authored ownership boundary.** Matches how `generated/` already works,
  including "regeneration may destroy and recreate `generated/`" which is what
  `cargo xtask generate` does today.
- **W5 §37's refusal to treat synthesis coverage as a quality score.**

---

## Decisions for Timo

| decision | options | cost | default if nobody answers |
|---|---|---|---|
| **The command↔transition gap (C1)** | (a) add `transition:` to a command outcome and `entity:` to a command, in `ess-domain`, *before* wave 4; (b) ship wave 4 without lifecycle scenarios and say so in writing; (c) match transition to command by name convention, stated in the artifact | (a) model change + validation + fixture growth, 2–3 days; (b) free, loses 2 of 7 fault rows; (c) ~1 day, adds a convention wave 3's precedent permits | **(a)**. Without it wave 4 cannot meet its own §50 and wave 5 cannot start §W5.3. It is the only irreversible item here. |
| **Grow the billing fixture (C1, C3)** | (a) add `IssueInvoice`/`PayInvoice`/`CancelInvoice` so the lifecycle and the read-your-writes view are reachable; (b) leave it and cover 4 of 7 faults | (a) fixture + regenerate 27 artifacts + re-pass `tests/agreement.rs`, ~1 day; (b) free | **(a)**. `OutstandingInvoices` is provably always empty today, so `F-VIEW-RACE` cannot be tested at all. |
| **Async or synchronous target (H5)** | (a) `async-trait` + tokio as workspace deps; (b) synchronous `ConformanceTarget`, the target blocks internally to satisfy both `AtLeast(token)` and `eventual` | (a) 2 new deps, a runtime inside `task check`; (b) no new deps, and every adapter must implement the wait | **(b)**. It matches `aep-contract`'s recorded position (`testing.rs:1`, *"A specification crate should not choose anyone's async runtime"*) and it removes W4's §15-vs-§40 self-contradiction. |
| **Where the runner lives (H6)** | (a) new `crates/ess-conformance` for the runner, synthesis stays in `ess-gen`; (b) all of it in `ess-gen/src/conformance/` | (a) a 4th ESS crate; (b) breaks `ess-gen`'s "no clock" (`lib.rs:24`) and its infallible `Generator` (`artifact.rs:44`) | **(a)**. The clock forces the split; F9's "argued about twice" is about premature abstraction, not about a mechanical constraint. |
| **Refusal as data or as error (H6)** | (a) carry `Refused { scenario, reason, ess_ref }` in the suite, committed and drift-checked; (b) make synthesis fallible | (a) one struct, and `escalate`'s refusal becomes an auditable line; (b) reverses a recorded wave-3 decision | **(a)**. It is W5 §2's idea and wave 4 needs it more. |
| **Scenario format: §18 or step-list (H7)** | (a) step-list, recorded in `reconciliation-v0.2.md` §5; (b) keep §18's `given/expect/eventually` | (a) one paragraph in §5; (b) cannot express `ExpectNoEvent`, which W4 §10 makes non-negotiable | **(a)**. Record it, and spell the step keys `expect`/`eventually` so `AssertionStyle::as_str` stays the single source. |
| **Wave 4 as one wave or three (M9)** | (a) split 4a / 4b / 4c; (b) one wave | (a) three tags and three plan pages; (b) a wave ~3× the size of any prior one | **(a)**. 4a alone is a wave-3-sized deliverable. |
| **W5 §28 capability derivation (H8)** | (a) delete the grant table, keep §14's profile selection; (b) keep it | (a) free; (b) breaks invariant 6, which `CapabilityPolicy::restrict` enforces mechanically | **(a)**. |
| **`EssConformanceResult` digests (M5)** | (a) add `spec_digest`/`report_digest`; (b) keep digests in the report only | (a) struct + facts + schema regen + possibly an ADP observable; (b) free, but evidence cannot be tied to a model version | **(a)**, at 4c — and widen `Provenance`'s 16-hex digest if a completion decision is going to rest on it. |
| **`Unknown` in witness synthesis (P1)** | (a) evaluate, classify the `Truth`, and refuse on `Unknown` with the undecidable predicate named; (b) treat `Unknown` as a filter miss and redraw | (a) one match arm plus a refusal reason; (b) free until the first spec with a text ordering or a union-valued `when`, then it reads as a flaky test | **(a)**. Invariant 5 (`AGENTS.md:39`) already forbids the collapse, and the billing fixture cannot catch (b) being wrong. |
| **The input→`FactSource` flattener (P1)** | (a) build it first, refusing at `List`/`Map`/`Union` boundaries; (b) restrict phase 2 to outcomes whose `when` reads only scalar paths | (a) the real first task of witness synthesis, ~2 days; (b) free, and it covers both fixture commands today | **(b) then (a)**. (b) is enough for slice 1 and defers the deep-path rule `ir.rs:59` leaves open; (a) before any second fixture. |
| **Wave 5 document scope (M8)** | (a) split §41–§43 out into their own roadmap entries (waves 6, 7, 8+); (b) leave the document as filed | (a) an edit to the roadmap; (b) the roadmap and the design disagree about what wave 5 is | **(a)**. The roadmap is what the repository follows. |

---

## The smallest first slice for wave 4

The repository's practice is a demo that bites, then the machinery. The smallest thing that bites here
is **not** the scenario IR — it is one fault failing one check. W4 §49 spends three steps on machinery
before anything has bitten once.

### Slice 0 — pre-wave, in the model: one command drives one transition

Add `transition:` to a command outcome and `entity:` to a command in `ess-domain`. Add
`IssueInvoice: Draft → Issued` to `examples/billing/domains/invoice.yaml`, wired to the existing
`issue` transition.

**Acceptance:** `protocol ess compile` resolves it; `ResolvedOutcome` carries the transition;
`OutstandingInvoices` is reachable; the 27 committed artifacts regenerate and `tests/agreement.rs`
still passes.

This is the only irreversible piece and everything else waits on it. Roughly 2–3 days.

### Slice 1 — the demo that bites: one scenario, one fault, no synthesis

- Hand-write **one** `ConformanceScenario` *value* for `CreateInvoice/rejected`:
  `amount = {amount: 0, currency: "EUR"}` → outcome `rejected`, error `InvalidAmount`, event
  `InvoiceCreated` **absent**. (Amount `0`, not `-1` — see M1.)
- Hand-write the smallest billing target that satisfies it: one struct, a `BTreeMap`, no transport.
- Hand-write a **synchronous** `ConformanceTarget` for it, reusing
  `aep_contract::consistency::{ConsistencyToken, QueryConsistency}` (H1).
- Add one fault — `AcceptInvalidAmount` — following the `FaultyBackend` pattern (H2), and assert the
  scenario fails with a `Check` naming `billing.invoice.CreateInvoice.outcomes.rejected`.

**No** generator, no scenario IR serialization, no report type, no evidence, no async, no clock, no
new workspace dependency.

**Acceptance:** two tests. The correct target passes. The faulted target fails **that** check and no
other.

~400 lines, and it answers the only question that matters: does an oracle derived from *this* model
catch a real defect in a real implementation? Everything in W4 §49 steps 1–3 is machinery for a thing
that has not yet bitten once.

### Then, in order

- **4a** — replace slice 1's hand-written scenario with a generated one. Scenario IR, synthesis as a
  fifth `Generator`, `Refused` as data, `Provenance` extended with `suite_version`, committed
  `generated/conformance/suite.json`, drift-checked by the existing `cargo xtask generate --check`.
- **4b** — the rest of the fault matrix, binding scenarios (success path; `escalate` refuses per C5),
  lifecycle scenarios (now possible after slice 0), `eventual` views, the full
  `EssConformanceReport`. New crate `ess-conformance`, and the H5 decision lands here.
- **4c** — `From<EssConformanceReport> for EssConformanceResult`, and rewire
  `crates/aep-engine/tests/end_to_end.rs:245` to a real report instead of two literals (M4). ~50 lines.

Then wave 5, whose §W5.9 acceptance — "run the existing ESS conformance suite unchanged" — is only
meaningful once 4b has proven the suite bites.
