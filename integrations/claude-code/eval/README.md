# Plugin eval

A repeatable, inspectable check that the planning plugin actually teaches an agent to plan: a
headless Claude process is dropped into a scratch copy of a minimal project with the plugin
loaded, given a fixed dummy task ([`prompt.md`](./prompt.md)), and what it created is then
inspected mechanically.

```bash
./run.sh              # or: task plugin-eval   (from the repository root)
```

## What a run produces

A scratch directory (under `$TMPDIR`, kept after the run, path printed at the end) containing:

| Path | What it is |
|---|---|
| `project/` | the fixture project the agent worked in — its `.engineering/planning/` holds whatever the agent created |
| `plugin/` | the copy of this plugin the agent ran with (`eval/` excluded) |
| `result.jsonl` | the full `stream-json` transcript, one event per line |
| `stderr.log` | the agent process's stderr |
| `metrics.txt` | the informational metrics block, as the report prints it |
| `review-input.md`, `review.md`, `timeline.txt` | what the adversarial reviewer saw, and what it said |

## What green means

A run is checked by **composition**: the workspace is judged by looking at files, and the
transcript is judged by a typed document. `run.sh` exits 0 only if both halves hold.

**The workspace, in the shell** — these are questions about files and they stay where they are:

1. `protocol artifact validate` exits 0 on the created store — every status lifecycle-legal,
   every relation resolvable, every file parseable.
2. At least one epic and at least two stories exist.
3. Every story carries a `derived_from`/`decomposes` relation to an epic.

**The transcript, as a document** — one call to `protocol trace check` against
[`expectations.trace.yaml`](./expectations.trace.yaml):

```bash
protocol trace check --spec expectations.trace.yaml --transcript "$WORK/result.jsonl"
```

That file is 41 expectations over 40 kinds of the `trace-spec/1` vocabulary, and it replaced five
assertions written in three idioms — a `grep` for a string anywhere in 86KB of JSON, two `jq`
filters each carrying a weaker `grep` fallback for when `jq` was absent, and one `jq` filter that
passed *unconditionally* when it was. The claims it carries include:

* the planning skill **completed** — the `Skill` tool's structured result reports `success: true`,
  a boolean the harness set rather than a sentence the model wrote;
* artifacts were created through a `Bash` call whose command matches `protocol artifact new` — a
  tool call with a name and an argument matcher, not a string found somewhere in the file;
* the terminal record is clean: `is_error: false`, `terminal_reason: completed`, no API error
  status, zero permission denials;
* the environment is **hermetic** — the init event lists exactly one plugin,
  `engineering-protocols`. The run gets a scratch `CLAUDE_CONFIG_DIR` holding only a copy of your
  login credentials, so your own plugins, skills and output style cannot leak in (before this
  existed, five of them did);
* auth is the **login**, not a stray API key: `apiKeySource: none` — the check that catches an
  exported `ANTHROPIC_API_KEY` before a single turn is spent;
* the skill was consulted *before* the store was touched, nothing shelled out to `rm -rf`, and
  every `protocol artifact` call came back in under two seconds.

Twenty-three further expectations are **advisory**: cost, tokens, cache state, latency, rate-limit
headroom and the model's resolved name. They are evaluated, printed as `note` rows in the verdict
table and counted separately — and they never move the exit code, because a gate that goes red
when a cache was cold is a gate people learn to ignore. An advisory expectation is *not* a disabled
one: a check that is switched off reads exactly like a check that passed.

Every bound in the file carries the observed value in the comment beside it, so the next reader
knows what it was calibrated against, and `cargo test -p trace-spec` checks the whole document
against two committed real transcripts — so a bound that stops holding is caught by the ordinary
gate rather than by a paid eval run.

`EVAL_USE_API_KEY=1` passes `--advisory billed-to-the-session`, which downgrades exactly that one
row: it is still evaluated, still printed, and the report names it as downgraded. An id the
document does not declare is a usage error, so a typo there fails loudly instead of quietly
relaxing nothing.

Exit codes are the checker's, mirroring `ess conform`: `0` conformant, `1` contradicted, `3`
nobody found out. Exit 3 means an event the adapter could not read, or a field this transcript does
not carry — *"the format moved under us"*, which wakes a different person than *"the agent did the
wrong thing"*. `run.sh` treats both as a failure of the run, which is the CI job making the choice
the checker deliberately refuses to make on its behalf.

`protocol trace inspect --transcript <file>` prints the same census the metrics block below does —
event families, per-tool traffic in both directions, each step's `gen`/`exec` split — from the same
IR the checker judges.

The verdict table, the created file list, `protocol artifact list`, the validate output and the
run's API cost are printed on every run, pass or fail — plus an **informational metrics block**
(never asserted, because the numbers vary run to run): resolved model and Claude Code version,
API-key source and the loaded plugin set, turns / API requests / assistant events / iterations
(four different quantities — bounds belong on the right one), token counts, cache read/created
and hit ratio, TTFT and durations, the account's rate-limit status including whether the run
billed into overage, and **tool traffic**: per tool, how many calls, how many failed, and how
many bytes (≈ tokens) their results injected into the context window, plus the count of
identical repeated calls — failing and repeated calls are how you see whether the model actually
understood the tooling. Every step also carries two derived durations from the recorded event
timestamps: **gen** (the inference interval that produced the tool call — attributed to the call
that follows it) and **exec** (call issued to result back), with a `time-split` total showing how
much of the wall clock was model inference versus tools running.

## The adversarial review

After the mechanical inspection, a **second, independent headless session** reviews the run
adversarially: it gets the task, the verdict table, the metrics, a timing-annotated summarized
timeline and the created artifacts verbatim, and reports what assertions cannot see — guardrails followed to the letter but not in spirit, wasted
or repeated calls, risky idioms (a whole-file `Write` where a targeted `Edit` was safer). Its
findings cite timeline lines and end in one line: `ADVISORY: sound` or `ADVISORY: concerns — …`.

It is **advisory by design and never touches the exit code**: an LLM's judgement is not a
deterministic check, and this eval's authority stays with the assertions. The review is printed
in the report and kept as `review.md` in the scratch directory beside `review-input.md` (exactly
what the reviewer saw) and `timeline.txt`. `EVAL_REVIEW_MODEL` overrides the reviewer's model;
`EVAL_SKIP_REVIEW=1` skips the stage.

## What this is not

- **Not part of `task check`.** It reaches the Claude API: network and money. The gate stays
  hermetic.
- **Not a benchmark.** One fixed task, pass/fail assertions; its job is catching the plugin
  teaching the wrong mechanics (hand-edited statuses, unlinked stories), not scoring plan
  quality. `EVAL_MODEL` / `EVAL_MAX_TURNS` override the defaults (`sonnet`, 30).
- **Not the native eval framework.** `claude plugin eval` is early-access and org-gated at the
  time of writing; when it is available here, these cases should become a native suite and this
  script the fallback.
