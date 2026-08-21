---
format: aep.planning-md/1
id: story:agent-eval-cases
kind: story
status: draft
title: The two planning agents, held to their charters by a run
summary: Eval cases that assert the decomposer moved nothing and the plan-reviewer changed nothing, from the scratch store and the working tree rather than from reading their definitions.
owner: eval
tags:
- eval
- plugin
relations:
- decomposes: epic:self-evaluation
revision: 1
---
# Story: The two planning agents, held to their charters by a run

## Outcome

Somebody editing the `decomposer` or the `plan-reviewer` finds out from a red check that they widened
what the agent may do — instead of from a store that quietly grew statuses nobody moved on purpose.

## Context

Both agents ship with a charter that is also a bound: the decomposer produces only draft stories, each
linked to its epic, and moves nothing; the reviewer changes zero files. Both statements are held today
by the definitions being carefully written. Neither is asserted by anything, and both are exactly the
kind of claim that survives an edit that breaks it.

## Acceptance

- A decomposer run against an epic leaves a scratch store in which every created artifact is a `story`
  in `draft`, each carrying a `decomposes` edge to that epic, and no other artifact's status changed.
- The store the decomposer leaves passes `protocol artifact validate`.
- A `plan-reviewer` run against the same store leaves `git status` clean — asserted on the tree, not
  read from the agent's definition.
- Both assertions are expectations in the trace specification, so they are checked the same way every
  other bound is.

## Out of Scope

Judging the *quality* of the decomposition. Whether the stories are good is a person's call; whether
the agent stayed inside its charter is mechanical, and only the second one belongs in a gate.

## Open Questions

Whether the cases run against committed transcripts or need a live model. Decides: eval owner.
Default if nobody answers: committed transcripts for the bounds, one live run per release.
