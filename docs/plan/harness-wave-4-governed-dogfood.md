# Harness wave 4 — the repository governed by its own driver

> **Status: proposed, and sequenced by nothing yet. Its predecessor has now shipped.**
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
| **one small real story, not a representative one** | chosen for a bounded diff and a suite that already exists; named on this page when the wave opens | a large story tests the model's stamina and the wave's patience, not the driver's enforcement, and it fails for reasons this wave cannot fix. The wave is about whether the *mechanism* holds |
| the operator is **in the loop by design**, not as a fallback | W4.1 runs `development.standard` with `--pause-on-approval`; the review becomes an `operator` step | D3: a headless run **refuses to start** when an approval is reachable, and `development.standard`'s `approval-gates` is reachable. The two ways to avoid the pause are both refused — dropping to `development.fast`, which deliberately cannot summon a human (`profiles/development-fast.yaml:25-27`), would test a weaker profile than the work deserves; auto-approving is refused under every flag by D3 |
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
