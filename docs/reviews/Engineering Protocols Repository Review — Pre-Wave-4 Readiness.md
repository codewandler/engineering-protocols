# Engineering Protocols Repository Review — Pre-Wave-4 Readiness

**Review target:** `codewandler/engineering-protocols`, public `main` at `3647f80`  
**Review date:** 20 August 2026  
**Purpose:** Determine whether the repository is structurally and semantically ready to begin ESS Wave 4: closed-loop execution and conformance.

## Executive verdict

**Overall assessment: CONDITIONAL GO.**

The repository is in strong architectural shape. AEP has a coherent semantic core, command/query contract, deterministic engine, reference backend and unusually good black-box conformance machinery. ESS has progressed from a model into a normalized IR and deterministic projection system without collapsing semantic concepts into transport-specific representations. The central architecture is holding together.

I would **not begin Wave 4 implementation on the current commit**, however.

Before Wave 4 starts, I recommend one small, explicit **“pre-Wave-4 reconciliation” milestone** that closes four issues:

1. **Resolve a real semantic gap around binding failure escalation.** The normative billing ESS says `on_failure: escalate`, but ESS does not currently define an observable semantic consequence by which conformance can prove that escalation happened.
2. **Review and reconcile the Wave 4 design against the actual post-Wave-3 API.** The committed design explicitly says it was filed unreviewed and still contains the obsolete assumption that `ess-gen` is local/unpushed.
3. **Close or consciously amend the Wave 3 acceptance gap around OpenAPI/AsyncAPI meta-schema validation.** The roadmap promises validation against their own schemas, while the projection tests explicitly document that this is not fully true.
4. **Repair repository control-document drift** in `README`, `AGENTS.md`, the ESS roadmap, and package metadata before another agent/design cycle starts.

These are bounded changes. They do **not** imply redesigning AEP, ESS, the compiler, or `ess-gen`.

My recommended sequence is therefore:

> **Reconciliation → freeze Wave 4 design → Wave 4 implementation → falsification → ADP closure → only then Wave 5.**

The original handoff described the same intended closed loop—ESS → scenario synthesis → canonical suite → real target → conformance report → independent `EssConformance` evidence → ADP completion. The remote has now completed the projection work that the handoff still described as local, but it has not yet crossed that closed-loop milestone.

---

# 1. Review scope and confidence

This was an architecture-wide source review rather than a superficial README review. I examined the current repository organization and build metadata, AEP domain/contract/engine/conformance/evidence paths, ESS domain semantics, compiler/normalized IR, `ess-gen` and selected projection tests, the normative billing fixture, generated-artifact/drift approach, CLI/xtask/CI setup, repository guidance documents, the prior review history, and the newly committed Wave 4 and Wave 5 designs.

I did **not** have a local checkout from which I could personally execute every Rust test. Consequently, test counts reported by the repository are treated as repository claims, not as tests I independently executed. The current GitHub Actions run for HEAD `3647f80` was also still displayed as **in progress** at the time of this review.

That means there are two separate readiness questions:

- **Architectural readiness:** high, subject to the findings below.
- **Build-state readiness of HEAD:** not yet independently established by a completed HEAD CI result.

---

# 2. Repository scorecard

| Area | Assessment | Pre-Wave-4 status |
|---|---|---|
| AEP semantic/domain model | **A** | Ready |
| AEP command/query contract | **A** | Ready |
| AEP deterministic engine | **A** | Ready |
| AEP backend conformance | **A** | Ready; excellent Wave-4 precedent |
| AEP/ADP evidence bridge | **A-** | Ready with one naming/version reconciliation |
| ESS domain model | **A-** | One important failure-observability gap |
| ESS compiler / normalized IR | **A-** | Ready with intentionally bounded predicate limitations |
| ESS projections / `ess-gen` | **B+/A-** | Implementation strong; acceptance claim needs reconciliation |
| Generated-artifact discipline | **A** | Ready |
| CLI / xtask / local gates | **A-** | Ready |
| CI | **A-** | Good structure; add MSRV/docs coverage |
| Repository documentation/governance | **C+** | Must reconcile before Wave 4 |
| Wave 4 design | **B, provisional** | Strong concepts, but explicitly unreviewed/stale |
| Wave 4 overall readiness | **B / Conditional GO** | Reconciliation first |

