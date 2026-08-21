---
format: aep.planning-md/1
id: story:reusable-workflow-nodes
kind: story
status: draft
title: Retry, circuit-break, and a third party simulated from its own spec
summary: Common step behaviours as typed, testable decorators in the step map — and a dependency we do not own, doubled from its ESS specification so a drive run needs no third party.
owner: driver
tags:
- driver
- harness
relations:
- decomposes: epic:reference-driver
- depends_on: story:default-step-map
- informed_by: story:retry-budgets
revision: 2
---
# Story: Retry, circuit-break, and a third party simulated from its own spec

## Outcome

A step map declares the behaviours every real workflow ends up needing — *retry this*, *stop calling
that after it has failed enough* — as typed, testable things in the map, and a run that depends on a
third party we do not own can be driven **offline**, against a double synthesised from that party's
own specification.

## Context

Asked for by the operator, 2026-08-21, on top of the reference driver rather than as adopter feedback.
Two halves, and the second is the one this repository is unusually placed to do.

**Reusable node behaviours.** Retry and circuit-break are written into every step map by hand, or not
at all, and each hand-written copy is a slightly different set of rules that no test covers.
`story:retry-budgets` gives the driver a per-*kind* budget and is deliberately about crash recovery —
this story is the other axis: a **declared decorator on one step** in the map, typed and validated
before anything runs, so *this command retries twice* is a property of the map a reader can see and a
test can exercise. Circuit-break is the same construct with a different exit condition; it stops a
step map from hammering something that is already down.

**Simulated externals.** For a dependency nobody here owns, the machinery already exists and has never
been pointed at a step map:

- `ess conform` **synthesises behaviour from a specification** — scenarios generated per construct,
  refusing what the specification does not say enough about to test rather than omitting it
  (`docs/guide/specification.md:100-111`);
- the `external:` construct on an outcome says an input cannot decide it — *"a generator reads
  `external` and injects a fault instead of trying to construct an input"* (`docs/guide/specification.md:355-363`);
- `--inject <fault>` already breaks one property on purpose and names the scenario that catches it
  (`docs/guide/specification.md:127-131`).

So a step map should be able to declare *this dependency is simulated per specification X* and be
driven with no network, no credential and no third party — which is exactly the shape
`story:shell-echo-harness` proved for the model seam, applied to the dependency seam.

## Acceptance

- A step map declares a retry-wrapped command step and a circuit-broken step; both are validated
  before the run starts, and an undeclared or malformed decorator is **refused** rather than ignored —
  the same rule the untyped failure policy in `story:adopter-bugs` failed.
- A step map declares an external dependency as simulated against a named specification, and the
  driver stands up the double from that specification.
- **One `protocol drive` run exercises both** — the retry fires and is counted, and the simulated
  external answers — with no real third party, no network and no credential, asserted by the run's own
  record.
- A fault injected into the simulated external is observable in the run: the step map's declared
  behaviour on that failure is what happens, and the record says which fault was injected.
- The retry decorator and the per-kind retry budget compose to a stated rule rather than to whichever
  fires first, and the rule is asserted.

## Out of Scope

Recording and replaying real traffic. The double comes from a specification, not from a capture — a
recorded fixture is a claim about one afternoon, and this is a claim about what the party said it
does.

## Open Questions

Whether a simulated external is allowed in a run whose evidence is admitted. Decides: driver owner.
Default if nobody answers: **allowed, and marked in the record** — a fact observed against a
simulation is a real fact about the simulation, and the record has to say so or the evidence model
learns to lie.
