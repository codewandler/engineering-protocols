# Engineering Protocols — Artifact Model Extension v0.1

> **Status:** Additive design extension  
> **Relationship to prior draft:** This document extends the original *Engineering Protocols — Design Draft v0.1*. It does not replace or correct the original design.  
> **Focus:** Engineering artifacts, planning decomposition, design and architecture records, review semantics, and repository conventions.

---

## 1. Summary

The original `engineering-protocols` design defines a machine-executable methodology around:

- tasks;
- principles;
- workflows;
- states;
- transitions;
- capabilities;
- evidence;
- verification;
- approvals;
- completion predicates.

This extension adds another first-class domain concept:

> **The engineering artifact graph.**

Engineering work does not only progress through workflow states. It also creates, consumes, refines, reviews, supersedes, and verifies artifacts such as:

- vision documents;
- product requirements;
- epics;
- stories;
- engineering specifications;
- designs;
- architecture designs;
- ADRs;
- test plans;
- reviews;
- release plans;
- runbooks;
- incident reports;
- postmortems.

The purpose of this extension is to give those artifacts explicit semantics so that a protocol can reason about them independently of where they are stored.

The core addition becomes:

```text
TASKS
   +
ARTIFACTS
   +
RELATIONSHIPS
   +
PRINCIPLES
   +
WORKFLOWS
   +
EVIDENCE
   +
CAPABILITIES
   +
VERIFIERS
        │
        ▼
   PROTOCOL ENGINE
```

The protocol should understand that a design exists, what it designs, whether it has been reviewed, which ADRs were derived from it, and whether the required artifacts exist before execution continues.

It should not need to assume that every design is a Markdown file in `docs/designs/`.

---

## 2. Motivation

A common engineering workflow contains concepts such as:

```text
Vision
  ↓
PRD
  ↓
Epic
  ↓
Story
  ↓
Specification
  ↓
Design
  ↓
Implementation
  ↓
Review
  ↓
Release
```

This representation is useful but incomplete.

In practice:

- one design may span several stories;
- one epic may require multiple designs;
- one architecture design may affect many repositories;
- one design may produce several ADRs;
- a story may not require a design at all;
- reviews can target designs, code, architecture, release plans, or incidents;
- a PRD may live in a product system rather than Git;
- an epic may exist in Linear or Jira;
- an ADR may live as Markdown;
- an architecture description may be generated from source;
- verification evidence may come from CI rather than a document.

The protocol therefore needs to represent **semantic relationships**, not merely paths in a directory tree.

---

## 3. Three Separate Concerns

This design distinguishes three layers.

### 3.1 Planning and decomposition model

This describes how intent is decomposed into executable work.

Typical examples:

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

The purpose is to answer:

> What are we trying to achieve, and how is that intent decomposed?

This is primarily a planning concern.

---

### 3.2 Engineering artifact model

This describes the durable artifacts created or consumed during engineering work.

Examples:

```text
Specification
Design
Architecture Design
ADR
Test Plan
Review Result
Release Plan
Runbook
Postmortem
```

The purpose is to answer:

> What durable engineering knowledge or evidence exists?

This belongs directly in `engineering-protocols`.

---

### 3.3 Physical repository convention

This describes where artifacts happen to live.

Example:

```text
docs/
├── specs/
├── designs/
├── architecture/
├── adr/
└── reviews/
```

The purpose is convenience for humans and tools.

This should be standardized only as a **recommended convention**, not as the semantic model itself.

---

## 4. Intent and Planning Hierarchy

The following hierarchy is a useful default planning model:

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

These concepts answer progressively narrower questions.

| Artifact | Primary question |
|---|---|
| Vision | Where are we going and why? |
| PRD | What outcome or product capability should exist? |
| Epic | What major deliverable contributes to that outcome? |
| Story | What independently meaningful behavior or change is required? |
| Task | What concrete unit of work must be performed? |

