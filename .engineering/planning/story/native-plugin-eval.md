---
format: aep.planning-md/1
id: story:native-plugin-eval
kind: story
status: draft
title: Move the plugin eval onto claude plugin eval
summary: Retire the hand-rolled runner for the harness's own eval surface once it is out of early access, keeping the trace specification as the thing that judges.
owner: eval
tags:
- eval
- plugin
relations:
- decomposes: epic:self-evaluation
revision: 1
---
# Story: Move the plugin eval onto `claude plugin eval`

## Outcome

The plugin is evaluated by the harness's own eval surface, so a person who knows that surface can read
and extend this repository's cases without learning a bespoke shell runner first.

## Context

`integrations/claude-code/eval/run.sh` exists because the native surface was gated at the time. It
works, and it is a hand-rolled runner for a job the harness now has a verb for. The part that must not
move is what judges: the trace specification is the thing that decides, and the runner is only how the
run is started — swapping the runner must not turn any bound into a shell assertion again.

## Acceptance

- The cases run under the harness's own eval verb, with the same expectations file judging them.
- Its report is consumed by the same path that produces `trace_conformance`, so evidence is minted the
  same way regardless of runner.
- The shell runner is removed rather than left beside the new one — two runners is two definitions of
  what the eval is.
- The install path documents the new invocation, and a reader without early access is told plainly
  that is why they cannot run it.

## Out of Scope

Changing any expectation. This is a runner swap; a bound that moves in the same change hides which of
the two caused it.

## Open Questions

Whether the native surface is out of early access yet. Decides: nobody here — this story stays in
draft until it is, and that is the whole reason it is not sequenced.
