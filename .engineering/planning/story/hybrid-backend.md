---
format: aep.planning-md/1
id: story:hybrid-backend
kind: story
status: draft
title: 'P6: aep-backend-hybrid, and what its atomicity actually is'
summary: A composite that writes to the database first and projects to markdown second, compensating through the protocol's own inverse commands when the projection fails.
owner: store
tags:
- backend
- store
relations:
- decomposes: epic:planning-store-as-backend
- depends_on: story:postgres-backend
revision: 1
---
# Story: P6 — `aep-backend-hybrid`, and what its atomicity actually is

## Outcome

A team gets both halves at once: the plan is in a database their tooling can query, and it is also in
markdown their pull requests can review — and when one of the two writes fails, somebody can say
exactly what state the plan is in.

## Context

The shape is settled and the guarantee is not. A composite holds a primary (SQL) and a projection
(markdown); the write goes to the primary first because that is the one with transactions, and the
projection follows. A projection that fails after a committed primary write leaves the two
disagreeing, and the composite compensates through the protocol's own inverse command rather than
through a `DELETE`. What exact atomicity that buys is an open design question, and the honest options
run from *eventually consistent with a repair verb* to *two-phase with a durable intent log*.

## Acceptance

- The atomicity guarantee is **written down first**, as a decision with its rejected alternatives,
  before the crate exists.
- The sixteen suites pass against the composite.
- A projection failure after a committed primary write is compensated by an inverse command, and the
  compensation is in the journal — not a repair somebody ran by hand.
- A divergence between primary and projection is detectable by a verb, and the verb says which side
  is authoritative.

## Out of Scope

Choosing the guarantee without P3's suites to test against, which would be guessing. That is why this
story is last in the epic rather than merely later.

## Open Questions

The guarantee itself. Decides: store owner, on a written decision, before any code. Default if nobody
answers: eventually consistent with an explicit repair verb — the weakest honest claim, which is the
one that cannot be quietly wrong.
