---
format: aep.planning-md/1
id: story:transcript-diff
kind: story
status: draft
title: 'protocol trace diff: two runs of one specification, compared'
summary: Two transcripts checked against one specification, reported as what each did differently, so a harness swap is judged by behaviour rather than by both exit codes being zero.
owner: trace
tags:
- trace
relations:
- decomposes: epic:cross-harness-portability
revision: 1
---
# Story: `protocol trace diff` — two runs of one specification, compared

## Outcome

Someone changing a model, a prompt or a harness can see what actually changed in the behaviour —
which expectations moved, which tools were used differently, where the cost went — instead of
observing that both runs exited zero and hoping that means the same thing.

## Context

Two runs both passing is the least informative true statement the checker can make. The data to do
better is already retained: every verdict cites the transcript event indices behind it, and
`protocol trace inspect` already prints the census — event families, per-tool traffic in both
directions, each step's `gen`/`exec` split. What is missing is the comparison, and it is what makes a
harness swap or a model change reviewable rather than merely green.

## Acceptance

- Two transcripts checked against one specification produce one report of differences: verdicts that
  moved in either direction, and census figures that differ beyond a stated bound.
- A verdict that moved from `ok` to `gap` is reported separately from one that moved to `unk` — a
  regression and a blind spot are different findings.
- The report names the specification digest once and refuses two transcripts checked against
  different specifications.
- No score and no percentage: differences are listed, never aggregated into a number.

## Out of Scope

Deciding which run is better. The diff reports; a person or a specification judges.

## Open Questions

Whether cost and token differences need a tolerance to be useful, given model routing drift. Decides:
trace owner. Default if nobody answers: report the absolute difference and let the reader decide,
because a tolerance chosen here is a bound nobody stated.