This hierarchy should be treated as a common planning profile, not a mandatory universal structure.

Some organizations may use:

```text
Initiative → Epic → Story
```

Others:

```text
Objective → Project → Work Item
```

AEP should therefore model planning relationships generically while allowing named artifact kinds.

---

## 5. Engineering Specification

An engineering specification answers:

> Precisely what behavior or constraints must the implementation satisfy?

It is distinct from a PRD.

A PRD may say:

```text
Users must be able to sign in with passkeys.
```

An engineering specification may state:

```text
- Passkey credentials are scoped to exactly one user.
- Failed authentication must never create a session.
- Existing password authentication remains supported.
- The authentication API remains backward compatible.
```

Specifications therefore sit near the boundary between product intent and executable engineering constraints.

A development protocol may require that a specification exists before implementation begins.

---

## 6. Design

A design describes the proposed solution.

Typical contents may include:

- system context;
- affected components;
- data flow;
- API changes;
- storage changes;
- invariants;
- failure modes;
- security considerations;
- migration strategy;
- observability;
- rollout;
- rollback.

A design answers:

> How do we intend to satisfy the specification?

A design may target:

- a feature;
- a story;
- an epic;
- a service;
- a migration;
- a cross-system change.

Therefore:

```text
Design != Story
```

and:

```text
Design != Task
```

A design is a solution artifact, while stories and tasks are units of planned work.

---

## 7. Architecture Design

Architecture design is a specialized design artifact with broader scope.

A useful hierarchy is:

```text
Design
├── ComponentDesign
├── FeatureDesign
├── ApiDesign
├── DataDesign
└── ArchitectureDesign
```

Architecture designs commonly answer questions such as:

- which systems own which responsibilities;
- how services communicate;
- where boundaries exist;
- where data is stored;
- which invariants hold across systems;
- which major dependencies exist;
- how the target architecture differs from the current state.

Architecture is therefore not an unrelated concept outside design.

It is a design category with:

- broader scope;
- longer expected lifetime;
- stronger governance;
- greater cross-system impact.

A protocol may apply additional requirements when:

```yaml
change:
  architectural: true
```

For example:

```yaml
require:
  - artifact: architecture-design
  - artifact: architecture-decision-record
  - review: architecture
```

---

## 8. Architecture Decision Records

An ADR answers:

> What important decision was made, why was it made, and what are its consequences?

A design document describes a proposed solution as a whole.

An ADR records a specific durable decision.

Example design:

```text
Passkey Authentication Design

- authentication flow
- components
- credential storage
- API behavior
- migration
- rollout
```

Possible ADR extracted from that design:

```text
ADR-0042

Decision:
Store passkey credentials in the identity service.

Alternatives:
- dedicated credential service
- application-local credential storage

Consequences:
- identity service becomes credential owner
- authentication remains centralized
- migration must preserve existing identity IDs
```

One design may therefore produce:

```text
0..N ADRs
```

ADRs should generally be immutable historical records after acceptance, except for metadata such as status.

Later decisions should supersede earlier ADRs rather than silently rewriting their history.

---

## 9. Reviews

Review should not primarily be modeled as another document type.

It is first an **activity and protocol event**.

For example:

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

The review may produce a durable artifact or evidence record.

This distinction is important because an AEP workflow may require:

```yaml
transition:
  from: design
  to: implement

requires:
  review:
    subject: current-design
    result: approved
```

The protocol does not need to care whether the review occurred:

- in GitHub;
- in a pull request;
- in Gerrit;
- in an internal review system;
- via a signed approval artifact.

It only needs a verifiable `ReviewResult`.

---

## 10. Artifact Taxonomy

A possible initial Rust taxonomy:

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

AEP should not attempt to impose a complete universal ontology for every organization.

---

## 11. Artifact Domain Model

A basic representation:

```rust
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub version: ArtifactVersion,
    pub status: ArtifactStatus,
    pub location: ArtifactLocation,
    pub relations: Vec<ArtifactRelation>,
    pub metadata: ArtifactMetadata,
}
```

An artifact location should not assume a local file.

For example:

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

This permits artifacts to reside in:

- the repository;
- GitHub;
- Linear;
- Jira;
- Notion;
- Confluence;
- an internal planning system;
- another source of truth.

---

## 12. Artifact Relationships

The graph is more important than the location.

A possible model:

```rust
pub enum ArtifactRelation {
    InformedBy(ArtifactRef),
    DerivedFrom(ArtifactRef),
    Decomposes(ArtifactRef),

    Specifies(ArtifactRef),
    Designs(ArtifactRef),
    Implements(ArtifactRef),

    Decides(ArtifactRef),

    Reviews(ArtifactRef),
    Verifies(ArtifactRef),

    Blocks(ArtifactRef),
    DependsOn(ArtifactRef),

    Supersedes(ArtifactRef),
}
```

Example graph:

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
```

This graph gives the protocol a machine-readable chain of intent.

---

## 13. Artifact Status

Artifacts often have lifecycle state independent of workflow state.

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

Not all artifact kinds need every status.

Validated domain types should constrain legal transitions.

For example:

```text
ADR:
Proposed → Accepted → Superseded
```

while:

```text
Design:
Draft → InReview → Approved → Implemented
```

These may later become artifact-kind-specific state machines.

---

## 14. Protocol Requirements Over Artifacts

Protocols can require artifacts before workflow transitions.

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

Conditional requirements become particularly useful:

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

This makes engineering governance executable.

---

## 15. Artifact Requirements as Principles

Some requirements may belong to reusable principles rather than workflows.

For example:

```yaml
id: spec-driven

requires:
  before:
    state: implement

  artifacts:
    - kind: specification
      status: approved
```

Similarly:

```yaml
id: architecture-decision-records

applies_when:
  change.architectural: true

requires:
  artifacts:
    - kind: architecture-decision-record
```

A protocol profile can therefore compose artifact policy through principles.

---

## 16. Artifact Evidence

Artifact existence is not sufficient in every case.

The protocol may need evidence that an artifact:

- exists;
- is valid;
- has been reviewed;
- is current;
- refers to the correct task;
- has not been superseded;
- satisfies a schema;
- contains required sections.

A possible evidence type:

```rust
pub struct ArtifactEvidence {
    pub artifact: ArtifactRef,
    pub observation: ArtifactObservation,
}
```

For example:

```rust
pub enum ArtifactObservation {
    Exists,
    SchemaValid,
    Approved,
    Reviewed,
    Current,
    RelationshipValid,
}
```

These observations become inputs into transition predicates.

---

## 17. Planning vs Protocol Execution

Vision, PRD, epics, and stories largely belong to **intent decomposition**.

They are not necessarily generated or owned by the protocol engine.

AEP should instead:

1. model them;
2. resolve references to them;
3. validate required relationships;
4. use them as context and constraints.

For example:

```yaml
task:
  id: AUTH-142

derived_from:
  - artifact: story:AUTH-141

context:
  product_requirements:
    - artifact: prd:PASSKEYS
```

The protocol engine does not need to become a project-management product.

---

## 18. End-to-End Artifact Lifecycle

A useful generalized lifecycle:

```text
INTENT
  Vision
    ↓
  Product Requirements

DECOMPOSITION
  Initiative
    ↓
  Epic
    ↓
  Story / Task

SPECIFICATION
  Engineering Specification
  Acceptance Criteria

SOLUTION
  Design
  Architecture Design
  ADR

VERIFICATION DESIGN
  Test Plan
  Evaluation Plan
  Verification Criteria

EXECUTION
  Code
  Configuration
  Infrastructure
  Migration

VERIFICATION
  Tests
  Evals
  Static Analysis
  Contract Verification
  Telemetry

GOVERNANCE
  Review
  Approval

