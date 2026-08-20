# Semantic Infrastructure Discovery, Specification, Conformance & Multi-Cloud Realization - Design v0.1

> **Repository:** `codewandler/engineering-protocols`  
> **Status:** Proposed follow-on / cross-cutting design  
> **Audience:** Implementors extending the repository from software-system semantics into infrastructure discovery, infrastructure specification, conformance, portability, and governed infrastructure evolution  
> **Relationship to existing work:** Additive, but it refines the planned ESS topology-synthesis boundary. It must not disrupt the current Wave 4/5 gating sequence.  
> **Naming:** This document deliberately uses `InfraSpec`, `InfraIr`, and related names rather than introducing an `ISS` protocol/specification acronym in v0.1. The architectural boundary should prove itself before a new top-level acronym is made normative.

---

## 1. Purpose

The repository currently has two primary semantic halves:

```text
AEP / ADP / AOP
    defines HOW engineering work is governed

ESS
    defines WHAT software system must exist
```

ESS is intentionally technology-independent. Its topology layer already begins to express runtime requirements such as workloads, replica minima, external dependencies, data services, and event delivery. Later design work proposes lowering those requirements through a platform target into Kubernetes or other deployment artifacts.

A second problem now becomes visible:

> What if the infrastructure already exists, is spread across cloud accounts and Kubernetes clusters, and is only partially documented or specified?

And, once infrastructure can be specified semantically:

> Can the same infrastructure intent be tested, documented, refactored, migrated, or realized on a different provider without reducing the problem to file-format translation?

This design introduces a provider-neutral infrastructure semantic layer with two independent entry paths:

```text
BROWNFIELD / DISCOVERY

AWS + Kubernetes + telemetry + IaC
        |
        v
ObservedInfraSnapshot
        |
        v
analysis / documentation / conformance
        |
        v
candidate intent
        |
        v
reviewed InfraSpec


GREENFIELD / SYNTHESIS

ESS runtime requirements + authored InfraSpec
        |
        v
InfraIr
        |
        v
InfraSynthesisPlan
        |
        +--> AWS target
        +--> Azure target
        +--> GCP target
        +--> Kubernetes target
```

The same semantic machinery then enables:

- infrastructure inventory and architecture recovery;
- service/dependency and trust-graph discovery;
- static and runtime infrastructure conformance;
- invariant testing;
- architecture documentation and diagrams;
- semantic drift detection;
- pre-change impact and blast-radius analysis;
- security posture and attack-path reasoning;
- resilience, availability, capacity, and disaster-recovery checks;
- environment parity analysis;
- infrastructure refactoring;
- cloud-provider migration;
- multi-cloud realizability and portability analysis;
- candidate IaC generation for unmanaged infrastructure;
- agent-assisted architecture search under deterministic constraints;
- AOP-governed infrastructure transitions.

The central rule remains the same as the rest of the project:

> **Do not confuse what is specified, what is statically derived, and what has actually been observed.**

And the synthesis rule remains:

> **Never guess. Generate it, create an explicit obligation, record an unknown, or refuse.**

---

## 2. Executive Design Decisions

This design makes the following primary decisions.

### 2.1 Do not make observed infrastructure the specification

A scanner produces evidence about reality. It does not recover intent perfectly.

```text
ObservedInfraSnapshot != InfraSpec
```

Promotion from observation into normative intent must be explicit and reviewable.

### 2.2 Do not use one IR for uncertain observations and normative intent

Observation is partial, time-bounded, possibly conflicting, and evidence-backed. Normative specification is validated intent.

Use separate models:

```text
ProviderObservation*
        -> ObservedInfraSnapshot

InfraSpec source
        -> InfraIr
```

### 2.3 Keep ESS topology and infrastructure semantics separate but linked

ESS should continue to describe the software system and the runtime capabilities that system requires.

The infrastructure layer should describe the provider-neutral platform resources and constraints that satisfy those requirements.

```text
ESS topology
    -> InfrastructureRequirementSet

InfraSpec + InfraTarget
    -> concrete infrastructure realization
```

### 2.4 Reframe planned W8 topology synthesis

The existing W8 intent is correct: ESS topology requirements should eventually produce deployment artifacts.

The refinement is that ESS itself should not become the owner of a large provider/Kubernetes ontology.

Recommended future split:

```text
W8a
ESS topology -> InfrastructureRequirementSet

Infra track
InfrastructureRequirementSet + InfraSpec + InfraTarget
    -> InfraSynthesisPlan
    -> provider/platform projections
```

### 2.5 Treat topology, identity, security, communication, and availability as graph semantics

The most useful output is not an inventory table. It is a typed semantic graph with provenance.

### 2.6 Discovery must combine multiple evidence planes

Cloud inventory alone cannot prove actual network/data flows. Runtime telemetry alone cannot prove intended policy or all dormant dependencies.

The model must support configuration, declared IaC, runtime, identity/security, and historical evidence.

### 2.7 Unknown and conflicting evidence are first-class

A fact that cannot be established is not a pass.

```text
Pass
Fail
Unknown
Conflict
NotApplicable
```

### 2.8 Provider portability is capability satisfaction, not service-name mapping

AWS RDS is not semantically equivalent to an Azure resource merely because both vendors call something a managed database.

A provider target must demonstrate that it satisfies the required semantic capabilities.

### 2.9 Scoring is policy, not infrastructure truth

The semantic core emits deterministic facts and invariant results. A separately versioned policy may classify or score them.

### 2.10 Migration safety is a separate conformance problem

Two individually conformant infrastructure realizations do not prove that the transition between them is safe.

### 2.11 Agents propose and repair; deterministic machinery establishes facts

The project remains deliberately not an LLM orchestration framework. Agents may propose InfraSpec changes, mappings, invariants, remediations, and migration plans. Independent scanners, compilers, planners, and verifiers remain authoritative for facts and acceptance.

---

## 3. Why This Fits the Repository

The proposal is not a separate infrastructure product bolted onto the repository. It follows existing architectural commitments.

The repository already states that:

- AEP governs engineering work while ESS defines the software target;
- semantic concepts are primary and transports/deployment formats are projections;
- AEP can support infrastructure changes, migrations, compliance, disaster recovery, and capacity management as profiles over common primitives;
- AOP covers telemetry, blast-radius control, reversible change, health verification, rollback, preparation, and migration;
- ESS topology is intentionally narrow today and is expected to grow later;
- topology synthesis is intentionally deferred until semantic realization independence is demonstrated;
- synthesis should classify requirements as Generated, Obligation, or Refused;
- a `Realization` records one concrete implementation choice and is eventually what release/AOP should operate on.

This infrastructure layer therefore extends existing ideas rather than replacing them.

Conceptually:

```text
                    PRODUCT / SYSTEM INTENT
                              |
                              v
                             ESS
                              |
                 software runtime requirements
                              |
                              v
               InfrastructureRequirementSet
                              |
                              v
                         InfraSpec
                              |
                              v
                           InfraIr
                              |
                   InfraSynthesisPlanner
                              |
               +--------------+--------------+
               |              |              |
           Generated       Obligations     Refusals
               |              |              |
               +--------------+--------------+
                              |
                              v
                      InfraRealization
                              |
                      InfraConformance
                              |
                              v
                            Evidence
                              |
                    AEP / ADP / AOP gates
```

Brownfield discovery provides another path into the same model:

```text
running cloud / clusters
        |
        v
ObservedInfraSnapshot
        |
        +--> documentation / graph queries
        +--> invariant analysis
        +--> drift against InfraSpec
        +--> candidate InfraSpec extraction
        +--> incident / blast-radius context
```

---

## 4. Use-Case Exploration

The following use cases were used to stress-test the design boundary. They are grouped by what semantic capability they require rather than by cloud product.

### 4.1 Brownfield inventory and architecture recovery

Scan a large existing environment and answer:

```text
What exists?
Where is it?
Who owns it?
What is public?
What depends on what?
Which resources are shared?
Which resources appear orphaned?
Which clusters/accounts/subscriptions/projects form one environment?
```

The output should be a navigable semantic graph, not a vendor resource dump.

### 4.2 Automatic current-state architecture documentation

Generate:

- resource inventories;
- network/topology diagrams;
- service maps;
- identity/trust diagrams;
- data-service maps;
- ingress/egress maps;
- environment summaries;
- provider-specific appendix material;
- provenance and freshness annotations.

Documentation is a projection from the observed or normative graph, not a separately maintained source.

### 4.3 Static service-dependency analysis

Derive potential communication/dependency edges from:

- Kubernetes Services, Endpoints, Gateway/Ingress objects;
- environment/config references;
- security groups/network ACLs;
- routing tables;
- managed-service bindings;
- IAM resource references;
- DNS records;
- load-balancer target groups;
- declared IaC relationships.

These are **possible/declared/permitted** relationships, not automatically observed traffic.

### 4.4 Runtime dependency discovery

Observe actual communication through sources such as:

- Hubble/eBPF service flows;
- VPC flow logs;
- service-mesh telemetry;
- OpenTelemetry service graphs;
- load-balancer/access logs;
- DNS observations;
- cloud audit/activity logs.

This produces time-bounded facts such as:

```text
billing-api communicated_with postgres:5432
observation_window = 24h
```

It must not be interpreted as proof that no unobserved dependency exists outside the window.

### 4.5 Desired-vs-observed infrastructure conformance

Given a normative InfraSpec:

```text
InfraSpec + ObservedInfraSnapshot
        -> InfraConformanceReport
```

Questions include:

- is every required resource/capability present?;
- is any forbidden exposure present?;
- do observed security properties satisfy requirements?;
- does runtime behavior violate declared communication constraints?;
- are availability/failure-domain requirements met?;
- is the deployed provider configuration a valid realization of the spec?

### 4.6 Continuous semantic drift detection

Compare observations over time:

```text
snapshot(t1) + snapshot(t2)
        -> ObservedInfraDelta
```

Examples:

```text
public exposure introduced
IAM authority widened
replica/failure-domain coverage reduced
new runtime dependency observed
encryption property changed
network path became reachable
cluster version changed
managed service moved region
```

This is much more useful than a raw JSON/YAML diff.

### 4.7 Declared-IaC vs observed-reality drift

Treat Terraform/OpenTofu, CloudFormation, Kubernetes manifests, Helm/Kustomize, or GitOps state as a separate evidence plane.

```text
declared IaC
      +
observed provider state
      -> drift facts
```

This finds manual changes and unmanaged resources without declaring either side automatically normative.

### 4.8 Environment parity analysis

Compare semantic infrastructure between:

```text
staging <-> production
region A <-> region B
cluster A <-> cluster B
```

Rather than demanding textual equality, ask whether relevant capability and invariant sets are equivalent.

### 4.9 Security posture and trust graph

Build a graph combining:

