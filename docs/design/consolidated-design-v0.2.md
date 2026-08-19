# Engineering Protocols — Consolidated Design Specification v0.2

> **Repository:** `engineering-protocols`  
> **Status:** First consolidated design draft  
> **Relationship to earlier drafts:** This document consolidates and supersedes the *Design Draft v0.1*, *Artifact Model Extension v0.1*, and *Entity Interaction Contract v0.1* as a single standalone design. It incorporates all of their concepts and adds complete audit/causality semantics plus a backend-independent conformance test model.  
> **Primary implementation language:** Rust  
> **Primary design goal:** A strongly typed, portable, machine-executable specification for how autonomous engineering work is represented, constrained, executed, verified, audited, and integrated.

---

# 1. Summary

`engineering-protocols` defines a machine-executable methodology and interaction contract for agentic engineering work.

The project is intended to move engineering rules such as:

- spec-driven development;
- test-driven development;
- contract testing;
- clean-room implementation;
- property-based testing;
- differential testing;
- mutation testing;
- fuzz testing;
- static analysis;
- design by contract;
- invariant checking;
- least privilege;
- approval gates;
- reversible changes;
- progressive delivery;
- incident response;
- release verification;
- provenance tracking;

out of natural-language prompts and into:

- strongly typed domain models;
- executable protocol definitions;
- explicit state machines;
- typed commands;
- read-only queries;
- artifact relationships;
- machine-readable evidence;
- independent verifiers;
- capability policies;
- audit records;
- deterministic transition rules;
- reusable conformance tests.

The language model may remain probabilistic.

The governing protocol and interaction semantics should be deterministic.

The central system model is:

```text
INTENT
  │
  ▼
ARTIFACT GRAPH
  │
  ├──────── TASKS
  ├──────── PRINCIPLES
  ├──────── WORKFLOWS
  └──────── PROFILES
             │
             ▼
      PROTOCOL ENGINE
             │
      requirements +
       capabilities
             │
             ▼
           AGENT
             │
          COMMANDS
             │
             ▼
       DOMAIN SYSTEM
             │
      ┌──────┴──────┐
      │             │
   ENTITIES       EVENTS
      │             │
      └──────┬──────┘
             │
          EVIDENCE
             │
             ▼
         VERIFIERS
             │
             ▼
        TRANSITIONS
             │
             ▼
       UPDATED GRAPH

Every step:
AUDITED + CORRELATED + ATTRIBUTED
```

---

# 2. Motivation

Agentic engineering systems are often governed by instructions such as:

> Follow TDD, do not break the API, verify your work, ask before deploying, and document architectural decisions.

These instructions are useful as human guidance but weak as executable policy.

They leave critical questions unresolved:

- What exactly constitutes following TDD?
- What evidence proves that a test failed before implementation?
- Which artifacts must exist before implementation?
- What counts as an architecture-changing decision?
- Which commands can the agent perform?
- Which operations require approval?
- How is an approval tied to a specific revision?
- What does “verify your work” mean?
- When is a task actually complete?
- What happens when verification fails?
- How is a chain of agent actions reconstructed later?
- Who initiated a change?
- Which agent or service executed it?
- Which previous command, event, or decision caused it?
- Which later commands did it trigger?
- How can a new backend implementation prove that it obeys the same semantics as another implementation?

`engineering-protocols` attempts to answer these questions in a portable way.

The intended result is not another agent framework.

It is a **domain model, protocol model, interaction contract, and conformance specification** that agent harnesses and storage implementations can share.

---

# 3. Repository Name

The canonical repository name is:

```text
github.com/<org>/engineering-protocols
```

The repository contains the semantics of engineering work, not project-specific engineering artifacts.

Project-specific documents such as actual designs, ADRs, stories, or runbooks remain in their owning systems or repositories.

---

# 4. Terminology

## 4.1 AEP — Agentic Engineering Protocol

AEP is the common protocol model for controlled agentic engineering work.

AEP defines shared concepts such as:

- entities;
- artifacts;
- tasks;
- principles;
- profiles;
- workflows;
- states;
- transitions;
- commands;
- queries;
- relations;
- capabilities;
- evidence;
- verifiers;
- approvals;
- events;
- audit records;
- completion predicates.

---

## 4.2 ADP — Agentic Development Protocol

ADP is an AEP profile for software development.

Typical concerns include:

- specification;
- decomposition;
- design;
- implementation;
- TDD;
- contract testing;
- static analysis;
- property testing;
- regression verification;
- architectural decisions;
- review;
- merge readiness.

Typical development workflow:

```text
RECEIVE
   ↓
SPECIFY
   ↓
DECOMPOSE
   ↓
DESIGN
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

---

## 4.3 AOP — Agentic Operations Protocol

AOP is an AEP profile for operational and SRE work.

Typical concerns include:

- desired state;
- telemetry;
- diagnosis;
- change execution;
- blast-radius control;
- production permissions;
- reversible changes;
- health validation;
- rollback;
- auditability.

A generic operational workflow may be:

```text
OBSERVE
   ↓
ASSESS
   ↓
PLAN
   ↓
CHANGE
   ↓
VERIFY
   ↓
RECORD
```

---

## 4.4 Incident Profile

Incident response is an AEP/AOP profile.

```text
DETECT
   ↓
TRIAGE
   ↓
FORM_HYPOTHESIS
   ↓
TEST_HYPOTHESIS
   ↓
MITIGATE
   ↓
RECOVER
   ↓
VERIFY
   ↓
LEARN
```

Common principles include:

```text
preserve-evidence
least-privilege
hypothesis-driven-diagnosis
reversible-changes
blast-radius-limitation
verify-after-action
provenance-tracking
```

---

## 4.5 Release Profile

Release management is another AEP/AOP profile.

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

Common principles include:

```text
progressive-delivery
automated-verification
reversible-changes
blast-radius-limitation
contract-testing
provenance-tracking
```

---

## 4.6 Additional Profiles

The same protocol model may later support:

- migrations;
- security response;
- dependency upgrades;
- infrastructure changes;
- compliance activities;
- disaster recovery;
- capacity management.

These are profiles over common primitives rather than independent orchestration systems.

---

# 5. Core Design Principles

## 5.1 Rust is the source of truth

Protocol concepts are modeled first as Rust types.

Generated interchange schemas are outputs.

```text
Rust domain model
      ↓
Generated JSON Schema
      ↓
YAML / JSON documents
      ↓
Parser
      ↓
Raw domain objects
      ↓
Semantic validation
      ↓
Validated domain objects
      ↓
Execution
```

Schemas should not be maintained independently from the authoritative Rust types unless there is a specific compatibility reason.

---

## 5.2 Invalid states should be difficult to represent

Parsing and validation are separate.

```rust
RawWorkflow
    ↓ TryFrom
Workflow
```

Runtime code should operate on validated types.

Example:

```rust
let raw: RawWorkflow = serde_yaml::from_str(input)?;
let workflow = Workflow::try_from(raw)?;

