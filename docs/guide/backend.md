# Implementing a backend

For someone storing engineering entities — stories, specifications, designs, ADRs, reviews,
approvals — behind the AEP interaction contract. A backend may be a database, a git repository, a
directory of files, a remote API, or several of those at once. The contract defines observable
behaviour only, which is what makes it possible to prove an implementation belongs without reading
its source.

Two traits, and the split is the whole design:

```text
CommandService   every state change, one boundary
QueryService     read-only, never mutates
```

One mutation boundary is what makes validation, authorisation, idempotency, optimistic concurrency,
provenance, events, audit, correlation and causation attach to a single place. A second write path is
a second place to forget every one of them.

## The two traits

`CommandService` has one method.

| Method | Must do |
|---|---|
| `execute(CommandEnvelope<Command>)` | Apply once; return the **original** result for a replay of an already-applied logical command; refuse with `RevisionConflict` when `expected_revision` does not match; leave state unchanged on any error, and still record the refusal |

`QueryService` has seven, and each is a question someone actually asks.

| Method | Answers | Must not |
|---|---|---|
| `get(&EntityRef, QueryConsistency)` | This entity, at least this fresh | hide an archived entity |
| `resolve(&EntityLocator)` | Which identity does `ep://acme/payments/design/passkeys-auth` name? | invent one for an address nothing has |
| `query(&EntityQuery)` | Entities by type, organisation, space, body fields, or relation | accept a filter and ignore it |
| `relations(&RelationQuery)` | Edges from *or* to an entity — "what supersedes this ADR?" is the inverse question and must be askable | answer only one direction |
| `history(&EntityRef)` | Every revision, oldest first, with actor, executor, command and audit id | keep only the latest |
| `audit(&AuditQuery)` | Records by entity, correlation, command, actor, kind, time — or `rejected_only` | omit the refusals |
| `describe_type(&EntityType)` | What a design *is*: mutable or not, which commands may target it, which relations it may have | require the caller to hard-code it |

Both use `async fn` and the contract crate depends on no runtime. A synchronous backend implements
them with futures that complete on the first poll; `aep_contract::testing::block_on` drives one
without pulling an executor into a specification crate.

The skeleton compiles as written:

```rust
use aep_contract::command::{CommandEnvelope, CommandResult, CommandService};
use aep_contract::consistency::QueryConsistency;
use aep_contract::error::{CommandError, QueryError};
use aep_contract::query::{
    AuditQuery, EntityEnvelope, EntityQuery, Page, QueryService, Relation, RelationQuery,
    RevisionRecord,
};
use aep_contract::registry::TypeDescriptor;
use aep_domain::audit::AuditRecord;
use aep_domain::command::Command;
use aep_domain::entity::{EntityId, EntityLocator, EntityRef, EntityType};

pub struct AcmeBackend;

impl CommandService for AcmeBackend {
    type Command = Command;

    async fn execute(
        &self,
        _command: CommandEnvelope<Self::Command>,
    ) -> Result<CommandResult, CommandError> {
        todo!()
    }
}

impl QueryService for AcmeBackend {
    type AuditRecord = AuditRecord;

    async fn get(
        &self,
        _reference: &EntityRef,
        _consistency: QueryConsistency,
    ) -> Result<EntityEnvelope, QueryError> {
        todo!()
    }

    async fn resolve(&self, _locator: &EntityLocator) -> Result<EntityId, QueryError> {
        todo!()
    }

    async fn query(&self, _query: &EntityQuery) -> Result<Page<EntityEnvelope>, QueryError> {
        todo!()
    }

    async fn relations(&self, _query: &RelationQuery) -> Result<Page<Relation>, QueryError> {
        todo!()
    }

    async fn history(&self, _reference: &EntityRef) -> Result<Vec<RevisionRecord>, QueryError> {
        todo!()
    }

    async fn audit(&self, _query: &AuditQuery) -> Result<Page<Self::AuditRecord>, QueryError> {
        todo!()
    }

    async fn describe_type(
        &self,
        _entity_type: &EntityType,
    ) -> Result<TypeDescriptor, QueryError> {
        todo!()
    }
}
```

Entity bodies are untyped (`Entity<Node>`) on purpose: the generic contract moves entities without
knowing what a design is. Deserialise into a domain type in a layer above when you want one.

## The five behaviours that are the point

Everything else is bookkeeping. These are the ones that are easy to get subtly wrong, and each has a
suite named after it.

### 1. A replay applies once

`CommandOutcome` has three values and they are not interchangeable:

| Outcome | Meaning |
|---|---|
| `Accepted` | Applied for the first time. `changed_state()` is `true` |
| `Replayed` | Recognised as a repeat of an already-applied logical command; the **original** result comes back |
| `NoOp` | Accepted, and nothing changed, because the state already matched |

A replay must not advance the revision. From the reference scenario
([`crates/aep-backend-memory/tests/scenario.rs`](../../crates/aep-backend-memory/tests/scenario.rs)):

