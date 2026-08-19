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

**Roughly 62% implemented.** There is a runnable end-to-end path: resolve a task against the document
tree, submit evidence, be told what is owed, what is permitted and whether the work is done. The
interaction contract and conformance suites are not written yet.

| Component | Weight | Done | State |
|---|---:|---:|---|
| `aep-domain` — core model | 25% | 100% | 22 modules, 91 tests |
| `aep-engine` — resolution, evaluation, transitions | 15% | 100% | 38 unit + 16 integration tests |
| Protocol, principle, workflow and profile documents | 10% | 100% | 49 documents, validated in CI |
| `protocol-cli` | 7% | 100% | 7 subcommands, 14 integration tests |
| `aep-schema` + `xtask` — documents and generated schemas | 6% | 100% | 10 schemas, drift-checked |
| `aep-contract` — command/query contract | 12% | 0% | crate skeleton |
| `aep-conformance` — black-box backend suites | 12% | 0% | crate skeleton |
| Entity identity, locators and types | 5% | 0% | specified in v0.2 §13–18 |
| `adp-domain`, `aop-domain` | 5% | 0% | crate skeletons |
| In-memory reference backend | 3% | 0% | — |

163 tests, 0 failures, 0 clippy warnings. Weights are an effort estimate, not a measurement; verify
the "done" column with `task check`.

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
* 49 documents — 3 protocols, 21 principles, 4 workflows, 5 profiles, 5 artifact lifecycles — each
  validated against the protocol vocabulary in CI.

### What does not work yet

There is no command/query contract, no backend and no conformance suite, so nothing yet *stores* an
execution beyond a snapshot file. Entities have no canonical identity or locator. See
[`docs/plan/`](docs/plan/) for what is next.

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
| [`docs/plan/wave-1-execution-core.md`](docs/plan/wave-1-execution-core.md) | the wave just delivered, with its acceptance criteria |
| [`docs/plan/document-authoring-brief.md`](docs/plan/document-authoring-brief.md) | how to write a valid principle, workflow, profile or lifecycle |
| [`CHANGELOG.md`](CHANGELOG.md) | what changed, per release |
| [`examples/development-passkeys/`](examples/development-passkeys/) | a worked example with real command output |
| [`docs/design/archive/`](docs/design/archive/) | v0.1 draft and artifact-model extension, kept for provenance |
| [`AGENTS.md`](AGENTS.md) | working agreement for humans and agents contributing here |

## Licence

Apache-2.0. See [LICENSE](LICENSE).
