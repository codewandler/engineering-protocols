# ESS wave 3.5 — reconciliation, and the gates wave 4 waits behind

> **Complete — all 20 gates closed.** `task check` is green with six steps: 45 suites, 1043
> tests, 0 clippy warnings, 0 rustdoc warnings, schemas and projections both up to date.

## Why a milestone between two waves

Wave 4 makes a specification into an oracle whose verdict decides whether ADP may call work complete.
That is a trust chain: the runner produces evidence, the engine consumes it, and a task closes or does
not. A trust chain is only as good as its weakest link, and two of the links below are currently
weaker than the design assumes — not in wave 4's unwritten code, but in what is already committed.

The other gates are cheaper and duller: the documents that tell an agent what this repository is have
drifted behind what it does. That matters more here than in an ordinary repository, because
`AGENTS.md` is read by every agent before it changes anything. Drift in a README is documentation
debt; drift in `AGENTS.md` is control-plane drift, and it produces work built on a false premise.

Three independent reviews fed this page: `docs/reviews/2026-08-20-full-repo-review.md` (this morning,
before wave 3 landed), `docs/reviews/Engineering Protocols Repository Review — Pre-Wave-4 Readiness.md`
(an outside review of `3647f80`, verdict *conditional go*), and a mutation-testing review whose
findings are the subject of gate G5.

## The gates

