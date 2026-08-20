---
title: Where this stands
sidebar_position: 1
description: What is implemented, what is in progress, and what is not built — with the numbers the repository's own gate reports.
---

# Where this stands

Honest as of the tag `0.6.1-ess-wave-6.5`. The repository's gate, `task check`, runs eight steps —
formatting, clippy with warnings as errors, the test suite, rustdoc with warnings as errors, a schema
drift check, a projection drift check, a conformance-suite drift check and a synthesis check that
regenerates the generated workspaces, compiles them and runs the committed suite against them — and
reports **69 suites and 1397 tests, with 0 clippy warnings and 0 rustdoc warnings**.

Nothing lands that does not pass all eight, and CI runs the same eight.

## AEP — the protocol

The v0.2 scope is complete: a task resolves against the document tree, evidence decides what may be
done and whether the work is finished, every object it touches has identity, revision and an audit
trail, and a backend can prove it implements the contract by running a suite against itself.

| | State |
|---|---|
| domain model, engine, documents, CLI | implemented |
| interaction contract, identity, audit | implemented, with an in-memory reference backend |
| conformance suites for backends | implemented — 16 suites, 3 levels, checked against a deliberately broken backend |
| running on a real project | in progress: a project can now be discovered, but nothing has been governed by it yet |

The document tree is 39 files — 3 protocols, 22 principles, 4 workflows, 5 profiles and 5 artifact
lifecycles — each validated against the protocol vocabulary in CI, and 12 generated JSON Schemas that
CI fails on if they drift from the Rust types.

## ESS — executable system specifications

Six waves delivered, and the hardening batch behind the sixth. A specification is now the source of its own documentation, its own
contracts, its own tests and the structural part of its own implementation — and when it changes,
the change itself is a typed, queryable object.

| | State |
|---|---|
| the typed model | implemented, `0.3.0-ess-wave-1` |
| resolution, IR and diagnostics | implemented, `0.3.1-ess-wave-2` — an unresolved reference is unrepresentable |
| four projections: docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 | implemented, `0.3.2-ess-wave-3` — output committed and drift-checked |
| the model reconciled before the oracle was built | implemented, `0.3.3-ess-wave-3.5` — 20 gates closed before the oracle was allowed to start |
| the join: artifact kind, evidence kind, the `ess-conformance` principle | implemented — a task can be blocked until something proves conformance |
| the specification as an oracle: generated suites, a conformance runner, evidence | implemented, `0.4.0-ess-wave-4` — 27 scenarios from the normative example and 31 from a second fixture at that tag, committed and drift-checked |
| what a change to a specification invalidates | implemented, `0.5.0-ess-wave-5` — a semantic delta over six construct families, and an impact closure that narrows what is owed and can never say "still valid" |
| generated Rust code — structural synthesis | implemented, `0.6.0-ess-wave-6`, **first slice as scoped**: a language-neutral plan giving every capability exactly one disposition — on billing, 45 capabilities: 33 generated, 8 obligations, 4 refused — and a committed zero-dependency Rust workspace: semantic types, typestate lifecycles whose illegal transitions do not compile, component ports, one transport. The committed wave-4 suite, unchanged, passes 27 of 27 against it linked with hand-written obligations, and fails a deliberately corrupted linkage at exactly the scenario that exists to catch it. Behaviour is never generated: every algorithm is a named obligation, yours to implement |
| the hardening batch — the claims made mechanical | implemented, `0.6.1-ess-wave-6.5` — three invariants that were enforced by nothing are now enforced by build-failing scans, the model digest is the full SHA-256, `proptest` phase 1 landed, an outcome can declare where an event's payload comes from (the one fault that was caught by nothing is now caught), and value-object invariant scenarios grow the billing suite 27→29 with its refusal count at zero |

Building the oracle changed the specification language four times, and that is the part worth
knowing. Two of the four gaps were found by review before a line of the synthesizer existed; the
other two were found by the synthesizer **refusing to generate a scenario** and saying which construct
it could not test. Every one of the four had been rendered without complaint by all four projections,
because a document does not have to run. `docs/plan/ess-wave-4-the-oracle.md` in the repository is
the full account, including what the wave did not close — and the wave-6 story repeated the pattern
at the next layer: the generated suite caught a real defect in the code generator before any human
did, a delivery policy that conflated a declared refusal with an unmet obligation.

## What works today

* **A task resolves into a plan**: `extends` chains merged, principles filtered by whether they
  apply, capabilities composed with the document responsible recorded for every entry, obligations
  collected — and the whole configuration refused if any rule in it could never fire.
* **Evidence drives transitions.** A workflow advances when the evidence satisfies the guard, and the
  refusal names what is missing.
* **Ordering is checkable.** Red-before-green is enforced as a fact about submission order.
* **An agent cannot verify itself.** `independent: true` is not satisfied by the agent's own report
  of a green suite.
