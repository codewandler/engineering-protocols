---
title: Proposed, not accepted
sidebar_position: 3
description: Four designs propose extending this. None is part of what the project has agreed to build, and two of them are not even the same subject.
---

# Proposed, not accepted

A project that keeps design documents accumulates a hazard: the newest and longest file starts
reading like the plan. It is not, and the repository is explicit about it — a proposal is not a work
order, however long and however recent it is.

Four designs currently propose extending this project. **None of them has been accepted.** They are
listed here so that nothing on this site, and nothing in that directory, can be mistaken for what has
been agreed.

| What it would add | Status |
|---|---|
| **Closed-loop execution and conformance** — the specification becomes an *oracle*: a verdict on an implementation, not only a projection of a model | reviewed, reconciled against the code and frozen for implementation, except four named open decisions. Sequenced first, as ESS wave 4 |
| **Semantic diff, impact and evolution** — a third axis: the system changing over time, impact closure, what a revision invalidates | reviewed; sequenced after wave 4 by decision. It is several waves of work, not one |
| **Structural synthesis, obligations and realizations** — generated applications, and human or agent work carried as typed obligations | proposed; reviewed once and not reconciled. Unsequenced |
| **Infrastructure discovery and multi-cloud realization** — a fourth domain, with infrastructure specified and checked beside the existing pair | proposed, unreviewed. Unsequenced |

## Why the last two are a different question

The first two are horizons the thesis already implies: "specified once and compiled" already promises
the tests and the skeleton.

The other two are not. "Specified once and compiled" says nothing about a system *changing*, and
infrastructure is a second subject matter rather than a further projection of the first. Absorbing
either into the thesis is a decision someone has to take deliberately, with a reason — and until
somebody does, they stay on this page rather than in the description of what this project is.

Infrastructure carries a second problem: the design as written would put cloud discovery adapters
inside the workspace, making live API calls under a credential. That is the one thing this project
[says it does not do](../deliberately-not.md), so the fix is to move the design rather than the
boundary.

## What is actually sequenced

| Wave | Goal | State |
|---|---|---|
| ESS wave 4 | a generated conformance suite, and proof that it bites — checked against an implementation that is deliberately wrong | not started. A reconciliation pass over the model was sequenced in front of it, because several of its items are model changes that are cheaper before a synthesizer is built around their absence |
| ESS wave 5 | a generated invoice service and email service that compile, and that pass the suite wave 4 generated | gated behind wave 4 closing its loop in both directions: a correct target producing evidence that lets a task complete, and a faulty one producing a failure that refuses it |

The ordering rule underneath both: **each wave must be falsifiable by the one before it.** A
generated artifact nothing can check is a claim, not a deliverable. Generating code judged by an
oracle nobody has seen fail is the exact mistake that ordering exists to prevent.

---

**Sources.** `docs/VISION.md` § *Proposed, not accepted*; `AGENTS.md` § *Which documents are
normative*; `docs/plan/ess-roadmap.md` (waves 4 and 5, the ordering rule, and the review outcomes for
the two later proposals).
