# Engineering Protocols — Design Draft v0.1

## 1. Summary

`engineering-protocols` defines a machine-executable methodology for agentic engineering work.

The core idea is to move engineering principles such as spec-driven development, test-driven development, least privilege, reversible changes, incident response, release verification, and evidence collection **out of natural-language prompts and into strongly typed protocol definitions**.

The project provides:

- a strongly typed Rust domain model;
- executable protocol and workflow semantics;
- generated JSON Schema for interchange;
- reusable principles and profiles;
- deterministic state-transition rules;
- evidence and verification primitives;
- capability and approval policies;
- a stable interface for agent harnesses.

The language model performs reasoning and proposes actions. The protocol implementation determines whether those actions are permitted and whether sufficient evidence exists to advance the workflow.

---

## 2. Motivation

Current coding and operations agents are commonly governed by natural-language instructions such as:

> Follow TDD, avoid breaking existing APIs, verify your work, and ask before deploying.

These instructions are useful to humans but weak as machine-enforceable policy.

They leave several questions unresolved:

- What exactly constitutes following TDD?
- What evidence proves that a test failed before implementation?
- What actions can the agent perform?
- Which operations require approval?
- What does "verify your work" mean?
- When is a task considered complete?
- What happens when verification fails?
- How does one harness interpret the methodology consistently with another?

`engineering-protocols` makes these concepts explicit and executable.

The desired model is:

```text
Task
  ↓
Protocol resolution
  ↓
Allowed actions + obligations
  ↓
Agent proposes / executes work
  ↓
Evidence collection
  ↓
Independent verification
  ↓
Deterministic state transition
  ↓
Complete or iterate
```

The agent may be probabilistic.

The protocol semantics should not be.

---

## 3. Terminology

### AEP — Agentic Engineering Protocol

The common protocol model covering agentic engineering work.

AEP defines shared concepts such as:

- tasks;
- principles;
- workflows;
- states;
- transitions;
- actions;
- capabilities;
- evidence;
- verification;
- approvals;
- failure handling;
- completion predicates.

### ADP — Agentic Development Protocol

An AEP profile for software development work.

Typical concerns include:

- specification;
- decomposition;
- implementation;
- TDD;
- contract testing;
- static analysis;
- property testing;
- regression verification;
- review.

### AOP — Agentic Operations Protocol

An AEP profile for operational and SRE work.

Typical concerns include:

- telemetry;
- diagnosis;
- change planning;
- blast-radius control;
- production permissions;
- reversible changes;
- health verification;
- rollback.

Additional profiles may include:

- incident response;
- release management;
- migrations;
- security response;
- dependency upgrades;
- infrastructure changes.

Profiles share the same core protocol model rather than introducing independent execution systems.

---

## 4. Design Principles

### 4.1 Rust is the source of truth

Protocol concepts are modeled first as Rust types.

JSON Schema and other wire-format specifications are generated from these types.

```text
Rust domain model
      ↓
Generated JSON Schema
      ↓
YAML / JSON documents
      ↓
Parser
      ↓
Validated Rust model
      ↓
Protocol execution
```

Schemas are therefore outputs rather than independently maintained definitions.

### 4.2 Invalid states should be difficult to represent

The runtime should distinguish between parsed input and validated domain objects.

```rust
RawWorkflow
    ↓ validate
Workflow
```

Runtime execution should operate only on validated objects.

### 4.3 Generation and verification are separate

Agents may generate:

- plans;
- specifications;
- tests;
- code;
- hypotheses;
- fixes;
- proposed invariants.

Agents should not be trusted as the sole verifier of those outputs.

Where possible, verification is delegated to deterministic or independent systems:

```text
code                  → compiler
types                 → type checker
tests                 → test runner
API compatibility     → contract verifier
properties            → property tester
release health        → telemetry
production mutation   → policy engine
completion            → protocol engine
```

### 4.4 Evidence drives transitions

A workflow does not advance because the agent claims that a step is complete.

It advances because required evidence satisfies a transition predicate.

### 4.5 Capabilities are explicit

The protocol operates on semantic capabilities rather than assuming unrestricted shell access.

