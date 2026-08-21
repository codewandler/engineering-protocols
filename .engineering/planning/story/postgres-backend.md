---
format: aep.planning-md/1
id: story:postgres-backend
kind: story
status: draft
title: 'P5: aep-backend-postgres'
summary: 'The backend an organisation actually runs: concurrent writers, real transactions, the same contract.'
owner: store
tags:
- backend
- store
relations:
- decomposes: epic:planning-store-as-backend
- depends_on: story:sqlite-backend
revision: 1
---
# Story: P5 — `aep-backend-postgres`

## Outcome

A plan can live where an organisation already keeps things it cares about, with concurrent writers
and the backup story it already has, and nothing above the contract notices the difference.

## Context

This is the backend an organisation actually runs, and it is the first one where two people writing
at once is the normal case rather than the exception. The conformance suites are what decide whether
it is correct; the interesting part is that the same suites now have to be satisfied under real
concurrency rather than under a single-process assumption.

## Acceptance

- The sixteen suites pass against a live server, in CI, without a suite gaining a backend-specific
  branch.
- Two concurrent writers to one artifact resolve to one accepted write and one refusal that names the
  revision it lost to — not a silent last-writer-wins.
- Schema creation and upgrade are a command, not a README instruction.
- The dependency and the CI service it requires are recorded in `AGENTS.md` § *Dependencies*.

## Out of Scope

Multi-tenancy, row-level security and anything about who may read which plan. Authorisation is the
protocol's job, not the backend's, and putting it here would create a second place that decides.

## Open Questions

Whether the driver's store lock becomes an advisory lock in this backend rather than a file. Decides:
driver and store owners together. Not blocking: the file lock is per store, and a Postgres store is
still reachable through one project directory.
