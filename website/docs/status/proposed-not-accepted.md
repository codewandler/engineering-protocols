---
title: Proposed, not accepted
sidebar_position: 3
description: Four designs proposed extending this. One has been built, one was absorbed into the thesis by a deliberate amendment, and two are still nobody's work order.
---

# Proposed, not accepted

A project that keeps design documents accumulates a hazard: the newest and longest file starts
reading like the plan. It is not, and the repository is explicit about it — a proposal is not a work
order, however long and however recent it is.

Four designs proposed extending this project. **Two have since been taken up** — one built, one
absorbed into the thesis by an amendment somebody had to argue for and write down. **Two have not**,
and they are the reason this page still exists: nothing in that directory should be mistaken for what
has been agreed.

| What it would add | Status |
|---|---|
| **Closed-loop execution and conformance** — the specification becomes an *oracle*: a verdict on an implementation, not only a projection of a model | **built.** Delivered as ESS wave 4, `0.4.0-ess-wave-4`. Its four open decisions were each taken at the default the design named |
| **Semantic diff, impact and evolution** — the system changing over time, impact closure, what a revision invalidates | **core accepted**, as a stated amendment to the thesis, and sequenced as ESS wave 5. Not built. Two of its seventy-eight sections were rejected outright rather than deferred, being a different product |
| **Structural synthesis, obligations and realizations** — generated applications, and human or agent work carried as typed obligations | proposed, not accepted, not reconciled against the code. The review reads it as four waves rather than one |
| **Infrastructure discovery and multi-cloud realization** — a fourth domain, with infrastructure specified and checked beside the existing pair | proposed, not accepted, and **deferred whole** — roughly eleven waves at this repository's measured rate. Two ideas were harvested from it |

## What "accepted" had to cost

The first and the third rows are horizons the thesis already implied: "specified once and compiled"
promises the tests and the skeleton. The tests half is now delivered — and it is worth being precise
about what that buys, because a design is not the same thing as the code that came out of it. Four of
that design's constructs turned out to be untestable as written, and the repository's account of the
wave says which four and how each was found.

The second row was **not** implied. "Specified once and compiled" describes a system in the present
tense and says nothing about one *changing*, so absorbing it meant amending the thesis rather than
extending a roadmap — and the amendment is written down, with the reason, in the vision itself. What
forced it was the oracle: conformance evidence is bound to the specification revision it attests, so
without a semantic delta a change to a comment in an unrelated domain sends every conformance
requirement back to owed. Correct, and blunt.

Infrastructure is still a second subject matter rather than a further projection of the first.
Absorbing it would be another amendment of the same kind, and nobody has made that argument.

Infrastructure carries a second problem: the design as written would put cloud discovery adapters
inside the workspace, making live API calls under a credential. That is the one thing this project
[says it does not do](../deliberately-not.md), so the fix is to move the design rather than the
boundary.

## What is actually sequenced

| Wave | Goal | State |
|---|---|---|
| ESS wave 4 | a generated conformance suite, and proof that it bites — checked against an implementation that is deliberately wrong | **delivered.** Twelve deliberately wrong implementations, eleven of them caught by the named scenario that exists to catch them; the twelfth recorded as caught by nothing, with the reason |
| ESS wave 5 | two compiled specifications, and a typed answer to what moved between them | not started. It moved ahead of code generation by decision, because wave 4 left everything derived from a single revision with no way to say what a change invalidates |
| after that | a generated invoice service and email service that compile, and that pass the suite wave 4 generated | not started. It was gated behind wave 4 closing its loop in both directions — a correct target producing evidence that lets a task complete, and a faulty one producing a failure that refuses it. That gate is now open |

The ordering rule underneath all of them: **each wave must be falsifiable by the one before it.** A
generated artifact nothing can check is a claim, not a deliverable. Generating code judged by an
oracle nobody has seen fail is the exact mistake that ordering exists to prevent — and the oracle has
now been seen to fail, on purpose, once per fault.

---

**Sources.** `docs/VISION.md` § *Proposed, not accepted*; `AGENTS.md` § *Which documents are
normative*; `docs/plan/ess-roadmap.md` (the wave sequence, the ordering rule, and the review outcomes
for the three remaining proposals); `docs/plan/ess-wave-4-the-oracle.md` for what the built one
delivered and what it left open; `docs/reviews/` for the reviews the status column refers to.