Examples:

```text
repository.read
repository.write
tests.execute
network.read
network.write
production.read
production.write
deployment.create
deployment.rollback
telemetry.query
secret.read
```

### 4.6 Profiles compose principles

A task should normally select a profile rather than manually enumerate dozens of rules.

```yaml
protocol: aep/1
profile: development.standard
```

Profiles may then be customized:

```yaml
principles:
  add:
    - clean-room
    - differential-testing

  remove:
    - mutation-testing
```

---

## 5. Repository Structure

Initial layout:

```text
engineering-protocols/
├── Cargo.toml
├── crates/
│   ├── aep-domain/
│   ├── aep-engine/
│   ├── aep-schema/
│   ├── adp-domain/
│   ├── aop-domain/
│   └── protocol-cli/
│
├── protocols/
│   ├── aep/
│   ├── adp/
│   └── aop/
│
├── principles/
│   ├── development/
│   ├── verification/
│   ├── operations/
│   └── governance/
│
├── workflows/
│   ├── development/
│   ├── incidents/
│   ├── releases/
│   └── migrations/
│
├── profiles/
│   ├── development-fast.yaml
│   ├── development-standard.yaml
│   ├── development-critical.yaml
│   ├── incident-standard.yaml
│   └── release-progressive.yaml
│
├── schemas/
│   └── generated/
│
├── examples/
│
└── xtask/
```

### `aep-domain`

Contains protocol-neutral domain concepts.

### `aep-engine`

Evaluates policies, predicates, evidence, and state transitions.

### `adp-domain`

Contains development-specific concepts.

### `aop-domain`

Contains operations-specific concepts.

### `aep-schema`

Schema generation and wire representations.

### `protocol-cli`

Reference CLI for:

```text
validate
resolve
inspect
evaluate
schema
explain
```

---

## 6. Core Domain Model

A minimal top-level task could resemble:

```rust
pub struct Task {
    pub id: TaskId,
    pub kind: TaskKind,
    pub objective: Objective,
    pub protocol: ProtocolRef,
    pub profile: ProfileRef,
    pub constraints: Constraints,
    pub principle_overrides: PrincipleOverrides,
}
```

A resolved execution instance becomes:

```rust
pub struct ExecutionPlan {
    pub task: Task,
    pub protocol: Protocol,
    pub principles: Vec<Principle>,
    pub workflow: Workflow,
    pub capability_policy: CapabilityPolicy,
    pub completion: Predicate,
}
```

---

## 7. Principles

A principle describes an enforceable engineering rule.

Conceptually:

```rust
pub struct Principle {
    pub id: PrincipleId,
    pub version: Version,

    pub applicability: Predicate,

    pub obligations: Vec<Obligation>,
    pub evidence: Vec<EvidenceRequirement>,
    pub verification: Vec<VerificationRequirement>,

    pub capabilities: CapabilityPolicy,
    pub failure_policy: FailurePolicy,
}
```

Examples include:

```text
spec-driven
test-driven
contract-testing
clean-room
property-based-testing
differential-testing
mutation-testing
static-analysis
design-by-contract
invariant-checking
reversible-changes
least-privilege
approval-gates
provenance-tracking
preserve-evidence
blast-radius-limitation
progressive-delivery
```

A principle therefore does not merely say:

> Use test-driven development.

It specifies what that means operationally.

Example:

```yaml
id: test-driven
version: 1

applies_when:
  task.kind:
    any_of:
      - feature
      - bugfix

requires:
  before_implementation:
    - test.exists
    - test.result == failed

  before_completion:
    - test.result == passed
    - regression_suite.result == passed

evidence:
  - test_execution
  - source_diff
```

---

## 8. Workflow Model

A workflow is a state machine.

```rust
pub struct Workflow {
    pub id: WorkflowId,
    pub initial: StateId,
    pub states: BTreeMap<StateId, State>,
    pub transitions: Vec<Transition>,
}
```

Example development workflow:

