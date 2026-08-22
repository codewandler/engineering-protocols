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

Migration waves, in dependency order. The operator widened the goal on 2026-08-22: the eval does
not merely move *onto* the metaharness binary, it moves *into* the metaharness repository —
logic, recorded transcripts, contracts, results and metrics alike.

1. **Ask mode** — **delivered 2026-08-22.** Every `llm` step streams through
   `metaharness run claude --decisions ask`; `decide_tool` in `drive.rs` answers each
   `tool.requested` at decision time — the two shell hooks ported case for case, plus the
   per-state allowlist that used to ride on `--allowedTools`. Not yet wired:
   `Engine::authorize` at decision time (every decision so far is a refusal, and a refusal
   changes no engine state); the wiring lands with the first case where a decision would.
2. **The eval moves — into metaharness** — **delivered 2026-08-22.** The whole of
   `integrations/claude-code/eval/` lives at metaharness `evals/engineering-protocols/`:
   `run-driven.sh` reads the census from `tool.decided` events, `run.sh` is retired with its
   subject, the agent-eval checks and recorded transcripts moved intact, and the deliberate-denial
   case is kept. The trace-spec join is suspended by name until a `metaharness.event/1` trace
   adapter exists (`story:event-stream-trace-adapter`).
3. **The hooks retire** — **delivered 2026-08-22.** `integrations/claude-code/hooks/` is deleted;
   the plugin is skills and agents; every live document that named the hooks now names the
   driver's policy.
4. **The bare argv retires** — **delivered 2026-08-22.** `claude_argv`, the settings file and the
   step-context side channel left `drive.rs`; `harness: claude-code` and `harness: metaharness`
   both reach the one executor.
5. **Codex arrives as an adapter** — the 2026-08-21 Codex research (rollout JSONL, portable
   `PreToolUse` hook, 0.145) is implemented as `metaharness-codex`, not as a second executor
   here; the codex integration residue is migrated to metaharness `evals/codex/`.

## Out of Scope

- The plugin's **skills and agents** (`integrations/claude-code/skills/`, `agents/`): they are
  this repository's product for a person using Claude Code, not harness-driving machinery.
  Revisit only if metaharness grows a skill surface. Decides: operator.
- `workflows/`, `drivers/` and `trace-spec`: the documents and the IR are this repository's
  domain; metaharness projects into `trace-ir/1` and never rivals it (its D1). The three trace
  expectation documents moved to `conformance/trace/` — domain specifications, not eval machinery
  — and the migrated eval carries its own copies.
- The plan→map coverage refusal (F-W4.2-4): governance machinery, not harness machinery; it stays
  here and is sequenced independently.

## Open Questions

- Where the ask-mode tool-name → `ActionRequest` translation lives (carried from
  `story:metaharness-executor`). Decides: whoever builds wave 1.
- Whether a paid parity run (one driven step, both executors, same map) is required before wave 3
  deletes the hooks, or the free tiers suffice. Decides: operator, at wave 3's acceptance.
