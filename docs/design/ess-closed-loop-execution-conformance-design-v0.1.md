# ESS Closed-Loop Execution & Conformance — Design v0.1

> **Repository:** `codewandler/engineering-protocols`  
> **Status:** Proposed next implementation milestone  
> **Audience:** Implementor continuing the existing ESS work after `ess-domain`, `ess-compiler`, and `ess-gen`  
> **Relationship to existing design:** Additive. This document does not replace `docs/design/ess-implementor-design-v0.1.md` or `docs/plan/ess-roadmap.md`. It narrows the next milestone into an executable closed loop.
>
> **Local-state assumption:** `ess-gen` exists locally but is not yet pushed. This document assumes it implements, or is implementing, the projection boundary described by ESS wave 3. Reconcile names with the actual local API rather than creating duplicate abstractions.

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
└── ess-compiler
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

The local, not-yet-pushed `ess-gen` is assumed to provide the projection boundary from `EssIr`.

This document starts at that boundary.

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

A command result may return an opaque consistency token:

```rust
pub struct ConsistencyToken(String);
```

A view request can require:

```rust
pub enum ViewConsistency {
    Current,
    AtLeast(ConsistencyToken),
}
```

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

Use a bounded eventual assertion abstraction.

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
    poll_interval: 25ms
```

These values affect test execution, not the meaning of the ESS.

If the ESS later models a true semantic SLA such as "event must occur within 2 seconds", that is a different specification concept.

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
on_failure: escalate
```

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

Do not treat these names as normative if the unpushed `ess-gen` already established a coherent command shape.

The required semantics are more important than CLI spelling.

---

## 35. Where the Code Should Live

Respect the existing review decision to avoid premature crate proliferation.

At the start of this milestone, prefer modules inside the existing ESS implementation boundary.

Possible first shape:

```text
crates/
├── ess-domain/
├── ess-compiler/
└── ess-gen/
    └── src/
        ├── docs/
        ├── schema/
        ├── openapi/
        ├── asyncapi/
        └── conformance/
            ├── scenario.rs
            ├── synthesize.rs
            ├── target.rs
            ├── runner.rs
            └── report.rs
```

However, conformance has a strong chance of becoming a real independent boundary because it:

- consumes scenario IR;
- interacts with external implementations;
- is used without generating docs/contracts;
- will likely gain multiple target adapters.

Therefore apply the repository's existing rule:

> Split when the boundary has been argued about twice.

A later split into:

```text
ess-conformance
```

is appropriate once the API proves itself.

Do not create it merely because this document names the concept.

---

## 36. Relationship to `ess-gen`

`ess-gen` should remain the owner of deterministic derivation from `EssIr`.

Scenario synthesis is a projection.

Therefore:

```text
EssIr
  ↓
ess-gen
  ├── docs
  ├── JSON Schema
  ├── OpenAPI
  ├── AsyncAPI
  └── ConformanceSuite
```

The **runner** is different.

It executes a `ConformanceSuite` against a target.

If implementation pressure makes this distinction awkward inside one crate, that is the first concrete argument for extracting `ess-conformance`.

---

## 37. Determinism

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

---

## 38. Generated Suite as an Artifact

Commit the generated billing suite or an equivalent canonical fixture.

Example:

```text
examples/billing/generated/
├── docs/
├── schema/
├── openapi/
├── asyncapi/
└── conformance/
    └── suite.json
```

The exact layout can follow `ess-gen` conventions.

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
ess-gen rust
    ↓
generated billing workspace
    ↓
same ConformanceSuite
    ↓
same runner
    ↓
passed
```

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

Recommended order after `ess-gen` projection work is stable enough to consume.

### Step 1 — Scenario IR

Implement:

```text
ConformanceSuite
ConformanceScenario
ScenarioStep
SuiteProvenance
```

Acceptance:

```text
billing EssIr → deterministic inspectable scenario suite
```

### Step 2 — Outcome scenarios

Generate:

- accepted outcomes;
- rejected outcomes;
- declared errors;
- expected emitted events;
- negative event assertions.

Acceptance:

```text
CreateInvoice has both happy and rejection cases
```

### Step 3 — View semantics

Implement:

- read-your-writes consistency token flow;
- eventual view assertions;
- runner deadline config.

Acceptance:

```text
no sleeps
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

Produce `EssConformanceReport`.

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

Illustrative only; adapt to the actual local `ess-gen` tree.

```text
engineering-protocols/
├── crates/
│   ├── ess-domain/
│   ├── ess-compiler/
│   └── ess-gen/
│       └── src/
│           └── conformance/
│               ├── mod.rs
│               ├── scenario.rs
│               ├── synthesize.rs
│               ├── target.rs
│               ├── runner.rs
│               └── report.rs
│
├── examples/
│   └── billing/
│       ├── domains/
│       ├── components.yaml
│       ├── topology.yaml
│       ├── generated/
│       │   └── conformance/
│       │       └── suite.json
│       │
│       └── reference/
│           ├── Cargo.toml
│           └── src/
│
└── conformance/
    └── ess/
        └── fault-matrix/
```

If the runner/target API becomes independently reusable and causes `ess-gen` to mix generation with execution, extract:

```text
crates/ess-conformance/
```

only then.

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
    waves 1–5
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
