---
format: aep.planning-md/1
id: story:plan-map-coverage
kind: story
status: implemented
title: A step map is refused when it cannot mint what the plan will demand
summary: The launch check that compares a step map against the plan it will drive, so a run cannot spend a model budget discovering a gap two documents already stated.
owner: harness
tags:
- driver
- harness
relations:
- decomposes: epic:reference-driver
revision: 5
---
# Story: A step map is refused when it cannot mint what the plan will demand

## Outcome

Somebody starting a driven run against a map that can never satisfy its plan is told so **at launch,
in the shell, for free** — one line per evidence kind nothing can produce, naming the principle that
asks for it and the transition it holds shut. Before this, the same person found out at a guard six
states in, after the model budget had been spent.

## Context

**F-W4.2-4**, `docs/design/fact-scoped-applicability-design-v0.1.md` § 9 — carried on the plan page
as F-W4.2-7 of `docs/plan/harness-wave-4-governed-dogfood.md` § W4.2, where the numbering differs and
the finding does not. Both call it the expensive one.

`StepMap::check_run` validated the map **against the protocol** — every kind a step declares is a
kind the protocol declares — and never the converse. Run `W4-2/1`, 2026-08-21/22, is the
measurement: ten model sessions, 333 turns, 76 minutes and **$31.46** to reach
`adversarial_verify -> review` and read `evidence.missing = 2`. The two records it wanted were a
`specification` and an independent `verification`, and no step of `development/checks` declares
either kind. Every fact needed to say so was in two documents on disk before the run started.

The wave that found it also fixed the applicability half (`change.code`), which is why the number is
2 rather than 4 — and that fix is the reason this check has to respect scoping rather than compare
raw sets. A launch check that ignored `applies_when:` would refuse every documentation task in this
repository for two rules that do not apply to it.

## Acceptance

- `protocol drive run` computes, before the first step executes, the evidence kinds the resolved plan
  can demand and the kinds the map can produce, and **refuses** with exit 1 when the second does not
  cover the first.
- The refusal prints **one line per missing kind**, each naming the principle or document that
  demands it and what it blocks — a transition by name where that is knowable, `completion`
  otherwise — and closes with what to do about it. A refusal that does not answer the question it
  creates is a wall.
- **Applicability is respected.** A task declaring `change.code: false` is not refused for
  `contract_result` or `property_test_result`; a task declaring nothing **is**, because Unknown is
  not False and silence is not an exemption (invariant 5).
- **No false refusal on an unreachable rule.** A requirement written on a state nothing can walk into
  refuses nothing, and a conditional whose `when` the task's own facts make `False` demands nothing.
- **Undecidable is a warning, never a refusal.** A record only a person can produce — a human
  verifier, or the `approval` and `review` kinds the driver may not mint at all under invariant 7 —
  prints and does not block. So does a demand pinning a verifier no declaring step names, because a
  record's producer is fixed when the step runs.
- The check reproduces `W4-2/1`'s own numbers from documents alone: **2** missing kinds with
  `change.code: false`, **4** with nothing declared, for both shipped maps.
- A map that does declare a producer for every demanded kind launches, so the check is passable
  rather than a wall with no door.
- `task check` is green.

## What shipped, 2026-08-22

- `aep_engine::demanded_evidence(&ExecutionPlan)` — a read-only walk of the plan returning every
  evidence requirement it can demand, with the document that asked, the conditional branch that
  reaches it, whether it feeds `evidence.missing`, and a typed `Blocked` list. It sits in
  `crates/aep-engine/src/evaluate.rs` beside `completion_requirements`, deliberately: the two walk
  the same three sources and a drift between them would be invisible in separate files.
- `aep_driver::evidence_coverage(&ExecutionPlan, &StepMap)` — the set arithmetic and the report.
- `protocol drive run` refuses on a non-empty gap; `--allow-evidence-gap` is the acknowledged way
  through, and it weakens no rule the engine enforces — the gap is still printed and the run still
  blocks at the guard.
- `Workflow::reachable` became public so one traversal answers both validation's question and this
  one.

**Measured on the tree at this commit, and recorded rather than fixed:** neither shipped map passes.
`drivers/development/default.yaml` and `drivers/development/checks.yaml` both report `verification`
and `specification` missing under `development.driven` with `change.code: false`, and those two plus
`contract_result` and `property_test_result` with nothing declared. That is exactly what `W4-2/1`
measured at its guard. Closing it means adding steps to two documents, which is a decision and a
separate change — see *Out of Scope*.

## Out of Scope

- **Amending the two shipped maps** so they pass. Which verifier writes a `specification` record and
  which writes an independent `verification` one is a documents decision, and making the maps green
  inside the change that measures them would be marking one's own homework.
- **Predicate obligations.** `profiles/development-standard.yaml:38` asks for `contracts.failed == 0`
  — a fact only a `ContractResult` projects — and nothing in this workspace maps a fact path back to
  the kinds that project it. Building one here would be a second copy of `Evidence::facts`, which is
  how a fact and a requirement come to disagree. The check is blind to this class and says so in its
  own module documentation.
- **`protocol drive resume`.** A resume continues a run whose launch already answered the question,
  and refusing there would strand work. The plan is re-resolved on every resume, so a newly-appeared
  gap is currently silent.

**Owed and not done here, so the debt is named rather than assumed.** Three control documents still
describe this finding as open: `AGENTS.md`'s proposed-design table calls F-W4.2-4 *the expensive
one*, `docs/plan/harness-wave-4-governed-dogfood.md` § W4.2 carries it as F-W4.2-7, and
`docs/design/fact-scoped-applicability-design-v0.1.md` § 9 lists it among six follow-ups. This story
does not own those files.

## Open Questions

Whether `--allow-evidence-gap` should exist at all. Decides: the protocol owner. **Default if nobody
answers: it stays.** The refusal it bypasses is economic and not protocol — the engine's
`evidence.missing == 0` guard is untouched, so the flag buys a run that stops at the same place it
would have stopped anyway, at the cost the caller has just been shown. Removing it would also make
every fixture in `crates/protocol-cli/tests/drive_cli.rs` carry two steps that write evidence
documents nobody reads.
