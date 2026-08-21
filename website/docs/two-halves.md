---
title: The two halves
sidebar_position: 3
description: AEP governs how work is performed. ESS specifies what software must exist. They meet at evidence, and the join is a document you can read.
---

# The two halves

## AEP — how work is performed

**AEP** turns methodology into documents a program executes: principles with timed obligations,
workflows whose transitions are guarded by evidence, capabilities that default to denied, artifacts
with lifecycles and revision-bound approvals, and an audit trail that records refusals as carefully
as it records changes.

A harness asks the protocol what is owed, what is permitted and whether the work is done. The
protocol answers deterministically, and can always say why. It does no work of its own: it holds no
tools, calls no model, touches no repository, and never observes anything for itself — every answer
is a function of the validated documents plus the evidence submitted to it.

The document tree in the repository is 39 files: 3 protocols, 22 principles, 4 workflows, 5 profiles
and 5 artifact lifecycles, each validated against the protocol vocabulary in CI.

## ESS — what must exist

**ESS** turns a system design into a model a compiler consumes: domains, entities, commands, events,
views, state machines, components, bindings, topology. From one model it derives documentation,
OpenAPI, AsyncAPI and JSON Schema; a conformance suite that has been seen to catch deliberately
wrong implementations; a typed answer to what a revision changes and which results the change
invalidates; and the structural part of an implementation — types, typestate lifecycles, ports, one
transport — with everything it will not guess carried as a named obligation.
[Where this stands](./status/where-this-stands.md) keeps the numbers.

The distinction the model is built around: `CreateInvoice` is a **command**, and
`POST /invoices/commands/create-invoice` is **one way to expose it**. Transports are projections, not
the model. The same specification compiles to a modular monolith or to distributed services without
the domain changing, and it is what makes a generated test a statement about the system rather than
about its HTTP layer.

## Where they meet

They meet at exactly one place, and it is worth being concrete about what that means.

A task can carry an **artifact** of kind `executable-system-specification`. A principle in force can
then require **evidence** of kind `ess_conformance` before that task completes — evidence that must
be `independent: true` and must come from a `conformance-runner`, so the agent's own report that its
implementation matches the specification is not admissible.

```yaml
requires:
  before_completion:
    conditional:
      - when: artifact.executable-system-specification.exists
        require:
          predicates:
            - ess_conformance.passed
            - ess_conformance.scenarios.failed == 0
          evidence:
            - kind: ess_conformance
              independent: true
              verifier: conformance-runner
```

*From `principles/verification/ess-conformance.yaml`.*

Three things follow, and each is a consequence for a person rather than a property of a model:

* Nobody has to read a diff and judge whether it matches the specification. The specification judges
  it, and the protocol refuses to call the task done until something other than the author has run
  that judgement.
* A task with no specification owes nothing here. The condition is checked against the artifact graph
  at evaluation time, so adding a specification to a project turns the rule on without editing the
  rule.
* A conformance run only counts if it was run against **this** revision of the specification. The run
  carries a `spec_digest`, and it has to be the `model_digest` the specification artifact records.

The last one **fails closed**: if the specification artifact records no `model_digest`, no run can be
shown to be current and the requirement can never be satisfied. That is deliberate. Evidence that
cannot demonstrate which revision produced it is not assumed to be fresh.

[The join, in full](./in-practice/the-join.md) shows the whole document and what it refuses.

## What is honest about this today

The rule works, and since ESS wave 4 the runner it asks for exists: a specification generates its
own conformance suite, `protocol ess conform run` executes it, and the evidence record is minted in
the same process that ran the suite — so no caller can author its own verdict. When the runner
arrived, nothing on the protocol side changed, which is what the shape being right was for.

What is still honest to say: the runner reaches only in-process implementations. Holding an external
system to a specification means depending on the conformance crate from that system's own tests —
see [where this stands](./status/where-this-stands.md).

---

**Sources.** `docs/VISION.md` § *What each half is for*; `docs/guide/harness.md` (the engine's seven
calls, and that it observes nothing itself); `docs/guide/specification.md`;
`principles/verification/ess-conformance.yaml`; document counts from `protocols/`, `principles/`,
`workflows/`, `profiles/` and `artifacts/lifecycles/` (3 + 22 + 4 + 5 + 5 = 39 files).
