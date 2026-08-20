# ESS Structural Synthesis, Obligations & Realizations — Design v0.1

> **Repository:** `codewandler/engineering-protocols`  
> **Status:** Proposed follow-on design after closed-loop ESS conformance  
> **Audience:** Implementors extending ESS from verified specification into generated applications and agent-completed implementation gaps  
> **Relationship to existing work:** Additive. This document assumes the ESS model/compiler, projection layer (`ess-gen`), and the closed-loop ESS conformance milestone already exist and are trusted.

---

## 1. Purpose

Once ESS can:

- describe a software system;
- compile to a normalized semantic IR;
- generate deterministic projections;
- generate a conformance suite;
- validate a correct implementation;
- reject deliberately faulty implementations;
- emit independent `EssConformance` evidence consumed by AEP/ADP;

the next logical milestone is **structural synthesis**.

The goal is not merely:

> Generate Rust code.

The real goal is:

> Determine exactly which parts of a system are fully specified and may be synthesized deterministically, which parts remain underdetermined and therefore become explicit implementation obligations, and which parts cannot safely be synthesized at all.

The target architecture is:

```text
EssIr
  ↓
SynthesisPlanner
  ↓
SynthesisPlan
  ├── Generated
  ├── Obligations
  └── Refusals
        │
        ▼
   RustGenerator
        │
        ▼
generated realization
        │
        ├── fully generated pieces
        └── unresolved obligations
                 │
                 ▼
               ADP
                 │
                 ▼
             agent/human
                 │
                 ▼
          implementation
                 │
                 ▼
       existing ESS conformance
                 │
                 ▼
          pass / counterexample
```

The key rule is:

> **Never guess. Generate it, create an explicit obligation, or refuse synthesis.**

---

## 2. Architectural Hinge

The next layer should not be:

```text
EssIr
  ↓
RustGenerator
  ↓
files
```

Instead:

```text
EssIr
  ↓
SynthesisPlanner
  ↓
SynthesisPlan
  ↓
target-specific generation
```

This intermediate plan is critical because it makes the synthesis boundary explicit and inspectable.

For every required semantic capability, the planner produces exactly one disposition:

```rust
pub enum SynthesisDisposition {
    Generated(GeneratedCapability),
    Obligation(ImplementationObligation),
    Refused(SynthesisRefusal),
}
```

This turns synthesis from “best-effort code generation” into a deterministic planning step.

---

## 3. `SynthesisPlan`

A `SynthesisPlan` is a first-class deterministic projection of the ESS.

Illustrative shape:

```rust
pub struct SynthesisPlan {
    pub specification: SpecificationIdentity,
    pub target: SynthesisTarget,
    pub generated: Vec<GeneratedCapability>,
    pub obligations: Vec<ImplementationObligation>,
    pub refusals: Vec<SynthesisRefusal>,
}
```

Example serialized plan:

```yaml
spec:
  id: billing
  version: 3
  digest: abc123

target:
  language: rust
  generator: ess-gen-rust/1

generated:

  - source: billing.invoice.Invoice
    kind: domain_type

  - source: billing.invoice.CreateInvoice
    kind: command_contract

  - source: billing.invoice.InvoiceCreated
    kind: event_type

  - source: billing.invoice.lifecycle
    kind: state_machine

  - source: billing.notify-on-invoice-created
    kind: binding

obligations:

  - id: billing.email.EmailGateway
    kind: external_effect

    contract:
      command: billing.email.SendEmail

    verification:
      scenarios:
        - email/send-success
        - email/send-failure

refused: []
```

Before code generation an implementor should be able to answer:

```text
What will be generated?

What is not derivable?

Why is it not derivable?

Which ESS element caused each obligation?

Which verifier/scenarios prove each obligation?

Which target capability caused a refusal?
```

---

## 4. Synthesis Categories

ESS constructs should be classified conservatively.

A useful initial matrix:

