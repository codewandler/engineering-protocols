---
format: aep.planning-md/1
id: story:claim-retirement
kind: story
status: draft
title: An answered question must not leave a permanent re-check obligation behind it
relations:
- decomposes: epic:adopter-feedback-round-1
- informed_by: story:evidence-horizons
revision: 3
---
# Story: An answered question must not leave a permanent re-check obligation behind it

## Outcome

A maintainer of a dated-claim corpus can retire an annotation whose subject has stopped being
mutable, as a recorded move rather than a deletion — so an answered question does not sit in the
store as a claim forever going stale, and the convention does not decay into noise that gets muted.

## Context

From the adopter's session, the same evening `story:evidence-horizons` shipped. Re-verifying the
claims their fixed parser surfaced found one **false inside its horizon, within 48 hours of the
fixture row for exactly that trap being written** — trap 1 occurring for real, which is a stronger
argument for shortening-is-cheap than the fixture row itself. The instructive part was the fix: the
right move was **not** a new `Verify:` with a bumped date. The subject had shipped, so the claims
about it had become **durable dated facts** — "X was cut by Y at T" — and a durable fact must not
carry a horizon at all. The model has decay and re-observation; what it lacks is the third move: a
claim gracefully **stopping being live-state**. Without it, every answered question leaves a
permanent re-check obligation behind it, and a corpus of those is a corpus somebody eventually
stops reading.

The retirement rule is the same family as the horizon-immutability rule: both exist because the
convenient edit (extend the horizon; delete the stale line) is the one that erases the record.

## Acceptance

- Retiring a claim is a supported operation, distinct from deletion: the record survives with its
  observation date, and what changes is that it no longer generates a re-check obligation.
- A retired claim states why it is durable (the subject stopped being mutable), and the statement
  is attributable — who retired it, and when.
- A durable fact carries no horizon; writing one onto a retired record is refused the way horizon
  mutation is refused.
- Un-retiring is a new observation, never an edit — the same one-directional shape as
  re-verification.
- The coverage self-report counts retired records on their own axis, so retirement is visible in
  the numbers and mass-retirement reads as the finding it would be.

## Out of Scope

Time-based transitions in general (`story:time-based-transitions`), and any change to how live
claims decay — this story only adds the exit. Also out: deciding *for* the maintainer when a
subject has stopped being mutable; that is a judgement the record carries, not one the parser
makes.

## Open Questions

Whether retirement is a marker in the annotation convention (a scanner concern, in
`aep-backend-markdown::claim`) or a state on the evidence record (a domain concern). Decides:
protocol owner. Default if nobody answers: **the convention first** — the scanner is where the
corpus lives today, and a domain state with no surface writing it would be vocabulary without a
speaker.
