# Harness wave 4 — the repository governed by its own driver

> **Status: proposed. W4.1 has been run once — `W4-1/1`, 2026-08-21, and it stopped short of the
> person it was meant to stop at.** The record is in § W4.1 below, under *The first run*; the wave's
> other three items are untouched by it. Nothing about that run changes this page's standing: it is
> still a proposal, and a proposal is not a work order however much of it has now happened.
>
> **Its predecessor has shipped.**
> Wave 3 — the driver build, W3.0–W3.6 — is **delivered, 2026-08-21**: the acceptance for every item
> is in [`harness-wave-2-driver-decision.md`](harness-wave-2-driver-decision.md) § *Wave 3 — built,
> 2026-08-21*, together with a seventh item the review never saw (`protocol workflow render`). Every
> acceptance line below therefore rests on code that exists rather than on a crate being written —
> which changes what this page is waiting for, and changes nothing about its standing. Nothing here
> is a work order: [`AGENTS.md`](../../AGENTS.md) § *Which documents are normative* says a proposal
> is not one however recent it is.
>
> Design: [`harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md)
> §§ 4.7–4.9 — the corrected wave-2 decisions. Review:
> [`2026-08-21-driver-feasibility-review.md`](../reviews/2026-08-21-driver-feasibility-review.md).
> W4.3 proposes a second design,
> [`story-completion-evidence-design-v0.1.md`](../design/story-completion-evidence-design-v0.1.md),
> and **accepting or refusing it is one of this wave's acceptance criteria** — not one of its builds.

**Goal: the first actually governed task. One real story in this repository is picked up, worked and
closed by a run in which every transition was permitted by the engine, every gate was a real gate,
and the record of how the agent worked was read by a program rather than accepted from the agent.**

Wave 1 built the store. Wave 2 took the decisions. Wave 3 built the driver and proved the adapter
seam against a harness with no model in it, then ran a real model through a two-step eval.
**Nothing so far has been run against work that mattered**, and that is the one gap the previous
three waves cannot close by construction: a driver that has only ever driven a fixture and an eval is
a driver whose step maps were written to fit them.

## What this wave is, in one sentence each way

For the person adopting this: the difference between a repository whose rules are in a file that
agents are asked to read, and a repository where the story you are working on cannot reach
*implemented* until something other than the agent said it was — and where the reason a run stopped
is printed as a refusal you can act on rather than discovered three weeks later in a review.

For the machinery: wave 3's acceptance was `task check` green with a shell-echo harness standing in
for a model (W3.5), plus a driven eval whose two model sessions worked a scratch store (W3.6). Wave 4
removes every remaining stand-in at once — a real store, a real story, a real step map with `cargo`
in it, the real ten-step gate (`Taskfile.yml:16-25`) and a real person at the `operator` step — and
reports what broke.

## Decisions, taken

| decision | taken as | why |
|---|---|---|
| **dogfood before portability** | W4.1 (one real story, driven) lands **before** W4.4 (a second real harness). Codex is last in this wave, not first | **an adapter proven on toy tasks tells you less than a driver proven on real work.** W3.5 already tests the *seam* with no model, so the marginal information in a second adapter is about the harness, not about the specification. The marginal information in a real story is about everything at once: the step map, the retry budgets, the approval pre-flight, the hook channel and whether an operator can resume. Adding a second harness first would double the surface under test before the first one has ever been run against something that could fail for a real reason |
| the subject is **this repository's own backlog** | W4.1 drives a story out of the `.engineering/` store W4.0 landed — 33 artifacts under `initiative:the-repo-governs-itself`. `examples/planning-passkeys/` stays a fixture and is not driven | a fixture is a story somebody wrote to be drivable, so it cannot refute the claim under test. The claim is that a real story — with a gate that takes minutes, a diff across crates, and an acceptance line somebody argued about — is drivable |
| **one small real story, not a representative one** | **named, 2026-08-21: `story:agent-eval-cases`** — 48 lines, and the smallest story in the store that both touches no crate (`integrations/claude-code/**`) and is not parked on somebody else's gate | a large story tests the model's stamina and the wave's patience, not the driver's enforcement, and it fails for reasons this wave cannot fix. The wave is about whether the *mechanism* holds. The disjoint surface is a second, smaller reason: a story whose diff lands where nothing else is being worked on is a story whose *failure* can only have come from the driver |
| the operator is **in the loop by design**, not as a fallback | ~~W4.1 runs `development.standard`~~ **`development.driven`, with `--pause-on-approval`, taken 2026-08-21** — the choice the inline note in § W4.1 left open. The review is still an `operator` step and the pause is unchanged, because `development.driven` extends `development.standard` | D3: a headless run **refuses to start** when an approval is reachable, and `approval-gates` is reachable under both profiles. The two ways to avoid the pause are both refused — dropping to `development.fast`, which deliberately cannot summon a human (`profiles/development-fast.yaml:25-27`), would test a weaker profile than the work deserves; auto-approving is refused under every flag by D3. What forced the change off `development.standard` is not the pause but the shell: without `command.execute` a driven `llm` step cannot reach a single `protocol artifact` verb, and run `W4-1/1` made **48 allowed calls** through exactly that grant — 47 `protocol artifact`, one `protocol trace` |
| ~~**the F13 answer is produced, not scheduled again**~~ **— produced by wave 3, and this row is kept as the record of why it was made an acceptance criterion** | **W3.6 ran the deliberate-denial case on 2026-08-21 and the answer is *yes, one-for-one*:** three hook refusals produced exactly three `permission_denials` entries, each naming its tool. It is written into design § 4.8 (*F13, answered*) and the gap-register row is **closed by code**. W4.2 is no longer the backstop for it | the review named the closing command in one sentence — one `claude -p` run with a denying hook, then read the last line — and a row whose closing command has been written down for two waves and never run is a row nobody intends to close. Making it an acceptance criterion is what got it run one wave earlier than this page expected |
| **W4.3 produces a decision, not a build** | the design is written proposed-not-accepted; the wave's acceptance is *accepted / accepted-in-part / refused, with the reason recorded* | both shapes it could take are domain changes — a new `ArtifactStatus` variant, or a new mode on a write verb. That is the shape gap-register **D-5** already went through for `EvidenceKind`, and the lesson recorded there is that the decision belongs in the acceptance decision rather than being discovered during implementation |
| the Codex facts are **an input, not a dependency** | the research was run in parallel and **has landed** — [`2026-08-21-codex-harness-research.md`](../reviews/2026-08-21-codex-harness-research.md), every fact labelled verified / documented / inferred / unknown. W4.4's three acceptance tiers stay, because a wave whose last item blocks on research nobody sequenced is a wave that does not close | it changed two things rather than confirming the plan: the adapter's input is the **session rollout JSONL**, not `codex exec --json` stdout, and the enforcement layer turns out to be **portable rather than Claude-specific** — Codex 0.145 ships a stable `PreToolUse` hook with the same decision contract. Both are recorded in W4.4 below rather than restated |
| **no new enforcement mechanism in this wave** | wave 4 runs what wave 3 built. The only new code it authorises is W4.4's adapter; W4.3's build, if accepted, is a later wave | a dogfood wave that also grows the enforcement surface cannot say which half its findings came from. The point of running the thing is to learn what the thing does |
| a wedged run is a **result**, not a retry loop | W4.1's acceptance admits failure as an outcome, on the record | a dogfood wave that reports only its successes is marketing. The repository's own standard — *a rule nothing checks is a rule that has already drifted somewhere* — has an analogue here: a mechanism nobody was allowed to report a failure of is a mechanism nobody has tested |

## W4.0 — the repository's own `.engineering/` store

**Built in parallel and landed; this page reads it rather than owning it.** The wave-1 markdown store
pointed at this repository: `.engineering/planning/<kind>/<slug>.md`, the four planning lifecycles,
and the ids the wave-1 scheme allocates by slug and never by counter.

This has an ancestor. [`wave-4-dogfooding.md`](wave-4-dogfooding.md) § W4.2 asked for
`.engineering/` for this repository in the AEP wave 4 and got the **discovery** half — `0.2.1`,
project discovery, in the delivered table (`docs/status.md`). The *planning* half did not exist
until harness wave 1 built the backend, and it is the half wave 4 needs, because a driver with no
store has no artifacts to evaluate a gate against (D2).

**Acceptance — met, 2026-08-21, by the parallel build.** The evidence is the commands, not the
claim:

```text
$ protocol artifact validate
33 file(s) in …/engineering-protocols/.engineering/planning: 33 artifact(s)
valid                                                                  # exit 0

$ cd crates && protocol artifact list --kind epic
epic:reference-driver           epic  draft  The reference driver
epic:cross-harness-portability  epic  draft  Harness-neutral, and tested by a second harness
epic:evidence-gated-completion  epic  draft  Done is a claim with evidence behind it
…                                                                      # exit 0, no --store
```

* `protocol artifact list` run from a subdirectory, **with no `--store`**, answers from
  `.engineering/planning/` — the discovery path, not a flag
  (`crates/protocol-cli/src/planning.rs:90-106`);
* `protocol artifact validate` is green over 33 artifacts, exit 0;
* the store holds **this wave's own stories**, so the first thing the repository governs with it is
  the plan that governs it — see the mapping below;
* **one decision left, and it is not met by the above: whether `artifact validate` joins the gate.**
  *Default: yes.* It is local, clock-free and sub-second, which is the same argument `status-check`
  is placed on (`Taskfile.yml:41-52`). If it joins, the gate is **eleven** steps and `AGENTS.md`
  § *Gate* gains its row in the same change — a gate whose step list disagrees with the Taskfile is
  the drift invariant 1 exists to prevent, one directory over. Until it does, a store that stopped
  validating would be found by whoever ran the verb next, which is nobody in particular.

**This page's items are artifacts in that store**, which is the smallest honest form of the wave's
own claim — a plan page that talks about a governed backlog and is not in it would be arguing for
something it does not do:

| wave item | artifact | status today |
|---|---|---|
| W4.0 | `story:own-engineering-store` | `active` |
| W4.1 | `story:governed-dogfood-run` (`depends_on: story:own-engineering-store`) | `draft` |
| W4.2 | `story:retry-budgets`, `story:operator-resume-ux`, and the denial case inside `story:driven-eval-acceptance` | `draft` / `draft` / `proposed` |
| W4.3 | `story:completion-needs-evidence`, `story:completion-audit-join`, under `epic:evidence-gated-completion` | `draft` |
| W4.4 | `story:codex-adapter`, under `epic:cross-harness-portability` | `draft` |

All of it sits under `initiative:the-repo-governs-itself`. **The mapping is not a claim that this
page and the store cannot drift** — nothing checks it, and until something does, it is two copies of
one plan. Making the store the single copy is what W4.1 is for.

## W4.1 — one real story, driven end to end

The centre of the wave, and the only item the others exist to support.

`protocol drive` over `drivers/development/default.yaml` (W3.2) under `development.standard`: one
`claude -p` session per `llm` step (D4), the tool set at each state from
`tool_config(effective_policy(execution))` on the launch line, the plugin's hooks configured through
`--settings` and **never** `--bare` (D4/F15), `cargo test` and `clippy` as `command` steps **the
driver executes** — because no development profile grants `command.execute`, so the model holds no
shell at any point in the run (§ 4.8) — and the review as an `operator` step that persists, releases
the lock and exits 0 (D3).

> **One input this paragraph predates, from wave 3's delivery: `development.standard` is very likely
> the wrong profile for a driven run, and this page is not the place that decides it.** Building the
> driven eval found that the planning store has no tool surface other than the `protocol` CLI, so
> under `development.standard` a driven `llm` step cannot create an artifact at all — the run does
> not fail, it never moves. `development.driven` exists for exactly that (design § 4.8, and the
> profile's own header), and `cargo test` and `clippy` stay `command` steps the driver executes under
> either. Whichever this wave picks is a decision it takes when it opens, with the consequence for
> the review step — `development.driven` extends `development.standard`, so the `approval-gates`
> pause D3 relies on is unchanged.

**Acceptance:**

* **every transition is evidence-permitted.** No state is entered except through
  `Engine::transition` returning `Moved`; every gate the driver wanted and the engine refused appears
  as a `TransitionBlocked` with one reason per unmet requirement
  (`crates/aep-engine/src/engine.rs:397-415`), and the run report names each. The assertion is
  against the snapshot's audit trail, not against the driver's own log;
* **the diff lands through the ten-step gate.** `task check` green, exit 0, before the review step
  (`Taskfile.yml:16-25`), and the `test_result` and `static_analysis` records submitted to the engine
  are the ones that run produced — not a summary the model wrote about it;
* **the transcripts are checked, and the run cannot complete without it.** `protocol trace check`
  over each `llm` step's transcript against a trace specification, `protocol trace evidence`
  submitting `trace_conformance`, and the completion gate reading it. `trace_conformance` and
  `trace-checker` are declared for development work and nowhere else
  (`protocols/adp/1.yaml:17-33`), which is what makes this admissible at all;
* **every status move goes through `protocol artifact move` and by no other means** — asserted by
  inspecting the store afterwards, the way W1.2's eval asserts it, with the
  `.engineering/planning/**` write-guard hook as the enforcement and `artifact validate` as the audit
  (§ 4.8 row 6);
* **the run is readable by somebody who was not there.** `.engineering/runs/<run-id>/` carries the
  cursor, the transcripts, the hook decision log and the report, and a second person can reconstruct
  what happened without asking anyone;
* **what is deliberately not claimed: byte-repeatability.** Two runs of the same story produce
  different transcripts and different digests — D4 says so, and the resume rule says a resumed run is
  a new session whose digest differs. Every assertion here is over the store, the audit trail and the
  gate's exit code, never over the model's prose, which is the rule W1.2 already set and the same
  reason § 4.8 states *text is free*;
* **a run that wedges is a recorded result.** If the driver cannot get a real story through, the
  wave records where it stopped, what the cursor said and which decision was wrong. That outcome
  closes this item; quietly retrying until it works does not.

### The first run — `W4-1/1`, 2026-08-21: **blocked in `establish_verifiers`, and the last clause above is the one it lands on**

The run happened. It was not faked, not scoped down and not retried until it worked, and **it did not
reach the person it was supposed to stop at** — it stopped four states short of `review`, for two
reasons the engine printed and neither of which is a defect in the engine. That is the outcome the
acceptance line above admits, so this item is closed by it rather than left open, and everything
below is the record the line asks for.

```text
$ protocol drive run --project . --plugin-dir integrations/claude-code \
    --pause-on-approval --max-iterations 40            # no --map: the shipped map is selected by fitting
run        W4-1/1
map        step map development/default
status     blocked
state      establish_verifiers
steps      5 run, 1 submitted
moved      receive -> specify
moved      specify -> decompose
moved      decompose -> establish_verifiers
blocked because:
  - establish_verifiers -> implement: ? artifact specification (approved) — declared: specification:agent-charter-eval-cases (draft) [principle spec-driven]
  - establish_verifiers -> implement: ✗ test.first_result == failed — test.first_result = passed [principle test-driven]
resume with: protocol drive resume W4-1/1                                                  # exit 1
```

**The subject.** `story:agent-eval-cases` — *"The two planning agents, held to their charters by a
run"*, `decomposes: epic:self-evaluation`, 48 lines. Its implementation surface is
`integrations/claude-code/**`, which is what selected it: of the two stories in the store that are
both near-smallest and touch no crate, the other — `story:native-plugin-eval` — says in its own Open
Questions that it *"stays in draft until"* an early-access gate opens, so driving it would have
measured a gate somebody else holds. The
task document is `.engineering/task.yaml`, `id: W4-1`, `kind: feature`, `derived_from:
story:agent-eval-cases`, and it declares `profile: development.driven` — which is the decision the
inline note above left to whoever opened the wave, taken the way that note predicted, and confirmed
by the run: `protocol resolve` reports `command.execute` allowed, and all 48 CLI invocations the four
sessions were allowed — 47 `protocol artifact`, one `protocol trace` — went through the shell that
grant opens.

**What ran, per state.**

| state | steps | outcome |
|---|---|---|
| `receive` | 1 `llm` | created `task:w4-1-agent-eval-cases` through `protocol artifact new`, body written by targeted `Edit`. Moved |
| `specify` | 1 `llm` | created `specification:agent-charter-eval-cases`, status `draft`. Moved on `artifact.specification.exists` |
| `decompose` | 1 `llm` | created **9** `task` artifacts, each related through `protocol artifact relate`. Moved (unguarded) |
| `establish_verifiers` | 1 `llm` + 1 `command` | wrote **nine red shell checks — one per decomposed task** — under `integrations/claude-code/eval/checks/`, then the driver ran `cargo test --workspace`: **138 suites `ok`, 0 `FAILED`**. **Blocked** |
| `implement` … `review` | — | never entered. The `operator` step was never reached |

**The numbers, all read out of the run's own records.**

| quantity | value | where it is |
|---|---|---|
| model sessions | 4, `is_error: false` in every one | `.engineering/runs/W4-1/1/transcripts/*.jsonl` |
| resolved model | `claude-opus-5[1m]` — the CLI's default; the driver passes no `--model` | `claude_argv`, `crates/protocol-cli/src/drive.rs:1178-1211` |
| turns / wall clock / cost | 224 turns, 34 m 39 s of session time, **$15.42** | terminal `result` events |
| hook decisions | **80** — 69 allow, 11 deny | `hook-decisions.jsonl` |
| … by hook | `driven-surface` 48 allow / 10 deny; `store-integrity` 21 allow / 1 deny | same |
| `permission_denials` | **11**, summing the four terminal records: 3 / 3 / 2 / 3 | same four transcripts |
| evidence submitted | **1** — `test_result`, `suite: unit`, `passed: 1`, `producer: verifier/test-runner`, `command: cargo test --workspace` | `snapshot.json` |
| audit trail | 11 events: 3 `transition_performed`, 1 `evidence_produced`, 1 `transition_blocked` carrying both unmet reasons | `snapshot.json` |
| store afterwards | 47 → **58** artifacts, `protocol artifact validate` **exit 0** | the verb |
| files outside the intended surface | **0** — no tracked file was modified at all; every write is a new file under `.engineering/**` or `integrations/claude-code/eval/checks/**` | `git status` |

**What held, stated as claims with their evidence.**

* **Every transition was the engine's.** Three `Moved`, one `TransitionBlocked` with one line per
  unmet requirement, all in the snapshot's audit trail rather than in the driver's log.
* **The guards discriminated.** 69 allows and 11 denies is the shape the driven eval demands of
  itself — a guard that denies everything audits as little as one that denies nothing. Every deny is
  a `Bash` outside `protocol artifact …`/`protocol trace …` (`ls`, `find`, `cat`, and composed
  command lines), except one: a `store-integrity` refusal of an `Edit` whose `new_string` rewrote
  the machine-owned `id:` of an artifact's frontmatter.
* **F13 holds on the shipped map, not only on the eval's.** 11 hook denies, 11 `permission_denials`
  entries, one-for-one, each naming its tool — a second, independent confirmation of the answer
  W3.6 produced, on a different step map and a different model.
* **The auth was the login.** `apiKeySource: none` in all four sessions, with `ANTHROPIC_API_KEY`
  unexported and `CLAUDE_CONFIG_DIR` pointed at a scratch home holding a copy of the credentials.
* **`protocol trace check` decides these transcripts.** Run against
  `eval/expectations.driven-step.trace.yaml` — a document written for the eval's step, not for these
  — `decompose` is **conformant** (9 ok, 2 advisory gaps) and the other three contradict
  `nothing-was-badly-refused-here`, whose bound is `at most 2` denials. `protocol trace evidence`
  minted a `trace_conformance` record from the `decompose` transcript, `status: passed`,
  `producer: verifier/trace-checker`.

**What the run found, and it is about the step map rather than about the enforcement.** Five things,
in the order they cost the run:

1. **`drivers/development/default.yaml`'s only verifier is `cargo`, so a story whose tests are not
   Rust tests cannot satisfy `test-driven` at all.** The `establish_verifiers` `llm` step did exactly
   what it was asked — its own README says *"They are red, and red is the product"* — and wrote nine
   failing shell checks, because that is the idiom the chosen story's acceptance is written in. The
   step after it runs `cargo test --workspace`, which was green (138 `ok`), so `test.first_result`
   was recorded `passed`, and `test.first_result` is the **first** outcome ever recorded and never
   changes (`crates/aep-engine/src/execution.rs:366-378`). The run could not move afterwards by any
   route. **This is the map's problem and not the workflow's**: the map is where a repository says
   how evidence is obtained, and this one says `cargo` in every state that says anything.
2. **An `llm` step is told what must hold *in* its state and never what guards the way *out* of
   it.** `StepContext.requirements` is built from `Evaluation.requirements`
   (`crates/aep-driver/src/run.rs:672-676`), which is documented as *"what must hold while in this
   state"* (`crates/aep-engine/src/evaluate.rs:131-132`); the outgoing guard lives in
   `Evaluation.transitions[].requirements` and is not passed. So the model was never told that
   `implement` needed a red suite and an approved specification. It was not asked and it did not
   guess, which is the correct order of blame.
3. **Nothing in the run moves a specification to `approved`.** `protocol artifact new` leaves
   `draft`, `spec-driven.before_implementation` wants `approved`
   (`principles/development/spec-driven.yaml:20-25`), and the lifecycle is `draft → in_review →
   approved` — two `protocol artifact move` calls that no prompt asks for and no step performs.
4. **`diff.exists` is satisfied by `git diff` exiting zero, not by a diff existing.** Every file this
   run produced is new, so `git --no-pager diff --stat HEAD` would have printed nothing and exited 0,
   and `mint` writes a `ChangeSet` with all-zero counts on any zero exit
   (`crates/protocol-cli/src/drive.rs:1274-1281`, which says so in its own comment). The run never
   reached `implement`, so this one is a reading of the code rather than an observation of the run —
   labelled as such deliberately.
5. **The driver never checks a transcript, so this item's third acceptance bullet is not met by the
   run — only by a person typing the verb afterwards.** That bullet asks for `protocol trace check`
   over each `llm` step's transcript, `protocol trace evidence` submitting `trace_conformance`, and
   the completion gate reading it. `drivers/development/default.yaml` contains **no `trace` step at
   all**, so no `trace_conformance` record was minted by the run and nothing could have gated on one.
   The two invocations quoted above were run by hand against the finished transcripts, which
   demonstrates the verbs and not the gate. Closing this is a step-map change, and therefore the same
   decision as finding 1 rather than a separate one.

**One hermeticity gap, and it is not the one the eval guards against.** The `decompose` and
`establish_verifiers` sessions' init events list **three MCP servers** — `claude.ai Google Drive`,
`Gmail`, `Google Calendar`, all `status: needs-auth` — while `receive` and `specify` list none. There
is no `.mcp.json` in the tree and no `mcpServers` key in the scratch config home, so these are
account-level and arrive over the network: **a scratch `CLAUDE_CONFIG_DIR` cannot exclude them.**
Nothing was reachable through them — the tool inventory is 28 in all four sessions and no `mcp__*`
tool appears — and no expectation in any specification here asserts `mcp_servers == 0`, which is why
this was found by reading a transcript rather than by a gate.

**What this run does not say.** It says nothing about whether the mechanism can carry a story to
`complete`, because it did not carry one. It says nothing about the retry budgets W4.2 asks about:
**no step was retried, no state was re-entered, and no budget was touched** — `visits` is 1 for all
four states and every `attempts` entry is 1, so the three numbers stay guesses and W4.2's acceptance
line about them is untouched by this run. And it says nothing about `--pause-on-approval`'s resume
line, which was printed as `resume with: protocol drive resume W4-1/1` by the **blocked** path
(`crates/protocol-cli/src/drive.rs:611-613`) rather than by an `operator` pause, so W4.2's third item
— *"nobody has read that line"* — is still true of the line it means.

**Resuming it changes nothing on its own.** The cursor sits in `establish_verifiers` with both its
steps done, so a resume re-takes the lock, asks for the transition, is refused for the same two
reasons and exits 1 again. Making the run movable is a change to the step map, and *"changing a
workflow, a profile or a principle to make the run go through"* is on this page's own
**deliberately-not-in-this-wave** list. The next decision is therefore whether finding 1 is a defect
in `drivers/development/default.yaml` — a map that can only drive Rust changes in a repository whose
backlog is a third documents and plugin shell — and that is a decision, not a fix.

## W4.2 — hardening: the numbers wave 3 had to guess at

Three items, each of which is a decision taken in wave 2 with no observation behind it.

**Retry budgets against reality.** D5 fixes budgets *per step kind* — `command` retries, `llm` once,
`operator` never, spent and not reset, counted in the cursor. Wave 3 has since put **numbers** on
them, by argument rather than by observation: `DEFAULT_VISIT_BUDGET = 3`,
`DEFAULT_COMMAND_RETRIES = 2`, `LLM_RETRIES = 1`, the last deliberately not configurable
(`crates/aep-driver-spec/src/map.rs:57-69`, dispatched at `:354-360`), with a per-step `retries`
override available on a `command` step (`:244-246`). **W4.1 is what can say whether they are right** —
how often a `command` step failed for a reason a retry could fix, how often a retried `llm` step
produced anything different, and whether any state hit the visit budget.

**~~The F13 empirical answer, folded into the design.~~ Done by wave 3; this item is discharged.**
W3.6's deliberate-denial case ran on 2026-08-21 and answered it: a hook's `permissionDecision: deny`
**does** increment the transcript's `permission_denials` array, one entry per refusal, each carrying
the tool's name. It is folded into design § 4.8 and the gap-register row is closed by code. The field
is still a **whole-run count**, so the gating record stays the hook-decision log and `protocol
artifact validate`, and the transcript row stays advisory — which is a narrower claim than this item
was written expecting, and is the claim the observation supports.

**Operator-step resume UX.** D3 says an owed approval becomes an `operator` step that prints the
`CompletionExplanation` verbatim, persists, releases the lock and exits 0 with a resume line. Nobody
has read that line.

**Acceptance:**

* each of the three budgets is either **kept with an observation behind it** or changed, and this page
  names the run the number came from. *"Unchanged after one run"* is a result and is recorded as one;
  a budget nothing was measured against stays a guess whatever value it holds;
* ~~design § 4.8 row 1's audit column states **whether** a hook deny increments
  `permission_denials`~~ — **met early, by wave 3.** The column says *yes, one-for-one*, with the run
  that produced it named. Left here rather than deleted, because an acceptance line that vanishes
  once something else met it is a line nobody can check was met at all;
* an operator who **did not start the run** resumes it from the printed line alone, in a shell with
  no other context; the resume re-takes the lock (D6) and the cursor records that it did;
* nothing in this item changes an enforcement mechanism. Where the run shows one is wrong, that is a
  finding recorded here and a decision for a later wave — see the *decisions, taken* row above.

### A fourth item, added 2026-08-22 — **fact-scoped applicability: the rule that had no honest producer**

Not planned in wave 2. It was **found by the run below**, which is what a dogfood wave is for, and it
is written down here rather than in a later page because the finding and its fix arrived together.

**The finding, in one line.** `development.driven` obliged a *documentation* task to produce a
`contract_result` from a `contract-runner` and a `property_test_result` from a `property-tester`,
both `independent: true`. Neither verifier can observe prose. The task had **119 passing checks and
0 failures** and still could not leave `adversarial_verify`, because the only ways to satisfy those
two rules were to forge the record — which `independent: true` exists to forbid — or to strip them
from every task the profile governs, including the ones that do change code.

**The fix, and its exact size.** Two `applies_when:` clauses over one declared fact, `change.code`,
in `constraints.facts` beside the existing `change.public_contract`. It is
[`fact-scoped-applicability-design-v0.1.md`](../design/fact-scoped-applicability-design-v0.1.md),
**proposed, not accepted** — this section is the acceptance surface, and the verdict below is
*accepted in part*. No engine change, no protocol change, no new grammar and **no new enforcement
mechanism**, which is what keeps it inside this wave's third *decisions, taken* row: it narrows an
existing mechanism (`Principle::applies`, `crates/aep-domain/src/principle.rs:688-692`) using
grammar that `differential-testing.yaml` already uses.

**The Kleene posture is the whole safety argument and is unchanged.** A principle falls away only on
an explicit `False`; an *undeclared* `change.code` evaluates `Unknown`, and unknown applicability
leaves a rule in force. Silence is not an exemption — only a written `false` is, and only in a task
document under review.

**Verdict: accepted in part.** The two clauses ship. What the design proposed and this page refuses
to record as met is its own § 0 claim in its first draft — that the change would finish the run. It
does not. Measured, on the live run:

| | `evidence.missing` at `adversarial_verify -> review` |
|---|---|
| before | **4** |
| after | **2** |

An adversarial review of the design (one agent, read-only, 75 tool calls) returned **REFUTED** with
five blocking findings; four are folded into the document and the fifth — its false provenance
claim, that a plan page had proposed it — is corrected by this section existing. The two findings
worth carrying up here are in the table below as **F-W4.2-4** and **F-W4.2-7**.

### The second governed run — `W4-2/1`, 2026-08-21/22: **blocked in `adversarial_verify`, and the block moved but did not lift**

Unlike `W4-1/1`, this one **did** reach a person — at `establish_verifiers`, where the map puts an
`operator` step to get the specification approved. It stopped one state short of the person it was
written to stop at.

```text
$ protocol drive resume W4-2/1 --project <worktree> --map development/checks \
    --task .engineering/task-w4-2.yaml --max-iterations 60 --pause-on-approval
run        W4-2/1
map        step map development/checks
status     blocked
state      adversarial_verify
steps      0 run, 0 submitted
blocked because:
  - adversarial_verify -> review: guard: evidence.missing == 0
      evidence.missing = 2                                                                 # exit 1
```

**The subject.** `story:open-vocabulary-audit`, chosen because its product is a document and its
acceptance is checkable without a compiler — exactly the work `development/checks` exists for, and
exactly the shape `development/default` could never drive. The task is `.engineering/task-w4-2.yaml`,
`kind: feature`, `profile: development.driven`.

**What ran, per state.**

| state | steps | outcome |
|---|---|---|
| `receive` | 1 `llm`, **3 attempts** | two sessions died before a turn (see *credential*, below); the third created `task:w4-2-open-vocabulary-audit`. Moved |
| `specify` | 1 `llm` | created `specification:open-vocabulary-audit`. Moved on `artifact.specification.exists` |
| `decompose` | 1 `llm` | created **2 stories and 13 tasks**, related through `protocol artifact relate`. Moved |
| `establish_verifiers` | 1 `llm` + 1 `command` + **1 `operator`** | wrote **13 check units** under `.engineering/checks/`; the driver ran them **red** — `test_result` `passed: 0, failed: 1`, and the engine recorded `verification_failed`. Then it **paused for a person**, who moved `specification:open-vocabulary-audit` to `approved`; the run was resumed and moved |
| `implement` | 1 `llm`, **3 attempts** | two sessions died on an expired credential; the third wrote `docs/guide/open-vocabulary.md`, 165 lines, 18 audit rows. `trace_conformance` and `diff` submitted. Moved |
| `verify` | **3 `command`**, no model | checks green, `protocol validate` green, `protocol artifact validate` green — three records. Moved |
| `adversarial_verify` | 1 `llm` + 1 `command` | the adversary added checks; the suite ran **119 pass, 0 fail, 0 broken, 0 undeclared** across 13 units. **Blocked** |
| `review` | — | never entered |

**The numbers, all read out of the run's own records.**

| quantity | value | where it is |
|---|---|---|
| model sessions | **10** — 6 `is_error: false`, **4 `is_error: true`** | `.engineering/runs/W4-2/1/transcripts/*.jsonl` |
| turns / session wall clock / cost | **333 turns, 75.7 min, $31.46** | terminal `result` events |
| resolved model | `claude-opus-5[1m]` on the eight hermetic sessions; **`claude-opus-5`** on the two that leaked | `init` events |
| evidence submitted | **7** — 4 `test_result`, 1 `trace_conformance`, 1 `diff`, 1 `static_analysis` | `snapshot.json` |
| audit trail | **25** events: 6 `transition_performed`, 7 `evidence_produced`, 1 `verification_failed`, **2 `transition_blocked`** (the second is the resume) | `snapshot.json` |
| hook decisions | **0 — there is no `hook-decisions.jsonl`** | the run directory; `W4-1/1` has one with 80 |
| store afterwards | 59 → **76** artifacts, `protocol artifact validate` **exit 0** | the verb |
| the product | `docs/guide/open-vocabulary.md`, 165 lines, 18 rows; 13 check units, 119 checks | the worktree |
| rate-limit posture at the time | `seven_day`, **0.91 utilization**, `allowed_warning` | `rate_limit_event`, first transcript |

**Five findings. Each is a thing the run did, not a thing the design predicted.**

| # | finding | evidence |
|---|---|---|
| **F-W4.2-3** | **A raw launch leaves hermeticity to the caller, and the two things that fix it fight each other.** The first two `receive` sessions loaded **6 plugins — 5 of them the operator's, nothing to do with this run** (`rust-analyzer-lsp`, `gopls-lsp`, `typescript-lsp`, `track`, `flux-agent`), 26 skills instead of 16, and billed against **`apiKeySource: ANTHROPIC_API_KEY`** rather than the intended subscription: both died on *"Credit balance is too low"*. Pointing `CLAUDE_CONFIG_DIR` at a clean home fixed the leak — and removed the **`engineering-protocols` plugin too**, so the eight sessions that then succeeded ran with `plugins: 0` and **no enforcement hooks at all**. `development.driven` grants `command.execute` on the stated understanding that `driven-surface.sh` narrows it; for this entire run that hook was absent | `init` events of all ten transcripts; the missing `hook-decisions.jsonl`; `profiles/development-driven.yaml` header |
| **F-W4.2-4** | **`resume` re-reads none of its four flags — and there is a fifth.** `--map`, `--task`, `--pause-on-approval` and `--plugin-dir` must all be passed again; none is stored. **`--max-iterations` is cumulative over the life of the run and defaults to 25**, so resuming a run that already spent 25 iterations exhausts the budget *before evaluating anything*: the first resume returned `status budget-exhausted`, `steps 0 run`, having done nothing. The printed resume line — `resume with: protocol drive resume W4-2/1` — carries none of the five, which is the W4.2 operator-UX item above, answered by observation: **the line as printed does not work** | `crates/protocol-cli/src/drive.rs:238-253`; `resume-1` output; cursor `iterations: 26` |
| **F-W4.2-5** | **A copied OAuth credential expires mid-run and cannot refresh.** Two `implement` sessions returned *"Failed to authenticate: OAuth session expired and could not be refreshed"*. The SDK reported them as `subtype: "success"` with `is_error: true` in the same frame, which is worth knowing before trusting a summary field | `transcripts/implement-0-1.jsonl`, `-0-2.jsonl` |
| **F-W4.2-6** | **The applicability gap**, above. Closed in part | the design note |
| **F-W4.2-7** | **A step map is never checked against the plan it will drive, and this is the expensive one.** `StepMap::check_run` validates map → protocol — every evidence kind a step declares is one the protocol declares — and **never the converse** (`crates/aep-driver-spec/src/map.rs:710-750`). `development/checks` submits four kinds; the plan requires `specification` and `verification` as well, and **no step of the map produces either**, for a code task or a documentation one. So the map loads, the run walks six states, and the mismatch surfaces at the guard — **after $31.46 and 76 minutes of model time**. A load-time check had every fact it needed | the map's seven `evidence:` blocks; `evidence.missing = 2` after the fix |

**One interaction worth writing down before someone rediscovers it.** The story's own acceptance
criterion 7 became check **H3** — *git status lists changed paths only under `docs/` and
`.engineering/`* — and bringing the two amended principle documents into the run's worktree by file
copy turns it **red**: 23 changed paths, two of them `principles/**`, and the suite goes 118 / 1.
That is not a defect in either the check or the change. It is what *uncommitted* looks like: the
principle edits are the **repository's** work, not the story's product, and once they are committed
on `main` and the worktree sits on a commit containing them, `git status` lists neither and H3 is
green again. Recorded because the intermediate state is confusing and looks like a regression.

**Acceptance: not met, and the reason is F-W4.2-7 rather than the applicability gap.** Seven things
stand between this run and `complete`; the applicability fix closes two of them. The other five are
listed in the design note's § 8 and three of them are structural: the map cannot produce a
`specification` or a `verification` record; `contracts.failed == 0` is a **profile completion
condition**, which a principle falling away does not remove (`profiles/development-standard.yaml:38`,
`crates/aep-engine/src/evaluate.rs:252-254, 313-345`), and only a `ContractResult` projects it; and
`review.approved` needs a record **no CLI verb writes**.

**No `W4-2/2` was started, and that is a decision rather than an omission.** A fresh run would walk
the same six states at the same cost and stop at the same guard, because none of the five remaining
blockers is a property of the run. The wave records the blocked run as its result, exactly as
W4.1's acceptance line requires of W4.1 — *"a run that wedges is a recorded result … quietly
retrying until it works does not"*.

**What the run did prove, and it is not nothing.** Two things.

**The operator pause/resume cycle works.** W4.2's third item said *"Nobody has read that line"*.
Somebody has now: `establish_verifiers` step 2 paused, the run persisted and released its lock, a
person moved `specification:open-vocabulary-audit` from `draft` to `approved`, and the resume
carried on from the step after it and crossed the `artifact.specification.exists` guard. That half
of the item is met. The other half — *resuming from the printed line alone* — is **refused by
observation**, F-W4.2-4.

**A resume re-resolves the plan from the current documents.** the same run, resumed after two principle documents changed, reported
`evidence.missing = 2` where it had reported 4, and dropped both principles from its own
explanation. The plan is *not* pinned in the snapshot; only the workflow reference, the map id, the
map digest and the engine version are (`crates/aep-driver-spec/src/cursor.rs:283-315`). Amending a
principle and resuming is therefore a supported operation, which is the mechanism the next wave will
need to close F-W4.2-7 without re-running from `receive`.

## W4.3 — story completion, evidence-gated: a design, and a decision

**The deliverable is a document and a verdict on it, not a build.**

[`story-completion-evidence-design-v0.1.md`](../design/story-completion-evidence-design-v0.1.md)
proposes the rule in one line: **a story reaches `implemented` only when the graph holds evidence
that it was** — a `trace_conformance` record for the run that did the work, and an independent
`test_result` for the change it produced — expressed as a principle over facts in the shape
`principles/verification/ess-conformance.yaml` already uses one layer down.

It carries three things this wave needs decided rather than assumed: the `delivers` relation
formalised (`RelationKind::Delivers` exists in Rust at `crates/aep-domain/src/artifact.rs:957` and
`artifacts/relations/relations.yaml` declares twelve relations without it — the CLI already says so
in a doc comment at `crates/protocol-cli/src/planning.rs:855-861`); the lifecycle question weighed
both ways (a new `released` rung against an evidence gate on the existing terminal move); and the
enforcement mechanism named honestly, because today a status move consults a
`LifecycleRegistry` and nothing else (`crates/aep-backend-markdown/src/document.rs:115-142`).

**Acceptance:**

* the design exists, is marked **proposed, not accepted**, and names its own limits in a deviation
  register in house style — including the two that shape it: the fact projection emits **no relation
  facts** (`crates/aep-domain/src/artifact.rs:1818-1829`), and an artifact requirement's relation
  clause is satisfied graph-wide rather than per subject (`crates/aep-domain/src/requirement.rs:516-544`);
* **this wave records a verdict** — accepted, accepted in part, or refused — with the reason, on this
  page. A design filed and left unjudged is the state `AGENTS.md` § *Which documents are normative*
  exists to prevent;
* if accepted, the **build is a later wave** and this page sequences nothing;
* if refused, the reason is recorded and the gap-register row for it says refused rather than
  disappearing;
* **owed and not done here:** `AGENTS.md`'s proposed-design table and
  [`control-document-updates.md`](control-document-updates.md) need the verdict. This wave does not
  own those files, so naming the debt is the honest close.

**One thing the store already says that this page should not obscure.** The second half of this idea
— *"what made this done, answerable from the store"*, `story:completion-audit-join` — carries
`depends_on: story:journal-backed-store`, which is **P3**. So the rule can be *stated* and *enforced*
without the journal, and it cannot be *interrogated* without it: asking a store which run closed a
story is a query against a history the store does not keep (deviation **D-P3**). W4.3 proposes the
first half only, and the design's **D-S3** says why the join is a statement in a file rather than a
binding.

## W4.4 — a second real harness: Codex on the same step map

The first time the harness-neutrality claim meets a harness somebody else built. Two of the three
adapter points from § 4.9 — invoke the agent (`LlmStepExecutor`) and read the transcript
(`read_transcript` returning a `TraceIr`) — implemented for Codex, against the **same**
`drivers/development/default.yaml` and the **same** trace specification. Point 2, `tool_config`, is a
pure function and is deliberately not re-implemented: a trait there would let a second harness quietly
re-decide that `repository.write` admits a shell.

W3.5's shell-echo harness proved the seam with no model, no network and no credential. It cannot
prove portability, and trace wave 1 already says the neutrality claim is untested
(`trace-wave-1-transcript-checker.md:263-265`).

**The input landed, and it moved two decisions.**
[`docs/reviews/2026-08-21-codex-harness-research.md`](../reviews/2026-08-21-codex-harness-research.md)
is the record — verified against a local codex-cli 0.145.0 install and 2,437 rollout files, with
every fact labelled by how it is known. It is cited rather than restated; two of its findings change
what this item is:

* **the adapter reads the session rollout JSONL, not `codex exec --json` stdout.** The stdout stream
  carries no timestamps, no durations and no cost, and `trace-ir/1` wants all three. The consequence
  for the build is that W4.4 is *smaller* than budgeted, not larger;
* **the enforcement layer is portable.** Codex ships a stable, default-on `PreToolUse` hook with the
  same decision contract shape as Claude Code — `permissionDecision: deny` plus a reason, exit 2 also
  blocks, `hooks.json` in the repository. § 4.9's *"three adapter points"* therefore understates the
  reusable surface: it is three adapter points **plus one hook contract that holds on both
  harnesses**. That is a strengthening of design § 4.8's portability claim and is **owed to the
  design, which this page does not own.**

Two questions stay open in that record and neither blocks the tiers below: the rollout format has no
documented stability guarantee (mitigation: version-gate on `session_meta.cli_version` and treat
unknown shapes as opaque, never as a failure), and the approval-event wire shape is unverified
because no local session ran a prompting mode — one `-a untrusted` run answers it.

**Acceptance, in three tiers, so the item cannot fail silently:**

| tier | what lands | what it establishes |
|---|---|---|
| **full** | one `llm` step of the same step map runs under Codex; a second `read_transcript` reads the rollout into a `TraceIr`; `protocol trace check` decides it against the same specification file | the specification is portable, and the seam is the IR rather than an accident of one adapter |
| **partial** | the transcript reader exists and is tested against a **recorded** rollout; no live run | what the shell-echo harness is to the driver, one layer out: the reader is real and the invocation is not |
| **refused, with a reason** | the reader cannot be version-gated into something stable, or the live run contradicts the hook contract the research documents | a finding about the specification's portability, written up as one. **The research has narrowed what could land here**: the *"the harness cannot deny a tool call"* outcome, which was the one this tier was written for, is now documented not to be the case, so a refusal at this point would be a finding about *format drift* rather than about enforcement |

Whichever tier lands is named on this page and in the gap-register row for the neutrality claim.

## What is deliberately not in this wave

* **Building W4.3.** The design is written and judged here; a `move --task` mode, a validator rule or
  a new status variant is a later wave's work under whatever the verdict says.
* **A third harness, and a trait in `trace-spec`.** § 4.9 refuses the trait by name until there is a
  second implementation to design it against — W4.4 is that second implementation, and the trait
  question opens *after* it, not during it.
* **Attested evidence.** Gap-register **D-3** stays proposed. `independent: true` remains a
  structural statement about which component produced a record.
* **P3, the journal-backed store.** Deviation D-P1 stands for this wave: the store is not a contract
  implementation and the sixteen `aep-conformance` suites still do not run against it. What wave 4
  contributes is the first store with real content in it, which is what makes the absence of a
  history something a person notices rather than something a register records.
* **Changing a workflow, a profile or a principle to make the run go through.** If `adp/default`
  refuses this repository's real work, that is a finding about the workflow and it is written up as
  one. Editing the rule until the run passes is the failure mode the whole project is against.
* **Adding `eval/run.sh`-style model calls to `task check`.** The gate reaches no network
  (`AGENTS.md` § *Dependencies*). Everything in W4.1 and W4.4 that calls a model is run deliberately
  and recorded with the wave, the same rule wave 1 set for its eval.

## What this wave will not be able to say

Named on the way in, so a reader does not find them by their absence.

| unknown | why this wave cannot close it | what it costs |
|---|---|---|
| **whether the driver is better than a careful agent with a good prompt** | there is no control arm. One story driven once is an existence proof that the mechanism holds, not a comparison | nothing yet — but the first time somebody asks *"is this worth the token cost"*, the answer is a measurement nobody has taken. D4's per-step sessions are the cost, and the trace census is where the number would come from |
| **whether the trust model for plugin-supplied hooks holds** | still undocumented for Claude Code (§ 4.8's named assumption). Running a hook successfully does not establish that it runs without consent in somebody else's install. **Codex's is documented and is the opposite**: a non-managed hook needs explicit trust, with a bypass flag for automation (Codex research record, § *Enforcement portability*) | if the Claude assumption is wrong, that hook layer degrades to advisory and `--allowedTools` carries enforcement alone. On Codex the cost is known and is a setup step, not an unknown |
| **whether one governed story generalises to a backlog** | one story is one story | the honest claim after this wave is *"the mechanism held once, on real work"* — which is strictly more than the repository can say today, and strictly less than *"this is how work happens here"* |