| # | gate | severity | blocks wave 4 | closed by |
|---|---|---|---|---|
| G1 | the control documents describe the repository that exists | high operationally | yes | **closed** — the drift list below is empty |
| G2 | `on_failure: escalate` has an observable consequence | high | yes | **closed** — `escalate:` names the event it emits; the bare word is refused as `missing_declaration` |
| G3 | wave 3's acceptance criterion says what is actually true | medium | no | **closed** — roadmap and tests agree |
| G4 | the wave 4 design is reconciled against the code and frozen | high | yes | **closed** (D1–D4 open by design) — the design's baseline is the real API and its refusal model is stated |
| G5 | the guards on the safety envelope actually guard | **critical** | yes | **closed** — all 7 survivors fail a test; the engine's three `== Granted` sites verified by mutation |
| G17 | a validated type cannot be conjured from a document | **critical** | yes | **closed** — a source scan over 10 `Raw*`→validated pairs, no new dependency |
| G18 | a number a document cannot round-trip is refused at the door | high | no | **closed** — non-finite refused in `new`, in `parse_literal`, and in a hand-written `Deserialize` |
| G19 | conformance evidence is bound to the specification *revision* it attests | high | no (blocks trusting wave 4's output) | **closed** — `Artifact::is_at_revision`, fail-closed, and the fail-open polarity verified by mutation |
| G20 | the published schema accepts what the parser accepts | high | no | **closed** — 17 aliases across 7 schemas; the test reads the serde attributes and checks both directions |
| G6 | property-test evidence can reproduce its own counterexample | medium | no (blocks the fuzz work) | **closed** — `Seed` on the run, an opaque string so no tool has to encode a lie |
| G7 | HEAD passes its own gate | — | **already closed** | `task check` exit 0; CI green on `3647f80` |
| G8 | MSRV and rustdoc are checked, not just declared | low | no | **closed** — MSRV job green on 1.85; rustdoc at 0 warnings and `doc-check` now in the gate |
| G9 | a document cannot choose the recursion depth of the parser | high | no (blocks the fuzz work) | **closed** — parse bounded at 32, measured 25x under the smallest overflow floor |
| G10 | `Version`'s two spellings agree | medium | no | **closed** — `Version::from_number` refuses what `parse` refuses; the schema bounds it too |
| G11 | ESS conformance evidence names the specification it attests | high | yes | **closed** — `SpecDigest` required on `EssConformanceResult`, with `attests()` |
| G12 | a conformance run is reproducible, not merely unslept | high | yes | **closed** — the runner's clock and id source are injected, as the engine's already are |
| G13 | a duplicated YAML key is refused on the AEP side too | medium | no | **closed** — `read_yaml` guards every AEP document kind, re-parsing so spans survive |
| G14 | a command can be tied to the transition and the entity it drives | **critical** | yes | **closed** — an outcome declares `creates:`/`moves:`/`updates:`; an uncaused transition is refused |
| G15 | the normative example can exercise what the oracle must prove | high | yes | **closed** — `examples/oracle-fixture/`, 11 tests, every fault row machine-derived |
| G16 | a candidate input can be evaluated against a predicate at all | high | yes (blocks witness synthesis) | **closed** — `ess-conformance` decides a guard against a candidate; `Unknown` refuses with a named reason |

---

### G1 — the control documents describe the repository that exists

Every item verified against the file as it stands.

| location | says | is |
|---|---|---|
| `README.md:122` | entities, views and actors "do not reach the IR, so no projection" renders them | they reach the IR and the documentation renders all three |
| `AGENTS.md` § Current state | `aep-conformance`, `adp-domain`, `aop-domain` are "skeletons with documented planned surfaces"; ESS is not mentioned | all three shipped in `0.2.0-wave-3`; three ESS crates exist and are gated |
| `AGENTS.md` § Gate | four steps | five since wave 3 added `generate-check` |
| `Taskfile.yml:10` | "Format check, lint, test and schema check" | the body runs five tasks |
| `docs/plan/ess-roadmap.md:3` | "Waves 1 and 2 delivered; 3 to 5 proposed" | wave 3 is delivered and tagged |
| `Cargo.toml:24` | `repository = "https://github.com/timofriedlberlin/engineering-protocols"` | the remote is `codewandler/engineering-protocols` |
| wave 4 design § 2 | `ess-gen` is local and unpushed; ESS is `ess-domain` + `ess-compiler` | `ess-gen` is on `main` |

The metadata one is worth a second look rather than a blind fix: a published crate's `repository`
field is what a consumer follows to report a bug, so pointing it at a path that is not the
authoritative remote sends every bug report somewhere nobody reads.

Two of these are mechanically checkable and should become checks rather than corrections, because a
document that drifted once will drift again. A wave number in the roadmap and a step count in a prose
paragraph are not: those stay human, and the mitigation is that this table exists.

### G2 — `on_failure: escalate` has an observable consequence

`examples/billing/components.yaml:56` declares `on_failure: escalate`. `Failure::Escalate`
(`crates/ess-domain/src/binding.rs:168`) documents it as "Surface it to a person". Nothing in the
model says what surfacing *is*: no event, no command, no view field, no state. So a conformance target
cannot be asked to prove escalation happened, and the flagship example contains a requirement the
flagship oracle cannot check.

The model already reasons about exactly this, one variant away. `Failure::Drop` carries the note that
losing work "is a decision, and the decision has to be findable in the document that made it"
(`binding.rs:171`), and `delivery:`/`on_failure:` are required words precisely so a binding cannot
fail silently. `Escalate` is the variant where that reasoning was not carried through: it names a
consequence outside the system and then says nothing about how the system shows it happened.

Fix the model, not the runner. The wave 4 design is already right that scenario synthesis must refuse
rather than invent an assertion, and the outside review is right that the repair must not be a
`was_escalated(binding)` hook on `ConformanceTarget` — that moves unspecified behaviour into the test
framework, which is the failure this repository exists to prevent.

Options, in preference order:

1. **`escalate` names a declared semantic action** — an event the system emits or a command it invokes
   on terminal binding failure. Escalation then has the same observability as anything else the model
   describes, and a scenario asserts that event. Cost: a construct in `ess-domain`, resolution in the
   compiler, a rendering in every projection, and an edit to the normative example.
2. **`escalate` requires an observable and refuses without one** — the word stays, but a binding using
   it must say what to observe. Same cost as 1, framed as a validation rule rather than a new field.
3. **Accept the hole and let synthesis refuse it**, recording the refusal in the generated suite. Cost:
   nearly nothing, but the normative example keeps a requirement the oracle reports as unverifiable,
   which is a poor advertisement for the central claim.

**Default if nobody decides: option 1**, because it is the one that makes the demo prove something.

### G3 — wave 3's acceptance criterion says what is actually true

`docs/plan/ess-roadmap.md` § W3.2 asks that OpenAPI and AsyncAPI be "validated against their own
schemas". What ships validates every schema the documents *embed* against the 2020-12 meta-schema — 39
fragments, by a real validator — and checks the envelopes structurally. The tag message and
`docs/plan/ess-wave-3-projections.md` both already say so plainly; the roadmap does not.

So this is milestone accounting, not a defect: no reader is being misled by the code, only by the
roadmap. The outside review's principle is right — never call a wave complete under a criterion its
own tests say is unmet — and the cheap resolution is to amend the criterion to what is delivered and
record the meta-schemas as deferred with the vendoring decision attached.

**Default: amend the roadmap.** Vendoring two third-party meta-schemas (~100 KB, licence, pinning, an
update path) was already weighed and declined; nothing since changes that.

### G4 — the wave 4 design is reconciled against the code and frozen

Beyond the stale baseline in G1, three reconciliations change what gets built:

**Scenario synthesis is fallible, and is not an `ess_gen::Generator`.** `ess-gen`'s trait is
deliberately infallible for a valid IR — a construct it cannot render is a defect in the crate, not an
outcome. Synthesis is the opposite: refusing is a legitimate, expected result whenever no safe witness
exists, a failure policy is unobservable (G2), or a valid predicate is not constructively satisfiable.
Those two contracts do not belong behind one trait, and the refusals must be typed and diagnostic
rather than a silent omission.

**Synthesis consumes the decisions the compiler already made.** `ResolvedOutcome::test_strategy`
(`crates/ess-compiler/src/ir.rs:484`) and `ResolvedView::assertion_style` (`ir.rs:602`) exist so that no
generator decides independently whether a branch is reachable by constructing an input, or whether a
view assertion must be retried. Both doc comments say why: a decision made per projection is a
decision made wrong eventually. If synthesis asks those questions again, that is a regression.

**A serialized suite identifies semantic objects, not handles.** Handles are valid only inside one
`EssIr`. A committed `ConformanceSuite` outlives the process that produced it, so it must carry stable
ESS identity. Related: `generator_version` in the provenance needs one unambiguous meaning once the
synthesizer is a separate thing from `ess-gen` — reproducibility means knowing which oracle produced a
verdict.

The feasibility review in `docs/reviews/` is the input to this gate; freeze the design only after its
findings are folded in.

### G5 — the guards on the safety envelope actually guard

A mutation review ran 20 single-edit mutations against the load-bearing rules — the mistake a competent
engineer would actually make, one at a time, reverted between each. **13 were caught, 7 survived, and 2
survived all five steps of `task check`.** Six of the seven live in the approval path. Full detail and
per-mutation evidence in `docs/reviews/2026-08-20-guard-efficacy-review.md`.

| # | what breaking it means for a person | survived |
|---|---|---|
| M4 | a reviewer reads a change, **refuses** it, records the refusal — and has thereby granted the production write | the whole gate |
| M3 | a profile spelling `allow: [deployment.create]` ships to production with nobody approving; the careful spelling `deployment.create:production` is protected, the lazy one is not | tests |
| M1 | a principle that denies production writes is silently downgraded to "ask someone" — and someone can always be asked | tests |
| M8, M9 | a guard written `not deployment.failed` passes **because no verifier has run**: `Truth::not()` has no test at all, so `not Unknown` collapsing to `True` is unobserved | tests |
| M6 | the audit trail accepts a record that says "refused" and lists the rows it changed — invariant 15, stated and unguarded | tests |

The class, named because it is the one hand-written example tests systematically miss: **the rule is
correct, its doc comment states it, and no fixture anywhere constructs the state in which the rule is
load-bearing.** `deny_beats_approval_which_beats_allow` is the clearest case — nothing in `crates/`,
`profiles/`, `principles/` or `protocols/` puts one capability in both `deny` and `approval_required`, so
the middle link of the chain that test is named after has no observable state anywhere in the repository. A
test can assert a rule it cannot see.

One thing this makes newly clear, and it is uncomfortable: **the two `xtask` drift checks are drift
detectors, not contract guards.** Both compare generated output against the code that generated it, so
weakening the shared type mapping and then running the `cargo xtask generate` the error message tells you
to run turns both green. The same held after deleting an invariant from the normative example. In each case
the only surviving guard was a single hand-written assertion. That is not an argument against the checks —
they catch the drift they were built for, and they caught real drift twice today — but they must not be
counted as protection for the contract's *content*.

Calibration matters as much as the failures, so: `tests/agreement.rs`, the conformance `identity` suite,
`tests/faults.rs` and both orphan scans all bit hard, with the best failure messages in the repository.
`faults.rs` verifies its own `caught_by()` claim mechanically, so that mapping is not luck.

### G17 — a validated type cannot be conjured from a document

Invariant 2 is the load-bearing one in the whole design: a document deserializes into a `Raw*` type and
becomes a domain type through `TryFrom`, and **validated types do not implement `Deserialize`, so the only
way to obtain one is to validate**. Adding `Deserialize` to a validated type — the exact thing the
invariant forbids, and the exact shortcut a tired engineer takes to save a conversion — compiles, passes
all 916 tests, passes clippy, and passes the whole five-step gate. Nothing anywhere enforces it: no
`trybuild` case, no source scan, no test.

The consequence is the one the invariant exists to prevent: anyone can hand the engine a `Protocol` that
was never validated, and every rule downstream is then reasoning about a value whose invariants were never
checked. Every other guarantee in this repository sits on top of that one.

There is a precedent for enforcing a rule like this mechanically rather than by review:
`crates/ess-compiler/tests/billing.rs:275` scans every source file in the crate for banned tokens
(`HashMap`, `SystemTime`, `rand::`) with the reasoning that a compile-twice test cannot catch either
failure mode. A scan for `Deserialize` on the validated types, or a `trybuild` case per type, is the same
shape of answer. Note the scan there covers only `ess-compiler/src`; `ess-domain` and `ess-gen` have no
equivalent, which is a second gap in the same mechanism.

### G6 — property-test evidence can reproduce its own counterexample

The protocol already models this class of verification: `Verifier::PropertyTester`,
`EvidenceKind::PropertyTestResult`, `TestSuite::{Property, Fuzz, Mutation, Differential}`, and
`Counterexample { verifier, property, input, expected, observed, note }`. But `PropertyTestResult`
(`crates/aep-domain/src/evidence.rs:281`) records `cases: usize` and no seed. A property-test result
this protocol accepts as evidence therefore cannot be re-run to reproduce the counterexample it
reports — which is the one thing anybody wants from such a result.

Not a wave 4 blocker: wave 4's suites are deterministic by design (§37 requires byte-identical
generation and no RNG). It blocks the property-based work below, and it is a small field addition.