engine.run(workflow)?;
```

Once `Workflow` exists, its constructor should guarantee core invariants.

---

## 5.3 Generic interaction, typed semantics

The integration surface should remain small even as the engineering domain grows.

Do not create a permanent low-level API consisting of:

```text
story_add
story_update
epic_add
design_add
adr_accept
incident_resolve
release_promote
...
```

Instead expose a generic command/query contract whose semantics are defined by types.

Typed SDKs may provide:

```text
board.story.add(...)
architecture.adr.accept(...)
incident.resolve(...)
release.promote(...)
```

but these are convenience layers.

---

## 5.4 Every state-changing operation is a command

This is a central rule.

Creation, updates, relation changes, approvals, state transitions, releases, and other mutations are all represented canonically as commands.

For example:

```text
entity.create(...)
```

is SDK sugar over:

```text
command.execute(CreateEntity)
```

Likewise:

```text
relation.create(...)
```

becomes:

```text
command.execute(CreateRelation)
```

and:

```text
design.approve(...)
```

becomes:

```text
command.execute(ApproveDesign)
```

This creates one consistent mutation boundary for:

- validation;
- authorization;
- protocol enforcement;
- idempotency;
- optimistic concurrency;
- provenance;
- events;
- audit;
- correlation and causation.

---

## 5.5 Queries never mutate

The query side is read-only.

It supports:

- get;
- resolve;
- search;
- graph traversal;
- history;
- audit;
- type discovery;
- capability inspection.

A backend may internally use CQRS, event sourcing, relational tables, files, APIs, or something else.

The contract does not require a specific storage architecture.

---

## 5.6 Generation and verification are separate

Agents may generate:

- plans;
- specifications;
- tests;
- designs;
- code;
- hypotheses;
- fixes;
- candidate invariants.

Agents should not be the sole authority that those outputs are correct.

Prefer independent verifiers:

```text
code                       → compiler
types                      → type checker
tests                      → test runner
API compatibility          → contract verifier
properties                 → property tester
behavior preservation      → differential tester
release health             → telemetry
production mutation        → policy engine
review requirement         → review evidence
completion                 → protocol engine
```

---

## 5.7 Evidence drives transitions

The workflow does not advance because the agent says it is finished.

The workflow advances because required evidence satisfies explicit transition predicates.

---

## 5.8 Capabilities are explicit

Agent permissions are modeled as semantic capabilities.

Examples:

```text
repository.read
repository.write
tests.execute
network.read
network.write
telemetry.read
production.read
production.write
deployment.create
deployment.rollback
secret.read
```

The harness maps these semantic capabilities onto actual tools and credentials.

---

## 5.9 Everything important is addressable

Every significant engineering object should have:

- stable identity;
- type;
- logical location;
- provenance;
- history or revision semantics where applicable.

This includes not only stories and designs but also:

- protocols;
- principles;
- workflows;
- profiles;
- evidence;
- reviews;
- approvals;
- incidents;
- releases;
- runbooks.

---

## 5.10 Everything important is auditable

Every mutation attempt, protocol decision, approval, transition, verifier result, and important automation trigger should be reconstructable later.

Auditability is not a backend-specific optional feature.

It is part of the logical contract.

---

# 6. Conceptual Layers

Several concerns must remain distinct.

## 6.1 Planning and decomposition

This describes how intent becomes work.

Example:

```text
Vision
  ↓
PRD
  ↓
Epic
  ↓
Story
  ↓
Task
```

---

## 6.2 Engineering artifacts

These are durable engineering knowledge objects.

Examples:

```text
Specification
Design
ArchitectureDesign
ADR
TestPlan
ReviewResult
ReleasePlan
Runbook
Postmortem
```

---

## 6.3 Protocol execution

This defines:

```text
current state
requirements
allowed commands
required evidence
verification
transition rules
completion
```

---

## 6.4 Physical storage

This defines where information happens to live.

Examples:

```text
Git
filesystem
Postgres
GitHub
Linear
Jira
Notion
Confluence
incident platform
remote service
```

Physical storage is explicitly outside the protocol semantics.

---

# 7. Planning and Intent Model

A useful default hierarchy is:

```text
                    WHY
                     │
                  VISION
                     │
                     ▼
                    PRD
               what / for whom
                     │
            ┌────────┴────────┐
            ▼                 ▼
          EPIC              EPIC
            │
       ┌────┴────┐
       ▼         ▼
     STORY     STORY
       │
       ▼
      TASK
```

| Artifact | Primary question |
|---|---|
| Vision | Where are we going and why? |
| PRD | What outcome or product capability should exist? |
| Epic | What major deliverable contributes to that outcome? |
| Story | What independently meaningful behavior/change is required? |
| Task | What concrete work needs to be performed? |

AEP should not require these exact names.

Organizations may use:

```text
Objective → Initiative → Work Item
```

or:

```text
Initiative → Epic → Story
```

The protocol should model decomposition relationships generically.

---

# 8. Engineering Specification

An engineering specification answers:

> Precisely what behavior and constraints must the implementation satisfy?

A PRD may say:

```text
Users can authenticate with passkeys.
```

An engineering specification may state:

```text
- Passkey credentials belong to exactly one user.
- Failed authentication never creates a session.
- Existing password authentication remains supported.
- Existing clients remain API-compatible.
```

Specifications sit between product intent and solution design.

A protocol may require an approved specification before implementation.

---

# 9. Design

A design answers:

> How do we intend to satisfy the specification?

Typical contents include:

- context;
- goals;
- non-goals;
- current state;
- proposed solution;
- affected components;
- interfaces;
- data model;
- invariants;
- failure modes;
- security;
- observability;
- migration;
- rollout;
- rollback;
- open questions.

A design is not a story or task.

A design may span:

- one story;
- many stories;
- an epic;
- a service;
- a migration;
- several repositories.

---

# 10. Architecture Design

Architecture design is a specialized design category with broader scope and governance.

```text
Design
├── FeatureDesign
├── ComponentDesign
├── ApiDesign
├── DataDesign
└── ArchitectureDesign
```

Architecture design commonly describes:

- responsibility boundaries;
- service relationships;
- data ownership;
- communication patterns;
- persistence boundaries;
- security boundaries;
- cross-system invariants;
- current and target state.

Architecture design is not a separate universe from design.

It is a design class whose scope often causes stricter protocol requirements.

---

# 11. Architecture Decision Records

An ADR answers:

> What important decision was made, why was it made, and what are its consequences?

A design describes the proposed solution as a whole.

An ADR records a specific durable decision.

Example:

```text
ADR-0042

Decision:
Store passkey credentials in the identity service.

Alternatives:
- dedicated credential service
- application-local storage

Consequences:
- identity service owns credentials
- authentication remains centralized
- migration preserves identity IDs
```

One design may produce:

```text
0..N ADRs
```

ADRs should generally be historical records.

Later decisions supersede earlier ADRs rather than rewriting historical rationale.

---

# 12. Review Semantics

Review is primarily an activity and protocol event, not merely a Markdown document.

```text
Design
   ↓
Design Review
   ↓
Review Result
   ├── approved
   ├── changes_requested
   └── rejected
```

A review result is evidence.

It should identify the exact revision reviewed.

An approval for design revision 3 must not silently approve revision 7.

Example:

```rust
pub struct ReviewResult {
    pub subject: VersionedEntityRef,
    pub reviewer: ActorRef,
    pub disposition: ReviewDisposition,
    pub findings: Vec<Finding>,
}
```

---

# 13. Universal Entity Model

The fundamental addressable primitive is:

```text
Entity
```

An entity is a typed node in the engineering graph.

Examples:

```text
story:AUTH-142
design:passkeys
adr:0042
principle:test-driven
workflow:development-standard
incident:INC-312
review:9812
evidence:test-run-551
```

The universal graph is fundamentally:

```text
ENTITY ── RELATION ──▶ ENTITY
```

with commands producing valid changes to entities and relations.

---

# 14. Entity Identity

Every entity has a stable unique canonical identity.

```rust
pub struct EntityId(String);
```

The representation should be opaque to consumers.

Implementations may use:

```text
UUIDv7
ULID
generated opaque IDs
organization-specific opaque identifiers
```

Human-friendly identifiers such as:

```text
AUTH-142
ADR-0042
INC-312
```

are aliases, keys, or locators.

They are not the canonical identity.

---

# 15. Entity Locator

Every entity should be logically locatable.

Example:

```text
ep://acme/payments/story/AUTH-142
```

or:

```text
ep://acme/engineering-protocols/principle/test-driven
```

The locator is not a physical storage URL.

It means:

> Resolve this logical engineering entity.

The backend may resolve it from any supported source.

---

# 16. Entity Type

Every entity has a versioned type.

```rust
pub struct EntityType {
    pub namespace: TypeNamespace,
    pub name: TypeName,
    pub version: TypeVersion,
}
```

Examples:

```text
aep.story/v1
aep.specification/v1
aep.design/v1
aep.review-result/v1
adp.test-plan/v1
aop.incident/v1
```

The type determines:

- schema;
- semantic validation;
- legal commands;
- lifecycle;
- allowed relations;
- mutability;
- protocol semantics.

---

# 17. Entity Envelope

All entities use a common envelope.

```rust
pub struct Entity<T> {
    pub metadata: EntityMetadata,
    pub data: T,
}
```

Example metadata:

```rust
pub struct EntityMetadata {
    pub id: EntityId,
    pub locator: EntityLocator,
    pub entity_type: EntityType,

    pub revision: Revision,

    pub created_at: Timestamp,
    pub updated_at: Timestamp,

    pub provenance: Provenance,
}
```

Wire example:

```yaml
type: aep.story/v1