* **An approval names the revision it approved**, so an approval of version 3 stops satisfying a
  review requirement once the design is at version 7.
* **Every decision is explainable**, as a `✓ / ✗ / ?` checklist or as a machine-readable refusal.
* **Every mutation goes through one boundary**, carrying actor and executor, correlation and
  causation, an idempotency key and an asserted revision.
* **Nothing is deleted.** `ArchiveEntity` and `SupersedeEntity` are the vocabulary.
* **A backend can prove it conforms**, against suites that are themselves checked against a
  deliberately broken backend — because a suite that passes everything tells you nothing.
* **A specification compiles** into documentation, JSON Schema, OpenAPI 3.1 and AsyncAPI 3.0, all
  drift-checked.
* **A specification generates its own conformance suite** — 29 scenarios from the normative example,
  31 from a second fixture — committed to the repository and drift-checked in CI, with every
  construct that got *no* scenario listed with the reason it got none.
* **The suite is known to bite.** Thirteen implementations that are wrong in exactly one way each
  are run against it, and the matrix asserts *which named scenario* catches each fault, plus how
  many unrelated scenarios it is allowed to disturb. All thirteen are caught — the three faults once
  recorded as caught by nothing have each since been closed, two by teaching synthesis to ask for
  more and the last by changing the model itself.
* **A change to a specification is a typed object.** `ess diff` reports what moved as changes over
  the compiled models, and `ess impact` reports which conformance results the movement invalidates —
  with the path that produced each invalidation, and no vocabulary for "still valid".
* **A specification synthesises the code that was never yours to write.** `ess synthesize` emits a
  language-neutral plan and a zero-dependency Rust workspace — types, typestate lifecycles,
  component ports, one transport — with every non-generated capability carried as a named obligation
  or an explained refusal, never a guess. The committed workspace is regenerated, compiled and held
  to the committed suite by CI.
* **A conformance run closes, or refuses to close, a real task.** The run mints its own evidence
  record in the process that executed the suite; a passing run completes the task, a failing one
  leaves it blocked and names the principle that refused.

## What does not work yet

* **No durable backend.** The only implementation of the contract is in memory, and it forgets
  everything when the process exits.
* **Generated code is structural, not behavioural.** A specification synthesises types, lifecycles,
  ports, a plan and one transport; every algorithm is a typed obligation someone still has to
  implement, and behavioural synthesis is rejected in the roadmap rather than pending.
* **The diff does not know about generated code.** A change to a specification owes the whole
  generated workspace, per the fail-closed polarity — narrowing that is wave 7's first slice.
* **Predicates are compared only for canonical equality.** Entities, commands, views, bindings,
  conversions and topology are outside the six compared construct families; any difference in them
  invalidates the whole suite rather than being narrowed.
* **No federated artifact graphs.** An artifact manifest describes one project; cross-repository
  references are resolved by hand.
* **No remote conformance runner, on either side.** Proving your own backend means calling the
  conformance crate from your own test suite, and the same is true of the ESS oracle: the CLI runs
  the two reference implementations it was compiled with and says so outright. Nothing here speaks to
  an implementation over a socket.
* **`independent: true` is structural, not attested.** It says the record came from the runner rather
  than from the agent under review, checked by one comparison. Nothing signs it, and there is no key
  anywhere in the workspace — see [what you still have to trust](./what-you-have-to-trust.md).
* **The OpenAPI and AsyncAPI envelopes are checked structurally**, not against their own
  meta-schemas — see [what you still have to trust](./what-you-have-to-trust.md).

## The next honest milestone

For AEP it is not a feature. It is a team whose work the protocol actually governs, and that has not
happened yet. For ESS it is wave 7, closing the loop wave 6 opened: generated artifacts carry the
digest of the model slice they derive from, so `ess impact` can narrow "the specification moved, the
whole workspace is owed" to the artifacts whose slice moved — and entities and commands join the
delta as conservative canonical equality. The wave's two further slices — a second emitter, and
obligations becoming artifacts and tasks — are deferred by decision.

---

**Sources.** `README.md` § *Status*; `docs/VISION.md` § *Where this stands*; `Taskfile.yml` (the
eight steps of `check`); `docs/plan/ess-roadmap.md`; `docs/plan/ess-wave-4-the-oracle.md`;
`docs/plan/ess-wave-6-structural-synthesis.md`; `CHANGELOG.md` §§ *0.6.0* and *0.6.1*. Suite and
test counts from `cargo test --workspace` at the tag (69 suites, 1397 tests, 0 failed); scenario
counts and the billing plan's 45 = 33 / 8 / 4 from `suites/generated/README.md` and
`generated/rust/billing/PLAN.md`; the fault count and the closure of the three once-uncaught rows
from `crates/ess-conformance/src/faulty.rs`, where a test asserts the list length. Document and
schema counts verified by counting files under `protocols/`, `principles/`, `workflows/`,
`profiles/`, `artifacts/lifecycles/` and `schemas/generated/`.
