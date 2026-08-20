# ESS Semantic Diff, Impact Analysis & Evolution Planning - Design v0.1

> **Repository:** `codewandler/engineering-protocols`  
> **Status:** Proposed follow-on / cross-cutting design  
> **Audience:** Implementors extending ESS from single-revision specification and realization into semantic change analysis, proposal evaluation, and governed system evolution  
> **Relationship to existing work:** Additive. Semantic diff can be introduced as soon as `EssIr` is stable. Realization-aware evolution planning depends on later synthesis/realization work, but does not require waiting for every advanced verification wave.

---

## 1. Purpose

The current ESS/AEP architecture is primarily organized around one authoritative specification revision:

```text
ESS revision
    -> normalized EssIr
    -> deterministic projections / conformance
    -> synthesis plan
    -> implementation obligations
    -> realization
    -> release / operation
```

That answers an important greenfield question:

> Given one ESS revision, what system should exist, what can be generated, what must be implemented, and how do we independently prove that the result conforms?

The next major capability should begin with a simpler and more broadly useful question:

> What semantically changed between two valid ESS revisions, and what does that change affect?

That capability is useful **before** a project becomes brownfield. It supports:

- review of proposed ESS changes;
- pull-request impact summaries;
- architecture and design review;
- selective invalidation of generated artifacts and evidence;
- verification-scope estimation;
- policy gates for breaking or security-sensitive changes;
- comparison of multiple LLM-proposed improvements;
- constrained architecture search;
- change-budget enforcement;
- later, migration and rollout planning for existing realizations.

The design therefore separates two layers:

```text
Layer A - semantic change analysis

EssIr A + EssIr B
        -> EssDelta
        -> ImpactReport
        -> optional policy assessment

Layer B - brownfield evolution

EssDelta + current Realization + target plan/capabilities
        -> EvolutionPlan
        -> explicit evolution obligations / refusals
        -> governed AOP transition
```

The key rule remains the same as the rest of the project:

> **Never guess. Record a deterministic fact, produce an explicit unknown/obligation, or refuse.**

---

## 2. Core Thesis

An LLM may propose a future system state. It must not be the authority on the consequences of that proposal.

The intended architecture is:

```text
human / agent objective
        |
        v
LLM proposes candidate ESS
        |
        v
ESS compiler
        |
        +-- invalid -> deterministic diagnostics -> revise
        |
        v
candidate EssIr
        |
        v
SemanticDiffEngine
        |
        v
EssDelta
        |
        v
ImpactAnalyzer
        |
        +-- semantic blast radius
        +-- compatibility facts/questions
        +-- affected scenarios/evidence
        +-- affected generated artifacts/obligations
        +-- later: realization/migration effects
        |
        v
versioned policy / constraints
        |
        +-- accepted
        +-- rejected with reasons
        +-- advisory risk/churn classification
        |
        v
human / agent revises candidate
```

This turns the LLM into a **search heuristic over possible future specifications** while deterministic machinery remains responsible for:

- validity;
- semantic difference;
- dependency closure;
- compatibility facts that are actually derivable;
- provenance;
- policy enforcement;
- later, evolution planning and conformance.

---

## 3. Why Semantic Diff Should Exist Before Brownfield Evolution

Semantic diff is not merely an implementation detail of migration planning.

It is valuable as soon as two ESS revisions can be compiled.

For example, a greenfield system may not have a production database or deployed clients yet, but a proposed change can still affect:

```text
command contracts
outcome semantics
events
views
state machines
invariants
bindings
actor permissions
component ownership
topology requirements
conformance scenarios
projection artifacts
implementation obligations
```

A code/text diff can show that lines changed. It cannot reliably answer:

```text
Did the accepted input domain narrow?
Did a command gain a possible failure?
Did an event contract change?
Did a view consistency guarantee weaken?
Did a state transition disappear?
Did an actor gain authority?
Which binding is now affected transitively?
Which conformance scenarios are no longer evidence for the candidate revision?
```

Those are questions about the resolved semantic graph, not source text.

Therefore semantic diff should be treated as an independent ESS capability, with brownfield evolution as a later consumer.

---

## 4. Architectural Placement

The diff engine should compare **compiled semantic IR**, not source YAML and not generated OpenAPI/AsyncAPI.

```text
ESS source A                 ESS source B
    |                            |
    v                            v
validate + compile          validate + compile
    |                            |
    v                            v
 EssIr A                      EssIr B
      \                        /
       \                      /
        v                    v
          SemanticDiffEngine
                  |
                  v
               EssDelta
                  |
                  v
             ImpactAnalyzer
                  |
                  v
             ImpactReport
```

This follows the existing ESS rule:

> Strong typed semantic model first; technology-specific projections second.

A semantic diff must not be a JSON Patch over canonical IR serialization. Canonical serialization is useful for identity/digests and reproducibility, but a correct diff needs typed knowledge of which fields are semantic, which orders matter, and what kind of change occurred.

---

## 5. Inputs and Preconditions

The initial diff contract should be narrow:

```rust
pub fn diff(
    before: &EssIr,
    after: &EssIr,
) -> Result<EssDelta, DiffRefusal>;
```

Initial preconditions:

1. Both specifications independently validate and compile.
2. They describe the same logical system identity.
3. The diff implementation understands both IR format versions.
4. No handle minted by one `EssIr` is used as a lookup key in the other.

A comparison between unrelated systems is a different feature and should not be smuggled into revision diffing.

Illustrative refusal:

```text
DIFF_REFUSED_DIFFERENT_SYSTEM
before: billing
 after: ordering
```

---

## 6. Identity Rules

The existing ESS identity model already provides the correct foundation.

`QualifiedName` is the stable logical identity of a specification concept. Wire and display names are separate because they have different consequences.

Therefore v1 semantic diff should use these rules:

```text
same qualified identity
    -> compare semantics

qualified identity only in before
    -> Removed

qualified identity only in after
    -> Added

wire name changed
    -> WireNameChanged

display name changed
    -> DisplayNameChanged
```

The engine must **not** heuristically infer renames from similarity.

Bad:

```text
InvoiceCreated removed
InvoiceIssued added
names look similar
=> probably rename
```

Correct:

```text
InvoiceCreated removed
InvoiceIssued added
```

