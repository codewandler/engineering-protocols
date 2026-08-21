# Evaluating the Codex variant

Two halves, and only one of them exists.

| half | what it would establish | state |
|---|---|---|
| **the instruction surface is well-formed and reaches the model** | the files under test are what Codex parses, and what they say is in front of the model before anything is invoked | **built** — [`check-instruction-surface.sh`](./check-instruction-surface.sh), no API, no network, no token |
| **an agent given those instructions plans correctly** | the variant works, which is the claim the Claude Code plugin's `eval/run.sh` makes about its side | **not built**, and the blocker is named below |

## What is built

```bash
./check-instruction-surface.sh
```

Nine rows, all mechanical. Dependencies are rows rather than skips: a machine without `codex` fails
the script with a row saying so, on the same rule the gate holds for the Go toolchain — a check that
quietly passes without its subject reads exactly like a check that passed.

1. `codex`, `python3` and Codex's own plugin validator are present. The validator is
   `$CODEX_HOME/skills/.system/plugin-creator/scripts/validate_plugin.py`, shipped inside the CLI
   and materialized into the Codex home. It is used instead of a hand-written schema check because
   it is the vendor's definition of a well-formed plugin rather than this repository's opinion of
   one — it is what rejects a `hooks` key in the manifest, and what requires the skill's frontmatter
   to carry a non-empty `name` and `description`.
2. The manifest and the skill pass it.
3. A scratch project carrying this variant renders the skill and the `AGENTS.md` text into the
   model-visible prompt. `codex debug prompt-input` builds that prompt locally and prints it as
   JSON; the rows assert the skill is listed by name and description, that the project's `AGENTS.md`
   arrives as `# AGENTS.md instructions for <path>`, and that guardrail 1 is in the prompt with
   nothing having been invoked.
4. Nothing leaked. The fixture runs under a scratch `CODEX_HOME` — the analogue of the Claude
   eval's scratch `CLAUDE_CONFIG_DIR` — and every skill locator in the rendered prompt must sit
   inside the fixture or that scratch home. Without this row the check would be measuring the
   machine it ran on.

**Verified by breaking it**, per `AGENTS.md` § *Conventions*, on 2026-08-22 against codex-cli
0.145.0. Against the tree as committed: `9 pass, 0 fail`, exit 0. Against a copy with guardrail 1
reworded and the skill's description replaced: `7 pass, 2 fail`, exit 1, naming both. Against a copy
whose fixture points at the operator's real `CODEX_HOME`: `8 pass, 1 fail`, listing six foreign skill
locators. Re-run it rather than trusting those numbers.

## What is not built, and why

**There is no `run.sh`.** The Claude Code eval is a composition — the workspace is judged by looking
at files, and the transcript is judged by `protocol trace check` against a typed document. On Codex
the first half would port unchanged; the second half has nothing to run against, because
**`trace-spec` has no Codex adapter**. `crates/trace-spec/src/adapter.rs` reads one format, Claude
Code `stream-json`, and declares itself as `claude-code/stream-json`.

So a live Codex run today would produce a transcript that this repository cannot read. A script that
spent money to produce a file nothing judges would not be a weaker eval — it would be an eval that
reports nothing while looking like one, which is the failure mode the whole trace domain exists
against. The honest state is that the second half is blocked on a code change this variant is not
allowed to make, and it is written down here rather than approximated.

**What the adapter would read is already decided, and it is not stdout.**
[`docs/reviews/2026-08-21-codex-harness-research.md`](../../../docs/reviews/2026-08-21-codex-harness-research.md)
settles it against a local install and 2,437 rollout files: `codex exec --json` carries no
timestamps, no durations and no cost, and `trace-ir/1` wants all three. The input is the session
rollout JSONL under `$CODEX_HOME/sessions/YYYY/MM/DD/`, and the recommended flow is to use
`codex exec --json` only to learn the thread id and then read the matching rollout.

**The tiers this would land in are already written.**
[`docs/plan/harness-wave-4-governed-dogfood.md`](../../../docs/plan/harness-wave-4-governed-dogfood.md)
§ W4.4 names three — *full* (a live run judged by the same specification), *partial* (the reader
exists and is tested against a recorded rollout, no live run) and *refused, with a reason*. This
variant is **below all three**: it is the instruction surface without the reader. Nothing here
should be read as W4.4 having landed.

## What this does not check

Named on the way in, so a reader does not find them by their absence.

* **Whether the instructions work.** No model is called. That the guardrails are in the prompt is
  not evidence that they are followed — the Claude Code plugin needed a paid run and an adversarial
  reviewer to say anything about that, and this variant has neither.
* **Whether a Codex hook would enforce them.** No hook is shipped; see the parent README
  § *What is deliberately absent*.
* **The `AGENTS.md` walk.** Only a root `AGENTS.md` is exercised. Nested files, `AGENTS.override.md`
  precedence and the root-first concatenation order are documented in the research record and are
  not asserted here.
* **The plugin install route.** The manifest is validated; it is never installed. Installing would
  write to the operator's `~/.agents/plugins/marketplace.json` and Codex configuration, and a check
  that mutates the machine it is auditing is not a check.
* **Whether `codex debug prompt-input` works offline.** It ran with no credential in a scratch
  `CODEX_HOME`; whether it reaches the network for anything it prints is unverified.
