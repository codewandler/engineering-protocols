---
format: aep.planning-md/1
id: story:metaharness-executor
kind: story
status: implemented
title: 'The metaharness executor: one policy, one enforcer'
relations:
- decomposes: epic:metaharness-migration
revision: 5
---
# Story: The metaharness executor — one policy, one enforcer

## Outcome

An operator who drives a step with `harness: metaharness` gets a session whose per-state surface is
enforced by the metaharness seam itself, so a run can no longer look clean while running unenforced
— the failure mode run `W4-2` paid for eight sessions to expose when a resume forgot `--plugin-dir`.

## Context

Accepted directly by the operator, 2026-08-22 ("CONTINUE with meta-harness impl and integration"),
against §§ 10.1–10.3 of the metaharness protocol design
(`beyond10x/metaharness`, `docs/design/metaharness-protocol-v0.1.md`). The integration is over the
**binary seam**: this repository is public and metaharness is not, so no Cargo dependency crosses —
the shared artifact is the sealed `metaharness.frame/1` document (metaharness amendment a5), and
the working directory travels as the a6 `--cwd` declaration. metaharness commits `9dd3e61`,
`edacc3a`, `c27bdef` are the other half.

## Acceptance

- `run_llm` selects a second executor by the existing harness-name seam (§ 4.9 point 3); the
  default `claude-code` path is byte-for-byte unchanged. **Met**: `drive.rs`, gate exit 0.
- The step's surface travels as a sealed frame document whose digest metaharness verifies without
  this repository linking its crates. **Met and cross-verified**: an externally sealed document
  passed resolution against the real binary and failed only on the deliberately missing prompt; a
  corrupted byte was refused naming both digests.
- The operation rendering mirrors `allowed_tools` decision-for-decision, including `subagent.spawn`
  never being offered. **Met**: unit tests beside the `allowed_tools` ones.
- Denials arrive in the event stream the executor writes as the transcript, not in a side-channel
  log a forgotten flag can silence. **Met by construction**: `--decisions frame`; the plugin's
  hooks no-op without the step-context environment metaharness's H3 scrub drops.

## Out of Scope

- `--decisions ask` with `Engine::authorize` called per call inside the driver — the full § 10.1
  shape, and with it the hooks' per-argument narrowing (one program, two verbs, no pipes). Frame
  mode admits or refuses whole operations; this is stated in the executor's own doc comment.
- Any change to the eval scripts (§ 10.2) or a driven run over the real step maps (§ 10.3).
- A paid live run through the new executor. Nothing here spends money; the cross-check used
  refusal paths only.

## Open Questions

- ~~When the executor moves to `ask` mode, where does the tool-name → `ActionRequest` translation
  live — the executor, or a table beside `allowed_tools`?~~ **Answered 2026-08-22: a table beside
  `allowed_tools`.** `action_for` in `crates/protocol-cli/src/drive.rs` renders one call as the
  action it is, immediately below the function that renders a capability set as tool names — the
  two are the same seam read in opposite directions, and splitting them would put half of adapter
  point 2 in the executor. It decides nothing: which capability an action needs stays
  `Action::required_capability`'s. Two offered tools render to nothing on purpose — `Skill`, which
  takes no action, and `WebSearch`, which names no URL a `NetworkRequest` could honestly carry —
  and for those the engine is not consulted.