```text
RECEIVE
   ↓
SPECIFY
   ↓
DECOMPOSE
   ↓
ESTABLISH_VERIFIERS
   ↓
IMPLEMENT
   ↓
VERIFY
   ↓
ADVERSARIAL_VERIFY
   ↓
REVIEW
   ↓
COMPLETE
```

A transition contains a predicate:

```rust
pub struct Transition {
    pub from: StateId,
    pub to: StateId,
    pub when: Predicate,
}
```

Example:

```yaml
from: verify
to: review

when:
  all:
    - unit_tests.failed == 0
    - contract_tests.failed == 0
    - static_analysis.errors == 0
    - required_evidence.missing == 0
```

The harness cannot move to `review` until the predicate evaluates to true.

---

## 9. Actions and Capabilities

Actions represent meaningful operations available to an agent.

```rust
pub enum Action {
    RepositoryRead(RepositoryRead),
    RepositoryWrite(RepositoryWrite),
    TestExecute(TestExecute),
    CommandExecute(CommandExecute),
    NetworkRequest(NetworkRequest),
    TelemetryQuery(TelemetryQuery),
    Deploy(Deploy),
    Rollback(Rollback),
    SecretRead(SecretRead),
}
```

Capabilities authorize categories of actions.

```rust
pub enum Capability {
    RepositoryRead,
    RepositoryWrite,
    TestExecution,
    NetworkRead,
    NetworkWrite,
    TelemetryRead,
    ProductionRead,
    ProductionWrite,
    Deploy(Environment),
    Rollback(Environment),
}
```

An execution context may expose:

```rust
pub struct CapabilityPolicy {
    pub allow: BTreeSet<Capability>,
    pub deny: BTreeSet<Capability>,
    pub approval_required: BTreeSet<Capability>,
}
```

This enables protocols such as:

```yaml
capabilities:
  allow:
    - repository.read
    - repository.write
    - tests.execute

  require_approval:
    - production.write

  deny:
    - secret.read
```

The harness remains responsible for translating protocol capabilities into actual tool access.

---

## 10. Evidence

Evidence represents observable facts produced during execution.

```rust
pub enum Evidence {
    TestResult(TestResult),
    StaticAnalysis(StaticAnalysisResult),
    ContractResult(ContractResult),
    PropertyTestResult(PropertyTestResult),
    DeploymentResult(DeploymentResult),
    MetricObservation(MetricObservation),
    Approval(ApprovalRecord),
    Diff(ChangeSet),
    Artifact(ArtifactRecord),
}
```

Evidence should contain provenance.

```rust
pub struct EvidenceEnvelope<T> {
    pub id: EvidenceId,
    pub produced_at: Timestamp,
    pub producer: Producer,
    pub subject: SubjectRef,
    pub value: T,
    pub provenance: Provenance,
}
```

The protocol engine evaluates evidence but should not generally manufacture it.

---

## 11. Verification

Verification evaluates claims or evidence.

Possible verifier classes include:

```rust
pub enum Verifier {
    Compiler,
    TestRunner,
    ContractRunner,
    StaticAnalyzer,
    PropertyTester,
    ModelChecker,
    TelemetryQuery,
    PolicyEngine,
    HumanApproval,
    ExternalTool(ToolRef),
}
```

Verification produces structured results:

```rust
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub evidence: Vec<Evidence>,
    pub counterexamples: Vec<Counterexample>,
}
```

Failures should preferably produce actionable counterexamples.

```yaml
counterexample:
  verifier: property-test

  property:
    parse_serialize_roundtrip

  input:
    amount: "-0.00"
    currency: "EUR"

  expected:
    semantic_equivalence: true

  observed:
    semantic_equivalence: false
```

The result can be fed back into the agent without allowing the agent to alter the verification criterion.

---

## 12. Harness Interface

The protocol library should not itself be an agent harness.

Instead it exposes deterministic decisions to one.

Possible interface:

