---
title: AEP document reference
sidebar_position: 2
description: The syntax of principle, workflow, profile and lifecycle documents, requirement sets, and the identifier rules the validator holds you to.
---

# AEP document reference

The authoritative version of this reference lives in the repository as
[`docs/plan/document-authoring-brief.md`](https://github.com/codewandler/engineering-protocols/blob/main/docs/plan/document-authoring-brief.md);
the declared vocabulary (capabilities, evidence kinds, fact families) is in the
[vocabulary reference](./vocabulary.md). Documents are YAML, validated against generated JSON
Schemas plus cross-document checks, and indexed by the `id` declared inside the file — never by
path.

## Principle

```yaml
id: test-driven                  # lower-case kebab-case
version: 1
title: Test-driven development
summary: >-
  One or two lines: what this enforces, and what goes wrong without it.
applies_when:                    # omitted means always
  task.kind: {any_of: [feature, bugfix]}
requires:                        # phase-keyed form
  before_implementation:
    - test.exists
  before_completion:
    - tests.unit.failed == 0
  always:
    - ...
evidence:                        # must exist by completion
  - kind: test_result
    independent: true
verification:                    # verifiers that must have spoken
  - verifier: test-runner
  - verifier: human-review
    subject_kind: design
    before: {phase: implementation}
capabilities:                    # a principle may only take away
  deny: [secret.read]
  require_approval: [production.write]
on_failure: block                # block | abort | {action: retry, max_attempts: 2, then: block}
                                 # | {action: escalate, to: oncall}
                                 # | {action: rollback, rollback: {require: [<predicate>]}}
```

The alternative `requires` form, for a rule about a specific state:

```yaml
requires:
  before: {state: implement}     # or {phase: implementation}
  artifacts:
    - kind: specification
      status: approved
```

Rules the validator enforces:

* `before_<phase>` keys use `_` for `-` in phase names (`before_verification_setup` → phase
  `verification-setup`), and every named phase must exist in the workflow the profile uses.
* Requirements with no stated timing default to **before completion**.
* A principle must enforce something — obligations, evidence, verification or a capability policy.
* A principle's `capabilities:` may only deny or require approval, never allow.

## Requirement sets

Wherever `requires:` or `completion:` appears:

```yaml
requires:
  predicates:
    - tests.unit.failed == 0
  evidence:
    - test_result                    # shorthand
    - kind: test_result              # or the full form
      at_least: 1
      independent: true              # an agent's own assertion does not satisfy it
      verifier: test-runner
  artifacts:
    - kind: design
      status: approved
      fresh: true                    # default; excludes superseded/rejected
      relation: {kind: designs, target_kind: specification}
  reviews:
    - subject_kind: design
      result: approved               # approved | changes_requested | rejected
      human: true
      fresh: true                    # must cover the artifact's current version
  approvals:
    - security-review
  conditional:
    - when: {change.architectural: true}
      require:
        artifacts:
          - kind: architecture-design
            status: approved
```

A bare list under `requires:` is read as predicates. An unrecognised mapping key is read as a fact
predicate — so `requires: {change.architectural: true}` works, and a misspelt key becomes an
`unobservable_fact` error instead of being ignored.

## Workflow

```yaml
id: adp/default                  # namespaced with `/`; last segment must not be a number
version: 1
title: Standard development workflow
initial: receive
states:
  receive:
    title: Receive
    phases: [intake]
    requires: { ... }            # what must hold to enter this state
    capabilities: { ... }        # adjustments while here
    irreversible: false
    on_failure: block
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: verify
    to: review
    when:
      all:
        - tests.unit.failed == 0
        - static_analysis.errors == 0
    requires: { ... }
    on_failure: { ... }
allow_unreachable_states: false
```

Enforced rules: the initial state must exist; every `from`/`to` must exist; every non-terminal state
needs an outgoing transition; every state must be reachable unless `allow_unreachable_states: true`;
at most one transition per `from`/`to` pair (combine guards with `any`); an `irreversible: true`
state must not have a rollback failure policy; a rollback policy must state its precondition; and a
workflow whose states declare no `completion` phase fails resolution, because obligations default to
being owed before completion.

## Profile

```yaml
id: development.standard         # dotted kebab-case
version: 1
title: Standard development
summary: >-
  When to choose this over its neighbours.
protocol: adp/1                  # development profiles use adp/1, operations aop/1
extends: development.fast        # optional; inherits workflow, principles, capabilities, completion
workflow: adp/default            # required unless inherited
principles: [spec-driven, test-driven]
without_principles: [mutation-testing]   # drop something inherited
capabilities:
  allow: [repository.read, repository.write, tests.execute]
  require_approval: [production.write]
  deny: [secret.read]
completion:
  all:
    - specification.satisfied
    - tests.unit.failed == 0
    - evidence.missing == 0
facts:                           # profile-level context facts
  risk: medium
```

Extending can only make completion harder: conditions are conjoined, a principle may be added or
dropped, and a denial cannot be granted back. The protocol's approval floor applies on top:
`production.write` and `deployment.create:production` may never appear in `allow`.

## Artifact lifecycle

```yaml
kind: architecture-decision-record
initial: proposed
transitions:
  proposed: [accepted, rejected]
  accepted: [superseded]
  rejected: []
```

An artifact whose status is not in its kind's lifecycle is a validation error. A `superseded`
artifact must have a successor declaring `supersedes:` it.

## Task and artifact manifest

See [Govern a task](../guides/govern-a-task.md) for complete examples. A task names an `id`, `kind`,
`objective`, `protocol`, `profile`, an optional `manifest:` path, and `constraints.facts` for
context facts nothing can observe. A manifest (`version: aep.artifacts/1`) lists artifacts with
`id` (`<namespace>:<name>`), `kind`, `status`, `location`, and `relations`.

## Identifier rules

| Kind | Shape | Example |
|---|---|---|
| principle, phase, approval, claim | lower-case kebab | `test-driven` |
| profile, workflow | kebab segments joined by `.` or `/`, last segment not a number | `development.standard`, `adp/default` |
| state | kebab or snake segments | `adversarial_verify` |
| artifact id | `<namespace>:<name>` | `design:passkeys-auth` |
