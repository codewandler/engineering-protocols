# ESS Closed-Loop Execution & Conformance — Design v0.1

> **Repository:** `codewandler/engineering-protocols`  
> **Status:** **Reviewed and reconciled 2026-08-20 — frozen for implementation**, except the open decisions listed in §2 under *Open decisions this freeze does not close*.  
> **Milestone:** ESS wave 4 in [`docs/plan/ess-roadmap.md`](../plan/ess-roadmap.md).  
> **Audience:** Implementor continuing the existing ESS work after `ess-domain`, `ess-compiler`, and `ess-gen`  
> **Relationship to existing design:** Additive. This document does not replace `docs/design/ess-implementor-design-v0.1.md` or `docs/plan/ess-roadmap.md`. It narrows the next milestone into an executable closed loop.
>
> **What the reconciliation changed, 2026-08-20.** The v0.1 draft was written against a repository
> state that no longer exists — it assumed `ess-gen` was local and unpushed. Two independent reviews
> were folded into this document **in place**, not appended: the repository baseline (§2), the refusal
> model that makes synthesis fallible and keeps it out of `ess_gen::Generator` (§11, §36), the crate
> boundary for the runner (§35), four things to reuse rather than rebuild (§14, §23, §25, §30), the
> compiler decisions synthesis must consume by name rather than re-derive (§11, §14), suite identity
> being semantic rather than handle-based (§21), execution determinism (§37), and the §15/§40
> contradiction over sleeps. The header's original instruction — *reconcile names with the actual local
> API rather than creating duplicate abstractions* — is what this pass carried out.
>
> **Frozen means:** implement what is written and do not re-litigate it. The four open decisions in §2
> are the exceptions; each must be settled before the code it governs is written.
>
> **Before any of it:** the blocking gates in
> [`docs/plan/ess-wave-3.5-reconciliation.md`](../plan/ess-wave-3.5-reconciliation.md) come first.
> Several are model changes — a command has no declared link to the lifecycle transition it causes or
> the entity it acts on (G14), and `on_failure: escalate` has no observable consequence (G2) — and
> both are far cheaper to fix before a synthesizer is built around their absence than after.
>
> **After it:** wave 5, structural synthesis, stays gated on wave 4 closing its loop — **both** halves:
> a correct target producing valid evidence that lets an ADP task complete, *and* a faulty target
> producing a failure that refuses completion. An oracle nobody has watched fail is not an oracle.

---

## 1. Purpose

The ESS model and compiler answer:

> Can we describe a software system in a typed semantic form, resolve it, validate it, and compile it into a normalized representation?

`ess-gen` begins answering:

> Can we deterministically project that semantic representation into useful artifacts such as documentation, JSON Schema, OpenAPI, and AsyncAPI?

The next milestone must answer the stronger question:

> Can the same ESS specification act as an executable oracle against a real implementation, produce independent conformance evidence, and close an actual ADP task?

This document specifies that milestone.

The target loop is:

```text
                        ESS SOURCE
                            │
                            ▼
                     ess-compiler
                            │
                            ▼
                          EssIr
                            │
              ┌─────────────┴─────────────┐
              │                           │
              ▼                           ▼
           ess-gen                 scenario synthesis
      docs/contracts/etc.                  │
                                          ▼
                                 canonical scenarios
                                          │
                                          ▼
                                conformance runner
                                          │
                    ┌─────────────────────┴─────────────────────┐
                    │                                           │
                    ▼                                           ▼
          correct billing target                      faulty billing target
                    │                                           │
                    └─────────────────────┬─────────────────────┘
                                          ▼
                                 conformance report
                                          │
                                          ▼
                              EssConformance evidence
                                          │
                                          ▼
                                     AEP / ADP
                                          │
                                          ▼
                                 completion decision
```

The milestone is complete only when the loop works end-to-end.

---

## 2. Repository Baseline

The design assumes the current `engineering-protocols` architecture.

Already present in the repository:

```text
AEP
├── aep-domain
├── aep-contract
├── aep-engine
├── aep-schema
├── aep-backend-memory
├── aep-conformance
├── adp-domain
├── aop-domain
└── protocol-cli

ESS
├── ess-domain
├── ess-compiler
└── ess-gen
```

The existing AEP side already provides:

- typed engineering artifacts;
- ESS as an AEP artifact kind;
- `EssConformance` as an evidence kind;
- an `ess-conformance` principle;
- deterministic protocol evaluation;
- independent-evidence requirements;
- command/query semantics;
- identity and revisions;
- audit;
- black-box backend conformance.

The existing ESS side already provides:

- the typed ESS domain model;
- opaque identity, logical location, and wire naming;
- domains;
- actors and roles;
- entities and value objects;
- commands with outcomes;
- declared errors;
- views with consistency semantics;
- state machines;
- typed predicates/invariants;
- components;
- bindings;
- binding delivery/failure semantics;
- topology;
- validation;
- normalized IR;
- deterministic compilation;
- source-aware diagnostics;
- the normative billing fixture.

`ess-gen` provides the projection boundary from `EssIr`. It is **on `main`**, tagged
`0.3.2-ess-wave-3`, and provides:

- four projections — Markdown + Mermaid, JSON Schema per message and named type, OpenAPI 3.1 and
  AsyncAPI 3.0 per component;
- one `Generator` trait all four implement;
- `Provenance` on every artifact — system, specification version, a digest of the resolved IR,
  compiler version, generator version;
- one shared type mapping across the projections, with agreement asserted keyword by keyword;
- 27 committed artifacts plus a generated index under `generated/`, drift-checked in CI by
  `cargo xtask generate --check`;
- 123 tests.

Two properties of that crate constrain this design rather than merely informing it. §35 and §36 turn
on them:

- **`Generator::generate` is infallible by contract.** "A generator reaching a construct it cannot
  project is a gap in this crate, not a fault in the specification — and the specification has already
  been refused if it was wrong […] So there is nothing left for a `Result` to report."
  (`crates/ess-gen/src/artifact.rs:45`.)
- **The crate holds no clock and no randomness.** "Same IR in, byte-identical bytes out. No clock, no
  RNG, `BTreeMap`/`BTreeSet` only." (`crates/ess-gen/src/lib.rs:24`.)

This document starts at that boundary.

### Open decisions this freeze does not close

Four, each named again at the section that raises it. Each has a default, so silence does not block the
work — but each must be settled before the code it governs is written, because all four decide type
signatures rather than internals.

| # | decision | section | default if nobody decides |
|---|---|---|---|
| D1 | which crate owns the scenario IR, the synthesizer and the runner | §35, §36 | a new `ess-conformance` crate — `ess-gen` can hold neither the synthesizer's fallibility nor the runner's clock |
| D2 | what drives `async fn run_suite`; this workspace has no async runtime | §27 | make the runner synchronous, and keep `async` out until a transport-level target needs it |
| D3 | how a bounded eventual assertion waits, given that §40 forbids sleeps | §15, §40 | the target waits; the runner passes a deadline and never sleeps |
| D4 | what `generator_version` means once the synthesizer is not `ess-gen` | §23 | two fields — `generator_version` for the projector, `synthesizer_version` for the suite producer |

Everything else here is settled. A reader who disagrees with a settled part records the disagreement in
[`docs/design/reconciliation-v0.2.md`](reconciliation-v0.2.md) §5 rather than diverging silently, which
is this repository's standing rule.

---

## 3. Why This Is the Next Milestone

The project thesis is not proven by parsing an ESS.

It is not proven by generating attractive OpenAPI.

It is not even proven by generating a test suite.

The thesis is proven when:

```text
the specification
    generates the oracle

an implementation
    is checked by that oracle

the check
    produces independent evidence

the protocol
    consumes the evidence

the protocol
    permits or refuses completion
```

Until then, ESS remains a strongly typed design language with promising projections.

After this milestone, ESS becomes part of an executable engineering control loop.

---

## 4. Scope

This milestone includes:

1. a canonical conformance scenario model derived from `EssIr`;
2. a technology-independent test-side target contract;
3. one correct hand-written billing implementation;
4. deliberately faulty billing implementations or fault modes;
5. a conformance runner;
6. generated positive and negative scenarios;
7. deterministic handling of eventual consistency;
8. binding-flow verification;
9. precise fault-to-test assertions;
10. a structured conformance report;
11. conversion of that report into AEP `EssConformance` evidence;
12. one real ADP task whose completion depends on that evidence.

