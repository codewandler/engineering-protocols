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

Two halves is what exists. Six further designs in [`docs/design/`](docs/design/) proposed extending
it. Three have been taken up: the specification as an oracle is **implemented** as ESS wave 4,
semantic diff — what a revision invalidates — is **implemented** as ESS wave 5 after being accepted
into the thesis as a stated amendment and extended by wave 7 down to the generated artifact, and
structural synthesis is **delivered through wave 7** — three emitters behind one plan, and one
specification running as two applications held to one behaviour in every gate run. The fourth —
infrastructure, a fourth domain — stays deferred as designed; a Kubernetes-scoped subset is being
built as infrastructure waves, its first two delivered, with the discovery adapter outside this
workspace, per the boundary the vision draws. The fifth is the newest: a planning store and a
reference driver, whose Phase 1 — the store, the `protocol artifact` verbs and the Claude Code
plugin — is accepted as **harness wave 1**, and whose driver is decided and not yet built. The
sixth is newer still and wholly unaccepted: transcript conformance, a typed specification over an
agent-run transcript. [`docs/VISION.md`](docs/VISION.md) § *Proposed, not accepted* is where their
status is kept.

### AEP — the protocol (v0.2 scope, complete)

A task resolves against the document tree, evidence decides what may be done and whether the work is
finished, every object it touches has identity, revision and an audit trail, and a backend can prove
it implements the contract by running a suite against itself.

| Component | Weight | Done | State |
|---|---:|---:|---|
| `aep-domain` — core model | 25% | 100% | 27 modules, 197 tests |
| `aep-engine` — resolution, evaluation, transitions | 15% | 100% | 55 unit + 20 integration tests |
| Protocol, principle, workflow and profile documents | 10% | 100% | 39 documents, validated in CI |
| `protocol-cli` | 7% | 100% | 11 subcommands, 88 tests |
| `aep-schema` + `xtask` — documents and generated schemas | 6% | 100% | 12 schemas, drift-checked, each validated against every document shipped |
| `aep-contract` — command/query contract | 12% | 100% | 21 tests |
| Entity identity, locators and types | 5% | 100% | 14 tests; commands, events and audit add 57 more |
| In-memory reference backend | 3% | 100% | passes the §104 reference scenario |
| `aep-conformance` — black-box backend suites | 12% | 100% | 16 suites, 3 levels, a faulty backend that proves they bite |
| `adp-domain`, `aop-domain` | 5% | 100% | 44 + 49 tests |

### ESS — executable system specifications (seven waves delivered)

