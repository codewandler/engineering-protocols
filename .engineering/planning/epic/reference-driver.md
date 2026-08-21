---
format: aep.planning-md/1
id: epic:reference-driver
kind: epic
status: draft
title: The reference driver
summary: 'A specified workflow that runs strictly: aep-driver-spec, aep-driver, protocol drive, the step map, and the hooks that enforce the per-state tool set.'
owner: driver
tags:
- driver
- harness
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: The reference driver

## Outcome

An operator types `protocol drive` and a specified workflow runs strictly: at each state the tools
that exist are the ones that state permits, the engine decides every transition, and the run either
reaches a state the protocol calls complete or stops with the engine's own words for why. The
difference from today is not that the agent is told to write the test first — it is that during that
step it cannot do anything else.

## Why Now

`docs/guide/harness.md` publishes a contract of seven calls and three rules, and nothing in this
workspace implements it. A published contract with zero implementations is a shape nobody has been
forced to fit, which is the same defect as an invariant nothing enforces. Wave 2 closed the six
architectural holes in the design § 4 and a feasibility review judged them against the code —
23 confirmed, 14 needs-change, 3 infeasible, all applied. The decisions are taken; what is missing is
the crate.

## Scope

`aep-driver-spec` and `aep-driver`, the first step map under `drivers/`, `protocol drive` with its
run directory and store lock, and the plugin hooks that hold the per-state tool set from the other
side. The sequence is W3.0–W3.4 of
[`docs/plan/harness-wave-2-driver-decision.md`](../../../docs/plan/harness-wave-2-driver-decision.md);
the retry and lock-UX stories are the hardening this epic does not finish without.

## Out of Scope

Anything that decides. Gates are evaluated by the engine and never by the driver, and an `llm` step
has no field to put evidence in — a purity claim held by a type rather than a rule. Also out: a
second real harness (`story:codex-adapter`), and any narrowing of what a model may write, which is a
boundary the design states rather than an omission.

## Risks

The trust model for plugin-supplied hooks is undocumented — if an installed plugin's hooks need a
per-invocation consent step, the hook layer degrades to advisory and `--allowedTools` carries
enforcement alone. It is named as an assumption in the design rather than assumed silently. The
second risk is the router: `aep-driver` claims a purity stronger than `aep-engine`'s, and a liveness
probe or a clock read slipping into it would be invisible to a banned-token scan.

## Done When

A real task in this repository is driven end to end, its transcripts pass `protocol trace check`,
and the resulting `trace_conformance` record is admitted by the engine — with the same step map, the
same workflow and the same `tool_config` function also driving a harness that is not Claude Code.