| ESS construct | Default synthesis disposition |
|---|---|
| Primitive/domain wrapper | Generated |
| Struct | Generated |
| Enum | Generated |
| Tagged union | Generated |
| Optional/List/Map | Generated |
| Command input type | Generated |
| Declared error type | Generated |
| Command outcome type | Generated |
| Event payload | Generated |
| View output type | Generated |
| Lifecycle state enum | Generated |
| Legal state transition API | Generated |
| Typed field mapping | Generated |
| Component command port | Generated |
| Component event port | Generated |
| View query port | Generated |
| Explicit HTTP surface | Generated where target supports it |
| Explicit event transport | Generated where target supports it |
| Arbitrary business algorithm | Obligation |
| External service behavior | Obligation |
| Persistence implementation | Usually obligation/adapter |
| Complex policy | Obligation |
| Unknown conversion/mapping | Obligation |
| Unsupported runtime guarantee | Refused or runtime obligation |

The dividing line is:

```text
semantic mechanics
    → synthesize aggressively

business choice / unspecified behavior
    → obligation

unsupported semantics
    → refusal
```

---

## 5. No Guessing

Generated code must never silently invent business semantics.

Bad:

```rust
fn calculate_tax(...) -> Money {
    // plausible-looking generated logic
}
```

unless the ESS completely specifies the rule.

Correct alternatives:

```text
fully specified
    → generate implementation

contract known, behavior unknown
    → ImplementationObligation

target cannot represent requirement
    → SynthesisRefusal
```

This keeps the ESS authoritative.

---

## 6. State-Safe Lifecycle Synthesis

ESS lifecycle definitions are particularly valuable for synthesis.

Example:

```text
Draft
Issued
Paid
Cancelled
```

with:

```text
IssueInvoice:
    Draft → Issued

PayInvoice:
    Issued → Paid

CancelInvoice:
    Draft | Issued → Cancelled
```

A naive generator might emit only:

```rust
enum InvoiceState {
    Draft,
    Issued,
    Paid,
    Cancelled,
}
```

The stronger target is state-safe domain APIs.

Conceptually:

```rust
struct Invoice<S> {
    data: InvoiceData,
    state: PhantomData<S>,
}
```

and:

```rust
impl Invoice<Draft> {
    pub fn issue(self) -> Invoice<Issued> {
        // generated transition
    }
}

impl Invoice<Issued> {
    pub fn pay(self) -> Invoice<Paid> {
        // generated transition
    }
}
```

This prevents invalid transitions inside the typed domain API.

---

## 7. Runtime State Requires a Hybrid Model

Pure typestate is insufficient because persisted or wire state is known only at runtime.

Therefore use:

```text
wire/storage representation
        ↓
runtime state enum
        ↓
validated refinement
        ↓
typed domain state
```

Example:

```rust
pub enum InvoiceState {
    Draft,
    Issued,
    Paid,
    Cancelled,
}

pub struct InvoiceSnapshot {
    pub state: InvoiceState,
    pub data: InvoiceData,
}

pub enum AnyInvoice {
    Draft(Invoice<Draft>),
    Issued(Invoice<Issued>),
    Paid(Invoice<Paid>),
    Cancelled(Invoice<Cancelled>),
}
```

Then:

```rust
let invoice = snapshot.refine()?;

match invoice {
    AnyInvoice::Draft(invoice) => { /* ... */ }
    AnyInvoice::Issued(invoice) => { /* ... */ }
    AnyInvoice::Paid(invoice) => { /* ... */ }
    AnyInvoice::Cancelled(invoice) => { /* ... */ }
}
```

The goal is:

> Runtime validation at the system boundary; strong state guarantees inside the domain.

---

## 8. Command Outcomes as Algebraic Types

Commands should not synthesize to:

```rust
Result<(), Error>
```

when ESS already declares multiple semantic outcomes.

Example ESS semantics:

```text
CreateInvoice
  accepted
  rejected InvalidAmount
```

Generated Rust should preserve that distinction:

```rust
pub enum CreateInvoiceOutcome {
    Accepted {
        invoice_created: InvoiceCreated,
    },

    Rejected {
        error: InvalidAmount,
    },
}
```

Then:

```rust
pub trait CreateInvoiceHandler {
    async fn handle(
        &self,
        command: CreateInvoice,
    ) -> CreateInvoiceOutcome;
}
```

Infrastructure errors should remain separate:

```rust
Result<CreateInvoiceOutcome, InfrastructureError>
```

This prevents conflating:

```text
domain rejection
```

with:

```text
transport/storage/runtime failure
```

and keeps generated code aligned with ESS conformance semantics.

---

## 9. Events as Semantic Values

Domain events should be generated as values, not transport-aware objects.

Example:

```rust
pub struct InvoiceCreated {
    pub invoice_id: InvoiceId,
    pub customer_email: Email,
    pub amount: Money,
}
```

The generated domain event should know nothing about Kafka.

Instead generate ports:

```rust
pub trait InvoiceEvents {
    async fn publish(
        &self,
        event: InvoiceEvent,
    ) -> Result<(), PublishError>;
}
```

Then transport projections provide:

```text
InvoiceEvent
    ├── local event bus
    ├── Kafka adapter
    └── NATS adapter
```

The semantic event remains unchanged.

---

## 10. Binding Synthesis

Typed ESS bindings are one of the strongest synthesis opportunities.

Example:

```text
InvoiceCreated.customer_email : Email
SendEmail.recipient            : Email
```

with ESS mapping:

```yaml
recipient: event.customer_email
```

can deterministically synthesize:

```rust
pub fn map_invoice_created_to_send_email(
    event: &InvoiceCreated,
) -> SendEmail {
    SendEmail {
        recipient: event.customer_email.clone(),
        template: EmailTemplate::InvoiceCreated,
    }
}
```

This is actual constrained program synthesis.

No LLM is required because the program is fully determined by the specification.

---

## 11. Binding Mapping vs Delivery Semantics

Do not conflate:

```text
what should happen?
```

with:

```text
how reliably must it happen?
```

Example:

```text
Binding
├── transformation
│    InvoiceCreated → SendEmail
│
└── delivery
     at_least_once
     on_failure: escalate
```

The transformation may be completely generated.

The delivery guarantee depends on target runtime capabilities.

Therefore the synthesis planner must consider both:

```text
ESS semantics
+
target capabilities
```

---

## 12. Typed Synthesis Targets

A target should eventually be more precise than:

```text
--language rust
```

Conceptually:

```yaml
target:

  language:
    rust:
      edition: 2024

  runtime:
    component_model: processes

  transports:
    commands: http
    events: kafka

  persistence:
    kind: postgres
```

Then:

```text
ESS requirements
       +
target capabilities
       ↓
SynthesisPlan
```

Example:

```text
ESS:
    delivery = at_least_once

Target:
    Kafka adapter with at-least-once support

Result:
    Generated
```

Versus:

```text
Target:
    unsupported callback runtime

Result:
    Refused
```

or a generated runtime capability obligation.

---

## 13. `ImplementationObligation`

An implementation obligation means:

> The ESS defines the required contract, but the implementation is not deterministically derivable.

Example:

```yaml
id: billing.email.EmailGateway

type: ess.implementation-obligation/v1

source:
  command: billing.email.SendEmail

reason:
  kind: external-effect

contract:

  input:
    type: billing.email.SendEmail

  outcomes:
    - sent
    - failed

verification:

  required_scenarios:
    - billing.email.SendEmail/sent
    - billing.email.SendEmail/failed
```

This is dramatically stronger than:

```rust
todo!("send email")
```

because the obligation is machine-readable and auditable.

---

## 14. Obligation Taxonomy

A small first taxonomy is useful:

