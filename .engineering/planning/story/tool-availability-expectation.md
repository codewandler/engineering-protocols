---
format: aep.planning-md/1
id: story:tool-availability-expectation
kind: story
status: proposed
title: 'env.tool_available: the 50th expectation kind'
summary: A trace specification can assert which tools a session was offered, so the per-state allowlist has something that audits it.
owner: trace
tags:
- driver
- trace
relations:
- decomposes: epic:reference-driver
revision: 2
---
# Story: `env.tool_available` — the 50th expectation kind

## Outcome

Someone reading a run can tell which tools the session was offered, and a specification can require
that a step held only the tools its state permits — so the per-state allowlist is audited rather than
asserted.

## Context

The per-state tool set is the driver's primary enforcement mechanism, and the standard the design
sets for itself is that *an enforcement mechanism nobody audits is a claim*. `SessionStart.tools` is
already in the IR; what is missing is a kind that can read it. It ships **first** in the driver
sequence for exactly that reason: shipping the allowlist before the thing that audits it would meet
the letter of the design and not its standard.

## Acceptance

- A specification asserting a tool was offered passes against a transcript whose `SessionStart` lists
  it, and gaps against one that does not.
- A specification asserting a tool was **not** offered gaps when the session was given it.
- A transcript whose adapter could not read the offered tool list yields `unk`, not a pass.
- The existing drift test that asserts the raw and validated vocabularies agree catches a half-done
  job — a variant added without its name arm fails.

## Out of Scope

Asserting that a tool was *used*; tool traffic already has kinds. This one is about what the session
was offered, which is the only thing that can audit an allowlist.

## Open Questions

None. The kind mirrors `env.skill_available` line for line, and the shape is settled.
