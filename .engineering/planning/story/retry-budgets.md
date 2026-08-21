---
format: aep.planning-md/1
id: story:retry-budgets
kind: story
status: draft
title: Retry budgets per step kind, spent and never reset
summary: A crashed step retries within its kind's budget, a green third attempt does not erase the first two, and exhaustion leaves a resumable snapshot.
owner: driver
tags:
- driver
relations:
- decomposes: epic:reference-driver
- depends_on: story:protocol-drive-verb
revision: 1
---
# Story: Retry budgets per step kind, spent and never reset

## Outcome

A run survives a process that died without pretending it never died: the operator sees *green on the
third try* as three attempts, not as one success.

## Context

A crashed step and a failing suite are different facts and must land on different sides — nothing
observed versus something observed to be false. The budget belongs to the step **kind**, because the
reasons differ: a `command` step retries because a process died, an `llm` step retries once because a
model call errored, and an `operator` step never retries, because re-prompting a person who has not
answered is the driver deciding a human is a transient fault.

## Acceptance

- A `command` step whose executable does not exist submits **zero** evidence and leaves the
  evaluation unchanged.
- A suite that runs and fails submits a `TestResult` with failures, and the next transition moves
  `verify → implement`.
- The cursor's attempt count survives the retry that succeeded, and the run report names it.
- Exhaustion leaves a resumable snapshot, prints `Blocked`'s reasons and the completion explanation
  **verbatim**, and adds exactly one line of the driver's own naming the budget and the step.
- The per-state visit budget and the per-step retry budget are separate bounds — a legitimate
  `verify → implement` cycle is not reported as a wedged command.

## Out of Scope

Backoff. An unbounded retry with backoff is a bound nobody can state, which is a token budget nobody
can state.

## Open Questions

Whether `command` steps get a per-step override in the map. Decides: driver owner. Default if nobody
answers: no override — one number per kind, in the map's header, is what makes a run's behaviour
readable.
