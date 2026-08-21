---
title: Architecture overview
sidebar_position: 1
description: What the pieces are, what runs where, and where the project's responsibility deliberately ends.
---

# Architecture overview

`engineering-protocols` is a **library and a specification, not a service**. Nothing in it calls a
model, holds a credential, schedules an agent, or reaches a network. It consumes typed documents and
recorded evidence, and produces deterministic answers, generated artifacts and refusals. Everything
that acts — the agent, the CI system, the deployment pipeline — lives outside and asks it questions.

## The pieces

```text
            AEP (how work is performed)          ESS (what must exist)
            ───────────────────────────          ─────────────────────
documents   protocols/ principles/ workflows/    a specification
            profiles/ artifacts/lifecycles/      (one file or a directory)
                 │                                    │
                 ▼                                    ▼
engine      aep-engine: resolve, authorize,      ess-compiler: validate, resolve
            evaluate, transition                 into a normalized IR
                 │                                    │
                 ▼                                    ▼
outputs     plans, decisions, refusals,          docs, JSON Schema, OpenAPI, AsyncAPI,
            audit trail                          conformance suites, semantic diffs,
                                                 synthesized structural code
                 │                                    │
                 └──────────── evidence ──────────────┘
                    a conformance run becomes an AEP
                    evidence record; completion is a
                    predicate over such records
```

### AEP: the protocol side

| Component | Role |
|---|---|
| `aep-domain` | the typed model: tasks, principles, workflows, capabilities, evidence, predicates, audit |
| `aep-engine` | resolution, evaluation and transitions — deterministic, clock-injected, no I/O of its own |
| `aep-contract` | the storage-independent command/query contract for engineering entities |
| `aep-backend-memory` | the in-memory reference implementation of that contract |
| `aep-conformance` | black-box suites a backend runs to prove it implements the contract |
| `adp-domain`, `aop-domain` | development-specific and operations-specific vocabularies |
| `protocol-cli` | the reference CLI over all of it |

The document tree (`protocols/`, `principles/`, `workflows/`, `profiles/`,
`artifacts/lifecycles/` — 39 files in this repository) is data, not code. Teams vendor it and add
their own documents beside it; see [Govern a task](../guides/govern-a-task.md).

### ESS: the specification side

| Component | Role |
|---|---|
| `ess-domain` | the typed model of a specification: domains, types, entities, commands, events, views, actors, components, bindings, topology |
| `ess-compiler` | resolution into a normalized IR — an unresolved reference is unrepresentable in the output |
| `ess-gen` | deterministic projections: Markdown docs, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 |
| `ess-conformance` | scenario synthesis from the IR, a runner, and evidence minting |
| `ess-diff` | the semantic delta between two revisions, and the impact closure over it |
| `ess-synth` | a language-neutral synthesis plan and three emitters: Rust, Go, and a WebAssembly browser bridge |

### Infrastructure: a second instance of the same pattern

`infra-domain`, `infra-compiler`, `infra-analyze`, `infra-spec` and `infra-project` apply the ESS
shape to observed Kubernetes infrastructure: an external scanner writes an observation bundle, this
workspace validates and compiles it, diagnoses it, evaluates it against a declared desired state,
and projects gaps back as a reviewable patch tree. Nothing in the workspace reaches a cluster; the
scanner is a separate repository holding the credentials. See
[Check infrastructure against a desired state](../guides/check-infrastructure.md).

## Design properties that hold everywhere

These are the cross-cutting rules; [Design principles](./design-principles.md) covers each with its
enforcement mechanism.

* **Deterministic.** Same validated state plus same evidence produces the same decision; generated
  output is byte-identical across runs. The engine takes an injected clock so executions replay.
* **Parse, then validate.** Documents deserialize into raw types and become domain types only
  through validation; validated types cannot be deserialized directly.
* **Errors accumulate.** A document with four broken references reports four errors with four
  stable codes, not the first one.
* **Nothing here observes anything.** Every answer is a function of the documents plus submitted
  evidence. The engine never manufactures a fact.
* **Generated output is committed and drift-checked.** Schemas, projections, suites and synthesized
  trees are all regenerated in CI and compared byte-for-byte against what is committed.

## Boundaries: what this is deliberately not

Stated so it can be held.

* **Not an LLM orchestration framework.** Nothing calls a model or holds a prompt. The harness does
  that; the protocol answers the harness's questions.
* **Not a CI system, an incident-management product, a workflow engine or a message broker.**
  External systems do the work; this project decides what the results permit.
* **Not a policy language meant to replace OPA.** The subject is engineering work and the software
  it produces, not general authorisation.
* **Not a mandate for microservices, CQRS or event sourcing.** A component is a unit of ownership;
  whether it ships as a process or a module is a separate decision, and transports are projections
  of the model, not part of it.
* **Not a deployment platform.** Compiling a specification into a file that *describes*
  infrastructure, and judging whether an observed state conforms, is in scope. Operating a system —
  calling a cloud API, holding a credential, applying a plan, watching a rollout — is not.

The responsibility, in one sentence per half: define the semantics by which engineering work can be
constrained, evidenced, verified and progressed — and the semantics by which a software system can
be specified once and compiled into its contracts, its tests and as much of itself as the
specification safely determines.

---

**Sources.** `README.md` § *Repository layout*; `AGENTS.md` § *What this repository is*, § *Current
state*, § *Invariants*; `docs/VISION.md` § *What this is deliberately not*.
