---
format: aep.planning-md/1
id: task:agent-eval-live-evidence
kind: task
status: draft
title: One live run, two mutation runs, and the fixtures they leave
summary: 'Demonstrate the specification''s Acceptance Criteria: a green live run, the transcripts committed from it, and a recorded red gating row for each of the two deliberate charter mutations.'
owner: eval
tags:
- eval
relations:
- decomposes: specification:agent-charter-eval-cases
- derived_from: task:w4-1-agent-eval-cases
- depends_on: task:agent-eval-offline-mode
- depends_on: task:decomposes-edge-examples
- verifies: specification:agent-charter-eval-cases
revision: 4
---
# Task: the live run, the mutation runs, and the fixtures

## What

Produce the evidence the specification's Acceptance Criteria demand, and commit what it leaves:

1. **Run 1 — live, unmutated.** `run-agents.sh` against the shipped agents. Keep its two transcripts
   as `integrations/claude-code/eval/fixtures/`.
2. **Run 2 — decomposer mutated.** Delete `agents/decomposer.md`'s hard rule 1, re-run, record the
   verdict table, restore the file.
3. **Run 3 — plan-reviewer mutated.** Remove `agents/plan-reviewer.md`'s "You change nothing"
   section, re-run, record the verdict table, restore the file.
4. **Offline checks** against the committed fixtures, present and removed.

Each run's verdict table is recorded as output, not summarised.

## Why

Criteria 2 and 3 are the ones that matter: runs 1, 4 and 5 show the case *runs*, and only a
deliberate mutation shows it *discriminates*. A check that has never been seen red is a check whose
red path has never been executed. The specification says this is shown by a recorded run, not argued.

## Done When

| # | Acceptance |
|---|---|
| L1 | Run 1 exits 0, and its verdict table contains every row of `D1`–`D9`, `P1`–`P7` and both trace documents, each named. The table is recorded verbatim. |
| L2 | Run 2 — decomposer hard rule 1 deleted — exits non-zero with **at least one gating row red**, and the recorded table names which. |
| L3 | Run 3 — plan-reviewer's "You change nothing" section removed — exits non-zero with `P1` or one of its `Bash` absences red, and the recorded table names which. |
| L4 | Both agent files are byte-identical to their pre-mutation state afterwards, shown by `git diff` over `integrations/claude-code/agents/` being empty except for the `decomposes` edge change. |
| L5 | `--offline` against the fixtures committed from run 1 exits 0, makes no API call, and names the tree-side assertions it did not cover. |
| L6 | `--offline` with `eval/fixtures/` removed exits non-zero with a reason naming the missing file. |
| L7 | `git status` in **this** repository shows changes only under `integrations/claude-code/`. |
| L8 | The committed fixtures are the transcripts of run 1 itself — shown by their session ids matching run 1's recorded output, not by resemblance. |

## Notes

- L2 and L3 mutate files in the **working tree** and restore them. Never commit a mutated agent
  charter; never leave one behind.
- If run 2 or run 3 comes back green, the finding is that the case does not discriminate, and the
  answer is to fix the assertions — not to pick a larger mutation until something goes red.
- Depends on every other task in this set. This is the last one.

## Verifier

`integrations/claude-code/eval/checks/check-live-evidence.sh`. L1–L8 are its rows, and every one of
them reads a committed recording rather than making a call: the paid runs happen once, and this
check is what holds them to what they are claimed to have said.

The recordings' paths and required shape — including the `exit:` line and the two `session:` lines
`L8` matches against the committed fixtures — are `checks/contracts/evidence-manifest.txt`.

**L7's carve-out, stated rather than hidden.** `git status` in this repository also shows
`.engineering/`: the task document, the specification, these nine tasks and the run directory, all
written by `protocol drive` rather than by this change. The check excludes that prefix and **prints
every path it excluded**, so the next thing that lands there is visible instead of absorbed.