This milestone does **not** require:

- generated Rust application code;
- Kubernetes generation;
- multiple production transports;
- behavioral synthesis;
- formal verification;
- distributed deployment;
- a durable AEP backend;
- a generic service mesh or runtime framework.

Those remain later work.

---

## 5. Architectural Rule

The conformance system is a **consumer of ESS semantics**.

It must not invent new semantics.

```text
ess-domain
    owns semantic vocabulary

ess-compiler
    owns resolution + validated IR

ess-gen
    owns deterministic projections

ESS conformance
    consumes the same IR
    and turns semantics into executable observations
```

If the conformance runner needs information that does not exist in `EssIr`, the default response is:

> Improve the ESS model or IR.

Do not hide missing semantics inside the runner.

---

## 6. A Test-Side Contract, Not an Application Framework

A critical distinction:

The conformance interface is a **test adapter contract**.

It is not a runtime architecture that every ESS-conformant application must adopt internally.

A Rust service, Java service, serverless implementation, modular monolith, or distributed system may all be conformant.

Each implementation supplies an adapter capable of translating the canonical ESS test operations into that implementation.

Conceptually:

```text
                    canonical ESS scenarios
                              │
                              ▼
                      ConformanceTarget
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   Rust in-process        HTTP/Kafka             Java
      adapter               adapter              adapter
        │                     │                     │
        ▼                     ▼                     ▼
 implementation A       implementation B       implementation C
```

The adapter is not the system.

It is the test-side bridge into the system.

---

## 7. The Canonical `ConformanceTarget`

Use a small semantic interface.

Illustrative Rust shape:

```rust
#[async_trait]
pub trait ConformanceTarget {
    async fn identity(
        &self,
    ) -> Result<ImplementationIdentity, TargetError>;

    async fn begin_scenario(
        &self,
        scenario: &ScenarioContext,
    ) -> Result<(), TargetError>;

    async fn execute_command(
        &self,
        request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError>;

    async fn query_view(
        &self,
        request: SemanticViewRequest,
    ) -> Result<SemanticViewResult, TargetError>;

    async fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError>;

    async fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError>;

    async fn end_scenario(
        &self,
        scenario: &ScenarioContext,
    ) -> Result<(), TargetError>;
}
```

The exact trait may change.

The semantic responsibilities should not.

Note what is deliberately **absent** from those seven methods: a clock, a seed and an id source. That
is not an oversight to be corrected by adding them here — it is the boundary §37 draws. Everything that
varies is minted by the runner and handed to the target; the target mints nothing the report will
compare.

---

## 8. Scenario Isolation

Every generated scenario needs an isolated logical execution context.

```rust
pub struct ScenarioContext {
    pub scenario_id: ScenarioId,
    pub correlation_id: CorrelationId,
}
```

A target may implement isolation through:

- a fresh in-memory runtime;
- database transaction/reset;
- tenant namespace;
- generated IDs;
- temporary schema;
- container reset;
- another mechanism.

The conformance API cares only that observations from one scenario cannot accidentally satisfy another.

No scenario should depend on global execution order unless the ESS explicitly specifies shared state.

---

## 9. Command Invocation

A semantic command request should identify the ESS command, actor when relevant, input, and scenario context.

```rust
pub struct SemanticCommandRequest {
    pub command: CommandRef,
    pub actor: Option<ActorRef>,
    pub input: Value,
    pub correlation_id: CorrelationId,
}
```

A successful invocation returns an observation, not implementation internals.

```rust
pub struct SemanticCommandResult {
    pub outcome: OutcomeRef,
    pub error: Option<DeclaredErrorValue>,
    pub consistency: Option<ConsistencyToken>,
    pub direct_events: Vec<ObservedEvent>,
}
```

The result must distinguish declared domain rejection from adapter/runtime failure.

For example:

```text
CreateInvoice(amount = -1)

declared result:
    outcome = rejected
    error = InvalidAmount

NOT:
    TargetError::Http500
```

Conformance tests must fail if declared domain behavior is surfaced merely as an untyped infrastructure error.

---

## 10. Outcomes Are the Primary Test Unit

ESS commands already model outcomes.

Test synthesis should generate at least one scenario per reachable declared outcome.

Example ESS shape:

```yaml
CreateInvoice:

  outcomes:

    accepted:
      when: amount.amount > 0
      emits:
        - InvoiceCreated

    rejected:
      when: amount.amount <= 0
      error: InvalidAmount
```

Generated suite:

```text
CreateInvoice / accepted
    valid positive amount
    → accepted
    → InvoiceCreated

CreateInvoice / rejected
    zero or negative amount
    → rejected
    → InvalidAmount
    → InvoiceCreated must not occur
```

Negative assertions are first-class.

A happy-path-only suite is non-conformant with the design.

---

## 11. Input Witness Generation

A scenario needs concrete command inputs.

Do not immediately build an arbitrary property-based solver.

For v0.1 use a constrained strategy:

1. prefer explicit examples/fixtures if the ESS provides them;
2. derive trivial witnesses for primitive predicates where deterministic;
3. allow the normative example to provide scenario fixture values;
4. refuse generation when no safe witness can be produced.

The important invariant is:

> Never generate an arbitrary value and claim it satisfies an outcome predicate unless the generator can prove or evaluate that it does.

A refusal is better than a false test.

### Consume `ResolvedOutcome::test_strategy`; do not re-derive it

The compiler has **already decided**, per outcome, whether that branch is reachable by constructing an
input. The decision is stored: `ResolvedOutcome::test_strategy`
(`crates/ess-compiler/src/ir.rs:488`), computed once during resolution, for the reason its own doc
comment gives — a decision made per projection is a decision made wrong eventually.

Synthesis therefore **reads `test_strategy` and branches on it**. It does not re-inspect the predicate
and form an independent opinion about reachability. Asking that question a second time is a
**regression**, not an optimisation: it reintroduces exactly the divergence the field was added to
prevent, and the divergence will not be visible until two projections disagree about one outcome.

### `Unknown` refuses — it is not "try another candidate"

Predicate evaluation is Kleene three-valued and the workspace invariant is blunt: *`Unknown` is not
`False`*. A witness search that treats `Unknown` as a near miss spends its whole budget on a
specification defect and then reports the result as a flaky test.

So: keep `True`, discard `False`, and **refuse on `Unknown`**, with a diagnostic naming the predicate
and the fact path that could not be resolved. `Unknown` says the specification does not yet say enough
— which is information for the person who wrote it, not noise for the generator to retry through.

### Refusal is an outcome, not a defect

These are legitimate, expected results of synthesis, not bugs in it:

- no safe witness can be produced for the outcome's predicate;
- the failure policy a scenario would have to assert has no observable in the model (see
  [G2](../plan/ess-wave-3.5-reconciliation.md));
- the predicate is valid but not constructively satisfiable;
- evaluation returns `Unknown`.

Each is returned as a **typed diagnostic** carrying a stable code, a structured body and the ESS element
that caused it — the same shape `ess-compiler` already uses for a bad document, because a coding agent
consumes both as repair instructions. Silently omitting the scenario is the one unacceptable option.

This is why scenario synthesis cannot be an `ess_gen::Generator`. See §36.

Possible later extension:

```text
typed predicate
    ↓
constraint solver
    ↓
automatically generated witnesses
```

but this is not required for the first closed loop.

---

## 12. External Outcomes

Some commands have outcomes that cannot be predicted from input alone.

The billing example intentionally exercises this.

For such commands, the conformance target needs a test-control capability.

Example:

```rust
pub struct ExternalOutcomeControl {
    pub command: CommandRef,
    pub force_outcome: OutcomeRef,
}
```

This is a **test adapter control**, not an ESS runtime capability.

It allows the suite to test:

```text
SendEmail
    external success → EmailSent

SendEmail
    external failure → EmailFailed / declared error
```

without making the system specification lie about determinism it does not possess.

A production adapter may implement this by:

- swapping a test double;
- controlling an external fake;
- setting a fixture;
- injecting a port implementation.

---

## 13. Event Observation

Events are semantic facts.

The conformance runner needs to observe them independent of transport.

```rust
pub struct ObservedEvent {
    pub event: EventRef,
    pub payload: Value,
    pub correlation_id: Option<CorrelationId>,
    pub sequence: Option<u64>,
}
```

The adapter may obtain these from:

