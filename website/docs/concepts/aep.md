---
title: "AEP: governing engineering work"
sidebar_position: 2
description: The document types, how a task resolves into a plan, how capabilities compose, how a workflow advances on evidence, and what actually runs the steps.
---

# AEP: governing engineering work

AEP turns engineering methodology into documents a program executes. This page explains the
document types and the resolution and evaluation model. For syntax, see the
[document reference](../reference/documents.md); for hands-on use, see
[Govern a task](../guides/govern-a-task.md).

## The document types

| Document | Declares | Example |
|---|---|---|
| **Protocol** | the vocabulary: which capabilities, evidence kinds, verifiers, phases and observable fact families exist | `aep/1` (base), `adp/1` (development), `aop/1` (operations) |
| **Principle** | one enforceable rule: when it applies, what it requires, by when, and who may attest it | `test-driven`, `approval-gates` |
| **Workflow** | a state machine whose transitions are guarded by predicates over evidence | `adp/default` |
| **Profile** | a bundle: protocol + workflow + principles + capability policy + completion condition | `development.standard` |
| **Artifact lifecycle** | which statuses an artifact kind may hold and how they may change | `architecture-decision-record` |
| **Step map** | which program, model or person runs in each workflow state | `development/default` |
| **Task** | the unit of work: objective, kind, profile, declared context facts, artifact manifest | `AUTH-142` |
| **Artifact manifest** | where the specs, designs and reviews live — references, not copies | `artifacts.yaml` |

Documents are YAML, validated against generated JSON Schemas, and indexed by the `id` declared
*inside* the file — never by filename. A team's own principles and profiles live beside the vendored
upstream ones in the same tree.

## Resolution: a task becomes a plan

`resolve` (or `Engine::initialize` in the API) takes a task and produces a plan:

1. The task names a **profile**; the profile's `extends` chain is merged. Extension can only
   tighten: completion conditions are conjoined, and a denied capability cannot be granted back.
2. Each principle's `applies_when` is evaluated against the task's facts. A rule that does not
   apply is **absent**, not vacuously satisfied — so every check in a report meant something.
3. Capabilities compose across protocol, profile, principles and workflow state, with the document
   responsible recorded for every entry.
4. Obligations are collected and timed against workflow phases.
5. The whole configuration is **refused if any rule in it could never fire** — a predicate reading
   an undeclared fact, an obligation timed against a phase no state declares, a task needing a
   capability the resolved policy denies.

The task states only what cannot be derived: its objective, its kind, and facts nothing can observe
(for example `change.database_schema: true`). Everything else is derived, so it cannot drift.

## Capabilities: default deny, with an approval floor

A capability no document mentions is **not granted**. Three decision levels exist, and precedence is
fixed: `deny` beats `require_approval` beats `allow`. A `deny` cannot be granted back by a later
document, so a denial works as a safety envelope rather than a starting suggestion.

Two rules shape who may say what:

* A **principle may only restrict** (deny or require approval). Only a profile or a protocol may
  grant.
* The protocol holds an **approval floor**: `aep/1` refuses to resolve any profile that puts
  `production.write` or `deployment.create:production` in its `allow` list. The mistake cannot be
  made, rather than being caught in review.

Every action an agent might take maps to exactly one capability
(`repository.write`, `tests.execute`, `secret.read`, …), so authorisation is a lookup, not a
judgement. The full list is in the [vocabulary reference](../reference/vocabulary.md).

## Workflows: evidence moves the state

A workflow is a validated state machine: an initial state, terminal states, and transitions guarded
by predicates over recorded facts. Validation refuses unreachable states, non-terminal dead ends,
duplicate transitions for one `from`/`to` pair, and rollback policies that cannot state their
precondition.

The engine advances a workflow only when the guard evaluates `True` against submitted evidence, and
a blocked transition names exactly what is missing. When more than one transition is legal, the
first in document order is taken and the rest are reported — a choice is visible, never a coin
flip.

States can carry phase labels (`intake`, `implementation`, `verification`, …). Principles time their
obligations against phases (`before_implementation:`), which is what lets one principle work across
different workflows.

## Step maps: something has to actually run the steps

A workflow says which states exist and what moves between them. It does not say who does the work.
A **step map** does: for each state, a list of steps, each of one of three kinds.

| Kind | What runs |
|---|---|
| `command` | a program — a test suite, a linter, a build |
| `llm` | a model, in a headless session |
| `operator` | a person; the run stops and waits |

`protocol drive run` walks the map, makes the engine's calls in order, and records what it did. It
evaluates **no gate itself**. That restraint is the design: a driver that could decide whether a
transition is permitted would be a second implementation of the protocol with none of the
conformance suites behind it, and the first time the two disagreed, the one nobody tested would win.
So the driver asks, the engine answers, and a blocked run prints the engine's reasons and exits
non-zero — which is what happened the first time it was pointed at a real story, recorded in
[Where this stands](../status/where-this-stands.md).

A driven run needs a shell to reach the planning store, which is why `development.driven` exists: it
extends `development.standard` with exactly one capability, `command.execute`, and says in the
document why. The narrowing back down is a `PreToolUse` hook that refuses any shell call that is not
one simple `protocol artifact` or `protocol trace` invocation. The profile states plainly that this
is pattern-based and best-effort rather than claiming the capability is fully enforced.

## Everything is recorded, including refusals

Every mutation of an engineering entity goes through one boundary — a command carrying actor and
executor, correlation and causation, an idempotency key and an asserted revision. A retry is
recognised; a stale write is refused rather than merged. There is no delete: `ArchiveEntity` and
`SupersedeEntity` are the vocabulary.

Refusals are first-class: a refused command changes nothing and still lands in the audit trail, and
`authorize` records the request and the answer even when the answer is no. "An agent tried to change
production and was stopped, by this rule" is only worth something written down beside the changes
that succeeded.

## Approvals bind to revisions

An approval names the revision it approved. An approval of version 3 of a design stops satisfying a
review requirement once the design reaches version 7 — otherwise a reviewer's name ends up attached
to a decision they never saw. Review requirements can demand `fresh: true` (the review covers the
artifact's current version) and `human: true`.

## What evaluation returns

`evaluate` answers three questions at once: what is permitted now, what is owed and by which
document, and whether the task is complete. Every requirement line names the document that asked for
it, and every predicate outcome is three-valued — see
[Evidence and completion](./evidence.md) for why `Unknown` and `False` must stay distinct.

A requirement can also declare a **horizon**, after which the evidence that satisfied it stops
counting and the requirement reads `Unknown` again. That is the same page's subject: a fact has a
date, and an old fact is not a wrong one.

---

**Sources.** `docs/guide/adopting.md`; `docs/guide/harness.md`;
`docs/plan/document-authoring-brief.md`; `drivers/development/default.yaml` and
`crates/aep-driver-spec/src/map.rs` (the three step kinds); `profiles/development-driven.yaml`;
`AGENTS.md` § *Invariants* 6, 10, 14–16; `protocols/aep/1.yaml` (`approval_floor`).
