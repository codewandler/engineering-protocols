---
format: aep.planning-md/1
id: story:protocol-drive-verb
kind: story
status: proposed
title: 'protocol drive: the run that touches the world'
summary: The command, llm and operator executors, the run directory, the store lock, and the flags that resume, restart or take it.
owner: driver
tags:
- cli
- driver
relations:
- decomposes: epic:reference-driver
- depends_on: story:driver-router
- depends_on: story:default-step-map
revision: 2
---
# Story: `protocol drive` — the run that touches the world

## Outcome

An operator starts a run in their repository, walks away, and comes back to either a completed
workflow or a run that stopped somewhere specific with the engine's own words for why — and a
directory they can resume from.

## Context

This is the impure half, deliberately in `protocol-cli`: the three executors that touch the world,
the run directory, the store lock and the pid-liveness probe. It is where the driver's decisions
either survive contact or do not. The store's graph is rebuilt **between steps**, not per state,
because a `command` step can create an artifact and the next step in the same state would otherwise
evaluate against a store one write behind.

## Acceptance

- The lock lives at one fixed path per store, taken with `create_new` **before** a run id is
  allocated — two invocations racing cannot both succeed.
- A second `drive` against a locked store exits non-zero, prints the holder's run id, pid, host and
  the state the cursor says it is in, and **writes nothing** — asserted by an unchanged run directory
  and a clean tree.
- A store whose report is not clean stops the run and prints the accumulated errors verbatim; a
  single unparseable file stops it, asserted by the fact store being *unchanged* rather than silently
  shrunk.
- An artifact created by a step changes `artifact.<kind>.count` in the next evaluation, asserted on
  the fact store; a mutation is not observable through a previously built graph.
- A run that reaches an approval under `--pause-on-approval` persists, exits 0, releases the lock, and
  resumes.
- Two `Engine` values in one process do not collide on a run directory.

## Out of Scope

The hooks, which are the plugin's side of the same enforcement and ship after this. Retry accounting
and the lock-refusal wording are their own stories.

## Open Questions

Whether `--restart` should carry the previous run's cursor forward for reading. Decides: driver
owner. Default if nobody answers: it does not — a run directory is never reused, and `--take-lock`
already records what it superseded.