### G8 — MSRV and rustdoc are checked, not just declared

**MSRV: closed.** A job on a pinned `1.85.0` toolchain runs `cargo build --workspace --locked`,
verified green on a clean checkout of HEAD (16s cold). It is a job rather than a step because it is a
different toolchain, which a step in `gate` cannot be — so the mirroring rule at `ci.yml:1-4` is
satisfied the same way `schemas` and `projections` satisfy it, and no `Taskfile` change is owed.

It builds and does not test, and that scoping is the decision rather than a shortcut. `cargo test
--workspace` on 1.85 does not compile and fail — it refuses to resolve at all, because the
dev-dependency `jsonschema` pulls `idna 1.1` → `idna_adapter 1.2.2` (needs 1.86) → `icu_* 2.3` (needs
1.88). **`rust-version` is a promise to whoever consumes the published crates, and a consumer never
builds this workspace's dev-dependencies.** So the job checks exactly the surface the promise covers:
every lib and bin. Making the tests run on 1.85 would mean pinning those crates back or raising the
declared MSRV, which is a decision about what this repository supports — recorded here rather than
settled by CI.

No `rust-toolchain.toml`: pinning 1.85 there would make `cargo test` fail workspace-wide for everyone,
pinning `stable` is a no-op, and either would override all three existing jobs and invalidate every
`rust-cache` key.

