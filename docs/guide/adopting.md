# Adopting the protocol in your repository

For a team that already has rules — in a wiki, a `CONTRIBUTING.md`, or in one senior engineer's head
— and wants them enforced instead of restated. This is the narrative version of the
[document authoring brief](../plan/document-authoring-brief.md); the brief holds the reference tables
(every capability, evidence kind, fact path, predicate operator) and this page holds the order to do
things in.

Every command below assumes `B=target/debug/protocol` after `cargo build -p protocol-cli`, run from
the root of the tree being checked.

## What you bring

Three things, and only the first two are yours to write.

| You provide | What it is | Smallest useful form |
|---|---|---|
| `task.yaml` | What is being done, and which profile governs it | 5 lines |
| `artifacts.yaml` | Where the specs, designs, ADRs and reviews are — references, not copies | 1 entry |
| a profile name | Which bundle of rules applies | one of `development.fast` / `.standard` / `.critical` |

Everything else — the workflow, the principles in force, what the agent may do, what counts as
finished — is derived from the profile. Restating any of it in the task is how it drifts.

A task that resolves:

```yaml
id: BILL-88
kind: feature
objective: move-invoices-to-partitioned-table
protocol: adp/1
profile: acme.service
manifest: artifacts.yaml
constraints:
  facts:
    # Declared here because nothing can observe it. A principle reads it to decide whether it applies.
    change.database_schema: true
    change.public_contract: false
    change.architectural: false
```

And the manifest it points at:

```yaml
version: aep.artifacts/1

artifacts:
  - id: spec:invoice-partitioning
    kind: specification
    status: approved
    location:
      path: docs/specs/invoice-partitioning.md

  - id: plan:invoice-partitioning
    kind: migration-plan
    status: approved
    location:
      path: docs/migrations/0007-partition-invoices.md
    relations:
      - derived_from: spec:invoice-partitioning
```

The manifest holds no copies. The design stays in `docs/`, the PRD stays in the planning tool, and
neither has to move for the protocol to reason about them.

## Where the documents live

The loader walks six directories under one root, recursively, skipping anything dot-prefixed:
`protocols/`, `principles/`, `workflows/`, `profiles/`, `artifacts/lifecycles/` and `drivers/`. That
root is a **document tree**, and everything below is about which tree your documents are in.

```text
your-repo/
  protocols/                        vendored, unchanged — the declared vocabulary
  workflows/                        vendored, plus any state machine of your own
  principles/
    upstream/                       vendored
    migration-has-a-way-back.yaml   yours
  profiles/
    upstream/                       vendored
    acme-service.yaml               yours
  artifacts/lifecycles/             vendored
  drivers/                          step maps, only if you drive runs with `protocol drive`
```

Vendor by submodule, subtree or copy — the loader cares about content, not provenance. Documents are
indexed by the `id` declared *inside* the file, never by path, so a subdirectory layout of your
choosing costs nothing.

| Lives in this repository | Lives in yours |
|---|---|
| `protocols/` — the vocabulary. Extending it is a protocol change, not a configuration change | your principles |
| `principles/`, `workflows/`, `profiles/` — the defaults, worth reading before replacing | your profiles, usually `extends:` one of theirs |
| `artifacts/lifecycles/` — what statuses each artifact kind may hold | your `task.yaml` and `artifacts.yaml`, per task |

## Two shapes of adoption, and the one thing that decides between them

You can point at somebody else's tree instead of owning one. A project does that with a small file,
`.engineering/project.yaml`, which every command finds by walking up from where it is run:

```yaml
protocol: adp/1
profile: acme.service
protocols: ../engineering-protocols   # the tree, relative to .engineering/
```

(The directory is called `.engineering` by default. If that name is taken, or your team calls it
something else, set `AEP_PROJECT_DIR=.workflow` and discovery looks for that instead — read once per
process, so it cannot change mid-run.)

**Pointing at a tree merges two of the six document kinds, and only two.** Documents in
`.engineering/principles/` and `.engineering/profiles/` are loaded *over* the tree's. Nothing else
is: a workflow, a protocol, a lifecycle or a driver step map placed under `.engineering/` is not
read at all, and the failure surfaces where the workflow is named rather than where the file sits:

