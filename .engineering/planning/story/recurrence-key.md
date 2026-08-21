---
format: aep.planning-md/1
id: story:recurrence-key
kind: story
status: draft
title: Two incidents with one root-cause shape, countable
summary: A cross-incident recurrence key on incident/standard, so a rollup over root-cause shapes exists without a hand-written index.
owner: protocol
tags:
- adoption
- workflow
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: Two incidents with one root-cause shape, countable

## Outcome

The second time the same shape of failure happens, the store says so. A rollup over root-cause shapes
exists without anybody maintaining an index by hand.

## Context

An early adopter's review, round 1 — **item G1**, filed by them under *smaller* and marked **cheap,
high value**. `workflows/incidents/standard.yaml` ends at `learn` on purpose — its own header says an
incident workflow that ends at `recover` produces the same incident again — and then gives `learn`
nothing to write the recurrence into. Each incident's lessons land in its own document, and nothing
joins two incidents that failed the same way.

The adopter runs the missing piece by hand: root-cause shape tags, **181 lines across 21 incidents**,
with a working rollup on top. That is the evidence for *cheap*: the thing being asked for is a key and
a rollup over it, not an analysis engine.

G2 — the untyped failure policy, the report's other *smaller* item — is **not** here: it rides
`story:adopter-bugs`, because it is a one-line validation fix with no design question, and splitting
it out would have made a story that is finished before it is read.

The value is the count. One incident with a shape is an anecdote; the same shape three times is a
decision to make, and it is invisible today unless the same person happens to remember both.

## Acceptance

- `incident/standard` carries a cross-incident recurrence key, declared where the workflow's other
  outputs are declared, and `learn` is the state that owes it.
- Two incidents sharing a key are reported together, with a count, by the store rather than by a
  hand-written page.
- The key vocabulary is open — an adopter names their own root-cause shapes without a code change.
- An incident that reaches `learn` without a key is visible as owing one, on the same principle that a
  refusal is printed beside the scenarios that exist.

## Out of Scope

Inferring the shape. Nothing here clusters incidents, suggests a key or reads a postmortem; a person
names the shape and the protocol counts.

## Open Questions

Whether the key is a single value or a set of tags per incident. Decides: protocol owner. Default if
nobody answers: **a set** — the adopter's working version is tags, and an incident with two causes is
the normal case rather than the exception.