- workload identities;
- Kubernetes ServiceAccounts and RBAC;
- cloud IAM roles/policies;
- secret references;
- KMS/key relationships;
- certificates;
- network boundaries;
- public ingress;
- permitted and observed communication.

Then answer questions such as:

```text
Which principals can ultimately reach customer-data storage?
Which workloads can assume production roles?
Where can an internet-originating path reach a privileged workload?
Which identities have wildcard or cross-environment access?
```

### 4.10 Attack-path and privilege-chain analysis

The graph can support bounded path queries:

```text
Internet
  -> public ingress
  -> workload
  -> service account
  -> cloud role
  -> secret store
  -> database
```

The engine should report the path and evidence, not make claims beyond what the modeled permission semantics can establish.

### 4.11 Infrastructure invariant testing

Examples:

```text
No production database may be internet reachable.

Every public application endpoint must terminate authenticated TLS.

Production workloads must span at least two failure domains.

Every persistent data service must have encryption at rest enabled.

Every production namespace must have an explicit network-isolation policy.

A workload may communicate only with declared dependencies.

Secrets must be referenced, never embedded in workload configuration.
```

### 4.12 Resilience and availability analysis

Model and verify:

- region/AZ placement;
- replica minima;
- quorum/failure-domain relationships;
- single points of failure;
- load-balancer redundancy;
- database HA settings;
- backup requirements;
- recovery-point/recovery-time requirements where specified;
- dependency failure domains.

### 4.13 Blast-radius analysis

Given a resource, failure domain, or proposed change:

```text
failure / change
      -> typed dependency closure
      -> affected workloads/data paths/identities/ESS realizations
```

This can feed both design review and AOP incident/change workflows.

### 4.14 Disaster-recovery validation

Verify that the declared recovery story has real resources and evidence:

- backups exist;
- restore has been exercised;
- secondary region capabilities exist;
- DNS/cutover paths are available;
- secrets/keys are recoverable;
- dependent services have compatible recovery ordering.

DR claims should be evidence-backed rather than documentation-only.

### 4.15 Controlled decommissioning

Before deleting a resource or service, establish:

- no required inbound semantic edges;
- no observed inbound edges within a declared observation window;
- no unresolved identity aliases;
- no dependent recovery/runbook assumptions;
- rollback/recreation feasibility where required.

Absence of observed traffic alone is insufficient proof of safe deletion.

### 4.16 Capacity and headroom analysis

The semantic layer can describe required capacity properties and observed utilization facts without becoming a metrics system.

Examples:

- replica count below policy minimum;
- resource requests/limits inconsistent with policy;
- node-pool headroom below threshold;
- database storage near declared capacity boundary;
- one failure domain cannot absorb another's load.

### 4.17 FinOps and ownership analysis

Add provider-neutral ownership/cost-allocation relationships:

```text
resource -> environment -> product/team/cost-center
```

The core can emit deterministic facts such as unowned resources, duplicated capabilities, idle resources supported by evidence, or cross-environment sharing. A FinOps policy/estimator can translate those facts into cost recommendations.

### 4.18 Data residency and sovereignty

Express requirements such as:

```text
customer-eu data must remain in approved EU regions
backups must remain within the same residency boundary
keys must be controlled in approved jurisdictions
```

The infrastructure graph can then prove or fail placement and replication relationships where provider evidence is sufficient.

### 4.19 Platform dependency and upgrade impact

Track provider/platform versions and capabilities:

- Kubernetes API versions;
- CRDs/operators/controllers;
- ingress/gateway implementations;
- CNI/CSI dependencies;
- managed database versions;
- deprecated cloud resources/APIs.

A proposed upgrade can be evaluated as a semantic delta and capability change rather than merely a package bump.

### 4.20 Policy projection without replacing policy engines

Infrastructure invariants may be projected into enforcement/checking technologies such as OPA, Gatekeeper, Kyverno, admission controls, cloud policy services, or CI linters.

These are projections/verifiers.

The repository should not become a replacement for those systems.

### 4.21 Incident-response context

During an incident, produce a bounded context closure:

```text
symptomatic workload
  + dependency graph
  + recent infra deltas
  + shared failure domains
  + identities
  + rollout history
  + relevant runbooks
```

This can improve agent/human diagnosis while AOP continues to govern permissions, hypotheses, mitigations, verification, and rollback.

### 4.22 Pre-change infrastructure impact analysis

A proposed InfraSpec change can produce:

- semantic change facts;
- impact closure;
- security-sensitive changes;
- expected regenerated resources;
- provider capability shortfalls;
- obligations;
- expected conformance reruns;
- rollback constraints.

This mirrors ESS semantic-diff proposal evaluation.

### 4.23 Infrastructure refactoring within one provider

Examples:

```text
EC2 -> EKS
self-managed PostgreSQL -> managed PostgreSQL
public services -> private + gateway
single VPC -> segmented VPCs
one cluster -> workload-specific clusters
self-managed Kafka -> managed event service
```

The target architecture can change substantially while the normative capability requirements remain stable.

### 4.24 Cloud-provider migration

Given:

```text
InfraSpec
AWS InfraRealization
Azure target capabilities
```

produce a provider-migration plan containing:

```text
Generated mappings
Migration obligations
Compatibility/transition requirements
Refusals
```

No 1:1 AWS-to-Azure service mapping is assumed.

### 4.25 Multi-cloud realizability

One InfraSpec can be evaluated against multiple targets:

```text
              InfraSpec
            /     |      \
           v      v       v
        AWS     Azure    GCP
```

Each target produces its own synthesis plan.

This proves whether the specification is actually portable rather than merely written without vendor names.

### 4.26 Portability-profile design constraints

Define a target set:

```text
AWS production
Azure production
GCP production
```

and require:

```text
all targets realizable
no semantic weakening
provider-specific obligations <= budget
no unsupported required capability
```

An LLM or human architecture proposal can then be rejected deterministically if it violates the portability profile.

### 4.27 Acquisition / estate onboarding

For a newly acquired or undocumented environment:

```text
scan -> normalize -> graph -> classify -> candidate InfraSpec
```

This accelerates architecture archaeology while retaining uncertainty and provenance rather than pretending automatic discovery recovered design intent.

### 4.28 Candidate IaC recovery

For manually managed infrastructure, generate a candidate provider projection or IaC representation from a reviewed InfraSpec.

The flow should be:

```text
observation
  -> candidate intent
  -> human/agent review
  -> normative InfraSpec
  -> synthesis
```

Not:

```text
scan -> automatically declare everything intentional -> generate IaC
```

### 4.29 Compliance and audit evidence

Map selected controls into semantic invariants and evidence requirements.

Examples include encryption, public exposure, identity boundaries, logging requirements, backups, region restrictions, and separation of duties.

The resulting evidence can become AEP artifacts/evidence without turning the infrastructure model into a generic compliance framework.

### 4.30 Agent context engineering

For an infrastructure task, build a deterministic closure instead of dumping the entire estate:

```text
target resource/change
  -> dependencies
  -> security boundaries
  -> observed communication
  -> relevant invariants
  -> recent deltas
  -> target capabilities
  -> failing counterexamples
```

This gives an operations or migration agent small, relevant, reproducible context.

### 4.31 Chaos/fault-injection planning - later

Once failure domains and invariants are reliable, the graph can generate candidate fault experiments:

```text
remove one zone
block one dependency
revoke one identity
fail one broker/database replica
```

AOP would govern execution. The semantic layer defines what should remain true.

---

## 5. Review of the Initial Infrastructure Idea

The initial idea was broadly correct:

```text
scan AWS + Kubernetes
  -> map resources to IR
  -> test/validate/document
  -> discover connectivity/security
  -> formalize specification
  -> lower to manifests/provider resources
  -> test again
```

The review identified several places where that simple pipeline needs stronger boundaries.

### 5.1 One scanner cannot establish the whole architecture

Cloud control-plane APIs establish configuration. They do not necessarily establish actual runtime communication.

For example, AWS Config documents resource relationships but explicitly notes that those relationships do not include network-flow or data-flow dependencies and cannot represent application architecture. Runtime sources such as VPC Flow Logs or Hubble provide different evidence.

Therefore the design must be multi-plane.

### 5.2 A resource inventory is not a semantic graph

Provider inventory APIs naturally return provider-specific resources.

The useful model must distinguish:

```text
resource exists
resource contains another resource
resource may reach another resource
resource did reach another resource
resource requires another resource
resource is authorized to use another resource
```

Those are different typed relations.

### 5.3 Observation is epistemic, specification is normative

A production system may contain:

- deliberate architecture;
- temporary migration infrastructure;
- manual drift;
- abandoned resources;
- emergency incident changes;
- provider defaults;
- historical leftovers.

A scan cannot know which is intentional.

### 5.4 Cross-cloud portability cannot be implemented as service translation

A translation table such as:

```text
RDS -> Azure Database for PostgreSQL
ALB -> Azure Application Gateway
```

is useful implementation knowledge but not sufficient semantic proof.

The model must first state the requirement:

```text
managed PostgreSQL-compatible data service
private connectivity
encryption at rest
backup retention >= 30d
multi-failure-domain availability
```

Then each target demonstrates whether and how it satisfies those capabilities.

### 5.5 Infrastructure scoring must remain explainable

A single score such as `83/100` cannot be the primary semantic result.

The engine should first report facts and failed/unknown invariants. Scoring policy remains versioned and inspectable above that layer.

### 5.6 Transition correctness is not endpoint correctness

An AWS and Azure realization may independently satisfy the same InfraSpec while an attempted migration between them loses data, breaks identity, or violates availability.

Migration requires its own transition invariants and evidence.

### 5.7 ESS topology should not absorb the infrastructure estate

ESS topology already owns runtime requirements from the application's perspective.

Putting cloud accounts, IAM policies, VPC routes, Kubernetes operators, backup vaults, shared clusters, organization hierarchy, and multi-tenant platform resources directly into ESS would turn ESS into a universal ontology and make one software specification responsible for shared infrastructure it does not own.

A typed boundary is cleaner.

---

## 6. Terminology

This document uses the following vocabulary.

### `InfraSpec`

A normative, provider-neutral specification of infrastructure/platform requirements and intended infrastructure relationships.

### `InfraIr`

The validated, resolved semantic IR compiled from one InfraSpec revision.

### `ProviderObservation`

One raw or normalized fact emitted by a discovery adapter from a provider/platform/evidence source.

### `ObservedInfraSnapshot`

A time/scoped, provenance-bearing semantic representation of what has been observed about an infrastructure estate.

It may contain unknowns, conflicts, partial identity mappings, and time-bounded runtime observations.

### `InfrastructureRequirementSet`

A provider-neutral set of runtime/platform requirements emitted by ESS topology or authored by another consumer and submitted to the infrastructure layer for satisfaction.

