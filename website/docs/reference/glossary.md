---
title: Glossary
sidebar_position: 4
description: The project's terms, defined once.
---

# Glossary

| Term | Definition |
|---|---|
| **AEP** | Agentic Engineering Protocol — the half that governs *how* engineering work is performed. |
| **ESS** | Executable System Specification — the half that specifies *what* software must exist. |
| **ADP / AOP** | The development-specific (`adp/1`) and operations-specific (`aop/1`) protocols extending `aep/1` with their own phases, fact families and vocabularies. |
| **Protocol (document)** | The vocabulary declaration: which capabilities, evidence kinds, verifiers, phases and observable fact families exist. |
| **Principle** | One enforceable rule: when it applies, what it requires, by when, and who may attest it. |
| **Profile** | A bundle of protocol, workflow, principles, capability policy and completion condition that a task names to be governed. |
| **Workflow** | A validated state machine whose transitions are guarded by predicates over evidence. |
| **Phase** | A label on workflow states (`implementation`, `verification`, …) that principles time obligations against, so one principle works across workflows. |
| **Task** | The unit of governed work: objective, kind, profile, declared context facts, artifact manifest. |
| **Artifact** | A referenced engineering document — a spec, design, ADR, review — with a kind, a status from its kind's lifecycle, and relations to other artifacts. The manifest holds references, not copies. |
| **Capability** | A named permission (`repository.write`, `production.write`, …) every governable action maps onto. Default deny; `deny` beats `require_approval` beats `allow`. |
| **Approval floor** | Capabilities the protocol refuses to let any profile grant outright — `production.write` and `deployment.create:production` under `aep/1`. |
| **Evidence** | A typed record of an observation — test result, approval, diff, conformance run — with a kind, a producer and provenance. Facts are projected from evidence; predicates read facts. |
| **Producer** | Who made an observation: `Agent` or `Verifier`. Requirements marked `independent: true` are satisfied only by verifier-produced records. |
| **Verifier** | A class of independent fact-producer: test-runner, static-analyzer, human-review, conformance-runner, … |
| **Truth** | The three-valued result of predicate evaluation: `True`, `False`, `Unknown`. Only `True` permits a transition; the CLI renders `False` as `✗` and `Unknown` as `?`. |
| **Harness** | The system that runs an agent and asks the engine what is owed, permitted and done. The engine never observes anything itself. |
| **Execution** | One task's run through a workflow: state, evidence in submission order, event stream, audit trail. Snapshots persist it; the plan is always re-resolved, never stored. |
| **Backend** | An implementation of the AEP storage contract (`CommandService` + `QueryService`). The reference implementation is in-memory. |
| **Contract conformance** | Whether a *backend* implements the AEP contract — checked by `protocol conformance`. |
| **Semantic conformance** | Whether an *implementation* satisfies a specification — checked by `protocol ess conform`. |
| **Specification (ESS)** | The typed model of a system: domains, types, entities, commands, events, errors, views, actors, components, bindings, topology. |
| **IR** | The compiled, normalized form of a specification, in which an unresolved reference is unrepresentable. Everything downstream consumes the IR. |
| **Projection** | A derived artifact: documentation, JSON Schema, OpenAPI, AsyncAPI. Deterministic, provenance-stamped, drift-checked. |
| **Outcome** | One declared result branch of a command — including refusal branches, which the model gives no way to omit. |
| **Binding** | A declared event→command reaction across contexts, with required `delivery:` and `on_failure:` policies. |
| **Conversion** | An explicit, directional, justified permission to treat one newtype as another at a context crossing. |
| **Oracle** | The specification acting as judge: the generated conformance suite plus the runner that executes it. |
| **Scenario** | One generated conformance check, named after the obligation it verifies (`billing.invoice.CreateInvoice/outcome/rejected`). |
| **Model digest / spec digest** | The SHA-256 content identity of a resolved model. Conformance evidence binds to it; the check fails closed. |
| **Contract digest** | The digest of the model *slice* a generated artifact derives from — what lets impact analysis narrow "regenerate everything" to the artifacts actually reached. |
| **Semantic delta** | The typed difference between two compiled revisions (`ess diff`): widenings, narrowings and changes over ten construct families. |
| **Impact closure** | What stood on what moved (`ess impact`): scenarios owed again and artifacts owed regeneration, each with its dependency path. It narrows; it never says "still valid". |
| **Synthesis plan** | The language-neutral output of `ess synthesize`: every capability of a specification dispositioned as generated, obligation or refused, with reasons. Byte-identical across emitters. |
| **Obligation (synthesis)** | A named piece of work the specification cannot determine — every algorithm is one — implemented by a human in a realization crate against a declared seam. |
| **Realization** | The hand-written crate implementing a plan's obligations, linked into the generated tree. |
| **`TARGET.md`** | The per-emitter declaration of what that target holds more weakly or cannot represent — a named weakening, never a silent downgrade. |
| **Observation bundle / infra IR** | A scanner-produced snapshot of a cluster (`infra-observation/1`) and its compiled, content-addressed form (`infra-ir/1`). |
| **Expectation** | One clause of a declared infrastructure desired state (`infra-spec/1`), evaluated three-valued: `ok`, `gap`, `unk`. |
| **Gate** | The repository's own check suite, `task check` — nine steps, all mirrored in CI, including every drift check. |
