---
title: A governed task, end to end
sidebar_position: 2
description: The worked passkeys task — what it declares, what resolution derives, the evidence walk, the refusals, and the stale approval.
---

# A governed task, end to end

The repository's worked example: adding passkey authentication under `development.standard`. The
integration tests replay this directory, so what follows is what the engine does, not an
illustration of it. All files are under `examples/development-passkeys/`.

## What the task declares

`task.yaml`, in full apart from its comments:

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

The task names what it is and which profile governs it. The rules in force, the workflow, the
capabilities and the completion condition are all **derived**, so none of them can drift out of step
with the profile. The two `constraints.facts` are declared rather than observed, deliberately:
whether a change touches an interface someone else calls is not something a tool can see, and a
principle reads `change.public_contract` to decide whether it applies at all.

## What resolution derives

Nine principles, ten obligations, twelve capability decisions — each with the document responsible
recorded. The capability half:

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

A harness reads that list and exposes exactly those tools, nothing else. `secret.read` is denied,
and a `deny` cannot be granted back by a later document.

## A refusal, with the rule attached

```text
$ protocol explain --task examples/development-passkeys/task.yaml --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
```

Four lines, each doing work: the rule that decided, what would unlock it, and where in the workflow
the question was asked. Nobody wrote this denial into the task or the profile — `approval-gates` is
in force because the profile includes it, and `aep/1` holds `production.write` in the approval
floor, so a profile granting it outright would have failed to resolve at all.

Refusing is not silence, either: asking is itself an event, and the request and its answer both land
in the audit trail. The audit type rejects a rejection that carries a change record, so the trail
cannot claim a refusal changed something.

## The evidence, in the order it arrives

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

Two fields carry the weight: `producer` says a verifier produced this, not the agent — so it can
satisfy requirements marked `independent: true` — and `provenance.command` says what was run.
Submission order is recorded, so red-before-green is a fact
(`evidence.first_seq.test_result < evidence.first_seq.diff`), and submitting a *passing* run first
stops the walk early: `test-driven` requires `test.first_result == failed` before implementation.

## Where it stops, and why

```text
Task incomplete in `implement`:
  ✗ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
      tests.unit.failed = 1; unobserved: static_analysis.errors; evidence.missing = 7
  ? specification.satisfied                                       [principle spec-driven]
      unobserved: specification.satisfied
```

`✗` is a fact that is wrong; `?` is a fact nobody has observed. They want different responses — fix
the code, or run something — and only `True` permits a transition.

Submit all five evidence files and the task reaches `complete`. Drop the last one — the independent
statement that the change is the one described — and the work stops at adversarial verification with
`evidence.missing = 1`. That is the `provenance-tracking` principle doing its job.

## The stale approval

The manifest holds the design at **version 7**; the recorded human approval was given against
**version 3**. Under `development.standard` that passes, because the profile requires no design
review. Under `development.critical`:

```text
  ✗ review of a design is approved (by a person)   [completion]
      the approved review of design:passkeys-auth was given against a different version
```

Someone approved a design they saw. Version 7 is a different design, and their name is not attached
to it. This is the enforceable form of a rule every team writes down and nobody can enforce by
reading: an approval names the revision it approved, and stops satisfying the requirement when the
artifact moves past it.

---

**Sources.** `examples/development-passkeys/` (`task.yaml`, `evidence/`, and the transcripts in its
`README.md`, replayed by `crates/aep-engine/tests/end_to_end.rs` and
`crates/protocol-cli/tests/cli.rs`); `crates/aep-engine/src/policy.rs` (the rule name);
`crates/aep-domain/src/requirement.rs` (the different-version message);
`crates/aep-domain/src/audit.rs`.
