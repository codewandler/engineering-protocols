---
title: Where this stands
sidebar_position: 1
description: What is implemented, what is in progress, and what is not built — with the numbers the repository's own gate reports.
---

# Where this stands

Honest as of the tag `0.7.0-ess-wave-7`. The repository's gate, `task check`, runs nine steps —
formatting, clippy with warnings as errors, the test suite, rustdoc with warnings as errors, a schema
drift check, a projection drift check, a conformance-suite drift check, an observation-IR drift check
and a synthesis check that regenerates the three generated trees, compiles them (`cargo check`;
`gofmt -l`, `go build`, `go vet`; `cargo build --target wasm32-unknown-unknown` plus a Node-driven
boundary test), runs the committed suite against the Rust workspace and executes the dual-target
demonstration — and reports **94 suites and 1693 tests, with 0 clippy warnings and 0 rustdoc
warnings**.

Nothing lands that does not pass all nine, and CI runs the same nine. The gate now needs the Go
toolchain, the `wasm32-unknown-unknown` Rust target and Node beside Rust's own, and says which is
missing rather than skipping.

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

Seven waves delivered. A specification is now the source of its own documentation, its own
contracts, its own tests and the structural part of its own implementation — in three targets — and
when it changes, the change itself is a typed, queryable object that narrows what the change owes,
down to the generated artifact.

| | State |
|---|---|
| the typed model | implemented, `0.3.0-ess-wave-1` |
| resolution, IR and diagnostics | implemented, `0.3.1-ess-wave-2` — an unresolved reference is unrepresentable |
| four projections: docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 | implemented, `0.3.2-ess-wave-3` — output committed and drift-checked |
| the model reconciled before the oracle was built | implemented, `0.3.3-ess-wave-3.5` — 20 gates closed before the oracle was allowed to start |
| the join: artifact kind, evidence kind, the `ess-conformance` principle | implemented — a task can be blocked until something proves conformance |
| the specification as an oracle: generated suites, a conformance runner, evidence | implemented, `0.4.0-ess-wave-4` — 27 scenarios from the normative example and 31 from a second fixture at that tag, committed and drift-checked |
| what a change to a specification invalidates | implemented, `0.5.0-ess-wave-5` — a semantic delta over the compiled models, and an impact closure that narrows what is owed and can never say "still valid" |
| generated Rust code — structural synthesis | implemented, `0.6.0-ess-wave-6`: a language-neutral plan giving every capability exactly one disposition — generated, obligation or refused, never a guess — and a committed zero-dependency Rust workspace the committed suite passes unchanged, while a deliberately corrupted linkage fails exactly the scenario that exists to catch it |
| the hardening batch — the claims made mechanical | implemented, `0.6.1-ess-wave-6.5` — build-failing invariant scans, the full SHA-256 model digest, `proptest` phase 1, declared event-payload provenance, and value-object invariant scenarios growing the billing suite 27→29 |
| the diff learns about generated artifacts | implemented, `0.7.0-ess-wave-7` (W7.1) — every generated artifact carries a `contract_digest`, the digest of the model slice it derives from, beside its whole-model digest; `ess impact` narrows "the specification moved, everything is owed" to the artifacts whose slice the delta reached, with the path one hop per line. On the fixture pair, six changes owe 16 of 22 artifacts. A stale contract digest fails the gate as its own finding |
| entities, commands, views and bindings join the delta | implemented, `0.7.0-ess-wave-7` (W7.2) — ten construct families, 74 new typed change kinds; where a construct carries a predicate the comparison is conservative canonical equality: a respelling is silence, anything canonically different is *changed*, and implication stays refused. The fail-closed catch-all still owes everything for what has no family: conversions, workloads, and each domain's naming |
| three emitters behind one plan | implemented, `0.7.0-ess-wave-7` (W7.3, W7.3b) — Rust, a standard-library-only Go module, and a browser realization: a `WebAssembly` bridge and a page built at load time from an emitted `catalog.json`, holding no model of its own. `PLAN.md` and `plan.json` are byte-identical in all three trees; what a target holds more weakly or cannot represent is a named weakening or a target-stage refusal in a `TARGET.md` beside the plan, never a silent downgrade |
| one specification, two applications, one surface | implemented, `0.7.0-ess-wave-7` (W7.5) — `examples/gatepass/` synthesised to Rust and Go, both serving HTTP because the model's one new word, `reached_by: network`, *derives* it: the only projected contract for a command surface is the `OpenAPI` document. Both binaries are started in every gate run: one startup record semantically equal outside `runtime`, seven exchanges answered with identical statuses and equal bodies, and `/openapi.json` and `/docs` byte-identical between the applications and to the committed artifacts |

