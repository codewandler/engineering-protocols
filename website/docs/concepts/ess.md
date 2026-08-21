---
title: "ESS: executable system specifications"
sidebar_position: 4
description: The specification model, the compile pipeline, and the five things derived from one document — docs, contracts, tests, diffs and structural code.
---

# ESS: executable system specifications

An **Executable System Specification** describes a system semantically: `CreateInvoice` is a
command, and `POST /invoices/commands/create-invoice` is one way to expose it. That distinction is
the whole design. The same specification can compile to a modular monolith or to distributed
services without the domain model changing, and a generated test is a statement about the system,
not about its HTTP layer.

## The model

A specification — one YAML file or a directory — declares:

| Construct | What it is |
|---|---|
| **system** | the root: name, version, the domains it contains |
| **types** | newtypes, structs, enums, unions — with invariants (`amount >= 0`) that travel into every projection |
| **entities** | identity-bearing state with a lifecycle: states, transitions, and invariants that must hold |
| **commands** | the only way state changes; each declares its input and its **outcomes** — including the refusal branches, which the model gives no way to omit |
| **events** | facts a command's outcome emits |
| **errors** | declared domain refusals |
| **views** | read models: what they show, filtered by what |
| **actors** | who may invoke which commands |
| **components** | units of ownership; `reached_by: network` states where callers are without naming a protocol |
| **bindings** | event → command reactions across contexts, including what happens when they fail |
| **topology** | how components group into deployable units — a separate file, because it is a separate decision |

Illegal lifecycle transitions are illegal **by absence**: there is no arrow, and no second rule
forbidding them, because two places for one truth eventually disagree. The generated documentation
lists the absent pairs explicitly, derived from the same transitions.

## The pipeline

```text
source ──validate──► consistent?  ──compile──► normalized IR ──┬─► generate   docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0
                                                               ├─► conform    scenario suite → runner → evidence
                                                               ├─► diff       semantic delta between two revisions
                                                               ├─► impact     what the delta invalidates
                                                               └─► synthesize language-neutral plan → Rust / Go / browser code
```

`validate` answers "is this document consistent" and reports every problem in one run. `compile`
resolves every name to what it points at — in the IR, an unresolved reference is unrepresentable.
Everything downstream consumes the IR, and compiling the same source twice is byte-identical.

## What gets derived

### Projections (`ess generate`)

| Kind | Output | Why it exists |
|---|---|---|
| `docs` | Markdown with Mermaid diagrams | the cheapest completeness check: a construct with no rendering is a hole in a page a person reads |
| `schema` | one JSON Schema per command input, event and error payload | the type system, projected without losing its distinctions — newtypes stay separate definitions |
| `openapi` | one OpenAPI 3.1 document per component | the specification *is* the HTTP contract |
| `asyncapi` | one AsyncAPI 3.0 document per component | the same for messaging, including what happens when a binding fails |

Every artifact carries provenance: specification version, a digest of the resolved model, and the
digest of the model *slice* it derives from (`contract_digest`). Committed output is drift-checked
in CI. See [the worked example](../examples/specification-to-contracts.md) for real input and
output side by side.

### The conformance suite (`ess conform`)

The specification acts as an oracle: `synthesize` derives one scenario per obligation the
specification states — each declared outcome, each lifecycle move, each move that must *not* be
honoured, each invariant, each binding claim. `run` executes them against an implementation;
`evidence` turns the result into an AEP evidence record.

A construct the specification does not say enough about to test is **refused, not omitted** — the
refusal prints beside the scenarios, because a suite quietly holding fewer checks than the
specification requires is the one failure a passing run cannot show. See
[Verify an implementation](../guides/verify-conformance.md).

### The semantic delta (`ess diff`, `ess impact`)

`diff` compares two compiled revisions: moving declarations between files, renaming files,
reordering blocks and rewriting comments report **nothing**; removing a currency variant reports one
narrowing. `impact` computes what stood on what moved — which conformance scenarios are owed again
and which generated artifacts are owed regeneration, each with the hop-by-hop dependency path that
explains it. It narrows what a change owes; it never claims a result still holds. See
[Track specification change](../guides/track-change.md).

### Structural synthesis (`ess synthesize`)

A language-neutral **synthesis plan** gives every capability of the specification exactly one
disposition: *generated*, *obligation* (a named piece of work a human must implement — every
algorithm is one), or *refused* (with the reason). Three emitters render the plan — a
zero-dependency Rust workspace, a standard-library-only Go module, and a WebAssembly browser
bridge — and the plan is byte-identical across all three trees. What a target holds more weakly is
declared in a `TARGET.md` beside the plan, never silently downgraded.

Behaviour is **never** generated. The generated billing workspace, linked with the hand-written
realization of its eight obligations, passes the committed 29-scenario suite unchanged — and a
deliberately corrupted linkage fails exactly the scenario that exists to catch it. See
[Synthesize code from a specification](../guides/synthesize.md).

## The same pattern, pointed somewhere else

The pipeline shape — observe, normalize into a content-addressed IR, declare a desired state, judge
three-valued — is reused twice more, which is the strongest evidence available that it is a shape
and not a special case.

**Infrastructure.** The `infra-*` crates read an observation bundle from an external scanner and
compile it to a content-addressed IR; a typed graph and twenty coded diagnosis rules read it; a
declared desired state (`infra-spec/1`, twelve expectation kinds) evaluates against a snapshot; and
a gap projects back as a reviewable patch tree in which every value either came from the gap or is a
named obligation for a human. Nothing reaches a cluster. See
[Check infrastructure](../guides/check-infrastructure.md).

**Agent runs.** The `trace-*` crates read a run transcript, normalize it to `trace-ir/1`, and hold
it against an authored `trace-spec/1` document of 51 possible expectation kinds. Same three values,
and a passing check mints an AEP evidence record. See
[Check a transcript](../guides/check-a-transcript.md).

---

**Sources.** `docs/guide/specification.md`; `crates/ess-domain/` through `crates/ess-synth/`;
`crates/ess-diff/src/lib.rs` (the ten construct families); `crates/infra-spec/src/spec.rs` and
`crates/trace-domain/src/spec.rs` (the expectation-kind counts); `examples/billing/` (the normative
specification); `suites/generated/*/suite.json` (the scenario counts);
`generated/rust/billing/PLAN.md`; `CHANGELOG.md` §§ *0.7.1*, *0.8.0*.
