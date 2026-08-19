# Changelog

Notable changes to `engineering-protocols`. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), where a **major**
version is a breaking change to a protocol's semantics, not merely to a Rust API.

Entries record what changed for someone using the protocol. Rationale that does not fit in a line
belongs in the commit message or in `docs/design/`.

## [Unreleased]

Nothing yet.

## [0.2.0-wave-3] — 2026-08-20

### Added

- **`aep-conformance`** — sixteen black-box suites a backend runs against itself to prove it
  implements the contract: identity, command execution, idempotency, optimistic concurrency, query,
  consistency, relations, history, immutability, audit, rejected-action audit, correlation, causation,
  provenance, events and type discovery. Reports name the *property* that failed, not the assertion,
  so a failure says what to fix.
- **Conformance levels** — `core`, `audited`, `full`. A backend states what it claims and the suite
  proves or refutes it, instead of a README asserting it.
- **`FaultyBackend`** — a wrapper that breaks exactly one property at a time. The crate's own tests
  assert that the suite responsible for each fault fails and the others still pass, because a suite
  that passes everything tells you nothing about whether it would catch anything.
- **`protocol conformance --level core|audited|full [--suite <name>] [--inject <fault>]`** — runs the
  suites, and can deliberately break a property to show which suite catches it.
- **`adp-domain`** — development types (`adp.specification/v1`, `adp.test-plan/v1`,
  `adp.acceptance-criteria/v1`, `adp.change/v1`) and commands (`adp.story.start/v1`,
  `adp.story.complete/v1`, `adp.test-plan.record/v1`, `adp.specification.satisfy/v1`). A
  specification declared satisfied by no evidence is refused — the exact claim the protocol exists to
  stop.
- **`aop-domain`** — operations types (`aop.incident/v1`, `aop.runbook/v1`, `aop.release/v1`) with
  their status ladders, and commands (`aop.incident.acknowledge|mitigate|resolve/v1`,
  `aop.release.promote|rollback/v1`). Promoting to production without naming an approval is refused
  at the command, which is a second defence beside the protocol's approval floor.
- **`docs/guide/`** — how to adopt the protocol, wire a harness to the engine, and implement and
  prove a backend.
- `Fault::caught_by()` names the suite responsible for each fault, and the crate's own tests assert
  that suite fails when the fault is injected. `DropAffected` fails eight suites, which is a finding
  about how load-bearing `affected` is rather than a flaw in the suites, and is recorded as such.

### Changed

- The in-memory backend now **refuses an update to an immutable type**. A review result records what
  someone concluded at a moment; editing it afterwards changes what the record says a person decided.
  Archiving stays available — keeping a record and editing it are different acts.

## [0.2.0-wave-2] — 2026-08-20

### Added

- **Identity.** Every addressable thing now has an opaque `EntityId`, a logical `EntityLocator`
  (`ep://acme/payments/design/passkeys-auth`), a versioned `EntityType` (`aep.design/v1`) and a
  monotonic `EntityRevision`. `AUTH-142` is a key in a locator, not identity — so two repositories can
  refer to the same design, and an approval can name the exact revision it approved.
- **`ActorRef`** — `human:alice`, `agent:planning-agent`, `service:release-controller`, `system`.
  Distinct from an evidence `Producer`: an actor bears responsibility, a producer made an observation.
  Commands carry both an actor and an executor, so "alice authorised it, agent-17 ran it" is
  answerable, and a trail that collapses them can answer neither question.
- **`aep-contract`** — the storage-independent interaction contract: `CommandService` and
  `QueryService`, command envelopes with the six identifiers that make a trail reconstructable,
  consistency tokens giving read-your-writes without sleeps, a typed failure taxonomy, and
  `TypeDescriptor` so a harness can ask what a design is instead of hard-coding it.
- **Commands** (`aep-domain::command`) — six generic (`CreateEntity`, `UpdateEntity`,
  `CreateRelation`, `RemoveRelation`, `ArchiveEntity`, `SupersedeEntity`) and three domain
  (`SubmitDesignReview`, `ApproveDesign`, `AcceptAdr`). A domain command can be validated where a
  generic patch cannot: `ApproveDesign{design@7, review}` checks that the review is about *that*
  revision.
- **Domain events** (`aep-domain::domain_event`) — a versioned event vocabulary with an open
  `Custom` variant, separate from the protocol's execution events. An event caused by a command
  names that command as its cause.
- **Audit records** (`aep-domain::audit`) — actor and executor, correlation and causation, decision
  records and change records with before/after revisions, and **rejected attempts**: a denied command
  changes nothing and still leaves a record, which is the half most systems lose.
- **`aep-backend-memory`** — a complete in-memory implementation of both contract surfaces, so the
  contract is exercised by something before anyone builds a durable backend. It passes the
  specification's nineteen-step reference scenario, including idempotent replay, stale-revision
  conflicts and the audit record a refusal leaves behind.
- **`aep-engine::trail`** — protocol decisions become audit records, and a command issued during an
  execution inherits its correlation, execution and task. A refusal by the protocol and a refusal by
  a backend now land in the same trail, queryable the same way.
- Evidence may be submitted as an entity reference, so the trail points at the stored evidence rather
  than at the engine's copy of it.
