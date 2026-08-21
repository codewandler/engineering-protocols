---
format: aep.planning-md/1
id: epic:checker-vocabulary-depth
kind: epic
status: draft
title: What the transcript checker still cannot say
summary: 'The four things trace-spec/1 deferred by name: the usage series, the skill''s text in context, streaming, and regular expressions.'
owner: trace
tags:
- trace
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: What the transcript checker still cannot say

## Outcome

The four things `trace-spec/1` deferred by name stop being footnotes: an author can assert how usage
moved across a run rather than only its totals, can say the skill's text actually entered the model's
context, can have a run stopped while it is still running, and knows exactly where the repository
stands on regular expressions.

## Why Now

Each of these was deferred deliberately and recorded where a reader would find it — the trace wave
page's own *deferred* table, and `AGENTS.md`'s acceptance row for the design. That is the correct way
to leave something owed, and it stays correct only while the register is worked. Two of the four cost
almost nothing now that the IR retains the data; the other two are decisions with a dependency
attached.

## Scope

The vocabulary, not the engine. Every item here is an expectation kind, a matcher, or an evaluation
mode over the event IR that already exists. Nothing in this epic reads a workspace, and nothing calls
a model.

## Out of Scope

Any assertion about *wording*. A matcher over "a synthetic event containing the skill's text" that
graded phrasing would be a wording assertion wearing a structural costume — the kind says the text
entered context and nothing about what it said. Also out: workspace inspection; the trace
specification owns the transcript and nothing else.

## Risks

`regex-matchers` is the one that can go wrong quietly: adopting a regular-expression engine adds a
dependency to a workspace with a written policy about them, and the alternative — keeping the refusal
— has to stay visible to authors or it reads as a bug. The streaming checker carries the design's own
warning: incremental evaluation is not designable against a format that is not stable.

## Done When

Each of the four rows has left the deferred table either by code or by a recorded decision, and none
of them left it by quietly disappearing.