---

# 3. What is working particularly well

## 3.1 The architecture is still respecting the central thesis

The strongest repository-level result is that the system has not drifted into “generate things directly from YAML.”

The direction remains:

**semantic ESS → validated/resolved IR → deterministic consumers**

rather than:

**source syntax → ad-hoc generators**

That distinction becomes increasingly important now that ESS is moving from documentation generation to executable semantics.

The Wave 4 design itself continues the same principle: conformance scenarios are synthesized from `EssIr`; projection generation remains a sibling consumer rather than the conformance oracle.

This is exactly the separation the handoff intended: semantic concepts are primary, while OpenAPI, AsyncAPI, Rust, Kubernetes and tests are projections.

I would preserve this boundary aggressively.

## 3.2 AEP's backend conformance architecture is the strongest precedent for Wave 4

`aep-conformance` already demonstrates a valuable pattern:

- black-box testing;
- deliberate fault injection;
- faults designed to be observably wrong;
- explicit mapping from each fault to the suite/property that should catch it;
- reports expressed in semantic properties rather than implementation details.

The faulty backend includes perturbations around idempotency, stale revisions, affected objects, events, audit, correlation, causation, provenance, stale reads, filters, history, relations, deletion/archive semantics and type discovery.

The corresponding reporting model records semantic suite/property results rather than merely surfacing a pile of assertion failures.

That is precisely the mentality Wave 4 needs.

Wave 4 should therefore **reuse the pattern, not necessarily the code**:

> correct implementation passes; one deliberately wrong semantic behavior fails the intended scenario; diagnostics say which ESS property was falsified.

This is stronger than ordinary generated-test coverage.

## 3.3 ESS already contains test-intent semantics that Wave 4 should consume

Several important decisions have already been made in the model/compiler, which reduces how much Wave 4 should invent.

Command outcomes distinguish conditions such as:

- ordinary `when` predicates;
- `otherwise`;
- externally determined outcomes.

External outcomes deliberately encode the fact that input alone cannot select the result and that a conformance environment must control the external result.

Views similarly carry consistency semantics and map those into assertion styles such as immediate expectation versus eventual expectation.

The normalized IR stores these resolved decisions—such as outcome test strategy and view assertion style—rather than requiring every downstream generator to recompute them.

This is excellent.

It also creates an important Wave 4 rule:

> **Scenario synthesis should consume the test strategy and assertion style already in `EssIr`. It must not introduce a second decision engine.**

If Wave 4 code starts asking independently, “is this an external outcome?” or “should this view be polled?”, that would be architectural regression.

## 3.4 The normative billing fixture is small but semantically useful

The billing specification is a good reference target because it gives Wave 4 several qualitatively different things to prove.

The invoice side contains state/lifecycle behavior, guarded acceptance/rejection, emitted events and both eventual and read-your-writes views. The email side includes an externally controlled command outcome, which is exactly what is needed to prove that conformance testing can distinguish semantic input conditions from environmental effects.

The fixture is not merely a “hello world.” It is sufficient to exercise the core closed-loop architecture—provided the failure-policy gap discussed below is fixed.

## 3.5 `ess-gen` keeps projection policy visibly separate from ESS semantics

The OpenAPI generator is refreshingly explicit that an ESS command is **not inherently an HTTP endpoint**.

Where ESS does not specify a transport choice, the generator documents conventions such as HTTP method, generated path and response/status mappings as generator policy. It also explicitly refuses to imply semantics ESS did not provide—for example, views without declared query exposure are not magically exposed through HTTP.