Building the oracle changed the specification language four times, and that is the part worth
knowing. Two of the four gaps were found by review before a line of the synthesizer existed; the
other two were found by the synthesizer **refusing to generate a scenario** and saying which construct
it could not test. Every one of the four had been rendered without complaint by all four projections,
because a document does not have to run. The pattern repeated at each layer since: the generated
suite caught a real defect in the code generator before any human did, and the dual-target
demonstration caught two of the six mutations held against it by the two applications disagreeing.

## Infrastructure — observed, compiled, diagnosed

A third subject matter entered the repository in this release, scoped exactly as the vision allows:
nothing in this workspace reaches a cluster or holds a credential. An external scanner
(`infra-scout`, its own repository) writes an `infra-observation/1` bundle; this workspace reads the
file, refuses it or compiles it.

| | State |
|---|---|
| the observation model and the compiled IR | implemented (IW1, extended by IW2.5) — seventeen Kubernetes kinds; a valid bundle compiles to a content-addressed `infra-ir/1` document, danglings carried as typed unresolved facts, a plain-string secret value refused without echoing it |
| the typed dependency graph | implemented (IW2) — edges exist where a reference resolved, each carrying the sites in the dependent that state it; a pod whose controller cannot be derived is a typed fact with the reason, never a guess |
| diagnosis | implemented (IW2) — twenty rules, `INFRA-DIAG-001`…`020`, each finding coded, severity-registered and carrying named evidence; findings never fail the run, because observed infrastructure is allowed to be wrong |
| invariant candidates and directions | implemented (IW2.5) — `INFRA-PROP-001`…`003` uniformity candidates with exceptions carried as evidence, and a severity-ranked directions summary grouped by shared root cause |
| the CLI | six verbs: `protocol infra validate\|compile\|inspect\|graph\|view\|diagnose` — `inspect --properties` for per-workload facts, `graph --format json\|mermaid\|html`, `view` writing and opening the self-contained HTML component page, `diagnose --candidates --directions` |
| a real example | `examples/k3d-dev-cluster/` in the repository — a trimmed, reviewed observation derived from a real k3d scan and the IR it compiles to, drift-checked in the gate |

The next infrastructure waves are named on the wave pages, not yet built: **IW3** — desired state, a
declared target model, semantic diff observed↔desired, and simulation of a change before anything
applies it — and **IW4** — manifests projected *from* the model, closing the loop the ESS side
already closed for code.

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
  31 from a second fixture, 12 from the demonstration specification — committed and drift-checked in
  CI, with every construct that got *no* scenario listed with the reason it got none.
* **The suite is known to bite.** Thirteen implementations that are wrong in exactly one way each
  are run against it, and the matrix asserts *which named scenario* catches each fault, plus how
  many unrelated scenarios it is allowed to disturb. All thirteen are caught.
* **A change to a specification is a typed object** — over ten construct families now — and what it
  invalidates is answered at both granularities: which conformance scenarios are owed again, and
  which generated artifacts, each with the path that produced the answer and no vocabulary for
  "still valid".
* **A specification synthesises the code that was never yours to write, in three targets.** A
  language-neutral plan, then a zero-dependency Rust workspace, a standard-library-only Go module
  and a browser realization whose page holds no model — with every non-generated capability carried
  as a named obligation or an explained refusal, and every target's honest limits in a `TARGET.md`.
* **One specification runs as two applications, provably.** Every gate run builds and starts the
  Rust and Go applications synthesised from `examples/gatepass/`, and holds their startup records,
  their answers to seven exchanges and their published documents to each other and to the committed
  bytes.
