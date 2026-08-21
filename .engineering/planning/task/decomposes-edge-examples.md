---
format: aep.planning-md/1
id: task:decomposes-edge-examples
kind: task
status: draft
title: Correct the two examples that teach derived_from for an epic edge
summary: decomposer.md's creation example and the planning skill's worked decomposition teach --relate decomposes:epic:..., matching all 39 stories in the store and what D4 asserts.
owner: eval
tags:
- plugin
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
revision: 1
---
# Task: the two examples that teach the wrong epic edge

## What

**R19.** Change the epic edge in the two places the plugin teaches it:

| File | From | To |
|---|---|---|
| `integrations/claude-code/agents/decomposer.md` | `--relate derived_from:epic:passkey-login` | `--relate decomposes:epic:passkey-login` |
| `integrations/claude-code/skills/planning/SKILL.md` | the same, in the worked decomposition | the same |

Nothing else in either file changes.

## Why

D4 asserts a `decomposes` edge, because that is what the stores contain: 39 of 39 stories in
`.engineering/planning/story/` carry `decomposes: epic:…`, none carries `derived_from: epic:…`, and
every story in `examples/planning-passkeys` does the same. Left alone, these two examples would make
D4 fail against an agent that followed its own charter correctly — the check would be red for
teaching, not for drift.

## Done When

Verifiable on its own, with no API call and without any other task in this set existing.

| # | Acceptance |
|---|---|
| E1 | Neither file contains `derived_from:epic:` in a `protocol artifact new` example. |
| E2 | Both files contain `decomposes:epic:` in the example that previously read `derived_from`. |
| E3 | The diff touches only those two files, and only the relation token on those lines — no surrounding prose is rewritten. |
| E4 | Running the corrected command verbatim against a scratch store creates a story whose frontmatter carries `decomposes: epic:…`, and `protocol artifact validate` exits 0. |

## Notes

- The alternative — D4 accepting either edge, as `run.sh` §3.3 does — is the specification's Open
  Question, and its default is **no**. Do not widen D4 instead of fixing these two lines.
- `run.sh` and `run-driven.sh` are out of scope, including §3.3.

## Verifier

`integrations/claude-code/eval/checks/check-decomposes-edge-examples.sh`. E1–E4 are its rows.

E3 is exact rather than approximate: it undoes the substitution on the current file and requires the
result to be byte-identical to the file at the pinned pre-task revision in
`checks/contracts/pre-task-blobs.txt`. Any other edit anywhere in either file survives that undo.