If a future evolution workflow needs to claim continuity across two logical identities, that claim must be explicit and auditable. The diff engine itself should not guess it.

The same principle applies to field, outcome, state, component and binding identities within their owning semantic scope.

---

## 7. Semantic Diff Is Not Text Diff

A semantic diff should intentionally erase source-only noise.

Examples that should not become semantic changes merely because source text changed:

- comments;
- whitespace;
- file splitting or file movement;
- YAML mapping order where the semantic model treats members as keyed sets;
- equivalent source spellings normalized by the parser/compiler.

Conversely, small textual changes may be major semantic changes:

```text
amount > 0
```

becoming:

```text
amount >= 100
```

or:

```text
consistency: read_your_writes
```

becoming:

```text
consistency: eventual
```

The diff therefore compares typed IR structures using element-specific comparators.

---

## 8. Comparison Semantics and Order

The diff engine must not assume every `Vec` in the IR has the same meaning.

For each IR structure, the comparator must define whether ordering is:

```text
semantic
presentation-only
or irrelevant
```

Examples:

- ordered event emission from one outcome may be semantic;
- map members keyed by qualified identity are not source-order changes;
- field declaration order may affect generated presentation but not necessarily the ESS behavioral contract;
- lifecycle transition identity and endpoints matter, not the order in which transitions were written.

If a reorder affects only a projection/presentation, it may be represented as a lower-severity projection/presentation change rather than a behavioral change.

This rule prevents canonical IR serialization from accidentally becoming the semantic definition.

---

## 9. `EssDelta`

`EssDelta` is a deterministic, serializable first-class artifact describing the semantic difference between two ESS revisions.

Illustrative shape:

```rust
pub struct EssDelta {
    pub format: DeltaFormatVersion,
    pub before: EssRevisionRef,
    pub after: EssRevisionRef,
    pub changes: Vec<SemanticChange>,
}
```

Where:

```rust
pub struct EssRevisionRef {
    pub system: QualifiedName,
    pub version: Version,
    pub digest: Digest,
}
```

The delta is not itself a migration plan and does not contain guessed implementation effort.

It answers:

```text
What changed semantically?
```

not:

```text
How do we deploy it?
How many engineer-days will it take?
Will production fail?
```

---

## 10. Typed Change Model

Avoid a generic `path + before JSON + after JSON` change representation as the canonical model.

Prefer typed changes:

```rust
pub enum SemanticChange {
    System(SystemChange),
    Domain(DomainChange),
    Type(TypeChange),
    Entity(EntityChange),
    Command(CommandChange),
    Event(EventChange),
    Error(ErrorChange),
    View(ViewChange),
    Actor(ActorChange),
    Component(ComponentChange),
    Binding(BindingChange),
    Conversion(ConversionChange),
    Topology(TopologyChange),
}
```

Typed changes make downstream reasoning explicit and exhaustive.

A new semantic feature added to ESS should force the diff engine to consciously decide how it is compared rather than silently appearing as an untyped JSON change.

---

## 11. Common Change Envelope

Each typed change should carry common provenance.

Illustrative shape:

```rust
pub struct ChangeEnvelope<T> {
    pub id: ChangeId,
    pub subject: EssSemanticRef,
    pub kind: ChangeKind,
    pub detail: T,
}
```

Possible `ChangeKind` values:

```rust
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}
```

More specific semantics live in the typed detail.

`ChangeId` should be deterministic from canonical change content rather than generated from a clock or RNG.

---

## 12. Type Changes

Initial type change taxonomy should cover at least:

```text
NamedTypeAdded
NamedTypeRemoved
TypeBodyChanged
StructFieldAdded
StructFieldRemoved
StructFieldTypeChanged
StructFieldOptionalityChanged
EnumVariantAdded
EnumVariantRemoved
UnionVariantAdded
UnionVariantRemoved
WireNameChanged
DisplayNameChanged
```

Where a relation is mechanically knowable, record it.

Example:

```text
Optional<T> -> T
```

can be represented as a requiredness strengthening / accepted-shape narrowing fact.

But do not generalize that into universal wire compatibility without a compatibility profile.

---

## 13. Entity and Lifecycle Changes

Entity changes should include:

```text
EntityAdded / Removed
IdentityFieldChanged
ObservableFieldAdded / Removed / Changed
InvariantAdded / Removed / Changed
StateAdded / Removed
InitialStateChanged
TransitionAdded / Removed
TransitionSourceChanged
TransitionTargetChanged
TransitionTriggerChanged
```

Some relations are directly knowable:

```text
transition removed
    -> lifecycle behavior narrowed

transition added
    -> lifecycle behavior expanded

invariant added
    -> allowed state space is constrained further

invariant removed
    -> allowed state space is relaxed
```

For a modified invariant predicate, v1 should usually report:

```text
InvariantPredicateChanged
relation: Unknown
```

unless a later proof engine can establish implication/equivalence.

---

## 14. Command and Outcome Changes

Command changes should include:

```text
CommandAdded / Removed
InputFieldAdded / Removed / Changed
OutcomeAdded / Removed
OutcomeConditionChanged
OutcomeErrorChanged
OutcomeEmittedEventsChanged
OutcomeEmissionOrderChanged
WireNameChanged
```

The current `EssIr` already preserves outcome branch semantics and test strategy. The diff must compare those resolved semantics rather than flattening outcomes into one possible-event set.

Important facts include:

```text
possible outcome set expanded
possible outcome set narrowed
accepted input shape changed
external-outcome control semantics changed
emitted semantic facts changed
```

A condition change should not automatically be called stronger or weaker unless the relation is derivable.

---

## 15. Event and Error Changes

Events and declared errors are contracts, not incidental structs.

Changes should include:

```text
EventAdded / Removed
EventFieldAdded / Removed / Changed
EventWireNameChanged
ErrorAdded / Removed
ErrorPayloadChanged
```

The semantic diff records the ESS-level contract change.

Whether an event-field addition is backward-compatible for a deployed consumer depends on the projection/encoding and consumer contract. That conclusion belongs to a compatibility assessment with an explicit profile, not to the bare delta.

---

## 16. View Changes

Views should include:

```text
ViewAdded / Removed
ViewSourceChanged
ProjectedFieldAdded / Removed / Changed
FilterChanged
ConsistencyChanged
WireNameChanged
```

