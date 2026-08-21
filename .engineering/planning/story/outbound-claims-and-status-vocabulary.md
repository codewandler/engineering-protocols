---
format: aep.planning-md/1
id: story:outbound-claims-and-status-vocabulary
kind: story
status: draft
title: A claim that has left the building, and a rung for it to land on
summary: An outbound assertion gets a lifecycle with a clearance gate, and ArtifactStatus stops being a closed ten-variant enum.
owner: protocol
tags:
- adoption
- lifecycle
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: A claim that has left the building, and a rung for it to land on

## Outcome

A statement made to a customer is modelled as what it is — near-irreversible, owed a correction if it
turns out wrong — and the state *sent, known wrong, audience not yet told* is sayable in the store
rather than living in somebody's head.

## Context

An early adopter's review, round 1 — **items D3 and B1**, taken as one story
because the adopter's ranking pairs them: D3 is the concept and B1 is the enum that blocks it, and
either alone ships half a mechanism.

**D3** — every evidence path in this protocol flows inward. Nothing models an assertion crossing the
boundary outward. Their incident: *"resolved"* was told to a customer roughly **seven hours before**
the contradicting verification landed, and the ordering *is* the finding. Two constructs already here
fit, which is why this is additive rather than a redesign: `cleared` is an approval gate — the last
point at which a wrong claim can still be stopped — and an outbound communication is a
`production.write`-shaped act against a human system, so the floor logic applies unchanged.

**B1** — `ArtifactStatus` is a closed ten-variant enum, so a lifecycle document can only rearrange
fixed rungs. The expensive case is exactly D3's:
`draft → cleared → sent → correction-owed → corrected | retracted`, where **`correction-owed` has no
rung and no near neighbour**. It is a debt to a person, live right now in a store somebody runs. Three
more distinctions flatten in the same enum today: `expired` is not `archived` (the premise went stale
versus we refused), `failed` is not `rejected` (the lookup broke versus a human said no — the
retry-legitimacy distinction), and `blocked` is unsayable at all.

## Acceptance

- A lifecycle document can declare a status this repository has never heard of, and `move`, `list`,
  `board` and `validate` all handle it without a code change.
- An outbound-communication lifecycle of the adopter's shape validates and moves end to end, including
  the `correction-owed → corrected | retracted` fork.
- The clearance point is an approval gate, so a claim that has not been cleared cannot reach `sent`,
  refused with the engine's own words.
- What stays closed is stated where a reader will look: `evidence_kinds` remains closed, and the
  reason — it is the seam whose semantics are guaranteed — is written down beside the change rather
  than left to be re-derived.

## Out of Scope

Any transport. Nothing here sends anything, watches a mailbox or integrates with a helpdesk; the
protocol models the claim's lifecycle and the gate before it, and the act itself stays where
`production.write` already puts it.

## Open Questions

Whether an open status vocabulary keeps the ten current variants as *reserved names with fixed
meaning*. Decides: protocol owner. Default if nobody answers: **yes, reserved** — an adopter may add
rungs and may not redefine `implemented`, because a cross-store report that cannot rely on one word
is a report nobody can read.