```rust
pub enum ObligationKind {
    BusinessPolicy,
    ExternalEffect,
    Persistence,
    Transformation,
    Algorithm,
    SecurityPolicy,
    RuntimeCapability,
}
```

Examples:

```text
PricingPolicy
    BusinessPolicy

EmailGateway
    ExternalEffect

InvoiceRepository
    Persistence

Email → VerifiedEmail
    Transformation

Tax calculation
    Algorithm

AuthorizationPolicy
    SecurityPolicy

AtLeastOnceDelivery
    RuntimeCapability
```

Different obligation kinds may later select different ADP profiles and evidence requirements.

---

## 15. Obligations Must Be Derived

Implementation obligations should not usually be handwritten work items.

They should be derived:

```text
ESS
 ↓
required behavior
 ↓
target cannot synthesize it
 ↓
ImplementationObligation
```

This gives every obligation provenance.

If the ESS changes:

```text
ESS revision changes
    ↓
obligation may disappear
    or
contract digest changes
```

The obligation therefore tracks actual semantic incompleteness rather than project-management intention.

---

## 16. Obligation Identity

Each obligation should have:

```rust
pub struct ImplementationObligation {
    pub id: ObligationId,
    pub locator: EntityLocator,

    pub source: Vec<EssRef>,

    pub kind: ObligationKind,
    pub contract: ObligationContract,

    pub contract_digest: Digest,

    pub verification: VerificationPlan,
}
```

The stable identity tracks the conceptual obligation.

The `contract_digest` answers:

> Does an existing implementation/evidence still satisfy the current contract revision?

This mirrors revision-bound design approvals.

---

## 17. Generated/User Ownership Boundary

Generated code must remain disposable.

Recommended layout:

```text
billing/
├── generated/
│   ├── billing-domain/
│   ├── invoice-component/
│   ├── email-component/
│   └── runtime-contract/
│
├── implementations/
│   ├── email-gateway/
│   └── pricing-policy/
│
└── Cargo.toml
```

Rule:

```text
generated/
    ESS owns completely

implementations/
    humans/agents own
```

Regeneration may destroy and recreate `generated/`.

It must never rewrite arbitrary authored implementation files.

---

## 18. Generated Interfaces Instead of `todo!()`

Avoid unresolved runtime panics.

Bad:

```rust
pub async fn send_email(...) {
    todo!()
}
```

Better:

```rust
pub trait EmailGateway {
    async fn execute(
        &self,
        command: SendEmail,
    ) -> SendEmailOutcome;
}
```

The system then reports:

```text
UNSATISFIED OBLIGATION:
billing.email.EmailGateway
```

A build/link step can require an implementation.

Incompleteness becomes explicit in the synthesis graph.

---

## 19. Synthesis Manifest

Every synthesis run should emit provenance.

Example:

```yaml
synthesis:

  spec:
    digest: abc123

  generator:
    version: 0.5.0

  target:
    id: rust-http-kafka/v1

  files:

    generated/billing-domain/src/invoice.rs:
      derived_from:
        - billing.invoice.Invoice

    generated/email-component/src/commands.rs:
      derived_from:
        - billing.email.SendEmail

  obligations:

    - id: billing.email.EmailGateway
      required_by:
        - billing.email.SendEmail
```

This creates a complete map:

```text
ESS semantic element
    ↔ generated artifact
    ↔ implementation obligation
```

---

## 20. Linker Model

A useful compiler analogy:

```text
generated application
    +
unresolved implementation obligations
```

behaves like:

```text
object code
    +
unresolved symbols
```

Define:

```rust
pub struct ImplementationSet {
    pub implementations: Vec<ImplementationDescriptor>,
}
```

Then conceptually:

```rust
pub fn link(
    plan: &SynthesisPlan,
    implementations: &ImplementationSet,
) -> Result<LinkedSystem, UnsatisfiedObligations>;
```

Example:

```text
Required:
    EmailGateway
    PricingPolicy

Provided:
    SesEmailGateway
    StandardPricingPolicy
```

When all required obligations are satisfied:

```text
linked realization
```

can be built and tested.

---

## 21. Multiple Implementations per Obligation

One obligation may have multiple valid implementations:

```text
EmailGateway
    ├── SesEmailGateway
    ├── SendgridEmailGateway
    └── MockEmailGateway
```

They can each declare:

```yaml
implements:
  obligation: billing.email.EmailGateway
  contract_digest: abc123
```

Different realizations can choose different implementations while preserving the same ESS.

---

## 22. `ImplementationObligation` as AEP Artifact

Implementation obligations should become addressable engineering artifacts.

Suggested concept:

```rust
ArtifactKind::ImplementationObligation
```

Possible graph:

```text
ESS
 └── derives → ImplementationObligation

Task
 └── implements → ImplementationObligation

ChangeSet
 └── satisfies → ImplementationObligation

ConformanceEvidence
 └── verifies → ImplementationObligation
```

The obligation is neither a story nor a specification.

It is a machine-derived implementation requirement.

---

## 23. Obligation → ADP Task

A synthesis obligation can become the subject of an AEP task.

Example:

```yaml
task:

  type: implementation

  subject:
    obligation: billing.email.EmailGateway

  objective:
    satisfy: billing.email.EmailGateway

  context:
    specification: billing/v3
    contract_digest: abc123

  protocol:
    profile: development.standard

  completion:
    require:
      - obligation_tests_passed
      - ess_conformance_passed
```

This reduces agent scope dramatically.

The agent receives:

```text
Implement this missing semantic contract
```

instead of:

```text
Implement the email service
```

---

## 24. Targeted vs Full Verification

Obligation work should have two verification levels.

### Development loop

Run the smallest relevant scenario subset:

```text
EmailGateway
    ↓
SendEmail/success
SendEmail/failure
```

This gives fast counterexamples.

### Final gate

Run the full canonical ESS suite:

```text
all system conformance scenarios
```

The final gate must remain the independent `EssConformance` oracle.

Targeted verification is not equivalent to full system conformance.

---

## 25. CEGIS-Like Agent Loop

Once obligations are explicit:

```text
ImplementationObligation
        ↓
Agent proposes candidate
        ↓
Compiler
        ↓
Targeted ESS verifier
        ↓
     pass?
     /   \
   no     yes
   │       │
counter-   ▼
example  full ESS suite
   │       │
   └──↺    ▼
       EssConformance
```

The agent is the candidate generator.

The verifier remains authoritative.

This is effectively a constrained counterexample-guided synthesis loop.

---

## 26. Counterexample Structure

Failure feedback should preserve ESS provenance.

Example:

```yaml
counterexample:

  obligation:
    id: billing.invoice.CreateInvoiceHandler

  source:
    command: billing.invoice.CreateInvoice
    outcome: rejected

  input:
    amount:
      amount: 0
      currency: EUR

  expected:
    outcome: rejected
    error: InvalidAmount
    absent_events:
      - InvoiceCreated

  observed:
    outcome: accepted
    emitted_events:
      - InvoiceCreated
```

This can be handed directly to the coding agent.

---

## 27. Context Engineering from the Artifact Graph

An obligation gives a deterministic context boundary.

For:

```text
billing.email.EmailGateway
```

the system can resolve:

```text
obligation
 ├── source command
 ├── input/output types
 ├── declared outcomes
 ├── relevant entity state
 ├── emitted events
 ├── downstream bindings
 ├── affected views
 ├── related design/ADRs
 ├── generated trait
 └── failing counterexamples
```

This allows a minimal, relevant agent context instead of broad repository dumping.

---

## 28. Capability Derivation

Obligation kinds can inform safe capability defaults.

Examples:

```text
BusinessPolicy
    repository.read
    repository.write
    tests.execute

ExternalEffect
    repository.*
    tests.execute
    controlled external fake

Persistence
    repository.*
    tests.execute
    local database fixture

RuntimeCapability
    runtime module scope only
```

Production capabilities should remain denied unless separately required by AEP/AOP.

This aligns obligation work with least privilege.

---

## 29. Cacheable Conformance Evidence

Implementation evidence can be cached against stable identities.

Example:

```text
implementation commit = SHA-X
obligation contract digest = D1
suite version = S1
result = passed
```

If none of those change, the evidence remains relevant.

If:

```text
ESS changes
    ↓
contract digest becomes D2
```

prior evidence no longer proves the current obligation.

This enables precise invalidation.

---

## 30. `Realization`

Introduce one additional first-class object:

> A `Realization` is one concrete implementation of a specific ESS using a specific synthesis target and a specific set of obligation implementations.

Example:

```yaml
type: ess.realization/v1

id: billing-production

specification:
  billing/v3

synthesis_target:
  rust-http-kafka/v1

components:

  invoice:
    implementation:
      generated: true

  email:
    implementation:
      obligation: billing.email.EmailGateway
      artifact: SesEmailGateway@abc123

conformance:
  report: report:991
```

The distinction is:

```text
ESS
    abstract semantic system

Realization
    concrete implementation choice
```

---

## 31. Realization Identity

A realization should be reproducible from:

```text
ESS digest
+
synthesis target
+
generator version
+
resolved implementation set
```

Example:

```yaml
build:

  ess_digest: 63a7...
  generator: ess-gen-rust/1
  target: rust-processes-http-kafka/1

  implementations:
    EmailGateway: git:abc123
    PricingPolicy: git:def456
```

This creates a strong build identity.

---

## 32. Realization as Release Input

A release should deploy a realization, not just an arbitrary source SHA.

Conceptually:

```text
ESS
 ↓
SynthesisPlan
 ↓
ImplementationSet
 ↓
Realization
 ↓
EssConformanceReport
 ↓
Release
```

A release protocol can require:

```yaml
requires:

  - realization.ess_conformance == passed
  - realization.obligations_unresolved == 0
  - realization.build_reproducible == true
```

This is the bridge from ESS/ADP into release/AOP work.

---

## 33. Artifact Graph

The graph becomes:

```text
PRD
 │
 ▼
ESS
 │
 ├── generates → SynthesisPlan
 │                 │
 │                 ├── generates → GeneratedArtifact*
 │                 │
 │                 └── derives → ImplementationObligation*
 │                                      │
 │                                      ▼
 │                                  AEP Task
 │                                      │
 │                                      ▼
 │                                  ChangeSet
 │
 ▼
Realization
 │
 ├── contains → GeneratedArtifact*
 ├── satisfies → ImplementationObligation*
 │
 ▼
EssConformanceReport
 │
 ▼
Evidence
 │
 ▼
ADP Complete
 │
 ▼
Release
 │
 ▼
AOP
```

This gives a coherent provenance path from product intent to running system.

---

## 34. Multiple Physical Realizations

After generated Rust works, the next architectural test should be two physical realizations of the same ESS.

Example:

```text
                    Billing ESS
                       │
            ┌──────────┴──────────┐
            ▼                     ▼
      modular process       split processes
            │                     │
       local binding          HTTP + events
            │                     │
            └──────────┬──────────┘
                       ▼
                same conformance
```

The canonical ESS suite must remain unchanged.

Only the target/adapter changes.

Passing both proves that ESS semantics are genuinely independent of deployment topology.

---

## 35. Topology Synthesis Comes Later

Once multiple realizations work:

```text
ESS topology requirements
        ↓
platform target
        ↓
Kubernetes projection
```

Example:

```yaml
ESS:

invoice-service:
  replicas:
    min: 2

requires:
  - postgres
  - event-delivery
```

Target:

```yaml
platform:
  kubernetes: 1.34

database:
  postgres:
    provider: cloudnative-pg

events:
  kafka:
    operator: strimzi
```

Apply the same synthesis algebra:

```text
fully derivable → Generated
decision required → Obligation
unsupported → Refused
```

Do not jump to Kubernetes before semantic realization independence is proven.

---

## 36. Behavioral Synthesis

Later ESS can become expressive enough to eliminate some obligations.

Example initial state:

```text
PricingPolicy
    → obligation
```

Later ESS adds formal policy rules:

```yaml
policy: PricingPolicy

rules:

  - when:
      customer.tier == premium

    result:
      discount_percent: 10

  - otherwise:
      discount_percent: 0
```

Now:

```text
previous obligation
    ↓
new ESS semantics
    ↓
fully synthesized behavior
```

This is a natural path toward richer system synthesis.

---

## 37. Synthesis Coverage

A useful metric may be:

```text
billing/v3

semantic units:       42
fully synthesized:    35
obligations:           7
refused:               0

synthesis coverage:   83%
```

This should not be treated as a quality score.

It is useful only as a measure of:

> How much of this system is completely determined by the current ESS vocabulary and target?

---

## 38. Agents as Residual Synthesizers

The deepest architectural consequence is:

```text
Requirements
    ↓
ESS
    ↓
deterministic synthesis
    ↓
remaining obligations
    ↓
agentic synthesis
```

The agent handles only what deterministic synthesis cannot derive.

A useful framing is:

> **Deterministic synthesis handles the specified portion of the program; agentic synthesis handles the residual implementation obligations.**

The LLM becomes a residual synthesizer rather than the primary generator of the entire software system.

---

## 39. Incremental Assurance

Different obligation types can require different levels of verification.

Examples:

```text
simple mapping
    → typecheck + conformance

parser
    → property testing

state machine
    → exhaustive model checking

security-critical algorithm
    → formal proof
```

An obligation may eventually declare:

```yaml
assurance:

  required:
    - typecheck
    - conformance

  additional:
    - property-test

  critical:
    - formal-proof
```

AEP can then translate risk into required evidence.

Formal verification need not apply to the whole application.

---

## 40. Suggested Implementation Roadmap

After closed-loop conformance is complete:

### W5.1 — `SynthesisPlan`

Implement:

```text
Generated
Obligation
Refused
```

Acceptance:

```text
billing EssIr → deterministic inspectable plan
```

### W5.2 — Rust semantic types

Generate:

```text
value objects
entities
events
commands
declared errors
outcomes
views
```

### W5.3 — State-safe lifecycles

Generate:

```text
runtime state enum
typed refined states
legal transition APIs
```

### W5.4 — Generated ports

Generate:

```text
command handlers
event publishers
view query ports
external dependency ports
```

### W5.5 — Deterministic binding mappings

Generate:

```text
event → command mapping code
```

when fully typed and specified.

### W5.6 — `ImplementationObligation`

Introduce:

```text
identity
contract
kind
provenance
verification plan
contract digest
```

### W5.7 — Generated workspace boundary

Create:

```text
generated/
implementations/
```

with strict ownership rules.

### W5.8 — Manual obligation resolution

Implement billing obligations by hand.

### W5.9 — Generated billing realization

Link generated + manual pieces.

Run the existing ESS conformance suite unchanged.

### W5.10 — Falsifiability

Override or corrupt one generated/custom implementation.

Ensure the same suite still fails correctly.

---

## 41. Agent Integration Roadmap

### W6.1 — Materialize obligations as AEP artifacts

Add:

```rust
ArtifactKind::ImplementationObligation
```

and relations.

### W6.2 — Obligation-derived task

Create an ADP task whose subject is one obligation.

### W6.3 — Agent implementation

Provide only the obligation-specific context closure.

### W6.4 — Targeted verification

Run relevant scenarios and produce counterexamples.

### W6.5 — Repair loop