metadata:
  id: 01K2R8JD3ZJME72AJGQY67E5F8
  locator: ep://acme/payments/story/AUTH-142
  revision: 4

data:
  title: Add passkey authentication
  status: ready
```

---

# 18. Entity References

Relations and commands use references.

```rust
pub struct EntityRef {
    pub id: EntityId,
}
```

Revision-sensitive operations use:

```rust
pub struct VersionedEntityRef {
    pub id: EntityId,
    pub revision: Revision,
}
```

The distinction is important.

```text
EntityRef
```

means:

> current logical entity X

while:

```text
VersionedEntityRef
```

means:

> exactly revision N of entity X

Reviews, approvals, and historical evidence often require versioned references.

---

# 19. Artifact Taxonomy

Artifacts are entities with engineering-document semantics.

Possible initial taxonomy:

```rust
pub enum ArtifactKind {
    // Intent and planning
    Vision,
    ProductRequirements,
    Initiative,
    Epic,
    Story,
    Task,

    // Engineering intent
    Specification,
    AcceptanceCriteria,

    // Solution design
    Design,
    FeatureDesign,
    ComponentDesign,
    ArchitectureDesign,
    ApiDesign,
    DataDesign,

    // Durable decisions
    ArchitectureDecisionRecord,

    // Verification
    TestPlan,
    EvaluationPlan,
    VerificationReport,

    // Governance
    ReviewResult,
    ApprovalRecord,

    // Delivery and operations
    ReleasePlan,
    MigrationPlan,
    Runbook,
    IncidentReport,
    Postmortem,
}
```

The taxonomy should remain extensible.

---

# 20. Artifact Locations

An artifact does not have to live in Git.

```rust
pub enum ArtifactLocation {
    RepositoryPath {
        repository: RepositoryRef,
        path: PathBuf,
    },

    Url(Url),

    External {
        provider: ProviderId,
        reference: String,
    },

    Inline,
}
```

Examples include:

```text
Git repository
GitHub
Linear
Jira
Notion
Confluence
internal planning system
incident platform
```

---

# 21. Relations

Relations are first-class graph edges.

```rust
pub struct Relation {
    pub id: RelationId,
    pub relation_type: RelationType,
    pub source: EntityRef,
    pub target: EntityRef,
    pub metadata: RelationMetadata,
}
```

Useful relations include:

```text
InformedBy
DerivedFrom
Decomposes
Specifies
Designs
Implements
Decides
Reviews
Verifies
Blocks
DependsOn
Supersedes
Delivers
```

Example:

```text
VISION-1
  └─ informs → PRD-12

PRD-12
  └─ decomposes → EPIC-44

EPIC-44
  ├─ decomposes → STORY-441
  └─ decomposes → STORY-442

SPEC-76
  └─ specifies → STORY-441

DESIGN-76
  └─ designs → SPEC-76

ADR-32
  └─ decides → DESIGN-76

REVIEW-91
  └─ reviews → DESIGN-76

CHANGE-151
  └─ implements → DESIGN-76

TEST-992
  └─ verifies → CHANGE-151
```

---

# 22. Artifact Status

Artifacts can have lifecycle state independent of the workflow.

Example:

```rust
pub enum ArtifactStatus {
    Draft,
    Proposed,
    InReview,
    Approved,
    Rejected,
    Active,
    Implemented,
    Superseded,
    Archived,
}
```

Not all statuses apply to all types.

Validated domain types should enforce legal transitions.

Example:

```text
ADR:
Proposed → Accepted → Superseded
```

and:

```text
Design:
Draft → InReview → Approved → Implemented
```

---

# 23. Principles

A principle describes an enforceable engineering rule.

```rust
pub struct Principle {
    pub id: PrincipleId,
    pub version: Version,

    pub applicability: Predicate,

    pub obligations: Vec<Obligation>,
    pub required_artifacts: Vec<ArtifactRequirement>,
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
eval-driven
property-based-testing
differential-testing
mutation-testing
behavior-driven
metamorphic-testing
model-based-testing
executable-specifications
formal-verification
hermetic-execution
design-by-contract
golden-testing
fuzz-testing
cegis
static-analysis
type-driven
refinement-types
automated-repair
sandboxed-execution
conformance-testing
invariant-checking
symbolic-execution
proof-carrying-code
specification-mining
context-engineering
task-decomposition
approval-gates
provenance-tracking
least-privilege
reversible-changes
preserve-evidence
blast-radius-limitation
progressive-delivery
verify-after-action
hypothesis-driven-diagnosis
```

---

# 24. Principle Example: TDD

Natural-language guidance:

```text
Follow TDD.
```

becomes an executable definition:

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
  - implementation_change
```

The agent does not get to self-certify these conditions.

The protocol engine evaluates evidence.

---

# 25. Principle Profiles

Tasks normally select profiles rather than enumerate every principle.

Example:

```yaml
protocol: aep/1
profile: development.standard
```

Profiles may be overridden:

```yaml
principles:
  add:
    - clean-room
    - differential-testing

  remove:
    - mutation-testing
```

Possible bundles:

```yaml
profiles:

  development.fast:
    - spec-driven
    - test-driven
    - static-analysis

  development.standard:
    - spec-driven
    - test-driven
    - contract-testing
    - property-based-testing
    - static-analysis
    - sandboxed-execution
    - provenance-tracking
    - least-privilege

  development.critical:
    - spec-driven
    - executable-specifications
    - contract-testing
    - property-based-testing
    - mutation-testing
    - fuzz-testing
    - invariant-checking
    - differential-testing
    - static-analysis
    - hermetic-execution
    - provenance-tracking
    - approval-gates
```

---

# 26. Four Principle Layers

The collected principles naturally form four major classes.

| Layer | Question | Examples |
|---|---|---|
| Intent | What should exist? | spec-driven, BDD, design-by-contract, executable specs |
| Construction | How should it be produced? | TDD, type-driven, clean-room, decomposition |
| Verification | How do we know it works? | property, differential, mutation, fuzz, symbolic, contract |
| Governance | Under what controls may it operate? | sandboxing, least privilege, provenance, approvals, hermetic execution |

Profiles can ensure that appropriate classes are represented for a task.

---

# 27. Workflow Model

A workflow is a state machine.

```rust
pub struct Workflow {
    pub id: WorkflowId,
    pub initial: StateId,
    pub states: BTreeMap<StateId, State>,
    pub transitions: Vec<Transition>,
}
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

Only the protocol engine decides whether a transition is valid.

---

# 28. Artifact Requirements in Workflows

Workflows can require artifacts.

Example:

```yaml
state: specify

requires:
  artifacts:
    - kind: specification
      status: approved

transition:
  to: design
```

Implementation may require:

```yaml
state: implement

requires:
  artifacts:
    - kind: design
      status: approved
```

Conditional requirements:

```yaml
conditional_requirements:

  - when:
      change.architectural: true

    require:
      artifacts:
        - kind: architecture-design
          status: approved

        - kind: architecture-decision-record

  - when:
      risk:
        gte: medium

    require:
      reviews:
        - subject: design
          result: approved
```

This turns engineering governance into executable policy.

---

# 29. Capabilities

Capabilities describe semantic permissions.

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

    SecretRead,
}
```

Policy:

```rust
pub struct CapabilityPolicy {
    pub allow: BTreeSet<Capability>,
    pub deny: BTreeSet<Capability>,
    pub approval_required: BTreeSet<Capability>,
}
```

Example:

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

---

# 30. Evidence

Evidence represents observable facts.

Evidence itself can be modeled as an entity.

Examples:

```text
TestResult
StaticAnalysisResult
ContractResult
PropertyTestResult
DeploymentResult
MetricObservation
ReviewResult
ApprovalRecord
ChangeSet
ArtifactObservation
```

Example Rust shape:

```rust
pub enum Evidence {
    TestResult(TestResult),
    StaticAnalysis(StaticAnalysisResult),
    ContractResult(ContractResult),
    PropertyTestResult(PropertyTestResult),
    DeploymentResult(DeploymentResult),
    MetricObservation(MetricObservation),
    Approval(ApprovalRecord),
    Review(ReviewResult),
    Diff(ChangeSet),
    Artifact(ArtifactObservation),
}
```

Evidence should carry provenance.

---

# 31. Verification

Verification evaluates claims and evidence.

Possible verifier classes:

```rust
pub enum Verifier {
    Compiler,
    TypeChecker,
    TestRunner,
    ContractRunner,
    StaticAnalyzer,
    PropertyTester,
    MutationTester,
    Fuzzer,
    DifferentialTester,
    ModelChecker,
    TelemetryQuery,
    PolicyEngine,
    HumanApproval,
    ExternalTool(ToolRef),
}
```

Verification produces structured results.

```rust
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub evidence: Vec<EntityRef>,
    pub counterexamples: Vec<Counterexample>,
}
```

---

# 32. Counterexample-Guided Iteration

Failures should produce structured counterexamples when possible.

Example:

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

