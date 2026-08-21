---
format: aep.planning-md/1
id: task:agent-eval-scratch-fixture
kind: task
status: draft
title: 'Scratch fixture: seeded store, git baseline, artifact digests'
summary: Build the run-agents scratch project from examples/planning-passkeys with its planning store intact, commit it, assert a clean tree, and record the pre-stage baseline of ids, statuses and file digests.
owner: eval
tags:
- eval
- plugin
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
revision: 1
---
# Task: the scratch fixture and its baseline

## What

Build the scratch project both stages run against, and record the facts every later assertion is
measured against. Covers **R1–R4** and **R10** of the specification.

- Copy `examples/planning-passkeys` **with its planning store intact** — all seven seed artifacts at
  their committed statuses. Not emptied, which is what `run.sh` does and what this case must not do.
- Carry the repository's `artifacts/lifecycles` and `artifacts/templates`, a copy of the plugin with
  `eval/` excluded, and a scratch `CLAUDE_CONFIG_DIR` holding only the operator's credentials —
  the isolation `run.sh` §1 already establishes.
- `git init` and one commit containing every file, before stage 1 runs.
- Record the baseline: for every artifact, its id, its status (from `protocol artifact list --format
  json`) and a content digest of its file.
- A committed `.gitignore` in the fixture excludes only paths the *harness* dirties, each with a
  comment naming what writes it. No pattern may also hide a write under `.engineering/planning/`.
- The scratch directory is created under `$TMPDIR` (falling back to `$HOME/.cache/claude-tmp`),
  never `/tmp`, survives the run, and its path is printed.

## Why

D5, D6 and P3 are differences against this baseline. Without it there is nothing to subtract, and
"no other artifact's status changed" is a sentence rather than a check. R3's clean tree is what makes
P1 a claim about the plan-reviewer rather than about the copy step.

## Done When

Verifiable on its own, with **no API call** — the fixture build is reachable without running a stage
(the handle is the implementer's choice: a flag, a sourceable function, a separate script).

| # | Acceptance |
|---|---|
| F1 | Building the fixture twice produces two directories, both under `$TMPDIR` or `$HOME/.cache/claude-tmp`, and prints each path. |
| F2 | The fixture's store lists **7** artifacts, each at the status its committed source file carries — compared field by field, not counted. |
| F3 | `protocol artifact validate` exits 0 inside the fixture. |
| F4 | `git status --porcelain` inside the fixture is **empty**, and the build fails loudly if it is not. |
| F5 | `git log --oneline` inside the fixture shows exactly one commit. |
| F6 | The baseline record contains one row per artifact, each with id, status and digest; mutating one artifact file and re-reading the digest changes exactly that row. |
| F7 | **No** artifact in the fixture store holds the story lifecycle's initial status, read from `protocol artifact lifecycle story`. The build fails with a named reason if one does. |
| F8 | `CLAUDE_CONFIG_DIR` points inside the scratch directory, and the plugin copy contains no `eval/` directory. |
| F9 | The fixture `.gitignore` matches no path under `.engineering/planning/` — shown by `git check-ignore` against a created artifact path returning non-zero. |

## Notes

- F7 is the specification's invariant, asserted rather than assumed: if a future fixture change
  introduces a `draft` seed artifact, D3 stops discriminating "created in draft" from "was already
  draft", and the fixture is what must be corrected.
- `examples/planning-passkeys` is **read and copied, never edited** — it is outside the task's
  implementation surface.
- Everything lands under `integrations/claude-code/eval/`.

## Verifier

`integrations/claude-code/eval/checks/check-scratch-fixture.sh`, written before this task starts and
red until it lands. F1–F9 are its rows, by those names.

The handle it builds through — `run-agents.sh --build-fixture-only`, printing `scratch:`, `fixture:`
and `baseline:` — is fixed in `checks/contracts/interface.md`, because "the handle is the
implementer's choice" leaves a check with nothing to call. The baseline's shape is
`checks/contracts/baseline-record.md`.
