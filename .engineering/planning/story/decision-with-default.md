---
format: aep.planning-md/1
id: story:decision-with-default
kind: story
status: draft
title: A decision the calendar answers anyway is a recorded event, not a silence
summary: A decision construct beside approval, with a required default and expiry, so a judgement call that nobody answers is provenance rather than a stalled item.
owner: protocol
tags:
- adoption
- lifecycle
relations:
- decomposes: epic:adopter-feedback-round-1
- depends_on: story:time-based-transitions
revision: 2
---
# Story: A decision the calendar answers anyway is a recorded event, not a silence

## Outcome

A judgement call that nobody answers produces a **recorded default with provenance** on its expiry
date, instead of an item that sits open forever while the world moves on without it.

## Context

An early adopter's review, round 1 — **item D1**. Approvals block, which is
right for capabilities and wrong for judgement calls — the calendar answers those anyway, and nothing
is recorded when it does. Their data is the argument: of **155 stateful items only 41% ever
transitioned**, while `decision` — the one class with a keystroke answer loop — runs **15/15**.

The proposal is a `decision` construct beside `approval`, with a required `default` and a required
`expires`. The point is not automation; it is that a defaulted decision is an *event with provenance*
— it says what was decided, that it was decided by expiry rather than by a person, and when. A silence
records nothing and is indistinguishable from an item nobody has looked at.

D2 (`story:time-based-transitions`) is the mechanism this needs and is a `depends_on`: an expiry that
nothing can fire is a field, not a decision.

## Acceptance

- A `decision` construct exists beside `approval`, and a declaration missing `default` or `expires` is
  refused at validation.
- Reaching the expiry with no answer records the default as an event naming the default, the expiry
  and that no person answered — distinguishable in the record from the same value chosen by a person.
- A `decision` does not block the way an `approval` does: the transition it governs proceeds on the
  defaulted value, asserted rather than described.
- The two constructs are documented side by side with the rule for choosing — capability versus
  judgement call — because the failure mode is an adopter reaching for the blocking one out of habit.

## Out of Scope

Reminders, escalation and anything that tries to get a human to answer before the expiry. That is
§ E1's escalation field and belongs to `story:external-clock-obligations`.

## Open Questions

Whether a defaulted decision can be re-opened after expiry. Decides: protocol owner. Default if nobody
answers: **no — it is superseded by a new decision**, so the record of what was actually in force
during that window survives, which is the entire reason for recording the default.