| Component | Done | State |
|---|---:|---|
| `ess-domain` — the typed model | 100% | 13 modules; entities, commands, views, actors, components, bindings, topology |
| `ess-compiler` — resolution, IR, diagnostics | 100% | an unresolved reference is unrepresentable; codes, spans, byte-identical output |
| `ess-gen` — four projections behind one `Generator` trait | 100% | Markdown + Mermaid, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0; one shared type mapping, agreement asserted; 131 tests |
| `ess-conformance` — the specification as an oracle | 100% | synthesis, a runner that owns its clock and id source, evidence, and twelve deliberately wrong implementations; 147 tests |
| `ess-diff` — what moved, and what that invalidates | 60% | ten construct families compared field by field, with conservative canonical equality where a construct carries a predicate; impact closure with the path recoverable and invalidation that can only narrow, answering for the conformance suite and — via each artifact's `contract_digest` — for the generated artifacts; 142 tests. Conversions, workloads and domain naming still fall to whole-suite invalidation |
| `protocol ess validate\|compile\|inspect\|generate\|synthesize\|graph\|diff\|impact\|conform` | 100% | one file or a directory; every problem in one run |
| `protocol infra validate\|compile\|inspect\|graph\|view\|diagnose` | 100% | an observation bundle from an external scanner becomes a validated, content-addressed IR, a typed graph, a coded diagnosis with candidates and directions, and a self-contained HTML view; nothing in this workspace reaches a cluster |
| The join — artifact kind, evidence kind, `ess-conformance` principle | 100% | a task can already be blocked until something proves conformance |
| Projections — documentation, JSON Schema, OpenAPI, AsyncAPI | 100% | 36 committed files under `generated/` (the projections and their generated indexes), provenance — model digest and contract digest — on each, drift-checked in CI |
| [ESS wave 3.5 — reconciliation](docs/plan/ess-wave-3.5-reconciliation.md) | 100% | all 20 gates closed, `0.3.3-ess-wave-3.5` |
| Generated conformance suites, committed and drift-checked | 100% | 29 scenarios from `examples/billing/` (27 at wave 4, grown by wave 6.5), 31 from `examples/oracle-fixture/` and 12 from `examples/gatepass/`, under `suites/generated/`, with every construct that got no scenario listed with its reason |
| Semantic diff — what a revision invalidates | 100% | `ess diff` and `ess impact`, `0.5.0-ess-wave-5` — first slice as scoped: six construct families; predicate-bearing constructs fall back to whole-suite invalidation |
| Structural synthesis (`ess-synth`) — the plan, and the first emitter behind it | 100% | wave 6 complete, `0.6.0-ess-wave-6` — for the delivered scope, `component-skeletons`: a language-neutral `SynthesisPlan` (every capability generated, owed or refused, with reasons), a committed zero-dependency workspace — semantic types, typestate lifecycles, component ports, one transport — and the executed criterion: the committed billing suite, unchanged, passes the workspace linked with `examples/billing-realization` (27 of 27) and fails the deliberately corrupted linkage at exactly the scenario that exists to catch it. Billing: 45 capabilities = 33 generated / 8 obligations / 4 refused; the linker never chooses (D-2) |
| Wave 6.5 hardening — the gap register emptied by code | 100% | `0.6.1-ess-wave-6.5`: invariants 7, 8 and 14 now enforced by scans and a write-surface test, each with an inverse assertion; model digest widened to the full SHA-256 (D-4); `proptest` phase 1 landed, fixed-seed; the input→event-payload construct closes the one fault that was caught by nothing; value-object invariant scenarios grow the billing suite 27→29 with its refusal count at zero |
| Three emitters behind one plan | 100% | Rust (wave 6), a standard-library-only Go module (W7.3) and a browser realization — a `WebAssembly` bridge and a page built at load time from an emitted catalogue (W7.3b). The plan gained not one line to admit either: `PLAN.md` and `plan.json` are byte-identical in all three trees, and what a target holds more weakly or cannot represent at all is in a `TARGET.md` beside the plan. All three committed under `generated/` and drift-checked, with `gofmt -l`, `go build`, `go vet`, `cargo build --target wasm32-unknown-unknown` and a Node-driven boundary test in the gate |
| One specification, two applications, one surface (W7.5) | 100% | `examples/gatepass/` synthesised to Rust **and** Go, both binaries serving the same HTTP surface. The model gained one word — `reached_by: network` on a component — which states where a component's callers are and names no protocol; the transport is *derived*, because the one contract this repository projects for a command surface is the `OpenAPI` document. Seven routes from one mapping, `/openapi.json` and `/docs` serving the committed bytes, and a startup record identical outside a declared `runtime`. `cargo xtask synth` builds both, starts each on an ephemeral port and compares records, statuses, bodies and documents |

`task check` passes 94 suites and 1693 tests, with 0 clippy warnings and 0 rustdoc warnings. Weights
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

* **A specification generates its own tests, and they bite.** 29 scenarios from
  [`examples/billing/`](examples/billing/), 31 from [`examples/oracle-fixture/`](examples/oracle-fixture/)
  and 12 from [`examples/gatepass/`](examples/gatepass/),
  committed under [`suites/generated/`](suites/generated/) and drift-checked in CI. Thirteen
  deliberately wrong implementations are held against them, and the matrix asserts *which named
  scenario* catches each fault — plus a blast-radius allowance, so a suite that starts over-reaching
  fails rather than looking thorough. All thirteen are caught; the three rows once recorded as
  caught by nothing have each since been closed, and stay in the matrix as caught rather than being
  deleted:

```console
$ protocol ess conform run --path examples/billing --target billing --inject accept-invalid-amount
billing v3 against billing-reference-accept-invalid-amount 0.1.0 — failed
  ...
  failed billing.invoice.CreateInvoice/outcome/rejected
  ...
  29 scenarios: 28 passed, 1 failed, 0 error, 0 unsupported
injected fault: an input that satisfies no branch's guard is accepted by the guarded one — expected to
be caught by `billing.invoice.CreateInvoice/outcome/rejected`
not conformant: the implementation contradicted the specification (exit 1)
```

