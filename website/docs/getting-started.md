---
title: Getting started
sidebar_position: 2
description: Build the CLI, validate the document tree, resolve a task, and watch the protocol refuse an action — in about ten minutes.
---

# Getting started

This page builds the reference CLI and runs it against the worked example that ships with the
repository. At the end you will have seen the four operations everything else builds on: validating
a document tree, resolving a task into a plan, asking whether an action is permitted, and evaluating
a task against evidence.

## Prerequisites

| Tool | Needed for |
|---|---|
| Rust (recent stable) | everything |
| [go-task](https://taskfile.dev) | the repository's gate (`task check`) — optional for this page |
| Go toolchain, `wasm32-unknown-unknown` target, Node | only the synthesis parts of the gate — not needed for this page |

## Build

```console
$ git clone https://github.com/codewandler/engineering-protocols
$ cd engineering-protocols
$ cargo build -p protocol-cli
$ B=target/debug/protocol
```

The rest of this page uses `$B` for the binary.

## 1. Validate the document tree

```console
$ $B validate
39 file(s): 3 protocol(s), 22 principle(s), 4 workflow(s), 5 profile(s), 5 lifecycle(s)
valid
```

`validate` is not a schema check. It refuses, among other things: a predicate that reads a fact no
protocol declares observable, a workflow state nothing can reach or leave, and a rollback policy
that cannot state its precondition — the ways a rule ends up looking enforced while doing nothing.
Every refusal carries a stable code and all problems are reported in one run.

## 2. Resolve a task into a plan

The worked example is a feature task — adding passkey authentication — governed by the standard
development profile. The task file names exactly two things: an objective and a profile.

```console
$ $B resolve --task examples/development-passkeys/task.yaml
inputs      . and examples/development-passkeys/task.yaml
task        AUTH-142 (feature)
objective   add-passkey-support
protocol    adp/1
profile     development.standard
workflow    adp/default (initial: receive)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking, contract-testing, property-based-testing, approval-gates, reversible-changes
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

The nine principles, the workflow and the twelve capability decisions are all **derived** from the
profile. Nothing in the task restates them, so nothing in the task can drift out of step with them.

## 3. Ask whether an action is allowed

```console
$ $B explain --task examples/development-passkeys/task.yaml --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
$ echo $?
1
```

Each line does work: the **reason** names a principle and a rule inside it that a person can go and
read, the **missing** line says what would unlock the action, and the **state** says where in the
workflow the question was asked. Nobody wrote this denial into the task or the profile —
`approval-gates` is in force because `development.standard` includes it.

## 4. Evaluate against evidence

Evidence is submitted as records — here, a test run that produced one failing test:

```console
$ $B evaluate --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/01-red-test.yaml \
    --advance
state       implement (Implement)
transitions
  implement -> verify [blocked]
      guard: diff.exists
Task incomplete in `implement`:
  ✗ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
      tests.unit.failed = 1; unobserved: static_analysis.errors; evidence.missing = 7
  ? specification.satisfied                                       [principle spec-driven]
      unobserved: specification.satisfied
  ...
```

One failing test plus an approved specification is enough to reach the `implement` state — and only
a *failing* one, because the workflow enforces red-before-green as a fact about submission order.

Note the two failure marks. They mean different things and want different responses:

| Mark | Meaning | Next move |
|---|---|---|
| `✗` | observed, and wrong | fix the code |
| `?` | nothing has observed it | run the verifier that would |

This is the protocol's three-valued evaluation: `Unknown` is not `False`, and only `True` permits a
transition. See [Evidence and completion](./concepts/evidence.md).

Submitting the example's remaining evidence files carries the task through to `complete`. The
directory `examples/development-passkeys/evidence/` holds all five, and
[Govern a task](./guides/govern-a-task.md) walks the full sequence.

## Machine output

Every command above takes `--format json` or `--format yaml`. Refusals, decisions and evaluations
all serialise; exit codes are stable (`0` success, `1` refused/invalid). Show the text to people and
the JSON to programs.

## Next steps

* [Govern a task](./guides/govern-a-task.md) — put a task of your own under a profile.
* [Write a principle](./guides/write-a-principle.md) — encode a rule of your team's.
* [Write a specification](./guides/write-a-specification.md) — the ESS half: derive contracts and
  tests from one document.
* [Integrate an agent harness](./guides/integrate-a-harness.md) — make these answers govern a real
  agent, via the engine API rather than the CLI.