```console
$ $B resolve
error: 1 document problem(s):
  - [unknown_workflow] workflow knowledge/curation: no workflow document declares
    `knowledge/curation` (hint: available: adp/default, incident/standard,
    migration/forward-only, release/progressive)
```

The file is right there in `.engineering/workflows/`. It was never read.

| What you want to add | Enough to point at a tree? |
|---|---|
| A principle of your own | **Yes** — `.engineering/principles/` |
| A profile of your own, usually `extends:` one of theirs | **Yes** — `.engineering/profiles/` |
| A state machine your work actually has — different states, different phases | No. Own a tree |
| A lifecycle for an artifact kind of your own | No. Own a tree |
| A protocol declaring facts, capabilities or evidence kinds nobody upstream declared | No. Own a tree |

So: **a new state machine means owning a tree.** Vendor the upstream documents into it, put yours
beside them, and point the project at it. The shortest form of that is `protocols: .`, which makes
the project directory itself the tree:

```yaml
protocol: adp/1
profile: acme.knowledge
protocols: .                # the tree is .engineering/ itself
principles: local/principles
profiles: local/profiles
```

```text
your-repo/.engineering/
  project.yaml
  protocols/      principles/      workflows/      profiles/      artifacts/lifecycles/
  ^ one tree, yours, with the upstream documents vendored into it and yours beside them
```

The last two lines of that `project.yaml` are load-bearing and easy to miss. `principles:` and
`profiles:` default to `principles/` and `profiles/` under `.engineering/` — which, once the tree
*is* `.engineering/`, are directories the tree loader has already read. Every file in them would be
loaded twice and refused as a duplicate id:

```console
$ $B resolve
error: 6 document problem(s):
  - .engineering/profiles/development-standard.yaml: [duplicate_principle] profile
    development.standard: a second profile document declares the id `development.standard`
```

Point the two merge paths at directories the tree does not contain — they need not exist — and the
project-local merge stays out of the way, because with your own tree you no longer need it.

## Choosing a profile

Measured on the [worked example](../../examples/development-passkeys/) task, same artifacts, only the
profile changed:

| | `development.fast` | `development.standard` | `development.critical` |
|---|---:|---:|---:|
| principles in force | 5 | 9 | 15 |
| obligations | 6 | 10 | 17 |
| completion checks | 14 | 24 | 45 |
| distinct evidence kinds owed | 5 | 7 | 7 |

The evidence *kinds* barely grow between standard and critical — what grows is the number of runs and
who has to sign. Read the cost as what a person has to do that they were not doing before:

| Profile | Choose it when | What it costs you |
|---|---|---|
| `development.fast` | Blast radius contained, contract surface private: internal tooling, scripts, a spike | A spec, a failing test first, static analysis, provenance. It cannot request a review or an approval, so a human is never in the loop |
| `development.standard` | Anything with an external consumer, persisted data, or a customer-visible path. The default | Adds contract tests and a property suite, and the ability — with it, the obligation — to ask a human |
| `development.critical` | A silent defect is worse than a late delivery: auth, money, migrations, crypto | Adds a mutation run, a differential run against the implementation being replaced, an invariant check, an adversarial verification, contracts on the code, an approved design related to the specification, a **fresh human review of that design**, and — where the project has an executable system specification — conformance to it |

The last line of `development.critical` is the one that bites. Under it, an approval of version 3 of a
design stops satisfying the review requirement once the design reaches version 7:

```console
$ $B evaluate --task /path/to/critical-task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/04-review.yaml | grep -A 1 review
  ✗ review of a design is approved (by a person)                  [completion]
      the approved review of design:passkeys-auth was given against a different version
```

Someone approved a design they saw. Version 7 is a different design and their name is not on it.

## Writing a principle of your own

A real rule: **a change that rewrites persisted data needs an approved migration plan before the code
is written, and something other than the agent has to have run the recovery path before it is
finished.**

Four decisions, in this order.

| Decision | Question it answers | In the document |
|---|---|---|
| Applicability | When is this rule even about this task? | `applies_when:` |
| Timing | At which point does it bite? | `before_<phase>:` keys under `requires:` |
| Obligation | What must be true or exist? | `predicates:`, `artifacts:` |
| Evidence | Who is allowed to say so? | `evidence:` with `independent: true` |

`principles/migration-has-a-way-back.yaml`:

```yaml
id: migration-has-a-way-back
version: 1
title: A schema migration has a way back
summary: >-
  A change that rewrites persisted data is not finished until something other than the agent has run
  the down path. Without it the recovery plan gets written during the incident, by whoever is awake.
applies_when:
  # A fact nothing can observe, so the task declares it. The alternative — applying the rule to every
  # task — gets it removed within a month.
  change.database_schema: true
requires:
  before_implementation:
    artifacts:
      - kind: migration-plan
        status: approved
  before_completion:
    predicates:
      - verification.recovery.passed
    evidence:
      # The agent's own report of a successful rollback rehearsal does not satisfy this.
      - kind: verification
        independent: true
```

And a profile that puts it in force:

```yaml
id: acme.service
version: 1
title: Acme service development
summary: Standard development, plus Acme's migration rule.
protocol: adp/1
extends: development.standard
principles:
  - migration-has-a-way-back
```

`applies_when` is doing real work. Two tasks, same profile:

```console
$ $B resolve --root . --task task.yaml | grep -E '^(task|principles|obligations)'
task        BILL-88 (feature)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking, contract-testing, property-based-testing, approval-gates, reversible-changes, migration-has-a-way-back
obligations 12
$ $B resolve --root . --task task-rename-a-button.yaml | grep -E '^(task|principles|obligations)'
task        BILL-89 (feature)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking, contract-testing, property-based-testing, approval-gates, reversible-changes
obligations 10
```

The rule is absent from the second task rather than present and vacuously satisfied, so nobody reads
a green report and wonders which of the ticks meant anything.

And the timing is not decorative. With the migration plan in the manifest, a failing test carries
BILL-88 into implementation. Delete the plan from the manifest and the same evidence stops one state
short:

```console
$ $B evaluate --root . --task task.yaml --artifacts artifacts-without-the-plan.yaml \
    --evidence red-test.yaml --advance | grep -E '^(state|transitions)|migration-plan'
state       establish_verifiers (Establish verifiers)
transitions
      ? artifact migration-plan (approved) — no migration-plan artifact is declared [principle migration-has-a-way-back]
```

The block names the principle. Somebody can go and read it and either write the plan or argue with
the rule — both better outcomes than an agent quietly writing the migration.

## A rule with a clock on it

A rehearsed rollback from six months ago is not a rehearsed rollback. Put a `horizon` on the
evidence requirement and the engine stops taking the old record's word for it:

```yaml
    evidence:
      - kind: verification
        independent: true
        horizon: 90          # whole days
```

Every evidence record already carries `observed_at` — the day somebody looked, written by whoever
submitted it, never stamped by the engine, and refused outright if it is in the future. A horizon is
what that date is measured against:

```console
$ $B evaluate --root . --task task.yaml --artifacts artifacts.yaml --evidence recovery-rehearsal.yaml
  ? evidence verification (independent) within 90d                [principle migration-has-a-way-back]
      the last observation was on 2023-11-13, the horizon is 90d, and it lapsed on 2024-02-11
```

**`?`, not `✗`, and the distinction is the whole point.** An expired observation is not a wrong
answer, it is an old one, so the task is blocked with *nobody knows* rather than accused of a
failure that never happened. The lapsed record's facts are withheld too, so a guard reading
`tests.unit.failed == 0` off a stale suite refuses rather than passing on it. `evidence.lapsed` is
the count of records in that state, and it sits beside `evidence.missing` because *nobody produced
it* and *somebody did and nobody has looked since* are different problems with different owners.

Two things a horizon cannot do, worth knowing before you set one:

* **It is a volatility guess, not a guarantee.** A seven-day claim can be false on day five and the
  gate will say it is fine. What follows is that *shortening* a horizon has to stay cheap and normal
  — if the only way to say "this moves faster than I thought" is prose, nobody says it.
* **There is no way to extend one.** The horizon lives on the requirement, in a reviewed document,
  and no command mutates it. The only refresh is to observe again and write a new date, which is
  deliberate: if extending were as easy to call as re-checking, it is the one that would get called
  by whoever is trying to get a gate green.