Consistency changes can often be classified semantically.

For example:

```text
read-your-writes -> eventual
```

is a weakened observation guarantee.

The reverse is a strengthened guarantee, although it may increase implementation/runtime requirements.

That distinction is useful because a change can improve client semantics while increasing implementation churn.

---

## 17. Actor and Authority Changes

Actor permissions must be visible in semantic change review.

Changes include:

```text
ActorAdded / Removed
GrantAdded
GrantRemoved
```

Useful direct facts:

```text
grant added
    -> authority expanded

grant removed
    -> authority reduced
```

This should be a first-class change dimension because security risk is not well represented by ordinary API compatibility language.

A policy may choose to require elevated review whenever authority expands.

---

## 18. Binding and Interaction Changes

Bindings connect semantic facts across components and therefore create large transitive blast radii.

Diffs should cover:

```text
BindingAdded / Removed
SourceEventChanged
TargetCommandChanged
FieldMappingAdded / Removed / Changed
DeliverySemanticsChanged
FailurePolicyChanged
```

A mapping change should preserve typed provenance such as:

```text
InvoiceCreated.customer_email
    -> SendEmail.recipient
```

so impact analysis can explain exactly which upstream and downstream contracts are connected.

---

## 19. Component and Topology Changes

Component changes should include ownership and surface changes:

```text
ComponentAdded / Removed
OwnedDomainChanged
AcceptedCommandChanged
PublishedEventChanged
PortChanged
```

Topology changes should include:

```text
WorkloadAdded / Removed
ReplicaRequirementChanged
StatefulnessChanged
ResourceDependencyAdded / Removed / Changed
TransportRequirementChanged
```

These are semantically relevant even before a concrete platform is selected, but operational compatibility conclusions may require a target/platform capability model.

---

## 20. Conversions

Declared conversions are particularly important for both synthesis and change analysis.

Changes include:

```text
ConversionAdded
ConversionRemoved
ConversionContractChanged
```

Removing a conversion may indirectly make a previously synthesizable binding or implementation path impossible.

That effect is not visible from a local file diff but should appear through dependency closure.

---

## 21. Semantic Relations Are a Partial Order, Not a Guessing Contest

The diff engine should distinguish:

```rust
pub enum SemanticRelation {
    Equivalent,
    Expanded,
    Narrowed,
    Strengthened,
    Weakened,
    Changed,
    Unknown,
}
```

Not every change kind supports every relation.

Examples that may be mechanically classified:

```text
permission grant added      -> Expanded
permission grant removed    -> Narrowed
transition added            -> Expanded
transition removed          -> Narrowed
consistency guarantee raised -> Strengthened
consistency guarantee lowered -> Weakened
```

Examples that commonly remain `Unknown` in v1:

```text
arbitrary predicate A -> predicate B
complex invariant A -> invariant B
business rule body changes
```

Later property testing, model checking, SMT/theorem machinery, or formal verification may refine `Unknown` into a proven relation.

This lets semantic diff become useful early and grow stronger as W10-W12 mature.

---

## 22. `SemanticDependencyGraph`

`EssDelta` answers what changed directly. `ImpactAnalyzer` answers what those changes can affect.

Construct a graph from `EssIr` with typed edges such as:

```text
type --used-by--> field
entity --projected-by--> view
entity --governed-by--> invariant
command --has-input--> type
command --has-outcome--> outcome
outcome --emits--> event
actor --may-invoke--> command
component --accepts--> command
component --publishes--> event
event --triggers-via-binding--> command
binding --maps-field--> field
topology --runs--> component
conversion --permits--> type crossing
```

The graph may contain logical helper nodes where useful, but every path must retain semantic provenance.

---

## 23. Impact Closure

For each direct change, calculate deterministic reachability over relevant before/after dependency graphs.

The output should distinguish:

```text
DirectlyChanged
DirectlyDependent
TransitivelyImpacted
PotentiallyImpacted
Unaffected
```

Do not call every reachable node "changed."

Example:

```text
Event field type changed
    |
    +-> event directly changed
    |
    +-> binding directly dependent
    |
    +-> target command mapping transitively impacted
    |
    +-> owning components transitively impacted
```

This distinction prevents blast-radius reports from overstating actual modifications while still surfacing review scope.

---

## 24. Explainable Impact Paths

Every impact should be explainable by at least one path.

Illustrative shape:

```rust
pub struct ImpactPath {
    pub change: ChangeId,
    pub target: EssSemanticRef,
    pub edges: Vec<DependencyEdge>,
}
```

Example human rendering:

```text
billing.invoice.InvoiceCreated.customer_email changed
  -> binding billing.notify-on-invoice-created reads that field
  -> binding constructs billing.email.SendEmail.recipient
  -> email-component accepts SendEmail
```

The system should never emit only:

```text
email-component risk = high
```

without showing the semantic reason.

---

## 25. `ImpactReport`

Illustrative artifact:

```rust
pub struct ImpactReport {
    pub delta: DeltaRef,
    pub direct_impacts: Vec<Impact>,
    pub transitive_impacts: Vec<Impact>,
    pub compatibility: Vec<CompatibilityAssessment>,
    pub invalidations: Vec<Invalidation>,
    pub metrics: ImpactMetrics,
}
```

The report remains deterministic given:

```text
before EssIr
after EssIr
known provenance manifests
compatibility profile (when used)
analysis version
```

---

## 26. Deterministic Churn Facts

The engine can provide useful churn measures without pretending to know engineering effort.

Examples:

```text
semantic_changes_total
semantic_elements_directly_changed
semantic_elements_transitively_impacted
components_impacted
public_contracts_changed
actor_grants_changed
state_machine_changes
binding_changes
topology_changes
```

As more project artifacts exist, add:

```text
conformance_scenarios_potentially_affected
generated_artifacts_invalidated
implementation_obligations_invalidated
implementation_obligations_new
evidence_records_invalidated
realization_assets_affected
migration_obligations_required
```

These are facts/counts, not a universal effort estimate.

---

## 27. Do Not Put a Universal Risk Score in the Semantic Core

The semantic engine should not emit:

```text
risk = 7.4 / 10
estimated effort = 6.3 engineer-days
```

as authoritative semantic facts.

