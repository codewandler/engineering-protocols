# Reconciliation: implementation vs Consolidated Design v0.2

Status date: 2026-08-19. Authoritative spec: [`consolidated-design-v0.2.md`](consolidated-design-v0.2.md).
Superseded inputs: [`design-draft-v0.1.md`](archive/design-draft-v0.1.md),
[`artifact-model-extension-v0.1.md`](archive/artifact-model-extension-v0.1.md) — both are subsumed by v0.2 and
kept only for provenance.

This document records what the current tree implements, what v0.2 changes, and the order the
remaining work is being done in. It is the reasoning record; the chat status is the summary.

---

## 1. What v0.2 keeps unchanged

Everything already built against v0.1 + the artifact extension survives v0.2 unmodified in intent:

| v0.2 section | concept | implemented in |
|---|---|---|
| 5.1, 5.2, 66 | Rust source of truth, `Raw*` → `TryFrom` → validated | every document type; `aep-schema::parse` |
| 5.6, 31, 32 | generation/verification separation, counterexamples | `verification.rs`, `EvidenceRequirement::independent` |
| 5.7, 30 | evidence drives transitions, provenance envelopes | `evidence.rs`, `Evidence::facts` |
| 5.8, 29 | explicit capabilities, default deny | `capability.rs` |
| 19–22 | artifact taxonomy, locations, relations, statuses, lifecycles | `artifact.rs` |
| 12, 29.1 | review semantics, revision-bound approval | `review.rs`, `ReviewResult::covers` |
| 23–25 | principles, obligations, timing, applicability | `principle.rs` |
| 27, 28 | workflow state machine, artifact requirements in states | `workflow.rs`, `requirement.rs` |
| 33, 60 | completion predicate, explainability lists | `requirement.rs`, `RequirementReport` |
| 67, 68 | generated schemas, versioning, unknown-major rejection | `aep-schema::schema`, `protocol.rs` |

No rework is needed for any of it. The predicate engine, the fact vocabulary, the artifact graph and
the requirement layer are the substrate v0.2's additions sit on.

## 2. What v0.2 adds

Five genuinely new layers, none of which existed in v0.1:

### 2.1 Universal entity model (§13–18)

v0.2 makes **`Entity`** the addressable primitive, not `Artifact`:

- `EntityId` — opaque canonical identity (ULID/UUIDv7-shaped), *not* `AUTH-142`.
- `EntityLocator` — `ep://acme/payments/story/AUTH-142`, a logical address, not a storage URL.
- `EntityType` — versioned, namespaced: `aep.design/v1`, `aop.incident/v1`.
- `Entity<T>` / `EntityMetadata` — id, locator, type, revision, timestamps, provenance.
- `EntityRef` vs `VersionedEntityRef` — "current X" versus "exactly revision N of X".

**Consequence for the current tree:** `ArtifactId` (`design:passkeys-auth`) becomes a *locator key*,
not identity. The artifact graph stays valid as the in-memory projection an execution reasons over;
identity, revision and type move up into the entity layer. Existing code keeps working because
`ArtifactRef` already carries an optional version — that becomes `VersionedEntityRef` semantics.

### 2.2 Command side (§5.4, 35–43)

Every mutation becomes a command: `CommandEnvelope` + `CommandContext`
(`request_id`, `command_id`, `idempotency_key`, `actor`, `executor`, `correlation_id`, `causation`,
`trace`, `execution_id`, `task`, `protocol`), `CommandResult` with affected revisions, emitted events,
audit refs and a `ConsistencyToken`. Idempotent replay and `expected_revision` conflict detection are
contract-level requirements, not backend choices. No universal physical delete.

### 2.3 Query side (§44–47)

Read-only `QueryService`: `get`, `resolve`, `query`, `relations`, `history`, `audit`,
`describe_type`, with `QueryConsistency::AtLeast(token)` giving technology-independent
read-your-writes. `TypeDescriptor` makes semantics discoverable so a harness need not hard-code
domain types.

### 2.4 Audit model (§52–59)

`AuditRecord` with actor **and** executor, correlation and causation, decision records, change
records (before/after revision), and — explicitly — **rejected attempts**. A denied
`production.write` must not mutate state but must be reconstructable.

### 2.5 Conformance (§75–86, 103–105)

A reusable black-box suite (`aep-conformance` + `conformance/` fixtures) that any backend can run to
prove it implements the contract, plus 20 named system invariants (§105) the suite exists to defend.

