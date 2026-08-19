# Wave 2 — identity, the interaction contract and a reference backend

Goal: **make the protocol's objects addressable and its mutations auditable.** After wave 1 the
engine can decide; it cannot yet say *which* design was approved, by whom, on whose behalf, in which
revision, or prove that a retried command did not create a second one.

Wave 1 delivered the decision layer. This wave delivers the layer underneath it, from
`docs/design/consolidated-design-v0.2.md` §13–18 (entities), §34–47 (command and query), §49–59
(events and audit).

Projected status: **≈62% → ≈85%.** Conformance (§75–86) and the ADP/AOP domain types are wave 3.

## Why this order

The artifact graph today is keyed by a human-readable id (`design:passkeys-auth`) that lives in one
manifest. That is enough to answer "is there an approved design" and not enough for anything a real
organisation does with the answer:

* two repositories cannot refer to the same design;
* an approval cannot be pinned to a revision that exists independently of the file's contents;
* a retried "approve this design" command cannot be recognised as the same command;
* nothing records that an agent *tried* to write production and was refused.

Every one of those is an identity or a mutation-boundary problem, so identity comes first.

## Dependency order

```text
W2.1 entity layer ──▶ W2.2 aep-contract ──┬─▶ W2.4 events + audit ──┐
                                          └─▶ W2.3 commands ────────┼─▶ W2.5 in-memory backend
                                                                    │
W2.6 engine integration (needs W2.4) ───────────────────────────────┘
W2.7 CLI surface (needs W2.5)
```

---

## W2.1 Entity layer — `aep-domain::entity`

| Type | Shape | Why it is not what we have |
|---|---|---|
| `EntityId` | opaque, ULID-shaped | `AUTH-142` is a *key* people type, not identity. Two systems can hold the same key for different things. |
| `EntityLocator` | `ep://acme/payments/story/AUTH-142` | a logical address that resolves through a connector; not a storage URL |
| `EntityType` | `aep.design/v1` — namespace, name, version | the type decides schema, legal commands, lifecycle and allowed relations; today those are hard-coded per Rust type |
| `Entity<T>` / `EntityMetadata` | id, locator, type, revision, timestamps, provenance | gives every artifact a revision, which is what an approval pins to |
| `EntityRef` vs `VersionedEntityRef` | "current X" vs "exactly revision N of X" | `ReviewResult::covers` currently compares version *labels*; this makes it structural |
| `ActorRef` | `human:alice`, `agent:planning-agent`, `service:release-controller` | `Producer` answers "what produced this evidence"; `ActorRef` answers "on whose behalf", which is a different question and the one audit needs |
| `Relation` | `RelationId`, type, source, target, metadata | edges become addressable, so a relation can itself be created and removed by command |

`ArtifactId` stays, as the locator key. The artifact graph keeps working; it gains identity and
revision underneath.

Acceptance: ~25 tests. `ArtifactRef` round-trips to `VersionedEntityRef`; the existing artifact tests
still pass unchanged.

## W2.2 `aep-contract` — the interaction traits

```text
command/       CommandEnvelope, CommandContext, CommandResult, CommandService
query/         QueryService: get, resolve, query, relations, history, audit, describe_type
consistency/   ConsistencyToken, QueryConsistency::{Current, AtLeast(token)}
error/         the typed taxonomy: RevisionConflict, NotFound, Unauthorised, Invalid, Conflict
registry/      TypeDescriptor: schema, lifecycle, legal commands, allowed relations, mutability
```

`CommandContext` carries the six identifiers that make an audit trail reconstructable — `request_id`,
`command_id`, `idempotency_key`, `actor`, `executor`, `correlation_id` — plus `causation`,
`execution_id` and the task. The distinction that earns its keep: **actor** is who authorised,
**executor** is what ran. `actor: human:alice, executor: agent:release-agent-17` is the normal case,
and an audit trail that collapses them cannot answer either question.

No implementation, no async runtime opinion: traits and types only.

Acceptance: ~20 tests over envelope construction, context validation and the error taxonomy.

## W2.3 Commands — `aep-domain::command`

Generic: `CreateEntity`, `UpdateEntity`, `CreateRelation`, `RemoveRelation`, `ArchiveEntity`,
`SupersedeEntity`. Domain: `SubmitDesignReview`, `ApproveDesign`, `AcceptAdr`.

The point of a domain command over a generic patch: `ApproveDesign { review }` can be *validated* —
the review exists, it targets this design's current revision, its disposition is approval, the actor
holds the capability, the workflow permits it. `PATCH status = "approved"` can be validated for none
of that.

No physical delete anywhere: `ArchiveEntity` and `SupersedeEntity` are the vocabulary, because an
engineering record whose history can be erased is not a record.

Acceptance: ~20 tests, including one per validation an `ApproveDesign` performs.

## W2.4 Events and audit — `aep-domain::{event, audit}`

Two streams, deliberately separate:

* **Domain events** (§49–50) — `EventEnvelope<E>` with `event_id`, `event_type`, subject,
  `entity_revision`, `command_id`, correlation, causation, provenance. Facts about what occurred.
* **Audit records** (§52–59) — `AuditRecord` with actor **and** executor, decision records, change
  records (before/after revision), and **rejected attempts**.

The existing `ProtocolEvent` becomes the execution-scoped stream it already is, and gains a mapping
into audit records so a protocol refusal — `production.write denied, rule
production-write-requires-approval` — is queryable next to the mutations it prevented.

Acceptance: ~20 tests. The one that matters: a denied command produces an audit record and no state
change.

## W2.5 In-memory reference backend — `crates/aep-backend-memory`

A complete implementation of both traits over `BTreeMap`s, so the contract is exercised by something
before anyone builds a real one. Deliberately boring and deliberately correct: idempotent replay
returns the original result, `expected_revision` conflicts are typed, and `AtLeast(token)` is
satisfied trivially because the backend is immediately consistent.

Acceptance: ~30 tests, including the §104 end-to-end scenario — create story, specification, design,
relate them, review at a revision, approve, query relations, query history, query audit by
correlation, replay the approval idempotently, attempt a stale-revision update, confirm the typed
conflict and the rejected-attempt audit record.

## W2.6 Engine integration

`submit_evidence` accepts an `EntityRef` to an evidence entity as well as an inline envelope (§61).
Protocol decisions emit audit records. `Execution` carries a `CommandContext` so everything it does
inherits correlation and causation.

Acceptance: ~10 tests; the existing engine tests keep passing.

## W2.7 CLI

`protocol entity get|resolve|history`, `protocol audit --correlation <id>`, and `protocol command`
for the generic commands, all against the in-memory backend loaded from a manifest.

Acceptance: ~8 integration tests.

---

## Out of scope for this wave

`aep-conformance` and its fixtures (§78, §104 as a *reusable* suite rather than one test),
`adp-domain` and `aop-domain` types, any persistent backend. Wave 3.

## Risks

| Risk | Handling |
|---|---|
| The entity layer duplicates the artifact graph rather than sitting under it | `ArtifactId` becomes a locator key and the artifact tests stay unchanged; if they need editing, the layering is wrong |
| The contract acquires an async runtime opinion | traits are defined with `async fn` in trait, no executor dependency; the in-memory backend is synchronous under it |
| Audit and events drift into one another | one rule: an **event** is what happened, an **audit record** is who caused it and what was decided; a denied command has an audit record and no event |
| The command surface grows per artifact kind | generic commands cover the graph; a domain command is added only where it validates something a generic update cannot |