### `InfraTarget`

A selected realization target, such as an AWS/EKS platform profile, Azure/AKS profile, GCP/GKE profile, raw Kubernetes platform, or other supported target.

### `InfraTargetCapabilities`

The machine-readable semantic capabilities and constraints of one InfraTarget.

### `InfraSynthesisPlan`

A deterministic plan classifying each normative requirement as Generated, Obligation, or Refused for one target.

### `InfrastructureObligation`

A required infrastructure behavior/decision/transition whose contract is known but cannot be safely derived automatically.

### `InfraRealization`

One concrete provider/platform realization of one InfraSpec and resolved requirement set using a specific target and obligation implementation set.

### `InfraConformanceReport`

Independent evidence describing whether a realization/observed environment satisfies the normative InfraSpec and linked requirements.

### `ObservedInfraDelta`

A semantic change set between two observed snapshots.

### `InfraSpecDelta`

A semantic change set between two normative InfraSpec revisions.

### `InfraDriftReport`

A comparison between normative infrastructure intent and observed infrastructure reality.

### `InfraEvolutionPlan`

A governed plan for moving from one known infrastructure realization to another, including transition obligations and invariants.

---

## 7. Architectural Placement

The complete conceptual architecture is:

```text
                             PRD / Architecture / ADR
                                      |
                                      v
                                     ESS
                                      |
                              runtime requirements
                                      |
                                      v
                       InfrastructureRequirementSet
                                      |
                                      +----------------------+
                                      |                      |
                                      v                      v
                                  InfraSpec          shared platform spec
                                      |
                                      v
                                    InfraIr
                                      |
                              InfraSynthesisPlanner
                                      |
                          Generated / Obligation / Refused
                                      |
                                      v
                               InfraRealization
                                      |
                               InfraConformance
                                      |
                                      v
                                   Evidence
                                      |
                             AEP / ADP / AOP


BROWNFIELD EVIDENCE PATH

cloud APIs + K8s + IaC + telemetry + identity + history
                         |
                         v
                 ProviderObservation*
                         |
                         v
                ObservedInfraSnapshot
                         |
          +--------------+---------------+
          |              |               |
          v              v               v
      analysis      documentation      drift/conformance
          |                              |
          +--------------+---------------+
                         |
                         v
              candidate spec / change
```

This layer should be reusable even when no ESS exists. That is important for brownfield estates, platform infrastructure, and shared services.

---

## 8. The ESS / InfraSpec Boundary

This is the most important design boundary in the document.

### 8.1 ESS owns application and software-system semantics

ESS should continue to own concepts such as:

- semantic domains;
- commands/events/views;
- component ownership;
- inter-component bindings;
- external software dependencies;
- workload identity at a logical level where needed;
- runtime requirements tied to the software system;
- minimum replicas / statelessness / required persistence/event delivery;
- externally exposed software surfaces;
- software-level health requirements.

Example:

```yaml
topology:
  workloads:
    invoice-service:
      replicas:
        min: 2
      stateless: true
      requires:
        - postgres: invoice-store
        - publish: invoice-events
```

### 8.2 InfraSpec owns platform/infrastructure semantics

InfraSpec should own concepts such as:

- organizations/accounts/subscriptions/projects;
- environments/regions/zones;
- networks/subnets/routing boundaries;
- clusters/namespaces/node pools;
- gateways/load balancers/DNS;
- provider-neutral managed data/messaging capabilities;
- shared platform services;
- infrastructure identity and effective authorization;
- network-security controls;
- secrets/key/certificate infrastructure;
- availability/failure-domain rules;
- backups/DR infrastructure;
- capacity/platform constraints;
- observability infrastructure requirements;
- provider realization choices.

### 8.3 The bridge is a requirement contract

Do not make ESS point directly at AWS or Kubernetes resources.

Instead:

```text
ESS topology
     |
     v
InfrastructureRequirementSet
     |
     v
Infra planner resolves requirements
against InfraSpec + target capabilities
```

Illustrative requirement:

```yaml
requirement:
  id: billing.invoice-store
  kind: relational-database
  engine: postgres

  connectivity:
    exposure: private

  availability:
    failure_domains:
      minimum: 2

  durability:
    backup:
      required: true

  security:
    encryption_at_rest: required
```

The infrastructure target may realize this through RDS, Azure Database for PostgreSQL, Cloud SQL, CloudNativePG, or refuse if it cannot honor the required semantics.

### 8.4 One InfraSpec can serve multiple ESS realizations

Shared clusters, networks, identity, observability, secret systems, gateways, and databases often outlive or host many application systems.

Therefore:

```text
ESS A ----\
          \
ESS B ------> InfrastructureRequirementSet* -> InfraSpec -> platform
          /
ESS C ----/
```

### 8.5 One ESS can resolve against multiple infrastructure targets

This preserves the current ESS goal of topology-independent semantic realizations.

---

## 9. Reframing Existing W8 Topology Synthesis

The existing design says:

```text
ESS topology requirements
        -> platform target
        -> Kubernetes projection
```

and deliberately places this after multiple software realizations are proven.

That sequencing remains correct.

The proposed refinement is architectural ownership:

```text
ESS topology requirements
        |
        v
InfrastructureRequirementSet
        |
        v
InfraSpec / InfraIr
        |
        v
InfraTargetCapabilities
        |
        v
InfraSynthesisPlan
        |
        v
Kubernetes / AWS / Azure / GCP projections
```

This does three things:

1. preserves ESS as a software-system specification;
2. creates a reusable infrastructure semantic layer for brownfield discovery and shared platforms;
3. makes multi-cloud and infrastructure evolution possible without adding provider semantics to ESS.

If infrastructure work is not ready when W8 is reached, W8 should at minimum introduce an internal requirement/target capability boundary compatible with this future split.

---

## 10. Observation Architecture

Infrastructure discovery should be adapter-based.

```text
AWS adapter
Azure adapter
GCP adapter
Kubernetes adapter
IaC adapter
Runtime-flow adapter
Identity adapter
History/change adapter
        |
        v
ProviderObservation*
        |
        v
normalization + identity correlation
        |
        v
ObservedInfraSnapshot
```

### 10.1 Discovery adapters are not planners

An adapter should answer:

```text
what did this source report?
when?
under what scope?
with what locator/schema/version?
```

It should not infer business intent.

### 10.2 Discovery should be read-only by default

Scanning production is an observation operation. Mutation belongs to a later explicitly governed phase.

### 10.3 Scopes are explicit

Examples:

```text
AWS organization/account/region
Azure tenant/subscription/resource group
GCP organization/folder/project
Kubernetes cluster/namespace
```

Partial scans must be represented as partial scope, not silently treated as the whole estate.

---

## 11. Evidence Planes

The infrastructure model should support several evidence planes.

### 11.1 Control-plane configuration

Examples:

- AWS Cloud Control / service APIs / Config;
- Azure Resource Graph / Resource Manager;
- GCP Cloud Asset Inventory / service APIs;
- Kubernetes API discovery and resource objects.

Strong for:

- existence;
- configured properties;
- declared attachments/relationships;
- provider identity/location;
- lifecycle/configuration history where available.

Weak for:

- actual runtime traffic;
- undocumented application-level dependencies;
- intent.

### 11.2 Declared infrastructure-as-code

Examples:

- Terraform/OpenTofu configuration/state/plan;
- CloudFormation;
- Kubernetes YAML;
- Helm/Kustomize/GitOps repositories;
- Crossplane resources.

Strong for:

- declared intended resources;
- provenance to repositories/modules;
- planned changes;
- ownership conventions.

Weak for:

- manual drift;
- provider defaults not modeled in source;
- actual runtime behavior.

### 11.3 Runtime network and service observations

Examples:

- Hubble/eBPF;
- VPC flow logs;
- service meshes;
- OpenTelemetry;
- gateway/LB/access logs;
- DNS telemetry.

Strong for:

- actual communication during a time window;
- protocol/port and sometimes L7 information;
- dependency confirmation.

Weak for:

- dormant/rare dependencies;
- intended but currently unused paths;
- full authorization semantics.

### 11.4 Identity and authorization evidence

Examples:

- cloud IAM policies;
- effective-role analysis;
- Kubernetes RBAC;
- workload identity bindings;
- secret/key access policies.

Strong for:

- allowed authority where semantics are modeled;
- trust relationships.

### 11.5 Historical/change evidence

Examples:

- provider resource history;
- audit/activity logs;
- IaC commit history;
- deployment history.

Strong for:

- what changed;
- provenance;
- incident/change correlation.

---

## 12. Observation Epistemics

Do not represent discovery confidence as an unexplained floating-point score in v1.

Use explicit epistemic categories.

Illustrative model:

```rust
pub enum ClaimStatus {
    Declared,
    Derived,
    Observed,
    Corroborated,
    Conflicting,
    Unknown,
}
```

A claim should retain evidence references:

```yaml
claim:
  subject: workload://prod/billing-api
  relation: communicates_with
  object: database://prod/customer-db

  value:
    protocol: tcp
    port: 5432

  status: observed

  evidence:
    source: aws-vpc-flow-logs
    window:
      start: 2026-08-20T08:00:00Z
      end: 2026-08-20T09:00:00Z
```

If a Hubble observation and declared service mapping corroborate the same relationship, the normalized claim may be `Corroborated` while retaining both evidence paths.

Conflicts must be visible.

---

## 13. Time and Freshness Are Semantic

Infrastructure observations age.

Every snapshot and time-varying claim should carry:

- observed/captured time;
- source freshness where known;
- observation window for telemetry;
- snapshot completeness/scope;
- source version/schema where relevant.

A conformance rule may require freshness:

```text
security-group state <= 15 minutes old
runtime-flow evidence covers >= 24 hours
backup restore evidence <= 90 days old
```

A stale observation can produce `Unknown` rather than a false pass.

---

## 14. Identity and Correlation

Identity correlation is likely one of the hardest parts of infrastructure discovery.

The same logical object may appear through:

- a Kubernetes object UID/name;
- a cloud load-balancer ARN/resource ID;
- a DNS record;
- an IaC address;
- a flow-log interface ID;
- a service-mesh identity;
- an ESS component/workload locator.

### 14.1 Preserve source-native locators

Never discard provider identity.

### 14.2 Correlation produces explicit mappings

Illustrative:

```yaml
identity_mapping:
  logical: infra://prod/billing/public-gateway

  locators:
    - kind: kubernetes
      value: gateway/prod/billing

    - kind: aws-arn
      value: arn:aws:elasticloadbalancing:...

    - kind: dns
      value: api.example.com
```

### 14.3 Ambiguous correlation is not silently merged

If two resources may represent the same semantic object but correlation is uncertain:

```text
UnresolvedIdentityCorrelation
```

should remain an explicit diagnostic/obligation.