**Rustdoc: blocked on the warnings it exists to catch.** The step is written, sits last in `gate`, and
costs about two seconds because it reuses the dependency build `Test` already paid for. It lands red:
**35 warnings across 11 files and 8 crates** — 17 redundant explicit link targets, 7 links from public
documentation into private items, one `resolve` that is both a function and a module
(`crates/aep-engine/src/lib.rs:29`), and one genuinely broken link at `crates/ess-gen/src/openapi.rs:175`
(`BTreeSet`, not in scope).

Clippy already denies `missing_docs`, so this is not that. What only rustdoc sees is the other half of
invariant 11: a link that resolves to nothing, and public documentation pointing at an item a reader
cannot reach. In a repository whose doc comments carry the design reasoning, a link that goes nowhere
loses an argument rather than a hyperlink — which is why the one real broken link matters more than the
seventeen cosmetic ones.

`task doc-check` is defined and deliberately **not** wired into `task check` yet, because wiring it in
while HEAD has 35 warnings hands every contributor a red gate they did not cause. It goes into `check`
in the same change that clears the count.

### G18 — a number a document cannot round-trip is refused at the door

Found while auditing the six "serialisation cannot fail" panics, which turned out to be unreachable —
but the audit found something worse next to them, because it is silent. `Number::new`
(`crates/aep-domain/src/facts.rs`) refused NaN and accepted the infinities, and `FactValue::parse_literal`
checked `!is_nan()` for the same reason. `1e400` does not fail to parse as an `f64`; it overflows to
`INFINITY`. JSON has no spelling for an infinity, so `serde_json` writes it as **`null`**.

