---
title: Where this stands
sidebar_position: 1
description: What is implemented, per component, with the numbers the repository's own gate reports.
---

# Where this stands

Current as of the tag `0.7.1-infra-waves-1-4` (2026-08-21). The repository's gate, `task check`,
runs nine steps — formatting, clippy with warnings as errors, the test suite, rustdoc with warnings
as errors, and five drift checks that regenerate every schema, projection, suite and synthesized
tree and compare bytes — and at this tag reports **106 suites, 1811 tests, 0 failures, 0 clippy
warnings, 0 rustdoc warnings**.

CI runs the same nine steps; nothing lands that does not pass all of them. The gate needs the Go
toolchain, the `wasm32-unknown-unknown` Rust target and Node beside Rust's own, and a step whose
toolchain is missing fails and names it rather than skipping — a skipped check reads exactly like a
passing one.

## AEP — the protocol

The v0.2 scope is complete.

| | State |
|---|---|
| domain model, engine, documents, CLI | implemented |
| interaction contract, identity, audit | implemented, with an in-memory reference backend |
| conformance suites for backends | implemented — 16 suites, 3 levels, checked against a deliberately broken backend |
| running on a real project | **not yet**: a project can be discovered, but no team's work has been governed by this |

The document tree is 39 files — 3 protocols, 22 principles, 4 workflows, 5 profiles, 5 artifact
lifecycles — validated against the protocol vocabulary in CI, plus 12 generated JSON Schemas that CI
fails on if they drift from the Rust types.

## ESS — executable system specifications

Seven waves delivered. A specification is the source of its own documentation, contracts, tests and
the structural part of its own implementation in three targets; when it changes, the change is a
typed, queryable object.

| | State | Since |
|---|---|---|
| the typed model (`ess-domain`) | implemented | `0.3.0-ess-wave-1` |
| resolution, IR, diagnostics (`ess-compiler`) | implemented — an unresolved reference is unrepresentable | `0.3.1-ess-wave-2` |
| four projections: docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 (`ess-gen`) | implemented, committed output drift-checked | `0.3.2-ess-wave-3` |
| the specification as oracle: generated suites, runner, evidence (`ess-conformance`) | implemented — 29 + 31 + 12 scenarios across three example specifications, 13 deliberately wrong implementations all caught by their named scenario | `0.4.0-ess-wave-4` |
| semantic diff and impact closure (`ess-diff`) | implemented — ten construct families; impact narrows down to scenario and generated artifact, with the path attached | `0.5.0-ess-wave-5`, extended by wave 7 |
| structural synthesis (`ess-synth`) | implemented — a language-neutral plan (billing: 45 capabilities = 33 generated / 8 obligations / 4 refused) and three emitters: Rust, Go, browser | `0.6.0-ess-wave-6` through `0.7.0-ess-wave-7` |
| the claims made mechanical — invariant scans, full SHA-256 digests, property tests | implemented | `0.6.1-ess-wave-6.5` |
| one specification, two running applications | implemented — `examples/gatepass/` synthesized to Rust and Go, both started in every gate run and compared on records, seven exchanges and published documents | `0.7.0-ess-wave-7` |

One result worth knowing when weighing the approach: building the oracle changed the specification
language four times. All four gaps had been rendered without complaint by all four projections —
a document does not have to run — and two were found only by the synthesizer refusing to generate a
scenario. The pattern repeated at each layer: the generated suite caught a real defect in the code
generator before any human did, and the dual-target demonstration caught two of the six mutations
held against it by the two applications disagreeing.

## Infrastructure — observed, diagnosed, held to a desired state

Four waves delivered, scoped exactly as the vision allows: nothing in the workspace reaches a
cluster or holds a credential. An external scanner (`infra-scout`, its own repository) writes the
observation; this workspace reads the file, refuses it or compiles it.

| | State |
|---|---|
| observation model and content-addressed IR (IW1) | implemented — seventeen Kubernetes kinds, eleven `INFRA-*` refusal codes, secrets only as digests |
| typed dependency graph, diagnosis, candidates, directions (IW2) | implemented — twenty `INFRA-DIAG-*` rules, three `INFRA-PROP-*` candidate rules, an HTML view |
| desired state and simulation (IW3) | implemented — `infra-spec/1`, twelve expectation kinds, three-valued verdicts; a False without a gap or an Unknown without a reason is unrepresentable |
| gaps projected back as patches (IW4) | implemented — a reviewable patch tree: mechanical changes as patches, human decisions as obligations, contradictions as refusals; the round trip is asserted |
| the CLI | nine verbs: `protocol infra validate\|compile\|inspect\|graph\|diagnose\|view\|simulate\|diff\|project` |
| a real example | `examples/k3d-dev-cluster/` — observation, IR, desired state (28 expectations: 11 hold, 12 gaps, 5 undecidable), a drifted second scan, and the committed projection (9 patches, 16 obligations), all drift-checked |

## Not built

The compact list; [Limitations](./limitations.md) carries the consequences of each.

* No durable backend — the only implementation of the storage contract is in memory.
* No out-of-process conformance runner, on either side.
* No behavioural synthesis — every algorithm is an obligation, by design.
* No attested evidence — `independent: true` is structural, not cryptographic.
* No federated artifact graphs across repositories.
* Obligations are plan entries, not yet artifacts a task can own (deferred by decision).

---

**Sources.** `task check` run at the working tree of `0.7.1-infra-waves-1-4`, 2026-08-21 (exit 0;
106 `test result:` lines, 1811 tests passed, 0 failed); `README.md` § *Status*; `AGENTS.md`
§ *Current state*; `CHANGELOG.md` §§ *0.7.0*, *0.7.1*; `git tag -n99`; document counts from
`protocols/`, `principles/`, `workflows/`, `profiles/`, `artifacts/lifecycles/`.
