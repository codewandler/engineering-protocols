# Changelog

Notable changes to `engineering-protocols`. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), where a **major**
version is a breaking change to a protocol's semantics, not merely to a Rust API.

Entries record what changed for someone using the protocol. Rationale that does not fit in a line
belongs in the commit message or in `docs/design/`.

## [Unreleased]

### Added

- **A system can be specified, and the specification can be refused.** `ess-domain` is the typed
  model for an Executable System Specification: domains, entities with lifecycles, commands with
  outcomes, events, errors, views with declared consistency, actors and a type system with tagged
  unions. `protocol ess validate --path <file-or-directory>` parses one and reports every problem in
  a single run, each with a code and a location.
- **[`examples/billing/`](examples/billing/)** — the single normative example, parsed by a test, and
  checked to exercise *every* construct the model has: each type kind, each primitive,
  `Optional`/`List`/`Map`, both consistency levels, an actor with grants and one without. A construct
  added to the model without reaching the example fails the build, because what the normative example
  leaves out is what nothing checks.
- **A command that can be refused says so.** Outcomes rather than a bare `emits` list: a command with
  a precondition has at least two results, and a specification recording only the happy one generates
  a suite that never checks the branch where the money does not move.
- **An outcome the input cannot decide says that too.** `external: <the cause>` marks a branch caused
  by the world — a mail provider rejecting an address — so a generator injects a fault instead of
  trying to construct an input for it. `when: false` would have claimed the branch was unreachable,
  which is a different and false statement.
- **A projection declares its consistency**, which is what decides whether a generated assertion is
  `eventually` or immediate — rather than a sleep, which makes a suite test the machine it runs on.
- **A declaration is addressable from outside** — `ep://acme/billing/ess-command/billing.invoice.CreateInvoice`,
  the protocol's own scheme rather than a new `ess://` one, so an approval against a command in a
  specification is recorded the same way as an approval against a design.
- **[`schemas/generated/ess.schema.json`](schemas/generated/ess.schema.json)** — an editor validates
  a specification as it is typed. Generated from the same Rust types the validator runs, drift-checked
  in CI, and the generated index now lists every published schema so one cannot land undocumented.
- **[`docs/guide/specification.md`](docs/guide/specification.md)** — how to write one, and what the
  model insists on.
- **[`docs/VISION.md`](docs/VISION.md)** — what this project is for, and how its two halves compose:
  AEP governs how engineering work is performed, ESS specifies what software must exist, and they
  meet at evidence.
- **[`docs/design/ess-implementor-design-v0.1.md`](docs/design/ess-implementor-design-v0.1.md)** —
  the Executable System Specification design: a system described once as a typed semantic model, from
  which contracts, documentation, tests, deployment artifacts and structural code are derived.
- **[`docs/design/ess-review-v0.1.md`](docs/design/ess-review-v0.1.md)** — a review of that design
  against what this repository learned building the same shape twice: eleven findings, three of which
  would make generated tests assert false things, and a narrower recommended v0.1 scope.
- **A task can require conformance to a specification.** `ArtifactKind::ExecutableSystemSpecification`,
  `EvidenceKind::EssConformance` and the `ess-conformance` principle — conditional on the project
  having a specification, and satisfied only by `independent: true` evidence from a
  `conformance-runner`. An agent's own report that its implementation matches the specification is
  not evidence that it does.

### Changed

- **A validation error names what actually went wrong.** A specification had been borrowing the
  protocol's document codes, so a duplicated command name reported `duplicate_principle` and a
  missing event reported `unknown_state`. Nine codes now say what they mean —
  `undeclared_reference`, `duplicate_declaration`, `missing_declaration`, `empty_declaration`,
  `conflicting_declaration`, `type_mismatch`, `unsupported_format_version`,
  `non_exhaustive_branches`, `unreachable_branch` — and sixteen places in the protocol half moved
  onto them too, so an undeclared reference is not one code in a specification and a different one
  in an artifact manifest.
- **The published schemas accept what the parser accepts.** Ten document types had a hand-written
  parser and a derived JSON Schema, so the schema described the *representation* rather than what an
  author writes: a bare `- verification` evidence requirement, a one-line objective, a
  `require_approval` capability, an `in-review` status. Twenty-eight rejections across eighteen of
  this repository's own documents. Every schema is now checked against every document the repository
  ships.
