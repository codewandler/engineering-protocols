---
format: aep.planning-md/1
id: story:evidence-horizons
kind: story
status: implemented
title: A green result from three weeks ago is not a fact
summary: Evidence requirements carry a horizon; past it the fact reads Unknown, which permits no transition.
owner: protocol
tags:
- adoption
- evidence
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 4
---
# Story: A green result from three weeks ago is not a fact

## Outcome

A reader of a store can tell *checked, and it held* from *checked once, at a date, and nobody has
looked since*. Past its horizon a fact reads `?`, and a guard over `?` does not fire — so the failure
mode is a refused transition with the reason "nobody knows", never a wrong one.

## Context

An early adopter's review, round 1 — **item C1** — **first** in the adopter's
own ranked fix order, on the grounds that it is the smallest change with the largest corpus behind
it. Their store carries **145 live dated claims**, each with a means and a horizon, and it exists
because one round of re-checking found **four** assertions that were true when written and false when
read. To this engine an admitted fact is timeless: nothing records when the observation happened, and
nothing says how long it is worth anything.

The proposal is `horizon: 3d` on an evidence requirement. The decay direction is the whole design:
past the horizon the fact becomes `Unknown`, never `False`, which is a distinction the three-valued
engine already holds — `Truth::Unknown` propagates through the Kleene operators in `aep-domain` and
permits nothing. Two traps come with it and are inherited deliberately rather than solved: a claim can
be false *inside* its horizon, because a horizon is a volatility guess and not a guarantee; and
re-verifying means re-checking and re-dating, never extending the life of the record that already
exists.

`story:time-based-transitions` (§ D2) is the general mechanism for a clock the protocol can see. This
story may land first on the narrowest clock read it needs, which is why the edge between the two is
`informed_by` and not `depends_on` — the ranked-first item does not wait for the general case.

## Acceptance

- An evidence record carries an observation time, and a requirement may carry a horizon; both survive
  a round trip through the store and the CLI.
- Past the horizon the observable the record fed reads `Unknown`, and the transition it used to permit
  is refused with a reason naming the horizon and the observation time.
- Re-submitting the identical record does not restore it; only a record with a new observation time
  does — asserted, not documented.
- A fixture built to the shape of the adopter's corpus (a dated claim, a means, a horizon) evaluates
  the same way in both renderings.
- The synthesized corpus at `examples/evidence-horizons-corpus/` (invented subjects, leak-scanned,
  built by the adopter's session for exactly this story) is the regression target: 42 raw
  annotations of which the adopter's own reference implementation finds only 37 —
  `expected.json` says so itself (`reference_is_not_ground_truth: true`) and names each gap. A
  conforming implementation finds all 42 and still rejects the two deliberate negatives.
- **Coverage is self-reported**: an implementation that scans human-written documents reports
  occurrences-seen versus records-produced, and a divergence is a finding, never a silent drop —
  the adopter measured ~9% of a live corpus invisible to the gate whose job was making unchecked
  claims visible, and the one-line raw-vs-parsed count is what surfaced it.
- The malformed-default horizon (14d) is also a legitimately chosen horizon — malformedness is a
  carried flag on the record, never inferred from the value.
- The two traps (`corpus/05-traps.md`) both classify `ok` on purpose: no parser can catch them.
  What the model must make true instead: shortening a horizon is cheap and attributable, and a
  horizon that grew while its observation date did not is detectable from history in the store —
  the API offers no horizon mutation at all.
- **Observation and schedule are two fields, never one.** `observed_at` is required, is the
  identity of the fact, and **a future value is a validation error, not a fresh record** — the
  adopter found the one-field convention silently classifying scheduled-but-never-performed
  checks as the freshest records in the corpus (negative age inflates remaining horizon). A
  planned re-check is a different object from a decaying observation; the model must be able to
  answer "has anyone ever looked at this?". Cheap gate: an observation date in the future is
  rejected — one comparison that makes the conflation unwritable.
- Calibration: 42/42 on the vendored corpus is the target; the adopter's corrected reference
  reaches 158/160 on their live corpus and is explicitly not a completeness claim.

## Post-implementation, 2026-08-21 (same evening)

The adopter re-pulled the corpus after fixing their reference against it: `expected.json` is now
**ground truth** — 43 raw, 43 parsed, `missed_by_reference: 0` — with `reference_is_not_ground_truth`
kept as a field, `false`, and the reason beside it (positions 1–3 were each believed complete before
4–7 turned up; assume there is another). The revision added **position 7** (a backticked annotation
mid-line, after prose) and the inverse rule: an annotation inside a **fenced code block** is an
illustration and is excluded from parsing *and* from the coverage denominator — fence it if you are
illustrating, anything else parses. The scanner and this repository's vendored copy follow;
`aep-backend-markdown` finds 43/43. Re-verification on their side also produced trap 1 for real —
a claim false inside its horizon within 48 hours of the fixture row for that trap being written —
and the durable-fact consequence is `story:claim-retirement`, not an amendment here.

## Out of Scope

Whether a *verifier* can be trusted to state its own observation time honestly, which is § C5's
territory and belongs to a later round. Also out: C3's environment revision and C4's determinism
model — both are about what an observation was *of*, not when it happened.

## Open Questions

Whether the horizon is declared on the requirement, on the evidence record, or on both with the
stricter winning. Decides: protocol owner. Default if nobody answers: **on the requirement**, because
the requirement is the thing the protocol controls, and a record that could set its own expiry is a
record that can extend itself — the exact move the adopter's second trap forbids.
