---
format: aep.planning-md/1
id: story:shell-echo-harness
kind: story
status: proposed
title: A second harness with no model, no network and no credential
summary: A shell-echo LlmStepExecutor and a reader for its own transcript dialect, so harness-neutrality becomes a gate inside task check instead of a sentence.
owner: trace
tags:
- harness
- trace
relations:
- decomposes: epic:cross-harness-portability
- depends_on: story:driver-router
revision: 2
---
# Story: A second harness with no model, no network and no credential

## Outcome

Anyone can run `task check` and watch the neutrality claim be tested — and watch it go red when
somebody breaks the seam for a harness that is not Claude Code.

## Context

Today one adapter exists and *harness-neutral* is a property nothing has ever tested. A second
**real** harness would test it and would also need credentials, a network and a bill, which is why
the acceptance for the seam is a **fake** one: a shell script that reads a prompt on stdin, writes a
fixed set of files, and emits a transcript in a dialect of its own. It proves all three adapter
points at once — two executor implementations, one `tool_config` consumed by both, and a
`trace_conformance` record minted from a transcript no Claude Code wrote.

## Acceptance

- The shell-echo executor and its transcript reader run inside `task check`, with no model, no
  network and no credential.
- The same step map, the same workflow, the same `tool_config` function and the same checker drive
  both harnesses.
- The reader returns `TraceIr` with its own `AdapterRef`, and `check` plus `to_evidence` mint a
  record from it that `protocol evaluate --evidence` accepts.
- Breaking the executor seam for either harness fails the gate, naming which one.

## Out of Scope

Making the fake harness realistic. It is a seam test, not a simulator; anything it does beyond
exercising the three points is surface a real harness will contradict.

## Open Questions

None blocking. Whether the fake harness's dialect should be documented for outside use is decided
against for now: a dialect nobody outside this repository writes is not a format.