- an in-memory event sink;
- Kafka test consumer;
- event table;
- HTTP callback sink;
- application instrumentation;
- another implementation-specific source.

The runner must not care.

Transport headers are adapter concerns unless ESS explicitly models them.

---

## 14. View Queries and Consistency

ESS already distinguishes:

```text
read_your_writes
eventual
```

The runner must preserve that distinction.

### Read-your-writes

**Both types already ship in `aep-contract`. Use them; do not declare a second pair.**

| this section originally proposed | what exists |
|---|---|
| `pub struct ConsistencyToken(String)` | `aep_contract::consistency::ConsistencyToken` — opaque, backend-issued, validated (`crates/aep-contract/src/consistency.rs:30`) |
| `pub enum ViewConsistency { Current, AtLeast(_) }` | `aep_contract::consistency::QueryConsistency` — `Current` as the default, and a token-bearing variant beside it (`crates/aep-contract/src/consistency.rs:76`) |

They are the same idea, arrived at twice. A parallel pair would buy nothing but a translation table
between two spellings of one concept, and a translation table is a place for the two to drift apart.

Generated flow:

```text
command
    ↓
token T
    ↓
query view AtLeast(T)
    ↓
assert immediately once target returns
```

The test does not sleep.

The target adapter is responsible for waiting until it can satisfy `AtLeast(T)`.

### Eventual

For an eventual view, the runner may poll through the semantic query API until:

- the predicate is satisfied; or
- a scenario deadline expires.

Again:

> No arbitrary sleep in generated tests.

Use a bounded eventual assertion abstraction. What "bounded" means concretely — a deadline the runner
owns, and waiting that happens inside the target — is settled in §15.

### Consume `ResolvedView::assertion_style`; do not re-derive it

Whether a view assertion is immediate or must be retried is not a question for the synthesizer either.
`ResolvedView::assertion_style` (`crates/ess-compiler/src/ir.rs:609`) holds the answer, computed once
from the view's declared consistency, stored "for the reason `ResolvedOutcome::test_strategy` is: it is
a decision, and a decision made per projection is a decision made wrong eventually".

Synthesis reads it. Deciding `expect` versus `eventually` again, by reading the consistency word a
second time, is a regression for the same reason `test_strategy` is.

---

## 15. Deadlines Are Runner Configuration

The ESS says whether something is eventual.

It should not initially say:

```text
wait exactly 750ms
```

Time budgets belong to the conformance execution environment unless the domain itself has a time-bound semantic guarantee.

Example runner config:

```yaml
conformance:

  eventual:
    timeout: 5s
```

These values affect test execution, not the meaning of the ESS.

If the ESS later models a true semantic SLA such as "event must occur within 2 seconds", that is a different specification concept.

### `poll_interval` is gone — §40 wins

An earlier draft of this section configured `poll_interval: 25ms` beside the timeout. §40 forbids
sleeps as synchronisation. **A poll interval is a sleep by another name** — the same fixed delay,
spelled as configuration and repeated in a loop — so the two sections contradicted each other. Resolved
here, in favour of §40: **the runner configures a deadline and nothing else about waiting.**

`ResolvedView::assertion_style`'s own doc comment names this failure mode from the other side:
asserting an eventual view with `expect` "races the projection, and the repair everyone reaches for is
a sleep — which makes the suite a test of the machine it runs on"
(`crates/ess-compiler/src/ir.rs:606`). Configuring the sleep centrally does not stop it being that
test.

**What waits instead is the target.** A view query or an event observation carries the scenario's
deadline; the target returns when it can answer or when the deadline expires. *How* it waits — a
channel, a notification, a condition variable, the backend's own consistency mechanism — is an adapter
concern the runner must not know about, exactly as §14 already makes the target responsible for
satisfying `AtLeast(token)`.

**Open decision D3.** If a target genuinely cannot block — a stateless HTTP adapter with no
subscription — someone has to wait somewhere. That case is out of wave 4's scope, whose reference
target is in-process, and it is deferred rather than pre-solved with an interval. If it is ever
answered with a poll loop, the interval belongs to the **target**, is recorded in the report as part of
that target's identity, and §40's invariant is amended in writing rather than quietly broken.

---

## 16. Binding Scenarios

A binding is executable cross-component semantics.

Example:

```text
InvoiceCreated
    ↓
notify-on-invoice-created
    ↓
SendEmail
    ↓
EmailSent
```

The generated scenario should verify the observable flow.

At minimum:

```text
1. execute upstream command that emits InvoiceCreated
2. observe InvoiceCreated
3. observe downstream declared behavior
4. verify the binding's success/failure semantics
```

Do not require the runner to observe the internal `SendEmail` command if that command is not externally observable.

It may instead prove the flow through the resulting `EmailSent`.

Where an adapter can expose semantic command tracing, that may be additional evidence, but it should not become a requirement for every implementation.

---

## 17. Binding Delivery Semantics

The existing model requires binding delivery and failure semantics.

The generated suite must test what is actually specified.

For an `at_least_once` binding, a conformant implementation may legitimately produce duplicates unless the downstream contract or domain requires idempotency.

Therefore the test must not accidentally assert exactly-once behavior.

Example:

```text
BAD TEST:
    exactly one EmailSent exists

if ESS only says:
    delivery: at_least_once
```

Instead assert the minimum guaranteed semantic condition.

If duplicates would violate the business domain, that must be expressed separately.

---

## 18. Binding Failure Semantics

For:

```yaml
delivery: at_least_once
on_failure:
  escalate:
    emits: billing.email.DeliveryEscalated
```

Since gate G2 landed, `escalate` names the event it emits, so the scenario below asserts that event
rather than needing an observation the model could not make.


the suite must have a scenario that forces the downstream failure.

The expected observation must come from the ESS semantics.

If the ESS does not yet define an observable representation of `escalate`, scenario synthesis must refuse that check rather than invent one.

This is an important feedback loop:

```text
test cannot express expected failure behavior
    ↓
model is semantically incomplete
    ↓
improve ESS vocabulary
```

Do not hide this with implementation-specific assertions.

---

## 19. Lifecycle Scenarios

Entity lifecycle/state machines generate two classes of tests.

### Legal transitions

For every declared transition:

```text
state A
    command C
        ↓
state B
```

generate a scenario that proves it can occur under a valid witness.

### Illegal transitions

The absence of a transition is itself semantics.

For relevant state/command combinations where no transition exists:

```text
state Paid
    CancelInvoice
        ↓
must not reach Cancelled
```

The exact rejection mechanism must come from the declared command/error semantics.

Do not generate vague "operation fails" tests if the domain declares a specific error.

### This section is blocked on a model gap

Both classes above are written as sequences of `ExecuteCommand` steps, so both need to know **which
command drives a transition** — and the model does not say. `ResolvedCommand` names its domain, its
input and its outcomes, but no subject (`crates/ess-compiler/src/ir.rs:501`); an entity's `Transition`
names a `from`, a `to` and its own label, but no cause (`crates/ess-domain/src/entity.rs:157`). In the normative billing example that is three declared
transitions and zero commands that drive any of them.

That is gate G14 in [`ess-wave-3.5-reconciliation.md`](../plan/ess-wave-3.5-reconciliation.md), and it
is a model change — a construct in `ess-domain`, resolution in the compiler, a rendering in the
projections, an example edit. **Do not work around it in the synthesizer.** Inferring the driving
command from a name, an event or an ordering is exactly the kind of invention §11 refuses, and it would
bake the omission in behind a heuristic nobody could later find.

---

## 20. Invariant Scenarios

Typed invariants can produce runtime assertions where witnesses are available.

For v0.1:

- evaluate invariants after successful state-changing commands;
- evaluate invariant predicates against observable entity/view state where possible;
- do not require full formal proof.

Later:

```text
typed invariant
    ↓
property-based generator
    ↓
many generated executions
```

and later still:

```text
typed invariant
    ↓
formal model
    ↓
proof / counterexample
```

The current milestone only needs deterministic conformance cases.

Same dependency as §19: "evaluate invariants after a state-changing command" needs a command→entity
link to answer *whose* invariants to evaluate after `CreateInvoice`. Gate G14 again. And evaluating a
typed invariant against a candidate value needs a projection from an input to a `FactSource`, which
does not exist yet either — gate G16. Both are prerequisites of this section, not work inside it.

---

## 21. Canonical Scenario IR

