---
title: Where this stands
sidebar_position: 1
description: What is implemented, what is in progress, and what is not built — with the numbers the repository's own gate reports.
---

# Where this stands

Honest as of the tag `0.5.0-ess-wave-5`. The repository's gate, `task check`, runs seven steps —
formatting, clippy with warnings as errors, the test suite, rustdoc with warnings as errors, a schema
drift check, a projection drift check and a conformance-suite drift check — and reports **57 suites
and 1305 tests, with 0 clippy warnings and 0 rustdoc warnings**.

Nothing lands that does not pass all seven, and CI runs the same seven.

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

Five waves delivered. A specification is now the source of its own documentation, its own
contracts and its own tests — and when it changes, the change itself is a typed, queryable object.

| | State |
|---|---|
| the typed model | implemented, `0.3.0-ess-wave-1` |
| resolution, IR and diagnostics | implemented, `0.3.1-ess-wave-2` — an unresolved reference is unrepresentable |
| four projections: docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 | implemented, `0.3.2-ess-wave-3` — output committed and drift-checked |
| the model reconciled before the oracle was built | implemented, `0.3.3-ess-wave-3.5` — 20 gates closed before the oracle was allowed to start |
| the join: artifact kind, evidence kind, the `ess-conformance` principle | implemented — a task can be blocked until something proves conformance |
| the specification as an oracle: generated suites, a conformance runner, evidence | implemented, `0.4.0-ess-wave-4` — 27 scenarios from the normative example and 31 from a second fixture, committed and drift-checked |
| what a change to a specification invalidates | implemented, `0.5.0-ess-wave-5` — a semantic delta over six construct families, and an impact closure that narrows what is owed and can never say "still valid" |
| generated Rust code | **not built.** Proposed, and sequenced next |

Building the oracle changed the specification language four times, and that is the part worth
knowing. Two of the four gaps were found by review before a line of the synthesizer existed; the
other two were found by the synthesizer **refusing to generate a scenario** and saying which construct
it could not test. Every one of the four had been rendered without complaint by all four projections,
because a document does not have to run. `docs/plan/ess-wave-4-the-oracle.md` in the repository is
the full account, including what the wave did not close.

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
* **A specification generates its own conformance suite** — 27 scenarios from the normative example,
  31 from a second fixture — committed to the repository and drift-checked in CI, with every
  construct that got *no* scenario listed with the reason it got none.
* **The suite is known to bite.** Twelve implementations that are wrong in exactly one way each are
  run against it, and the matrix asserts *which named scenario* catches each fault, plus how many
  unrelated scenarios it is allowed to disturb. Eleven of the twelve are caught. The twelfth is
  recorded as caught by nothing, with the reason.
* **A conformance run closes, or refuses to close, a real task.** The run mints its own evidence
  record in the process that executed the suite; a passing run completes the task, a failing one
  leaves it blocked and names the principle that refused.

## What does not work yet

* **No durable backend.** The only implementation of the contract is in memory, and it forgets
  everything when the process exits.
* **No generated code.** A specification projects into documentation, contracts and a conformance
  suite, not yet into a service.
* **No answer to what a change invalidates.** Everything is derived from a single revision, and
  conformance evidence fails closed when the model moves — so any edit to a specification, including
  one to a comment in an unrelated domain, sends every conformance requirement back to owed.
* **One deliberate fault is caught by no scenario.** An event may be published carrying a value
  nobody supplied, because nothing in the model relates a command's input to an emitted event's
  payload. It is recorded as uncaught rather than quietly dropped, and a test asserts it is still
  uncaught, so closing the hole breaks the row rather than being forgotten.
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
happened yet. For ESS the oracle now exists, so the next milestone is the one it exposed: a
specification that can say what *changed* and what that change invalidates, instead of invalidating
everything.

---

**Sources.** `README.md` § *Status*; `docs/VISION.md` § *Where this stands*; `AGENTS.md` § *Current
state*; `Taskfile.yml`; `docs/plan/ess-roadmap.md`; `docs/plan/ess-wave-4-the-oracle.md`. Suite and
scenario counts read from `suites/generated/README.md` and the two `suite.json` documents beside it;
the fault count and which of them is caught by nothing from `crates/ess-conformance/src/faulty.rs`,
where a test asserts the list length. Document and schema counts verified by counting files under
`protocols/`, `principles/`, `workflows/`, `profiles/`, `artifacts/lifecycles/` and
`schemas/generated/`.