This is exactly right.

It gives Wave 4 another hard boundary:

> **ESS semantic conformance must not use generated OpenAPI/AsyncAPI conventions as the semantic oracle.**

For example, whether an HTTP generator chose `422` versus another status is not the same question as whether `CreateInvoice` semantically produced `InvalidAmount`.

Transport projection conformance can exist later. It should not be confused with ESS conformance.

## 3.6 Generated artifact ownership and drift checks are strong

The repository now has explicit generation/check paths rather than relying on maintainers to notice generated drift manually. Local task orchestration includes formatting, Clippy, tests, schema drift and projection drift checks, and CI separates important generated-artifact checks into visible jobs.

That matters for Wave 4 because canonical scenarios will themselves become generated semantic artifacts. The existing discipline gives a clear precedent for deterministic generation, committed fixtures where appropriate, and drift detection.

---

# 4. High-priority finding: `escalate` is currently not semantically observable

**Severity: HIGH**  
**Classification: semantic blocker for full Wave 4 acceptance**

This is the most important finding in the review.

The normative billing component declares a binding with delivery semantics and:

`on_failure: escalate`



The ESS domain represents failure policy using values including `Retry`, `Escalate`, and `Drop`; `Escalate` is described conceptually as surfacing the failure to a person.

But there is no declared semantic consequence in the billing ESS that a black-box conformance target can observe to prove:

> “the failure was escalated.”

There is no declared escalation command, event, view state, outcome or other semantic observation tied to that policy.

The Wave 4 design correctly recognizes this exact problem. It says that testing an `on_failure: escalate` binding requires forcing a downstream failure and checking an observation derived from ESS; if ESS has no observable representation of escalation, **scenario synthesis must refuse instead of inventing an assertion**.

That means the current normative fixture contains a behavior that the intended canonical oracle cannot fully verify.

### Recommendation

Fix the **model**, not the runner.

The right answer is not to let `ConformanceTarget` expose a magic method such as:

`was_escalated(binding_id)`

unless escalation itself is part of ESS semantics. That would move unspecified behavior into the test framework and violate the project's own architecture.

Instead, make failure policy semantically addressable.

One possible direction—not a prescribed final syntax—is for an escalation policy to reference a declared semantic action, for example an emitted domain/system event or a command that is invoked on terminal binding failure.

Conceptually:

```text
binding failure
    → declared semantic escalation action
    → observable event/command/state
```

The important requirement is not the YAML shape. It is:

> **ESS must say what observable semantic fact constitutes successful escalation.**

Do this before writing scenario synthesis for binding failure.

---

# 5. High-priority finding: Wave 4 design needs formal reconciliation before implementation

**Severity: HIGH**  
**Classification: process + architecture**

HEAD itself says the Wave 4 and Wave 5 documents were committed **unreviewed** and deliberately not acted on. The commit message specifically says the first design must be reconciled against what the repository actually built.

The design still says that `ess-gen` exists locally but has not been pushed, and its repository baseline lists only `ess-domain` and `ess-compiler` under ESS. That is now objectively stale.

This is not merely editorial because the actual code already settled several abstractions the proposed design discusses.

### Most important reconciliation: scenario synthesis must be fallible

`ess-gen`'s current `Generator` abstraction is intentionally effectively infallible for a valid `EssIr`: unsupported valid ESS is treated as a generator defect rather than an ordinary generation outcome.

Wave 4 is different.

Scenario synthesis explicitly needs legitimate refusal cases:

- a safe witness cannot be derived;
- a required failure behavior is not observably represented;
- a predicate is valid but not constructively satisfiable by the current synthesizer;
- semantic information is insufficient to create a sound test.

Therefore I strongly recommend **not reusing `ess_gen::Generator` as the Wave 4 synthesis abstraction**.