Do not generate Rust tests directly from `EssIr`.

Introduce a small technology-neutral scenario representation.

```rust
pub struct ConformanceSuite {
    pub suite_version: SuiteVersion,
    pub spec: SpecificationIdentity,
    pub scenarios: Vec<ConformanceScenario>,
}
```

Example:

```rust
pub struct ConformanceScenario {
    pub id: ScenarioId,
    pub purpose: ScenarioPurpose,
    pub steps: Vec<ScenarioStep>,
    pub source: Vec<EssSemanticRef>,
}
```

Possible steps:

```rust
pub enum ScenarioStep {
    ConfigureExternalOutcome(...),
    ExecuteCommand(...),
    ExpectOutcome(...),
    ExpectError(...),
    ExpectEvent(...),
    ExpectNoEvent(...),
    QueryView(...),
    ExpectView(...),
    EventuallyEvent(...),
    EventuallyView(...),
}
```

This scenario IR is the stable bridge between:

```text
ESS semantics
    and
execution technology
```

It should be serializable for inspection and future cross-language runners.

### A serialized suite carries semantic identity, not handles

`EssIr` handles are valid only inside the IR that minted them. Using one against a different `EssIr`
**panics by design** — "a handle belongs to the IR that minted it"
(`crates/ess-compiler/src/ir.rs:141`) — because inside one process that is a programming mistake, not a
specification's problem.

A committed `ConformanceSuite` is not inside one process. It is written to `generated/`, drift-checked
in CI, read back by a runner in a later process on a later checkout, and referred to by scenario id
from the fault matrix. **So every reference it holds is a stable ESS semantic name** — `CommandRef`,
`OutcomeRef`, `EventRef`, `ViewRef`, `EssSemanticRef` — resolvable against any compilation of the same
specification. No handle, no index into a `Vec`, no slot number.

A suite whose references only mean something inside the process that produced it is not an artifact; it
is a cache. It also cannot be what §22 promises — a portable bridge to a future cross-language runner —
because a handle is not portable across a process, let alone a language.

---

## 22. Why Scenario IR Matters

Without a scenario IR:

```text
EssIr
  ↓
Rust test generator
```

the first runner becomes the semantic definition by accident.

With:

```text
EssIr
  ↓
ConformanceSuite
  ↓
Rust runner
  ↓
future HTTP runner
  ↓
future external certification runner
```

the semantics remain portable.

This mirrors the broader project rule:

> Strong typed semantic model first; technology-specific projections second.

---

## 23. Scenario Provenance

Every generated suite must carry provenance:

```rust
pub struct SuiteProvenance {
    pub spec_version: String,
    pub spec_digest: Digest,
    pub compiler_version: String,
    pub generator_version: String,
    pub suite_version: String,
}
```

**This type largely exists.** `ess_gen::Provenance` (`crates/ess-gen/src/provenance.rs:13`) already
carries the system, the specification version, a digest of the resolved model, the compiler version and
the generator version, and every projected artifact emits it. Extend or reuse it rather than declaring
a parallel shape: two provenance types in one repository are two answers to "which specification
produced this", and the point of provenance is that there is one.

**Open decision D4.** Once the synthesizer is a separate thing from `ess-gen` (§36), `generator_version`
has two possible referents, and a report that cannot say which oracle produced a verdict is not
reproducible — which is the whole purpose of the field. Default: two fields, `generator_version` for
the projector and `synthesizer_version` for the suite producer.

Every scenario should also identify which ESS semantic elements caused it to exist.

Example:

```text
scenario:
    billing.invoice.CreateInvoice/outcome/rejected

derived_from:
    command billing.invoice.CreateInvoice
    outcome rejected
    error billing.invoice.InvalidAmount
```

This makes failures explainable and auditable.

---

## 24. Correct Reference Implementation

Before trusting generated tests, implement the billing system by hand.

The implementation should be intentionally boring.

Its purpose is not to showcase architecture.

Its purpose is to provide a known target for the specification.

Recommended shape:

```text
examples/billing/reference/
├── Cargo.toml
├── src/
│   ├── invoice.rs
│   ├── email.rs
│   ├── runtime.rs
│   └── lib.rs
└── tests/
```

Prefer an in-process implementation first.

Reasons:

- no broker setup;
- no container orchestration;
- no network flakiness;
- faster scenario iteration;
- semantic errors remain visible;
- the same ESS can later be tested against distributed adapters.

The reference implementation must still respect ESS semantics.

It is not privileged to cheat the suite.

---

## 25. Deliberately Faulty Implementations

A generated test suite is not trustworthy until its failures are demonstrated.

Do not ship only:

```text
correct implementation → green
```

Ship a fault matrix.

| Fault ID | Deliberate defect | Expected check |
|---|---|---|
| `F-WRONG-EVENT` | `CreateInvoice` emits wrong event | accepted outcome event assertion |
| `F-REJECTION` | invalid amount is accepted | rejected outcome scenario |
| `F-ILLEGAL-TRANSITION` | paid invoice can be cancelled | lifecycle negative test |
| `F-DROPPED-BINDING` | `InvoiceCreated` never invokes email flow | binding scenario |
| `F-WRONG-MAPPING` | wrong email recipient mapped | downstream event payload assertion |
| `F-VIEW-RACE` | adapter returns stale view despite `AtLeast(token)` | read-your-writes consistency |
| `F-EXTERNAL-OUTCOME` | forced email failure still reports success | external outcome scenario |

Implementation options:

### Option A — Separate faulty targets

```text
reference-correct/
reference-wrong-event/
reference-dropped-binding/
...
```

Simple but repetitive.

### Option B — One target with fault injection

```rust
enum Fault {
    WrongEvent,
    AcceptInvalidAmount,
    AllowIllegalTransition,
    DropBinding,
    WrongMapping,
    IgnoreConsistencyToken,
}
```

Prefer this if the fault controls remain obvious and isolated.

**Option B already ships, one crate over.** `aep-conformance` does exactly this for the AEP backend
suites, and it is the precedent to copy rather than reinvent:

| the design proposes | what exists |
|---|---|
| `enum Fault { … }` | `Fault`, a small `#[non_exhaustive]` enum with a `Fault::ALL` constant (`crates/aep-conformance/src/faulty.rs:34`) |
| "each fault fails its intended check" | `Fault::caught_by()`, naming the one suite that exists to catch each variant (`crates/aep-conformance/src/faulty.rs:88`) |
| §26's meta-test | `each_fault_is_caught_by_the_suite_that_exists_to_catch_it`, which iterates `ALL` and fails on any fault no designated suite catches (`crates/aep-conformance/tests/faults.rs:55`) |
| a deliberately broken implementation | `FaultyBackend`, the target those faults are injected into |

The ESS fault matrix is that pattern with ESS scenario ids where AEP suite names are. What is worth
carrying over unchanged is the `ALL` constant: it is what makes the meta-test a matrix rather than a
list someone has to remember to extend.

The important invariant:

> Each deliberate fault must fail the specific scenario intended to detect it.

A generic panic that causes the entire suite to fail proves nothing.

---

## 26. Fault Matrix Test

CI should contain a meta-test roughly equivalent to:

```text
correct implementation
    → all scenarios pass

fault WRONG_EVENT
    → scenario S_EVENT fails
    → unrelated core scenarios still pass

fault DROPPED_BINDING
    → scenario S_BINDING fails

fault ILLEGAL_TRANSITION
    → lifecycle negative scenario fails
```

This is the same falsifiability principle already used by AEP backend conformance.

---

## 27. Conformance Runner

The runner consumes:

```text
ConformanceSuite
+
ConformanceTarget
+
RunnerConfig
```

and emits:

```text
ConformanceReport
```

Illustrative API:

```rust
pub async fn run_suite<T: ConformanceTarget>(
    suite: &ConformanceSuite,
    target: &T,
    config: &RunnerConfig,
) -> ConformanceReport;
```

The runner owns:

- scenario sequencing;
- isolation;
- eventual polling;
- comparison;
- diagnostics;
- report assembly.

The target owns:

- invoking the implementation;
- observing semantic outputs;
- mapping implementation-specific failures into target errors;
- consistency waiting where required.

The split above is about responsibility. §37 makes the same split about *variation* — the runner owns
every source of it, and the target owns none — and that is the stricter of the two.

### There is no async runtime in this workspace — open decision D2