`protocol evidence inspect <file>` reads the dates back out of a record without submitting anything,
and `protocol evidence scan <dir>` does the same for claims written into markdown by hand — with a
coverage line comparing annotation-shaped occurrences against records the parser actually produced,
because a scanner over human-written documents that quietly stops seeing half of them reports green
either way. [`examples/evidence-horizons-corpus/`](../../examples/evidence-horizons-corpus/) is the
regression corpus behind that, contributed by an adopter who had been keeping such claims by hand.

## The failure worth learning first

A predicate may only read facts the protocol declares observable. Suppose the rule had reached for
`migration.rollback_tested`, which reads perfectly well in English:

```console
$ $B validate --root .
46 file(s): 3 protocol(s), 23 principle(s), 4 workflow(s), 7 profile(s), 8 lifecycle(s), 1 step map(s)
1 problem(s):
  - [unobservable_fact] principle migration-has-a-way-back.obligations.migration-has-a-way-back/before-completion: `migration.rollback_tested` is not declared observable by protocol adp/1 (hint: declared families: ess_conformance.**, trace_conformance.**, mutation.**, differential.**, invariant.**, clean_room.**, build.**, types.**, task.**, change.**, risk, severity, state.**, workflow.**, principle.**, evidence.**, required_evidence.**, tests.**, test.**, unit_tests.**, contract_tests.**, regression_suite.**, static_analysis.**, contracts.**, property_test.**, coverage.**, specification.**, diff.**, source_diff.**, artifact.**, review.**, verification.**, approval.**, approvals.**, deployment.**, metric.**, service.**)
$ echo $?
1
```

`migration.**` exists — but only under `aop/1`, and this principle is in force under `adp/1`. The
error lists every family that *is* available, which is usually enough to find the right spelling
(`verification.recovery.passed`, here).

There is a second, quieter version of the same mistake. A fact in a declared family but a spelling
nothing projects passes validation and then never becomes true:

```console
$ $B validate --root .
46 file(s): 3 protocol(s), 23 principle(s), 4 workflow(s), 7 profile(s), 8 lifecycle(s), 1 step map(s)
valid
$ $B evaluate --root . --task task.yaml | grep passsed
  ? verification.recovery.passsed                                 [principle migration-has-a-way-back]
      unobserved: verification.recovery.passsed
```

A `?` that never becomes `✓` is a task nobody can finish, and it looks like a stuck agent rather than
a typo. Section 2 of the [authoring brief](../plan/document-authoring-brief.md#2-facts-the-engine-actually-projects)
lists the spellings the engine actually projects — check a new predicate against it before writing the
rest of the rule.

## Keeping the documents honest

Two commands, and they catch different things. `validate` reads the tree on its own; `resolve` also
needs a task, because some checks only mean anything once a profile and a workflow are paired.

| Caught by | Refusal | Because otherwise |
|---|---|---|
| `validate` | a predicate reading a fact the protocol does not declare | the rule can never be satisfied, and nobody finds out until a task hangs |
| `validate` | a workflow state that cannot be reached | `[unreachable_state]` — the state is decoration |
| `validate` | a non-terminal state with no way out | `[dead_end_state]` — execution wedges there |
| `validate` | a rollback policy with no precondition | `[incomplete_rollback_policy]` — "rolled back" describes a wish |
| `resolve` | an obligation timed against a phase no state declares | `[unknown_phase]` — the rule is not strict, it is absent |
| `resolve` | a task needing a capability the resolved policy denies | `[capability_conflict]` — the agent would find out mid-task |

Both exit 1, and both accumulate: a document with four broken references reports four errors, not the
first one. Add them to CI as their own step, so a broken rule reads as a broken rule rather than as
one failed assertion inside a test log:

```yaml
- name: Documents
  run: |
    cargo run -p protocol-cli -- validate --root .
    cargo run -p protocol-cli -- resolve --root . --task examples/typical-task.yaml
```

Keep at least one representative task per profile you ship. `validate` alone will not tell you that a
principle is timed against a phase your workflow does not have.

This repository does the same thing from the other side: `crates/aep-engine/tests/documents.rs` loads
the tree, asserts it has no failures, and resolves a task against every profile — so a principle that
could never fire cannot be committed. Its own gate is `task check`, ten steps, and a step whose
toolchain is missing fails and names it rather than skipping.

## Next

* [`harness.md`](harness.md) — wiring an agent to the engine so these rules actually govern it.
* [`backend.md`](backend.md) — storing the entities the manifest points at.
