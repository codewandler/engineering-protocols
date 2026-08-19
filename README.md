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

**Roughly 30% implemented.** The domain model and the document layer are done and gated; execution,
the interaction contract and conformance are not yet written.

| Component | Weight | Done | State |
|---|---:|---:|---|
| `aep-domain` — core model | 25% | 100% | 13.2k lines, 91 tests, clippy-pedantic clean |
| `aep-schema` + `xtask` — documents and generated schemas | 6% | 100% | 7 tests, 10 schemas generated |
| `aep-engine` — resolution, evaluation, transitions | 15% | 0% | crate skeleton, planned surface documented |
| `aep-contract` — command/query contract | 12% | 0% | crate skeleton |
| `aep-conformance` — black-box backend suites | 12% | 0% | crate skeleton |
| Protocol, principle, workflow and profile documents | 10% | 0% | directories in place, no documents yet |
| `protocol-cli` | 7% | 0% | subcommands declared, each reports "not implemented" |
| Entity identity, locators and types | 5% | 0% | specified in v0.2 §13–18 |
| `adp-domain`, `aop-domain` | 5% | 0% | crate skeletons |
| In-memory reference backend | 3% | 0% | — |

Weights are an effort estimate, not a measurement. Verify the "done" column with `task check`.

### What works today

* Every AEP document type parses and is semantically validated: protocols, principles, workflows,
  profiles, tasks, artifact manifests and artifact lifecycles.
* The predicate language, in both its compact form (`tests.unit.failed == 0`) and its structured
  form, with three-valued evaluation and minimal-cause explanations.
* The engineering artifact graph: kinds with a subtype hierarchy, statuses, per-kind lifecycles,
  typed relations, cycle and dangling-edge detection, and projection into facts.
* Requirements over evidence, artifacts, reviews, approvals, and conditions
  (`if change.architectural then an architecture design and an ADR are required`).
* Capability policy with default-deny and `deny > require_approval > allow` precedence.
* Evidence with provenance, and its projection into the fact vocabulary predicates read.
* 10 published JSON Schemas, regenerated from the Rust types and checked in CI.

### What does not work yet

There is no runnable end-to-end path: you cannot yet resolve a task, submit evidence and be told
whether a transition is permitted. That is `aep-engine`, and it is next.

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
  aep-conformance/  black-box conformance suites for backends
  adp-domain/       development-specific types (ADP)
  aop-domain/       operations-specific types (AOP)
  protocol-cli/     reference CLI: validate, resolve, inspect, evaluate, explain, schema
protocols/          protocol declarations: aep, adp, aop
principles/         reusable enforceable rules
workflows/          state machines: development, incidents, releases, migrations
profiles/           bundles of protocol + workflow + principles + completion
artifacts/          artifact kinds, relations, lifecycles and templates
schemas/generated/  generated JSON Schema — do not edit by hand
conformance/        fixtures, scenarios and expected results
docs/design/        the design specifications
xtask/              repository automation
```

## Build

Requires a recent stable Rust and [go-task](https://taskfile.dev).

```console
task check          # format check, clippy (warnings are errors), tests, schema check
task test
task schema         # regenerate schemas/generated/
task doc
```

Without `task`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo xtask schema --check`.

## Documents

| Document | Role |
|---|---|
| [`docs/design/consolidated-design-v0.2.md`](docs/design/consolidated-design-v0.2.md) | authoritative specification |
| [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) | what is implemented, what v0.2 adds, work order, recorded deviations |
| [`docs/design/archive/`](docs/design/archive/) | v0.1 draft and artifact-model extension, kept for provenance |
| [`AGENTS.md`](AGENTS.md) | working agreement for humans and agents contributing here |

## Licence

Apache-2.0. See [LICENSE](LICENSE).