The signature above says `async fn`, and nothing in this repository can drive it. The only executor
present is `aep_contract::testing::block_on` (`crates/aep-contract/src/testing.rs:22`), which polls a
future up to a million times with a no-op waker and then panics. That is right for what it was written
for — a synchronous backend whose futures are ready on the first poll, where still-pending genuinely
means deadlock — and wrong for a runner awaiting a target that really yields. Handing it a suite
against an HTTP target would either burn a million polls or panic on a future that was merely slow.

So the runner's execution model is an open decision, not an implementation detail:

| option | cost |
|---|---|
| **synchronous runner** (default) | no new dependency; the in-process reference target needs nothing more; a future transport target must block internally, which §15 already asks of it |
| add a real executor (`tokio`, `smol`) as a workspace dependency | the first async runtime in a workspace that has deliberately had none, and a `[lints]`/MSRV surface to keep green |
| keep `async fn` and require the caller to bring an executor | pushes the choice onto adopters, and makes the runner untestable in this workspace without one |

Settle it before step 1 of §49: it decides whether every signature in the scenario IR and the target
trait is `async`, and changing that afterwards is a rewrite rather than a refactor.

---

## 28. Result Semantics

Each scenario result should be structured.

```rust
pub struct ScenarioResult {
    pub scenario: ScenarioId,
    pub status: ScenarioStatus,
    pub checks: Vec<CheckResult>,
    pub observations: Vec<Observation>,
    pub duration: Duration,
}
```

Statuses:

```text
passed
failed
error
unsupported
```

Distinguish:

```text
failed
    implementation contradicted ESS

error
    runner/adapter could not execute the check

unsupported
    target cannot currently expose required semantic observation
```

An `unsupported` required scenario makes overall conformance fail.

Do not silently skip required semantics.

---

## 29. Failure Diagnostics

A failure should answer:

```text
what semantic rule was checked?
which ESS element defined it?
what command/input was executed?
what was expected?
what was observed?
```

Example:

```text
ESS-CF-OUTCOME-003

scenario:
  billing.invoice.CreateInvoice/outcome/rejected

source:
  billing.invoice.CreateInvoice.outcomes.rejected

input:
  amount.amount = 0

expected:
  outcome = rejected
  error = InvalidAmount
  event InvoiceCreated absent

observed:
  outcome = accepted
  event InvoiceCreated emitted
```

This output should be usable directly as agent repair feedback.

---

## 30. Conformance Report

The report should contain enough identity to become evidence.

```rust
pub struct EssConformanceReport {
    pub spec: SpecificationIdentity,
    pub implementation: ImplementationIdentity,
    pub suite_version: SuiteVersion,

    pub compiler_version: String,
    pub generator_version: String,

    pub started_at: Timestamp,
    pub completed_at: Timestamp,

    pub status: ConformanceStatus,
    pub scenarios: Vec<ScenarioResult>,
}
```

Possible aggregate statuses:

```text
passed
failed
error
```

If any required scenario is unsupported, aggregate status is not `passed`.

**Read the existing report type before designing this one.** `aep_conformance::report` already models a
conformance run with per-check results: `ConformanceReport`, carrying the level that was claimed and a
`SuiteReport` of what each suite found (`crates/aep-conformance/src/report.rs:204`). The ESS report is
genuinely a different shape — scenarios rather than suites, and it must carry specification identity,
which the AEP one has no need of — but the per-check result structure, the status vocabulary and the
aggregation rule are all solved there, tested, and shipped.

---

## 31. AEP Evidence Handoff

This is where ESS meets the existing protocol system.

The conformance report must be convertible into the existing:

```text
EvidenceKind::EssConformance
```

The AEP evidence should contain at least the fields already defined by the repository:

```text
spec version
implementation identity
suite version
compiler version
generator version
status
```

Prefer also attaching or referencing:

```text
spec digest
report digest
conformance report artifact
```

if the existing evidence model permits this without expanding scope unnecessarily.

The conformance runner is an independent verifier.

Therefore the resulting evidence should satisfy an AEP requirement marked:

```yaml
independent: true
```

assuming the actor/producer metadata meets AEP's independence rules.

---

## 32. Do Not Let the Agent Manufacture Conformance Evidence

The agent may trigger the runner.

The agent may read the report.

The agent may repair failures.

The agent should not construct the authoritative `EssConformance` evidence payload by assertion.

Correct:

```text
agent
  → requests conformance execution

runner
  → executes generated suite

runner
  → produces identified report

integration
  → converts report to evidence
```

Incorrect:

```text
agent:
  "all ESS tests passed"
      ↓
EssConformance(passed=true)
```

That would defeat the AEP/ESS join.

---

## 33. The First Real ADP Task

Create one repository task fixture that requires ESS conformance.

Example intent:

```text
Implement the normative billing system.
```

Its profile should include the existing `ess-conformance` principle.

Completion must require:

```text
ess_conformance.status == passed
```

Run the full sequence:

```text
task resolves
    ↓
implementation exists
    ↓
conformance suite runs
    ↓
report says passed
    ↓
EssConformance evidence submitted
    ↓
protocol engine reevaluates
    ↓
task completes
```

Then run the same task against a faulty implementation:

```text
suite fails
    ↓
evidence status != passed
    ↓
ADP refuses completion
```

That second half is part of acceptance.

---

## 34. CLI Integration

Preserve the current CLI organization rather than introducing a second executable.

The current CLI uses:

```text
protocol ess validate
protocol ess compile
protocol ess inspect
protocol ess graph
```

`ess-gen` will presumably extend this locally.

Suggested eventual commands:

```text
protocol ess generate <spec> --kind docs
protocol ess generate <spec> --kind schema
protocol ess generate <spec> --kind openapi
protocol ess generate <spec> --kind asyncapi

protocol ess test generate <spec>
protocol ess test run <spec> --target <adapter>
protocol ess test inspect <spec>

protocol ess conformance run <spec> --target <adapter>
protocol ess conformance report <report>
```

The shipped verbs are `protocol ess validate | compile | inspect | graph | generate`, and
`generate --kind docs|schema|openapi|asyncapi` is already the established shape — so the conformance
verbs extend that surface rather than proposing a parallel one. `protocol conformance` is separately
taken by the AEP backend suites; whatever the ESS spelling ends up being, it must not read as the same
command.

The required semantics are more important than CLI spelling.

---

## 35. Where the Code Should Live

Respect the existing review decision to avoid premature crate proliferation. But the shape this section
originally proposed — a `conformance/` module tree inside `ess-gen`, holding `scenario.rs`,
`synthesize.rs`, `target.rs`, `runner.rs` and `report.rs` — is **not available**, and the reason is in
`ess-gen`'s own documentation rather than in anyone's taste.

### `ess-gen` cannot hold the runner

| what the runner is | what `ess-gen` says about itself |
|---|---|
| **fallible** — a target can be unreachable, a check can `error`, an observation can be `unsupported` | `Generator::generate` is infallible by contract: a construct it cannot project is a defect in the crate, and "there is nothing left for a `Result` to report" (`crates/ess-gen/src/artifact.rs:45`) |
| **takes a clock** — deadlines, `ScenarioResult::duration`, the report's `started_at`/`completed_at` | "No clock, no RNG, `BTreeMap`/`BTreeSet` only" (`crates/ess-gen/src/lib.rs:24`) |
| **talks to an external implementation** | pure `EssIr` → bytes, with a determinism test per generator that generates twice and compares |

Putting a fallible, clock-taking, implementation-touching runner inside that crate does not stretch
those two claims — it **falsifies** them. And they are load-bearing: the byte-identical determinism
tests rest on the second, and the "a gap is a crate defect, not a specification fault" reasoning that
lets every projection be infallible rests on the first.

The same argument applies, one degree weaker, to the synthesizer: it is fallible (§11, §36) even though
it holds no clock.

### Open decision D1 — the crate boundary

Settle before step 1 of §49: it decides where the scenario IR type lives, and every later module
imports it.

| option | scenario IR | synthesizer | runner + target | note |
|---|---|---|---|---|
| **A — one `ess-conformance` crate (default)** | `ess-conformance` | `ess-conformance` | `ess-conformance` | one boundary, one crate; `ess-gen` untouched and both of its claims intact |
| B — synthesis in `ess-gen`, execution in `ess-conformance` | `ess-conformance` | `ess-gen` | `ess-conformance` | keeps §36's "all derivation from `EssIr` is `ess-gen`" — but only by adding a second, fallible trait beside `Generator` in a crate whose doc says a failure there is a defect |
| C — everything in `ess-gen` | `ess-gen` | `ess-gen` | `ess-gen` | **rejected**: falsifies the table above |

