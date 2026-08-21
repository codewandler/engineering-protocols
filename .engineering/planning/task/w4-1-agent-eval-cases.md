---
format: aep.planning-md/1
id: task:w4-1-agent-eval-cases
kind: task
status: draft
title: 'W4-1: agent-eval-cases, driven as a governed run'
summary: 'The request recorded at intake: task W4-1 (kind feature, objective agent-eval-cases) asks for story:agent-eval-cases under protocol adp/1 and profile development.driven.'
relations:
- derived_from: story:agent-eval-cases
revision: 1
---
# Task: W4-1 — agent-eval-cases

Intake record. This is the request as `.engineering/task.yaml` states it, not an interpretation of
it. Everything below is either a field copied from that task document or a quotation from it or from
the one artifact it names; nothing here is specified, designed or decomposed, because none of that
has been asked for yet.

## What

What the task document declares, field by field:

| Field | Value |
|---|---|
| task id | `W4-1` |
| task kind | `feature` |
| objective | `agent-eval-cases` |
| protocol | `adp/1` |
| profile | `development.driven` |
| derived from | `story:agent-eval-cases` |

Its own header calls it "W4.1's governed dogfood run, and the first task document this repository has
ever written about itself", and says that it "names one story out of `.engineering/planning/` and
nothing else. The story is the contract; this file is only what a run needs in order to resolve a
plan against it."

The requester's stated reason for `development.driven` rather than `development.standard`, verbatim:
"the planning store has no tool surface other than the `protocol` CLI, so under
`development.standard` a driven `llm` step cannot create an artifact at all and the run does not
fail — it never moves. `development.driven` extends `development.standard`, so `approval-gates` and
its `review → complete` guard are unchanged, and the run still stops at a person."

## Why

The one artifact the request names is `story:agent-eval-cases`, carried here by the task document's
own `derived_from` edge. The requester's note: "The story is `story:agent-eval-cases` in
`.engineering/planning/story/agent-eval-cases.md`; its Acceptance section is the specification this
run is measured against."

That story sits in `draft` under `epic:self-evaluation`, and states its own outcome as: "Somebody
editing the `decomposer` or the `plan-reviewer` finds out from a red check that they widened what the
agent may do — instead of from a store that quietly grew statuses nobody moved on purpose."

## Done When

The requester defers this to the story. Its Acceptance section, quoted unedited:

> - A decomposer run against an epic leaves a scratch store in which every created artifact is a `story`
>   in `draft`, each carrying a `decomposes` edge to that epic, and no other artifact's status changed.
> - The store the decomposer leaves passes `protocol artifact validate`.
> - A `plan-reviewer` run against the same store leaves `git status` clean — asserted on the tree, not
>   read from the agent's definition.
> - Both assertions are expectations in the trace specification, so they are checked the same way every
>   other bound is.

The story also draws a boundary, quoted unedited: "Judging the *quality* of the decomposition.
Whether the stories are good is a person's call; whether the agent stayed inside its charter is
mechanical, and only the second one belongs in a gate."

## Notes

Declared facts — `constraints.facts` in the task document, asserted by the requester and observed by
nothing:

| Fact | Value |
|---|---|
| `change.public_contract` | `false` |
| `change.architectural` | `false` |

The requester's comment on both: "The change is confined to `integrations/claude-code/` — the
plugin's eval surface and the two agent charters. No crate's public API and no published contract
moves."

`constraints.notes`, the two that are not the story pointer already quoted above:

- "Implementation surface is `integrations/claude-code/**`. Do not modify anything under `crates/`,
  `website/`, or the workspace `Cargo.toml` — other work is in flight there."
- "The story's own Open Question has a stated default — committed transcripts for the bounds, one
  live run per release — and that default holds unless somebody records otherwise."

The Open Question that default answers, from the story: "Whether the cases run against committed
transcripts or need a live model. Decides: eval owner."

Sources: `.engineering/task.yaml`; `.engineering/planning/story/agent-eval-cases.md`.
