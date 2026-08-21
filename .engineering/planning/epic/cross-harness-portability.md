---
format: aep.planning-md/1
id: epic:cross-harness-portability
kind: epic
status: draft
title: Harness-neutral, and tested by a second harness
summary: A fake harness inside task check, a real second adapter, and a way to compare two runs of one specification.
owner: trace
tags:
- harness
- trace
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: Harness-neutral, and tested by a second harness

## Outcome

Someone who does not run Claude Code can run this repository's workflows, and someone who does can
prove that the neutrality claim is true rather than repeated. The same step map, the same workflow,
the same `tool_config` function and the same checker drive a second harness, and two runs of one
specification can be compared as behaviour rather than as two exit codes that both happened to be
zero.

## Why Now

Exactly one adapter exists — Claude Code `stream-json` — and *harness-neutral* is a property nothing
has ever tested. `docs/plan/trace-wave-1-transcript-checker.md` states this on the way in rather than
assuming it: *"a second harness is a second adapter and not a second specification language, and
until there is one the claim is untested."* A claim tested by one implementation is the shape of
defect this repository writes registers about.

## Scope

The fake harness first, then the real one. A shell-echo executor with a transcript dialect of its own
proves the three adapter points with no model, no network and no credential, which is what lets it be
a step of `task check` instead of a paid run. The Codex adapter follows as the third implementation,
and it is the point at which the deliberately-postponed executor trait gets designed with evidence
rather than for symmetry. `protocol trace diff` is what makes a harness swap reviewable.

## Out of Scope

A second *specification* language. The trace specification is harness-neutral by construction; what
varies is the reader that produces `TraceIr`. Also out: making a real second harness a prerequisite
for the driver — W3.5's fake one is what tests the seam, and a real one replaces it as a third
implementation rather than as the first test.

## Risks

The fake harness can pass while proving nothing, if its dialect is shaped around the reader that will
read it. The mitigation is that it must exercise the same `check` and `to_evidence` path and mint a
`trace_conformance` record no Claude Code wrote. For the Codex adapter the risk is the opposite: a
format nobody here controls can move under the adapter, which is what `unk` and exit 3 exist for.

## Done When

`task check` goes red when the executor seam is broken for a harness that is not Claude Code, and two
transcripts from two harnesses can be checked against one specification and their differences read
from one report.
