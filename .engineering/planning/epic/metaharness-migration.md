---
format: aep.planning-md/1
id: epic:metaharness-migration
kind: epic
status: active
title: Everything harness-shaped leaves this repository for metaharness
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 4
---
# Epic: Everything harness-shaped leaves this repository for metaharness

## Outcome

This repository states protocols, workflows, step maps and trace expectations, and drives them
through one neutral seam; nothing in it constructs a vendor argv, installs a vendor hook, or
assembles a hermetic scratch home. All of that lives in `beyond10x/metaharness`, behind
`metaharness run <kind>` and the sealed `metaharness.frame/1` document — so a second harness is a
metaharness adapter, never a second copy of this repository's enforcement.

## Why Now

Goal set by the operator, 2026-08-22. The seam is no longer hypothetical:
`story:metaharness-executor` drives real steps through the binary with a cross-verified frame
document, and metaharness M2 proved the denial path against a paid session from the vendor's own
record. Every harness-specific line that stays here from now on is a second, weaker copy of policy
metaharness already enforces — the exact failure `W4-2` paid to demonstrate when a forgotten
`--plugin-dir` silenced the hook copy while the run looked clean.

## Scope

Migration waves, in dependency order; each is a story when picked up:

1. **Ask mode** — the executor moves to `--decisions ask` streamed over the binary seam:
   `Engine::authorize(&mut Execution, …)` answers each `tool.requested` at decision time, and the
   hooks' per-argument narrowing (one program, two verbs, no pipes) returns as embedder code.
   Removes the last capability the shell hooks have that the seam does not.
2. **The eval moves** — `integrations/claude-code/eval/run.sh` sections 1–2 (scratch home,
   credential copy, env scrub, flags) become `metaharness run claude --hermetic strict`, the trace
   check becomes `--audit --spec … --auditor protocol`, and `run-driven.sh`'s denial census reads
   `tool.decided` events instead of `hook-decisions.jsonl`. The deliberate-denial case is kept.
3. **The hooks retire** — after waves 1–2 prove parity, `integrations/claude-code/hooks/` is
   deleted here; the enforcement register rows move their mechanism column to the seam.
4. **The bare argv retires** — driven maps default to `harness: metaharness`; `claude_argv` and
   its settings/transcript plumbing leave `drive.rs`.
5. **Codex arrives as an adapter** — the 2026-08-21 Codex research (rollout JSONL, portable
   `PreToolUse` hook, 0.145) is implemented as `metaharness-codex`, not as a second executor
   here; `epic:cross-harness-portability`'s acceptance is run through it.

## Out of Scope

- The plugin's **skills and agents** (`integrations/claude-code/skills/`, `agents/`): they are
  this repository's product for a person using Claude Code, not harness-driving machinery.
  Revisit only if metaharness grows a skill surface. Decides: operator.
- `workflows/`, `drivers/`, `trace-spec` and the expectations files: the documents and the IR are
  this repository's domain; metaharness projects into `trace-ir/1` and never rivals it (its D1).
- The plan→map coverage refusal (F-W4.2-4): governance machinery, not harness machinery; it stays
  here and is sequenced independently.

## Open Questions

- Where the ask-mode tool-name → `ActionRequest` translation lives (carried from
  `story:metaharness-executor`). Decides: whoever builds wave 1.
- Whether a paid parity run (one driven step, both executors, same map) is required before wave 3
  deletes the hooks, or the free tiers suffice. Decides: operator, at wave 3's acceptance.