* **Planning artifacts have somewhere to live, and a status move is checked.**
  `protocol artifact new|move|relate|list|board|graph|validate|kinds|relations|lifecycle` keeps
  epics, stories, tasks and initiatives as markdown under `.engineering/planning/`. A move is
  validated against the kind's lifecycle, and a refusal names every status legal from where the
  artifact stands rather than saying "illegal transition"; `validate` reports every problem in one
  run. The file format belongs to `aep-backend-markdown` — `aep-domain` gained nothing for it — and
  is described by a generated schema like every other document type here.
* **Claude Code can plan through it.** [`integrations/claude-code/`](integrations/claude-code/) is a
  plugin: one `planning` skill, two agents — `decomposer` drafts, `plan-reviewer` only reads — and no
  hooks, on purpose. The skill carries rules and no vocabulary; it asks the CLI which kinds, statuses
  and moves exist at the moment it needs them, because a prose copy of a validated document is drift.

* **And the protocol decides on the result.** `protocol ess conform evidence` mints the record in the
  same process that ran the suite, so no caller can author its own verdict;
  [`examples/billing-conformance/`](examples/billing-conformance/) walks both directions — a passing
  run completes the task, a faulty one leaves it blocked, naming the principle that refused.

### What does not work yet

No durable backend — the only implementation is in memory. No federated artifact graphs across
repositories. Generated code is structural, never behavioural: a specification synthesises types,
typestate lifecycles, ports, transports and a plan, and every algorithm remains a typed
obligation someone still has to implement — and an obligation is a plan entry, not yet an artifact
a task can own (W7.4, deferred by decision). The delta still has a fail-closed arm: conversions,
workloads and a domain's naming have no compared construct family, so a change there owes the whole
suite rather than being narrowed; and predicates are compared only for canonical equality, so a
provably weaker rewrite still reads as *changed* — implication stays refused. **The conformance runner cannot reach
an out-of-process implementation**: `ConformanceTarget` is a Rust trait, this binary runs only the
reference implementations it was compiled with, and holding your own system to a specification means
depending on `ess-conformance` from your own tests. The generated OpenAPI and AsyncAPI *envelopes*
are checked structurally
rather than against the OpenAPI 3.1 and AsyncAPI 3.0 meta-schemas, neither of which is vendored here;
every schema those documents embed is validated against the real JSON Schema 2020-12 meta-schema, so
what is unchecked is the envelope, not the types. See [`docs/plan/`](docs/plan/) for what was built
and in what order, and [`docs/guide/`](docs/guide/) for how to use what exists.

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
  aep-backend-markdown/ the durable planning store: artifacts as markdown under .engineering/planning/
  aep-conformance/  black-box conformance suites for backends
  adp-domain/       development-specific types (ADP)
  aop-domain/       operations-specific types (AOP)
  ess-domain/       the typed model for an executable system specification
  ess-compiler/     resolution, normalized IR, diagnostics
  ess-gen/          deterministic projections: docs, JSON Schema, OpenAPI, AsyncAPI
  ess-conformance/  scenario synthesis, the runner, and the implementations that are wrong on purpose
  ess-diff/         the semantic delta between two revisions, and the impact closure over it
  ess-synth/        the language-neutral synthesis plan, and the Rust, Go and browser emitters behind it
  infra-domain/     the typed model of an observed cluster (infra-observation/1)
  infra-compiler/   validation, and the content-addressed infra-ir/1 document
  infra-analyze/    the typed dependency graph, diagnosis, candidates, directions, the HTML view
  protocol-cli/     reference CLI: validate, resolve, inspect, evaluate, explain, schema, ess, infra