### 14.4 Avoid LLM-only identity authority

An LLM may propose a correlation. Deterministic evidence or human approval should establish it.

---

## 15. Observed Resource Model

The provider-neutral vocabulary should start small and capability-oriented.

Potential v1 resource families:

```text
Scope
  Organization
  Account / Subscription / Project
  Environment
  Region
  FailureDomain

Network
  Network
  Subnet
  RouteBoundary
  FirewallBoundary
  Gateway
  LoadBalancer
  DnsZone / DnsName

Compute / Orchestration
  Cluster
  Namespace
  NodePool
  Workload
  Service
  Endpoint

Data / Messaging
  RelationalDatabase
  KeyValueStore
  ObjectStore
  Cache
  Queue
  Topic
  EventBroker

Identity / Security
  Principal
  WorkloadIdentity
  Role
  Policy
  SecretStore
  Key
  Certificate

Operations
  BackupStore
  TelemetrySink
  HealthEndpoint
```

The semantic model should resist mirroring every provider resource type.

Provider-specific details remain attached as realization/observation properties when necessary.

---

## 16. Relationship Model

Relationships are central.

Initial relationship vocabulary may include:

```text
contains
located_in
runs_on
scheduled_to
exposes
routes_to
resolves_to
fronts
backs
uses
requires
publishes_to
consumes_from
reads_from
writes_to
replicates_to
backed_up_to

communicates_with
permitted_to_communicate
denied_from_communicating

assumes_identity
authorized_by
grants_access_to
references_secret
encrypted_by
authenticated_by

realizes
satisfies
corresponds_to
```

Each relation should define:

- allowed endpoint kinds;
- directionality;
- semantic meaning;
- whether it is normative, derived, or observable;
- whether evidence can be time-bounded;
- whether transitive reasoning is valid.

Do not treat every edge as transitive.

---

## 17. Required, Permitted, and Observed Communication

These must remain separate.

```text
REQUIRED
ESS / InfraSpec says A must communicate with B.

PERMITTED
Network/security configuration allows A to communicate with B.

OBSERVED
Telemetry saw A communicate with B during a window.
```

This enables useful queries:

### Required but not permitted

Likely deployment/configuration defect.

### Required and permitted but never observed

May be normal, dormant, or suspicious. Not automatically a failure without a liveness expectation.

### Observed but not required

Potential undocumented dependency or unexpected communication.

### Permitted but neither required nor observed

Potentially excessive network authorization.

This three-way comparison should be a first-class analysis surface.

---

## 18. Protocol and Security Semantics

Communication edges may include:

```text
network protocol
application protocol when observed/declared
port
TLS requirement
mutual authentication requirement
identity expectation
certificate/key relationship
data classification where specified
```

Example normative edge:

```yaml
connection:
  from: workload://billing-api
  to: database://invoice-store

  protocol:
    application: postgres
    transport: tcp

  security:
    encryption_in_transit: required
    workload_identity: required

  exposure:
    private_only: true
```

An L4 flow source may prove TCP/port reachability but be insufficient to prove authenticated TLS. The result should remain partially known.

---

## 19. Effective Authorization Graph

Security analysis should distinguish:

```text
policy document
      -> grants
      -> role/principal
      -> assumption/binding
      -> workload
      -> effective authority
```

Do not stop at listing IAM/RBAC objects.

Useful semantic facts include:

```text
workload X may read secret Y
workload X may write bucket Z
service account A may bind/assume cloud role B
principal P may mutate production cluster resources
```

Provider-specific policy evaluators/adapters can establish facts that are then normalized into provider-neutral authority relations.

---

## 20. Secrets and Sensitive Data Boundary

Discovery must avoid ingesting secret values.

The semantic model should contain:

- secret metadata;
- references;
- storage/backing system;
- encryption/key metadata;
- access relationships;
- rotation metadata where safely available;

but never require raw secret contents for infrastructure understanding.

This should be an explicit security invariant of discovery adapters.

---

## 21. Normative `InfraSpec`

An InfraSpec describes intended capabilities and constraints rather than raw provider resources wherever possible.

Illustrative shape:

```yaml
spec:
  id: commerce-platform-prod
  version: 4

scopes:
  production:
    residency:
      regions:
        allowed: [eu-central, eu-west]

networks:
  application:
    exposure: private-by-default

platforms:
  primary-cluster:
    kind: kubernetes
    availability:
      failure_domains:
        minimum: 3

services:
  relational-data:
    engine: postgres
    availability:
      failure_domains:
        minimum: 2
    backups:
      retention_days:
        minimum: 30
    security:
      encryption_at_rest: required
      public_exposure: forbidden

identity:
  workload_identity: required

network_policy:
  default: deny
```

The syntax is illustrative. The semantic types matter more than YAML shape.

---

## 22. `InfraIr`

`InfraIr` is the resolved normative model.

It should:

- assign stable semantic identity;
- resolve references;
- normalize ordering;
- resolve typed relationships;
- resolve invariant subjects;
- expose deterministic graph queries;
- support canonical digests;
- contain no unresolved provider-specific interpretation.

Provider choices belong in target/realization data, not by default in the normative core.

---

## 23. Infrastructure Invariants

Infrastructure invariants should be typed semantic assertions over InfraIr and/or an observed target.

Examples:

```text
forall Database where environment == production:
    public_exposure == false

forall PersistentStore:
    encryption_at_rest == enabled

forall Workload where criticality >= high:
    failure_domain_count >= 2

forall ObservedCommunication:
    exists RequiredOrApprovedCommunication(edge)
```

Some invariants are purely spec-internal consistency checks. Others require observed evidence.

---

## 24. Static vs Dynamic Conformance

Separate two classes.

### 24.1 Static/configuration conformance

Uses current provider/Kubernetes/IaC state.

Examples:

- resource exists;
- region is allowed;
- encryption configured;
- replicas/failure domains configured;
- network policy exists;
- IAM binding does not exceed allowed scope.

### 24.2 Runtime conformance

Requires observations over time.

Examples:

- no undeclared service communication occurred during a window;
- health remained available during a failover test;
- no plaintext edge was observed where encryption is required;
- canary/transition SLOs held.

Do not make runtime absence claims stronger than the observation window supports.

---

## 25. `InfraConformanceSuite`

Use the same principle as ESS conformance: canonical semantics first, adapter second.

```text
InfraSpec / InfraIr
       |
       v
InfraConformanceSuite
       |
       v
InfraConformanceTarget
       |
       v
provider + cluster + telemetry adapters
       |
       v
InfraConformanceReport
```

A target may provide semantic queries such as:

```text
resolve resource
query configuration property
query topology relationship
query effective authority
query observed flows
query evidence freshness
```

It should not provide hidden methods that simply answer the desired invariant.

---

## 26. Conformance Result Model

An invariant/check should produce more than boolean.

```rust
pub enum InfraCheckStatus {
    Passed,
    Failed,
    Unknown,
    ConflictingEvidence,
    NotApplicable,
}
```

Each result should include:

- invariant/check ID;
- semantic subject(s);
- expected condition;
- observed/derived facts;
- evidence references;
- timestamps/freshness;
- counterexample path where applicable;
- verifier identity/version.

---

## 27. Counterexamples

Failures should preserve semantic provenance.

Example:

```yaml
counterexample:
  invariant: production-databases-not-public

  subject:
    database: customer-prod

  expected:
    public_exposure: false

  observed:
    public_endpoint: true
    source:
      provider: aws
      resource: arn:aws:rds:...

  path:
    - internet
    - public-route
    - security-boundary
    - database/customer-prod
```

This is useful for humans, policy, and agent repair.

---

## 28. Documentation and Graph Projections

From either a normative InfraIr or an observed snapshot, generate deterministic projections:

```text
inventory tables
architecture docs
network diagrams
service dependency maps
trust/identity maps
public exposure reports
data-service maps
failure-domain maps
provider appendix
security findings
semantic diffs
```

Observed projections must display evidence/freshness where it affects interpretation.

---

## 29. Semantic Infrastructure Diff

Infrastructure should reuse the semantic-diff architecture already proposed for ESS.

There are several distinct comparisons.

### 29.1 Observed snapshot delta

```text
ObservedInfraSnapshot A
          +
ObservedInfraSnapshot B
          -> ObservedInfraDelta
```

Answers: what changed in reality?

### 29.2 Normative specification delta

```text
InfraIr A + InfraIr B
        -> InfraSpecDelta
```

Answers: what infrastructure intent changed?

### 29.3 Desired-vs-observed drift

```text
InfraIr + ObservedInfraSnapshot
        -> InfraDriftReport
```

Answers: where does reality fail or differ from desired intent?

### 29.4 Realization delta

```text
InfraRealization A + InfraRealization B
        -> realization impact/evolution facts
```

Useful for migration and rollout planning.

---

## 30. Typed Infrastructure Changes

Potential semantic changes include:

```text
ResourceRequirementAdded / Removed
ProviderRealizationChanged
RegionChanged
FailureDomainRequirementChanged
PublicExposureChanged
RouteChanged
CommunicationPermissionChanged
ObservedCommunicationAdded / Removed
IdentityBindingChanged
EffectiveAuthorityChanged
EncryptionRequirementChanged
BackupRequirementChanged
ReplicaRequirementChanged
DataResidencyChanged
PlatformVersionChanged
ProviderCapabilityChanged
```

A change should carry impact paths and evidence/provenance rather than only old/new values.

---

## 31. Churn, Risk, and Infrastructure Scoring

Do not make one universal score part of InfraIr.

The semantic engine should emit facts such as:

```text
2 public exposure violations
1 wildcard effective privilege path
3 undeclared observed dependencies
0 unencrypted persistent stores
2 single-failure-domain critical workloads
4 resources with unresolved ownership
1 target portability refusal
```

A versioned policy can then classify:

```text
Security: high risk
Resilience: medium risk
Portability: low
Overall review requirement: architecture + security approval
```

A calibrated estimator may predict effort/cost probabilistically, but it is not authoritative semantic evidence.

---

## 32. Policy and Change Gates

Infrastructure policy may consume:

- InfraSpecDelta;
- ImpactReport;
- InfraConformanceReport;
- portability report;
- obligations/refusals;
- transition risk facts.

Example policy:

```yaml
change_policy:
  forbid:
    - production_database_public_exposure
    - unreviewed_cross_environment_identity_grant

  require_approval:
    when:
      - data_residency_changed
      - irreversible_migration_present
      - blast_radius.critical_services > 2
```

Policy belongs above deterministic facts.

---

## 33. Provider/Platform Target Model

A target must be typed and versioned.

Illustrative:

```yaml
target:
  id: aws-eks-production/v1

  provider:
    aws:
      regions: [eu-central-1, eu-west-1]

  orchestration:
    kubernetes:
      version: 1.34
      distribution: eks

  networking:
    ingress: alb
    cni: vpc-cni

  persistence:
    postgres:
      options: [rds, aurora]

  events:
    kafka:
      options: [msk]
```