Those numbers depend on organization, technology, team experience, production context, change frequency, controls, and historical calibration.

Instead use three layers:

```text
1. deterministic impact facts
2. versioned organizational policy classification
3. optional empirical/probabilistic estimator
```

Layer 1 is authoritative semantic machinery.

Layer 2 may deterministically classify facts, for example:

```text
public event removed -> Critical
actor authority expanded -> High + security review
view consistency weakened -> High
only display text changed -> Low
```

Layer 3 may estimate effort or incident likelihood, but must be explicitly advisory and carry model/version/calibration provenance.

---

## 28. `ChangePolicy`

A versioned policy interprets impact facts for one organization or project.

Illustrative concept:

```yaml
policy:
  id: engineering.change-policy/v3

  deny:
    - public_event_removed_without_deprecation
    - actor_authority_expansion_without_security_review

  classify:
    - when: migration_obligations > 0
      risk: high

    - when: impacted_components > 5
      risk: high

    - when: only_display_changes
      risk: low
```

The same `EssDelta` can therefore receive different policy outcomes in different environments without changing semantic truth.

---

## 29. Compatibility Is Directional and Contextual

Avoid a single Boolean:

```text
compatible = true/false
```

Compatibility is directional.

Typical questions include:

```text
Can an old caller invoke the new command contract?
Can a new caller invoke the old command contract?
Can an old consumer process new events?
Can a new consumer process historical events?
Can old and new component versions coexist during rollout?
Can a new realization read old persisted state?
```

Schema ecosystems commonly distinguish backward and forward compatibility for exactly this reason.

ESS should generalize this idea across semantic surfaces rather than limiting it to message schemas.

---

## 30. `CompatibilityProfile`

Some compatibility results depend on assumptions external to the bare ESS semantic model.

For example, whether an added event field breaks an old consumer depends on the serialization/projection and consumer behavior.

Therefore compatibility analysis should accept an explicit profile:

```rust
pub struct CompatibilityProfile {
    pub id: CompatibilityProfileId,
    pub surfaces: SurfaceCompatibilityRules,
}
```

A profile might state:

```text
JSON clients ignore unknown response fields
command inputs reject unknown fields
event consumers tolerate additive optional fields
public wire-name changes are breaking
mixed-version rollout is required
```

No profile means the engine may return `Unknown` for questions that require those assumptions.

---

## 31. Compatibility Dimensions

Useful dimensions include:

```text
semantic behavior
command/caller contract
event producer/consumer contract
view/query contract
state/data compatibility
security/authority compatibility
deployment coexistence
wire/projection compatibility
```

A change may be compatible in one dimension and breaking in another.

Example:

```text
view consistency: eventual -> read_your_writes

client semantic guarantee: strengthened
implementation requirement: increased
wire shape: unchanged
operational cost: potentially increased
```

A single "breaking" label loses too much information.

---

## 32. Evidence and Scenario Invalidation

After closed-loop conformance exists, each scenario already has semantic provenance back to ESS concepts.

`EssDelta` can use that provenance to determine which scenarios are **potentially affected** by a candidate change.

Conceptually:

```text
SemanticChange
    -> source semantic refs
    -> scenario provenance intersection
    -> affected scenario set
```

This supports fast proposal feedback and targeted development verification.

Important rule:

> Selective impact analysis does not replace the final full ESS conformance gate.

It estimates what must be reconsidered. The authoritative final conformance claim still comes from the canonical full suite.

---

## 33. Generated Artifact and Obligation Invalidation

Once structural synthesis emits a manifest relating semantic elements to generated artifacts and implementation obligations, impact analysis becomes much richer.

```text
SemanticChange
      |
      +-> generated artifact provenance
      |       -> regenerate
      |
      +-> obligation source refs / contract digest
              -> unchanged / invalidated / disappeared / new
```

This turns the existing `contract_digest` concept into a change-analysis primitive.

For each obligation, the system can answer:

```text
still valid
must be reimplemented
must be reverified
no longer required
newly required
```

without estimating from source-code line counts.

---

## 34. Artifact Graph Integration

Eventually model change-analysis outputs as addressable AEP artifacts.

Possible artifact kinds:

```text
EssDelta
ImpactReport
PolicyAssessment
EvolutionPlan
EvolutionObligation
```

Possible relations:

```text
ESS v4 --compared-against--> ESS v3
EssDelta --describes-change--> ESS v3..v4
ImpactReport --analyzes--> EssDelta
ImpactReport --affects--> ImplementationObligation
ImpactReport --invalidates--> Evidence
ChangeProposal --evaluated-by--> ImpactReport
EvolutionPlan --plans-transition--> Realization v3 -> Realization v4
```

This preserves provenance from proposal through implementation and release.

---

## 35. Change Proposals as First-Class Inputs

A proposal can remain conceptually separate from the candidate ESS itself.

Illustrative shape:

```yaml
proposal:
  id: proposal:billing-fraud-controls
  base: billing/v3

  objective:
    improve: fraud-prevention

  constraints:
    - no_public_wire_break
    - no_topology_change
    - max_impacted_components: 3
```

The proposal describes intent and constraints.

The candidate ESS describes the proposed future semantic system.

This separation prevents product/design intent from being encoded as diff-engine policy.

---

## 36. LLM Proposal Evaluation Loop

The engine can support agentic architecture search without trusting the agent's impact claims.

```text
objective + constraints + base ESS
              |
              v
          LLM candidate
              |
              v
           compile
        /             \
   invalid             valid
      |                  |
 diagnostics             v
      |              EssDelta
      |                  |
      |              ImpactReport
      |                  |
      |              policy gate
      |             /          \
      +---------- revise      acceptable
```

The LLM may say:

> Candidate B preserves external contracts while achieving the objective.

But the engine independently determines whether that statement is true under the selected policy/profile.

---

## 37. Change Budgets and Proposal Constraints

A user should eventually be able to ask:

```text
Improve fraud detection while:
- making no public event or command wire break;
- touching at most three components;
- introducing no topology change;
- invalidating at most two implementation obligations.
```

Represent these constraints explicitly rather than leaving them only in a prompt.

Illustrative rules:

```rust
pub enum ProposalConstraint {
    ForbidPublicWireBreak,
    ForbidTopologyChange,
    MaxImpactedComponents(u32),
    MaxInvalidatedObligations(u32),
    ForbidAuthorityExpansion,
}
```