DELIVERY
  Release Plan
  Release

OPERATIONS
  Runbook
  Observation
  Incident

LEARNING
  Incident Report
  Postmortem
  ADR / Design Update
```

This is not necessarily one linear workflow.

It is a map of common engineering artifacts and their roles.

---

## 19. Recommended Project Repository Convention

The protocol should define artifact semantics independently of paths.

However, a default repository convention is still useful.

A human-oriented structure:

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

The categories are intentionally recognizable without understanding AEP.

---

## 20. Machine-Readable Project Metadata

A separate machine-oriented directory may hold protocol metadata:

```text
project/
├── docs/
│   ├── specs/
│   ├── designs/
│   ├── architecture/
│   └── adr/
│
└── .engineering/
    ├── protocol.yaml
    ├── task.yaml
    ├── artifacts.yaml
    └── state.yaml
```

The division is:

```text
docs/
    human-readable engineering artifacts

.engineering/
    machine-readable protocol metadata
```

The `.engineering` directory should remain small.

It should point at artifacts rather than duplicate them.

---

## 21. Example Artifact Manifest

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

  - id: review:design-passkeys-auth
    kind: review-result
    location:
      provider: github
      reference: review-9181

    relations:
      - reviews: design:passkeys-auth
```

---

## 22. Project Repository vs `engineering-protocols`

The actual project repository contains project-specific engineering artifacts.

Example:

```text
github.com/acme/payments
```

may contain:

```text
docs/
├── specs/
├── designs/
├── architecture/
├── adr/
└── runbooks/
```

By contrast:

```text
github.com/acme/engineering-protocols
```

defines:

- what a `Specification` means;
- what a `Design` means;
- what an `ArchitectureDecisionRecord` means;
- valid artifact relationships;
- artifact schemas;
- workflow requirements;
- principles requiring certain artifacts;
- reference templates;
- validation logic.

The split is:

```text
engineering-protocols
        │
        ├── semantics
        ├── schemas
        ├── validation
        ├── protocol rules
        └── conventions


application repositories
        │
        ├── actual specs
        ├── actual designs
        ├── actual ADRs
        ├── actual runbooks
        └── artifact manifests
```

---

## 23. Proposed Repository Extension

The original `engineering-protocols` structure can be extended additively:

```text
engineering-protocols/
├── crates/
│   ├── aep-domain/
│   ├── aep-engine/
│   ├── aep-schema/
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
├── principles/
├── workflows/
├── profiles/
├── schemas/
├── examples/
└── xtask/
```

The `artifacts/` tree contains reference definitions and templates.

The authoritative domain representation remains Rust.

---

## 24. Rust Domain Extension

The AEP domain crate gains artifact concepts.

For example:

```rust
pub struct ArtifactId(String);

pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub status: ArtifactStatus,
    pub version: ArtifactVersion,
    pub location: ArtifactLocation,
    pub relations: Vec<ArtifactRelation>,
}
```

The execution context can expose a graph:

```rust
pub struct ArtifactGraph {
    artifacts: BTreeMap<ArtifactId, Artifact>,
}
```

Possible API:

```rust
impl ArtifactGraph {
    pub fn get(
        &self,
        id: &ArtifactId,
    ) -> Option<&Artifact>;

    pub fn related(
        &self,
        id: &ArtifactId,
        relation: RelationKind,
    ) -> Vec<&Artifact>;

    pub fn validate(
        &self,
    ) -> Result<(), ArtifactValidationErrors>;
}
```

---

## 25. Semantic Validation

Artifact validation should go beyond generated JSON Schema.

Examples:

- every relation target exists;
- an artifact cannot supersede itself;
- an ADR marked `Superseded` must identify a successor where policy requires one;
- an approved review must reference a reviewable subject;
- a design cannot reference a nonexistent specification;
- duplicate active artifact identifiers are invalid;
- an artifact relation cannot create a forbidden cycle;
- an artifact required by a profile must be resolvable;
- an architecture change may require at least one architecture-level design artifact.