This is target configuration. The semantic capability model sits below provider product names.

---

## 34. `InfraTargetCapabilities`

Planning should operate on semantic capabilities such as:

```text
supports relational postgres semantics
supports private endpoint
supports >= N failure domains
supports customer-managed encryption key
supports workload identity
supports L7 ingress with TLS
supports network-policy enforcement
supports backup retention >= X
supports required residency region
supports event delivery semantics
```

A capability may include constraints and evidence of which provider realization supplies it.

---

## 35. Infrastructure Synthesis Planning

Use the existing synthesis algebra.

```rust
pub enum InfraSynthesisDisposition {
    Generated(GeneratedInfraCapability),
    Obligation(InfrastructureObligation),
    Refused(InfraSynthesisRefusal),
}
```

For every required semantic capability, exactly one disposition exists.

### Generated

The target fully determines a compliant implementation.

### Obligation

The contract is known but a decision/action/implementation cannot be safely derived.

### Refused

The target cannot satisfy the required semantics without weakening them.

---

## 36. `InfrastructureObligation`

Illustrative obligation kinds:

```text
ProviderSpecificDecision
NetworkCutover
IdentityMapping
DataMigration
SecretTransfer
ExternalDnsChange
CertificateMigration
CapacityPlan
BackupRestoreValidation
ManualApproval
SharedPlatformCoordination
UnsupportedAutomaticTransformation
```

Example:

```yaml
obligation:
  id: migration/customer-db/currency-key
  kind: data-migration

  source:
    requirement: data/customer-db

  contract:
    preserve:
      - data_integrity
      - encryption
      - write_order

  verification:
    required:
      - target_database_conformance
      - row_count_match
      - application_read_validation
```

Infrastructure obligations should be addressable AEP artifacts once the model is mature.

---

## 37. Provider-Specific Generated Artifacts

Potential target projections include:

```text
Kubernetes manifests
Helm/Kustomize inputs
Crossplane resources
CloudFormation
OpenTofu/Terraform modules or plans
AWS Cloud Control operations
Azure deployment artifacts
GCP deployment artifacts
provider policy resources
network/security policy projections
```

The first implementation should not support all of these.

The architecture must simply avoid making the first output format authoritative.

---

## 38. `InfraRealization`

A realization records one concrete provider/platform implementation.

Illustrative:

```yaml
type: infra.realization/v1
id: commerce-prod-aws

specification:
  infra: commerce-platform/v4

target:
  aws-eks-production/v1

resources:
  cluster:
    locator: arn:aws:eks:...

  customer-db:
    provider_type: AWS::RDS::DBInstance
    locator: arn:aws:rds:...

obligations:
  satisfied:
    - migration/customer-db/...

conformance:
  report: infra-report:991
```

An observed environment is not automatically an InfraRealization. It becomes one only when it is correlated to a normative InfraSpec/target and its provenance is established.

---

## 39. Provider Portability

Portability is not the absence of vendor names.

It is the ability of multiple target capability sets to satisfy the same normative requirements.

```text
InfraIr
  |
  +--> plan(AWS)
  +--> plan(Azure)
  +--> plan(GCP)
```

A realizability report should expose:

```text
requirements directly generated
requirements requiring obligations
requirements refused
provider-specific semantic dependencies
semantic weakening required (must be refused unless spec changes)
```

---

## 40. `PortabilityProfile`

Illustrative:

```yaml
portability_profile:
  id: major-clouds/v1

  targets:
    - aws-production/v1
    - azure-production/v1
    - gcp-production/v1

  constraints:
    all_targets_realizable: true
    semantic_weakening: forbidden
    provider_specific_obligations:
      maximum: 2
```

This becomes a powerful design-time constraint for humans and LLM proposals.

---

## 41. Multi-Cloud as Multiple Realizations

Do not model multi-cloud as one giant provider-neutral deployment file.

Model:

```text
                     InfraSpec
                   /     |      \
                  /      |       \
                 v       v        v
          AWS realization  Azure realization  GCP realization
                 \       |        /
                  \      |       /
                   same normative conformance
```

Some organizations may operate several simultaneously; others use the second target only as migration/DR capability.

---

## 42. Infrastructure Evolution

Once `InfraRealization` exists, the brownfield transition problem becomes:

```text
Current InfraRealization
        +
Target InfraSpec / InfraTarget
        |
        v
InfraEvolutionPlanner
        |
        v
InfraEvolutionPlan
```

The planner should classify transition work with the same principles as semantic evolution.

---

## 43. Same-Provider Refactoring

Examples:

```text
AWS EC2 -> AWS EKS
self-managed DB -> RDS
one VPC -> segmented VPCs
classic ingress -> Gateway API
single cluster -> multi-cluster
```

The provider stays constant while physical architecture changes.

This is the simplest infrastructure-evolution proving ground before cross-cloud migration.

---

## 44. Cross-Provider Migration

Provider migration is:

```text
InfraSpec
  +
AWS realization
  +
Azure target
  -> InfraEvolutionPlan
```

The plan may contain:

- generated target infrastructure;
- data-migration obligations;
- identity-mapping obligations;
- DNS/certificate cutover steps;
- dual-run compatibility window;
- routing transition;
- rollback/fix-forward boundary;
- conformance checks before/after each stage.

---

## 45. Transition Invariants

Migration requires invariants that hold **during** the transition.

Examples:

```text
At least one healthy ingress path exists throughout cutover.

No acknowledged write is lost.

At most one writer is authoritative where single-writer semantics are required.

Traffic cannot move to the target database until replication lag is below threshold.

Secrets may only transit approved encrypted channels.

Rollback remains possible until irreversible checkpoint X.
```

These are not proven by endpoint conformance alone.

---

## 46. Infrastructure Evolution Conformance

Conceptually:

```text
InfraEvolutionPlan
       |
       v
TransitionConformanceSuite
       |
       v
AOP execution + telemetry
       |
       v
TransitionConformanceReport
```

A transition may be:

- fully reversible;
- reversible until a checkpoint;
- forward-only.

The existing AEP migration workflow is a strong precedent for explicitly modeling irreversible phases rather than pretending rollback always exists.

---

## 47. Bigger Picture - Artifact Graph

A future artifact graph could become:

```text
PRD / Architecture Design / ADR
             |
             +--------------------------+
             |                          |
             v                          v
            ESS                      InfraSpec
             |                          |
             v                          v
InfrastructureRequirementSet      InfraSynthesisPlan
             \                          /
              \                        /
               +------> InfraRealization
                           |
                    InfraConformanceReport
                           |
                           v
                        Evidence
                           |
                           v
                   Release / Migration Plan
                           |
                           v
                          AOP
```

Brownfield artifacts add:

```text
ObservedInfraSnapshot
       |
       +--> documents / diagrams
       +--> InfraDriftReport
       +--> candidate InfraSpec
       +--> incident/change context
```

---

## 48. AEP Artifact Integration

Potential future artifact kinds:

```text
InfrastructureSpecification
InfrastructureObservationSnapshot
InfrastructureSynthesisPlan
InfrastructureObligation
InfrastructureRealization
InfrastructureConformanceReport
InfrastructureEvolutionPlan
InfrastructureDriftReport
```

Do not add all of these immediately.

Introduce an artifact kind only when it has stable identity, lifecycle, relations, and governance value.

Large observation snapshots may be content-addressed external artifacts referenced by AEP rather than embedded directly.

---

## 49. AEP Relations

Potential typed relations:

```text
ESS --requires--> InfrastructureRequirementSet
InfrastructureRequirementSet --constrains--> InfraSpec
InfraSpec --generates--> InfraSynthesisPlan
InfraSynthesisPlan --derives--> InfrastructureObligation
Task --implements/resolves--> InfrastructureObligation
ChangeSet --satisfies--> InfrastructureObligation
InfraRealization --realizes--> InfraSpec
InfraConformanceReport --verifies--> InfraRealization
ObservedInfraSnapshot --observes--> InfraRealization
InfraDriftReport --compares--> InfraSpec / ObservedInfraSnapshot
InfraEvolutionPlan --transitions--> InfraRealization A / B
```

Exact relation taxonomy should be reconciled with existing generic AEP relations rather than creating near-duplicates.

---

## 50. ADP Integration

ADP governs engineering work that changes specifications, adapters, code, generated infrastructure definitions, and obligations.

Possible future development tasks:

```text
implement AWS observation adapter
add semantic resource mapping
resolve identity-correlation obligation
implement Azure target capability adapter
satisfy infrastructure obligation
add invariant/verifier
repair target conformance failure
```

Once stable, a new evidence kind such as `infra_conformance` could mirror `ess_conformance`, with an independent infrastructure conformance runner.

The coding/configuration agent must not self-certify the result.

---

## 51. AOP Integration

AOP already has the right conceptual vocabulary:

```text
telemetry
blast radius
reversible change
health verification
rollback
preparation
migration
```

Infrastructure operations should therefore extend AOP through profiles/workflows rather than creating a separate orchestration engine.

Potential work classes:

```text
read-only estate discovery
infrastructure change
infrastructure migration
cloud-provider migration
DR exercise
capacity change
security remediation
```

---

## 52. Existing Release Workflow Reuse

The current progressive release workflow models:

```text
qualify -> stage -> canary -> observe -> promote -> verify -> complete
```

with independent telemetry evidence and rollback preconditions.

That is useful for infrastructure changes that can be staged/progressively exposed.

Examples:

- gateway replacement;
- node-pool migration;
- new cluster traffic cutover;
- routing changes;
- read-replica promotion.

Do not assume every infrastructure change maps directly to an application release, however.

---

## 53. Existing Migration Workflow Reuse

The forward-only migration workflow explicitly models an irreversible `migrate` state and requires preparation/dry-run evidence before the point of no return.

That is directly applicable to:

- destructive data migrations;
- provider cutovers with irreversible state movement;
- storage-format transitions;
- certain identity/security migrations;
- resource transitions whose source cannot remain available.

Infrastructure evolution should reuse this principle rather than introducing a universal rollback fiction.

---

## 54. Need for an Infrastructure Change Profile

Eventually a dedicated AOP profile/workflow may be warranted.

For example:

```text
profile: infrastructure-change.standard

principles:
  least-privilege
  blast-radius-bounded
  reversible-when-possible
  preflight-conformance
  post-change-conformance
  provenance
  approval-gates
```

But the first infrastructure milestone should prove the semantic model before adding protocol surface.

---

## 55. Incident Integration

Observed infrastructure context can enrich AOP incident work:

```text
incident symptom
    |
    v
service/workload locator
    |
    +--> current dependency closure
    +--> shared failure domains
    +--> current network permissions
    +--> observed traffic
    +--> recent infrastructure deltas
    +--> release/migration history
```