The evaluator returns structured violations that become direct repair context for an LLM or human architect.

---

## 38. Multi-Candidate Architecture Search

Multiple valid candidates can be compared using deterministic impact vectors.

Example:

| Candidate | Public contract changes | Components impacted | Obligations invalidated | Topology change |
|---|---:|---:|---:|---|
| A | 3 | 7 | 5 | yes |
| B | 0 | 3 | 2 | no |
| C | 1 | 2 | 0 | no |

The engine should not declare the "best" candidate without a declared objective/policy.

Instead it can expose a Pareto-like frontier:

```text
B: lower compatibility cost than A
C: lower implementation churn than B, but one public contract change
```

Product/business value remains an external objective unless modeled explicitly.

---

## 39. Semantic Diff as Review Output

A pull request changing ESS could include an automatically generated review summary:

```text
ESS Semantic Change Summary

Direct changes:
  1 command outcome condition changed
  1 event payload changed
  1 actor grant added

Blast radius:
  2 domains
  3 components
  1 binding

Verification impact:
  7 scenarios potentially affected

Authority:
  expanded: billing.support -> RefundInvoice

Compatibility:
  public event shape changed
  old-consumer compatibility: unknown under current profile

Policy:
  HIGH - security review required
```

This is more useful than saying "42 YAML lines changed."

---

## 40. Example - Behavioral Change Without Shape Change

Suppose:

```text
before:
CreateInvoice.accepted when amount > 0
```

becomes:

```text
after:
CreateInvoice.accepted when amount >= 100
```

A schema diff may show nothing.

`EssDelta` should show:

```text
CommandOutcomeConditionChanged
subject: billing.invoice.CreateInvoice.accepted
relation: Unknown (v1 predicate reasoner)
```

Impact analysis may find:

```text
CreateInvoice scenarios
InvoiceCreated emission expectations
views updated after successful creation
bindings triggered by InvoiceCreated
```

Even before brownfield migration exists, this is high-value review information.

---

## 41. Example - Wire Rename vs Logical Rename

### Wire rename

```text
logical identity:
  billing.invoice.InvoiceCreated

wire:
  invoices.created.v1
    -> invoices.created.v2
```

Delta:

```text
EventWireNameChanged
```

The event remains the same semantic identity.

### Logical rename

```text
billing.invoice.InvoiceCreated
    -> billing.invoice.InvoiceIssued
```

Delta:

```text
EventRemoved: InvoiceCreated
EventAdded: InvoiceIssued
```

No heuristic rename inference.

This distinction is already encoded in the ESS identity model and should remain load-bearing.

---

## 42. Example - LLM-Proposed Improvement

Assume an agent proposes replacing a general `Email` value at a notification boundary with `VerifiedEmail` and adding a verification requirement.

A correct workflow is not:

```text
LLM: this is a small safe refactor
```

It is:

```text
candidate ESS
   -> compile
   -> semantic delta
   -> impact closure
```

Possible deterministic output:

```text
VerifiedEmail type added
InvoiceCreated field type changed
SendEmail recipient contract changed
binding mapping affected
conversion requirement changed
2 components impacted
3 conformance scenarios affected
1 implementation obligation contract invalidated
```

A compatibility profile or policy can then determine whether the public/wire effects are acceptable.

The LLM receives the actual constraints and may propose a lower-churn alternative.

---

## 43. Brownfield Extension - Why `EssDelta` Is Necessary but Not Sufficient

Once a production `Realization` exists, a semantic diff tells us what the target specification changed, but not how to transform the existing running system.

Two realizations can both be independently conformant while the transition between them is unsafe.

```text
Realization R3 --?--> Realization R4
```

Therefore brownfield work adds a new planning layer.

---

## 44. `EvolutionPlanner`

Conceptually:

```text
before EssIr
      +
after EssIr
      |
      v
   EssDelta
      +
current Realization
      +
target SynthesisPlan
      +
target/platform capabilities
      +
compatibility/rollout requirements
      |
      v
EvolutionPlanner
      |
      v
EvolutionPlan
```

The planner answers:

> Given what exists and what is now required, which transition steps are mechanically derivable, which require explicit work/decisions, and which cannot safely satisfy the requested constraints?

---

## 45. Evolution Dispositions

Evolution needs a slightly richer algebra than greenfield synthesis because existing assets may be reusable.

Possible dispositions:

```rust
pub enum EvolutionDisposition {
    Unchanged,
    Reuse,
    Regenerate,
    Reverify,
    Obligation(EvolutionObligation),
    Refused(EvolutionRefusal),
}
```

Examples:

```text
unchanged generated type
    -> Reuse or deterministic Regenerate

changed generated API
    -> Regenerate

custom implementation contract unchanged but dependency closure touched
    -> Reverify

custom implementation contract digest changed
    -> EvolutionObligation / reimplementation work

legacy data requires unknown mapping
    -> Migration obligation

required mixed-version guarantee impossible on target
    -> Refused
```

---

## 46. `EvolutionObligation`

An evolution obligation means:

> The desired target and transition requirement are known, but the transition step cannot be safely derived from ESS + realization + target capabilities.

Possible kinds:

```rust
pub enum EvolutionObligationKind {
    DataMigration,
    HistoricalEventMigration,
    ExternalSystemChange,
    ClientTransition,
    RolloutCoordination,
    RollbackStrategy,
    ManualSemanticDecision,
}
```

These are not generic project tasks. They are derived residual transition requirements with provenance.

---

## 47. Migration Obligations

A migration obligation is the most obvious brownfield case.

Example:

```text
before:
Invoice.amount: Decimal

after:
Invoice.amount: Money { amount, currency }
```

Generating the new `Money` type may be deterministic.

Migrating old rows may not be:

```text
What currency should historical values use?
```

Correct result:

```text
Generated:
  new semantic type / adapters that are fully specified

EvolutionObligation(DataMigration):
  determine and transform historical currency semantics

Refused:
  if policy demands a lossless automatic migration and no derivation exists
```

Never infer a plausible currency from deployment geography, naming, or LLM intuition.

---

## 48. Compatibility Windows and Mixed Versions

Brownfield evolution introduces intermediate system states.

