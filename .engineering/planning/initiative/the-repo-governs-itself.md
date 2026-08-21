---
format: aep.planning-md/1
id: initiative:the-repo-governs-itself
kind: initiative
status: draft
title: The repository runs on its own protocol
summary: 'The harness family: a driver that walks this repository''s own workflows, a planning store that answers as a backend, and completion that carries evidence.'
owner: protocol
tags:
- dogfooding
- harness
revision: 1
---
# Initiative: The repository runs on its own protocol

## Outcome

Work on this repository is planned, driven and closed by the protocol this repository publishes. A
story moves because the engine admitted evidence for it. An agent's run is judged by a typed
specification rather than read afterwards. The workflow it followed is one a driver walked, not one
a prompt asked for politely. Anyone can point at the mechanism behind each of those three sentences.

## Why Now

Three claims in this repository currently have no implementation behind them, and each is the exact
shape of defect `AGENTS.md` § *Invariants* says has already drifted: a harness contract of seven
calls that no program in the workspace makes in order; four development commands
(`adp.story.start/v1` and its three siblings) that no crate depends on; and six planning kinds whose
lifecycles nothing had ever moved an artifact through until this store existed. Meanwhile the waves
are planned in hand-written markdown that nothing validates — the protocol's own methodology not
applied to the protocol's own backlog, which is the least defensible gap on the list.

## Scope

The harness family. The reference driver and the documents it walks; the planning store's road from
a plain durable store to a contract implementation with a journal; the evidence that gates a story
reaching `implemented`; the transcript checker that says what a run did; and the second harness that
turns *harness-neutral* from a sentence into a gate. Six epics decompose it.

## Out of Scope

The ESS family and the infrastructure family, which have their own roadmaps and their own wave pages
(`docs/plan/ess-roadmap.md`, `docs/plan/infra-wave-*.md`). Attested evidence stays where gap-register
**D-3** left it: proposed, not accepted, and nothing here assumes it.

## Done When

`protocol drive` has walked a real task in this repository end to end under `adp/default`; the
transcripts of that run were checked and submitted as `trace_conformance`; the planning store passed
the sixteen `aep-conformance` suites; and no story in this store reached `implemented` without a
record the engine admitted.