```rust
pub trait ProtocolEngine {
    fn initialize(
        &self,
        task: Task,
    ) -> Result<Execution, ProtocolError>;

    fn requirements(
        &self,
        execution: &Execution,
    ) -> Vec<Requirement>;

    fn capabilities(
        &self,
        execution: &Execution,
    ) -> CapabilityPolicy;

    fn submit_evidence(
        &self,
        execution: &mut Execution,
        evidence: Evidence,
    ) -> Result<(), ProtocolError>;

    fn evaluate(
        &self,
        execution: &Execution,
    ) -> Evaluation;

    fn transition(
        &self,
        execution: &mut Execution,
    ) -> Result<TransitionResult, ProtocolError>;
}
```

The interaction becomes:

```text
Harness
   │
   ├── asks protocol for current requirements
   │
   ├── asks protocol for allowed capabilities
   │
   ├── exposes permitted tools to agent
   │
   ├── agent performs work
   │
   ├── harness collects evidence
   │
   ├── submits evidence
   │
   └── asks protocol whether transition is permitted
```

---

## 13. Development Profile Example

```yaml
protocol: aep/1
profile: development.standard

workflow: adp/default

principles:
  - spec-driven
  - test-driven
  - contract-testing
  - property-based-testing
  - static-analysis
  - sandboxed-execution
  - provenance-tracking
  - least-privilege

completion:
  all:
    - specification.satisfied
    - tests.unit.failed == 0
    - tests.contract.failed == 0
    - static_analysis.errors == 0
    - evidence.missing == 0
```

A higher-assurance profile may additionally require:

```text
mutation-testing
fuzz-testing
differential-testing
invariant-checking
formal-verification
approval-gates
```

---

## 14. Incident Profile Example

Incident handling uses the same AEP primitives.

```text
DETECT
   ↓
TRIAGE
   ↓
DIAGNOSE
   ↓
MITIGATE
   ↓
RECOVER
   ↓
VERIFY
   ↓
LEARN
```

Example:

```yaml
protocol: aep/1
profile: incident.standard

principles:
  - preserve-evidence
  - least-privilege
  - hypothesis-driven-diagnosis
  - reversible-changes
  - blast-radius-limitation
  - verify-after-action
  - provenance-tracking

capabilities:
  allow:
    - telemetry.read
    - production.read

  require_approval:
    - production.write

completion:
  all:
    - service.health == healthy
    - error_rate < service.slo.error_threshold
    - recovery_verified == true
```

---

## 15. Release Profile Example

```text
QUALIFY
   ↓
STAGE
   ↓
CANARY
   ↓
OBSERVE
   ↓
PROMOTE
   ↓
VERIFY
   ↓
COMPLETE
```

Example principles:

```text
progressive-delivery
reversible-changes
automated-verification
blast-radius-limitation
contract-testing
provenance-tracking
```

A failed observation predicate may result in:

```yaml
on_failure:
  action: rollback

rollback:
  require:
    - deployment.previous_revision.exists
```

---

## 16. Structural vs Semantic Validation

JSON Schema validates syntax and structure.

Examples:

- required fields exist;
- enum values are valid;
- identifiers match expected formats;
- collections contain appropriate value types.

Rust performs deeper semantic validation.

Examples:

- every non-terminal state has a valid outgoing transition;
- referenced principles exist;
- referenced states exist;
- required evidence has an available verifier;
- rollback cannot be required for an irreversible action;
- a task cannot require a capability explicitly denied by policy;
- production mutation cannot occur without the required approval policy;
- completion predicates reference observable values;
- workflow graphs do not contain unreachable states unless explicitly permitted.

The target API is:

```rust
let raw: RawProtocol = serde_yaml::from_str(input)?;

let protocol = Protocol::try_from(raw)?;

// `protocol` is semantically valid from this point onward.
engine.execute(protocol)?;
```

---

## 17. Schema Generation

Rust types should derive serialization and schema information where practical.

For example:

```rust
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub struct RawPrinciple {
    // ...
}
```

An `xtask` command could regenerate all public schemas:

```text
cargo xtask schema
```

CI verifies that generated schemas match the committed output.

```text
Rust types
   ↓
cargo xtask schema
   ↓
schemas/generated/*.schema.json
```

Generated schemas form the public interoperability contract.

---

## 18. Versioning

Protocols, principles, profiles, and schemas need explicit versioning.

Possible references:

```text
aep/1
adp/1
principle:test-driven/1
workflow:incident-standard/2
```

