---
format: aep.planning-md/1
id: epic:planning-store-as-backend
kind: epic
status: draft
title: The planning store answers as a backend
summary: 'P3-P6: the markdown store writes through CommandService, runs the sixteen conformance suites, and gains database and hybrid siblings.'
owner: store
tags:
- backend
- store
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: The planning store answers as a backend

## Outcome

The store this plan lives in is a real implementation of the storage contract, not a special case
beside it. It writes through `CommandService`, it has a journal that can say what an artifact looked
like three revisions ago and who moved it, and it passes the same sixteen `aep-conformance` suites
any other backend has to pass. After that, the same plan can live in SQLite or Postgres without a
second write path or a second vocabulary.

## Why Now

`aep-backend-markdown` writes through its own `create`/`update` rather than through
`CommandService` — deviation **D-P1** against invariant 14, recorded on the way in rather than
discovered. The consequence is stated in the gap register: the sixteen suites do not run against it,
and it has no journal, no audit join and no history (**D-P3**). Until P3, *"there is a durable
backend"* is a claim the suites do not support, and `AGENTS.md` § *Current state* has to say both
halves.

## Scope

P3, P4, P5 and P6 of the backend roadmap in
[`harness-planning-and-driver-design-v0.1.md`](../../../docs/design/harness-planning-and-driver-design-v0.1.md)
§ 2.8. P3 is the load-bearing one: it is what makes every later backend a narrowing rather than a
redesign, because the seam is the existing `aep-contract` traits and the suites are a black-box
definition of what implementing them means.

## Out of Scope

A new `PlanningStore` trait. That would create a second write path, which invariant 14 exists to
forbid, and `crates/aep-contract/tests/write_surface.rs` would fail the moment it was declared —
which is the invariant saying no, not an obstacle to route around. Also out: a
`protocol entity --planning` bridge, because at P3 there is nothing to bridge; the store answers as
a backend.

## Risks

P6 is the least settled item in the family. What atomicity a primary-plus-projection composite
actually buys runs from *eventually consistent with a repair verb* to *two-phase with a durable
intent log*, and choosing between them without P3's suites to test against would be guessing — which
is why it is last rather than merely later. The second risk is dependency cost: P4 and P5 each add a
third-party crate to a workspace with a written policy about that.

## Done When

`protocol conformance` runs the sixteen suites against the markdown store and passes; a story's
history is answerable from the store rather than from `git log`; and at least one database backend
passes the same suites unchanged.
