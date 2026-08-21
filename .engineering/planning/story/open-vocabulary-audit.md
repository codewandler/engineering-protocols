---
format: aep.planning-md/1
id: story:open-vocabulary-audit
kind: story
status: draft
title: Every adopter-facing declaration, checked for whether it is actually open
summary: 'The audit the meta-defect asks for: for each thing the docs invite an adopter to declare, is the vocabulary open, and is the closure deliberate and stated.'
owner: protocol
tags:
- adoption
- protocol
relations:
- decomposes: epic:adopter-feedback-round-1
- informed_by: story:outbound-claims-and-status-vocabulary
- informed_by: story:adopter-bugs
revision: 3
---
# Story: Every adopter-facing declaration, checked for whether it is actually open

## Outcome

For each thing the documentation invites an adopter to declare, the answer to *"can I put my own value
here?"* is written down — and where the answer is no, the closure is deliberate, stated, and has a
reason a reader can argue with.

## Context

An early adopter's review, round 1 — **item B**, the meta-defect: **things the
docs invite an adopter to declare keep turning out to be fixed in the engine.** Three instances in one
afternoon, all found by *writing* a tree rather than by reading the guide — `ArtifactStatus` closed
(B1, now `story:outbound-claims-and-status-vocabulary`), `PROJECT_DIRECTORY` a compile-time constant
(B2, riding `story:adopter-bugs`), and A2's kind ladder defined over built-in variants only. Their
closing line is the request this story answers: *for every adopter-facing declaration, check the
vocabulary is actually open.*

The report also says what constrains the audit, and it is the more useful half: phases, verifiers,
artifact kinds, capabilities and observables **were** open, and a domain with no compiler and no
deployment slotted in without touching the engine. `evidence_kinds` being closed is **correct** — it
is the seam whose semantics are guaranteed, and their knowledge work mapped onto the existing kinds
honestly. So the audit's output is not "open everything". It is a table with three columns: the
declaration, whether it is open, and — where closed — the guaranteed semantics that closure buys.

## Acceptance

- Every adopter-facing declaration surface in the published guides appears in one table with an
  open/closed verdict and, for the closed ones, the guarantee the closure buys.
- Each closed entry names where its reason is written down for adopters, not only in this table.
- Every closed entry found to have **no** guarantee behind it gets a story or a recorded decision that
  it stays closed; a closed vocabulary with no stated reason does not survive the audit unremarked.
- The audit is repeatable: it says how it was produced, so the next round is a diff and not a rewrite.

## Out of Scope

Opening any specific vocabulary. Each one that should open is its own story with its own migration
question; this story produces the verdict and the list.

## Open Questions

Whether the table lives in the guide or in `docs/reviews/`. Decides: protocol owner. Default if nobody
answers: **the guide**, because the audience is an adopter deciding what they may declare, and a
review page is where this repository talks to itself.
