---
format: aep.planning-md/1
id: story:plugin-enforcement-hooks
kind: story
status: proposed
title: Hooks that deny, and a record of every denial
summary: PreToolUse denies from the per-state tool set, a write guard over the planning store, and the hook-decisions channel the driver folds into its run report.
owner: driver
tags:
- driver
- plugin
relations:
- decomposes: epic:reference-driver
- depends_on: story:tool-availability-expectation
- depends_on: story:protocol-drive-verb
revision: 2
---
# Story: Hooks that deny, and a record of every denial

## Outcome

A step that is not allowed to edit the plan cannot edit the plan, whatever the model decides to try —
and every refusal is in a channel the run report folds in, so *"nothing was denied"* and *"denials
are counted somewhere else"* stop looking identical.

## Context

`--allowedTools` is fixed at session launch, so the per-state tool set is enforced primarily by the
flag. The hook layer is the backstop over the same derived set, plus the one rule the flag cannot
express: a path check. `tool_input.file_path` is what makes the planning-store guard a path rule
rather than a tool ban — the model may write files, and it may not write these files.

## Acceptance

- A `PreToolUse` deny fires for a tool outside the state's derived set, and the reason reaches the
  model rather than only the log.
- A write to `.engineering/planning/**` from a step that is not permitted to move the plan is denied
  by path, with the tool itself still available for every other path.
- The driver's `claude -p` invocation carries `--settings` and never `--bare`, asserted on the
  constructed command line rather than by inspection.
- Denials land in `hook-decisions.jsonl` and appear in the run report, counted, with the rule that
  produced each.
- Hooks deny and never grant: a hook cannot widen the tool set the flag established.

## Out of Scope

Whether a plugin's hooks run without a per-invocation consent step. That is an assumption named in
the design and an unknown the wave could not close by reading; it is `story:driven-eval-acceptance`
that finds out.

## Open Questions

If plugin hooks turn out to need consent, does the driver ship its own settings file instead?
Decides: driver owner, on the evidence from the driven eval.
