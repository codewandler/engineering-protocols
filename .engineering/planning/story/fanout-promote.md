---
format: aep.planning-md/1
id: story:fanout-promote
kind: story
status: draft
title: A promotion is a set of targets, each with its own guard
summary: release/progressive's single promote step becomes fan-out state with per-target guards and a hold-back operation the executor cannot overwrite.
owner: protocol
tags:
- adoption
- workflow
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: A promotion is a set of targets, each with its own guard

## Outcome

A release that is live in nine places and held back in five is *readable as that*, and the hold-back
survives whatever the deployment tooling does next — because a guard whose state the executor can
overwrite is not a guard.

## Context

An early adopter's review, round 1 — **item F2**. `workflows/releases/progressive.yaml` models
`promote` as **one step** — *"Move the release to the whole of production"*
(`workflows/releases/progressive.yaml:48-57`), with `on_failure: rollback` requiring
`deployment.previous_revision.exists`. Real fleets are a set: theirs is **14 namespaces on three
clocks**, with live per-tenant hold-back.

The failure they hit is the one worth building against. The hold-back was implemented as a **revert**,
and a downstream `git push --force` silently undid it — the fleet promoted to a tenant that was
explicitly held back, and nothing recorded that anything had been overridden. Their conclusion is the
requirement: *a guard whose state the executor can overwrite is not a guard.*

What is missing, in their words: **fan-out state with a per-target guard, and a hold-back operation.**
A single `promote` state cannot say that nine targets are done, one failed and four are held — so
today the workflow either lies about the fleet or is not used for it.

## Acceptance

- `promote` carries per-target state, and a release with some targets promoted and some held is a
  legal, expressible state of the workflow rather than an aggregate that has to round to one answer.
- Each target's guard is evaluated for that target: one failing objective holds one target and does
  not stop the others, asserted on a fixture fleet.
- A hold-back is an **operation with a record** — who held what, and why — and it cannot be cleared as
  a side effect of a deployment action; clearing it is its own explicit move.
- Rollback semantics stay per-target: `deployment.previous_revision.exists` is asked of the target
  being rolled back, not of the fleet.

## Out of Scope

Talking to any deployment system. The workflow models the fan-out and the guards; who actually rolls
a namespace stays outside, exactly as it is today.

## Open Questions

Whether targets are enumerated in the workflow document or supplied per run. Decides: protocol owner.
Default if nobody answers: **supplied per run, validated against a declared shape** — a fleet list
baked into a published workflow is a document every adopter would have to fork.