```rust
assert_eq!(replay.outcome, CommandOutcome::Replayed);
assert_eq!(
    replay.affected, approval.affected,
    "a replay returns the original result rather than a second approval"
);
let unchanged = block_on(backend.get(&design.unversioned(), QueryConsistency::Current))
    .expect("still readable");
assert_eq!(
    unchanged.metadata.revision, approved_at,
    "the replay did not advance the revision"
);
```

Reusing an idempotency key for a *different* command is the opposite case, and must be refused with
`IdempotencyMismatch`. Accepting it silently makes the key useless.

### 2. A stale write is refused, never merged

```rust
let error = block_on(backend.execute(
    envelope("cmd-9", stale, context("req-9", "key-9", 10_000))
        .expecting(EntityRevision::INITIAL),
))
.expect_err("the design has moved on");
match &error {
    CommandError::RevisionConflict { expected, actual, .. } => {
        assert_eq!(expected.get(), 1);
        assert_eq!(actual.get(), 2);
    }
    other => panic!("expected a revision conflict, got {other:?}"),
}
assert_eq!(error.code(), "revision_conflict");
```

Both revisions are in the error because the client has to be able to refetch, decide whether its
intent still holds, and reissue. `is_retryable()` is deliberately `false` for a revision conflict:
reissuing the same command unchanged would assert something that is no longer true.

Without this, two agents working the same design overwrite each other and the second one wins by
accident. That is not a race a person can find afterwards, because the audit trail shows two
successful writes.

### 3. A refusal changes nothing and is still recorded

Both halves. Refuse and forget, and "what did this agent try to do and get stopped from doing?" has
no answer. Refuse and half-apply, and the trail lies.

```rust
let rejections = block_on(backend.audit(
    &AuditQuery::for_correlation(CORRELATION.parse().expect("correlation id")).rejected(),
))
.expect("rejections are queryable");
assert_eq!(rejections.len(), 1, "exactly the one refused command");
let rejection = &rejections.items[0];
assert_eq!(rejection.kind, AuditKind::CommandRejected);
assert!(rejection.change.is_none(), "a refusal changed nothing");
let decision = rejection.decision.as_ref().expect("a refusal has a decision");
assert!(!decision.allowed);
assert_eq!(decision.rule.as_deref(), Some("revision_conflict"));
```

`AuditRecord::validate` rejects a rejection that carries a change record, so a backend that gets this
wrong cannot write the contradiction down.

### 4. Nothing is physically deleted

`ArchiveEntity` and `SupersedeEntity` are the vocabulary; there is no delete command. An archived
entity is still readable, its revision advances like any other change, and the count does not drop:

```rust
let after = block_on(backend.get(&story.unversioned(), QueryConsistency::Current))
    .expect("an archived entity is still readable");
assert_eq!(after.metadata.revision.get(), 2);
let Node::Map(fields) = &after.data else {
    panic!("the body is a mapping");
};
assert_eq!(fields.get("status"), Some(&Node::from("archived")));
assert_eq!(backend.len(), 1, "archiving removes nothing");
```

An engineering record whose history can be erased is not a record. This also rules out the tempting
optimisation of dropping an archived row from an index — "gone from every query" and "deleted" are
the same thing to the person asking why a decision was made.

### 5. Read-your-writes runs on tokens, not sleeps

Every accepted mutation returns an opaque `ConsistencyToken`. A query may demand
`QueryConsistency::at_least(token)`, and the backend must not answer from an older view.

```rust
let after = block_on(backend.get(
    &design.unversioned(),
    QueryConsistency::at_least(approval.consistency.clone()),
))
.expect("read-your-writes");
```

An immediately consistent backend satisfies this for free; a projected one blocks until its
projection catches up; neither has to say which it is. A token it cannot reach is
`QueryError::ConsistencyTimeout`, not a stale answer:

```rust
let from_elsewhere =
    aep_contract::consistency::ConsistencyToken::new("seq-999999999999").expect("token");
let error = block_on(backend.query(
    &EntityQuery::default().with_consistency(QueryConsistency::at_least(from_elsewhere)),
))
.expect_err("this store has not reached that point");
assert_eq!(error.code(), "consistency_timeout");
```

Nothing outside the backend that issued a token may interpret its contents. It is passed back, not
read. The reason the contract has tokens at all is that a conformance suite cannot sleep: a suite
that sleeps tests the machine it runs on, the first slow CI box turns a correct backend red, and the
fix everyone reaches for is a longer sleep.

## Proving it

Sixteen suites, three levels, one call. Point them at your backend from your own test suite:

```rust
use aep_backend_memory::MemoryBackend;
use aep_conformance::{run, Level};

let report = run(&MemoryBackend::new(), Level::Full);
assert!(report.passed(), "{report}");
```

From the command line, against the reference backend:

```console
$ protocol conformance --level full
identity — 8/8 properties hold
  ✓ a created entity is addressable by the identity it was given
  ...
conformance full: 89 properties hold
```

And the check that matters most — that the suites catch anything at all:

```console
$ protocol conformance --suite idempotency --inject replay-applies
idempotency — 1/6 properties hold
  ✗ a replay does not advance the revision — the command left the entity at revision 2, and after
    replaying it the entity is at revision 3
  ...
conformance full: 5 of 6 properties do not hold
injected fault: a replayed command is applied a second time — expected to be caught by the
`idempotency` suite
$ echo $?
1
```

