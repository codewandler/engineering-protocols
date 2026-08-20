---
title: Where this stands
sidebar_position: 1
description: What is implemented, what is in progress, and what is not built — with the numbers the repository's own gate reports.
---

# Where this stands

Honest as of the tag `0.3.2-ess-wave-3`. The repository's gate, `task check`, runs six steps —
formatting, clippy with warnings as errors, the test suite, rustdoc with warnings as errors, a schema
drift check and a projection drift check — and reports **41 suites and 953 tests, with 0 clippy
warnings and 0 rustdoc warnings**.

Nothing lands that does not pass all six, and CI runs the same six.

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

Roughly 60% of its design. Three waves delivered.

| | State |
|---|---|
| the typed model | implemented, `0.3.0-ess-wave-1` |
| resolution, IR and diagnostics | implemented, `0.3.1-ess-wave-2` — an unresolved reference is unrepresentable |
| four projections: docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 | implemented, `0.3.2-ess-wave-3` — output committed and drift-checked |
| the join: artifact kind, evidence kind, the `ess-conformance` principle | implemented — a task can already be blocked until something proves conformance |
| the specification as an oracle: generated tests, a conformance runner | **not built.** ESS wave 4 |
| generated Rust code | **not built.** ESS wave 5 |

Wave 4 has not started. A reconciliation pass over the model sits between it and wave 3, because
several of its open items were model changes that are cheaper to make before a synthesizer is built
around their absence.

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

## What does not work yet

* **No durable backend.** The only implementation of the contract is in memory, and it forgets
  everything when the process exits.
* **No generated tests, and no generated code.** A specification projects into documentation and
  contracts, not yet into a suite or a service.
* **No federated artifact graphs.** An artifact manifest describes one project; cross-repository
  references are resolved by hand.
* **No remote conformance runner.** Proving your own backend means calling the conformance crate from
  your own test suite.
* **The OpenAPI and AsyncAPI envelopes are checked structurally**, not against their own
  meta-schemas — see [what you still have to trust](./what-you-have-to-trust.md).

## The next honest milestone

For AEP it is not a feature. It is a team whose work the protocol actually governs, and that has not
happened yet. For ESS it is the specification as an *oracle*: a verdict on an implementation rather
than a projection of a model.

---

**Sources.** `README.md` § *Status*; `docs/VISION.md` § *Where this stands*; `AGENTS.md` § *Current
state*; `Taskfile.yml`; `docs/plan/ess-roadmap.md`. Document and schema counts verified by counting
files under `protocols/`, `principles/`, `workflows/`, `profiles/`, `artifacts/lifecycles/` and
`schemas/generated/`.
