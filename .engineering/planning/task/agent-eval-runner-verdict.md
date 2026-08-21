---
format: aep.planning-md/1
id: task:agent-eval-runner-verdict
kind: task
status: draft
title: 'run-agents.sh: two stages, one verdict table, an honest exit code'
summary: Sequence both stages, expand the trace rows by the rule the sibling evals use, print the table on every path including failure, and exit non-zero when a gating row failed.
owner: eval
tags:
- eval
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
- depends_on: task:agent-eval-decomposer-stage
- depends_on: task:agent-eval-reviewer-stage
- depends_on: task:agent-eval-trace-documents
revision: 4
---
# Task: the runner, the verdict table and the exit code

## What

**R15–R16.** `integrations/claude-code/eval/run-agents.sh`: the script that builds the fixture, runs
both stages in sequence, checks each stage's trace document against that stage's own transcript, and
prints **one** verdict table.

It prints, pass or fail: the table, the created artifacts, the `validate` output, the `git status`
output, both trace verdicts, and the run cost. It exits non-zero if any gating row failed.

Trace rows expand into the table by the rule `run.sh` and `run-driven.sh` already use:

- a gating `gap` or `unk` **fails**;
- an advisory row of any verdict is a **note**;
- a stage that produced **zero** rows **fails** — a table with no transcript rows in it goes green
  while checking nothing.

Model and turn bounds are environment overrides with defaults, named `EVAL_MODEL` and
`EVAL_MAX_TURNS`, as its two siblings name them.

## Why

Without this the stages are two scripts nobody runs together, and the tree-side and transcript-side
halves of each bound never land in the same place. The zero-rows rule is what stops the case
reporting success for a check that did not happen.

## Done When

| # | Acceptance |
|---|---|
| R1 | One live `run-agents.sh` prints a single verdict table containing every row named in the two stage tasks (`D1`–`D9`, `P1`–`P7`) and one row per expectation in both trace documents, each named. |
| R2 | It exits 0 when every gating row is green, and non-zero when any one is red. Shown both ways. |
| R3 | Forcing a stage to fail early still prints the full verdict table before exit — no assertion aborts the script before the report. Shown for a failure in each stage. |
| R4 | A trace document that yields **zero** rows for a stage makes the run fail with a reason naming that stage. Shown with an emptied document. |
| R5 | A gating `unk` verdict fails; an advisory row of any verdict does not. Shown with one of each. |
| R6 | Each stage's document is checked against **that stage's own transcript** — two sessions, two transcripts. Shown by the printed transcript paths differing. |
| R7 | The output ends with the run cost and the scratch directory path. |
| R8 | `EVAL_MODEL` and `EVAL_MAX_TURNS` are read with defaults; overriding each changes the invocation, shown by a dry printout of the command. |
| R9 | `run.sh` and `run-driven.sh` are byte-identical to their pre-task state, and no `Taskfile.yml` target was added. Shown by `git diff --stat`. |

## Notes

- Two sessions, not one: a single session's transcript would carry both agents' calls, and every
  `tool.absent` bound would then be a claim about the wrong agent. This is the specification's
  default for that Open Question.
- Never part of `task check` — the live mode reaches the Claude API. The gate stays hermetic.
- `task agent-eval` beside `plugin-eval` and `driven-eval` is a follow-up, not this task: the root
  Taskfile is outside the declared surface.

## Verifier

`integrations/claude-code/eval/checks/check-runner-verdict.sh`. R1–R9 are its rows; the ids collide
with the specification's requirement numbers because they are this task's own, and renaming them
would break what a verdict table is for.

R2–R8 run the whole runner hermetically, through the four `EVAL_REPLAY_*` variables in
`checks/contracts/interface.md` — with all four set, a complete `run-agents.sh` makes no API call,
which is what lets "exit 0 when green, non-zero when red" be shown both ways for free.
