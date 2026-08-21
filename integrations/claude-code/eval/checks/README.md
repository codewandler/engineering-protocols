# The W4.1 verifiers

One check per decomposed task, written in the `establish_verifiers` state of run `W4-1/1` — before
`run-agents.sh`, the two prompts, the two trace documents, `fixtures/` or the README section
existed. **They are red, and red is the product.** A test that passes before the code exists is a
test of nothing, and the `establish_verifiers → implement` transition is guarded on `test.exists`
precisely so the order cannot be argued about afterwards.

```bash
./run-checks.sh                          # every check
./run-checks.sh trace-documents readme   # only those
```

Nothing here calls the Claude API.

| Check | Decides | Rows |
|---|---|---|
| `check-scratch-fixture.sh` | `task:agent-eval-scratch-fixture` | F1–F9 |
| `check-decomposes-edge-examples.sh` | `task:decomposes-edge-examples` | E1–E4 |
| `check-trace-documents.sh` | `task:agent-eval-trace-documents` | T1–T8 |
| `check-decomposer-stage.sh` | `task:agent-eval-decomposer-stage` | S1–S8 |
| `check-reviewer-stage.sh` | `task:agent-eval-reviewer-stage` | V1–V8 |
| `check-runner-verdict.sh` | `task:agent-eval-runner-verdict` | R1–R9 |
| `check-offline-mode.sh` | `task:agent-eval-offline-mode` | O1–O8 |
| `check-readme.sh` | `task:agent-eval-readme` | M1–M7 |
| `check-live-evidence.sh` | `task:agent-eval-live-evidence` | L1–L8 |

The row ids are the tasks' own. A check reports every id it declares, exactly once, on every path.

## Three rules these checks hold themselves to

**A missing deliverable is a red row, never an absent one.** `red_all` in `lib.sh` puts every
declared row in the table with one shared reason under it. A check that reported nothing when its
subject did not exist would go green in `run-checks.sh` for having no failures, which is the same
defect as a gate that was switched off.

**A live-only row is asserted against a recording, never skipped.** `S1`, `S2`, `V1`, `V2`, `R1` and
`L1`–`L6` are claims about a run that costs money. They are checked against the files
[`contracts/evidence-manifest.txt`](./contracts/evidence-manifest.txt) names — the same recordings
the specification's Acceptance Criteria already demand ("shown by a recorded run, not argued"). A
recording that is missing is red, so the live half of the case stays visible between releases
instead of quietly falling out of the table.

**Discrimination is checked, not assumed.** `S3`–`S5`, `V4`–`V6`, `T3`–`T5`, `R2`–`R5`, `O4`, `O5`,
`O7`, `L2` and `L3` each break exactly one thing and require exactly the right row to move. Most of
them also assert that *nothing else* moved, which is what catches an assertion written so loosely
that any mutation reddens it.

## What is in here besides checks

| Path | What it is |
|---|---|
| [`contracts/`](./contracts) | the handles, ids, record shapes and pinned revisions the checks read |
| [`transcripts/`](./transcripts) | small, deliberately-broken transcripts — inputs to `T3`–`T5` and `V6` |
| `lib.sh` | rows, reasons, scratch directories, and the two parsers for a verdict table |

`contracts/` is where a check and an implementation meet. `contracts/interface.md` fixes the flags
and environment variables the tasks leave to the implementer; `contracts/verdict-rows.txt` fixes the
D and P ids the runner's table must name; `contracts/trace-expectations.txt` fixes the expectation
ids R12 states as kinds only. Changing a name there is a real change — it moves what the checks
assert — and that is why it is a file rather than a convention.

`transcripts/` is **not** `../fixtures/`. These are hand-written inputs; `../fixtures/` holds the
live run's own transcripts and is what `--offline` replays.

## One conflict these verifiers surface

`T7` is written literally, as its task states it: *every `tool.absent` is paired with a `tool.called`
over the same tool in the same document*. Applied to R12's decomposer row `tool.absent — Write,
file_path contains .engineering/planning`, it demands a `tool.called` over `Write` — which a correct
decomposer run never produces, and which R14 itself argues is the mark of a bound that cannot fail.
The row is left as written rather than quietly softened. Resolving it is the eval owner's call.
