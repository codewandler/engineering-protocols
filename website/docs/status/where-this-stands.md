---
title: Where this stands
sidebar_position: 1
description: What is implemented, per component, with the numbers the repository's own gate reports.
---

# Where this stands

Current as of the tag `0.10.0-horizons-dogfood-lab` (2026-08-21). The repository's gate, `task
check`, runs ten steps — formatting, clippy with warnings as errors, the test suite, rustdoc with
warnings as errors, and six drift checks that regenerate every schema, projection, suite,
synthesized tree, infrastructure IR and the delivered-waves record and compare bytes. This page
states no suite or test counts: four hand-written counts drifted apart within the repository's first
48 hours, so the count now lives in exactly one place — the gate's own output. Run `task check` for
the measurement.

CI runs the same ten steps; nothing lands that does not pass all of them. The gate needs the Go
toolchain, the `wasm32-unknown-unknown` Rust target and Node beside Rust's own, and a step whose
toolchain is missing fails and names it rather than skipping — a skipped check reads exactly like a
passing one.

## The headline: the protocol has now governed a real run, and the run stopped

For most of this project's life the honest answer to *has this governed anybody's work?* was **no**.
On 2026-08-21 it changed. `protocol drive` walked a story out of this repository's own planning
store under the `development.driven` profile, with four headless model sessions doing the work and
the Claude Code plugin's hooks as the enforcement arm.

**It blocked in `establish_verifiers`, four states short of the person it was meant to stop at**,
for two reasons the engine printed:

```text
blocked because:
  - establish_verifiers -> implement: ? artifact specification (approved) — declared: specification:agent-charter-eval-cases (draft) [principle spec-driven]
  - establish_verifiers -> implement: ✗ test.first_result == failed — test.first_result = passed [principle test-driven]
