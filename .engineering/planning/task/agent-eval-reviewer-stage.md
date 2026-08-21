---
format: aep.planning-md/1
id: task:agent-eval-reviewer-stage
kind: task
status: draft
title: 'Stage 2: run the plan-reviewer and assert P1-P7'
summary: The runner's own commit of stage 1's output, a review prompt, and the seven gating assertions that separate a held bound from a reviewer that died in its first turn.
owner: eval
tags:
- eval
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
- depends_on: task:agent-eval-decomposer-stage
- depends_on: task:agent-eval-scratch-fixture
revision: 3
---
# Task: stage 2, the plan-reviewer, and P1–P7

## What

**R8–R10.** The runner's own commit of stage 1's output, `eval/prompt-plan-reviewer.md`, and the
stage that runs a headless session over the same store with the
`engineering-protocols:plan-reviewer` agent.

The commit between the stages is **the runner's, not an agent's**: it makes the tree clean again so
P1 is a claim about stage 2 alone.

Every row is gating:

| id | Assertion |
|---|---|
| P1 | `git status --porcelain` in the scratch project is empty |
| P2 | `protocol artifact validate` exits 0 |
| P3 | every artifact's status equals its post-stage-1 status |
| P4 | the terminal record has `is_error: false` and `terminal_reason: completed` |
| P5 | the session's final text is non-empty and names at least one artifact id from the store |
| P6 | at least one `Bash` call ran a read verb (`protocol artifact list`, `board`, `graph` or `validate`) |
| P7 | the run spawned at least one subagent |

P1 is asserted on the **whole tree**. A harness-dirtied path is excluded only by naming that path in
the fixture's committed `.gitignore`, with a comment saying what writes it.

## Why

A reviewer that died in its first turn also leaves the tree clean. P1 alone cannot tell a held bound
from an absent run — P5, P6 and P7 are what make the green mean something.

## Done When

| # | Acceptance |
|---|---|
| V1 | One live stage-2 run prints all seven rows, each named `P1`…`P7`, each with a verdict. |
| V2 | On that run every row is green and the stage exits 0. |
| V3 | The inter-stage commit exists in the fixture's `git log` with a message identifying it as the runner's, and `git status --porcelain` is empty immediately after it. |
| V4 | Replaying stage 2's checks against a tree with **one file touched** turns P1 red and nothing else. |
| V5 | Replaying against a store with **one status moved** turns P3 red. |
| V6 | Replaying against a transcript with an **empty final text** turns P5 red; against one with no `Bash` read verb turns P6 red; against one with no subagent turns P7 red. |
| V7 | Neither the prompt nor the stage reads a file under `integrations/claude-code/agents/`. Shown by grep. |
| V8 | The fixture `.gitignore` is unchanged by this task, or each line it adds names a single harness-written path with a comment — and `git check-ignore` against a path under `.engineering/planning/` returns non-zero. |

V4–V6 are checked against saved state and saved transcripts, not by paying for more live runs.

## Notes

- Depends on stage 1 having run: P3's reference is the **post-stage-1** status set, not the fixture
  baseline.
- The reviewer's write bound is P1 plus the `Bash` absences in the trace document. `tool.absent` over
  `Write` or `Edit` is **not** acceptable — the charter grants `[Read, Grep, Glob, Bash]`, so those
  tools are never offered and such a check is true of every possible run.
- The agent is invoked, never simulated.

## Verifier

`integrations/claude-code/eval/checks/check-reviewer-stage.sh`. V1–V8 are its rows.

V6 replays three transcripts from `checks/transcripts/`, each broken in exactly one way — an empty
final text, no `protocol artifact` read verb, no subagent — and requires P5, P6 and P7 respectively
to notice. They are the rows that separate a held bound from a reviewer that died in its first turn,
so they are the ones checked against a deliberate failure rather than against a green run.
