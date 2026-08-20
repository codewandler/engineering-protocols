---
title: Why agents change this
sidebar_position: 2
description: An agent given a wiki page in a prompt produces work that reads as though it followed it. Prose instructions fail silently and plausibly.
---

# Why agents change this

A person who ignores the wiki page can be asked why. An agent given the same page in a prompt will
produce something that reads as though it followed it, at whatever scale you run it.

That is the whole difficulty, and it is not a difficulty about model quality. Prose instructions do
not fail loudly. They fail silently and plausibly — the output has the shape of compliance, and the
only way to find out otherwise is to read it.

## Review does not scale to what agents produce

Reviewing the output was always the fallback: a person reads the diff and judges whether it followed
the rules. Two things break that when the author is an agent.

* **Volume.** The cost of producing a change fell; the cost of reading one did not.
* **Plausibility.** An agent's report of its own work is written by the same process that did the
  work. "The tests pass" and "I ran the tests" are assertions, and reading them tells you nothing
  a competent generator could not have produced without running anything.

So the question is not how to review more. It is what can be decided **without** reading the output:
which facts, produced by whom, permit calling the work done.

## What changes when the rules are typed and executable

Each line below is a prose rule on the left and the mechanism that replaced it on the right.

| The rule everyone writes down | What it becomes here |
|---|---|
| "An agent cannot verify itself" | a type. An evidence requirement marked `independent: true` is not satisfied by the agent's own report of a green suite; a verifier's submission is a different `Producer` from an agent's |
| "Get approval before touching production" | a resolution failure. The protocol refuses to resolve a profile that grants `production.write` outright — `aep/1` holds it in an approval floor, so the mistake cannot be made rather than being noticed in review |
| "Write the test first" | an ordering fact: `evidence.first_seq.test_result < evidence.first_seq.diff`. Submission order is recorded, so red-before-green is checked rather than asserted |
| "Ada approved the design" | an approval that names the revision it approved. Version 7 is not covered by a review of version 3, so a reviewer's name does not end up attached to a decision they never saw |
| "Build what we specified" | a suite. The contracts, the tests and the skeleton come from one model, and conformance is something the implementation is run against |

## The part that is not a rule at all

`Unknown` is not `False`. `tests.unit.failed == 0` is *false* when a suite failed and *unknown* when
nothing ran. A harness needs different behaviour in each case — fix the code, or go run the tests —
and only `true` permits a transition.

That third value is what stops "we did not look" from reading like "we looked and it was fine", which
is the failure an agent produces most often and the one hardest to see in a summary.

---

**Sources.** `docs/VISION.md` § *Why this matters more with agents than without*; `README.md`
§ *Design decisions worth knowing*; `protocols/aep/1.yaml` (`approval_floor`); `AGENTS.md`
invariant 5 (`Unknown` is not `False`, enforced by the `Truth` type having no `From<bool>` and no
`as_bool`).