- `RelationKind::Delivers`, and `ArtifactKind::entity_type()` mapping the human-facing artifact
  vocabulary onto entity types.
- **CLI**: `protocol entity list|get|history|relations`, `protocol audit [--correlation|--entity|
  --rejected]` and `protocol describe <type>`, backed by an in-memory backend seeded from an artifact
  manifest through real commands — so seeding produces history and audit records like anything else.

### Changed

- **Nine new `ValidationCode`s** — `self_reference`, `empty_change`, `refusal_mutated_state`,
  `unreconstructable_change`, `unexplained_decision`, `redaction_inconsistent`,
  `event_payload_mismatch`, `incomplete_event_subject`, `missing_causation`. Previously these
  failures all reported `unknown_state`, so a caller could not tell "this audit record claims a
  refusal changed something" from "this workflow references a state that does not exist".
- Minimum supported Rust version is 1.85 (`Waker::noop`, which lets the contract define `async fn`
  traits without an executor dependency or a line of `unsafe`).
- A protocol may declare an **approval floor** — capabilities no profile may grant outright.
  `aep/1` declares `production.write` and `deployment.create:production`, and a profile that grants
  one fails to resolve.

## [0.2.0-wave-1] — 2026-08-20

### Added

- **The execution core.** `aep-engine` resolves a task against a document tree and answers what is
  owed, what may be done, which transitions are permitted and whether the task is complete:
  - `registry` — the documents in force, with the cross-document checks (unknown references, pinned
    version mismatches, undeclared capabilities and evidence kinds, evidence no verifier can
    establish);
  - `load` — reads a document tree, reporting every bad file with its path rather than the first;
  - `resolve` — task + registry → execution plan: `extends` chains merged, principles filtered by
    applicability, capabilities composed with the document responsible recorded for each entry,
    obligations collected, and the whole configuration checked for rules that could never fire;
  - `execution` — live state with derived facts (`evidence.first_seq.*`, `test.first_result`,
    `evidence.missing`) and a serialisable snapshot;
  - `evaluate`, `policy`, `explain` — what is owed, capability decisions naming the rule that
    decided, and the `✓ / ✗ / ?` completion checklist;
  - `engine` — the `ProtocolEngine` trait, deterministic transitions, an injected `Clock`.
- **The documents.** 42 of them: `aep/1` plus `adp/1` and `aop/1`; 21 principles across intent,
  construction, verification and governance; 4 workflows (development, incident, progressive release,
  forward-only migration); 5 profiles; 5 artifact lifecycles; artifact kind and relation definitions;
  8 templates.
- **`protocol` CLI** — `validate`, `resolve`, `inspect`, `evaluate`, `explain`, `schema`, with
  `--format text|yaml|json`.
- **Worked example** (`examples/development-passkeys/`) — a task, its artifact graph and a five-step
  evidence sequence that walks to completion, replayed by the integration tests.
- **Protocol approval floor.** A protocol may declare capabilities no profile can grant outright;
  `aep/1` declares `production.write` and `deployment.create:production`. A profile that grants one
  fails to resolve.
- **`Action::ProductionMutate`** — production changes that are not deployments now have an action, so
  a policy naming only deployments cannot let them through.
- **CI** — GitHub Actions mirroring `task check`, with schema drift as its own job.

### Fixed

- `evidence.missing` counted evidence required by conditional rules that did not apply, so a task
  could show every requirement met and still be unable to finish.
- The approval floor is now violated by any *overlap*: granting `deployment.create` for every
  environment no longer slips past a floor on `deployment.create:production`.
- A task may name the base protocol its profile refines (`aep/1` with a profile written against
  `adp/1`), which is the form the design documents use.

### Changed

- Evidence files spell the envelope's subject `about`, not `subject`, so it cannot silently consume a
  payload's own `subject` — a review's subject is the artifact reviewed.
- `protocol evaluate` exits `0` whenever it produced a report. A blocked execution is an answer, not
  a failure; `explain --action` still exits `1` when an action is refused.

## [0.1.0] — 2026-08-19

### Added

- **`aep-domain`** — the source-of-truth model: identifiers and versioned references, a three-valued
  predicate language, facts and ordered scales, capabilities with default-deny, actions, evidence with
  provenance, verifiers and counterexamples, the artifact graph with lifecycles and typed relations,
  review semantics with revision-bound approval, requirements over evidence/artifacts/reviews/
  approvals/conditions, principles with phase-timed obligations, workflows, tasks, protocols,
  profiles, execution plans and the audit event vocabulary.
- **`aep-schema`** — document reading that separates syntax from semantic failure, and JSON Schema
  generation for six document types and four interchange types.
- **`xtask schema [--check]`** — schemas are generated from the Rust types, and CI proves they match.
- Repository scaffolding: workspace, `Taskfile.yml` gate, Apache-2.0 licence, `AGENTS.md`.

[Unreleased]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-3...HEAD
[0.2.0-wave-3]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-2...0.2.0-wave-3
[0.2.0-wave-2]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-1...0.2.0-wave-2
[0.2.0-wave-1]: https://github.com/codewandler/engineering-protocols/compare/0.1.0...0.2.0-wave-1
[0.1.0]: https://github.com/codewandler/engineering-protocols/releases/tag/0.1.0
