---
format: aep.planning-md/1
id: story:skill-text-in-context
kind: story
status: draft
title: An expectation kind for the skill's text entering context
summary: The synthetic injection is in the IR and no expectation kind can name it, so a specification cannot say the skill was actually read.
owner: trace
tags:
- trace
relations:
- decomposes: epic:checker-vocabulary-depth
revision: 1
---
# Story: An expectation kind for the skill's text entering context

## Outcome

A specification can say that the skill was actually read — that its text entered the model's context —
rather than only that the skill file exists and that the model behaved as if it had read it.

## Context

The design records the synthetic injection as *observable with no expectation kind in v0.1*. The
event is already in the IR as `SyntheticInjection`, so the kind costs almost nothing to add; what was
refused was the wrong version of it. A matcher over "a synthetic event containing the skill's text"
that graded phrasing would be a wording assertion wearing a structural costume, and that is not what
this kind is for.

## Acceptance

- A specification can assert that a named skill's text entered context, satisfied by the presence of
  the injection event for that skill.
- The assertion says nothing about what the text contained beyond identifying the skill.
- A transcript from an adapter that does not surface the injection yields `unk`.
- The verdict cites the injection event's index, like every other verdict.

## Out of Scope

Any assertion about the skill's content, its length, or whether the model followed it. What the model
did with the text is judged by the other forty-nine kinds.

## Open Questions

Whether the kind identifies a skill by name or by digest. Decides: trace owner. Default if nobody
answers: by name — a digest would make every skill edit break every specification that mentions it.