The repository's rule is:

> Split when the boundary has been argued about twice.

It has now been argued about twice — once in this document's first draft, which put it inside `ess-gen`,
and once in the review that reconciled the draft against the code. The argument settled that the
boundary is real: conformance consumes scenario IR, interacts with external implementations, is used
without generating docs or contracts, and will gain target adapters. That is the condition the rule
asks for, which is why option A is the default here rather than a premature split.

---

## 36. Relationship to `ess-gen`

`ess-gen` remains the owner of **infallible** deterministic derivation from `EssIr`.

Scenario synthesis is derivation too — but it is not the same contract, and the original diagram here,
which hung `ConformanceSuite` off `ess-gen` as a fifth branch beside docs, JSON Schema, OpenAPI and
AsyncAPI, hid the difference.

### Two contracts, not one trait

| | `ess_gen::Generator` | scenario synthesis |
|---|---|---|
| result | `Vec<Artifact>` — always | a `ConformanceSuite` **or** typed refusal diagnostics |
| a construct it cannot handle | a gap in the crate; a defect to fix (`crates/ess-gen/src/artifact.rs:45`) | a legitimate outcome, reported to whoever wrote the specification |
| what a refusal means | nothing — there is no refusal to express | "this specification does not yet say enough for an oracle to check it" |
| who acts on a failure | the maintainer of `ess-gen` | the author of the ESS |

The refusals are the whole reason. Synthesis legitimately declines when no safe witness can be produced
for an outcome's predicate, when a binding's failure policy has no observable to assert against, when a
valid predicate is not constructively satisfiable, and when predicate evaluation returns `Unknown`
(§11). None of those is a bug in the synthesizer, and none of them can be expressed by returning
`Vec<Artifact>` — which is why forcing synthesis behind `Generator` would mean either lying (emit a
scenario that asserts nothing) or panicking (turn a thin specification into a broken tool). Both are
failures this repository exists to prevent.

```text
EssIr
  ├── ess-gen — infallible
  │     ├── docs
  │     ├── JSON Schema
  │     ├── OpenAPI
  │     └── AsyncAPI
  │
  └── scenario synthesis — fallible
        └── ConformanceSuite | Vec<SynthesisRefusal>
```

A `SynthesisRefusal` is **typed and diagnostic**, in the shape `ess-compiler` already uses for a bad
document: a stable code, a structured body, and the ESS element that caused it. A coding agent consumes
both as repair instructions, so they should read the same way.

Silently omitting a scenario is the one unacceptable option. A suite that quietly contains fewer checks
than the specification requires is precisely the "generated tests are green" failure §51 rules out —
and unlike a refusal, nothing about it is visible in a passing run.

### The runner is a third thing

It executes a `ConformanceSuite` against a target, it is fallible, and it takes a clock. It does not
belong in `ess-gen` under any option; see §35 and open decision D1.

---

## 37. Determinism

Two claims are needed, not one: a suite must be **generated** the same way twice, and it must be **run**
the same way twice. The original section made only the first. The second is added below, because wave
4's product is evidence that decides whether work may be declared complete, and evidence from a run
nobody can repeat is a claim rather than evidence.

### Generation determinism

Generated scenario suites must follow the same deterministic rules as the compiler and other projections.

Given identical:

```text
EssIr
compiler version
generator version
generation config
```

the serialized `ConformanceSuite` must be byte-identical.

Rules:

- deterministic maps/sets;
- deterministic scenario ordering;
- no timestamps in generated suite content;
- no RNG;
- canonical serialization;
- trailing newline;
- committed fixture regeneration checked in CI.

Execution reports may contain timestamps.

Generated definitions may not.

### Execution determinism

Everything above is about *generating* a suite. It says nothing about *running* one, and that closing
pair — reports may carry timestamps, definitions may not — is a formatting rule, not a reproducibility
claim.

The repository already answered this one layer down. **Invariant 8:** the domain crate is clock-free and
randomness-free, and the engine takes a `Clock` "so an execution is replayable". `aep-conformance`'s
`Harness` is the worked example — a monotonic sequence counter and a clock starting at a fixed instant,
both held by the harness rather than read from the environment
(`crates/aep-conformance/src/harness.rs:77`).

The `ConformanceTarget` in §7 has seven methods and not one of them is a clock, a seed or an id source.
So a correlation id, a timestamp or an idempotency key minted during a run currently comes **from
nowhere in particular**. Fix that here, before the runner exists.

**A `ScenarioId` is a semantic name, never a counter.** This document says both in two places, and only
one of them survives contact with a specification that changes. A counter is stable "across unchanged
input", which is what §50 asks for — but insert one outcome and every scenario after it is renumbered,
so the committed suite re-keys wholesale, the fault matrix's references rot, and every stored report
names scenarios that no longer exist. Worse, a semantic diff of two specifications cannot line up
yesterday's result with today's scenario, which is the whole basis of deciding whether prior evidence
still holds.

So a scenario id is derived from what the scenario is *about* — the qualified name of the command, the
outcome, the transition or the binding it exercises — and two scenarios about the same thing in the same
way are the same id. It is a name, so it is diffable, greppable, and stable under every change that does
not touch the construct it names. A counter is none of those.

**A scenario's `source` is its dependency set, not the construct that spawned it.** `derived_from` as
sketched lists what *caused* the scenario to exist — a command, an outcome, an error. What a later
consumer needs is every construct the scenario *depends on*: the types its input mentions, the entity it
moves, the view it asserts, the event it expects. The two differ exactly where it matters. A scenario
generated from `CreateInvoice.rejected` and asserting an `InvalidAmount` payload depends on `Money`; if
`Money` gains a field, that scenario's result is stale and a `derived_from` naming only the outcome will
not say so. Collecting the dependency set costs nothing at generation time, because the generator has
just walked every one of those constructs to build the scenario. Reconstructing it afterwards means
regenerating the suite.

**The runner owns every source of variation.** It is constructed with them, and nothing below it reaches
for an ambient one:

| the runner is given | so that |
|---|---|
| a `Clock` | deadlines, `ScenarioResult::duration`, and the report's `started_at`/`completed_at` are reproducible under a fixed clock |
| an id source — a monotonic counter, seeded from the suite | correlation ids and idempotency keys are derived from the suite, not from a wall clock or a random device. **Not scenario ids** — see below |
| the suite, the target and `RunnerConfig` | those are the only inputs; the runner reads nothing else |

**The target owns nothing that varies.** It may not mint a correlation id, read a clock for anything the
report will contain, or invent an identifier the runner will compare. What it owns is what §27 already
says: invoking the implementation, observing semantic outputs, mapping implementation failures into
`TargetError`, and waiting until it can satisfy a consistency requirement (§15). A target that needs an
id or a timestamp is **given** one in the request.

**The claim this buys.** With the same injected clock and id source, against a deterministic target, two
runs of one suite produce **byte-identical reports** — which is what makes a committed report reviewable
by diff. Against a live target under a real clock, two runs differ *only* in the fields below.

| may legitimately differ between two runs | may not |
|---|---|
| `started_at`, `completed_at`, `ScenarioResult::duration` | any `status` — on any scenario, on any check |
| how many times a bounded eventual assertion had to ask before it was satisfied | which checks ran, and in what order |
| implementation-internal detail quoted inside a diagnostic — a hostname, an address | scenario ids, correlation ids, expected values, or the set of observations compared |

**A `status` that differs between two runs is a defect, never noise.** It is a bug in the runner, in the
target, or in the specification, and the answer is to find it rather than to retry. §26's fault matrix
is the test that catches it: if a scenario's verdict is not a function of its inputs, the matrix has
stopped being a matrix and become a sample.

`error` (§28) exists for the case where the runner or adapter could not execute a check. It is not a
place to hide non-determinism: an `error` that appears in one run and not the next is itself the finding,
and is reported as one.

---

## 38. Generated Suite as an Artifact

Commit the generated billing suite or an equivalent canonical fixture.

Example:

```text
generated/
├── docs/
├── schema/
├── openapi/
├── asyncapi/
└── conformance/
    └── suite.json
```

