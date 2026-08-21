---
format: aep.planning-md/1
id: story:time-based-transitions
kind: story
status: draft
title: A transition the clock can trigger
summary: 'The mechanism under horizons, expiry, staleness and SLAs: time as something the protocol can see, instead of scripts it cannot.'
owner: protocol
tags:
- adoption
- lifecycle
relations:
- decomposes: epic:adopter-feedback-round-1
- informed_by: story:evidence-horizons
revision: 2
---
# Story: A transition the clock can trigger

## Outcome

Time is something the protocol can see. Expiry, staleness, an SLA and an evidence horizon stop living
in scripts beside the tree — where they are invisible to `explain` and to every reader — and become
declarations the engine evaluates.

## Context

An early adopter's review, round 1 — **item D2**. The protocol has **no time-based transitions at
all**. Everything that ought to happen because a date passed is implemented outside it, which means
`explain` cannot name it, `evaluate` cannot account for it, and two stores with the same documents can
be in different states because one of them runs a cron job.

Three of this round's items sit on top of this one: D1's expiry (`story:decision-with-default`, a hard
`depends_on`), E1's due date (`story:external-clock-obligations`), and **C1's horizon**
(`story:evidence-horizons`), which is the overlap worth stating out loud. C1 is ranked first and is
allowed to land its own narrow clock read before this story exists; when this lands, the horizon
becomes an instance of the general mechanism rather than a second implementation of it. The failure
this ordering avoids is two clocks in one engine — the general design must be able to absorb the
narrow one, and the acceptance says so.

The engine's determinism invariant is the constraint that shapes the whole story: `aep-domain` is
clock-free and RNG-free, scanned for banned tokens by `crates/aep-domain/tests/determinism.rs`. So the
clock is **read at the edge and passed in**, never read in the domain — an evaluation is a pure
function of the documents, the facts and one supplied instant, and the same inputs give the same
answer a year later.

## Acceptance

- A transition can declare a time condition, and evaluating it takes the instant as an input; the
  banned-token scan over `aep-domain` still passes unchanged.
- The same documents and facts evaluated at two supplied instants give the two expected answers, and
  re-evaluating at the earlier instant gives the earlier answer again — no hidden state advanced.
- `explain` names a time condition as the reason a transition is or is not permitted, in the same
  words it uses for every other condition.
- The horizon mechanism from `story:evidence-horizons` is expressible in this construct, demonstrated
  by expressing it — not by asserting that it could be.

## Out of Scope

A scheduler. Nothing here runs at midnight, wakes up or sends anything; the protocol says what is true
at an instant it is given, and whatever asks the question — a CI job, a driver step, a person — brings
the instant with it.

## Open Questions

Whether time conditions may use wall-clock dates only, or also durations since a recorded event.
Decides: protocol owner. Default if nobody answers: **both, with the duration resolved against a
recorded timestamp in the store**, because a horizon is a duration and an obligation's due date is a
date, and this round needs both.
