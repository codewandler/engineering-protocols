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

`protocol validate` loads six kinds from a tree and says how many of each it found. Each kind has
one directory, and a document outside it is not loaded at all — so a misfiled file goes missing
rather than half-applying:

| Kind | Directory |
|---|---|
| protocol | `protocols/` |
| principle | `principles/` |
| workflow | `workflows/` |
| profile | `profiles/` |
| artifact lifecycle | `artifacts/lifecycles/` |
| step map | `drivers/` |

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

* `requires:` is keyed by `always`, `before_<phase>` or `during_<phase>`. The keys use `_` for `-`
  in phase names (`before_verification_setup` → phase `verification-setup`), and every named phase
  must exist in the workflow the profile uses.
* Requirements with no stated timing default to **before completion**.
* A principle must enforce something — obligations, evidence, verification or a capability policy.
* A principle's `capabilities:` may only deny or require approval, never allow.
* `applicability` and `failure_policy` are accepted spellings of `applies_when` and `on_failure`,
  and the generated schema publishes both, so an editor pointed at it accepts either. Writing both
  is a duplicate field and is refused.

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
      horizon: 7d                    # optional; without it a record never decays
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

`horizon:` is the one field that makes a satisfied requirement stop being satisfied. It is written
as a number of days — `7d`, `7D`, or a bare `7` — and it says how long an observation counts for.
Past it, the requirement reads **Unknown** rather than False, because nobody has established that
the thing is broken; they have established that nobody has looked lately:

```text
? evidence test_result from test-runner (independent) within 7d  [principle test-driven]
    the last observation was on 2023-11-13, the horizon is 7d, and it lapsed on 2023-11-20
```

Unknown blocks a transition exactly as False does, so the practical effect is that a green test run
from three weeks ago no longer lets the work complete. Two rules follow from the shape: an age
exactly equal to the horizon is still covered, and `at_least: 0` beside a horizon is refused —
a decay rule over a set nobody consults is a gate that cannot fire. The horizon lives on the
*requirement*, in a reviewed document, never on the record: a record that carried its own expiry
would let whoever wrote it choose how long it counted for.

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

Lifecycle documents live in `artifacts/lifecycles/`, one per kind, beside `artifacts/kinds/`,
`artifacts/relations/` and `artifacts/templates/`.

```yaml
kind: architecture-decision-record
initial: proposed
transitions:
  proposed: [accepted, rejected]
  accepted: [superseded]
  rejected: []                     # terminal: a refused decision is still part of the record
  superseded: []
```

An artifact whose status is not in its kind's lifecycle is a validation error. A `superseded`
artifact must have a successor declaring `supersedes:` it.

A lifecycle document that names **no** `kind:` is the tree's fallback — the lifecycle every kind
with no nearer one is held to, and the only way to bind kinds nobody has enumerated. One tree may
declare at most one. A lifecycle registered for the kind itself, or for a kind it specialises,
always wins over it.

## Evidence records

The document `protocol evaluate --evidence` submits, and `protocol evidence inspect` reads: a list
of records, each naming its `kind`, the fields that kind declares, and who produced it.

```yaml
- kind: test_result
  observed_at: 2023-11-13          # required: when somebody looked
  suite: unit
  passed: 61
  failed: 0
  producer:
    producer: verifier             # or `agent`; `independent: true` needs this one
    verifier: test-runner
  about: task:AUTH-142             # optional: what it is about
  provenance:
    command: cargo test -p auth
```

`observed_at` is required on every record and has no default. It is the caller's, because the caller
is the only party that knows: a suite run three weeks ago and submitted this morning is three weeks
old, and an engine that inferred the time from submission would have recorded it as fresh. Write it
as a calendar date (`2026-08-30`, midnight UTC) or as epoch milliseconds (`1788134400000`); the
engine emits the second and accepts both.

