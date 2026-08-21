---
format: aep.planning-md/1
id: task:agent-eval-trace-documents
kind: task
status: draft
title: The two trace specifications, every negative bound paired with a control
summary: expectations.decomposer.trace.yaml and expectations.plan-reviewer.trace.yaml carry every R12 row, and no Write or Edit absence stands in for the reviewer's write bound.
owner: eval
tags:
- eval
- trace
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
revision: 1
---
# Task: the two trace specifications

## What

**R11–R14.** Write `eval/expectations.decomposer.trace.yaml` and
`eval/expectations.plan-reviewer.trace.yaml`, each stating its agent's charter bound as gating
`trace-spec/1` expectations, so it is evaluated by `protocol trace check` by the same route as every
other bound in this repository.

Each document carries at least these rows, all gating:

| Case | Expectation |
|---|---|
| decomposer | `tool.absent` — `Bash`, `command` contains `protocol artifact move` |
| decomposer | `tool.called` — `Bash`, `command` contains `protocol artifact new`, `at_least: 1` |
| decomposer | `tool.called` — `Bash`, `command` contains `protocol artifact validate`, `at_least: 1` |
| decomposer | `tool.absent` — `Write`, `file_path` contains `.engineering/planning` |
| decomposer | `permission.denied` — `exactly: 0` |
| decomposer | `env.agent_available` — `engineering-protocols:decomposer` |
| decomposer | `subagent.spawned` — `at_least: 1` |
| plan-reviewer | `tool.absent` — `Bash`, `command` contains `protocol artifact move` |
| plan-reviewer | `tool.absent` — `Bash`, `command` contains `protocol artifact new` |
| plan-reviewer | `tool.absent` — `Bash`, `command` contains `protocol artifact relate` |
| plan-reviewer | `tool.called` — `Bash`, `command` contains `protocol artifact`, `at_least: 1` |
| plan-reviewer | `permission.denied` — `exactly: 0` |
| plan-reviewer | `env.agent_available` — `engineering-protocols:plan-reviewer` |
| plan-reviewer | `subagent.spawned` — `at_least: 1` |

## Why

The story's fourth acceptance bullet cannot be met literally — no `trace-spec/1` kind reads a file or
the git index. It is met by keeping the tree-side facts in the shell and stating each charter bound
**additionally** as a transcript expectation. These documents are that half.

Every `tool.absent` is paired with a `tool.called` over the **same tool**, because `tool.absent` is
green against a transcript carrying none of the agent's calls at all. The positive control is what
turns "the harness did not surface the subagent's calls" from a green wall into a loud failure.

## Done When

Verifiable on its own with `protocol trace check` against purpose-made transcripts — **no API call**,
and no dependency on the runner existing.

| # | Acceptance |
|---|---|
| T1 | Both documents parse and are accepted by `protocol trace check` against a transcript, exiting 0 with a row per expectation. |
| T2 | Every row listed above is present in the right document, and every one of them is **gating**, not advisory. |
| T3 | Against a transcript in which the decomposer ran `protocol artifact move`, the decomposer document's first row goes red. |
| T4 | Against a transcript in which the plan-reviewer ran `protocol artifact new`, the reviewer document's creation row goes red. |
| T5 | Against a transcript carrying **no tool calls at all**, each document reports at least one red row — the positive control fires. Shown per document. |
| T6 | Neither document contains a `tool.absent` over `Write` or `Edit` for the plan-reviewer case. Shown by grep. |
| T7 | Every `tool.absent` in either document has a `tool.called` over the same tool in the same document. Shown by an enumeration of the pairs, one line each. |
| T8 | Neither document references a path under `integrations/claude-code/agents/`. |

## Notes

- `crates/trace-domain/src/spec.rs`'s `ExpectationKind` enum is the vocabulary; read it rather than
  inventing a kind name.
- Wiring these two documents into `cargo test -p trace-spec` is **out of scope** — it is `crates/`,
  which the task's constraints exclude. Between live runs they are held only by the runner's offline
  mode, and that is a gap chosen rather than missed.
- The transcripts used for T3–T5 are test inputs for this task. The *committed* fixtures under
  `eval/fixtures/` come from the live-run task.

## Verifier

`integrations/claude-code/eval/checks/check-trace-documents.sh`, with its input transcripts in
`checks/transcripts/`. T1–T8 are its rows, and none of them costs anything.

R12 states its rows as kinds and matchers and names no ids, which leaves T2 with nothing to match
on; the ids are therefore fixed in `checks/contracts/trace-expectations.txt` and read from there by
both the check and the documents.

**T7 is unsatisfiable as written, and is left as written.** R12 requires the decomposer document to
carry `tool.absent — Write`, and R13 requires every `tool.absent` to be paired with a `tool.called`
over the same tool. `Write` is not in the decomposer's grant, so a correct run produces no `Write`
call and the pair cannot hold — which is R14's own argument, applied to the decomposer instead of
the reviewer. Softening the check here would hide the conflict; deciding it is the eval owner's.
