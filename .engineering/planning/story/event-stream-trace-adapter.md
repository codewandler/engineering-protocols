---
format: aep.planning-md/1
id: story:event-stream-trace-adapter
kind: story
status: draft
title: A trace adapter for the metaharness event stream
relations:
- decomposes: epic:metaharness-migration
revision: 2
---
# Story: A trace adapter for the metaharness event stream

## Outcome

A driven run's transcript — now a `metaharness.event/1` stream — can be checked against a trace
expectation document, so the trace-spec join the migrated eval suspended
(`evals/engineering-protocols/run-driven.sh` § 3.4 in the metaharness repository) comes back.

## Context

Left open by `epic:metaharness-migration` wave 2: `protocol trace check` reads Claude stream-json,
and the driver's transcripts stopped being that format when every `llm` step moved onto the
metaharness seam. The event stream is richer, not poorer — `session.ended` carries the vendor's
`permission_denials` *and* the seam's own decision census in one record — so this is one adapter
for every harness metaharness ever drives, and the Claude stream-json adapter becomes the
recorded-fixture reader.

## Acceptance

- A `metaharness.event/1` file lifts into `trace-ir/1` with session start, tool calls, decisions,
  denials and usage populated; an absent field stays absent, never zero.
- The three documents under `conformance/trace/` check against a driven event stream without a
  specification change, or every changed expectation is named with its reason.
- The migrated eval's suspended § 3.4 turns back on.

## Out of Scope

- Removing the Claude stream-json adapter: the recorded fixtures under the migrated eval and
  `crates/trace-spec/tests/fixtures/` still read through it.
- `Engine::authorize` at decision time — tracked on the epic, lands with the first case where a
  decision would change engine state.

## Open Questions

- Whether the adapter lives beside the stream-json reader in `trace-domain` or behind
  `metaharness project` (its Q9). Decides: whoever picks this up, recorded here.