These invariants belong in Rust.

---

## 26. Artifact Templates

The protocol repository may ship reference templates.

For example:

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

These are defaults, not the normative semantic model.

An organization may replace the templates while continuing to conform to the same protocol schema.

---

## 27. Example Design Template Semantics

A design profile might require sections conceptually equivalent to:

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

A validator may check for required semantic sections.

The exact Markdown headings should not necessarily be part of the core protocol unless a profile explicitly chooses that convention.

---

## 28. Example ADR Template Semantics

An ADR might require:

```text
Title
Status
Context
Decision
Alternatives
Consequences
```

The semantic fields could also exist in a structured representation:

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

Markdown generation can then be a presentation concern.

---

## 29. Review as Evidence

A design review should produce structured evidence.

For example:

```rust
pub struct ReviewResult {
    pub subject: ArtifactRef,
    pub reviewer: Reviewer,
    pub result: ReviewDisposition,
    pub findings: Vec<Finding>,
    pub reviewed_revision: ArtifactVersion,
}
```

With:

```rust
pub enum ReviewDisposition {
    Approved,
    ChangesRequested,
    Rejected,
}
```

The `reviewed_revision` is important.

An approval for version 3 of a design should not silently approve version 7.

---

## 30. Architecture Governance Example

A profile may define:

```yaml
principle: architecture-governance

applies_when:
  any:
    - change.cross_service == true
    - change.persistence_model == changed
    - change.public_contract == changed
    - change.security_boundary == changed

requires:
  artifacts:
    - kind: architecture-design

  reviews:
    - subject_kind: architecture-design
      result: approved

conditional:
  - when:
      decision.durable == true

    require:
      - artifact: architecture-decision-record
```

This turns architecture governance into executable policy rather than convention.

---

## 31. Development Protocol with Artifact Graph

A complete development path may now look like:

```text
TASK
  │
  ├─ derived from → STORY
  │
  ├─ constrained by → SPECIFICATION
  │
  ├─ solved by → DESIGN
  │                   │
  │                   └─ decisions → ADRs
  │
  ├─ governed by → PRINCIPLES
  │
  ├─ executed through → WORKFLOW
  │
  ├─ produces → IMPLEMENTATION
  │
  └─ validated by → EVIDENCE + REVIEWS
```

The workflow engine can inspect this graph during every transition.

---

## 32. Example Transition

Before entering implementation:

```yaml
transition:
  from: design
  to: implement

requires:

  artifacts:
    - kind: specification
      status: approved

    - kind: design
      status: approved

  reviews:
    - subject_kind: design
      result: approved

  predicates:
    - required_adrs_resolved == true
```

Before completion:

```yaml
transition:
  from: verify
  to: complete

requires:

  predicates:
    - specification.satisfied
    - implementation.matches_design
    - required_reviews.approved
    - required_evidence.missing == 0
```

---

## 33. Cross-Repository Architecture

Architecture artifacts may span multiple repositories.

The artifact graph therefore should not be scoped exclusively to one repository.

Example:

```text
architecture:identity-platform
    │
    ├─ designs → repo:identity/design:passkeys
    ├─ designs → repo:web/design:passkey-login
    └─ decides → adr:credential-ownership
```

A future implementation may support federated artifact manifests.

The initial design only needs stable artifact references.

---

## 34. Planning Tool Integration

AEP should allow references to planning-system objects without importing those systems' entire data models.

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

The harness or integration layer resolves those references.

AEP defines the artifact semantics.

The connector defines how the artifact is fetched.

---

## 35. Artifact Provenance

Artifacts themselves need provenance.

For example:

```rust
pub struct ArtifactProvenance {
    pub created_by: Actor,
    pub created_at: Timestamp,
    pub source: ArtifactSource,
    pub revision: Revision,
}
```