  attempt: 2
```

The generic loop is:

```text
candidate
   ↓
verifier
   ↓
counterexample
   ↓
revised candidate
   ↓
verifier
   ↓
...
```

This connects TDD, automated repair, eval-driven development, property testing, and CEGIS under a common feedback model.

---

# 33. Completion Predicate

Completion is formally evaluated.

Example:

```yaml
done_when:

  all:
    - specification.status == satisfied
    - tests.unit.failed == 0
    - tests.contract.failed == 0
    - tests.properties.failed == 0
    - static_analysis.errors == 0
    - required_evidence.missing == 0
    - required_reviews.pending == 0
    - approvals.pending == 0
```

The agent cannot override the completion predicate.

---

# 34. Interaction Architecture

The interaction layer is split into:

```text
COMMAND SIDE
    state changes

QUERY SIDE
    reads and inspection
```

This resembles CQRS conceptually but does **not** require a CQRS storage architecture.

A backend may be implemented with:

- a relational database;
- event sourcing;
- Git;
- filesystem documents;
- remote APIs;
- a composite of multiple systems.

The contract defines observable behavior only.

---

# 35. Command Side

All state-changing operations are commands.

Canonical interface:

```rust
pub trait CommandService {
    async fn execute(
        &self,
        command: CommandEnvelope,
    ) -> Result<CommandResult, CommandError>;
}
```

Examples of generic commands:

```text
CreateEntity
UpdateEntity
CreateRelation
RemoveRelation
ArchiveEntity
SupersedeEntity
```

Examples of domain commands:

```text
StartStory
CompleteStory

SubmitDesignReview
ApproveDesign
SupersedeDesign

AcceptAdr
SupersedeAdr

AcknowledgeIncident
MitigateIncident
ResolveIncident

PromoteRelease
RollbackRelease
```

---

# 36. Command Envelope

Every command has a common envelope.

```rust
pub struct CommandEnvelope<C> {
    pub command_id: CommandId,
    pub command_type: CommandType,

    pub target: Option<EntityRef>,
    pub expected_revision: Option<Revision>,

    pub payload: C,

    pub context: CommandContext,
}
```

The command type is versioned.

Example:

```text
aep.entity.create/v1
aep.design.approve/v1
aop.incident.resolve/v1
```

---

# 37. Command Context

The context provides complete attribution and causal tracing.

```rust
pub struct CommandContext {
    pub request_id: RequestId,
    pub idempotency_key: IdempotencyKey,

    pub actor: ActorRef,
    pub executor: Option<ActorRef>,

    pub correlation_id: CorrelationId,
    pub causation: Option<CausationRef>,

    pub trace: Option<TraceContext>,

    pub execution_id: Option<ExecutionId>,
    pub task: Option<EntityRef>,
    pub protocol: Option<EntityRef>,

    pub issued_at: Timestamp,
}
```

The fields have distinct meanings.

### `request_id`

Identifies one transport/API attempt.

Retries receive new request IDs.

### `command_id`

Identifies one logical command.

A retry of the same logical command should reuse the same command ID.

### `idempotency_key`

Allows a client to safely retry the same intended mutation.

### `actor`

The principal on whose behalf the action occurs.

Examples:

```text
human:alice
agent:planning-agent
service:release-controller
```

### `executor`

The runtime actually performing the action.

For example:

```text
actor: human:alice
executor: agent:release-agent-17
```

This allows the audit trail to answer both:

> Who authorized/initiated this?

and:

> Which agent or service executed it?

### `correlation_id`

Stable across one logical chain of work.

For example:

```text
user request
  ↓
design command
  ↓
review event
  ↓
protocol transition
  ↓
implementation command
```

All may share one correlation ID.

### `causation`

Identifies the immediate cause.

A caused command can point to:

- a command;
- an event;
- a protocol decision;
- a verifier result;
- an approval.

### `trace`

Optional observability trace/span information.

### `execution_id`

Identifies the AEP protocol execution when applicable.

---

# 38. Correlation and Causation

Correlation and causation have different purposes.

```text
Correlation:
"What belongs to the same overall activity?"

Causation:
"What directly caused this specific action?"
```

Example:

```text
USER REQUEST
  correlation=C42
        │
        ▼
COMMAND A
  command=CA
  correlation=C42
        │
        ▼
EVENT A1
  correlation=C42
  causation=CA
        │
        ▼
PROTOCOL DECISION D1
  correlation=C42
  causation=A1
        │
        ▼
COMMAND B
  command=CB
  correlation=C42
  causation=D1
        │
        ▼
EVENT B1
  correlation=C42
  causation=CB
```

This allows the entire chain to be reconstructed while preserving immediate causal links.

---

# 39. Command Result

A successful command returns structured results.

```rust
pub struct CommandResult {
    pub command_id: CommandId,
    pub outcome: CommandOutcome,

    pub affected_entities: Vec<EntityRevisionRef>,
    pub emitted_events: Vec<EventRef>,

    pub audit_records: Vec<AuditRef>,

    pub consistency: ConsistencyToken,
}
```

A rejected command also produces an auditable result where security policy permits exposure.

---

# 40. Idempotency

Mutating operations must support idempotency.

Example:

```text
create story
request times out
client retries same logical command
```

The retry must not create a second story.

The contract distinguishes:

```text
request_id       each invocation
command_id       logical command
idempotency_key  retry identity
```

A backend should return the original logical result for an accepted idempotent replay.

---

# 41. Optimistic Concurrency

Revision-aware commands prevent silent overwrite.

Example:

```text
current revision: 7

command expected_revision=7
→ succeeds
→ revision 8
```

while:

```text
current revision: 8

command expected_revision=7
→ revision_conflict
```

The error must be machine-readable.

---

# 42. Semantic Commands vs Generic Patch

A generic:

```text
PATCH status = "approved"
```

is insufficient for semantic state transitions.

Instead:

```text
ApproveDesign {
    review: review_ref
}
```

allows validation of:

```text
review exists
review targets current design revision
review disposition = approved
required ADR exists
actor has required capability
workflow permits approval
```

A structural update command may exist for ordinary mutable fields.

Semantic lifecycle transitions should use domain commands.

---

# 43. No Universal Physical Delete

The logical contract should not make physical deletion a normal operation.

Engineering objects often require durable history.

Prefer semantic commands:

```text
Story        → ArchiveStory
Design       → SupersedeDesign
ADR          → SupersedeAdr
Incident     → CloseIncident
Principle    → DeprecatePrinciple
```

Evidence and review records may be immutable.

Physical storage garbage collection is an implementation concern.

---

# 44. Query Side

The query contract is read-only.

Possible interface:

```rust
pub trait QueryService {
    async fn get(
        &self,
        reference: EntityRef,
        consistency: QueryConsistency,
    ) -> Result<EntityEnvelope, QueryError>;

    async fn resolve(
        &self,
        locator: EntityLocator,
    ) -> Result<EntityRef, QueryError>;

    async fn query(
        &self,
        query: EntityQuery,
    ) -> Result<Page<EntityEnvelope>, QueryError>;

    async fn relations(
        &self,
        query: RelationQuery,
    ) -> Result<Page<Relation>, QueryError>;

    async fn history(
        &self,
        reference: EntityRef,
    ) -> Result<Vec<EntityRevision>, QueryError>;

    async fn audit(
        &self,
        query: AuditQuery,
    ) -> Result<Page<AuditRecord>, QueryError>;

