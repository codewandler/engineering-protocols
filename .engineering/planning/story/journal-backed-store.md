---
format: aep.planning-md/1
id: story:journal-backed-store
kind: story
status: draft
title: 'P3: the markdown store writes through CommandService'
summary: The store's two write functions reroute through command envelopes, the journal becomes the history it does not have, and the sixteen conformance suites run against it.
owner: store
tags:
- backend
- store
relations:
- decomposes: epic:planning-store-as-backend
revision: 1
---
# Story: P3 — the markdown store writes through `CommandService`

## Outcome

Anyone asking *"is there a durable backend?"* gets an answer the sixteen conformance suites support,
and anyone asking *"what did this story look like three revisions ago, and who moved it?"* gets an
answer from the store rather than from `git log`.

## Context

Deviation **D-P1** was taken on the way in and recorded: the store writes through its own
`create`/`update` rather than through the contract's one write path, which is why the suites do not
run against it and why it has no journal, no audit join and no history (**D-P3**). The mitigation was
that every write funnels through exactly two functions. This is the story that spends that
mitigation.

## Acceptance

- Both write functions route through command envelopes; a source scan finds no other write path in
  the crate.
- `protocol conformance` runs all sixteen `aep-conformance` suites against the markdown store and
  passes, against the same `FaultyBackend` baseline that proves the suites catch injected defects.
- The journal answers what an artifact looked like at a given revision, and who moved it, without
  reading git.
- `describe_type` reports the kind's lifecycle, closing **D-P5**.
- An out-of-band file edit is still indistinguishable in the file (**D-P2**) and the store says so
  rather than pretending otherwise.

## Out of Scope

A new store trait. The seam is the existing `aep-contract` traits; a second trait would be a second
write path and `crates/aep-contract/tests/write_surface.rs` would fail on the declaration.

## Open Questions

Where the journal lives on disk — beside the artifacts or in one file per store. Decides: store
owner. Default if nobody answers: one append-only file per store, because a journal fragmented across
kind directories is a journal that merges badly.
