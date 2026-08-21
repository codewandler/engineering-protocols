---
title: Govern a task
sidebar_position: 1
description: Put a task of your own under a profile — the task file, the artifact manifest, choosing a profile, and keeping the documents honest in CI.
---

# Govern a task

This guide takes a team that already has rules — in a wiki, a `CONTRIBUTING.md`, or one senior
engineer's head — and puts a real task under the protocol. Commands assume
`B=target/debug/protocol` after `cargo build -p protocol-cli`.

## What you write

Three things, and only the first two are files of yours:

| You provide | What it is | Smallest useful form |
|---|---|---|
| `task.yaml` | what is being done, and which profile governs it | 5 lines |
| `artifacts.yaml` | where the specs, designs, ADRs and reviews live — references, not copies | 1 entry |
| a profile name | which bundle of rules applies | `development.fast` / `.standard` / `.critical`, or your own |

Everything else — workflow, principles in force, capabilities, completion condition — is derived
from the profile. Restating any of it in the task is how it drifts.

A task that resolves:

```yaml
id: BILL-88
kind: feature
objective: move-invoices-to-partitioned-table
protocol: adp/1
profile: development.standard
manifest: artifacts.yaml
constraints:
  facts:
    # Declared here because nothing can observe it. A principle reads it to
    # decide whether it applies.
    change.database_schema: true
    change.public_contract: false
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

The manifest holds no copies. The design stays in `docs/`, the PRD stays in the planning tool;
neither moves for the protocol to reason about them.

## Where the documents live

The loader walks five directories under one root, recursively, skipping anything dot-prefixed.
Vendor the upstream documents (submodule, subtree or copy — the loader cares about content, not
provenance) and put yours beside them:

```text
your-repo/
  protocols/                        vendored, unchanged — the declared vocabulary
  workflows/                        vendored, unchanged — unless you write your own state machine
  principles/
    upstream/                       vendored
    migration-has-a-way-back.yaml   yours
  profiles/
    upstream/                       vendored
    acme-service.yaml               yours — usually `extends:` an upstream profile
  artifacts/lifecycles/             vendored
```

Documents are indexed by the `id` declared inside each file, never by path, so the subdirectory
layout is yours to choose.

## Choosing a profile

Measured on the repository's worked example — same task, same artifacts, only the profile changed:

| | `development.fast` | `development.standard` | `development.critical` |
|---|---:|---:|---:|
| principles in force | 5 | 9 | 14 |
| obligations | 6 | 10 | 16 |
| completion checks | 14 | 24 | 43 |
| distinct evidence kinds owed | 5 | 7 | 7 |

The evidence *kinds* barely grow between standard and critical — what grows is the number of runs
and who has to sign.

| Profile | Choose it when | What it costs |
|---|---|---|
| `development.fast` | blast radius contained, contract surface private: internal tooling, scripts, a spike | a spec, a failing test first, static analysis, provenance. It cannot request a review or an approval — a human is never in the loop |
| `development.standard` | anything with an external consumer, persisted data, or a customer-visible path — the default | adds contract tests and a property suite, and the ability (with it, the obligation) to ask a human |
| `development.critical` | a silent defect is worse than a late delivery: auth, money, migrations, crypto | adds a mutation run, a differential run against the implementation being replaced, an invariant check, an approved design, and a **fresh human review of that design** |

The last row's freshness requirement is the one that bites: an approval given against version 3 of a
design stops satisfying the requirement once the design reaches version 7, and the evaluation output
says so in those words.

## Walking a task on its evidence

The worked example ships five evidence files. Submitting them in order and asking the engine to
advance shows the whole lifecycle; here is the first step
(see [Getting started](../getting-started.md) for the output):

```console
$ $B evaluate --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/01-red-test.yaml \
    --advance
```

Each evaluation lists what is owed, which document asked for it, and — for a blocked transition —
the guard that blocked it. Submit all five files and the same command reaches `complete`.

## Keeping the documents honest in CI

`validate` reads the tree on its own; `resolve` also needs a task, because some checks only mean
anything once a profile and a workflow are paired:

| Caught by | Refusal | Otherwise |
|---|---|---|
| `validate` | `unobservable_fact` — a predicate reading a fact the protocol does not declare | the rule can never be satisfied; a task hangs |
| `validate` | `unreachable_state` | the state is decoration |
| `validate` | `dead_end_state` — a non-terminal state with no way out | execution wedges there |
| `validate` | `incomplete_rollback_policy` | "rolled back" describes a wish |
| `resolve` | `unknown_phase` — an obligation timed against a phase no state declares | the rule is not strict, it is absent |
| `resolve` | `capability_conflict` — a task needing a capability the policy denies | the agent finds out mid-task |

Both exit 1, and both accumulate all problems in one run. Add them to CI as their own step, and keep
at least one representative task per profile you ship — `validate` alone cannot tell you that a
principle is timed against a phase your workflow does not have:

```yaml
- name: Documents
  run: |
    cargo run -p protocol-cli -- validate --root .
    cargo run -p protocol-cli -- resolve --root . --task examples/typical-task.yaml
```

## Next

* [Write a principle](./write-a-principle.md) — encode a rule of your own and put it in force.
* [Integrate an agent harness](./integrate-a-harness.md) — make these answers govern a real agent.
