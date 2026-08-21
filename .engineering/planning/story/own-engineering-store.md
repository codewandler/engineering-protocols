---
format: aep.planning-md/1
id: story:own-engineering-store
kind: story
status: active
title: The repository's own .engineering/, holding this backlog
summary: A project.yaml pointing at this repository as its own protocol tree, and the real roadmap as artifacts the driver can evaluate a gate against.
owner: protocol
tags:
- dogfooding
- store
relations:
- decomposes: epic:reference-driver
revision: 3
---
# Story: The repository's own `.engineering/`, holding this backlog

## Outcome

Somebody who clones this repository and types `protocol artifact list` — with no flags, from anywhere
inside it — sees the actual roadmap, in the store the repository publishes, governed by the
lifecycles the repository publishes.

## Context

The wave-1 store was built and pointed at a fixture. The repository's own plan stayed in hand-written
wave pages that nothing validates, which is the protocol's methodology not applied to the protocol's
own backlog. It is also a prerequisite rather than a gesture: a driver with no store has no artifacts
to evaluate a gate against, so the store has to exist before a real story can be driven.

## Acceptance

- `protocol artifact list` run anywhere inside this repository with **no** `--store` answers from
  `.engineering/planning/` — through project discovery, not through a flag.
- `protocol artifact validate` is green over the store, and the command and its output are recorded
  rather than the claim.
- `project.yaml` names this repository as its own protocol tree, and the path is right for the reason
  the file states: paths are resolved against `.engineering`, so the tree is `..`.
- The store holds this wave's own stories, so the first thing the repository governs with it is the
  wave that built it.

## Out of Scope

Migrating the wave pages into the store. The pages carry reasoning, which is what they are for; the
store carries state, which is what it is for. Duplicating one into the other creates two answers to
*what is the status of this*.

## Open Questions

Whether `artifact validate` joins the project gate. Decides: protocol owner. Default if nobody
answers: **yes** — it is local, clock-free and sub-second, the same argument that placed
`status-check` there — and `AGENTS.md` § *Gate* gains its row in the same change, because a gate whose
step list disagrees with the Taskfile is the drift invariant 1 exists to prevent.