    async fn describe_type(
        &self,
        entity_type: EntityType,
    ) -> Result<TypeDescriptor, QueryError>;
}
```

---

# 45. Query Examples

The generic query model should support questions such as:

```text
all stories with status=ready

all approved designs for epic X

all ADRs related to design Y

all evidence verifying change Z

all unresolved incidents owned by checkout

all actions performed by agent A

everything correlated with correlation C42

everything caused by command CA

all changes to entity E between revisions 2 and 7
```

The backend decides how to implement the query.

---

# 46. Query Consistency

Backends may use immediate state, projections, distributed stores, or external APIs.

Conformance tests must not rely on arbitrary sleeps.

Therefore every accepted mutation returns an opaque:

```rust
pub struct ConsistencyToken(String);
```

Queries may request:

```rust
pub enum QueryConsistency {
    Current,
    AtLeast(ConsistencyToken),
}
```

Semantics:

> `AtLeast(token)` must not return a view older than the successful command that produced the token.

An immediately consistent backend can satisfy this trivially.

A projected/eventually consistent backend may block until the projection reaches the requested token.

This creates technology-independent **read-your-writes** semantics.

---

# 47. Type Discovery

A generic harness must be able to discover semantics.

```rust
pub struct TypeDescriptor {
    pub entity_type: EntityType,
    pub schema: SchemaRef,

    pub lifecycle: Option<LifecycleDescriptor>,

    pub commands: Vec<CommandDescriptor>,
    pub relations: Vec<RelationDescriptor>,

    pub mutable: bool,
}
```

A harness can ask:

```text
What is a Design?
Which fields are required?
Which commands can target it?
Which relations can it have?
Is it immutable?
Which lifecycle states exist?
```

without hard-coding every domain type.

---

# 48. Typed SDK Layer

The generic contract should support ergonomic typed wrappers.

Example:

```rust
let story: Entity<Story> =
    board.stories().add(
        Story::new("Add passkey authentication")
    ).await?;
```

may compile to:

```rust
command.execute(
    CreateEntity::<Story> { ... }
).await?;
```

Likewise:

```text
architecture.adr.accept(...)
```

maps to:

```text
command.execute(AcceptAdr)
```

and:

```text
incident.resolve(...)
```

maps to:

```text
command.execute(ResolveIncident)
```

The SDK can be domain-friendly while the underlying contract remains generic.

---

# 49. Events

Entities represent addressable domain objects.

Events represent immutable facts about what occurred.

Examples:

```text
StoryCreated
StoryStarted
DesignSubmittedForReview
DesignApproved
AdrAccepted
IncidentMitigated
ReleasePromoted
```

A backend does not have to use event sourcing internally.

However, command results may emit domain events, and those events must have logical semantics.

---

# 50. Event Envelope

```rust
pub struct EventEnvelope<E> {
    pub event_id: EventId,
    pub event_type: EventType,

    pub subject: Option<EntityRef>,
    pub entity_revision: Option<Revision>,

    pub payload: E,

    pub command_id: CommandId,

    pub correlation_id: CorrelationId,
    pub causation: CausationRef,

    pub execution_id: Option<ExecutionId>,

    pub occurred_at: Timestamp,
    pub provenance: Provenance,
}
```

An event caused by a command should reference the command as its direct cause.

---

# 51. Triggered Work

Commands and events may trigger further protocol activity.

Example:

```text
ApproveDesign command
        ↓
DesignApproved event
        ↓
protocol observes requirement satisfied
        ↓
workflow transition
        ↓
implementation state entered
        ↓
agent receives new requirements
```

The new protocol decision uses:

```text
same correlation_id
causation = DesignApproved event
```

Any command generated from that decision uses:

```text
same correlation_id
causation = protocol decision
```

This provides a full causal chain.

---

# 52. Audit Model

Audit is a first-class cross-cutting concern.

The system must support answering:

```text
Who did it?
Who/what executed it?
What changed?
When did it happen?
Why was it allowed?
Which task/protocol execution was active?
What directly caused it?
Which broader activity did it belong to?
What did it trigger?
Which entity revisions were involved?
Which approval or evidence justified it?
Was an attempted action rejected?
```

Audit records are immutable logical records.

---

# 53. Audit Record

A possible common shape:

```rust
pub struct AuditRecord {
    pub audit_id: AuditId,
    pub kind: AuditKind,

    pub occurred_at: Timestamp,

    pub actor: ActorRef,
    pub executor: Option<ActorRef>,

    pub subject: Option<EntityRef>,

    pub request_id: Option<RequestId>,
    pub command_id: Option<CommandId>,
    pub event_id: Option<EventId>,

    pub correlation_id: CorrelationId,
    pub causation: Option<CausationRef>,
    pub trace: Option<TraceContext>,

    pub execution_id: Option<ExecutionId>,
    pub task: Option<EntityRef>,
    pub protocol: Option<EntityRef>,

    pub decision: Option<DecisionRecord>,
    pub change: Option<ChangeRecord>,
    pub evidence: Vec<EntityRef>,
}
```

---

# 54. Audit Kinds

Useful logical audit records include:

```text
CommandAttempted
CommandAccepted
CommandRejected
CommandExecuted

EntityCreated
EntityChanged
EntityArchived
EntitySuperseded

RelationCreated
RelationRemoved

ProtocolResolved
StateEntered
TransitionEvaluated
TransitionPerformed
TransitionRejected

CapabilityEvaluated
ActionAllowed
ActionDenied

VerificationRequested
VerificationPassed
VerificationFailed

ApprovalRequested
ApprovalGranted
ApprovalRejected

EventEmitted
TaskCompleted
```

An implementation may collapse or expand physical records as long as the logical information is queryable.

---

# 55. Auditing Rejected Attempts

Rejected actions matter.

For example:

```text
agent attempted production.write
policy denied it
```

This should be observable in the audit trail.

A denied command must not mutate domain state, but the attempt and denial should be recorded.

This is especially important for:

- production access;
- approval gates;
- invalid lifecycle transitions;
- stale revisions;
- authorization failures;
- protocol violations.

Security-sensitive systems may redact portions of the request while preserving attribution and reason codes.

---

# 56. What Changed

The audit model must make the mutation reconstructable.

At minimum, a successful entity-changing command records:

```text
entity id
before revision
after revision
command type
command payload or redacted representation
actor
executor
timestamp
correlation
causation
```

The system must support recovering the exact historical revisions or an equivalent canonical change set.

A backend may store:

- full revisions;
- patches;
- events;
- snapshots;
- another internal representation.

The logical contract only requires that “what changed” be reconstructable.

---

# 57. Sensitive Audit Data

Auditability must not require leaking secrets.

Fields may support:

```rust
pub enum AuditedValue<T> {
    Value(T),
    Redacted {
        digest: Digest,
        reason: RedactionReason,
    },
}
```

This allows the system to preserve:

- the fact that a value existed;
- integrity identity;
- causal traceability;

without storing sensitive material in plain text.

---

# 58. Actor and Execution Identity

Do not conflate:

```text
initiator
executor
approver
```

Example:

```text
Actor:
  human:alice

Executor:
  agent:release-agent-17

Approval:
  human:bob
```

The audit model should preserve each role separately.

Agent identity may include:

```text
agent definition
agent instance
model/harness version
execution ID
```

where available.

---

# 59. Protocol Decisions Are Auditable

The protocol engine should produce explainable decision records.

Example:

```yaml
decision:

  allowed: false

  operation:
    production.write

  reason:
    principle: least-privilege
    rule: production-write-requires-approval

  missing:
    - approval: production-change

  current_state:
    incident.diagnose
```

This decision can be linked causally to:

```text
command attempt
   ↓
capability evaluation
   ↓
denial
```

---

# 60. Explainability

Every blocked transition or command should be machine-explainable.

Example:

```text
Task incomplete:

✓ unit tests
✓ contract tests
✓ static analysis
✗ property test `session_isolation`
✗ security review
```

The agent should not need to infer missing requirements from free-form prose.

---

# 61. Protocol Engine

The protocol engine consumes validated domain objects and evaluates requirements.

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
        evidence: EntityRef,
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

The engine should itself be deterministic for a given validated state and evidence set.

---

# 62. Harness Interaction

The agent harness uses the protocol engine and interaction contract.

```text
Harness
   │
   ├── resolve task + artifacts
   ├── ask protocol for requirements
   ├── ask protocol for capabilities
   ├── expose permitted tools
   ├── agent reasons
   ├── agent proposes command
   ├── protocol/harness validates permission
   ├── execute command
   ├── collect events + evidence
   ├── run verifiers
   ├── submit evidence
   ├── evaluate transition
   └── repeat
```

The model can be replaced without changing the methodology.

---

# 63. Development Example

```yaml
task:
  id: AUTH-142
  type: feature

