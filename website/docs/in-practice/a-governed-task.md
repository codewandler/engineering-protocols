---
title: A task the protocol governs
sidebar_position: 3
description: One task, five pieces of evidence, and the two points where the protocol refuses to call it done.
---

# A task the protocol governs

A worked example from the repository: adding passkey authentication, under the standard development
profile. The integration tests replay this directory, so what follows is what the engine does rather
than an illustration of it.

## What the task says

`examples/development-passkeys/task.yaml`, in full apart from its comments:

```yaml
id: AUTH-142
kind: feature
objective: add-passkey-support

protocol: adp/1
profile: development.standard

derived_from:
  - story:AUTH-141
context:
  product_requirements:
    - prd:passkeys

manifest: examples/development-passkeys/artifacts.yaml

constraints:
  facts:
    change.public_contract: true
    change.architectural: false
  notes:
    - Existing password authentication must keep working for the whole rollout.
```

The task names two things: what it is, and which profile governs it. The rules in force, the
workflow, what the agent may do and what counts as finished are **derived** — none of them is
restated here, where it could drift out of step with the profile.

Two facts are declared rather than observed, and that is deliberate: whether a change touches an
interface someone else calls is not something a tool can see. A principle reads
`change.public_contract` to decide whether it applies at all.

## What it resolves to

Resolution produces the plan: nine principles, ten obligations and twelve capability decisions, each
with the document responsible for it recorded. The capability half of that output:

```text
capabilities
  allowed            approval.request
  allowed            artifact.read
  allowed            artifact.write
  requires_approval  deployment.create
  requires_approval  deployment.create:production
  requires_approval  network.write
  requires_approval  production.write
  allowed            repository.read
  allowed            repository.write
  allowed            review.request
  denied             secret.read
  allowed            tests.execute
```

A harness reads that list and exposes exactly those tools, and nothing else. `secret.read` is denied,
and a `deny` cannot be granted back by a later document. Asking anyway produces
[a refusal that names the rule](./refusals.md).

## The evidence, and the order it arrives in

The first submission is a **failing** test, before any code exists:

```yaml
- kind: test_result
  suite: unit
  passed: 0
  failed: 1
  producer:
    producer: verifier
    verifier: test-runner
  about: task:AUTH-142
  provenance:
    command: cargo test -p auth passkey_credential_is_scoped_to_one_user
```

Two fields carry the weight. `producer` says a **verifier** produced this, not the agent — an
evidence requirement marked `independent: true` is not satisfied by the agent's own report.
`provenance.command` says what was run.

Submission order is recorded, so red-before-green is a fact rather than an instruction:
`evidence.first_seq.test_result < evidence.first_seq.diff`. Submit a *passing* run instead and the
same walk stops early, because `test-driven` requires `test.first_result == failed` before the
implementation phase.

## Where it stops, and why

An evaluation is a checklist, and the three marks mean different things:

```text
Task incomplete in `implement`:
  ✗ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
      tests.unit.failed = 1; unobserved: static_analysis.errors; evidence.missing = 7
  ? specification.satisfied                                       [principle spec-driven]
      unobserved: specification.satisfied
```

`✗` is a fact that is wrong. `?` is a fact nobody has observed. They want different responses — fix
the code, or go and run something — and only `true` permits a transition.

Submit all five evidence files and the same command reaches `complete`. Drop the last one — the
independent statement that the change is the one described — and the work stops at adversarial
verification with `evidence.missing = 1`. That is `provenance-tracking` doing its job, not a bug.

## The stale approval

The manifest holds the design at **version 7**. The recorded human approval was given against
**version 3**. Under `development.standard` that is enough, because the profile requires no design
review. Under `development.critical` it is not:

```text
  ✗ review of a design is approved (by a person)   [completion]
      the approved review of design:passkeys-auth was given against a different version
```

Ada approved a design she saw. Version 7 is a different design, and her name is not attached to it.

This is the mechanism behind a rule everyone writes down and nobody can enforce by reading: an
approval names the revision it approved, and stops satisfying the requirement when the artifact moves
past it.

## Choosing how strict to be

Three profiles ship. The cost of each is best read as what a person has to do that they were not
doing before.

| Profile | Choose it when | What it costs |
|---|---|---|
| `development.fast` | blast radius contained, contract surface private: internal tooling, scripts, a spike | a spec, a failing test first, static analysis, provenance. It cannot request a review or an approval, so a human is never in the loop |
| `development.standard` | anything with an external consumer, persisted data, or a customer-visible path. The default | adds contract tests and a property suite, and the ability — with it, the obligation — to ask a human |
| `development.critical` | a silent defect is worse than a late delivery: auth, money, migrations, crypto | adds a mutation run, a differential run against the implementation being replaced, an invariant check, an approved design related to the specification, and a fresh human review of that design |

---

**Sources.** `examples/development-passkeys/` — `task.yaml`, `evidence/01-red-test.yaml`,
`evidence/05-provenance.yaml` and the worked transcripts in its `README.md`, replayed by
`crates/aep-engine/tests/end_to_end.rs` and `crates/protocol-cli/tests/cli.rs`;
`docs/guide/adopting.md` § *Choosing a profile*; `crates/aep-domain/src/requirement.rs` (the
different-version message).