### 2.6 New crates (§69, 70)

`aep-contract` (command/query/type-registry traits, consistency semantics) and `aep-conformance`
(black-box suites), plus a `conformance/` tree and `artifacts/{kinds,relations,templates}`.
`protocol-cli` gains `inspect` and `conformance`.

## 3. Deltas to apply to what already exists

| # | change | reason | size |
|---|---|---|---|
| D1 | add `aep-contract`, `aep-conformance` to the workspace | §69, 70 | small |
| D2 | add `entity` module to `aep-domain`: `EntityId`, `EntityLocator`, `EntityType`, `EntityRef`, `VersionedEntityRef`, `Entity<T>`, `EntityMetadata` | §13–18 | medium |
| D3 | `ActorRef` (`human:alice`, `agent:planning-agent`, `service:release-controller`) alongside the existing `Producer`; `Producer` becomes the evidence-specific view | §37 | small |
| D4 | `Relation` gains first-class identity (`RelationId`, `RelationType`, metadata) and the `Delivers` kind | §21 | small |
| D5 | split the existing `ProtocolEvent` (protocol-execution events) from v0.2's domain `EventEnvelope<E>` (event_id, event_type, subject, entity_revision, command_id, correlation, causation) | §49, 50 | medium |
| D6 | audit records as their own type, distinct from protocol events, including rejected attempts | §52–59 | medium |
| D7 | `Principle` gains a `layer` field (intent / construction / verification / governance) | §26 | small |
| D8 | `submit_evidence` takes an evidence **reference** as well as an inline envelope | §61 | small |
| D9 | protocol declares entity types and relation types, not just capabilities and evidence kinds | §16, 47 | small |

None of D1–D9 invalidates existing tests. D2 and D5 are additive; D3, D4, D7, D9 extend existing
types; D6 is new.

## 4. Order of work

**Status:** steps 1–4 are delivered (waves 1 and 2 — see `docs/plan/`). Step 5 is wave 3, step 6
follows it.


1. **Finish the execution core** — `aep-engine` (registry, resolution, execution, evaluation,
   explanation), the protocol/principle/workflow/profile documents, `protocol-cli`, `xtask schema`.
   Everything above depends on being able to resolve and evaluate a task at all, and none of it is
   invalidated by v0.2.
2. **Entity layer** (D2, D3, D4, D9) in `aep-domain`.
3. **`aep-contract`** — command/query traits, envelopes, context, consistency tokens, idempotency and
   revision-conflict error taxonomy (D5, D6, D8).
4. **In-memory reference backend** behind those traits, so the contract is exercised by something.
5. **`aep-conformance`** — the §78 suites and the §104 end-to-end scenario.
6. **`adp-domain` / `aop-domain`** — development and operations types and commands.

## 5. Deliberate deviations from the documents

Recorded here so they are decisions rather than drift:

1. **`Raw*` types live beside their validated counterparts** in `aep-domain`, not in `aep-schema`
   (§70 assigns "wire representations" to `aep-schema`). Reason: `TryFrom<Raw> for Validated` is the
   semantic-validation step, and splitting it across crates would either duplicate the types or make
   `aep-domain` depend on `aep-schema`. `aep-schema` owns schema generation and document reading, and
   re-exports the wire surface via `aep_domain::raw`.
2. **`ArtifactRelation` is a struct** (`kind` + `target`) rather than a twelve-variant enum. It is
   isomorphic to the document's enum and is what makes `graph.related(id, kind)` expressible.
3. **Three-valued predicate logic.** The documents imply boolean predicates. `Unknown` is added
   because "the tests have not run" and "the tests failed" require different responses from a
   harness; only `True` permits a transition, so no transition is loosened by it.
4. **Ordered scales are declared by the protocol.** `risk >= medium` (§14 of the artifact extension)
   needs an ordering for non-numeric values; inventing lexicographic order silently would make
   `high < low` true.
5. **A rollback failure policy must state its precondition.** `on_failure: rollback` with nothing
   further is rejected at validation time. Reason: a rollback plan that cannot say what it rolls back
   to is not a plan.
6. **An empty test suite is `inconclusive`, not passing.** Zero tests must not read as green.
7. **Evidence-kind and fact-path aliases** (`unit_tests.failed` alongside `tests.unit.failed`,
   `test_execution` alongside `test_result`) are accepted, because both spellings appear in the
   design documents and neither is worth forcing on a document author. Canonical forms are what the
   engine emits.
