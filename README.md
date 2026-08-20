# engineering-protocols

> A strongly typed, portable and machine-executable specification for how autonomous engineering
> work is performed and proven correct.

Coding and operations agents are usually governed by prose:

> *Follow TDD, don't break existing APIs, verify your work, and ask before deploying.*

That reads well and enforces nothing. It leaves every operative question open: what counts as
following TDD, what evidence proves a test failed *before* the implementation existed, which
operations need approval, what "verify your work" means, when the task is actually finished, and what
happens when verification fails.

`engineering-protocols` moves those rules out of prompts and into typed, executable protocol
definitions. The model still reasons. The protocol decides what the resulting facts permit.

```text
Task
  ↓  protocol resolution
Allowed actions + obligations
  ↓  agent proposes / executes work
Evidence collection
  ↓  independent verification
Deterministic state transition
  ↓
Complete, or iterate with a counterexample
```

The agent may be probabilistic. The protocol semantics are not.

## Status

This repository has two halves. **AEP** governs how engineering work is performed; **ESS** specifies
what software must exist. They meet at evidence: a task can be blocked until something proves an
implementation conforms to its specification. See [`docs/VISION.md`](docs/VISION.md).

Two halves is what exists. Four unaccepted designs in [`docs/design/`](docs/design/) would add a third
axis (a system *changing* over time) and a fourth domain (infrastructure); they are proposals, and
[`docs/VISION.md`](docs/VISION.md) § *Proposed, not accepted* is where their status is kept.

### AEP — the protocol (v0.2 scope, complete)

A task resolves against the document tree, evidence decides what may be done and whether the work is
finished, every object it touches has identity, revision and an audit trail, and a backend can prove
it implements the contract by running a suite against itself.

| Component | Weight | Done | State |
|---|---:|---:|---|
| `aep-domain` — core model | 25% | 100% | 26 modules, 164 tests |
| `aep-engine` — resolution, evaluation, transitions | 15% | 100% | 51 unit + 16 integration tests |
| Protocol, principle, workflow and profile documents | 10% | 100% | 39 documents, validated in CI |
| `protocol-cli` | 7% | 100% | 11 subcommands, 38 tests |
| `aep-schema` + `xtask` — documents and generated schemas | 6% | 100% | 12 schemas, drift-checked, each validated against every document shipped |
| `aep-contract` — command/query contract | 12% | 100% | 21 tests |
| Entity identity, locators and types | 5% | 100% | 14 tests; commands, events and audit add 57 more |
| In-memory reference backend | 3% | 100% | passes the §104 reference scenario |
| `aep-conformance` — black-box backend suites | 12% | 100% | 16 suites, 3 levels, a faulty backend that proves they bite |
| `adp-domain`, `aop-domain` | 5% | 100% | 44 + 49 tests |

### ESS — executable system specifications (~60% of the design)

| Component | Done | State |
|---|---:|---|
| `ess-domain` — the typed model | 100% | 13 modules; entities, commands, views, actors, components, bindings, topology |
| `ess-compiler` — resolution, IR, diagnostics | 100% | an unresolved reference is unrepresentable; codes, spans, byte-identical output |
| `ess-gen` — four projections behind one `Generator` trait | 100% | Markdown + Mermaid, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0; one shared type mapping, agreement asserted; 123 tests |
| `protocol ess validate\|compile\|inspect\|graph\|generate` | 100% | one file or a directory; every problem in one run |
| The join — artifact kind, evidence kind, `ess-conformance` principle | 100% | a task can already be blocked until something proves conformance |
| Projections — documentation, JSON Schema, OpenAPI, AsyncAPI | 100% | 27 artifacts plus a generated index, committed under `generated/`, provenance on each, drift-checked in CI |
| [ESS wave 3.5 — reconciliation](docs/plan/ess-wave-3.5-reconciliation.md) | 15/19 | in progress, and where the work currently is: 19 gates, 15 closed. The open four are G2 and G15 in the model, G16's predicate flattener, and G19 binding evidence to a specification revision. Wave 4 does not start until they land |
| Test synthesis, and an implementation deliberately wrong | 0% | ESS wave 4 |
| Rust structural synthesis | 0% | ESS wave 5 |