protocols/          protocol declarations: aep, adp, aop
principles/         reusable enforceable rules
workflows/          state machines: development, incidents, releases, migrations
profiles/           bundles of protocol + workflow + principles + completion
artifacts/          artifact kinds, relations, lifecycles and templates
schemas/generated/  generated JSON Schema — do not edit by hand
generated/          projections of examples/billing/, and under rust/, go/ and web/ the synthesised trees — do not edit by hand
suites/generated/   conformance suites, generated from the specifications — do not edit by hand
conformance/        fixtures, scenarios and expected results
integrations/       deliverables named after who they are for: claude-code/ is the planning plugin
docs/design/        the design specifications
xtask/              repository automation
```

## Build

Requires a recent stable Rust and [go-task](https://taskfile.dev).

```console
task check          # nine steps: format check, clippy, tests, rustdoc, schema, projections, suites, observation IR, synthesis
task test
task schema         # regenerate schemas/generated/
task generate       # regenerate generated/ — the projections of examples/billing/
task suite          # regenerate suites/generated/ — the conformance suites examples/ oblige
task synth          # regenerate generated/rust/, go/ and web/, then run the dual-target demonstration
task infra          # regenerate examples/k3d-dev-cluster/cluster.ir.json from its observation
task doc
task doc-check      # rustdoc with warnings as errors
```

The synthesis step needs the Go toolchain, the `wasm32-unknown-unknown` Rust target and Node beside
Rust's own, and says which is missing rather than skipping.

Without `task`: `cargo xtask fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`,
`cargo xtask schema --check`, `cargo xtask generate --check`, `cargo xtask suite --check`,
`cargo xtask infra --check`, `cargo xtask synth --check`.

## Documents

| Document | Role |
|---|---|
| [`docs/VISION.md`](docs/VISION.md) | why this exists, and how its two halves compose |
| [`docs/design/consolidated-design-v0.2.md`](docs/design/consolidated-design-v0.2.md) | **normative** — the specification for the protocol (AEP) |
| [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) | **normative** — what is implemented, what v0.2 adds, work order, recorded deviations |
| [`docs/design/ess-implementor-design-v0.1.md`](docs/design/ess-implementor-design-v0.1.md) | design for the Executable System Specification (ESS) — the model and the compiler are built, and a specification projects into documentation and contracts |
| [`docs/design/ess-review-v0.1.md`](docs/design/ess-review-v0.1.md) | review of that design, with the findings that change what gets built first |
| [`docs/design/ess-closed-loop-execution-conformance-design-v0.1.md`](docs/design/ess-closed-loop-execution-conformance-design-v0.1.md) | **implemented** as ESS wave 4 — the specification as an oracle. All four of its open decisions (D1–D4) were taken at their stated defaults |
| [`docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md`](docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md) | *core implemented* as ESS wave 5 — `ess diff` and `ess impact`. The feasibility review reads the full design as four waves rather than one, and two of its seventy-eight sections were rejected outright rather than deferred |
| [`docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md`](docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md) | *first slice implemented* as ESS wave 6 — the plan, the Rust emission, obligations as plan entries. The review reads the full design as four waves; §36's behavioural synthesis, §41's agent loop and §28's obligation-derived grants are rejected outright, and `Realization`, topology synthesis and formal verification stay proposed with the design |
| [`docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md`](docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md) | *proposed, reviewed, deferred whole* — infrastructure as a fourth domain, `InfraSpec`/`InfraIr`. Two ideas harvested; the design itself would put cloud discovery adapters inside this workspace, which the vision refuses. A Kubernetes-scoped subset is planned as infrastructure waves, with the discovery adapter external to this workspace |
| [`docs/design/harness-planning-and-driver-design-v0.1.md`](docs/design/harness-planning-and-driver-design-v0.1.md) | *Phase 1 accepted* as harness wave 1 — the markdown planning store, the `protocol artifact` verbs and the Claude Code plugin. Its Phase 2, a **reference driver** implementing the harness contract, is decided by the operator and **not accepted for build**: the vision narrowing that admits it is recorded, and the build waits behind a feasibility review |
| [`docs/design/transcript-conformance-design-v0.1.md`](docs/design/transcript-conformance-design-v0.1.md) | *proposed, not accepted* — a typed, executable specification over an agent-run transcript: which skill was loaded, which tool was called with which arguments, what happened before what, what the run cost. The `infra-spec/1` pattern in a third observation domain, with the same `ok`/`gap`/`unk` verdicts and no model anywhere in the checker |
| [`docs/plan/ess-wave-1-the-model.md`](docs/plan/ess-wave-1-the-model.md) | ESS wave 1 — the model, and what its review changed |
| [`docs/plan/ess-wave-2-the-compiler.md`](docs/plan/ess-wave-2-the-compiler.md) | ESS wave 2 — the IR, and where validation turned out to belong |
| [`docs/plan/ess-wave-3-projections.md`](docs/plan/ess-wave-3-projections.md) | ESS wave 3 — the projections, and what they refuse to guess |
| [`docs/plan/ess-wave-3.5-reconciliation.md`](docs/plan/ess-wave-3.5-reconciliation.md) | ESS wave 3.5 — the twenty gates wave 4 waited behind, and the evidence each one closed on |
| [`docs/plan/ess-wave-4-the-oracle.md`](docs/plan/ess-wave-4-the-oracle.md) | ESS wave 4 — the oracle, the four model gaps it found, and the fault nothing catches (caught since wave 6.5) |
| [`docs/plan/ess-wave-5-semantic-diff.md`](docs/plan/ess-wave-5-semantic-diff.md) | ESS wave 5 — the semantic delta and the impact closure, with what was accepted and what was rejected by name |
| [`docs/plan/ess-wave-6-structural-synthesis.md`](docs/plan/ess-wave-6-structural-synthesis.md) | ESS wave 6 — structural synthesis: the decisions taken, and what is deliberately not in it |
| [`docs/plan/ess-wave-7-closing-the-loop.md`](docs/plan/ess-wave-7-closing-the-loop.md) | ESS wave 7 — the loop closed over generated code, the second and third emitters, and the dual-target demonstration; W7.4 deferred |
| [`docs/plan/infra-wave-1-observe.md`](docs/plan/infra-wave-1-observe.md) / [`infra-wave-2-analyze.md`](docs/plan/infra-wave-2-analyze.md) | the infrastructure waves delivered so far — observation and IR, then graph, diagnosis, candidates and directions — and IW3/IW4 as what comes next |
| [`docs/plan/harness-wave-1-planning-plugin.md`](docs/plan/harness-wave-1-planning-plugin.md) | harness wave 1 — the planning store, its verbs and lifecycles, the Claude Code plugin, and the driver wave it deliberately does not open |
| [`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md) | the ESS waves, and what is deliberately outside them |
| [`docs/plan/wave-1-execution-core.md`](docs/plan/wave-1-execution-core.md) | the protocol's four waves, with their acceptance criteria |
| [`docs/plan/document-authoring-brief.md`](docs/plan/document-authoring-brief.md) | how to write a valid principle, workflow, profile or lifecycle |
| [`docs/reviews/`](docs/reviews/) | *snapshots, not maintained* — seven independent reviews: the vision, guard efficacy, next-wave feasibility, semantic-diff feasibility, infrastructure-design feasibility and an outside pre-wave-4 readiness review, all at `3647f80`, plus a full-repository review at `95e210f`. Each describes the commit it names; where one disagrees with this README, this README is current |
| [`CHANGELOG.md`](CHANGELOG.md) | what changed, per release |
| [`examples/development-passkeys/`](examples/development-passkeys/) | a worked protocol example with real command output |
| [`examples/billing/`](examples/billing/) | the normative executable system specification |
| [`examples/oracle-fixture/`](examples/oracle-fixture/) | a second specification, built to exercise what the oracle must prove |
| [`examples/billing-conformance/`](examples/billing-conformance/) | the closed loop, both directions: a run that completes a task and one that refuses to |
| [`examples/billing-realization/`](examples/billing-realization/) | the hand-written half of the synthesised workspace: one implementation per obligation in the generated plan |
| [`examples/gatepass/`](examples/gatepass/) | the dual-target demonstration's specification — visitor passes, and the one component whose `reached_by: network` derives the HTTP surface |
| [`examples/gatepass-realization/`](examples/gatepass-realization/), [`examples/gatepass-go-realization/`](examples/gatepass-go-realization/) | the hand-written halves of the two gatepass applications, written from the specification rather than from each other |
| [`docs/design/archive/`](docs/design/archive/) | v0.1 draft and artifact-model extension, kept for provenance |
| [`docs/guide/`](docs/guide/) | how to adopt the protocol, wire a harness, prove a backend, and specify a system |
| [`AGENTS.md`](AGENTS.md) | working agreement for humans and agents contributing here |

## Licence

Apache-2.0. See [LICENSE](LICENSE).
