---
format: aep.planning-md/1
id: story:driver-router
kind: story
status: active
title: 'aep-driver: the three-valued router'
summary: The pure half of the driver — the router, the LlmStepExecutor seam and tool_config over CapabilityPolicy::decide — with no clock, no network and no randomness.
owner: driver
tags:
- driver
relations:
- decomposes: epic:reference-driver
- depends_on: story:driver-spec-crate
revision: 3
---
# Story: `aep-driver` — the three-valued router

## Outcome

Given a state, a step map and what the engine said, one function says what happens next — and it says
it the same way every time, from the same inputs, with nothing ambient in the answer.

## Context

This is the half of the driver that decides *nothing about the protocol* and everything about
sequencing: which step is next, whether the step's verdict was true, false or unknown, and which
tools that step may hold. `tool_config` derives the tool set from `CapabilityPolicy::decide` rather
than from `allow` alone — `allow`, `approval_required` and `deny` are three independent sets and a
capability can legitimately be in all three. The purity claim here is stronger than `aep-engine`'s,
so it is held by a test rather than by a sentence.

## Acceptance

- `crates/aep-driver/tests/determinism.rs` ships with the crate and refuses a clock read, a random
  source and an ambient environment read.
- An approval-gated capability never appears in a step's tool set, asserted against a fixture whose
  capability is in `allow`, `approval_required` and `deny` at once.
- No development profile grants `command.execute`, so an `llm` step holds no shell — asserted, not
  observed once.
- The router is handed a `LockState` and never probes for one; a source scan finds no process or
  filesystem access in the crate.
- No `Producer::Human` and no `Evidence::Approval` is constructible anywhere in the crate.

## Out of Scope

The lock file, the liveness probe and the run directory, which sit on the impure side of the line in
`protocol-cli`. Also out: any evidence an `llm` step could carry — a model-calling step has no field
to put it in, by type.

## Open Questions

Whether the executor seam stays a function selected by harness name or becomes a trait. Decides:
driver owner, **after** the second implementation exists — designing it before that is the mistake
the gap register exists to catch.
