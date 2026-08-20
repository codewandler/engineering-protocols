---
title: The join, in full
sidebar_position: 4
description: One principle connects the two halves. Here it is, including the part that fails closed and the check that was deliberately not added.
---

# The join, in full

Everything on this site about "the two halves meet at evidence" comes down to one document. It is
short enough to read whole, so here it is — `principles/verification/ess-conformance.yaml`, comments
included, because the comments are where the reasoning lives:

```yaml
id: ess-conformance
version: 1
title: Conformance to the specification
summary: >-
  Where an executable system specification governs the work, the implementation must be checked
  against that specification's own generated suite, by something other than the agent that wrote the
  implementation.

requires:
  before_completion:
    conditional:
      # Only where a specification exists. A task with no ESS owes nothing here — and the condition
      # is checked against the artifact graph at evaluation time, not guessed at resolution time,
      # so adding a specification to a project turns the rule on without editing this document.
      - when: artifact.executable-system-specification.exists
        require:
          predicates:
            # Not `status == passed`: a run that reported success alongside failing scenarios is
            # contradicting itself, and this fact takes the pessimistic half of that.
            - ess_conformance.passed
            - ess_conformance.scenarios.failed == 0
          evidence:
            - kind: ess_conformance
              # The whole point. An agent's report that its own implementation conforms is not a
              # conformance run, and the requirement says so mechanically rather than in a comment.
              independent: true
              verifier: conformance-runner
          artifacts:
            - kind: executable-system-specification

verification:
  - verifier: conformance-runner

on_failure: block
```

## What it means for a person

Nobody has to read a diff and judge whether it matches the specification. The specification judges
it, and the protocol refuses to call the task done until it has.

## Three decisions in it worth noticing

**It is conditional on the artifact graph, not on configuration.** A project that adds a
specification turns this rule on without anyone editing the rule. A project without one owes nothing.

**It takes the pessimistic half of a contradiction.** The predicate is not `status == passed`. A run
that reported success alongside failing scenarios is contradicting itself, and
`ess_conformance.scenarios.failed == 0` is the half that does not let it through.

**The run must be current, and that fails closed.** The requirement binds a run to the specification
*revision*: it counts only if its `spec_digest` is the `model_digest` that specification records — so
a suite run against yesterday's model no longer closes a task built against today's. If the
specification artifact records no digest at all, no run can be shown to be current and **the
requirement can never be satisfied**. Evidence that cannot demonstrate which revision produced it is
not assumed to be fresh. The way out is declared on the artifact, in the manifest, where a person can
see it under review — not by loosening the rule.

## The check that was deliberately not added

The obvious extra predicate would be `ess_conformance.spec_digest.exists`. It was left out on
purpose, and the reason generalises past this file:

> the field has been required since gate G11 — and a check that cannot fail is worse than no check,
> because it reads as protection.

## What does not exist yet

The `conformance-runner` this principle asks for. Generating a conformance suite from a specification
is ESS wave 4, which has not started; the suite, the runner and the deliberately wrong implementation
that proves the suite bites are all part of it.

Until then a person produces that evidence by hand. The shape is already right: when the runner
arrives, nothing on the protocol side changes.

The acceptance criterion wave 4 is held to is worth quoting, because it is the lesson the protocol's
own conformance work already cost this project once:

> **The suite must fail the specific check each fault exists to break** — not merely fail. A
> generated suite that has never failed is a suite nobody has a reason to trust, and one that fails
> for the wrong reason is worse, because it looks like evidence.

That standard is already met on the protocol side of the house: sixteen black-box conformance suites,
three levels, and a deliberately broken backend the suites are checked against.

---

**Sources.** `principles/verification/ess-conformance.yaml` (quoted in full);
`docs/plan/ess-roadmap.md` § *ESS wave 4*; `docs/guide/specification.md` § *Requiring conformance*;
`README.md` (the conformance suites and the faulty backend).
