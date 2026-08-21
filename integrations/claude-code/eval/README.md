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

## What green means

`run.sh` exits 0 only if all of these hold:

1. `protocol artifact validate` exits 0 on the created store — every status lifecycle-legal,
   every relation resolvable, every file parseable.
2. At least one epic and at least two stories exist.
3. Every story carries a `derived_from`/`decomposes` relation to an epic.
4. The transcript shows at least one `protocol artifact new` invocation — the agent used the CLI
   rather than hand-writing frontmatter.
5. The planning skill **completed**: the `Skill` tool's structured result reports
   `success: true` for `engineering-protocols:planning` — not a text match.
6. The terminal record is clean: `is_error: false` and zero permission denials, so the
   sandbox contract (`--permission-mode dontAsk` + the allow-list) actually held.
7. The environment is **hermetic**: the init event lists exactly one plugin,
   `engineering-protocols`. The run gets a scratch `CLAUDE_CONFIG_DIR` holding only a copy of
   your login credentials, so your own plugins, skills and output style cannot leak in (before
   this existed, five of them did).
8. Auth is the **login**, not a stray API key: `apiKeySource: none` in the init event — the
   check that catches an exported `ANTHROPIC_API_KEY` before a single turn is spent. Skipped
   when `EVAL_USE_API_KEY=1` opts into key-based billing.

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