So a guard written `amount >= 1e400` was published in the IR as `any_of: [null]` — not a crash, not a
refusal, and indistinguishable downstream from a deliberate null. The author's value was replaced by a
different value with a different meaning, and every check in this repository passed.

A second hole sat beside it: `Number` derived `Deserialize` under `#[serde(transparent)]`, which reads
straight into the field and never calls the constructor. `.nan` in a document therefore produced a
`Number` whose own doc comment says it cannot exist — and a NaN makes the `Ord` implementation a lie for
every value compared against it, which is load-bearing for a type used in predicate comparison.

Both refused now: `is_finite` in the constructor and in `parse_literal`, and a hand-written
`Deserialize` that routes through the constructor. An unrepresentable literal stays the text the author
typed, which is both truthful and still comparable. Three tests, each verified by re-introducing the
defect and watching it fail.

### G19 — conformance evidence is bound to the specification *revision* it attests

G11 closed the first half: an `EssConformanceResult` must carry a `SpecDigest`, and evidence naming one
specification no longer satisfies a requirement about another. What is still unchecked is the digest
itself. Evidence can name the right specification and carry a digest from an older revision of it, and
nothing compares the two — because the artifact graph has no digest to compare against.

The consequence is narrow but exactly the one wave 4 exists to prevent: a suite run against
yesterday's specification produces evidence that satisfies today's requirement, and the task closes
having proven conformance to a specification nobody is building against any more. That is the same
defect class as an approval of version 3 covering version 7, which this repository already refuses —
`ReviewRequirement::evaluate` calls `review.covers(artifact)` precisely so an approval cannot outlive
the revision it approved.

The fix mirrors it: `Artifact` carries the model digest, and `EvidenceRequirement` checks the
evidence's digest against it. Two files in `aep-domain`, and the shipped
`principles/verification/ess-conformance.yaml` then has something real to require. Deliberately not
bodged in the meantime: adding an `ess_conformance.spec_digest.exists` predicate to that principle
would pass on every well-formed record, since G11 made the field required — a check that cannot fail
is the thing the guard-efficacy review spent the day finding.

Not blocking wave 4's *implementation*, because the runner can be built and the evidence produced
before the binding is enforced. Blocking anyone *trusting* the result.

### G20 — the published schema accepts what the parser accepts

`schemas/generated/ess.schema.json` **rejects `examples/billing/components.yaml`** — six errors,
reproduced independently with a conforming validator. It demands `name:` on a component and a binding,
while the parser accepts `component:` and `id:` and the normative example uses exactly those.