The scanner/graph does not diagnose the incident by itself. It produces deterministic context and facts for humans/agents operating under AOP.

---

## 56. Agent Roles

Agents may legitimately:

- propose candidate InfraSpec from observed infrastructure;
- propose semantic identity mappings;
- propose missing invariants;
- propose architecture refactors;
- propose provider-neutral alternatives;
- propose migration/evolution plans;
- implement infrastructure obligations;
- repair conformance failures;
- compare candidate target architectures;
- summarize graph/context for humans.

Agents may not authoritatively establish:

- that a scan is complete;
- that an identity correlation is correct without evidence/approval;
- that a realization conforms;
- that a migration succeeded;
- that absence of observed traffic proves absence of dependency;
- that provider semantics are equivalent when the capability model cannot prove it.

---

## 57. Agent Proposal Evaluation Loop

Example:

```text
Objective:
  Reduce production infrastructure cost by 20%
  without reducing availability or provider portability.

Agent proposes candidate InfraSpec
          |
          v
Infra compiler
          |
          v
InfraSpecDelta + ImpactReport
          |
          v
AWS/Azure/GCP realizability
          |
          v
policy / invariant checks
          |
    +-----+------+
    |            |
 accepted     counterexamples
                 |
                 v
            agent revises
```

The agent becomes a search heuristic over infrastructure designs.

---

## 58. Capability and Least-Privilege Integration

Read-only discovery may require broad visibility, but it must still be capability-scoped.

Existing AEP/AOP concepts such as production read, network read, telemetry read, repository read, and least privilege should be reused where they fit.

Potential future finer-grained capabilities may include:

```text
cloud.inventory.read
cluster.inventory.read
identity.policy.read
network.flow.read
cloud.resource.write
cluster.resource.write
network.route.write
identity.policy.write
```

Do not add new capability names merely to mirror every provider API. Add them only when they create meaningful governance boundaries.

---

## 59. Discovery Security Model

Infrastructure scanning is security-sensitive even when read-only.

Requirements should include:

- least-privilege discovery roles;
- no raw secret-value collection;
- scoped account/subscription/project/cluster access;
- audit of scanner identity and scope;
- explicit retention rules for observed topology;
- protection of network and IAM graph data;
- redaction/tokenization where provider identifiers are unnecessarily sensitive;
- deterministic record of incomplete access.

A failed permission should produce a coverage gap, not silently omit the resource and claim completeness.

---

## 60. Coverage and Completeness

Every snapshot should expose what was and was not scanned.

Illustrative:

```yaml
coverage:
  aws:
    accounts:
      discovered: 12
      scanned: 11
      inaccessible: 1

    regions:
      requested: all-enabled
      completed: 18

  kubernetes:
    clusters:
      requested: 7
      completed: 7

  runtime_flows:
    source: vpc-flow-logs
    coverage:
      vpcs: 9/12
```

A complete-looking graph with hidden coverage holes is dangerous.

---

## 61. External Discovery Feasibility

The design is feasible with current provider/platform APIs, while retaining provider-specific gaps.

### Kubernetes

Each Kubernetes cluster publishes Discovery API information and OpenAPI schemas for APIs it serves. This makes cluster-specific discovery possible, including resources added by extensions/CRDs.

### AWS

AWS Cloud Control provides a standardized CRUD-L model and JSON schemas for many AWS and third-party resource types, and can list/read existing supported resources even if they were not provisioned through Cloud Control.

AWS Config provides useful configuration relationships, but AWS explicitly states those relationships do not include network/data-flow dependencies.

VPC Flow Logs provide time-windowed IP traffic evidence at network-interface/VPC/subnet scope.

### Azure

Azure Resource Graph supports resource exploration/query at scale across subscriptions and exposes provider-returned resource properties and change-oriented queries.

### GCP

Cloud Asset Inventory provides organization/project/folder-scale inventory, IAM/policy visibility, resource history, and change-oriented inventory capabilities.

### Kubernetes runtime traffic

Cilium Hubble can discover service dependency graphs at L3/L4 and, where available, L7. This is a strong example of why runtime observations belong in a separate evidence plane.

No provider source should be treated as universally complete.

---

## 62. Cross-Layer ESS <-> Infrastructure Conformance

Once both layers exist, the system can test not only software semantics and infrastructure independently, but their compatibility.

Example ESS requirement:

```text
invoice-service
  requires private PostgreSQL
  requires event delivery
  requires >= 2 workload replicas
```

Infrastructure conformance can establish:

```text
private DB capability exists
workload identity can reach DB
network path is permitted
TLS/encryption requirement satisfied
event broker exists and is reachable
placement satisfies failure-domain requirement
```

This closes a gap that neither ESS application conformance nor raw Kubernetes validation can establish alone.

---

## 63. Cross-Layer Runtime Drift

The system can also detect contradictions such as:

```text
ESS says invoice-service communicates through event delivery.
Observed infra says it calls email-service:8080 directly.
```

or:

```text
ESS requires private database connectivity.
Observed infra exposes the database publicly.
```

or:

```text
ESS requires two replicas.
Observed cluster currently has one schedulable replica because both declared replicas land in one failed zone.
```

These are high-value semantic findings.

---

## 64. Shared Platform Constraints

A shared InfraSpec may need to arbitrate requirements from multiple ESS systems.

Examples:

- incompatible Kubernetes versions;
- conflicting network-isolation requirements;
- shared broker capacity;
- regional/data-residency constraints;
- namespace/tenant limits;
- identity-policy conflicts.

The infrastructure planner should expose conflicts rather than silently choose one consumer's preference.

---

## 65. Provider Capability Evolution

Targets change over time.

A provider/platform upgrade can change realizability without the InfraSpec changing.

Therefore semantic diff should support:

```text
InfraSpec constant
TargetCapabilities v1 -> v2
        -> TargetCapabilityDelta
        -> affected plans/realizations
```

This supports cloud deprecations, Kubernetes upgrades, operator changes, and new provider capabilities.

---

## 66. Platform Version and CRD Discovery

Kubernetes is especially dynamic because clusters may support different API groups and CRDs.

A Kubernetes target should derive a capability snapshot from:

```text
Discovery API
OpenAPI schemas
installed CRDs/operators/controllers
cluster version/features
```

Generation should target the capabilities of the selected cluster/profile rather than assume one universal Kubernetes schema.

---

## 67. IaC as Projection and Evidence

Terraform/OpenTofu/CloudFormation/Kubernetes YAML can play two roles:

### As generated projection

```text
InfraIr + Target -> IaC artifacts
```

### As discovered/declared evidence

```text
existing IaC -> observations / declared-state facts
```

The same format can appear on both sides without becoming the semantic source of truth.

---

## 68. Do Not Automatically Rewrite Existing IaC

A brownfield import should not immediately regenerate the entire estate and replace hand-authored IaC.

Safer stages:

```text
observe
  -> document
  -> correlate with IaC
  -> identify drift
  -> extract candidate InfraSpec
  -> review
  -> generate new managed boundary incrementally
```

This minimizes accidental semantic loss and organizational churn.

---

## 69. Infrastructure Ownership Boundaries

Use the same generated/authored separation as structural synthesis.

Conceptually:

```text
generated/
    target-owned infrastructure artifacts

implementations/
    authored obligation resolution / provider-specific extensions
```

Never require an agent/human to edit disposable generated files to satisfy an obligation.

---

## 70. Reference Fixture

The first reference environment should be deliberately small but cross-layer.

Suggested fixture:

```text
AWS account
  VPC
    public subnets
    private subnets

  EKS cluster
    namespace: billing
      invoice-service
      email-service

  RDS PostgreSQL
  MSK or simple event broker equivalent
  ALB / Gateway
  IAM workload identities
  KMS key
```

ESS:

```text
Billing
  invoice-service
  email-service
  CreateInvoice -> InvoiceCreated -> SendEmail
```

Runtime observation:

```text
Hubble or synthetic flow fixture
invoice-service -> database
invoice-service -> broker
email-service -> broker
```

The fixture should contain deliberate defects/fault variants.

---

## 71. Required Deliberate Faults

Discovery/conformance is not trusted until it rejects known-bad infrastructure.

Fault matrix should include:

```text
public database
missing encryption
single failure domain
undeclared workload communication
overbroad IAM grant
missing network policy
wrong workload identity binding
backup retention below requirement
provider target lacking required capability
stale observation causing Unknown rather than Pass
identity correlation conflict
```

Later migration faults:

```text
traffic cutover before replication catches up
secret/certificate missing on target
DNS cutover without healthy target
rollback claimed after irreversible data change
```

---

## 72. Determinism

Determinism requirements:

- stable resource and relationship ordering;
- canonical serialization/digests;
- stable diagnostics;
- stable graph traversal/impact paths;
- explicit timestamps excluded from semantic digest where appropriate;
- no heuristic provider equivalence;
- no LLM-dependent core normalization decisions;
- deterministic handling of identical evidence sets.

Observation itself is time-varying; processing of the same observation set should not be.

---

## 73. Versioning and Provenance

Generated/derived artifacts should carry:

```text
InfraSpec version/digest
InfraIr format version
scanner/adapter versions
source provider/API versions where relevant
observation snapshot digest
planner version
target capability profile version
generator version
conformance suite/verifier version
```

This allows later evidence invalidation and reproduction.

---

## 74. Failure and Unknown Handling

The infrastructure domain has many legitimate incomplete states.

Examples:

```text
provider resource type unsupported by adapter
cloud account inaccessible
runtime telemetry absent
identity correlation ambiguous
policy semantics unsupported
TLS status not observable from L4 evidence
provider target cannot satisfy a requirement
historical evidence window incomplete
```

These should become typed diagnostics, Unknown/Conflict results, obligations, or refusals.

Never silently omit them.

---

## 75. Non-Goals for v1 Discovery

Do not initially attempt:

- complete semantic coverage of every AWS/Azure/GCP resource type;
- universal service dependency inference;
- automatic intent recovery;
- LLM-based silent identity merging;
- full packet inspection;
- secret-value collection;
- real-time CMDB replacement;
- full cost model;
- automatic remediation;
- every provider simultaneously.

---

## 76. Non-Goals for v1 InfraSpec

Do not initially attempt:

- universal cloud ontology;
- every Kubernetes CRD/operator concept;
- every compliance framework;
- full provider price/performance optimization;
- formal equivalence of arbitrary managed services;
- application domain behavior already owned by ESS;
- replacement for Terraform/OpenTofu/CloudFormation/Crossplane;
- replacement for OPA/Kyverno/Gatekeeper;
- a deployment orchestrator.

---

## 77. Suggested Delivery Tracks

The work should be split so useful discovery/diff capabilities can arrive before full synthesis/evolution.