```

Both are correct refusals. The model wrote its failing checks as shell scripts — the idiom the
story's own acceptance is written in — and the shipped step map can only run `cargo`, so the
verifier the driver ran came back green and `test-driven` refused to advance. Nothing was changed to
make the run go through; the run is the finding, and the two reasons are now rows on the gap
register.

What the same run shows working, read out of its own records rather than from a summary:

| | |
|---|---|
| transitions | 3, every one the engine's, in the snapshot's audit trail |
| model sessions | 4, `is_error: false` in each |
| hook decisions | 80 — 69 allow, 11 deny |
| `permission_denials` in the transcripts | 11, one for one with the denies |
| planning store afterwards | 47 → 58 artifacts, `protocol artifact validate` exit 0 |
| tracked files modified | 0 |
| turns / session time / cost | 224 / 34 m 39 s / $15.42 |

The record, with the acceptance line that admits a failed run as an outcome, is
`docs/plan/harness-wave-4-governed-dogfood.md` § *W4.1 — The first run*.

## AEP — the protocol

The v0.2 scope is complete.

| | State |
|---|---|
| domain model, engine, documents, CLI | implemented — 17 top-level CLI verbs |
| interaction contract, identity, audit | implemented, with an in-memory reference backend |
| conformance suites for backends | implemented — 16 suites, 3 levels, 89 properties at `full`, checked against a deliberately broken backend |
| a durable planning store | implemented as markdown (`aep-backend-markdown`) — **not** an implementation of the storage contract; see [Limitations](./limitations.md) |
| a reference driver | implemented — `protocol drive run\|status\|resume`, with `command`, `llm` and `operator` steps |
| running on a real project | **once**, and it blocked; see above |

The document tree is 44 files — 3 protocols, 22 principles, 4 workflows, 6 profiles, 8 artifact
lifecycles and 1 step map — validated against the protocol vocabulary in CI, plus 15 generated JSON
Schemas that CI fails on if they drift from the Rust types.

### Evidence gained a time

An evidence record now carries two dates. `observed_at` is when somebody looked, is required, is the
caller's, and is the identity of the fact; `produced_at` remains the engine's. A date in the future
is refused (`observation_in_future`) rather than stored.

A requirement may declare a `horizon`. Past it the requirement reads `Unknown` — never `False`,
because a lapsed check has not failed, nobody has run it — and `evidence.lapsed` sits beside
`evidence.missing` so a stale gate is distinguishable from an empty one. The horizon is on the
requirement and nowhere else: a record has no horizon field, and a source scan over five crates
refuses any code path that would mutate one.

`protocol evidence scan` reads the same dated-claim convention out of human-written markdown and
reports its own coverage. Against the ground-truth corpus at `examples/evidence-horizons-corpus/`,
contributed by an outside adopter: **43 occurrences, 43 records, 0 unparsed**.

### The harness side

| | State |
|---|---|
| the markdown planning store (`aep.planning-md/1`) | implemented — this repository's own plan is 59 artifacts under `.engineering/planning/` |
| `protocol artifact new\|move\|relate\|list\|board\|graph\|validate\|kinds\|relations\|lifecycle` | implemented — a status move is checked against the kind's lifecycle |
| the Claude Code plugin | implemented — one `planning` skill, two agents (`decomposer` writes, `plan-reviewer` only reads), two `PreToolUse` hooks (`store-integrity` over edits, `driven-surface` over the shell) and an eval that spends real money on a real headless session and judges it two ways |
| `protocol workflow render` | implemented — `svg`, `html`, `png`, `tui`, and `--watch` to redraw a live run |

### Transcript conformance

A typed specification over what an agent run actually did, built as the third observation domain in
the repository after a specification and a cluster, and taking the same shape: an authored
`trace-spec/1` document, a content-addressed `trace-ir/1`, and `ok`/`gap`/`unk` verdicts where the
third value means the adapter did not understand the event.

`protocol trace check|inspect|evidence`, **51 expectation kinds**, and two severities: an
expectation gates by default, `Advisory` is evaluated and reported and gates nothing. `trace
evidence` mints an AEP record of kind `trace_conformance` — a summary, never the citations.

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
| observation model and content-addressed IR (IW1) | implemented — seventeen Kubernetes kinds, eleven `INFRA-*` refusal codes over the observation, secrets only as digests |
| typed dependency graph, diagnosis, candidates, directions (IW2) | implemented — twenty `INFRA-DIAG-*` rules, three `INFRA-PROP-*` candidate rules, an HTML view |
| desired state and simulation (IW3) | implemented — `infra-spec/1`, twelve expectation kinds, three-valued verdicts; a False without a gap or an Unknown without a reason is unrepresentable |
| gaps projected back as patches (IW4) | implemented — a reviewable patch tree: mechanical changes as patches, human decisions as obligations, contradictions as refusals; the round trip is asserted |
| the CLI | nine verbs: `protocol infra validate\|compile\|inspect\|graph\|diagnose\|view\|simulate\|diff\|project` |
| a real example | `examples/k3d-dev-cluster/` — 28 expectations: 11 hold, 12 gaps, 5 undecidable; the committed projection closes 9 gaps with generated changes, leaves 16 as obligations and refuses none, across 2 patch files and 3 new objects; all drift-checked |

## Not built

The compact list; [Limitations](./limitations.md) carries the consequences of each.

* No durable implementation of the AEP storage contract — the markdown planning store is durable but
  does not go through the contract, so the 16 conformance suites do not run against it.
* No out-of-process conformance runner, on either side.
* No behavioural synthesis — every algorithm is an obligation, by design.
* No attested evidence — `independent: true` is structural, not cryptographic.
* No federated artifact graphs across repositories.
* Obligations are plan entries, not yet artifacts a task can own (deferred by decision).
* The shipped step map can only verify Rust, which is what stopped the first governed run.
* A second harness has never been driven, so the harness-neutrality claim is still untested.

---

**Sources.** `git tag -l`; `target/debug/protocol validate`, `conformance --level full --format
json`, `evidence scan`, `--help` (verb counts) at `0.10.0-horizons-dogfood-lab`;
`Taskfile.yml` (the ten gate steps); `docs/plan/harness-wave-4-governed-dogfood.md` § *W4.1*;
`docs/plan/gap-register.md`; `examples/evidence-horizons-corpus/expected.json`;
`examples/k3d-dev-cluster/simulation.json` and `projection/SUMMARY.md`;
`generated/rust/billing/PLAN.md`; `suites/generated/*/suite.json`;
`crates/trace-domain/src/spec.rs` (the expectation kinds); `CHANGELOG.md` § *0.10.0*.
