---
format: aep.planning-md/1
id: story:blocker-relation
kind: story
status: draft
title: Parked on a credential does not look like actively worked
summary: A blocker typed by what clears it — decision, review, credential, third party, capacity, deploy — so five items on one decision is one conversation.
owner: protocol
tags:
- adoption
- lifecycle
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: Parked on a credential does not look like actively worked

## Outcome

Anybody reading the plan can see what is actually stopped, and on what. Five items waiting on one
decision show up as one conversation to have, rather than as five items somebody has to ask about
individually.

## Context

An early adopter's review, round 1 — **item D4**. Today a blocked item and a moving item are
indistinguishable in the store: `active` covers both *being worked on right now* and *parked for nine
days on a credential nobody has requested*. The blocked one is invisible precisely because nothing
about it changes.

Their proposal is to type a blocker by **what clears it** — decision, review, credential, third party,
capacity, deploy — because that is the field that turns a list of stuck items into an action. Five
items blocked on one decision is one meeting; five items blocked on five different things is five
conversations, and the current store cannot tell those two situations apart.

Their live case is the one that makes this a protocol concern rather than a label: a blocker joined to
an **evidence gate** — a CI evidence job blocked on a read-scope API token. The evidence a transition
needs cannot be produced, and the reason is a credential. That join is exactly what a protocol can
make legible and what a status field cannot.

The relation vocabulary already has `blocks`, with `source: [any]` and
`target: [story, task, epic, release-plan, migration-plan]` (`artifacts/relations/relations.yaml`), so
the edge exists and the **type** of the blocker does not.

## Acceptance

- A blocker carries a type drawn from a declared, open vocabulary, and an artifact blocked by one is
  distinguishable from an active artifact in `list` and `board` without opening the file.
- The store answers "what is blocked, by what type, and on which single item" — so several artifacts
  blocked by one thing appear as one group.
- A blocker joined to an evidence requirement is expressible: the reason a required fact does not
  exist is recorded as the blocker, and `explain` names it.
- Unblocking is a move like any other, leaving a record of when and by what — not an edit that erases
  the fact that anything was ever stuck.

## Out of Scope

Any notion of how long is too long. An age threshold on a blocker is an SLA, which is
`story:time-based-transitions`; this story records the fact and its type.

## Open Questions

Whether the blocker type vocabulary is open. Decides: protocol owner. Default if nobody answers:
**open, with the six reported types shipped as the documented starting set** — the meta-defect in this
same round is precisely this repository closing a vocabulary an adopter needed to extend.