A record whose `observed_at` is in the *future* is refused rather than stored. A check that was
scheduled and never performed would otherwise read as the freshest record there is, and the model
could no longer answer *has anyone ever looked at this?*. Horizons read `observed_at` and nothing
else — never the submission time.

## Step map

A workflow says what states exist and what evidence a transition needs; it deliberately does not say
how to obtain that evidence, which is what lets one workflow govern a Rust repository and a Terraform
one. A step map (`aep.driver-steps/1`) is the missing half, and it is what `protocol drive` reads.

```yaml
format: aep.driver-steps/1
id: development/default
workflow: adp/default/1          # pinned to a major version, and the pin is mandatory
title: Driving adp/default in this repository
states:
  verify:
    visit_budget: 3              # how many times this state may be entered before the run stops
    steps:
      - kind: command            # run a program, and map its result to evidence
        description: Run the unit suite.
        run: [cargo, test, --workspace]
        retries: 1
        evidence:
          kind: test_result
          verifier: test-runner
          suite: unit
      - kind: llm                # ask a model, with a tool set the protocol derived
        description: Explain what failed.
        skills: [planning]
        harness: claude-code     # `claude-code` when the document is silent
        prompt: >-
          …
      - kind: operator           # stop and hand the run to a person
        prompt: Approve the production change.
```

The workflow pin is mandatory because a step map names states and orders steps inside them: an
unpinned one is an instruction sheet for whatever happens to be in the tree. When `adp/default`
reaches version 2 this map is orphaned at load — refused, naming both versions — rather than quietly
applied to a state graph it was not written against.

Only a `command` step may carry `evidence:`. An `llm` step cannot be given one, so anything a model
is supposed to have achieved that is *checkable* is observed by the command step after it — which is
what keeps `independent: true` honestly satisfiable.

Without `record:`, the driver mints the record from the program's exit status, so the kind has to be
one an exit status can carry: `test_result`, `static_analysis`, `contract_result` or `diff`
(`EvidenceMapping::MINTABLE`, `crates/aep-driver-spec/src/map.rs:531-536`). `record: <path>` says
the program writes the record itself, and then the driver reads that document and submits what it
says rather than minting anything. That is what makes `trace_conformance` reachable from a map at
all: its record carries a specification digest, a transcript digest and three counts, and an exit
status carries none of them.

Two placeholders are expanded in a `command` step's `run` words and in its `record:` path:
`{run_directory}`, and `{transcript}`, the transcript of the `llm` step this one follows in the
same state, at the attempt that ran. The list is closed, so a misspelling is refused at load rather
than handed to a program as literal braces, and so is a `{transcript}` in a state with no `llm` step
before it. `{}` and `{a: .b}` match nothing and stay ordinary text, because `find -exec` and `jq`
write them.

## Task and artifact manifest

See [Govern a task](../guides/govern-a-task.md) for complete examples. A task requires `id`,
`objective`, `protocol` and `profile`. It may also name `kind`, an optional `manifest:` path,
`derived_from:` and `context:` for the artifacts it came from and is constrained by,
`principle_overrides:` for principles it adds or drops, and `constraints.facts` for context facts
nothing can observe. A manifest (`version: aep.artifacts/1`) lists artifacts with `id`
(`<namespace>:<name>`), `kind`, `status`, `location`, and `relations`.

`artifact_manifest`, `principles` and `type` are accepted spellings of `manifest`,
`principle_overrides` and `kind`. Give one or the other, never both — a document declaring a field
twice under two names is refused rather than resolved by precedence.

## Identifier rules

| Kind | Shape | Example |
|---|---|---|
| principle, phase, approval, claim | lower-case kebab | `test-driven` |
| profile, workflow | kebab segments joined by `.` or `/`, last segment not a number | `development.standard`, `adp/default` |
| state | kebab or snake segments | `adversarial_verify` |
| artifact id | `<namespace>:<name>` | `design:passkeys-auth` |