### Track ID - Infrastructure Discovery

```text
ID1  Observation/evidence model
ID2  Kubernetes inventory adapter
ID3  AWS inventory adapter
ID4  deterministic ObservedInfraSnapshot
ID5  identity correlation + typed static relationships
ID6  documentation/diagram projections
ID7  runtime flow adapter (Hubble or fixture)
ID8  AWS flow-log adapter
ID9  snapshot semantic diff
```

### Track IS - Infrastructure Specification

```text
IS1  minimal InfraSpec domain model
IS2  InfraIr compiler
IS3  infrastructure invariants
IS4  desired-vs-observed conformance
IS5  candidate-spec extraction workflow
IS6  semantic InfraSpec diff
```

### Track BR - ESS Bridge

```text
BR1  InfrastructureRequirementSet model
BR2  ESS topology -> requirement extraction
BR3  requirement satisfaction against InfraIr
BR4  cross-layer conformance
```

### Track TG - Target Generation

```text
TG1  InfraTargetCapabilities
TG2  InfraSynthesisPlan
TG3  Kubernetes target
TG4  AWS-native target
TG5  generated/obligation/refused falsification
TG6  InfraRealization
```

### Track MC - Portability / Multi-Cloud

```text
MC1  Azure target capability adapter
MC2  cross-target realizability report
MC3  PortabilityProfile
MC4  same InfraSpec -> AWS + Azure conformance
MC5  later GCP target
```

### Track EV - Infrastructure Evolution

```text
EV1  InfraEvolutionPlan
EV2  same-provider refactor fixture
EV3  migration obligations
EV4  transition invariants/conformance
EV5  cross-provider migration fixture
EV6  AOP-governed transition
```

---

## 78. Timing Relative to Current ESS Waves

Do not interrupt the currently gated Wave 4/5 implementation sequence.

Recommended timing:

```text
Current
  W4 closed-loop ESS conformance
  W5 structural synthesis
  W6 obligation -> ADP closure
  W7 Realization + multiple software realizations

Parallel design/prototype after model boundaries settle
  ID1-ID6 discovery + snapshots + docs
  IS1 minimal InfraSpec

At/around existing W8
  BR1-BR4 ESS topology bridge
  TG1-TG3 infrastructure target planning / Kubernetes projection

After InfraRealization proves itself
  multi-cloud targets
  infrastructure evolution
  AOP migration execution
```

The infrastructure discovery track can begin earlier because it does not depend on ESS Realization. Provider realization/evolution should wait until the relevant synthesis/realization primitives are stable.

---

## 79. Recommended W8 Revision

When W8 is formally designed, change its wording from a direct large Kubernetes generator into:

```text
W8.1  Formalize InfrastructureRequirementSet emitted from ESS topology
W8.2  Resolve requirements against an InfraTarget capability profile
W8.3  Produce InfraSynthesisPlan dispositions
W8.4  Generate one Kubernetes realization
W8.5  Run infrastructure conformance
W8.6  Prove deliberate faulty target/configuration fails
```

This retains the roadmap's intent while establishing a reusable infrastructure layer.

---

## 80. Suggested Crate/Module Boundaries

Names are provisional.

Potential future crates:

```text
infra-domain
    normative resource/relationship/invariant types
    observation/evidence types

infra-compiler
    InfraSpec -> InfraIr

infra-discovery
    adapter traits
    snapshot normalization
    identity correlation

infra-diff
    snapshot/spec/drift semantic diff

infra-conformance
    canonical checks/scenarios/reporting

infra-synthesis
    target capabilities
    InfraSynthesisPlan
    obligations/refusals

infra-gen-kubernetes
infra-target-aws
infra-target-azure
```

Do not create all crates up front. Start with the minimum boundaries needed to prevent provider adapters from contaminating semantic types.

---

## 81. Core API Sketches

Illustrative only.

```rust
pub trait ObservationAdapter {
    fn observe(
        &self,
        scope: &ObservationScope,
    ) -> Result<Vec<ProviderObservation>, ObservationError>;
}
```

```rust
pub fn normalize_observations(
    observations: &[ProviderObservation],
) -> Result<ObservedInfraSnapshot, ObservationDiagnostics>;
```

```rust
pub fn compile_infra_spec(
    spec: InfraSpec,
) -> Result<InfraIr, InfraDiagnostics>;
```

```rust
pub fn diff_snapshots(
    before: &ObservedInfraSnapshot,
    after: &ObservedInfraSnapshot,
) -> ObservedInfraDelta;
```

```rust
pub fn check_conformance(
    spec: &InfraIr,
    target: &dyn InfraConformanceTarget,
) -> InfraConformanceReport;
```

```rust
pub fn plan_infrastructure(
    spec: &InfraIr,
    requirements: &[InfrastructureRequirement],
    target: &InfraTargetCapabilities,
) -> InfraSynthesisPlan;
```

---

## 82. Query Surface

High-value graph queries should be deterministic and semantic.

Examples:

```text
resources(kind, scope)
relations(subject, relation, object)
path(from, to, relation_filter)
dependencies(resource, direction, depth)
effective_authority(principal)
public_exposure(resource)
observed_communications(window)
required_communications(resource)
permitted_communications(resource)
blast_radius(resource/change)
coverage(snapshot)
```

The query engine must retain evidence paths so answers can say *why* a relation exists.

---

## 83. Infrastructure Review Output

A pre-change or architecture review could eventually include:

```text
Infrastructure Change Impact

Semantic changes:
  1 network boundary changed
  1 database availability requirement strengthened
  2 workload placement requirements changed

Blast radius:
  4 workloads
  2 ESS realizations
  1 shared database

Security:
  no new public exposure
  one IAM mapping obligation

Portability:
  AWS: generated
  Azure: obligation - identity mapping
  GCP: refused - selected target lacks required feature

Verification:
  12 static invariants rerun
  3 runtime checks required

Transition:
  reversible until DNS cutover
  database copy is forward-only after checkpoint
```

This is the infrastructure analogue of semantic ESS change review.

---

## 84. Falsifiability

Every core claim must be falsifiable.

### Discovery

A deliberately incomplete adapter must create coverage gaps rather than a falsely complete snapshot.

### Correlation

Ambiguous identities must remain unresolved rather than silently merged.

### Conformance

Known-bad infrastructure must fail the intended invariant.

### Portability

A target missing a required semantic capability must refuse rather than generate a weakened approximation.

### Runtime evidence

Missing/stale telemetry must produce Unknown where the invariant cannot be established.

### Evolution

A deliberately unsafe migration must fail transition conformance even if both endpoints independently conform.

---

## 85. Discovery Acceptance Criteria

Discovery v1 is successful when:

- one Kubernetes cluster can be scanned deterministically;
- one AWS account/region reference estate can be scanned deterministically;
- source-native locators and provenance are preserved;
- coverage gaps are explicit;
- a provider-neutral resource graph is produced;
- static relationships are typed;
- identity ambiguities are explicit;
- documentation/diagrams are generated from the snapshot;
- repeated normalization of the same observations yields the same semantic digest.

---

## 86. Observation/Runtime Acceptance Criteria

Runtime observation is successful when:

- at least one flow source produces time-bounded communication relations;
- required/permitted/observed edges remain distinct;
- undeclared observed communication can be reported;
- absence of telemetry does not produce a false pass;
- evidence freshness/window is preserved.

---

## 87. InfraSpec Acceptance Criteria

InfraSpec v1 is successful when:

- provider-neutral infrastructure requirements can be authored/validated;
- references compile into stable InfraIr;
- infrastructure invariants are first-class;
- target/provider resource names are not required in core normative concepts;
- observed snapshots are not silently promoted to normative intent;
- semantic spec diff is deterministic.

---

## 88. Cross-Layer Acceptance Criteria

The ESS bridge is successful when:

- ESS topology emits typed InfrastructureRequirementSet objects;
- no provider-specific concepts enter ESS to satisfy the fixture;
- InfraSpec/target resolves each requirement;
- unsatisfied requirements produce explicit obligations/refusals;
- cross-layer conformance can prove that the deployed infrastructure satisfies the software runtime requirements.

---

## 89. Synthesis Acceptance Criteria

Infrastructure synthesis is successful when:

- every normative requirement receives Generated, Obligation, or Refused;
- no provider semantic weakening is silent;
- generated artifacts are deterministic/disposable;
- authored obligation implementations do not edit generated files;
- one Kubernetes/AWS realization is produced;
- the realization passes independent infrastructure conformance;
- a deliberately broken realization fails the same oracle.

---

## 90. Multi-Cloud Acceptance Criteria

Multi-cloud is successful when:

- one InfraSpec plans against at least AWS and Azure targets;
- target-specific resource choices differ while normative requirements remain unchanged;
- both realizations pass the same provider-neutral invariant/conformance suite where applicable;
- unsupported semantics cause explicit refusal;
- a PortabilityProfile can reject a candidate design that violates the selected target set.

GCP can follow after the second-provider proof demonstrates the abstraction is real rather than accidentally AWS-shaped.

---

## 91. Evolution Acceptance Criteria

Infrastructure evolution is successful when:

- one known InfraRealization can be refactored to another realization;
- transition obligations are explicit;
- transition invariants are checked independently;
- irreversible checkpoints are represented honestly;
- AOP can govern the execution permissions/approvals/evidence;
- endpoint conformance and transition conformance remain separate;
- a deliberately unsafe migration is refused or fails the intended verifier.

---

## 92. Most Important Risks

### 92.1 Universal-ontology creep

Trying to model every provider object will destroy semantic clarity.

Mitigation: capability-oriented core, provider details in adapters/targets/realizations.

### 92.2 False confidence from incomplete scans

Mitigation: explicit coverage, freshness, Unknown, and evidence provenance.

### 92.3 Incorrect identity correlation

Mitigation: stable source locators, explicit mapping evidence, unresolved conflicts.

### 92.4 Overclaiming network/application dependencies

Mitigation: required/permitted/observed edge separation and time windows.

### 92.5 Provider leakage into normative spec

Mitigation: target capability layer and portability testing with a second provider early.

### 92.6 Duplicating ESS topology

Mitigation: formal InfrastructureRequirementSet bridge and clear ownership boundaries.

### 92.7 Building an orchestrator by accident

Mitigation: external cloud/IaC/deployment systems perform actions; AEP/AOP decide what is permitted and what evidence is required.

### 92.8 Security of discovered graph data

Mitigation: least privilege, secret-value exclusion, artifact access control, provenance, retention policy.

---

## 93. Open Design Questions

These should be resolved through the reference fixture rather than abstract debate.

