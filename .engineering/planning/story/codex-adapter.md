---
format: aep.planning-md/1
id: story:codex-adapter
kind: story
status: draft
title: 'A real second harness: the Codex adapter'
summary: A third implementation of the executor and reader seam against a harness nobody here controls, and the trait decision the second one deliberately postponed.
owner: trace
tags:
- harness
- trace
relations:
- decomposes: epic:cross-harness-portability
- depends_on: story:shell-echo-harness
revision: 1
---
# Story: A real second harness — the Codex adapter

## Outcome

Somebody who runs Codex rather than Claude Code can drive this repository's workflows and have their
run judged by the same specification, and the neutrality claim is finally tested against a format
nobody here controls.

## Context

The fake harness proves the seam exists; it cannot prove the seam is in the right place, because its
dialect was written by the same person who wrote its reader. A real third implementation is what
turns the deliberately-postponed executor trait from a symmetry argument into a decision made with
evidence: if selecting the reader by harness name is awkward across three implementations, the trait
is a one-file change made for a reason.

## Acceptance

- A Codex run of the same step map produces a `TraceIr` that the existing specification checks
  without a specification change.
- A field this harness does not carry yields `unk` and exit 3, not a pass and not a failure.
- The executor seam is either kept as a function or becomes a trait, with the reason recorded either
  way.
- The adapter declares its own `AdapterRef` and version, and an adapter upgrade that starts
  understanding a field does not silently rename the run.

## Out of Scope

Feature parity between harnesses. Where one offers something the other does not, the specification
says `unk` — that is what the third verdict is for, and papering over it would make the checker lie.

## Open Questions

Which Codex output mode is stable enough to adapt. Decides: trace owner, from the vendor's own
documentation, before any code — the same rule that made the first adapter declare its format.