Wave 3 already committed the first four directories under `generated/` at the repository root, with an
index beside them, drift-checked by `cargo xtask generate --check`; the suite joins them there. (The
first draft of this section wrote `examples/billing/generated/`, which is not where anything landed —
`examples/billing/` holds the specification, `generated/` holds what the specification produces.)

CI should regenerate and diff.

This gives reviewers a stable artifact and lets the wrong-implementation matrix refer to scenario IDs that do not change accidentally.

---

## 39. Testing Layers

This milestone itself needs several test levels.

### Unit tests

Test:

- scenario derivation;
- command outcomes;
- event expectations;
- negative expectations;
- lifecycle cases;
- mapping of view consistency;
- external outcome controls;
- report aggregation.

### Golden tests

Given normative billing `EssIr`, assert canonical generated suite.

### Runner tests

Use fake `ConformanceTarget`s to test:

- immediate pass;
- declared domain rejection;
- target error;
- eventual polling;
- timeout;
- unsupported observation;
- correlation isolation.

### Reference target tests

The correct hand-written implementation passes.

### Fault matrix tests

Each injected defect fails its designated semantic check.

### AEP integration test

Conformance evidence changes ADP completion from false to true only when status is passed.

---

## 40. No Sleeps

Make this an explicit invariant.

Generated scenarios and runner tests must not contain:

```rust
sleep(Duration::from_millis(...))
```

as synchronization logic.

Use:

- consistency tokens for read-your-writes;
- bounded polling for declared eventual semantics;
- event observation with deadlines.

A fixed delay tests machine timing, not system semantics.

### `poll_interval` versus this invariant

§15's earlier `poll_interval: 25ms` was a fixed delay under a configuration name, which contradicted
this section outright. Resolved in §15 **in favour of this one**: the runner configures a *deadline* and
never sleeps. "Bounded polling" above therefore means a bounded number of semantic queries whose
*waiting* happens inside the target — the only layer that knows what it is waiting for. See §15 for the
resolution and open decision D3.

---

## 41. No Hidden Transport Assumptions

The reference implementation may be in-process.

A later real implementation may use HTTP + Kafka.

The canonical suite must remain unchanged.

Only the target adapter changes.

Therefore the scenario IR may refer to:

```text
semantic command
semantic outcome
semantic event
semantic view
semantic error
```

but not:

```text
HTTP 201
Kafka partition
Serde struct name
SQL table
Rust enum variant
```

unless the ESS outer-surface specification explicitly makes that item part of the contract.

Transport contract testing belongs to generated OpenAPI/AsyncAPI projections.

Semantic conformance remains above transport.

---

## 42. Contract Conformance vs Semantic Conformance

Keep these concepts separate.

### Contract conformance

Examples:

```text
HTTP request matches generated OpenAPI
event payload matches generated JSON Schema
Kafka surface matches generated AsyncAPI
```

### Semantic conformance

Examples:

```text
CreateInvoice with invalid amount is rejected
accepted creation emits InvoiceCreated
paid invoice cannot be cancelled
InvoiceCreated causes the specified email flow
InvoiceById eventually reflects the new invoice
```

The closed-loop suite focuses on semantic conformance.

Transport contract tests may be included as additional checks when a target exposes those surfaces.

---

## 43. Runtime Adapter vs Generated Runtime

Do not confuse the test-side `ConformanceTarget` with the later generated runtime.

Current milestone:

```text
hand-written implementation
    +
hand-written ConformanceTarget adapter
```

Later structural synthesis:

```text
generated implementation skeleton
    +
generated/adapted ConformanceTarget
```

The same canonical suite runs against both.

That reuse is a critical acceptance criterion for the later synthesis wave.

---

## 44. The Handoff to Structural Synthesis

Once the suite is trusted, the next milestone becomes straightforward:

```text
EssIr
    ↓
rust structural synthesis
    ↓
generated billing workspace
    ↓
same ConformanceSuite
    ↓
same runner
    ↓
passed
```