`task check` passes 41 suites and 953 tests, with 0 clippy warnings and 0 rustdoc warnings. Weights
are an effort estimate, not a measurement; verify the "done" column with `task check`. The ESS roadmap
is [`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md).

### What works today

```console
$ protocol explain --task examples/development-passkeys/task.yaml --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
```

* **A task resolves into a plan**: `extends` chains merged, principles filtered by whether they apply,
  capabilities composed with the document responsible recorded for every entry, obligations collected,
  and the whole configuration refused if any rule in it could never fire.
* **Evidence drives transitions.** A workflow advances when the evidence satisfies the guard, and the
  refusal names what is missing.
* **Ordering is checkable.** `evidence.first_seq.test_result < evidence.first_seq.diff` is how
  red-before-green is enforced — a fact, not an instruction.
* **An agent cannot verify itself.** `independent: true` on an evidence requirement is not satisfied
  by the agent's own report of a green suite.
* **An approval names the revision it approved**, so an approval of design version 3 stops satisfying
  a review requirement once the design is at version 7.
* **Every decision is explainable**, as a `✓ / ✗ / ?` checklist or as a machine-readable refusal.
* 39 documents — 3 protocols, 22 principles, 4 workflows, 5 profiles, 5 artifact lifecycles — each
  validated against the protocol vocabulary in CI.

* **Every mutation goes through one boundary.** `CommandService` carries actor *and* executor,
  correlation and causation, an idempotency key and an asserted revision — so a retry is recognised, a
  stale write is refused rather than merged, and a refusal leaves a record.
* **Nothing is deleted.** `ArchiveEntity` and `SupersedeEntity` are the vocabulary; an engineering
  record whose history can be erased is not a record.

* **A backend can prove it conforms.** Sixteen suites, three levels, and a deliberately broken
  backend the suites are checked against — because a suite that passes everything tells you nothing:

```console
$ protocol conformance --suite idempotency --inject replay-applies
  ✗ a replay does not advance the revision — the command left the entity at revision 2, and after
    replaying it the entity is at revision 3
injected fault: a replayed command is applied a second time — expected to be caught by the
`idempotency` suite
```

### What does not work yet

No durable backend — the only implementation is in memory. No federated artifact graphs across
repositories. A specification projects into documentation and contracts, but not yet into tests or
code: the generated conformance suite is ESS wave 4 and Rust structural synthesis is wave 5. The
generated OpenAPI and AsyncAPI *envelopes* are checked structurally rather than against the OpenAPI
3.1 and AsyncAPI 3.0 meta-schemas, neither of which is vendored here; every schema those documents
embed is validated against the real JSON Schema 2020-12 meta-schema, so what is unchecked is the
envelope, not the types. See [`docs/plan/`](docs/plan/) for what was built and in what order, and
[`docs/guide/`](docs/guide/) for how to use what exists.

## Design decisions worth knowing

**Unknown is not false.** `tests.unit.failed == 0` is *false* when a suite failed and *unknown* when
nothing ran. A harness needs different behaviour in each case — fix the code, or go run the tests —
and only `true` permits a transition, so nothing is loosened by the third value.

**Capabilities default to deny.** A capability no document mentions is not granted. `deny` cannot be
granted back by a later document, so it works as a safety envelope.

**An agent cannot verify itself.** An evidence requirement can be marked `independent: true`, which
an agent's own assertion never satisfies; the test runner's does.

**An approval names the revision it approved.** An approval of version 3 of a design does not
silently authorise version 7 — otherwise a reviewer's name ends up attached to a decision they never
saw.

**Rollback must say what it needs.** `on_failure: rollback` with no precondition is rejected at
validation time. A rollback plan that cannot say what it rolls back to is not a plan.

**Schemas are outputs.** `schemas/generated/` is derived from the Rust types by
`cargo xtask schema`; CI fails if the two disagree.

## Repository layout

```text
crates/
  aep-domain/       protocol-neutral domain model — the source of truth
  aep-contract/     storage-independent command/query contract
  aep-engine/       resolution, evaluation, transition logic
  aep-schema/       document reading and JSON Schema generation
  aep-backend-memory/ in-memory reference implementation of the contract
  aep-conformance/  black-box conformance suites for backends
  adp-domain/       development-specific types (ADP)
  aop-domain/       operations-specific types (AOP)
  ess-domain/       the typed model for an executable system specification
  ess-compiler/     resolution, normalized IR, diagnostics
  ess-gen/          deterministic projections: docs, JSON Schema, OpenAPI, AsyncAPI
  protocol-cli/     reference CLI: validate, resolve, inspect, evaluate, explain, schema, ess
protocols/          protocol declarations: aep, adp, aop
principles/         reusable enforceable rules
workflows/          state machines: development, incidents, releases, migrations
profiles/           bundles of protocol + workflow + principles + completion
artifacts/          artifact kinds, relations, lifecycles and templates
schemas/generated/  generated JSON Schema — do not edit by hand
generated/          projections of examples/billing/ — do not edit by hand
conformance/        fixtures, scenarios and expected results
docs/design/        the design specifications
xtask/              repository automation
```

## Build

Requires a recent stable Rust and [go-task](https://taskfile.dev).

```console
task check          # six steps: format check, clippy, tests, rustdoc, schema, projections
task test
task schema         # regenerate schemas/generated/
task generate       # regenerate generated/ — the projections of examples/billing/
task doc
task doc-check      # rustdoc with warnings as errors
```

Without `task`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`,
`cargo xtask schema --check`, `cargo xtask generate --check`.

## Documents

| Document | Role |
|---|---|
| [`docs/VISION.md`](docs/VISION.md) | why this exists, and how its two halves compose |
| [`docs/design/consolidated-design-v0.2.md`](docs/design/consolidated-design-v0.2.md) | **normative** — the specification for the protocol (AEP) |
| [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) | **normative** — what is implemented, what v0.2 adds, work order, recorded deviations |
| [`docs/design/ess-implementor-design-v0.1.md`](docs/design/ess-implementor-design-v0.1.md) | design for the Executable System Specification (ESS) — the model and the compiler are built, and a specification projects into documentation and contracts |
| [`docs/design/ess-review-v0.1.md`](docs/design/ess-review-v0.1.md) | review of that design, with the findings that change what gets built first |
| [`docs/design/ess-closed-loop-execution-conformance-design-v0.1.md`](docs/design/ess-closed-loop-execution-conformance-design-v0.1.md) | *proposed, reconciled and frozen* — ESS wave 4: the specification as an oracle. Frozen for implementation except four named open decisions (D1–D4) |
| [`docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md`](docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md) | *proposed, unreviewed* — semantic diff, impact closure, evolution planning. Sequenced after wave 4 |
| [`docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md`](docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md) | *proposed, reviewed, not reconciled* — generated applications, and work as typed obligations. The feasibility review reads it as four waves rather than one, and none of its findings is folded in. Unsequenced |
| [`docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md`](docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md) | *proposed, unreviewed* — infrastructure as a fourth domain, `InfraSpec`/`InfraIr`. Unsequenced |
| [`docs/plan/ess-wave-1-the-model.md`](docs/plan/ess-wave-1-the-model.md) | ESS wave 1 — the model, and what its review changed |
| [`docs/plan/ess-wave-2-the-compiler.md`](docs/plan/ess-wave-2-the-compiler.md) | ESS wave 2 — the IR, and where validation turned out to belong |
| [`docs/plan/ess-wave-3-projections.md`](docs/plan/ess-wave-3-projections.md) | ESS wave 3 — the projections, and what they refuse to guess |
| [`docs/plan/ess-wave-3.5-reconciliation.md`](docs/plan/ess-wave-3.5-reconciliation.md) | **in progress** — the gates wave 4 waits behind, and the evidence each one closes on |
| [`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md) | ESS waves 1 to 5, and what is deliberately outside them |
| [`docs/plan/wave-1-execution-core.md`](docs/plan/wave-1-execution-core.md) | the protocol's four waves, with their acceptance criteria |
| [`docs/plan/document-authoring-brief.md`](docs/plan/document-authoring-brief.md) | how to write a valid principle, workflow, profile or lifecycle |
| [`docs/reviews/`](docs/reviews/) | *snapshots, not maintained* — five independent reviews: the vision, guard efficacy, next-wave feasibility and an outside pre-wave-4 readiness review, all at `3647f80`, plus a full-repository review at `95e210f`. Each describes the commit it names; where one disagrees with this README, this README is current |
| [`CHANGELOG.md`](CHANGELOG.md) | what changed, per release |
| [`examples/development-passkeys/`](examples/development-passkeys/) | a worked protocol example with real command output |
| [`examples/billing/`](examples/billing/) | the normative executable system specification |
| [`docs/design/archive/`](docs/design/archive/) | v0.1 draft and artifact-model extension, kept for provenance |
| [`docs/guide/`](docs/guide/) | how to adopt the protocol, wire a harness, prove a backend, and specify a system |
| [`AGENTS.md`](AGENTS.md) | working agreement for humans and agents contributing here |

## Licence

Apache-2.0. See [LICENSE](LICENSE).