Create a separate semantic conformance-synthesis boundary directly over `EssIr`.

Conceptually:

```text
EssIr
  │
  ├── ess-gen
  │     deterministic projections
  │     valid IR → artifact set
  │
  └── scenario synthesis
        semantic oracle generation
        valid IR → suite OR typed refusals
```

Its result should support deterministic, structured diagnostics rather than `todo!()`, silent omissions or guessed witnesses.

For example:

```text
synthesize(EssIr)
    -> ConformanceSuite
    OR
       ScenarioSynthesisDiagnostics
```

Whether this lives in `ess-conformance`, `ess-scenario`, or another crate is less important than keeping its contract distinct from ordinary artifact projection.

### Existing IR decisions must be reused

During reconciliation, update the design around the actual `ResolvedOutcome::test_strategy` and resolved view assertion semantics already produced by the compiler.

Do not duplicate those decisions in scenario generation.

---

# 6. High-priority finding: Wave 3 is marked complete despite an explicit acceptance gap

**Severity: MEDIUM-HIGH**  
**Classification: milestone integrity**

The ESS roadmap's Wave 3 projection acceptance language says generated OpenAPI and AsyncAPI should be validated against their respective schemas, alongside deterministic generation.

The implementation has good validation coverage, but its own tests explicitly document a distinction:

- embedded JSON Schemas are validated as real schemas;
- the OpenAPI envelope is hand-checked rather than validated using a vendored/pinned full OpenAPI 3.1 meta-schema;
- AsyncAPI likewise uses structural checking and explicitly notes that the corresponding “validated against its own schema” roadmap criterion is not met without an AsyncAPI 3 meta-schema.

At the same time, repository status presents Wave 3/projections as complete.

This is a mismatch between **delivered behavior** and **declared acceptance criteria**.

It does not make `ess-gen` poor. The test comments are actually a sign of engineering discipline: the implementation is honest about what it proves.

The problem is milestone accounting.

### Recommendation

Choose one of two explicit resolutions before Wave 4:

**Preferred:** vendor/pin appropriate OpenAPI 3.1 and AsyncAPI 3.0 meta-schemas and validate generated documents against them.

**Acceptable:** amend the Wave 3 acceptance criterion and status documents to say exactly what is validated today, recording full meta-schema conformance as deferred work.

Given this project's emphasis on independent falsifiability, I favor completing the validation and preserving the stronger acceptance criterion.

The principle should be:

> **Never call a wave complete under an acceptance criterion that its own tests say is unmet.**

That becomes even more important once Wave 4 produces conformance evidence that other protocols trust.

---

# 7. High-priority finding: repository control documents have drifted behind implementation

**Severity: HIGH operationally, LOW technically**  
**Classification: fix before another agent-driven wave**

There is meaningful drift in documents that are supposed to govern future development.

### `AGENTS.md`

`AGENTS.md` instructs an agent to read it before changing the repository and establishes important invariants.

Those invariants are strong: deterministic collections, no unsafe Rust, command-only mutation, audit of refused commands, stable diagnostics, evidence independence and other rules.

But its “current state” section is materially older than the repository. It describes AEP portions as skeletons and does not reflect the delivered ESS stack.

Its documented gate also no longer completely matches the actual local task gate because projection drift generation/checking has since been added.

For a repository intended to be worked on by agents, this is not cosmetic documentation debt. It is **control-plane drift**.

### `README.md`

The README status table reflects much of the current Wave 3 state, but a later statement says entities, views and actors do not reach the IR / projections. That no longer matches the current resolved IR and projection code.

### ESS roadmap

The roadmap still describes Waves 1 and 2 as delivered and Wave 3 onward as proposed, despite Wave 3 now existing in the repository. It also contains stale sequencing language around structural synthesis.

### Wave 4 design

As already discussed, the design still assumes unpushed `ess-gen`.

### Recommendation

Make repository/document reconciliation the first commit before Wave 4 implementation:

- update `AGENTS.md` current state;
- make its gate match `task check`;
- remove stale README statements;
- update the roadmap to record Wave 3 accurately;
- mark Wave 4 design **reviewed/accepted** only after the semantic/API changes are incorporated;
- ensure Wave 5 remains explicitly gated on successful Wave 4 closure.

This should be treated as a **single source-of-truth restoration**, not a prose-polish exercise.

---

# 8. ESS compiler / IR review

## Assessment: A-

The normalized IR is a strong foundation for the next wave.

Its major architectural win is that identities and references are resolved into handles, consumers get normalized lookup operations, and downstream layers are prevented from repeatedly interpreting source-level names.

The IR also intentionally does **not** pretend to have solved every semantic reasoning problem.

One important example is predicates: predicate structures remain available, but leaf paths are not fully resolved into a rich typed path IR, and deeper paths remain an acknowledged area rather than being guessed by the compiler.

That is acceptable for the current stage.

### Wave 4 implication

Do not turn scenario synthesis into an accidental second compiler.

For Wave 4 v0.1:

- use explicit fixtures when available;
- synthesize trivial/direct witnesses where safety is obvious;
- support the normative predicates deliberately;
- produce a typed refusal when a valid predicate cannot be safely witnessed.

Do **not** write a general-purpose hidden constraint solver inside the conformance generator merely to make more tests appear.

If later ESS needs constructive value generation, give that capability an explicit semantic model/compiler abstraction.

This follows the project thesis much better than quietly generating arbitrary values until a predicate happens to pass.

### IR handle caution

Resolved handles are appropriate internal identifiers tied to one `EssIr`, but canonical `ConformanceSuite` artifacts should remain portable/serializable.

Scenario IR should therefore identify semantic objects using stable ESS identity/locators rather than serializing raw in-memory handles whose validity depends on a particular `EssIr` instance.

---

# 9. ESS domain model review

## Assessment: A- pending escalation fix

The model has several strong characteristics:

- semantic commands separated from transport;
- explicit outcomes and declared errors;
- external outcomes represented distinctly;
- domain events first-class;
- views with declared consistency;
- state machines and legal transitions;
- typed predicates/invariants;
- components;
- typed bindings;
- topology;
- actors/roles;
- identity and wire naming.

The domain is successfully becoming a **language of software semantics**, rather than an API-description format.

The main reason I do not rate it A today is the failure-policy observability issue.

More generally, Wave 4 will expose exactly these kinds of gaps, and that is desirable.

The rule should be:

> If the canonical oracle cannot express a meaningful assertion without inventing semantics, improve ESS rather than weaken the oracle.

The Wave 4 design already states this principle.

---

# 10. `ess-gen` review

## Assessment: B+/A-

The generator layer has a good ownership model:

- one normalized input;
- deterministic artifacts;
- clear generated paths;
- provenance;
- drift checking;
- projection-specific policy made visible rather than disguised as ESS semantics.

The generated OpenAPI/AsyncAPI behavior also shows healthy restraint around unspecified concepts.

The score is held below a clean A mainly by the Wave 3 meta-schema acceptance mismatch, not by a foundational generator design problem.

### Important Wave 4 rule

Do not extend `ess-gen` until it becomes:

```text
docs + schemas + OpenAPI + AsyncAPI + executable semantic oracle + test runner + target framework
```

That would blur two different contracts.

Keep projection generation and conformance synthesis adjacent but distinct.

---

# 11. AEP / ADP bridge review

## Assessment: A-

This side of the system is farther along than Wave 4 needs.

`EssConformanceResult` already carries the essentials needed to record an ESS conformance result: specification, implementation, pass/fail status, scenario counts, suite/compiler/generator versions and failed scenario identifiers.

AEP also constrains who may establish ESS conformance evidence: the conformance runner is the verifier rather than the coding agent.

This directly supports the desired trust boundary:

```text
agent writes candidate
        ↓
independent runner tests candidate
        ↓
runner creates evidence
        ↓
ADP consumes evidence
```

That is one of the strongest parts of the overall architecture.

### Small reconciliation item

Wave 4 should settle what `generator_version` means once the scenario synthesizer is intentionally separated from `ess-gen`.

Do not let this turn into a large evidence-model redesign, but make the provenance unambiguous.

Possible meanings include:

- suite format version;
- scenario synthesizer version;
- ESS compiler version;
- projection generator version where relevant.

The goal is reproducibility: given ESS revision + compiler/synthesizer versions + implementation identity, we should know which oracle produced the result.

---

# 12. CI, build, and repository engineering review

## Assessment: A-

The overall discipline is good.

Local checks and CI are deliberately close, with formatting, warning-denied Clippy, workspace tests and generated schema/projection drift checks.

There are several inexpensive improvements I would make around Wave 4.

### Test the declared minimum Rust version

The workspace declares Rust `1.85`, while CI currently uses the stable Rust toolchain rather than explicitly checking the declared version.

If `rust-version = 1.85` is a contract, test it.

Add a separate MSRV job or make one required build/test path use 1.85.

### Make rustdoc warnings a gate

For a repository whose documentation is part of the architecture, add something equivalent to:

```text
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

`task doc` exists, but documentation compilation is not part of the principal `task check`/CI path today.

This is not a Wave 4 blocker, but it is cheap and aligned with the project's quality bar.

### Fix repository package metadata

The root package/workspace metadata still points at an older/different GitHub owner path rather than the authoritative `codewandler/engineering-protocols` remote.

This is trivial to correct and belongs in the reconciliation change.

### Dependency/security auditing

I did not see dependency/security auditing elevated to the same level as schema/projection drift.

I would **not block Wave 4** on this, but before public release or external consumption, consider adding a pinned policy via `cargo-deny` and/or an advisory audit workflow.

---

# 13. Testing and falsifiability review

## Assessment: A-

Testing philosophy is one of the repository's strengths.

The AEP backend conformance fault matrix is especially valuable because it moves beyond “lots of tests” to “prove the tests reject specific wrong implementations.”

Wave 4 should adopt that standard immediately.

The acceptance rule should not be:

> generated billing tests pass.

It should be:

> the correct billing target passes, and each intentionally faulty semantic behavior fails the exact scenario family expected to detect it.

Examples from the proposed Wave 4 scope should include faults such as:

- accepted invalid amount;
- wrong declared error;
- wrong/missing event;
- stale read-your-writes view;
- eventual view never converges;
- illegal lifecycle transition;
- dropped binding;
- wrong field mapping;
- external failure mapped to the wrong semantic outcome;
- binding failure policy ignored.

The report should retain semantic provenance back to ESS so failures can become precise agent repair context.

That is where the project begins to distinguish itself from ordinary code generation.

---

# 14. What should explicitly **not** happen in Wave 4

I would record these as architectural guardrails before implementation.

### Do not generate Rust tests as the canonical representation

The proposed portable `ConformanceSuite` / scenario IR direction is correct. Rust should be one runner/adapter environment, not the semantic test definition.

### Do not hide missing ESS semantics inside `ConformanceTarget`

`ConformanceTarget` should expose semantic operations required to interact with an implementation.

It should not become a collection of test-only escape hatches such as:

```text
assert_the_binding_worked()
tell_me_whether_escalation_happened()
force_view_to_be_correct()
```

If such an operation cannot be related to a declared ESS semantic concept, it is suspicious.

### Do not make generated OpenAPI the oracle

HTTP method/path/status conventions belong to projection policy unless ESS explicitly models transport exposure.

Wave 4 proves semantic system conformance first.

### Do not add arbitrary sleeps

The model already distinguishes immediate/read-your-writes and eventual assertions. Use bounded polling/eventual semantics with explicit deadlines or consistency mechanisms; do not make timing nondeterminism part of the oracle.

### Do not silently skip unsupported scenarios

A valid ESS requirement must end in one of two places:

```text
tested
```

or

```text
explicitly refused with a semantic diagnostic
```

Never:

```text
not generated, nobody notices
```

This principle foreshadows Wave 5's later `Generated | Obligation | Refused` algebra and is worth adopting now.

### Do not begin structural synthesis yet

Wave 5 should remain gated by the successful closed loop.

The new Wave 5 design was deliberately filed after—and dependent upon—Wave 4, not as a parallel implementation path.

---

# 15. Recommended pre-Wave-4 reconciliation milestone

I recommend introducing a very small milestone before W4.1.

Call it something like:

**W3.5 — Wave 4 Reconciliation / Semantic Closure**

Its acceptance criteria should be:

### Repository truth restored

- `README.md` accurately reflects current IR and Wave 3.
- `AGENTS.md` accurately reflects current repository state and full gate.
- ESS roadmap records Waves 1–3 as actually delivered.
- Wave 4/5 sequencing is consistent.
- repository URL/package metadata is corrected.

### Wave 3 closure made honest

Either:

- OpenAPI/AsyncAPI documents pass pinned full meta-schema validation;

or:

- the Wave 3 acceptance language is explicitly amended and deferred work recorded.

No silent mismatch remains.

### Binding failure semantics made testable

For the normative billing `on_failure: escalate` behavior:

- ESS defines the semantic fact/action that constitutes escalation;
- compiler/IR resolves it;
- the normative billing fixture uses it;
- no test-runner-only hidden interpretation is required.

### Wave 4 architecture frozen against the real APIs

- design baseline includes current `ess-gen`;
- existing `ResolvedOutcome` test strategy is reused;
- existing view assertion style is reused;
- scenario synthesis is explicitly fallible;
- scenario synthesis is not conflated with `ess_gen::Generator`;
- `ConformanceSuite` uses stable semantic identity rather than runtime-only handles;
- semantic conformance is explicitly separated from OpenAPI/AsyncAPI projection conformance.

### Engineering gate complete

- HEAD CI is green;
- ideally add MSRV coverage;
- ideally add rustdoc warning enforcement.

Once those are satisfied, I would consider Wave 4 implementation **GO**.

---

# 16. Proposed Wave 4 implementation sequence after reconciliation

I would slightly tighten the proposed design into this execution order.

## W4.1 — Canonical Scenario IR

Define stable, serializable:

```text
ConformanceSuite
ConformanceScenario
ScenarioStep
ScenarioExpectation
ScenarioId
SemanticRef
```

Keep it technology-independent.

## W4.2 — Deterministic scenario synthesis for command outcomes

Start with the strongest existing semantics:

- `when`;
- `otherwise`;
- `external`;
- declared errors;
- emitted events/no-event expectations.

Use explicit refusal diagnostics where safe witnesses cannot be constructed.

## W4.3 — View semantics

Implement:

- read-your-writes/immediate expectations;
- bounded eventual expectations;
- deterministic fixture/reset/isolation behavior.

No sleeps.

## W4.4 — Semantic `ConformanceTarget`

Keep the interface small:

```text
execute command
query view
observe events
configure external outcome
isolate/reset scenario
```

Only expose concepts justified by ESS conformance.

## W4.5 — Correct billing target

Hand-write one reference target.

Do not optimize it for production architecture. Optimize it for semantic clarity and trustworthy oracle development.

## W4.6 — Fault matrix

Introduce faults one at a time and establish:

```text
fault
  → expected scenario/property
  → actual failure