This becomes especially important for agent-generated artifacts.

The protocol should be able to distinguish:

```text
human-authored
agent-authored
tool-generated
derived
imported
```

without assigning correctness based solely on authorship.

---

## 36. Generated vs Authored Artifacts

Some artifacts should be generated.

Examples:

- API documentation;
- dependency graphs;
- schema references;
- coverage reports;
- deployment manifests.

Others are primarily authored:

- design rationale;
- ADR context;
- product intent.

The model should support both.

A generated artifact should ideally include the generator identity and source revision in provenance.

---

## 37. Artifact Freshness

Artifact validity may depend on revision.

For example:

```text
Design approved against repository SHA A.

Implementation now at repository SHA B.
```

A protocol may determine whether the design review remains valid.

Possible future abstraction:

```rust
pub enum FreshnessPolicy {
    AlwaysValid,
    UntilSuperseded,
    BoundToRevision,
    BoundToDependencySet,
}
```

This allows review and evidence semantics to remain precise.

---

## 38. Initial Implementation Scope

The first artifact-model implementation should remain small.

### Artifact kinds

Start with:

```text
ProductRequirements
Epic
Story
Specification
Design
ArchitectureDesign
ArchitectureDecisionRecord
ReviewResult
Runbook
Postmortem
```

### Relationships

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

### Locations

Support:

```text
RepositoryPath
Url
ExternalReference
```

### Status

Support:

```text
Draft
InReview
Approved
Active
Implemented
Superseded
Archived
```

---

## 39. Non-Goals

This extension should not make `engineering-protocols`:

- a replacement for Jira or Linear;
- a document database;
- a documentation renderer;
- a knowledge-management system;
- a mandatory directory-layout standard;
- an enterprise architecture repository;
- a universal project-management ontology.

Its responsibility remains:

> Define enough artifact semantics for machine-executable engineering protocols to reason about engineering intent, design, decisions, review, verification, and operational knowledge.

---

## 40. Updated Core Thesis

The original design established:

> The model reasons.  
> The harness executes.  
> The environment observes.  
> The verifiers establish facts.  
> The protocol decides what those facts permit.

This extension adds:

> **Artifacts preserve intent, design, decisions, and engineering knowledge across workflow states.**

The complete abstraction becomes:

```text
                     INTENT
                       │
                 ARTIFACT GRAPH
                       │
              ┌────────┼────────┐
              │        │        │
          PRINCIPLES  TASKS   WORKFLOWS
              │        │        │
              └────────┼────────┘
                       │
                 PROTOCOL ENGINE
                       │
              allowed requirements
                       │
                       ▼
                     AGENT
                       │
                    ACTIONS
                       │
                       ▼
                   EVIDENCE
                       │
                       ▼
                  VERIFIERS
                       │
                       ▼
                  TRANSITIONS
                       │
                       └──────→ updated artifact graph
```

This gives `engineering-protocols` a durable model not only for **how engineering work proceeds**, but also for **what engineering knowledge must exist before, during, and after that work**.

---

## 41. Desired End State

A project may eventually contain:

```text
payments/
├── docs/
│   ├── specs/
│   │   └── passkeys.md
│   ├── designs/
│   │   └── passkeys.md
│   ├── architecture/
│   │   └── identity.md
│   └── adr/
│       └── 0042-passkey-credential-ownership.md
│
└── .engineering/
    ├── protocol.yaml
    └── artifacts.yaml
```

while `engineering-protocols` provides the meaning of those artifacts.

A harness can then ask deterministic questions such as:

```text
What specification governs this task?
Which design satisfies that specification?
Has the current design revision been approved?
Does this architectural change require an ADR?
Which ADR records the relevant decision?
Is the implementation allowed to begin?
Which artifact or evidence is missing?
```

Those questions should be answerable from typed domain objects and protocol rules rather than from ad-hoc prompt interpretation.

That is the purpose of this extension.
