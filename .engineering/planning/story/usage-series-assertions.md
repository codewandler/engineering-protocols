---
format: aep.planning-md/1
id: story:usage-series-assertions
kind: story
status: draft
title: Assertions over the per-request usage series
summary: A vocabulary for sequences — the cache-read ramp is monotone, cache creation is front-loaded, no request takes more than a share of the total — over data the IR already keeps.
owner: trace
tags:
- trace
relations:
- decomposes: epic:checker-vocabulary-depth
revision: 1
---
# Story: Assertions over the per-request usage series

## Outcome

An author can state how a run's usage should *move* — the cache-read ramp is monotone, cache creation
is front-loaded, no single request takes more than a share of the total — instead of only what it
totalled, which is what catches a context strategy that has quietly stopped working.

## Context

Deferred by the design itself, not by the wave running out of time. The data is already retained:
`TraceIr::requests` keeps every assistant event's usage. What is missing is a vocabulary for
*sequences*, which a single-field matcher does not have — and designing one under the previous wave's
deadline would have been designing it for a different feature.

## Acceptance

- A specification can assert a monotone trend over a named usage field across the request series, and
  gaps with the index of the first request that broke it.
- A specification can assert that no single request exceeds a stated share of a run's total.
- A run with a single request satisfies a trend assertion vacuously rather than gapping — an
  assertion about a sequence of one is not a failure.
- A field this transcript does not carry is `unk`, as everywhere else.

## Out of Scope

Statistics. No means, no percentiles, no smoothing — a trend the reader cannot recompute by looking at
the cited events is a verdict nobody can check.

## Open Questions

Whether the sequence vocabulary is shared with any future ordering assertions. Decides: trace owner,
when the second consumer exists — one consumer cannot justify a general mechanism.