```

The test suite itself is not accepted until the fault matrix demonstrates discriminating power.

## W4.7 — Bindings and failure behavior

Add:

- event→command mapping;
- field transformation;
- failure semantics;
- new explicit escalation observation.

## W4.8 — Lifecycle semantics

Prove legal transitions and rejection of illegal ones using the same scenario representation.

## W4.9 — Conformance report and provenance

Produce a report tied to:

- ESS identity/digest;
- suite version;
- compiler version;
- scenario synthesizer version;
- target/implementation identity;
- scenario results and counterexamples.

## W4.10 — Independent AEP evidence

Translate the authoritative runner result into the existing `EssConformance` evidence model.

## W4.11 — Actual ADP closure

Demonstrate both halves:

```text
correct target
→ valid independent evidence
→ ADP completion allowed
```

and:

```text
faulty target
→ conformance failure
→ no valid evidence
→ ADP completion refused
```

That last step is the real Wave 4 milestone.

---

# 17. Deferred items that should **not** hold up Wave 4

These are worth tracking but are not prerequisites for beginning once the reconciliation work is complete:

- general-purpose deep predicate solving;
- formal model checking;
- production persistence;
- transport-level OpenAPI/Kafka conformance targets;
- structural Rust synthesis;
- implementation obligations;
- realization/linker model;
- Kubernetes/cloud topology generation;
- broad security/release automation;
- generalized agent repair loops beyond one reference obligation.

The handoff correctly places most of those after closed-loop conformance.

Do not pull future-wave complexity forward.

---

# 18. Final architecture assessment

The repository is not suffering from a fundamental design problem.

That is the most important conclusion of this review.

There is no reason to rewrite the AEP contract, merge ESS into AEP, replace the IR, collapse commands into HTTP operations, turn `ess-gen` into a framework, or jump ahead to Rust/Kubernetes synthesis.

The existing architecture is converging on a coherent system:

```text
                   ESS
                    │
              validated model
                    │
                 EssIr
          ┌─────────┴─────────┐
          │                   │
    deterministic         executable
     projections            oracle
          │                   │
 docs/schema/API      ConformanceSuite
                              │
                      ConformanceTarget
                              │
                         implementation
                              │
                      independent report
                              │
                    EssConformance evidence
                              │
                             ADP
```

The remaining pre-Wave-4 problems are primarily **semantic closure and repository truthfulness**, not architecture.

The normative `escalate` gap is particularly valuable because it demonstrates why the closed-loop milestone matters: a specification can be syntactically valid, compile cleanly and produce attractive projections while still containing a requirement that is not falsifiable.

Wave 4 should turn those cases into first-class design pressure.

---

# 19. Go / no-go decision

## Current state: **NO-GO for writing Wave 4 implementation code today**

Not because the system is unstable, but because starting implementation immediately would force the scenario synthesizer to resolve questions that belong in ESS/design.

## After the reconciliation milestone: **GO**

I would authorize Wave 4 once these four gates are closed:

1. **Observable escalation semantics exist.**
2. **Wave 4 design has been reviewed and reconciled with the actual current IR/`ess-gen`.**
3. **Wave 3's meta-schema acceptance mismatch has been explicitly resolved.**
4. **Control documents accurately describe the repository and HEAD has a completed green gate.**

At that point, the next implementation target should be **canonical Scenario IR and fallible scenario synthesis**, not structural synthesis.

---

# 20. Bottom line

The repository is in **better shape than the handoff implies** because Wave 3 has landed and the compiler→projection boundary is now concrete. The handoff explicitly said to trust the repository where it had advanced, which is the correct approach here.

The architecture is sufficiently mature to attempt the most important proof yet:

> **Can one ESS become an independent, falsifiable oracle that determines whether an implementation satisfies the specification and whether ADP may declare the work complete?**

The answer is not proven yet.

But after a short pre-Wave-4 reconciliation, the repository is well positioned to prove it without redesigning what already works.

**Recommended next action: make one focused W3.5 reconciliation change, review it as a milestone boundary, then begin W4.1 `ConformanceSuite` / Scenario IR.**