**"Once the suite is trusted" is a gate, not a transition.** Wave 5 starts only after wave 4 has closed
its loop in both directions: a correct target producing valid evidence that lets an ADP task complete,
*and* a faulty target producing a failure that refuses completion. A suite that has only ever been
green has not been trusted — it has been unexamined, and generating code against it produces confident
nonsense that nothing can contradict. The wave 5 design
([`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](ess-structural-synthesis-obligations-realizations-design-v0.1.md))
opens by assuming this milestone "already exists and is trusted"; that assumption is this gate.

No new behavioral oracle should be written for generated Rust.

If Rust synthesis needs bespoke tests not derivable from the ESS suite, determine whether they are:

- generator implementation tests; or
- evidence that the ESS lacks required semantics.

Do not silently create a second source of behavioral truth.

---

## 45. Controlled Divergence

Generated code is disposable.

The conformance contract is durable.

Future workflow:

```text
                       ESS
                        │
              ┌─────────┴─────────┐
              ▼                   ▼
      generated default      custom implementation
              │                   │
              └─────────┬─────────┘
                        ▼
                 same conformance
                      suite
                        │
                ┌───────┴───────┐
                ▼               ▼
             conforms       does not conform
```

An engineer should be able to replace a generated implementation with a hand-written optimized one without changing the ESS.

Passing the same conformance suite proves semantic compatibility.

---

## 46. Explicit Implementation Obligations

Structural synthesis will eventually hit behavior the ESS cannot deterministically generate.

Do not fill such gaps with arbitrary generated code.

Represent them explicitly.

Example:

```rust
pub trait PricingPolicy {
    fn calculate(
        &self,
        input: PricingInput,
    ) -> Result<Money, PricingError>;
}
```

The synthesis output should also emit a machine-readable obligation:

```yaml
obligation:
  id: billing.invoice.PricingPolicy
  kind: implementation
  contract:
    input: PricingInput
    output: Money
  required_by:
    - billing.invoice.CreateInvoice
```

This creates a clean future agent loop.

---

## 47. Future Agent Synthesis Loop

After structural synthesis works:

```text
ESS
 ↓
compiler
 ↓
generated skeleton
 ↓
unfulfilled implementation obligation
 ↓
ADP task
 ↓
coding agent
 ↓
candidate implementation
 ↓
ESS conformance suite
 ↓
counterexample / pass
 ↓
EssConformance evidence
```

The agent fills only the semantically underdetermined gap.

Deterministic machinery owns everything that can be derived deterministically.

This is preferable to asking an agent to synthesize the whole repository from prose.

---

## 48. Counterexamples as Agent Feedback

The conformance runner's failure output should already be suitable for future CEGIS-like repair.

Example:

```text
obligation:
  billing.invoice.CreateInvoice

counterexample:
  input:
    amount:
      amount: 0
      currency: EUR

expected:
  outcome: rejected
  error: InvalidAmount

observed:
  outcome: accepted
  event: InvoiceCreated
```

The agent receives the counterexample, repairs the candidate, and reruns the exact same independent verifier.

No new special agent protocol is needed.

AEP already governs the retry process.

---

## 49. Implementation Order

`ess-gen`'s projection work is delivered and stable (§2), so this order starts now — after the blocking
gates in [`ess-wave-3.5-reconciliation.md`](../plan/ess-wave-3.5-reconciliation.md).

**Two open decisions come before step 1, not during it.** D1 (§35) decides which crate the step 1 types
live in; D2 (§27) decides whether their signatures are `async`. Both are changes to every signature in
the scenario IR and the target trait, so deciding either after step 1 means rewriting step 1. D3 (§15)
is needed by step 3 and D4 (§23) by step 8.

### Step 1 — Scenario IR

Implement:

```text
ConformanceSuite
ConformanceScenario
ScenarioStep
SuiteProvenance
```

Every reference inside them is a stable ESS semantic name, not an `EssIr` handle (§21).

Acceptance:

```text
billing EssIr → deterministic inspectable scenario suite
a suite serialized in one process resolves in another
```

### Step 2 — Outcome scenarios

Generate:

- accepted outcomes;
- rejected outcomes;
- declared errors;
- expected emitted events;
- negative event assertions.

Branch on `ResolvedOutcome::test_strategy` rather than re-deriving reachability (§11), and return typed
refusals rather than omitting a scenario.

Acceptance:

```text
CreateInvoice has both happy and rejection cases
a construct synthesis declines to cover appears as a typed refusal, not as an absence
```

### Step 3 — View semantics

Implement:

- read-your-writes consistency token flow, on `aep_contract::consistency` rather than a new pair (§14);
- eventual view assertions, driven by `ResolvedView::assertion_style` rather than by re-reading the
  consistency word (§14);
- runner deadline config — a deadline, and no poll interval (§15).

Acceptance:

```text
no sleeps, and no poll interval either
```

### Step 4 — Correct hand-written billing target

Implement in-process target and adapter.

Acceptance:

```text
outcome + event + view scenarios pass
```

### Step 5 — Fault matrix

Add deliberate defects.

Acceptance:

```text
each semantic fault fails its intended scenario
```

### Step 6 — Binding scenarios

Generate and execute:

```text
InvoiceCreated → SendEmail → EmailSent
```

including externally controlled downstream outcomes where required.

### Step 7 — Lifecycle suite

Generate legal and illegal transition checks.

### Step 8 — Full report

Produce `EssConformanceReport`, reusing `ess_gen::Provenance` and the per-check result structure of
`aep_conformance::report` (§23, §30). Settle D4 here.

Acceptance:

```text
two runs under one injected clock produce byte-identical reports
```

### Step 9 — AEP join

Convert passed report to independent `EssConformance` evidence.

### Step 10 — Real protocol completion test

A real task completes only after the evidence exists.

A faulty target cannot complete.

---

## 50. Acceptance Criteria

This milestone is complete when all of the following are true.

### Specification → suite

- Billing ESS compiles to one deterministic canonical conformance suite.
- Every command outcome is represented.
- Every declared error used by a command outcome is tested.
- Required emitted events are asserted.
- Relevant non-emission is asserted for rejection paths.
- Both view consistency modes produce correct runner semantics.
- Lifecycle transitions produce positive and negative checks.
- The billing binding produces an executable cross-domain scenario.
- External outcomes can be controlled without changing the ESS.

### Correct implementation

- A small hand-written billing implementation passes the complete suite.
- The suite uses semantic adapters, not implementation internals.
- No scenario requires arbitrary sleep.

### Falsifiability

- A wrong-event fault fails the event check.
- An invalid-amount acceptance fault fails the rejected outcome check.
- An illegal lifecycle transition fault fails the lifecycle check.
- A dropped binding fails the binding check.
- A wrong mapping fails the downstream payload check.
- A stale-view/ignored-token fault fails the consistency check.
- Each fault fails for the expected reason.

### Evidence

- The runner produces a structured `EssConformanceReport`.
- The report identifies spec, implementation, suite, compiler, and generator.
- A passed report can be converted to existing AEP `EssConformance` evidence.
- The evidence producer is independent from the coding agent.
- A real ADP task requiring ESS conformance completes when the report passes.
- The same task refuses completion when the target is faulty.

### Determinism

- Scenario generation is byte-identical.
- Generated suite is drift-checked in CI.
- Scenario IDs are stable across unchanged input.
- The committed suite resolves in a process that did not produce it — semantic names, no handles (§21).

### Execution determinism

- Two runs of one suite against one implementation agree on **every** status, on every scenario and
  every check.
- Under the same injected clock and id source, two runs produce byte-identical reports; under a real
  clock they differ only in the fields §37 permits.
- The runner takes its clock and its id source by injection. Neither the runner nor any target reads an
  ambient clock, seed or random device.
- No generated scenario and no runner test sleeps, and no configuration names a poll interval (§15, §40).

### Refusal

- Synthesis returns typed refusal diagnostics rather than silently omitting a scenario, and each names
  the ESS element that caused it.
- Predicate evaluation returning `Unknown` refuses, with the predicate and the missing fact path in the
  diagnostic. It is never treated as "try another candidate" (§11).

---

## 51. Explicit Non-Acceptance Criteria

The milestone is **not** complete if:

```text
OpenAPI validates
```

but no implementation has been checked.

It is not complete if:

```text
generated tests are green
```

but they have never been proven to fail against deliberate defects.

It is not complete if:

```text
the agent reports conformance
```

without an independent runner result.

It is not complete if:

```text
billing works
```

but the protocol does not consume the conformance evidence.

The closed loop is the deliverable.

---

## 52. Suggested Repository Additions

Illustrative only; adapt to the tree as it stands on `main`.

```text
engineering-protocols/
├── crates/
│   ├── ess-domain/
│   ├── ess-compiler/
│   ├── ess-gen/                     unchanged — infallible projections only (§35)
│   └── ess-conformance/             open decision D1, §35 option A
│       └── src/
│           ├── lib.rs
│           ├── scenario.rs          scenario IR — stable ESS names, no handles (§21)
│           ├── synthesize.rs        fallible: a suite, or typed refusals (§11, §36)
│           ├── target.rs            ConformanceTarget (§7)
│           ├── runner.rs            owns the clock and the id source (§37)
│           └── report.rs
│
├── examples/
│   └── billing/                     the specification
│       ├── domains/
│       ├── components.yaml
│       ├── topology.yaml
│       └── reference/
│           ├── Cargo.toml
│           └── src/
│
├── generated/                       what the specification produces — already drift-checked in CI
│   └── conformance/
│       └── suite.json
│
└── conformance/
    └── ess/
        └── fault-matrix/
```

Two corrections to the first draft of this tree, both matching what the repository actually does:
committed artifacts live under `generated/` at the root, not under `examples/billing/generated/`
(§38), and the conformance code is its own crate rather than a module inside `ess-gen` (§35). The
second is open decision D1; if it is settled the other way, only the crate path changes — the file
list and the responsibilities above do not.

---

## 53. Design Relationship Map

This document fits into the repository as follows:

```text
docs/VISION.md
    WHY AEP + ESS exist together
        │
        ▼
docs/design/consolidated-design-v0.2.md
    AEP semantics
        │
        ▼
docs/design/ess-implementor-design-v0.1.md
    ESS semantic model + compiler + synthesis direction
        │
        ▼
docs/design/ess-review-v0.1.md
    refinements:
      outcomes
      view consistency
      binding failure
      typed invariants
      identity
      reference + faulty implementation
        │
        ▼
docs/plan/ess-roadmap.md
    waves 1–5; waves 1–3 delivered
        │
        ▼
docs/plan/ess-wave-3.5-reconciliation.md
    the gates wave 4 waits behind
        │
        ▼
THIS DOCUMENT
    executable design for the bridge:
      ess-gen / scenario synthesis
      → conformance target
      → correct + faulty implementation
      → conformance report
      → AEP evidence
      → ADP completion
```

It is intentionally narrower than the ESS implementor design.

Downstream of it, and gated on it closing its loop:

```text
THIS DOCUMENT (ESS wave 4)
        │
        ▼
docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md (ESS wave 5)
```

---

## 54. Final System Shape

After this milestone:

```text
                         PRODUCT INTENT
                               │
                               ▼
                              ESS
                               │
                   ┌───────────┼───────────┐
                   │           │           │
                   ▼           ▼           ▼
                docs       contracts   conformance
                                           suite
                                             │
                                             ▼
                                      implementation
                                             │
                                             ▼
                                      conformance run
                                             │
                                   ┌─────────┴─────────┐
                                   │                   │
                                   ▼                   ▼
                                passed              failed
                                   │                   │
                                   ▼                   ▼
                            ESS evidence        counterexample
                                   │                   │
                                   ▼                   └──→ agent repair
                                  ADP
                                   │
                                   ▼
                             completion gate
```

After the later structural-synthesis milestone:

```text
ESS
 │
 ├──→ contracts
 ├──→ conformance suite
 └──→ generated implementation
               │
               ▼
        same conformance suite
               │
               ▼
            evidence
```

That is the point at which the project can credibly claim:

> The specification is not merely documentation and not merely code generation input. It is the executable oracle against which both generated and hand-written implementations are judged.

---

## 55. Core Thesis of This Milestone

> **Do not synthesize more software until the specification has proven that it can independently judge software.**

`ess-gen` proves that one semantic model can produce multiple deterministic projections.

This milestone proves that the model can also produce a verifier that bites.

Only after that should structural synthesis generate executable services.

The resulting order is:

```text
model
  ↓
compiler
  ↓
projections
  ↓
oracle
  ↓
trusted conformance loop
  ↓
structural synthesis
  ↓
agent-filled gaps
  ↓
same oracle
```

That order keeps the source of truth singular and every later claim falsifiable.
