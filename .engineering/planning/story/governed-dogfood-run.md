---
format: aep.planning-md/1
id: story:governed-dogfood-run
kind: story
status: draft
title: One story from this backlog, driven end to end
summary: protocol drive over the default step map, closing a real story of this repository's own plan, with every transition evidence-permitted and every status move made by the verb.
owner: driver
tags:
- dogfooding
- driver
relations:
- decomposes: epic:reference-driver
- depends_on: story:own-engineering-store
- depends_on: story:protocol-drive-verb
- depends_on: story:driven-eval-acceptance
revision: 1
---
# Story: One story from this backlog, driven end to end

## Outcome

A story in this store is closed by a run rather than by a person deciding it was finished — and a
second person, who was not there, can reconstruct what happened from the run directory without asking
anyone.

## Context

This is the centre of the dogfood: not a task invented for the driver, but a real item of this
repository's own plan, walked by `protocol drive` over the default step map under
`development.standard`. One session per `llm` step; `cargo test` and `clippy` executed **by the
driver** as `command` steps, because no development profile grants `command.execute` and the model
therefore holds no shell at any point; and the review as an `operator` step that persists, releases
the lock and exits 0.

## Acceptance

- No state is entered except through a transition the engine returned as `Moved`, and every gate the
  driver wanted and the engine refused appears with one reason per unmet requirement — asserted
  against the snapshot's audit trail, not against the driver's own log.
- The project gate is green before the review step, and the `test_result` and `static_analysis`
  records submitted are the ones that run produced, not a summary a model wrote about it.
- Each `llm` step's transcript is checked and submitted as `trace_conformance`, and the completion
  gate reads it — the run cannot complete without it.
- Every status move went through `protocol artifact move` and by no other means, asserted by
  inspecting the store afterwards, with the write-guard hook as enforcement and `validate` as audit.
- A run that wedges is a **recorded result**: where it stopped, what the cursor said, and which
  decision was wrong. Quietly retrying until it works does not close this.

## Out of Scope

Byte-repeatability. Two runs of one story produce different transcripts and different digests, and a
resumed run is a new session by design. Every assertion here is over the store, the audit trail and
the gate's exit code — never over the model's prose.

## Open Questions

Which story goes first. Decides: driver owner. Default if nobody answers: one whose acceptance is
already mechanical and whose blast radius is one crate, because the point of the first run is the
loop, not the difficulty.