  objective:
    add_support_for: passkeys

  specification:
    requirements:
      - existing_password_login_must_continue_working
      - passkeys_must_be_user_scoped
      - failed_authentication_must_not_create_session

    invariants:
      - unauthenticated_request_never_receives_session
      - one_credential_belongs_to_exactly_one_user

  protocol:
    version: aep/1
    profile: development.standard

  principles:
    add:
      - differential-testing

  completion:
    require:
      - acceptance_tests_pass
      - regression_tests_pass
      - contracts_pass
      - static_analysis_pass
      - provenance_complete
```

---

# 64. Incident Example

```yaml
protocol: aep/1
profile: incident.standard

objective:
  restore_service: checkout-api

principles:
  - minimize-customer-impact
  - preserve-evidence
  - least-privilege
  - reversible-changes
  - hypothesis-driven-diagnosis
  - verify-after-action

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

# 65. Release Example

```yaml
protocol: aep/1
profile: release.progressive

principles:
  - progressive-delivery
  - automated-verification
  - reversible-changes
  - blast-radius-limitation

workflow:
  - qualify
  - deploy-canary
  - observe
  - verify
  - promote

on_failure:
  action: rollback
```

---

# 66. Structural vs Semantic Validation

Generated JSON Schema validates structure.

Examples:

- required fields exist;
- enums are valid;
- identifier shapes are correct;
- arrays contain the correct value types.

Rust performs semantic validation.

Examples:

- every relation target exists;
- every non-terminal workflow state has a legal outgoing transition;
- every required evidence type has a verifier;
- rollback is not required for an irreversible action;
- a task does not require a capability that policy denies;
- production mutation requires configured approval where applicable;
- review evidence references the exact reviewed revision;
- completion predicates reference observable values;
- an entity cannot supersede itself;
- immutable evidence cannot be modified.

---

# 67. Generated Schemas

Rust types should derive schema information where practical.

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

A repository task generates schemas:

```text
cargo xtask schema
```

CI verifies generated schemas are current.

---

# 68. Versioning

Protocols, types, commands, relations, principles, profiles, workflows, and schemas all need explicit versions.

Examples:

```text
aep/1
adp/1

aep.design/v1
aep.design.approve/v1

principle:test-driven/1
workflow:incident-standard/2
```

Unknown major versions must not be silently interpreted.

Profiles should pin compatible versions.

---

# 69. Repository Structure

A consolidated initial repository layout:

```text
engineering-protocols/
├── Cargo.toml
│
├── crates/
│   ├── aep-domain/
│   ├── aep-contract/
│   ├── aep-engine/
│   ├── aep-schema/
│   ├── aep-conformance/
│   ├── adp-domain/
│   ├── aop-domain/
│   └── protocol-cli/
│
├── artifacts/
│   ├── kinds/
│   ├── relations/
│   ├── lifecycles/
│   └── templates/
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
├── conformance/
│   ├── fixtures/
│   ├── scenarios/
│   └── expected/
│
├── examples/
│
└── xtask/
```

---

# 70. Crate Responsibilities

## `aep-domain`

Contains:

- entity types;
- relations;
- artifact taxonomy;
- commands;
- events;
- evidence;
- audit types;
- validated domain semantics.

## `aep-contract`

Contains storage-independent interaction traits:

- command service;
- query service;
- type registry;
- consistency semantics.

## `aep-engine`

Contains:

- principle resolution;
- workflow evaluation;
- capability evaluation;
- completion predicates;
- transition logic.

## `aep-schema`

Contains:

- wire representations;
- schema generation;
- compatibility helpers.

## `aep-conformance`

Contains reusable black-box conformance tests for implementations.

## `adp-domain`

Development-specific types and commands.

## `aop-domain`

Operations/SRE-specific types and commands.

## `protocol-cli`

Reference commands such as:

```text
protocol validate
protocol resolve
protocol inspect
protocol explain
protocol schema
protocol conformance
```

---

# 71. Project Repository Convention

The protocol should not require a directory structure.

A recommended human-oriented convention is:

```text
project/
├── src/
├── tests/
│
└── docs/
    ├── product/
    ├── specs/
    ├── designs/
    ├── architecture/
    ├── adr/
    ├── releases/
    ├── runbooks/
    └── incidents/
```

---

# 72. Machine-Readable Project Metadata

A project may additionally use:

```text
project/
└── .engineering/
    ├── protocol.yaml
    ├── task.yaml
    ├── artifacts.yaml
    └── state.yaml
```

The division is:

```text
docs/
    durable human-readable engineering artifacts

.engineering/
    machine-readable protocol references and metadata
```

`.engineering` should point to artifacts rather than duplicate them.

---

# 73. Artifact Manifest Example

```yaml
version: aep.artifacts/1

artifacts:

  - id: prd:passkeys
    kind: product-requirements

    location:
      provider: linear
      reference: PASSKEYS-PRD

  - id: story:AUTH-142
    kind: story

    location:
      provider: linear
      reference: AUTH-142

    relations:
      - derived_from: prd:passkeys

  - id: spec:passkeys-auth
    kind: specification

    location:
      path: docs/specs/passkeys-auth.md

    relations:
      - specifies: story:AUTH-142

  - id: design:passkeys-auth
    kind: design
    status: approved

    location:
      path: docs/designs/passkeys-auth.md

    relations:
      - designs: spec:passkeys-auth

  - id: adr:0042
    kind: architecture-decision-record
    status: active

    location:
      path: docs/adr/0042-passkey-credential-ownership.md

    relations:
      - decides: design:passkeys-auth
```

---

# 74. Backend Adapters

Backend/storage implementations are intentionally outside the core repository semantics.

Possible adapters:

```text
GitAdapter
FilesystemAdapter
PostgresAdapter
GitHubAdapter
LinearAdapter
JiraAdapter
RemoteAepAdapter
CompositeAdapter
```

A composite backend might resolve:

```text
Story        → Linear
Design       → Git
ADR          → Git
Review       → GitHub
Incident     → incident system
Protocol     → engineering-protocols repository
```

while presenting one logical command/query surface.

---

# 75. Backend Conformance

A major goal of `engineering-protocols` is to make backend implementations independently verifiable.

A backend should not be considered conformant because its author says:

> It implements the contract.

It should be able to execute the standard conformance suite from this repository.

The suite must be **black-box**.

It uses only:

```text
command side
query side
type descriptions
```

It never inspects:

- tables;
- files;
- event logs;
- database internals;
- provider-specific APIs.

This allows implementations using completely different technologies to prove the same observable behavior.

---

# 76. Conformance Test Contract

A backend implementation provides a test fixture/factory.

Conceptually:

```rust
#[async_trait]
pub trait ConformanceFixture {
    type Backend: CommandService + QueryService + TypeRegistry;

    async fn fresh_backend(&self) -> Self::Backend;
}
```

The repository provides the tests.

Possible developer experience:

```rust
engineering_protocols_conformance!(
    MyBackendFixture
);
```

or:

```rust
aep_conformance::run_all(
    MyBackendFactory::new()
).await?;
```

The exact Rust testing API can be refined later.

The important principle is:

> **The test semantics are owned by `engineering-protocols`, not by the backend.**

---

# 77. Black-Box Test Philosophy

Every conformance test should operate as a real consumer.

Example:

```text
1. issue command
2. receive command result
3. query using returned consistency token
4. verify expected state
5. query audit
6. verify attribution and causality
```

No test may depend on a particular persistence design.

---

# 78. Core Conformance Suites

Conformance should be divided into suites.

## 78.1 Identity Conformance

Validate:

- canonical IDs are unique;
- locators resolve to the correct entity;
- aliases do not replace canonical identity;
- entity type is preserved;
- revision numbers obey contract semantics.

---

## 78.2 Command Conformance

Validate:

- valid commands succeed;
- invalid commands fail with typed errors;
- semantic lifecycle commands enforce invariants;
- rejected commands do not mutate state;
- command results identify affected revisions;
- command types are versioned.

---

## 78.3 Idempotency Conformance

Scenario:

```text
execute command C
retry same logical command C
```

Validate:

- no duplicate mutation occurs;
- resulting entity identity is unchanged;
- resulting revision is unchanged by the replay;
- returned logical result is equivalent;
- retry attempt remains auditable.

---

## 78.4 Concurrency Conformance

Scenario:

```text
read revision 4

execute command expecting revision 4
→ revision 5

execute second command expecting revision 4
```

Validate:

```text
revision_conflict
```

and no second mutation.

---

## 78.5 Query Conformance

Validate:

- get;
- resolve;
- type filtering;
- predicate filtering;
- relation traversal;
- pagination semantics;
- historical revision retrieval where supported by core profile.

---

## 78.6 Consistency Conformance

Scenario:

```text
execute command
receive consistency token T
query AtLeast(T)
```

Validate:

- query reflects the successful mutation;
- no arbitrary sleep is required;
- a backend may internally wait for projections.

---

## 78.7 Relation Conformance

Validate:

- allowed relations can be created;
- invalid source/target combinations are rejected;
- duplicate relation semantics are defined;
- immutable historical relations cannot be removed;
- graph queries return expected edges.

---

## 78.8 History Conformance

Validate:

- revisions are observable;
- historical state is reconstructable;
- revision ordering is deterministic;
- semantic state transitions appear in history.

---

## 78.9 Immutability Conformance

For immutable types such as certain evidence/review records:

```text
create
attempt mutation
```

Validate:

```text
mutation rejected
original revision unchanged
```

---

## 78.10 Audit Conformance

For every successful mutation validate that audit can answer:

```text
who
what
when
subject
before revision
after revision
command
correlation
```

Validate that audit records are immutable.

---

## 78.11 Rejected-Action Audit Conformance

Execute an intentionally invalid or forbidden command.

Validate:

- no domain mutation occurred;
- command rejection is queryable in audit;
- actor is present;
- command/request identity is present;
- reason code is machine-readable;
- correlation ID is preserved.

---

## 78.12 Correlation Conformance

Execute multiple commands under the same correlation ID.

Validate:

```text
audit(correlation_id=C)
```

returns the complete chain.

---

## 78.13 Causation Conformance

Execute a scenario where:

```text
command A
  ↓
event A1
  ↓
command B
```

Validate:

```text
A1.causation == command A
B.causation == A1 or the explicit protocol decision caused by A1
```

depending on the scenario.

The causal graph must be reconstructable.

---

## 78.14 Provenance Conformance

Validate:

- actor attribution;
- executor attribution where provided;
- creation provenance;
- update provenance;
- protocol execution reference where provided.

---

## 78.15 Event Conformance

Validate:

- successful semantic commands emit required events;
- events reference the causing command;
- events share the correct correlation ID;
- entity revision references are correct;
- events are immutable.

---

## 78.16 Type Registry Conformance

Validate:

- registered types can be described;
- schema references exist;
- command descriptors are discoverable;
- legal relation descriptors are discoverable;
- immutable types are marked correctly.

---

# 79. Domain Conformance Suites

Generic backend conformance verifies the universal contract.

Additional suites verify domain semantics.

Examples:

```text
ADP conformance
AOP conformance
Incident profile conformance
Release profile conformance
```

---

# 80. ADP Conformance Example

A standard scenario may define:

```text
create Story
create Specification
relate Specification → Story
create Design
relate Design → Specification
create ReviewResult for Design@revision
approve Design
```

Tests validate:

- approval cannot occur before required review;
- review must target current design revision;
- required relations exist;
- superseding design changes lifecycle correctly;
- audit chain remains complete.

---

# 81. Incident Conformance Example

A standard incident scenario may validate:

```text
AcknowledgeIncident
RecordHypothesis
RecordObservation
MitigateIncident
VerifyRecovery
ResolveIncident
```

Invalid sequences must be rejected.

For example:

```text
ResolveIncident
```

before recovery verification may fail under the selected profile.

---

# 82. Release Conformance Example

A progressive release scenario may validate:

```text
QualifyRelease
DeployCanary
RecordObservation
PromoteRelease
```

and failure path:

```text
DeployCanary
RecordFailedObservation
RollbackRelease
```

Audit and causal links must remain intact.

---

# 83. Reference Scenarios

The repository should contain reusable deterministic scenario definitions.

Example directory:

```text
conformance/
├── scenarios/
│   ├── entity-lifecycle/
│   ├── optimistic-concurrency/
│   ├── idempotency/
│   ├── audit-chain/
│   ├── design-review/
│   ├── incident-recovery/
│   └── progressive-release/
```

Each scenario defines:

```text
initial state
commands
expected results
queries
expected entities
expected relations
expected audit properties
expected errors
```

This enables non-Rust backends to eventually reuse the same semantic scenarios.

---

# 84. Language-Neutral Conformance

Rust is the source of truth for domain types, but conformance should not remain Rust-only forever.

Generated scenario fixtures can use a language-neutral representation such as YAML or JSON.

Example:

```yaml
scenario: optimistic-concurrency/v1

steps:

  - command:
      type: aep.entity.create/v1
      as: create_result

  - query:
      get: $create_result.entity
      as: entity_v1

  - command:
      type: aep.entity.update/v1
      expected_revision: $entity_v1.revision

  - command:
      type: aep.entity.update/v1
      expected_revision: $entity_v1.revision

    expect_error:
      code: revision_conflict
```

A Rust conformance runner is the reference implementation.

Other languages can later consume the same scenarios.

---

# 85. Conformance Levels

It may be useful to define formal compatibility claims.

For example:

```text
AEP Core Backend Conformant / v1
AEP Audit Conformant / v1
ADP Domain Conformant / v1
AOP Incident Conformant / v1
```

This avoids forcing every backend to implement every possible domain on day one.

---

# 86. Atomicity Requirements

The logical command contract should define a minimum atomicity guarantee.

For a successful command, the following must not become observably inconsistent:

```text
domain state mutation
required revision update
required event emission
required audit record
```

A caller must not observe:

```text
entity changed
but command audit permanently absent
```

or:

```text
command reported success
but entity state missing
```

The backend may achieve this through:

- database transactions;
- event logs;
- outbox patterns;
- other technology.

The mechanism is out of scope.

The observable guarantee is not.

---

# 87. Failed Command Atomicity

If a command is rejected:

```text
domain state must remain unchanged
```

while the rejection remains auditable.

The audit trail is therefore not equivalent to the domain transaction itself.

---

# 88. Querying Audit

Audit is part of the query surface.

Examples:

```text
all audit records for entity E

all records under correlation C42

all records caused by command CA

all actions by actor A

all commands executed by agent instance X

all denied production.write attempts

all protocol transitions for execution EXEC-882
```

This is important for:

- incident investigation;
- compliance;
- agent evaluation;
- debugging;
- replay analysis;
- provenance.

---

# 89. Artifact Provenance

Artifacts require provenance too.

Possible fields:

```rust
pub struct ArtifactProvenance {
    pub created_by: ActorRef,
    pub created_at: Timestamp,
    pub source: ArtifactSource,
    pub revision: Revision,
}
```

Authorship classes may include:

```text
human-authored
agent-authored
tool-generated
derived
imported
```

Authorship does not itself imply correctness.

---

# 90. Generated vs Authored Artifacts

Some artifacts are naturally generated:

```text
API documentation
schema references
dependency graphs
coverage reports
deployment manifests
verification reports
```

Others are primarily authored:

```text
product intent
design rationale
ADR context
postmortem analysis
```

Both are entities and can participate in the same graph.

Generated artifacts should identify their generator and source revision.

---

# 91. Artifact Freshness

The validity of an artifact may depend on revision.

Example:

```text
Design reviewed against source SHA A.

Implementation now corresponds to SHA B.
```

Possible future policy:

```rust
pub enum FreshnessPolicy {
    AlwaysValid,
    UntilSuperseded,
    BoundToRevision,
    BoundToDependencySet,
}
```

Reviews and approvals can use versioned references to avoid ambiguity.

---

# 92. Cross-Repository Architecture

Artifacts may span repositories.

Example:

```text
architecture:identity-platform
    │
    ├─ designs → repo:identity/design:passkeys
    ├─ designs → repo:web/design:passkey-login
    └─ decided-by → adr:credential-ownership
```

The first implementation only needs stable cross-repository entity references.

Federated resolution can evolve later.

---

# 93. Planning Tool Integration

AEP should reference planning-system objects without importing the entire provider data model.

Example:

```yaml
location:
  provider: linear
  reference: AUTH-142
```

or:

```yaml
location:
  provider: jira
  reference: IAM-938
```

The integration resolves the provider reference.

AEP defines its semantic type and relations.

---

# 94. Templates

The repository may ship non-normative reference templates:

```text
artifacts/templates/
├── specification.md
├── design.md
├── architecture-design.md
├── adr.md
├── test-plan.md
├── release-plan.md
├── runbook.md
└── postmortem.md
```

Templates are conveniences.

The typed semantics are normative.

---

# 95. Initial Design Template Semantics

A default design may contain:

```text
Context
Goals
Non-Goals
Current State
Proposed Design
Interfaces
Data Model
Failure Modes
Security
Observability
Migration
Rollout
Rollback
Open Questions
```

A profile may require semantic sections without requiring exact Markdown headings.

---

# 96. Initial ADR Semantics

An ADR may contain:

```text
Title
Status
Context
Decision
Alternatives
Consequences
```

This may also be represented as a typed object:

```rust
pub struct ArchitectureDecisionRecord {
    pub title: String,
    pub status: AdrStatus,
    pub context: Markdown,
    pub decision: Markdown,
    pub alternatives: Vec<Alternative>,
    pub consequences: Markdown,
}
```

Presentation remains separate from semantics.

---

# 97. Security and Least Privilege

The protocol model should support:

```text
allowed
denied
approval_required
```

at command/capability level.

The backend contract does not define credential issuance.

The harness and integration layer map protocol decisions onto:

- credentials;
- tool visibility;
- network access;
- environment permissions.

All privileged command attempts should remain auditable.

---

# 98. Clean-Room Implementation

Clean-room implementation can be represented as a principle constraining:

- permitted source artifacts;
- prohibited source artifacts;
- provenance;
- comparison/verifier methods.

Example:

```yaml
principle: clean-room

capabilities:
  deny:
    - source.original.read

requires:
  artifacts:
    - kind: specification

verification:
  - differential-testing
  - contract-testing
```

This illustrates why protocol, artifact, capability, and verification models belong together.

---

# 99. Context Engineering

Context engineering is a principle governing which entities and artifacts are made available to an agent.

The harness can construct context from:

```text
task
specification
design
related ADRs
relevant code
required principles
current workflow state
prior verifier results
counterexamples
```

and exclude unrelated or prohibited information.

Context becomes a controlled projection of the entity graph rather than an arbitrary prompt dump.

---

# 100. Task Decomposition

Task decomposition can be represented through entity creation and `Decomposes` relations.

Example:

```text
EPIC
  ├─ decomposes → STORY A
  └─ decomposes → STORY B
```

Agent-generated decomposition remains subject to validation and review policies.

---

# 101. SRE and Development Analogy

Several development principles have operational counterparts.

| Development | Operations/SRE |
|---|---|
| TDD | define health/success criteria before change |
| contract testing | dependency/SLO/interface validation |
| differential testing | canary vs control comparison |
| property/invariant testing | operational safety invariants |
| sandboxing | scoped production credentials |
| CEGIS | hypothesis → action → telemetry → revised hypothesis |
| provenance | incident/change audit trail |
| rollback tests | reversible operational change |

This supports one AEP model across development and operations.

---

# 102. Non-Goals

`engineering-protocols` should not initially become:

- an LLM orchestration framework;
- an agent implementation;
- a CI system;
- a deployment platform;
- a ticket tracker;
- a document database;
- a knowledge-management system;
- an incident-management product;
- an enterprise architecture repository;
- a shell sandbox;
- a credential provider;
- a replacement for OPA;
- a mandatory filesystem layout;
- a universal ontology for all software engineering;
- a required event-sourced persistence model.

Its responsibility is narrower:

> Define the semantics by which engineering entities are addressed, changed, related, constrained, evidenced, verified, audited, and progressed.

---

# 103. Initial Implementation Scope

Version 0 should remain intentionally small.

## Core entity types

Start with:

```text
Task
Specification
Design
ArchitectureDesign
ArchitectureDecisionRecord
ReviewResult
Evidence
Protocol
Principle
Workflow
Profile
```

## Core relations

Start with:

```text
DerivedFrom
Decomposes
Specifies
Designs
Decides
Reviews
Verifies
Supersedes
```

## Core commands

Start with:

```text
CreateEntity
UpdateEntity
CreateRelation
RemoveRelation
ArchiveEntity
SupersedeEntity

SubmitDesignReview
ApproveDesign
AcceptAdr
```

## Core query operations

Start with:

```text
get
resolve
query
relations
history
audit
describe_type
```

## Core principles

Start with:

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

## Core profiles

Start with:

```text
development.standard
incident.standard
release.progressive
```

## Core conformance

Start with:

```text
identity
command execution
idempotency
optimistic concurrency
query consistency
relations
history
audit
correlation
causation
type discovery
```

---

# 104. First Reference Backend Test Scenario

A minimal end-to-end conformance scenario should prove the entire design shape.

```text
1. Create Story
2. Query Story
3. Create Specification
4. Relate Specification → Story
5. Create Design
6. Relate Design → Specification
7. Create ReviewResult for Design@revision
8. Approve Design
9. Query Design and verify approved state
10. Query relations
11. Query entity history
12. Query audit by entity
13. Query audit by correlation ID
14. Verify actor/executor
15. Verify causal links
16. Replay approval command idempotently
17. Attempt stale-revision update
18. Verify typed conflict
19. Verify rejected attempt audit
```

A backend that passes this scenario has demonstrated much more than simple CRUD compatibility.

---

# 105. Core System Invariants

The consolidated design should protect at least these invariants:

1. Every entity has stable canonical identity.
2. Every entity has a versioned type.
3. Every mutation is represented as a command.
4. Queries never mutate state.
5. Successful commands are idempotently retryable.
6. Revision conflicts cannot silently overwrite newer state.
7. Successful state changes are queryable with read-your-writes semantics.
8. Relations are typed and validated.
9. Semantic lifecycle changes use semantic commands.
10. Required evidence is independent from agent self-assertion.
11. Protocol transitions are deterministic given state and evidence.
12. Reviews and approvals can target exact revisions.
13. Every important mutation is attributable.
14. Correlated activity can be reconstructed.
15. Immediate causation can be reconstructed.
16. Rejected high-value actions remain auditable.
17. Audit records are immutable logical records.
18. What changed is reconstructable.
19. Storage technology does not leak into domain semantics.
20. A backend can prove conformance through a standard black-box test suite.

---

# 106. Updated Core Thesis

The complete system can be summarized as:

```text
The model reasons.

The protocol constrains.

The command side changes state.

The query side observes state.

The harness executes.

The environment produces observations.

The verifiers establish facts.

The artifact graph preserves intent,
design, decisions, and evidence.

The protocol engine decides
what those facts permit.

The audit model records
who did what, when, why,
what caused it, and what followed.

The conformance suite proves
that independent implementations
honor the same observable contract.
```

---

# 107. Desired End State

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

and determine without prompt interpretation:

- which artifacts govern the task;
- which workflow applies;
- what the current state requires;
- which commands are available;
- which commands require approval;
- which capabilities the agent receives;
- what evidence must be produced;
- which verifiers must run;
- what constitutes failure;
- how retries work;
- what transitions are permitted;
- exactly what constitutes completion.

When the agent executes:

```text
ApproveDesign
```

the system should also be able to answer later:

```text
Who initiated the approval?
Which agent executed it?
Which design revision was approved?
Which review justified it?
Which protocol execution was active?
Which command caused the approval event?
What broader activity was it correlated with?
Which transition did it unlock?
Which later command was caused by that transition?
What exactly changed?
When did every step happen?
```

Finally, an independent backend implementation should be able to plug into:

```text
CommandService
+
QueryService
+
TypeRegistry
```

run the standard tests from `engineering-protocols`, and demonstrate:

```text
AEP Core Backend Conformant / v1
AEP Audit Conformant / v1
```

without the conformance suite knowing or caring whether the backend uses:

```text
Git
Postgres
event sourcing
filesystem files
Linear
Jira
GitHub
a distributed service
or a composite of all of them
```

That is the target architecture of `engineering-protocols`:

> **A strongly typed, storage-independent, fully auditable and testable contract for machine-executable engineering methodology.**