A safe transition may require:

```text
old producer + old consumer
new producer + old consumer
old producer + new consumer
new producer + new consumer
```

to coexist for some window.

The EvolutionPlan should therefore state required compatibility windows rather than merely checking the endpoints.

This is especially important for:

- events;
- rolling process upgrades;
- database schema transitions;
- dual-read/dual-write patterns;
- client/API deprecation windows.

---

## 49. Transition Invariants

Endpoint conformance is insufficient.

An evolution plan may need transition invariants such as:

```text
no invoice is lost during dual-write
all accepted commands produce exactly one permitted semantic outcome
old consumers remain supported until cutover marker X
new data is written in a form readable by both revisions during phase 2
rollback remains possible before irreversible step Y
```

Where ESS/AOP can express these semantics, they should become verifiable properties rather than prose checklist items.

---

## 50. `EvolutionPlan`

Illustrative shape:

```rust
pub struct EvolutionPlan {
    pub format: EvolutionPlanVersion,
    pub from: RealizationRef,
    pub target_spec: EssRevisionRef,
    pub delta: DeltaRef,
    pub actions: Vec<EvolutionAction>,
    pub obligations: Vec<EvolutionObligation>,
    pub refusals: Vec<EvolutionRefusal>,
    pub compatibility_windows: Vec<CompatibilityWindow>,
    pub required_evidence: Vec<EvidenceRequirement>,
}
```

The plan is inspectable and read-only.

Creating a plan must not mutate production.

This is analogous in spirit to an execution-plan workflow: first calculate and review the intended transition, then apply it under a separately governed protocol.

---

## 51. Release Becomes a Transition Between Realizations

The existing design already moves toward releases deploying a `Realization` rather than an arbitrary SHA.

Brownfield evolution makes the release relationship explicit:

```text
CurrentRealization
        |
        v
   EvolutionPlan
        |
        v
TargetRealization
```

A release may therefore require evidence such as:

```text
current realization conformed to previous ESS
candidate realization conforms to target ESS
all mandatory evolution obligations are satisfied
required compatibility windows are verified
transition invariants pass
rollback claims are supported by evidence
```

AOP governs execution of this transition.

---

## 52. Evolution Conformance

Later introduce an independent transition oracle.

It should prove more than:

```text
R4 conforms to ESS v4
```

It should test claimed properties of:

```text
R3 -> R4
```

Possible evidence:

```text
migration preserves required invariants
historical data remains interpretable
mixed-version compatibility claims hold
required events are not lost
roll-forward reaches a conformant target realization
rollback returns to a valid realization where rollback is promised
```

As elsewhere, the planner/agent must not produce its own authoritative evidence.

---

## 53. Role of AEP / ADP / AOP

The division becomes clean:

```text
ESS
    defines current and target system semantics

SemanticDiffEngine
    states what changed

ImpactAnalyzer
    states what the change reaches / invalidates

ChangePolicy
    governs proposal acceptance and review requirements

EvolutionPlanner
    determines how a known realization may transition

ADP
    governs implementation of residual change/evolution obligations

Conformance runners
    independently verify candidate and transition properties

AOP
    governs the actual runtime transition
```

This extends rather than replaces the existing protocol architecture.

---

## 54. Risk-Driven Evidence Selection

The structural-synthesis design already anticipates stronger evidence for higher-risk obligations.

Semantic impact facts provide a deterministic basis for selecting that evidence.

For example:

```text
simple additive mapping change
    -> typecheck + targeted conformance

parser/transform migration
    -> property testing

lifecycle transition change
    -> model checking

critical authorization/invariant change
    -> selective formal verification
```

The policy chooses the evidence strength. The impact engine supplies the facts.

---

## 55. Progressive Strengthening from W10-W12

Semantic diff should not wait for advanced verification.

Instead advanced verification should improve it over time.

### Initial diff

```text
predicate changed
relation: Unknown
```

### With property-based analysis

```text
counterexample found:
old accepts X, new rejects X
```

### With model checking / formal implication

```text
new invariant implies old invariant
relation: Strengthened
```

Thus W10-W12 become providers of stronger semantic-relation proofs, not prerequisites for the whole change-analysis capability.

---

## 56. Suggested Delivery Tracks

This refinement changes the earlier idea of placing all evolution work after W12.

Semantic diff can start much earlier.

### Track SD - Semantic Change Analysis

Can begin once `EssIr` is considered stable enough to compare.

```text
SD1  Typed EssDelta core
SD2  SemanticDependencyGraph + impact closure
SD3  Human + canonical JSON reports
SD4  Versioned ChangePolicy / proposal constraints
SD5  LLM multi-candidate evaluation loop
```

These are useful without a deployed realization.

### Track VI - Verification / Implementation Impact Enrichment

As current waves land:

```text
VI1  Map delta -> ConformanceSuite scenario provenance
VI2  Map delta -> synthesis manifest / generated artifacts
VI3  Map delta -> ImplementationObligation contract invalidation
VI4  Map delta -> evidence invalidation
```

### Track EV - Realization Evolution

Begins when `Realization` and release/AOP inputs are sufficiently concrete.

```text
EV1  Realization-aware ImpactReport
EV2  EvolutionPlan
EV3  EvolutionObligation / migration obligations
EV4  Compatibility windows + transition invariants
EV5  Evolution conformance
EV6  AOP-governed apply / rollback transition
```

W10-W12 may proceed in parallel and progressively strengthen SD/EV proofs.

---

## 57. Recommended Timing Relative to Current Waves

Do not interrupt the closed-loop conformance wave merely to implement semantic diff.

But there is no architectural reason to wait until all greenfield synthesis/formal-verification work is complete either.

A practical sequence is:

```text
current W4 conformance
        |
        +----------------------+
        |                      |
        v                      v
W5/W6 synthesis + agent     SD1/SD2 semantic diff
        |                      |
        +----------+-----------+
                   v
          richer impact mapping
                   |
                  W7
             Realization
                   |
                   v
            EV1/EV2 evolution
```

Advanced verification remains an enrichment track:

```text
W10/W11/W12
    -> stronger change-relation proofs
    -> stronger transition evidence
```

---

## 58. CLI / API Surface

Illustrative CLI only:

```text
ess diff --from billing-v3/ --to billing-v4/
ess impact --from billing-v3/ --to billing-v4/
ess evaluate-change --base billing-v3/ --candidate candidate/ --policy change-policy.yaml
```

Useful output modes:

```text
human
json
```

Machine-readable artifact formats:

```text
ess.delta/v1
ess.impact-report/v1
ess.policy-assessment/v1
ess.evolution-plan/v1
```

The exact crate/CLI ownership should follow repository pressure rather than be predetermined here.

---

## 59. Determinism

Given identical:

```text
before EssIr
after EssIr
analysis version
compatibility profile
known provenance manifests
```

outputs must be byte-identical.

Rules:

- deterministic maps/sets;
- deterministic change ordering;
- deterministic impact path ordering;
- no timestamps in canonical artifacts;
- no RNG;
- no LLM calls inside the diff or impact engine;
- canonical serialization;
- trailing newline.

An LLM may consume the report. It is never part of producing the authoritative report.

---

## 60. Stable Change Ordering

A canonical ordering can be defined by:

```text
semantic category
subject identity
change subtype
nested member identity
```

For example:

```text
system
domain
type
entity
command
event
error
view
actor
component
binding
conversion
topology
```

The order is an artifact-format contract, not an accident of hash iteration.

---

## 61. Versioning and Provenance

Every report should identify:

```text
before ESS version + digest
after ESS version + digest
compiler version
diff format version
diff engine version
impact analysis version
compatibility profile ID/version (if used)
change policy ID/version (for policy assessment)
```

Optional estimators must additionally carry their own model/calibration identity.

---

## 62. Security and Trust Boundaries

Proposal evaluation should be read-only by default.

An agent allowed to propose candidate ESS changes should not automatically gain permission to:

- replace the baseline ESS;
- change the policy used to evaluate itself;
- approve its own authority expansion;
- invalidate or create evidence;
- apply an EvolutionPlan to a runtime.

Those remain AEP/AOP capability and approval decisions.

This is especially important when an LLM is performing architecture search.

---

## 63. Failure and Unknown Handling

Expected non-success results should be typed.

Examples:

```text
DiffRefusal::DifferentSystem
DiffRefusal::UnsupportedIrVersion

Compatibility::Unknown(MissingProfileAssumption)
SemanticRelation::Unknown(RequiresPredicateReasoning)

ImpactUnknown::MissingProvenanceManifest
EvolutionRefusal::TargetCannotSatisfyMixedVersionRequirement
```

Do not collapse these into one string error or silently downgrade them to warnings.

---

## 64. Non-Goals for Semantic Diff v1

Do not attempt initially to:

- infer logical renames by similarity;
- compare unrelated systems as though they were revisions;
- estimate engineer-days from semantic change counts;
- predict production incidents from first principles;
- solve arbitrary predicate implication;
- inspect database tables or source-code ASTs as the semantic authority;
- replace canonical conformance with selective reruns;
- derive migration values from convention or LLM intuition;
- automatically apply changes to production.

---

## 65. Non-Goals for Evolution v1

Do not initially require:

- a universal database migration framework;
- Kubernetes-specific rollout logic;
- every external API/client ecosystem to be modeled;
- general distributed transaction synthesis;
- automatic recovery from arbitrary partial failures;
- full formal proof of every transition.

Prove the architecture with one bounded reference evolution first.

---

## 66. Reference Evolution Fixture

After the billing realization exists, introduce one intentionally small revision pair.

Example characteristics:

```text
billing v3 -> billing v4
```

Include at least:

- one additive type/field change;
- one behavior/predicate change;
- one event or view compatibility question;
- one implementation-obligation contract change;
- one deterministic regeneration;
- one migration obligation that cannot be guessed;
- one mixed-version compatibility requirement;
- one deliberate incompatible candidate that must be refused by policy/evolution planning.

Keep the fixture small enough that expected impact paths can be manually audited.

---

## 67. Semantic Diff Acceptance Criteria

The semantic-diff layer is successful when:

- two valid revisions of the same system produce a deterministic `EssDelta`;
- identical semantic IR produces an empty delta;
- source-only noise does not appear as semantic change;
- qualified-identity changes appear as remove/add, never heuristic rename;
- wire/display changes are distinct from logical identity changes;
- command outcome branch changes remain branch-specific;
- lifecycle, authority, view consistency, binding and topology changes are typed;
- unknown predicate relations remain `Unknown` rather than guessed;
- canonical output is byte-identical across runs;
- every change has provenance to the before/after ESS concepts.

---

## 68. Impact Analysis Acceptance Criteria

Impact analysis is successful when:

- direct and transitive impacts are distinct;
- every transitive impact has an explainable dependency path;
- impact closure uses both before and after graphs where required;
- affected components/contracts can be identified without source-code scanning;
- scenario provenance can be mapped once ConformanceSuite exists;
- synthesis/obligation provenance can be mapped once manifests exist;
- the report emits deterministic churn facts without claiming universal effort;
- compatibility returns typed `Unknown` where required assumptions are absent.

---

## 69. Proposal Evaluation Acceptance Criteria

Proposal evaluation is successful when:

- a candidate ESS must compile before evaluation;
- a proposal cannot modify its own policy invisibly;
- explicit constraints are deterministically checked against impact facts;
- violations are structured and attributable to semantic changes;
- multiple candidates can be compared by the same metrics/profile;
- the LLM remains a candidate generator rather than the acceptance oracle.

---

## 70. Evolution Acceptance Criteria

Brownfield evolution is successful when:

- a current conformant `Realization` and target ESS produce an inspectable `EvolutionPlan`;
- unchanged assets are explicitly reusable rather than treated as new work;
- deterministic regeneration is separated from residual transition work;
- underdetermined migration/coordination becomes an explicit `EvolutionObligation`;
- impossible requested transition semantics produce refusal rather than degradation;
- compatibility windows are explicit where mixed revisions must coexist;
- target realization conformance remains independent;
- claimed transition properties are independently verifiable;
- AOP applies an approved plan rather than an agent mutating runtime state directly.

---

## 71. Falsifiability

The change-analysis system itself must be tested against deliberate faults.

Examples:

```text
ignore an event-field type change
    -> expected diff test fails

classify a removed actor grant as expansion
    -> relation test fails

omit one dependency edge from event -> binding
    -> impact closure test fails

heuristically merge remove+add into rename
    -> identity test fails

emit non-deterministic change ordering
    -> byte comparison fails

claim compatibility without required profile assumption
    -> compatibility test fails
```

Later evolution faults should include:

```text
reuse implementation despite changed contract digest
skip required data migration
claim rollback after irreversible step
ignore mixed-version incompatibility
```

---

## 72. Relationship to Existing Design Principles

This design is a direct extension of existing principles rather than a new philosophy.

### Semantic concepts remain primary

Diff `EssIr`, not OpenAPI/Kafka/Rust files.

### Deterministic machinery handles what is known

Change classification and dependency closure are deterministic.

### Under-specification is explicit

Unknown semantic relations and migration decisions remain explicit.

### Agents solve residual/search problems

Agents propose candidate specifications and resolve obligations.

### Independent verification owns correctness claims

Agents do not certify their own candidate or migration.

### Provenance remains first-class

Every impact, invalidation and obligation points back to semantic sources.

---

## 73. Architectural End State

The broader lifecycle eventually becomes:

```text
                    objective / problem
                           |
                           v
                    proposed ESS change
                           |
                           v
                      candidate EssIr
                           |
                 +---------+---------+
                 |                   |
                 v                   v
             EssDelta          policy constraints
                 |
                 v
            ImpactReport
                 |
          acceptable change?
           /             \
         no               yes
         |                 |
         v                 v
       revise       synthesis / obligations
                           |
                           v
                    target Realization
                           |
                    current Realization
                           |          |
                           +----+-----+
                                v
                         EvolutionPlan
                                |
                     transition conformance
                                |
                                v
                               AOP
                                |
                                v
                         running system
                                |
                                v
                      observations / incidents
                                |
                                +----> next objective/change
```

This closes the lifecycle from specification to realization and then from one valid realization to the next.

---

## 74. Most Important Design Boundary

The architecture should preserve one final distinction:

```text
LLM / human
    proposes what might be better

ESS compiler
    decides whether the proposal is valid

Semantic diff
    states what changed

Impact analysis
    states what the change reaches

Policy
    decides what the organization permits / requires

Optional estimator
    predicts effort/risk, explicitly non-authoritatively

Evolution planner
    determines how an existing realization may transition

Conformance
    decides whether correctness claims are supported

AOP
    governs execution of the approved transition
```

Do not collapse these roles into one "AI change risk" function.

---

## 75. Recommended Next Design/Implementation Decision

The immediate design decision is not whether to start brownfield migration machinery.

It is whether to make **semantic change analysis** a permanent first-class layer now.

Recommendation:

> **Yes. Introduce `EssDelta` as a standalone deterministic artifact over pairs of `EssIr`, design `ImpactReport` on top of it, and let later conformance/synthesis/realization artifacts progressively enrich the report. Build `EvolutionPlan` only once a concrete `Realization` exists.**

This creates value throughout the remaining greenfield waves and avoids having to invent semantic diff under migration pressure later.

---

## 76. Proposed Roadmap Summary

```text
NOW / when scheduling permits

SD1  EssDelta
SD2  dependency graph + ImpactReport
SD3  human/JSON reporting
SD4  ChangePolicy + constraints
SD5  LLM candidate evaluation

AS W4-W6 LAND

VI1  scenario impact
VI2  generated-artifact impact
VI3  obligation invalidation
VI4  evidence invalidation

AFTER REALIZATION EXISTS

EV1  realization-aware impact
EV2  EvolutionPlan
EV3  migration/evolution obligations
EV4  compatibility windows + transition invariants
EV5  evolution conformance
EV6  AOP transition execution

IN PARALLEL / LATER

W10  property-based semantic relation evidence
W11  model-checking relation/transition evidence
W12  selective formal proofs
```

The important sequencing change is:

> **Semantic diff is not W13. It is a cross-cutting capability that can begin much earlier. Brownfield evolution is the later layer that consumes it.**

---

## 77. References and Design Influences

### Repository sources

- `crates/ess-domain/src/name.rs` - logical identity vs wire/display naming; qualified identity is explicitly what a diff compares.
- `crates/ess-compiler/src/ir.rs` - normalized resolved IR, deterministic collections, canonical serialization, resolved outcome/view decisions.
- `docs/design/ess-closed-loop-execution-conformance-design-v0.1.md` - canonical scenario IR, semantic provenance, independent conformance.
- `docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md` - synthesis plan, obligations, contract digests, realizations, release/AOP bridge, later assurance waves.
- Engineering Protocols / ESS session handoff - structural synthesis, obligation identity, realization identity and roadmap context.

### Non-normative external influences

- Terraform plan: previewing an intended transition before applying it.  
  https://developer.hashicorp.com/terraform/cli/commands/plan
- Confluent Schema Registry compatibility concepts: directional backward/forward/full compatibility.  
  https://docs.confluent.io/platform/current/schema-registry/fundamentals/schema-evolution.html
- Protocol Buffers schema evolution: explicit compatibility rules for evolving message contracts.  
  https://protobuf.dev/programming-guides/proto3/
- `cargo-semver-checks`: typed compatibility lints and witness-oriented explanations rather than raw textual API diffs.  
  https://docs.rs/crate/cargo-semver-checks/latest

These are conceptual influences only. ESS semantic change analysis is broader than infrastructure plans, wire-schema compatibility, or programming-language API SemVer because it includes behavior, lifecycle, authority, consistency, interactions and realization evolution.

---

## 78. Final Thesis

The greenfield architecture asks:

```text
What system should exist, and does this realization satisfy it?
```

Semantic change analysis adds:

```text
What changed, and what does that change affect?
```

Brownfield evolution then asks:

```text
Given what exists, how can we safely reach what should exist next?
```

Together:

```text
specify
  -> diff
  -> understand impact
  -> choose / govern change
  -> synthesize residual work
  -> verify
  -> realize
  -> evolve
  -> verify transition
  -> operate
  -> repeat
```

This is the natural extension of the project's central rule:

> **Keep probabilistic proposal generation outside the trusted semantic core; make every authoritative change, impact, compatibility and conformance claim deterministic, explainable, provenance-rich, and falsifiable wherever the specification contains enough information to do so.**
