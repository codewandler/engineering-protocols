---
format: aep.planning-md/1
id: story:evidence-subject-binding
kind: story
status: draft
title: Evidence names its subject, and a guard checks it is the one being moved
summary: A fact whose subject is not the transition's subject is refused, so weeks of green about a component nobody ships cannot happen silently.
owner: protocol
tags:
- adoption
- evidence
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: Evidence names its subject, and a guard checks it is the one being moved

## Outcome

A fact can no longer be green about something nobody is shipping. Evidence carries the subject it
observed, and a transition refuses a fact whose subject is not the subject being moved.

## Context

An early adopter's review, round 1 — **item C2** — fourth in the adopter's
ranked order, and the one with the most literal incident behind it: an e2e job port-forwarded a
legacy service while the deployment rolled its successor. **Weeks of green about a component
nobody was shipping.** Nothing in the run was broken; every assertion was true of the thing it
actually talked to, and the thing it actually talked to was not the thing under test.

The engine has the analogous rule one layer over: the approvals rule that *version 3 does not satisfy
version 7* already refuses a record bound to the wrong revision. C2 is the same refusal over a
different axis — not *which revision* but *which subject* — and C3 (the environment revision a test
observed) is the third axis, deliberately left to a later round.

Subject binding and horizons (`story:evidence-horizons`) are the two halves of the same sentence: a
fact is a claim about *what*, observed *when*. Neither implies the other and both are missing.

## Acceptance

- An evidence record names its subject, and the name survives a round trip through the store, the CLI
  and both renderings.
- A transition offered a fact whose subject differs from the transition's subject is refused, with a
  reason printing both names.
- A record with no subject is not silently admitted — it is refused, or it is admitted under a stated
  rule that says why the omission is safe.
- The reported case reproduces as a fixture: a fact observed of a legacy service does not move its
  successor, asserted rather than described.

## Out of Scope

Deciding what a subject *is* for every domain. The protocol takes a name and compares it; inferring
subject identity from a URL, a namespace or a port-forward is the adopter's problem and would be this
protocol guessing.

## Open Questions

Whether subject comparison is exact-string or admits a declared alias table. Decides: protocol owner.
Default if nobody answers: **exact string**, because the incident is precisely a near-miss between two
names that a fuzzy comparison would have called equal.
