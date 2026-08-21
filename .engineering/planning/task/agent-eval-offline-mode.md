---
format: aep.planning-md/1
id: task:agent-eval-offline-mode
kind: task
status: draft
title: 'Offline mode: replay the committed transcripts, and name what it did not cover'
summary: Re-check both trace documents against eval/fixtures/ with no API call, fail by name on a missing fixture, and print the tree-side assertions an offline run cannot reach.
owner: eval
tags:
- eval
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
- depends_on: task:agent-eval-trace-documents
- depends_on: task:agent-eval-runner-verdict
revision: 3
---
# Task: `--offline`, and the honesty of a partial run

## What

**R17–R18.** An `--offline` mode on `run-agents.sh` that re-checks both trace documents against the
committed transcripts in `integrations/claude-code/eval/fixtures/` and makes **no API call**.

It never skips. When a fixture is missing it fails with a named reason — the missing file. When it
succeeds it prints, in its own output, which assertions it did **not** cover: every tree-side row
(`D1`–`D7`, `P1`–`P3`), which cannot be replayed from a transcript.

## Why

This is the mode that holds the transcript bounds between live runs, under the task's standing
default: committed transcripts for the bounds, one live run per release. Nothing in `task check`
invokes it, and wiring the documents into `cargo test -p trace-spec` is out of surface — so a
partial run that does not say it is partial reads as a full one.

## Done When

Verifiable on its own, with **no API call**, against fixtures placed by hand if the live run has not
happened yet.

| # | Acceptance |
|---|---|
| O1 | `run-agents.sh --offline` with both fixtures present exits 0 and prints a verdict row per expectation in both documents. |
| O2 | It makes **no** network call — shown by running it with the API credential unset and with no network reachable, still exiting 0. |
| O3 | It creates no scratch project and runs no headless session; shown by no new directory appearing under `$TMPDIR`. |
| O4 | With `eval/fixtures/` removed it exits non-zero and the reason names the missing file by path. |
| O5 | With exactly one of the two fixtures removed it exits non-zero and names that one — it does not silently check the other and pass. |
| O6 | Its output names every tree-side assertion it did not cover, listing `D1`–`D7` and `P1`–`P3` explicitly, not as a count or a phrase. |
| O7 | A fixture transcript that violates a bound makes `--offline` exit non-zero with that row red. Shown with a hand-edited copy. |
| O8 | The word "skip" appears in no verdict the mode can print. |

## Notes

- O6 lists `D1`–`D7` and `P1`–`P3`. `D8`, `D9`, `P4`–`P7` are transcript-derived and **are** covered
  offline; do not list them as gaps.
- Depends on the trace documents existing and on the runner's table and exit rule.
- The fixtures this mode replays are committed by the live-run task; hand-made transcripts are
  acceptable inputs while proving this task.

## Verifier

`integrations/claude-code/eval/checks/check-offline-mode.sh`. O1–O8 are its rows, and this is the
one check in the set whose subject is hermetic by design — every row is a real assertion today
rather than a recording.

O2 asserts "no API call" by removing every way to make one — no credential, no config home, every
proxy pointed at a closed port — and requiring exit 0 anyway. O3 counts the directories `$TMPDIR`
gains. Neither reads the script and takes its word.
