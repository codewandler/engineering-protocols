---
format: aep.planning-md/1
id: epic:adopter-feedback-round-1
kind: epic
status: draft
title: The first tree that is not ours, and what it found
summary: 'Round 1 of adopter defects: three bugs, the open-vocabulary meta-class, the evidence model''s missing time and subject, four lifecycle concepts, external-clock obligations, and the advisory tier.'
owner: protocol
tags:
- adoption
- protocol
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: The first tree that is not ours, and what it found

## Outcome

The defects a first adopter found by *writing* a document tree against this spec are in the plan
with honest names, in the order their own evidence ranks them — not summarised into a page nobody
opens. Anyone can point at a story for each one, and at the reason it sits where it sits.

## Why Now

On 2026-08-21 an early adopter wrote the first document tree against this specification that is not
this repository's own — a protocol extending `aep/1`, with 4 workflows, 6 principles, 4 profiles, 4
lifecycles across 26 files — and it **validates**: 22 files valid, and `resolve`, `explain` and
`evaluate` all work. That is the first evidence this specification is adoptable by somebody who did
not write it, and the first evidence of what it costs. Every defect below reproduces against that
tree, and each carries a real incident or a counted corpus behind it rather than a preference. The
window in which an adopter still writes reports is short. Their written review is held by the
operator and is deliberately not in this tree, so the stories carry the evidence rather than a link.

## Scope

The report's seven clusters, one story per honest unit of work rather than one per letter: the three
unambiguous bugs (§ A, plus B2 and G2 riding with them); the open-vocabulary meta-class (§ B); the
evidence model's missing time and subject (§ C1, C2); the four lifecycle concepts (§ D); obligations
with an external clock (§ E); the enforcement tier (§ F); and the smaller items (§ G). The order the
stories are worked is the report's own ranking by evidence density, not ours.

## Out of Scope

C3, C4 and C5 — the environment revision, the determinism model and a verifier's own coverage. They
are real and they are recorded in the review; nothing here claims them, and a later round names the
story that takes them. Also out: adopting the adopter's own protocol, or
vendoring any of their documents into this tree. Their tree is evidence, not a dependency.

## Risks

The report is one adopter, and *one* is exactly the sample size that makes a fix look general when it
is a shape. The mitigation is in the evidence each item carries: 145 dated claims, 155 stateful items
at 41% transition rate, 425 obligations, 19 checkers of which 4 are advisory — the items with counts
behind them are the ones to build first, which is why the order is theirs. Second risk: three of these
widen a closed vocabulary (`ArtifactStatus`, the enforcement level, the evidence record), and a
vocabulary widened by one adopter's shape is a vocabulary widened badly. Each such story owes a
statement of what stays closed and why.

## Done When

Every row of the review's ranked fix order has either landed as code, or has a recorded decision
saying it will not — and the tree the adopter wrote still validates against the tree this repository
publishes, checked rather than assumed.