Breaking semantic changes require a new major version.

The protocol engine should reject unknown major versions rather than silently interpreting them.

Profiles should pin compatible principle versions to avoid changes in methodology occurring implicitly.

---

## 19. Explainability

Every protocol decision should be explainable.

Given a blocked action:

```text
production.write denied
```

the engine should be able to return:

```yaml
decision:
  allowed: false

reason:
  principle: least-privilege
  rule: production-write-requires-approval

missing:
  evidence:
    - human-approval

current_state:
  incident.diagnose
```

Likewise, incomplete tasks should expose the exact missing predicates.

```text
Task incomplete:

✓ unit tests
✓ contract tests
✓ static analysis
✗ required property test `session_isolation`
✗ approval `security-review`
```

This is important both for agent feedback and human auditing.

---

## 20. Audit and Provenance

Every meaningful execution event should be representable as an append-only record.

Possible events include:

```text
TaskCreated
ProtocolResolved
StateEntered
ActionRequested
ActionAllowed
ActionDenied
ActionExecuted
EvidenceProduced
VerificationPassed
VerificationFailed
ApprovalRequested
ApprovalGranted
TransitionPerformed
TaskCompleted
```

This event stream can later support:

- debugging;
- audits;
- incident review;
- reproducibility;
- policy analysis;
- agent evaluation.

AEP should define the semantics of these events without necessarily defining their persistence backend.

---

## 21. Initial Scope

Version 0 should remain intentionally small.

### Core

Implement:

- task;
- protocol;
- principle;
- workflow;
- state;
- transition;
- predicate;
- capability;
- evidence;
- verifier;
- approval;
- completion condition.

### First principles

Implement a small representative set:

```text
spec-driven
test-driven
contract-testing
static-analysis
least-privilege
reversible-changes
provenance-tracking
approval-gates
```

### First profiles

Implement:

```text
development.standard
incident.standard
release.progressive
```

### Reference tooling

Provide:

```text
protocol validate
protocol resolve
protocol explain
protocol schema
```

The first release does not need to implement an agent.

Its purpose is to make it possible for agent harnesses to consume the protocol consistently.

---

## 22. Non-Goals

The project should initially avoid becoming:

- an LLM orchestration framework;
- a CI system;
- a deployment platform;
- an incident management product;
- a shell sandbox;
- a policy language intended to replace general-purpose systems such as OPA;
- a universal ontology for all software engineering.

The responsibility of `engineering-protocols` is narrower:

> Define the semantics by which engineering work can be constrained, evidenced, verified, and progressed.

External systems perform the actual work.

---

## 23. Core Design Thesis

The fundamental abstraction is:

```text
              ┌─────────────┐
              │    TASK     │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │  PROTOCOL   │
              └──────┬──────┘
                     │
         ┌───────────┼───────────┐
         │           │           │
   PRINCIPLES     WORKFLOW   CAPABILITIES
         │           │           │
         └───────────┼───────────┘
                     │
              ┌──────▼──────┐
              │    AGENT    │
              └──────┬──────┘
                     │
                   ACTION
                     │
              ┌──────▼──────┐
              │  EVIDENCE   │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │  VERIFIERS  │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ TRANSITION  │
              └─────────────┘
```

The model reasons.

The harness executes.

The environment observes.

The verifiers establish facts.

The protocol decides what those facts permit.

---

## 24. Desired End State

A harness should eventually be able to receive:

```yaml
task:
  id: AUTH-142
  type: feature
  objective: add-passkey-support

protocol:
  version: aep/1
  profile: development.standard

principles:
  add:
    - clean-room
    - differential-testing
```

and derive, without relying on prompt interpretation:

- the workflow to execute;
- the requirements of the current state;
- which tools the agent may access;
- which actions require approval;
- what evidence must be produced;
- which verifiers must run;
- how failures are represented;
- when retries are permitted;
- which transitions are valid;
- exactly what constitutes completion.

This is the central goal of `engineering-protocols`:

> **A strongly typed, portable and machine-executable specification for how autonomous engineering work is performed and proven correct.**