- `v01` and `ess/01` are refused. Both parsed, and both were rejected by the pattern the same build
  published — a document an editor called invalid and the tool accepted.

### Fixed

- **A schema that called the normative example invalid.** `version: v3` is what every document says;
  the published schema required an integer.
- **A guard that could not guard.** The list of validation codes the tests iterate was maintained by
  hand and had fallen five codes behind the enum, while its own comment claimed that adding a variant
  without listing it would fail the test. The enum, its wire strings and the list are now generated
  from one declaration.
- Rules that existed and were never reached: an error's payload types and an event's duplicate fields
  were checked by methods nothing called.
- A specification could name a domain in the header that nothing declares, declare an actor no domain
  owns, define two types that cannot be built without each other, filter a view on a lifecycle state
  the entity does not have, declare a type no value can be, or declare a union with no tag field. All
  six are refused.
- **A misspelt key in a type declaration was silently dropped.** `invarants:` on a value object
  parsed clean and lost the invariant, because a flattened body rules out `deny_unknown_fields` at the
  outer level. It is now a parse error with a line number.
- **A type's invariants are predicates, checked against the type's own fields**, as an entity's
  already were. `nonexistent_field >= 0` on a value object was accepted, and so was text that is not
  a predicate at all.
- A field name must survive into generated code as an identifier. `""` and `not a field name!` were
  accepted.
- An entity invariant may read the identity field. It could not, although a view projecting the same
  entity could — so a valid specification was refused with a message that was not true.
- A field may not shadow the identity's name, which produced two fields with one name and different
  types.
- A state whose only transition returns to itself is a dead end. A self-loop was counted as an exit,
  so an entity could reach a state it can never leave.
- A domain can be given a wire and display name. `naming:` on a domain file was refused, although the
  model has always carried it — so a bounded context's wire name was unreachable from any document.
- A malformed header no longer hides the reference errors under it.
- `protocol ess validate` names the file a problem is in when the specification is one file, refuses
  a directory that is not a specification instead of reading every YAML file it can find, and reads
  each file once when a symlink points back up the tree.
- `cargo xtask schema --check` fails on a schema nothing generates any more, not only on one that
  drifted.

### Not built

No compiler, no OpenAPI, no test synthesis: those are ESS waves 2 and 3 in
[`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md). Conformance evidence is produced by hand.

## [0.2.1] — 2026-08-20

### Added

- **A project can be discovered.** `.engineering/project.yaml` names the protocol, the profile and
  where the protocol tree lives; `protocol resolve` and `protocol evaluate` run with no arguments
  anywhere inside a project, walking up to find it. An adopting team's first command no longer needs
  four paths.
- **Project-local principles and profiles.** `.engineering/principles/` and `.engineering/profiles/`
  are merged over the protocol tree's, because no organisation's rules are entirely somebody else's.
  They are documents in the same format, validated the same way — and a project-local profile still
  cannot grant a capability the protocol's approval floor keeps behind approval.
- `protocol resolve` and `protocol evaluate` report where their inputs came from, so it is never
  ambiguous whether a flag or the project supplied them.

### Fixed

- **The approval floor was inert for every `adp/1` and `aop/1` profile.** `Protocol::extend` merged
  capabilities, evidence kinds, verifiers, phases, observables and scales — but not the approval
  floor, and neither derived protocol declares one of its own. A profile written against `adp/1`
  could therefore grant `production.write` outright and resolution would accept it, while three
  documents claimed that was impossible. The shipped profiles were unaffected because each
  hand-writes `require_approval`; the check meant to make the mistake impossible was doing nothing.
  Now inherited, with a regression test over the real documents that fails without the fix.
- **The CLI crashed when its reader stopped reading.** `protocol inspect | head -3` ended in a panic
  and a stack trace, because Rust's `println!` panics on a closed pipe. Output now ends quietly.

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

[Unreleased]: https://github.com/codewandler/engineering-protocols/compare/0.2.1...HEAD
[0.2.1]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-3...0.2.1
[0.2.0-wave-3]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-2...0.2.0-wave-3
[0.2.0-wave-2]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-1...0.2.0-wave-2
[0.2.0-wave-1]: https://github.com/codewandler/engineering-protocols/compare/0.1.0...0.2.0-wave-1
[0.1.0]: https://github.com/codewandler/engineering-protocols/releases/tag/0.1.0