* **A conformance run closes, or refuses to close, a real task.** The run mints its own evidence
  record in the process that executed the suite; a passing run completes the task, a failing one
  leaves it blocked and names the principle that refused.
* **A scanned cluster becomes a validated, diagnosable model.** An observation bundle compiles to a
  content-addressed IR; `infra graph` renders what depends on what with the evidence on every edge,
  and `infra diagnose` reports what is wrong under stable codes — as a report about a cluster that
  is allowed to be wrong, never a gate.

## What does not work yet

* **No durable backend.** The only implementation of the contract is in memory, and it forgets
  everything when the process exits.
* **Generated code is structural, not behavioural.** A specification synthesises types, lifecycles,
  ports, transports and a plan; every algorithm is a typed obligation someone still has to
  implement, and behavioural synthesis is rejected in the roadmap rather than pending.
* **Obligations are plan entries, not artifacts or tasks.** W7.4 — an obligation becoming a typed
  artifact a task can own and evidence can close — stays deferred by decision, its precondition now
  met.
* **The delta still has a fail-closed arm.** Conversions, workloads and a domain's naming have no
  compared family; a change there owes the whole suite, stated as such. Predicates are compared
  only for canonical equality — implication is refused, so a provably weaker rewrite still reads
  as *changed*.
* **The demonstration is not a deployment.** The generated servers speak plain HTTP with no
  authentication and no TLS, take one connection at a time, and publish no `servers` block because
  the model has no URL. And the committed gatepass suite is not run against the two applications —
  the wire demonstration and the suite are two separate proofs.
* **No federated artifact graphs.** An artifact manifest describes one project; cross-repository
  references are resolved by hand.
* **No remote conformance runner, on either side.** Proving your own backend means calling the
  conformance crate from your own test suite, and the same is true of the ESS oracle: the CLI runs
  the two reference implementations it was compiled with and says so outright. Nothing here speaks to
  an implementation over a socket.
* **Infrastructure is observed, not desired.** There is no declared target model to diff a cluster
  against and no manifest projection — IW3 and IW4 are named, not built. The scanner lives outside
  this workspace by design, and raw scans are trusted to it.
* **`independent: true` is structural, not attested.** It says the record came from the runner rather
  than from the agent under review, checked by one comparison. Nothing signs it, and there is no key
  anywhere in the workspace — see [what you still have to trust](./what-you-have-to-trust.md).
* **The OpenAPI and AsyncAPI envelopes are checked structurally**, not against their own
  meta-schemas — see [what you still have to trust](./what-you-have-to-trust.md).

## The next honest milestone

For AEP it is not a feature. It is a team whose work the protocol actually governs, and that has not
happened yet. For ESS, wave 7 closed the loop it named, and what remains of it — W7.4, obligations
becoming artifacts and tasks — is deferred by decision rather than scheduled. For infrastructure it
is IW3: a declared desired state, and a semantic diff between what a cluster is and what it should
be, over exactly the per-workload facts IW2 already extracts.

---

**Sources.** `README.md` § *Status*; `docs/VISION.md` § *Where this stands*; `Taskfile.yml` (the
nine steps of `check`); `docs/plan/ess-roadmap.md`; `docs/plan/ess-wave-7-closing-the-loop.md`;
`docs/plan/infra-wave-1-observe.md` and `infra-wave-2-analyze.md` (the infra scope, and IW3/IW4 as
the named next waves); `CHANGELOG.md` § *0.7.0*. Suite and test counts from `cargo test --workspace`
at the tag (94 suites, 1693 tests, 0 failed); the gatepass plan's 29 = 22 / 5 / 2 from
`generated/rust/gatepass/PLAN.md`; scenario counts from `suites/generated/README.md`; the
16-of-22 artifact narrowing from `protocol ess impact` run against `examples/revision-pair/`; the
infra rule and candidate counts from `CHANGELOG.md` § *0.7.0* and `crates/infra-analyze`. Document
and schema counts verified by counting files under `protocols/`, `principles/`, `workflows/`,
`profiles/`, `artifacts/lifecycles/` and `schemas/generated/`.
