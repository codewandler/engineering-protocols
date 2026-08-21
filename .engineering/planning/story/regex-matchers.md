---
format: aep.planning-md/1
id: story:regex-matchers
kind: story
status: draft
title: regex matchers, or the recorded reason there are none
summary: 'The dependency decision behind TRACE-SPEC-008: adopt a regular-expression engine, or keep refusing regex by name and say so where an author will read it.'
owner: trace
tags:
- trace
relations:
- decomposes: epic:checker-vocabulary-depth
revision: 1
---
# Story: `regex` matchers, or the recorded reason there are none

## Outcome

An author writing a matcher knows exactly where this repository stands: either regular expressions
work, or `regex:` is refused by name with a message that names `glob` and the reason — and either way
nobody discovers it by having a specification mean something other than what it says.

## Context

`TRACE-SPEC-008` refuses `regex:` today, deliberately: the workspace carries no regular-expression
engine, and the standing policy is to prefer no dependency and record the refusal. Reading `regex:` as
`contains:` would be worse than refusing, and refusing an unknown *field* would tell the author the
wrong thing. What `glob` buys is the design's own example — `*/.engineering/planning/*.md` is a glob
wearing a regular expression's syntax. What it does not buy is alternation, capture and quantifiers,
and that loss is named rather than discovered.

## Acceptance

- Either: a regular-expression engine is adopted, named in `AGENTS.md` § *Dependencies* with the
  alternatives considered, and `regex:` matchers work with the same three-valued semantics as `glob`;
- Or: the refusal stands, and the reason is stated where an author writing a matcher will read it —
  not only in a design document.
- The decision is recorded in the gap register by name, and the row leaves the deferred table by
  decision rather than by disappearing.
- Whichever way it goes, an existing specification using `glob` keeps its exact meaning.

## Out of Scope

A hand-rolled regular-expression engine. That is a dependency written by us with none of the auditing
and all of the surface.

## Open Questions

Whether the alternation case is real. Decides: trace owner, from the expectations actually written —
if no committed specification wants alternation, the refusal is free and should stand.
