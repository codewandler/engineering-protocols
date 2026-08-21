---
format: aep.planning-md/1
id: story:sqlite-backend
kind: story
status: draft
title: 'P4: aep-backend-sqlite'
summary: The first database backend — one file, no server — passing the same sixteen suites the markdown store passes.
owner: store
tags:
- backend
- store
relations:
- decomposes: epic:planning-store-as-backend
- depends_on: story:journal-backed-store
revision: 1
---
# Story: P4 — `aep-backend-sqlite`

## Outcome

A team that wants their plan in a database instead of in files changes one line of `project.yaml` and
keeps every verb, every lifecycle and every relation exactly as it was.

## Context

The obvious next durability step after the journal: one file, no server, real transactions. It is
also the first honest test of the claim that the contract is the seam — a backend written against the
same sixteen suites, by someone reading the suites rather than the markdown store's source, either
passes or shows the suites are underspecified.

## Acceptance

- The crate passes the sixteen suites unchanged, with no suite gaining a backend-specific branch.
- A store created by the CLI, written to and read back survives a process restart and a concurrent
  reader.
- The dependency added is named in `AGENTS.md` § *Dependencies* with the refusal alternatives that
  were considered, per the standing policy.
- Switching a project between the markdown and SQLite backends does not change what any verb prints
  for the same plan.

## Out of Scope

Migration between backends. Moving an existing plan from one store to another is its own problem and
does not belong inside the second implementation of a contract.

## Open Questions

Whether the journal is a table or an append-only blob. Decides: store owner, together with P3's
on-disk question — the two answers should not be independent.