`run_suite(&backend, "idempotency")` re-runs one suite for a targeted check. Anything implementing
both traits over the AEP command and audit vocabulary satisfies the `Backend` bound automatically —
a backend that implements only half the contract fails to *compile* against the suites, rather than
failing them at run time with a confusing message.

A `SuiteReport` names the **property** that failed, not the assertion that fired:

```text
idempotency — 1/2 properties hold
  ✓ a replay returns the original result
  ✗ a replay applies the command once — the entity advanced to revision 3 after replaying a command that produced revision 2
```

"A replay applied the command twice" tells an implementer what to fix. "Assertion failed at line 214"
does not. `ConformanceReport` aggregates: `passed()`, `checks()`, `failures()`, `failing_suites()`.

Levels exist so a backend can be useful before it is complete, and so what it claims is checkable
rather than asserted in a README. They are ordered, and `Level::includes` is how a claim is checked.

| Level | Covers | Enough to |
|---|---|---|
| `Core` | identity, commands, idempotency, concurrency, query, consistency, relations | store and retrieve work without losing it |
| `Audited` | core, plus history, immutability, audit, rejected-audit, correlation, causation, provenance | reconstruct what happened |
| `Full` | everything, plus events and the type registry | run an engineering protocol against |

`aep_conformance::suites::all()` is the registry — name, one-line summary, and the weakest level that
requires each suite. Read it rather than this table if the two ever disagree.

## Why the suite ships a broken backend

`FaultyBackend` wraps a working backend and injects exactly one observable misbehaviour. The crate's
own tests then assert that the suite responsible for that property fails — *and* that the others
still pass, so a fault does not simply break everything.

```text
Fault::ReplayApplies          →  the idempotency suite must fail
Fault::IgnoreExpectedRevision →  the concurrency suite must fail
Fault::DropRejectionAudit     →  the rejected-audit suite must fail
```

Each fault perturbs only what goes in and what comes out, which is the same position a real backend's
clients are in. `Fault::caught_by()` names the suite that must catch it, and `Fault::describe()` says
what goes wrong in one line.

The point generalises past this crate. **A suite that passes everything is not evidence.** Nothing
about reading a test suite tells you whether it would catch anything; the only way to find out is to
hand it something wrong and watch it complain. If you write suites of your own for a backend
extension, write the broken backend too — otherwise a green run means the tests ran, not that the
behaviour holds.

## The reference to diff against

[`crates/aep-backend-memory/`](../../crates/aep-backend-memory/) is the known-good implementation: a
few `BTreeMap`s behind one lock, no persistence, no cleverness. It exists so the contract is exercised
by something real, and so a conformance suite has something to check itself against.

| File | What to copy from it |
|---|---|
| `src/command.rs` | the shape of one mutation boundary: idempotency, revision check, audit, events |
| `src/query.rs` | how filters, paging and consistency waits compose |
| `src/store.rs` | what has to be kept per entity — revisions, provenance, applied commands |
| `tests/scenario.rs` | the 19-step reference scenario, which is the fastest way to find out what you have not implemented |

You can watch it work without writing any code — the CLI seeds an in-memory backend from an artifact
manifest and answers against it:

```console
$ B=target/debug/protocol
$ $B entity list --artifacts examples/development-passkeys/artifacts.yaml
01MEM0000000000000001  aep.architecture-decision-record/v1  ep://local/manifest/architecture-decision-record/0042   r1
01MEM0000000000000004  aep.design/v1                        ep://local/manifest/design/passkeys-auth                r1
01MEM0000000000000007  aep.product-requirements/v1          ep://local/manifest/product-requirements/passkeys       r1
01MEM0000000000000010  aep.review-result/v1                 ep://local/manifest/review-result/design-passkeys-auth  r1
01MEM0000000000000013  aep.specification/v1                 ep://local/manifest/specification/passkeys-auth         r1
01MEM0000000000000016  aep.story/v1                         ep://local/manifest/story/AUTH-141                      r1
```

And `describe_type` is what stops a harness hard-coding what a design is:

```console
$ $B describe --artifacts examples/development-passkeys/artifacts.yaml aep.review-result/v1
type       aep.review-result/v1
summary    An artifact of kind `review-result`.
mutable    no
commands
  aep.entity.update/v1         revision-guarded  Change fields of the entity.
  aep.entity.archive/v1        revision-guarded  Archive the entity; nothing is deleted.
```

```console
$ $B describe --artifacts examples/development-passkeys/artifacts.yaml aep.design/v1
type       aep.design/v1
summary    An artifact of kind `design`.
mutable    yes
commands
  aep.entity.update/v1         revision-guarded  Change fields of the entity.
  aep.entity.archive/v1        revision-guarded  Archive the entity; nothing is deleted.
  aep.design.approve/v1        revision-guarded  Approve the design against a review of this revision.
```

A design may change; a review result may not. A review someone can edit after the fact is not
evidence, and a backend that lets them edit it has removed the reason the review existed.
