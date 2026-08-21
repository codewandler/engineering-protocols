---
format: aep.planning-md/1
id: epic:self-evaluation
kind: epic
status: draft
title: The repository evaluates its own agents
summary: The driven eval, the two planning agents' cases, and the native plugin eval runner once it stops being gated.
owner: eval
tags:
- eval
- harness
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: The repository evaluates its own agents

## Outcome

The agents and the plugin this repository ships are held to their charters by runs rather than by
their definitions being well written. A change that quietly lets the decomposer move a status, or the
reviewer touch a file, turns a check red instead of being noticed by a reader.

## Why Now

Two agents ship today with charters that are also their bounds — *the decomposer produces only draft
stories and moves nothing*, *the reviewer changes zero files* — and both statements are currently
held by prose. The eval that exists needs a `claude` binary, credentials and a network, so it cannot
be a step of `task check`, and the paid surface is exactly where an unrun check rots quietly.

## Scope

The driven eval that acceptance-tests the whole loop, including a denial triggered on purpose; the
two agent cases asserted from the scratch store and the working tree; and moving the runner onto the
harness's own eval surface when that stops being gated. What judges every one of them is a trace
specification, not a shell pipeline — five assertions in three shell idioms already became
forty-one expectations with the observed value beside every bound.

## Out of Scope

Asking a model whether the agent behaved reasonably. This is the single most tempting place in the
repository to break that rule and it stays refused: it would make every verdict unreproducible and
unfalsifiable at once, and the protocol would classify anything it said as `Producer::Agent` and
refuse it as independent evidence anyway. Also out: any score, percentage or leaderboard.

## Risks

Cost and flakiness. A paid eval that runs on every push is a bill and a false red; one that never runs
is a check nobody trusts. The split is deliberate — bounds are checked against committed transcripts
by the ordinary gate, and the paid run is separate, named, and honest about needing credentials.

## Done When

`task check` fails when either agent violates its charter without a paid run, the driven eval has
been run at least once against a real model, and `permission.denied` audits an observed denial rather
than reporting an ambiguous zero.
