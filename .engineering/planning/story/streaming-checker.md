---
format: aep.planning-md/1
id: story:streaming-checker
kind: story
status: draft
title: Checking a run while it is still running
summary: Incremental evaluation, partial verdicts and a halt signal, so a run that has already violated its specification stops costing money.
owner: trace
tags:
- trace
relations:
- decomposes: epic:checker-vocabulary-depth
revision: 1
---
# Story: Checking a run while it is still running

## Outcome

A run that has already violated its specification stops there, instead of finishing, costing what it
costs, and being judged afterwards by somebody reading a report.

## Context

Deferred by name as **D5**: batch only. The reason was not effort — incremental evaluation, partial
verdicts and a halt signal are not designable against a format that is not stable, and the transcript
format's stability is the thing the first adapter deliberately declined to assume. That makes this
story blocked on evidence rather than on scheduling, and the evidence comes from having watched a
second adapter and a real format change.

## Acceptance

- A specification can be evaluated over a prefix of a transcript, producing partial verdicts that
  never contradict the batch verdict over the same prefix.
- An expectation that can no longer be satisfied by any continuation reports as failed at the event
  that made it so, with that event cited.
- An expectation whose outcome still depends on the rest of the run reports as pending, never as
  passing.
- A halt signal is a caller's decision, not the checker's: the checker reports, the driver stops.

## Out of Scope

Streaming as a *transport*. This is about evaluating incrementally, not about how bytes arrive.

## Open Questions

Whether the format is stable enough to attempt this. Decides: trace owner, on the evidence from the
second adapter. Default if nobody answers: stay batch-only and keep the row on the deferred table,
which is honest and free.
