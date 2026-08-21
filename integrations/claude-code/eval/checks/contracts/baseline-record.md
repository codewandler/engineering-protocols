# The baseline record

`task:agent-eval-decomposer-stage` defers this: *"the shape of that record is that task's contract,
not this one's."* `task:agent-eval-scratch-fixture` R4 says what it must contain and leaves the form
open. This file closes it, because `D5`, `D6` and `P3` are all differences against this record and a
difference needs two things of the same shape.

## Form

A TSV file, one row per artifact, no header, sorted by id, `\n`-terminated:

```
id	status	digest
```

| Field | Source | Notes |
|---|---|---|
| `id` | `protocol artifact list --format json` | the store's id, `kind:slug` |
| `status` | the same call | never read from the file's frontmatter — the CLI is the authority on status |
| `digest` | the artifact's file | lower-case hex SHA-256 of the file's bytes, whole file including frontmatter |

Tabs separate; no field may contain a tab. The digest is over the **whole file** on purpose: the
charter bound `D6` states is "never touch an artifact you did not create", and a digest over the body
alone would be green for a hand-edited `revision:`.

## Where it lives

`<scratch>/baseline.tsv`, printed by `run-agents.sh --build-fixture-only` as its third line. A
second, post-stage-1 record is what `P3` compares against; the runner may keep it wherever it likes,
because no check reads it directly — `P3` is the runner's row to compute.

## Why a file and not a variable

`F6` mutates one artifact and requires the digest of exactly one row to change. That is a diff of two
recorded files, which needs the record to be recordable. It is also the only form in which the
baseline survives a failed stage, and the run directory is where a person looks after a red run.