1. What is the minimal provider-neutral resource vocabulary that is useful without becoming generic mush?
2. Which observed claim categories need first-class types versus generic relation evidence?
3. How should cross-source identity mapping be represented and approved?
4. Should `ObservedInfraSnapshot` be content-addressed and stored outside AEP by default?
5. Which network/security semantics can be normalized safely across AWS/Kubernetes/Azure without semantic loss?
6. What is the minimal `InfrastructureRequirementSet` ESS must emit at W8?
7. Which invariants belong in ESS topology versus InfraSpec?
8. How are shared platform capabilities allocated across multiple ESS consumers?
9. Which first AWS resources should be supported to prove the model?
10. Which Azure target should be the second-provider falsification test?
11. How should provider capability profiles be versioned against provider API/product evolution?
12. Which migration operations can safely be generated and which should always begin as obligations?

---

## 94. Recommended First Design Spike

Do not start with multi-cloud generation.

Build one narrow discovery/conformance vertical slice:

```text
Reference AWS/EKS billing environment
        |
        +--> Kubernetes observation adapter
        +--> AWS observation adapter
        |
        v
ObservedInfraSnapshot
        |
        v
resource + identity + network graph
        |
        +--> docs/diagram
        +--> 8-12 invariants
        +--> semantic snapshot diff
        |
        v
candidate minimal InfraSpec
        |
        v
desired-vs-observed conformance
```

Deliberately include:

- public exposure fault;
- wrong IAM binding;
- missing failure-domain redundancy;
- undeclared flow;
- inaccessible scan scope;
- identity-correlation ambiguity.

This proves the difficult epistemic/discovery design before provider generation is added.

---

## 95. Recommended Second Design Spike

Once the first slice is reliable:

```text
minimal InfraSpec
    +
AWS target capabilities
    -> InfraSynthesisPlan
    -> AWS/EKS realization
    -> conformance
```

Then implement an Azure target for the **same** InfraSpec.

The second target is the test that provider-neutral semantics are actually neutral.

Do not add GCP until that proof succeeds.

---

## 96. Relationship to Semantic Diff / Brownfield Evolution Design

This infrastructure design should reuse the previously proposed semantic-change architecture rather than invent another change engine.

Common concepts:

```text
typed semantic delta
impact closure
explainable paths
unknown rather than guessed relations
versioned policy above deterministic facts
obligation invalidation
proposal evaluation
LLM candidate search
realization-aware evolution later
```

The infrastructure layer adds observation-specific semantics:

```text
coverage
freshness
time windows
conflicting evidence
identity correlation
required/permitted/observed relation classes
```

And provider-target semantics:

```text
capability profiles
portability
multi-cloud realization
infrastructure transition invariants
```

---

## 97. Relationship to AEP Principles

The infrastructure layer should make existing AEP principles more executable.

Examples:

```text
least privilege
    -> effective authorization graph + policy evidence

reversible changes
    -> InfraEvolutionPlan rollback boundary

approval gates
    -> high-blast-radius/provider-migration transitions

provenance tracking
    -> observation/source/change evidence

contract/invariant testing
    -> InfraConformanceSuite

verify after action
    -> post-change observation + conformance
```

This is precisely the kind of typed evidence AEP is meant to govern.

---

## 98. Relationship to AOP

AOP remains the governing protocol for production operations.

The infrastructure layer supplies AOP with:

- target state;
- current observed state;
- semantic diff;
- blast-radius facts;
- preflight conformance;
- transition plan and irreversible boundaries;
- health/telemetry requirements;
- post-change conformance evidence.

AOP supplies the infrastructure layer with governance:

- capability/permission decisions;
- approval requirements;
- workflow transitions;
- rollback/escalation semantics;
- completion predicates;
- audit.

---

## 99. Relationship to Agents

The combined architecture becomes:

```text
human objective
      |
      v
agent proposes infra/system change
      |
      v
semantic compiler + diff + planner
      |
      +--> invalid/refused/counterexample
      |             |
      |             v
      |          agent repair
      |
      v
AEP/ADP/AOP governed execution
      |
      v
independent observation + conformance
      |
      v
evidence
```

The agent is powerful because the deterministic substrate can tell it exactly what was affected and why its proposal failed.

---

## 100. Architectural End State

The longer-term system becomes a closed semantic lifecycle across software and infrastructure:

```text
                         intent / requirements
                                |
                                v
                               ESS
                                |
                       software semantics
                                |
                                v
                     software Realization
                                |
                     runtime requirements
                                v
                            InfraSpec
                                |
                       infrastructure semantics
                                |
              +-----------------+------------------+
              |                 |                  |
              v                 v                  v
          AWS target        Azure target        GCP target
              |                 |                  |
              v                 v                  v
       InfraRealization  InfraRealization   InfraRealization
              |                 |                  |
              +-----------------+------------------+
                                |
                                v
                          running reality
                                |
                                v
                         infrastructure scan
                                |
                                v
                     ObservedInfraSnapshot
                                |
                  +-------------+-------------+
                  |                           |
                  v                           v
            semantic drift              runtime evidence
                  |                           |
                  +-------------+-------------+
                                |
                                v
                     AEP / ADP / AOP evidence
                                |
                                v
                         next governed change
```

This makes infrastructure neither a pile of manifests nor a separate operational universe. It becomes another typed, testable, evolvable semantic system connected to the same engineering governance model.

---

## 101. Final Thesis

The strongest form of the idea is not:

> Scan AWS and Kubernetes, convert them to YAML, then generate Azure YAML.

It is:

> **Lift provider/platform reality into an evidence-backed semantic graph; explicitly separate observation from intent; refine intent into a provider-neutral infrastructure specification; test that specification against reality; then lower it through typed target capabilities into one or more infrastructure realizations without silently weakening semantics.**

That gives the repository a coherent path from greenfield software specification to brownfield infrastructure understanding and eventually to governed multi-cloud evolution.

The division of responsibility becomes:

```text
ESS
    defines software-system semantics and runtime requirements

InfraSpec
    defines provider-neutral infrastructure/platform semantics

Observation adapters
    establish evidence about what actually exists and happens

Infra compiler / diff / planner
    deterministically normalize, compare, analyze, and plan

Infrastructure obligations
    make underdetermined decisions/work explicit

ADP agents
    implement or resolve residual engineering obligations

Infra conformance
    independently establishes infrastructure correctness

AOP
    governs production discovery, change, migration, verification, and rollback
```

The same core principles continue to hold:

> **Semantic concepts are primary. Provider resources and manifests are projections or observations.**

> **Do not use the agent where deterministic machinery already has enough information.**

> **Do not let deterministic machinery invent intent or semantics that the evidence/specification does not contain.**

> **Unknown is preferable to a plausible lie.**

> **Endpoint conformance does not prove transition safety.**

> **Portability is a property to prove against multiple capability targets, not a naming convention.**

---

## 102. Recommended Roadmap Summary

```text
CURRENT ESS ARC

W4  closed-loop ESS conformance
W5  structural synthesis
W6  obligation -> ADP closure
W7  Realization / multiple software realizations


EARLY INFRA TRACK - can begin once capacity allows

ID1-ID6
    discovery/evidence/snapshot/docs

ID7-ID9
    runtime observations + semantic snapshot diff

IS1-IS6
    minimal InfraSpec + InfraIr + invariants + drift/conformance


REFINED W8

BR1-BR4
    ESS topology -> InfrastructureRequirementSet -> infrastructure satisfaction

TG1-TG3
    target capabilities -> InfraSynthesisPlan -> Kubernetes/AWS realization


LATER

TG4-TG6
    provider target + InfraRealization

MC1-MC5
    Azure proof, portability profile, multi-cloud realizability

EV1-EV6
    infrastructure refactoring, migration, transition conformance, AOP execution
```

---

## 103. References and Grounding

### Repository sources

- `docs/VISION.md` - AEP/ESS division, evidence boundary, semantic source-of-truth thesis, and non-goals.  
  https://github.com/codewandler/engineering-protocols/blob/main/docs/VISION.md

- `docs/design/ess-implementor-design-v0.1.md` - ESS domain/component/interaction/topology layers; topology as semantic runtime requirements; deployment formats as compilation targets.  
  https://github.com/codewandler/engineering-protocols/blob/main/docs/design/ess-implementor-design-v0.1.md

- `docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md` - `Generated | Obligation | Refused`, `Realization`, multiple physical realizations, release bridge, and deferred topology synthesis.  
  https://github.com/codewandler/engineering-protocols/blob/main/docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md

- `docs/design/consolidated-design-v0.2.md` - AEP support for infrastructure changes, migrations, compliance, DR and capacity as profiles over common primitives; artifact and operations model.  
  https://github.com/codewandler/engineering-protocols/blob/main/docs/design/consolidated-design-v0.2.md

- `protocols/adp/1.yaml` - independent ESS conformance evidence and development semantics.  
  https://github.com/codewandler/engineering-protocols/blob/main/protocols/adp/1.yaml

- `protocols/aop/1.yaml` - telemetry, blast radius, reversible change, health verification, rollback, preparation and migration phases.  
  https://github.com/codewandler/engineering-protocols/blob/main/protocols/aop/1.yaml

- `workflows/releases/progressive.yaml` - staged/canary/observation/promotion workflow with independent telemetry and rollback preconditions.  
  https://github.com/codewandler/engineering-protocols/blob/main/workflows/releases/progressive.yaml

- `workflows/migrations/forward-only.yaml` - explicit irreversible migration semantics and independent dry-run/verification evidence.  
  https://github.com/codewandler/engineering-protocols/blob/main/workflows/migrations/forward-only.yaml

### External feasibility references - non-normative

- Kubernetes API discovery and OpenAPI publication:  
  https://kubernetes.io/docs/concepts/overview/kubernetes-api/

- AWS Cloud Control API - uniform resource CRUD-L, resource schemas, discovery of existing supported resources:  
  https://docs.aws.amazon.com/cloudcontrolapi/latest/userguide/how-it-works.html  
  https://docs.aws.amazon.com/cloudcontrolapi/latest/userguide/resource-operations-list.html

- AWS Config configuration item relationships and explicit limitation regarding network/data flow:  
  https://docs.aws.amazon.com/config/latest/developerguide/config-item-table.html

- AWS VPC Flow Logs:  
  https://docs.aws.amazon.com/vpc/latest/userguide/flow-logs.html

- Cilium Hubble network/service dependency observability:  
  https://docs.cilium.io/en/stable/observability/hubble/

- Azure Resource Graph:  
  https://learn.microsoft.com/en-us/azure/governance/resource-graph/overview

- Google Cloud Asset Inventory:  
  https://cloud.google.com/asset-inventory/docs/overview

### Related local design

- `ESS Semantic Diff, Impact Analysis & Evolution Planning - Design v0.1` - semantic diff, impact analysis, proposal evaluation, compatibility policy, and realization-aware evolution. This infrastructure design is intended to reuse those change-analysis primitives rather than create a separate change theory.
