# Worked example: passkey authentication under `development.standard`

A complete input set for one task — the documents, the artifact graph, and the evidence a harness
would submit — plus the exact commands to run against it. The integration tests replay this directory,
so it cannot drift from what the engine actually does.

```console
$ cargo build -p protocol-cli
$ B=target/debug/protocol
```

## What the task is held to

```console
$ $B resolve --task examples/development-passkeys/task.yaml
task        AUTH-142 (feature)
objective   add-passkey-support
protocol    adp/1
profile     development.standard
workflow    adp/default (initial: receive)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking,
            contract-testing, property-based-testing, approval-gates, reversible-changes
obligations 10
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

The task names two things: what it is, and which profile governs it. The nine principles, the
workflow and the twelve capability decisions are all derived — none of them is restated in the task
where it could drift.

## Why an agent cannot change production

```console
$ $B explain --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
$ echo $?
1
```

The refusal names a principle a person can go and read, and says exactly what would unlock it. Note
that no document had to remember to deny this: `aep/1` puts `production.write` in its **approval
floor**, so a profile that granted it outright would fail to resolve at all.

## Walking the task with evidence

```console
$ $B evaluate --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/01-red-test.yaml \
    --advance
state       implement (Implement)
```

One failing test and an approved specification are enough to reach implementation — and *only* a
failing one. Replace `01-red-test.yaml` with a passing run and the same command stops at
`establish_verifiers`, because `test-driven` requires `test.first_result == failed` before the
implementation phase. Red-before-green is checked here as an ordering fact
(`evidence.first_seq.test_result < evidence.first_seq.diff`), not asserted in a prompt.

Submitting the whole sequence walks the task to completion:

```console
$ $B evaluate --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/01-red-test.yaml \
    --evidence examples/development-passkeys/evidence/02-implementation.yaml \
    --evidence examples/development-passkeys/evidence/03-verification.yaml \
    --evidence examples/development-passkeys/evidence/04-review.yaml \
    --evidence examples/development-passkeys/evidence/05-provenance.yaml \
    --advance
state       complete (Complete)
Task complete in `complete`:
  ✓ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
  ...
```

Drop `05-provenance.yaml` and the work stops at `adversarial_verify` with
`evidence.missing = 1`: `provenance-tracking` wants a statement from something other than the agent
that the change is the one described. That is the principle doing its job, not a bug.

## The stale approval

`artifacts.yaml` has the design at **version 7**. `04-review.yaml` records a human approval of
**version 3**. Under `development.standard` that is enough — the profile requires no design review —
but under `development.critical` it is not:

```console
$ sed 's/development.standard/development.critical/' examples/development-passkeys/task.yaml > /tmp/critical.yaml
$ $B evaluate --task /tmp/critical.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/04-review.yaml | grep review
  ✗ review of a design is approved (by a person)   [completion]
      the approved review of design:passkeys-auth was given against a different version
```

Ada approved a design she saw. Version 7 is a different design, and her name is not attached to it.

## Files

| File | What it is |
|---|---|
| `task.yaml` | the task: what it is, which profile governs it, what it was derived from |
| `artifacts.yaml` | the artifact graph: PRD, story, specification, design, ADR, review |
| `evidence/01-red-test.yaml` | the failing test, submitted before any code |
| `evidence/02-implementation.yaml` | the change, produced by an agent and recorded as such |
| `evidence/03-verification.yaml` | what the verifiers found: unit, regression, contracts, static analysis, a property, the specification |
| `evidence/04-review.yaml` | a human approval — of version 3, deliberately |
| `evidence/05-provenance.yaml` | the independent verification `provenance-tracking` requires |