Agent revises candidate until targeted scenarios pass.

### W6.6 — Full ESS gate

Run full canonical conformance.

### W6.7 — Independent evidence

Produce `EssConformance`.

### W6.8 — ADP completion

Task completes only after full conformance passes.

---

## 42. Realization Roadmap

### W7.1 — `Realization` model

Define:

```text
ESS version
synthesis target
generator version
implementation set
build identity
conformance report
```

### W7.2 — First realization

Generated + manually resolved billing system.

### W7.3 — Second realization

Same ESS with different physical deployment shape.

### W7.4 — Same oracle

Both must pass the exact same semantic conformance suite.

---

## 43. Later Roadmap

After realizations are proven:

```text
W8   Topology synthesis

W9   Richer behavioral synthesis

W10  Property-based synthesis/testing

W11  Model checking

W12  Selective formal verification
```

Each later layer should reuse the same foundational rule:

```text
specified
    → deterministic machinery

underspecified
    → explicit obligation

unsupported
    → refusal
```

---

## 44. Acceptance Criteria for Structural Synthesis

Structural synthesis is complete when:

- A deterministic `SynthesisPlan` is produced from billing `EssIr`.
- Every synthesis decision is `Generated`, `Obligation`, or `Refused`.
- No best-effort guessed behavior exists.
- Domain/value/event/command/view types are generated.
- Declared command outcomes are represented explicitly.
- Lifecycle legality is reflected in generated APIs.
- Typed binding mappings are generated.
- Unspecified behavior becomes addressable `ImplementationObligation`s.
- Generated files are fully disposable.
- Manual implementations satisfy generated interfaces without editing generated code.
- A linked billing realization builds.
- The pre-existing ESS conformance suite passes unchanged.
- Deliberately faulty implementations still fail the same oracle.
- Synthesis output includes complete provenance.

---

## 45. Acceptance Criteria for Agent Completion

The agentic milestone is complete when:

- One generated `ImplementationObligation` becomes an AEP artifact.
- An ADP task is created for that obligation.
- The agent receives only the relevant semantic/context closure.
- The agent proposes a candidate implementation.
- Targeted ESS scenarios produce structured counterexamples.
- The agent repairs against those counterexamples.
- Full ESS conformance runs independently.
- Passed conformance becomes `EssConformance` evidence.
- ADP completes only on valid independent evidence.
- A failing implementation cannot self-certify completion.

---

## 46. Core System Thesis

The complete system becomes:

```text
                     ESS
                      │
              deterministic compiler
                      │
             ┌────────┴────────┐
             ▼                 ▼
      generated software   obligations
                               │
                               ▼
                              ADP
                               │
                               ▼
                         coding agent
                               │
                               ▼
                           candidate
                               │
                               ▼
                     ESS conformance oracle
                               │
                    ┌──────────┴──────────┐
                    ▼                     ▼
                 passed               failed
                    │                     │
                    ▼                     ▼
              realization          counterexample
                    │                     │
                    ▼                     └──→ repair
             release / AOP
```

The important architectural division is:

```text
ESS
    defines system semantics

SynthesisPlanner
    determines what is mechanically derivable

System synthesis
    generates everything fully determined

ImplementationObligation
    records everything still underdetermined

ADP agent
    solves those residual obligations

ESS conformance
    independently judges the result

Realization
    records one concrete fully linked system

AEP/AOP
    govern its engineering and operation
```

---

## 47. Final Principle

> **Do not use the agent where the compiler already has enough information.**

And equally:

> **Do not let the compiler invent behavior where the specification does not contain enough information.**

The intended progression is therefore:

```text
specification
    ↓
deterministic synthesis
    ↓
explicit residual obligations
    ↓
agentic synthesis
    ↓
independent conformance
    ↓
realization
    ↓
release
```

This keeps the source of truth singular, the generated software reproducible, the agent scope constrained, and every correctness claim falsifiable.