The cause is one line of type-level drift: `#[serde(alias = "component")]` on
`crates/ess-domain/src/component.rs` is invisible to `schemars`, so the generated schema describes the
canonical spelling only. `AGENTS.md` records that wire-format aliases are deliberate — both spellings
appear in the design documents, and aliases are accepted on input on purpose. The schema does not know
that, so it publishes half the language.

The consequence lands on the one person this artifact exists for. `docs/guide/specification.md:264`
tells an author to point their editor at this schema; doing so marks the normative example as invalid
and offers no fix, because the spelling it objects to is the spelling the guide's own examples use.

**The guard that should have caught it excluded the evidence.**
`crates/aep-schema/tests/published.rs:60` iterates a hard-coded list of three files —
`system.yaml`, `domains/invoice.yaml`, `domains/email.yaml` — and `components.yaml` is simply not in
it. A test named *the specification schema accepts the normative example* checks three fifths of the
normative example. That is the same shape as every survived mutation this milestone found: the
assertion is real, and the state where it would fail is unreachable from the fixture.

This exact defect class is recorded in this repository already, in
`crates/ess-gen/tests/schema.rs`'s module doc: *"this repository has published a schema that rejected
its own normative example, and … one that described a Rust representation rather than what an author
writes. Both were well formed. Both passed every check that only asked whether the output parsed."* It
has now happened a third time, in the one place that comment was not looking.

Two things to fix, and the second matters more: the schema should describe both spellings, since both
are legal input; and the test must **discover** the example's files rather than list them, so a file
added to the example cannot be silently exempt. `crates/ess-compiler/tests/billing.rs` already
discovers rather than lists, and says why.

## The property-based work, and where it sits

Decided separately: `proptest` on stable, framework first. It is not a wave 4 gate — it is the answer
to the class of defect G5 is made of, and G5's mutations are the argument for it. Two phases:

**Phase 1, harden this repository.** Generate adversarial *specifications* and assert the properties
that must hold for all of them: no panic, no non-termination, a refusal rather than an acceptance, and
byte-identical output on a second run. The generator must live in `tests/` or its own crate, never in
`src` — `crates/ess-compiler/tests/billing.rs:286` mechanically bans the token `rand::` under
`ess-compiler/src`, and that guard is worth keeping. The `proptest` dev-dependency must carry a
justification comment of the standard set by `crates/ess-gen/Cargo.toml:20`.

**Phase 2, witness synthesis for wave 4.** Gated on G16, which is where the optimism goes. Generate-and-
filter is the right shape — keep candidates whose `Predicate::evaluate` returns `True` and discard
`False`, no constraint solver — but the precondition is a flattener from a candidate input to a
`FactSource`, and that does not exist yet.

**Corrected after wave 4's synthesizer landed:** an earlier draft of this paragraph said to discard
`False` *and* `Unknown`, which reads as treating them alike — both meaning "try another candidate".
That is wrong, and it is the collapse invariant 5 forbids. `False` means this value does not satisfy
the guard, so another value might. `Unknown` means the guard cannot be decided at all — no scale
orders those two texts, or the path names nothing — and no candidate repairs that, so it **refuses**,
naming the predicate and the missing path. Retrying on `Unknown` would spend the whole budget on a
specification defect and then report it as a flaky test. The design says refuse; the implementation
refuses; this page was the only place that said otherwise. Add `proptest`'s shrinking to it and a failing witness arrives
minimal, which is the difference between a counterexample a person acts on and one they re-derive.

Two hazards specific to this codebase, both from the sweep: a handle from one `EssIr` used against another
panics *by design* (`crates/ess-compiler/src/ir.rs:141`), so a generator that mixes two compilations will
look like a crash rather than a mistake; and `Operand::parse`'s dot heuristic
(`crates/aep-domain/src/predicate.rs:225`) reads any unquoted bare word containing a dot as a fact path, so
a generated literal like `en.US` silently becomes a path lookup.

Phase 1 has a ready-made property, generalising the byte-identical test at
`crates/ess-compiler/tests/billing.rs:256`: **any generated document either yields at least one
`ValidationCode`, or compiles and re-serialises identically.** No panic, no hang, no third outcome.

## Decisions taken

