---
format: aep.planning-md/1
id: task:agent-eval-readme
kind: task
status: draft
title: README section for the agent eval, in the shape its two siblings use
summary: Document run-agents.sh, its offline mode, EVAL_MODEL and EVAL_MAX_TURNS, and why the eval is deliberately not part of task check.
owner: eval
tags:
- docs
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
- depends_on: task:agent-eval-runner-verdict
revision: 2
---
# Task: the README section

## What

A section for this eval in `integrations/claude-code/eval/README.md`, in the shape the two sections
beside it already use. It states:

- what `run-agents.sh` runs — two stages, one per shipped agent, against a seeded scratch store;
- how to invoke it, live and with `--offline`;
- `EVAL_MODEL` and `EVAL_MAX_TURNS`, with their defaults;
- which files make up the case, including the two trace documents and `fixtures/`;
- that it is **not** part of `task check`, and why: the live mode reaches the Claude API — network
  and money — and the gate stays hermetic;
- the standing cadence: committed transcripts for the bounds, one live run per release;
- the two named follow-ups — a `task agent-eval` target, and wiring the trace documents into
  `cargo test -p trace-spec` — each with the reason it is not here.

## Why

The two evals beside it are discoverable because that README describes them. An eval nobody knows how
to run is one that is run once, at implementation time, and never again — which is exactly the
failure mode a per-release cadence is supposed to prevent.

## Done When

Verifiable on its own by reading the file, with no API call.

| # | Acceptance |
|---|---|
| M1 | The section exists and follows the heading depth, ordering and layout of the `run.sh` and `run-driven.sh` sections — shown by placing the three side by side. |
| M2 | It names every file in the deliverable list, and each named path exists. |
| M3 | The invocation lines it prints are runnable verbatim: `--offline` copied from the README exits 0 against committed fixtures. |
| M4 | `EVAL_MODEL` and `EVAL_MAX_TURNS` are documented with the defaults the script actually reads — compared against the script, not from memory. |
| M5 | It states in plain words that the eval is not in `task check`, and gives the reason. |
| M6 | Both follow-ups are named, each with its reason. |
| M7 | The section describes no assertion the runner does not make — every row it mentions appears in the runner's table. |

## Notes

- M7 is the point of the task: a README that promises a check nobody wrote is a worse artefact than
  no README.
- Depends on the runner, since M3, M4 and M7 compare against it.

## Verifier

`integrations/claude-code/eval/checks/check-readme.sh`. M1–M7 are its rows.

M3 does not read the invocation the README prints — it extracts it and runs it. M4 reads each
default out of the script's own parameter expansion and requires the README to give that value. M7
collects every row id the section mentions and requires each to be one the runner's table actually
carries; the direction is deliberate, since a README may say less than the runner does, never more.