Ten, on 2026-08-20, so the defaults on this page stop being provisional. Recorded because a default
that was never chosen and a default that was chosen look identical six weeks later.

| # | decision | taken |
|---|---|---|
| 1 | `on_failure: escalate` **emits a declared event** | the smallest change that makes escalation observable by the same mechanism as every other published fact. Not "invokes a command": that is a second binding in all but name, and the extra power buys nothing the oracle needs yet |
| 2 | **ESS wave 4, then semantic diff** | the semantic-diff design says it can start as soon as `EssIr` is stable, which is now — but the oracle is what makes every other claim checkable, so it goes first |
| 3 | the conformance runner is a **new crate** | `ess-gen`'s `Generator` is infallible by contract and the crate documents that it holds no clock. A runner is fallible and takes a clock; putting it there falsifies both claims |
| 4 | the async runtime question stays open until implementation | `block_on` busy-polls a million times then panics, which is right for a future that never yields and unusable against a real target. The answer depends on the trait shape, which does not exist yet |
| 5 | MSRV is a **build-only** contract | `rust-version` is a promise to whoever consumes the published crates, and a consumer never builds this workspace's dev-dependencies |
| 6 | correct the VISION trust claim | attestation is a real feature that does not exist; the sentence claiming it does is the one the document turns on |
| 7 | this project **generates deployment artifacts and does not deploy** | deploying stays an explicitly optional thing that could come much later. The infrastructure design made the old one-line boundary ambiguous |
| 8 | **review the four proposed designs before wave 4 starts**, after the control documents are updated and committed | two of the four were written against the roadmap rather than the code, which already cost one reconciliation pass |
| 9 | G15 gets a **second fixture**, not a larger normative example | the billing example should stay readable as a specification of a real system rather than a corner-case museum |
| 10 | wave 3.5 gets **its own tag** | nineteen gates and four reviews is a wave by any measure |

Defaulted rather than debated, and still open to reversal: no meta-schema vendoring, no `schemaFormat` on
`AsyncAPI`, and the property-based work runs after the model changes rather than beside them.

## Schedule

Batched by which files a change touches, because the two model changes touch the model, the compiler,
every projection and the committed tree — running them beside anything else is how two correct diffs
produce one broken tree.

| batch | gates | why together |
|---|---|---|
| **A1** | G1, G3, G4, G12 | prose and design text only: `README`, `AGENTS.md`, `Taskfile` description, roadmap, manifest metadata, the wave 4 design |
| **A2** | G9 | the recursion bounds and their refusal codes, across `ess-domain`, `predicate.rs`, the compiler and two projection walkers |
| **A3** | G10, G13, G6, G11 | four small model and parser corrections in `name.rs`, `aep-schema`, `evidence.rs` and the engine fixture |
| **A4** | G8 | one file, `.github/workflows/ci.yml` |
| **A5** | G5, G17 | the mutation review has landed and named all 7 survivors, so this is now a test-writing task with a fixed list |
| **B1** | G14 | command → transition and command → entity: `ess-domain`, resolver, IR, every projection, the example, the committed tree |
| **B2** | G2 | `escalate` gains an observable consequence — same files as B1, so it follows rather than accompanies it |
| **B3** | G15 | the oracle fixture, once B1 and B2 have given it the constructs to use |
| **B4** | G16 | the input-to-`FactSource` flattener, once B1 fixes what a command is attached to |

A runs in parallel; B is serial and starts when A is green. Phase 1 of the property work follows A2,
because bounding the recursion is what makes "no panic, no hang" a property worth asserting rather than
one that fails on the first generated document. Phase 2 follows B4.

Wave 5 stays gated on wave 4 closing its loop, both halves: a correct target producing valid evidence
that lets ADP complete, and a faulty target producing a failure that refuses completion.

Wave 5 stays gated on wave 4 closing its loop, both halves: a correct target producing valid evidence
that lets ADP complete, and a faulty target producing a failure that refuses completion.

## Not in this milestone

Deep predicate solving, model checking, transport-level conformance targets, structural synthesis,
obligations, the linker model, `cargo-deny` and release automation. The outside review lists the same
set and the reason is the same: none of them is on the path to proving one specification can act as an
oracle.
