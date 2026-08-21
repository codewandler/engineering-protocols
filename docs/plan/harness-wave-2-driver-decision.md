# Harness wave 2 — the driver decisions, and wave 3's record

> **Wave 3 is delivered, 2026-08-21.** The decisions below became `aep-driver-spec`, `aep-driver`,
> `drivers/development/default.yaml`, `protocol drive`, the plugin's enforcement hooks, a
> second harness with no model in it, a driven eval that ran for real, and — operator-added, outside
> the reviewed breakdown — `protocol workflow render`. **The acceptance for every item is at the
> foot of this page**, under *Wave 3 — built, 2026-08-21*, with the evidence beside each line.
> This page carries both halves because wave 3 has no page of its own:
> the breakdown a reviewer judged and the record of what it became belong in one place, where they
> can be read against each other.

> **The operator set the goal on 2026-08-21: waves 2 and 3, the driver, built as the thing that
> makes a specified workflow run strictly rather than be steered towards.** This page takes **wave
> 2**, which produces decisions and a review and no crate. **The feasibility review is written and
> W2.3 is applied** —
> [`2026-08-21-driver-feasibility-review.md`](../reviews/2026-08-21-driver-feasibility-review.md),
> **23 CONFIRMED · 14 NEEDS-CHANGE · 3 INFEASIBLE · 0 UNRESOLVED**. Wave 3 opens behind it, and a
> decision not to build the driver remains a legitimate outcome of that review — it closes the gap-register row exactly as building it would, provided the
> VISION narrowing is reverted in the same change. Design:
> [`harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md)
> §§ 4.7–4.9, the wave-2 update **as corrected by the review**; §§ 4.1–4.6 are unchanged and stay as
> the record of what the review was originally given. Every NEEDS-CHANGE and INFEASIBLE item is
> applied in the design and in this page's table, each citing the finding that forced it. **Nothing
> was re-argued.**

**Goal: every one of the six holes in § 4 is a taken decision with a mechanism named in code that
exists — and the enforcement question has an answer, in a table, per rule class.**

Wave 1 built the store and the plugin. This wave builds nothing. It exists because
`harness-planning-and-driver-design-v0.1.md` § 4 was architecture with six named holes in it, and
because this repository's ordering rule — do not build from an unreviewed design — is what kept ESS
wave 4 from generating code against an oracle nobody had watched fail.

## What this wave is, in one sentence each way

For the person who will run this: the difference between *telling* an agent to write the test first
and *a run in which it cannot do anything else* — the tools that exist in a state are the ones the
protocol grants there, a refused action is refused by a program rather than by a paragraph, and a
checker reads the transcript afterwards and says whether that held.

For the machinery: the driver is the **sample runner**, not the only one. Every behavioural
specification here is a harness-neutral document, and a second harness adopts the set by implementing
three adapter points — invoke-the-agent, capabilities-to-tool-config, and a transcript reader.

## Decisions, taken

| decision | taken as | why |
|---|---|---|
| **D1 step-map versioning** | the map pins `workflow: adp/default/1` — a `WorkflowRef`, **mandatory** — and a workflow major bump orphans it at load, as a refusal | `Registry::workflow` filters on `WorkflowRef::accepts`, which is equality against the pin (`crates/aep-engine/src/registry.rs:118-123`, `crates/aep-domain/src/version.rs:206-208`), so the orphaning is free and loud. **§ 4.2's `adp/default@1` is wrong**: `Display` writes `{id}/{major}` (`version.rs:214-221`) and a second spelling of a version pin is a second parser |
| D1, second half | cross-validation runs in **two phases** — states and **named** verifiers at load, evidence kinds and the workflow pin at run start. **F5:** `Verifier::ExternalTool` is exempt at load | the protocol in force comes from the *task* (`crates/aep-domain/src/task.rs:338`), which no document loader has seen. One-phase validation would let a map pass and then fail at `ProtocolError::EvidenceRejected` (`crates/aep-engine/src/engine.rs:321-332`) mid-run — the exact failure the check exists to prevent |
| D1, the pin as a type | **F6:** `PinnedWorkflowRef` in the step-map crate — `TryFrom<WorkflowRef>` refusing `major() == None`, publishing the same pattern with the version group **required** | as written the decision was stricter than the schema it would publish: `major` is `Option` (`version.rs:200-202`), the pattern makes the group optional (`:296`) and the `JsonSchema` impl writes it verbatim (`:325`). An editor would have told an author their map was fine and the loader would have refused it. `ProtocolRef` is the type-level precedent, not just a rhetorical one (`version.rs:108-112`, `:132`) |
| D1, where the types live | **F1, INFEASIBLE as sequenced:** two crates — leaf **`aep-driver-spec`** (`RawStepMap`→`StepMap`, `PinnedWorkflowRef`, cursor, `ToolConfig`; `aep-domain` only) and **`aep-driver`** (router, `LlmStepExecutor`, `tool_config`) | one crate is the cycle `aep-schema → aep-driver → aep-engine → aep-schema`: `load_tree` is in `aep-engine` (`crates/aep-engine/src/load.rs:22-28`), its values are `aep_schema::parse::DocumentKind`, and the router must see `Evaluation` (`evaluate.rs:124`). `aep-schema` already carries sideways edges to three **leaves**; this is the fourth. `drivers/` is the **last** `TREE` row, because phase 1 reads `workflow.states` out of a registry the earlier rows fill |
| D1, runs in flight | the **cursor** records the resolved `workflow: <id>/<major>`, the step map's digest **and the engine version** (**F20**); `--resume` refuses when any moved | `Snapshot` carries no workflow id and no version (`crates/aep-engine/src/execution.rs:56-74`), and `Execution::restore` checks only the task id and that the *state name* still exists (`:257-293`). A workflow that renamed nothing and rewrote every guard restores silently today |
| **D2 store → facts** | rebuild the `ArtifactGraph` from the store **every iteration** and hand it to `Engine::restore`; no cache | `MarkdownStore::load()` → `StoreReport::graph()` (`crates/aep-backend-markdown/src/store.rs:85`, `:329-330`) and `Execution::restore` → `refresh_facts()` (`crates/aep-engine/src/execution.rs:291`, `:297-319`) already do it. The rebuild *is* the store's integrity check, so the cost buys the correctness. A cache is a second copy of the membership list — § 2.1's argument against an index file, one layer out |
| D2, when to stop | **F7:** stop when **`report.is_clean()` is false** (`crates/aep-backend-markdown/src/store.rs:314`) **or** `graph()` errors, consulting `is_clean` **first** | `graph()` returns `Ok` for a store with parse failures — the crate says so at `store.rs:319-328`, deliberately, because a listing beats a refusal for *reading*. A file that did not parse sits in `report.failures` (`:100-120`) and is not in the graph to be wrong about, so a `graph()`-only check evaluates a **completion gate** against a fact base that silently shrank |
| D2, the cost | **F8:** a full read and parse of **every** planning document (`store.rs:105-120`, `:371`) **plus a full plan re-resolution** (`engine.rs:250-258`) per iteration; registry loaded **once per invocation**, store rebuilt **per iteration** | the first draft said "one directory walk". Both are linear, pure, local and clock-free, so the conclusion holds — but the number is the number. The registry/store asymmetry was falling out of an implementation detail and is now chosen: D1's cursor pins the workflow precisely so a governing document cannot move under a live run |
| D2, the boundary | `artifact.**` from the graph; `evidence.**` / `required_evidence.**` / `state.**` / `approvals.granted` from the engine; `tests.*`, `diff.*`, `static_analysis.*`, `trace_conformance.*` from evidence payloads. **The driver writes into no family** | `ArtifactGraph::facts()` (`crates/aep-domain/src/artifact.rs:1830-1872`) and `Execution::derived_facts` (`execution.rs:322-398`) are the only two producers. A fact the driver minted would be a gate the driver evaluated, which § 4.1 forbids |
| **D3 `require_approval` headless** | an approval-gated capability is **never a tool**; a capability is offered **iff `policy.decide(&capability) == Allowed`** (**F3**) | *"derive from `.allow` only"* is not invariant 6's ordering — the ordering lives in `decide` (`capability.rs:588-599`). The three sets are independent (`grant` extends all three, `:619-624`) and membership is by `covers`, not equality (`:612-614`, `:240-246`), so an unscoped `allow: deployment.create` **covers** `deployment.create:production` and iterating `.allow` hands out the tool `approval-gates.yaml:38` gates. `release-progressive.yaml:29-31` avoids it *in a comment* |
| D3, interactive | an approval the plan owes becomes an `operator` step — print `CompletionExplanation` verbatim, persist, release the lock, exit 0 with a resume line | a driver holding a terminal open for a person loses the run when the terminal closes. The snapshot is already a queue that survives a reboot |
| D3, the walk | **F9:** the scan also walks `workflow.transitions[].requires` (`workflow.rs:127-138`) and **recurses** through nested conditionals (`requirement.rs:895-900`) | a transition's requirement set is first-class to the evaluator (`evaluate.rs:215`, beside `:203` and `:226`), so a `human: true` approval on a transition is genuinely owed and was invisible to the scan. `count_missing_evidence` descends one level *by design* because it counts; a reachability scan that stops there under-reports, and under-reporting starts a run that wedges |
| D3, two consequences | **F4(2):** `development.standard` starts **only because** `approval-gates`' guard is `defined(...)` (`predicate.rs:402`), and `development.critical` **refuses** a headless start on its unconditional `human: true` design review (`profiles/development-critical.yaml:46-52`) | the headline test is green for a reason outside D3. A future principle author writing the bare comparison instead of `defined(...)` would turn every headless development run into a refusal with nothing explaining why. The `critical` refusal is right behaviour and a surprise if nobody writes it down |
| D3, headless | **refuse to start** when an approval is *reachable*; `--pause-on-approval` is the opt-in route | *"refuse when `approval_required` is non-empty"* refuses **every** run: `least-privilege` has no `applies_when` and gates `production.write`, `deployment.create`, `network.write` for every task under every profile (`principles/governance/least-privilege.yaml:19-22`). Reachability is static and decidable — `human: true` approvals and reviews, human verifiers (`Verifier::is_human`, `crates/aep-domain/src/verification.rs:120-122`), and gated capabilities a `command` step would exercise |
| D3, auto-approve | **never, under any flag**; the driver constructs no `Evidence::Approval` and never stamps `Producer::Human` | `approval_recorded` matches on subject and decision and **does not check who granted it** (`crates/aep-engine/src/policy.rs:135-151`), unlike `ApprovalRequirement::evaluate`, which does (`crates/aep-domain/src/requirement.rs:839-874`). Nothing below the driver would stop a harness minting its own approval, so the refusal has to be the driver's |
| **D4 session granularity** | one `claude -p` session **per `llm` step**; context via the store and the prompt, never session memory | replay is only checkable if each step's input is a function of persisted state; the allowlist changes at every `Moved` (`effective_policy`, `crates/aep-engine/src/policy.rs:84-92`) and a session outliving a transition outlives its allowlist; a retry inside a shared session retries with the failure in context. The token cost is real, and is *measured* by the trace census rather than argued about |
| D4, the fourth reason | per-step sessions are the **only** granularity at which a launch-time flag can express a per-state tool set | `--allowedTools` is fixed at session launch, with no mid-session swap (hooks reference). A step never spans a transition, so every session launches with its state's set. Per-state sessions would already be wrong; one session per run would be unimplementable. The review found this while filling open cell (b), and it is the strongest of the four reasons |
| D4, the command line | **F15:** the driver **never passes `--bare`**; hook configuration goes through `--settings`, asserted by a test over the constructed argv | `--bare` skips hooks. Nothing constrained the command line, so an implementer reaching for a clean reproducible environment would **silently delete the driver's own enforcement arm** — partial, silent, and exactly what a register exists to catch |
| D4, resume | a resumed run is a **new session with the same inputs**, and its transcript digest differs | the step did run twice. A record claiming one run would be the record lying |
| **D5 failure taxonomy** | crashed step ⇒ **submit nothing** (`Unknown`); suite ran and failed ⇒ submit the failing evidence (`False`); budget exhausted ⇒ snapshot, `Blocked` reasons verbatim, exit | the engine has no `Unknown` to submit — absence is the fact not being in the store. With no test evidence **both** `verify` guards are `Unknown` and `transition()` returns `Blocked` (`workflows/development/default.yaml:106-127`, `engine.rs:397-415`), which is exactly the state to retry against. Collapsing a crash into `False` takes the back-edge and sends an agent to fix code nobody ran |
| D5, the ambiguous ones | one question — **did a verifier produce a verdict?** timeout, OOM, network error ⇒ Unknown; a suite that completed with failures ⇒ False | a partial suite is not a failing suite. `trace check` exit 3 is the one Unknown that is still *recorded*, because `trace evidence` writes `status: inconclusive` and `trace_conformance.passed` stays false (`crates/trace-spec/src/evidence.rs:43-54`) |
| D5, retries | budget **per step kind** — `command` retries, `llm` once, `operator` never — spent and not reset, counted in the cursor | a person is not a flaky dependency. A retried success does not erase the first attempt: there is no evidence to erase, but the attempt count stays and the run report names it |
| **D6 concurrency** | **F2, INFEASIBLE:** one fixed path, `.engineering/runs/lock.json`, `create_new` **before** any run-id allocation; run directories hold no lock; `--resume` **re-takes** it | the reviewed version put the lock inside the directory the lock was allocating. Two invocations counting at slightly different moments get `3` and `4` and **both `create_new` succeed** — D6's own rejected option, *"no lock, last writer wins"*, reached by accident. The holder's run id now lives inside the lock, so a refusal prints it without a second read. One atomic syscall, no advisory locking, and its failure mode is the one we want |
| D6, placement | **F19:** the lock file, the pid-liveness probe and the run directory are `protocol-cli`'s; `aep-driver` is handed a `LockState` | a liveness probe reads ambient OS state and uses neither `SystemTime::now` nor `rand`, so a banned-token scan would not catch it. Placement is the only thing keeping the pure crate's claim true — and it makes the lock testable without a second process |
| D6, invariant 16 | **F17, overturned in our favour:** a removable lockfile is **not** a breach; two adjacent rules adopted anyway | invariant 16's subject is the entity command vocabulary (`AGENTS.md:239-242`), not the filesystem. Adopted regardless: a run directory is never deleted or reused, and `--take-lock` **supersedes** — the stolen lock's contents go into the new run's cursor, so *"this run took the lock from pid 4711"* is in the record |
| D6, the run id | the run id is the **driver's**, allocated after taking the lock; the engine's `ExecutionId` goes *inside* the cursor | `ExecutionId` is `<task>.<ordinal>` where the counter starts at zero **in each `Engine` value** — **F10** sharpens this: it is a field (`engine.rs:173`, `:186-190`, `:210-213`), so two `Engine`s in **one** process collide too, which is the shape a test harness builds. The hazard is confined to `initialize`; `Execution::restore` preserves the id (`execution.rs:277`) |
| D6, stale locks | liveness, **never age**: pid alive ⇒ held; same host and dead ⇒ stale but still refused without `--take-lock`; another host ⇒ never stale | any age threshold must exceed the longest legitimate step, and that is *an operator step waiting for a person*, which has no bound. A two-hour timeout would break exactly the runs that paused correctly |
| D6, a second invocation | refuses, printing the holder's run id, pid, host and cursor state, and names both routes out | the same choice `protocol artifact move` makes for an illegal transition: the refusal is the answer |
| **enforcement mapping** | § 4.8 — one row per rule class, each naming the mechanism, the layer and what audits it. **All three open cells are filled**; three audit columns were wrong and are corrected | *a rule nothing checks is a rule that has already drifted somewhere* — applied to the section itself, which is what produced F12, F13 and F14 |
| the shell property | **the model never holds a shell in a development run.** `Bash` is offered only when `decide(command.execute) == Allowed`, and no development profile grants it (`profiles/development-fast.yaml:30-35`, `development-standard.yaml:28-30`). **Corrected by the build, W3.6 below: `development.driven` grants it, held to the `protocol` CLI by a hook. The two profiles named here are unchanged** | `Bash` is the one tool that is not a function of a capability: one call can be `tests.execute`, `repository.write`, `network.write` or `secret.read`. Rather than gate it by pattern — best-effort, and now stated as such — the property does the work: `cargo test` runs as a **`command` step the driver executes**, not as a tool the model holds. It is also what makes § 4.8's write-guard matcher exhaustive |
| `Skill` and `Task` | `Skill` is a **named exemption** (it loads instructions and takes no action); `Task` and the agent-spawning family are **never offered**, audited by `subagent.spawned: at_most 0` | a tool with no `Action` cannot be governed (`docs/guide/harness.md:144-146`), and a subagent's tool set is derived by nothing in D1–D6 — so it would be a route around the per-state allowlist. The audit kind already ships (`crates/trace-domain/src/spec.rs:797`) |
| **F12 — the missing audit** | a **50th expectation kind, `env.tool_available`**, becomes a named wave-3 build item in the trace crates, sequenced **before** the hooks | the per-state tool set had no audit: `SessionStart.tools` is in the IR (`crates/trace-domain/src/ir.rs:222-223`) and no expectation kind reads it (49 names, `spec.rs:777-830`). `tool.absent` is not a substitute — it asserts a tool was never *called*, and an allowlist bug offers a tool nobody calls |
| **F14 — the hook↔engine channel** | the hook appends to `.engineering/runs/<run-id>/hook-decisions.jsonl`; the driver folds each line in through `Engine::authorize` after the step exits | a hook is a separate process and `authorize` takes `&mut Execution` (`engine.rs:285`), so the audit column named a trail the hook cannot reach. Folding late is safe because `transition()` is not called until the step's process has exited (D4) — the same reason the TOCTOU window is zero. A socket adds a hang to a batch program; hook-enforces-without-asking would mean rewriting rows 1 and 2 to say *the transcript* |
| **F13/F15 — what is not claimed** | `permission.denied` is a **whole-run count** and `0` is ambiguous; the plugin-hook **trust model is undocumented** and is named as an assumption | attribution needs a deliberate-attempt case in W3.6. If the trust assumption is wrong the hook layer degrades to advisory and `--allowedTools` carries enforcement alone. Whether a hook deny increments `permission_denials` is unverified and closes with one command |
| the boundary | enforcement is complete over **actions** and **transitions**; **text is free** | every tool call maps to exactly one `Capability` or has no tool, and the driver never evaluates a gate. What the model *says* is not governed and is safe unbounded for three reasons already in place: an `llm` step cannot carry evidence, `Producer::Agent` does not satisfy `independent: true`, and a claim about how an agent worked is established by a checker reading the transcript |
| enforce **and** verify | hooks and the allowlist stop the action; `trace check` reads the transcript and says whether they did | an enforcement mechanism nobody audits is a claim; an audit with no enforcement is a report about a horse that has left |
| hooks versus § 3.6 | the plugin's refusal of hooks **stands for a plugin that ships alone**; hooks ship in the *driver's* wave, configured by the driver from `capabilities()` | § 3.6's reason was that a hook layer would be *a second, weaker driver* with no execution to ask about. Under the driver it is not a second driver — it is the driver's enforcement arm, and it has an execution |
| **adapter surface** | exactly **three** points: an `LlmStepExecutor` trait in `aep-driver`; `tool_config(&CapabilityPolicy) -> ToolConfig` as a **pure function, not a trait**, deciding by `decide()` (**F3**); a transcript reader returning `TraceIr` | a surface nobody bounded grows a fourth point the first time something is awkward. Point 2 is a function because a trait would let a second harness quietly re-decide that `repository.write` admits a shell |
| the transcript seam, corrected | **there is no adapter trait in `trace-spec`** — the seam is the IR: `read_transcript` (`crates/trace-spec/src/adapter.rs:102`) stamping `AdapterRef` into `TraceIr` (`crates/trace-domain/src/ir.rs:506`, `:516`, `:528`), with `check`, `CheckReport` and `to_evidence` all taking `&TraceIr` | a second adapter is a second free function, not a trait added speculatively before there is a second implementation to design it against. Trace wave 1 already says the neutrality claim is untested (`trace-wave-1-transcript-checker.md:263-265`) |
| where step maps live | `drivers/`, taking design open decision **D4** | `load_tree` walks a fixed directory-to-kind table (`crates/aep-engine/src/load.rs:22-28`) and a step map is a validated, versioned, schema-generated document exactly like the four in it. Anywhere else is a fifth kind loaded by a second mechanism |

## W2.1 — the decisions, recorded in the design

§§ 4.7, 4.8 and 4.9 of `harness-planning-and-driver-design-v0.1.md`: the six decisions with their
mechanisms, the enforcement mapping, and the adapter surface. §§ 4.1–4.6 are extended in place with
pointers and **not rewritten** — § 4.5's list of six holes stays exactly as the review was given it,
and § 4.2's wrong `@1` spelling is left standing with a line saying it is wrong and where it is
corrected.

**Acceptance — met.**

* every one of the six holes has a decision, a mechanism, a rejected alternative and a wave-3 test —
  four things, not one, because a decision without a rejected alternative reads as the only option
  anybody thought of;
* every code claim carries a `file:line` that resolves in this tree at the revision this page lands
  on;
* the three corrections found while deciding are **written as corrections** and not silently applied:
  the `@1` spelling (§ 4.2), the naive headless refusal that would refuse every run (D3), and the
  absent adapter trait (§ 4.9);
* no crate, no `drivers/` document, no `.engineering/runs/` writer — the wave produces prose.

## W2.2 — the feasibility review

A review in `docs/reviews/`, against the code, of the kind ESS waves 4 and 6 went through — the
precedent files are `docs/reviews/2026-08-20-next-waves-feasibility-review.md` and
`docs/reviews/2026-08-20-infrastructure-design-feasibility-review.md`. It is adversarial by
construction: its job is to find the decision that cannot be built, not to agree.

**Acceptance — met, 2026-08-21.**
[`docs/reviews/2026-08-21-driver-feasibility-review.md`](../reviews/2026-08-21-driver-feasibility-review.md),
708 lines, twenty findings F1–F20.

| criterion | how it was met |
|---|---|
| every decision has a verdict, none unaddressed | **23 CONFIRMED · 14 NEEDS-CHANGE · 3 INFEASIBLE · 0 UNRESOLVED**, one row per decision and per enforcement cell |
| every `does not hold` names the contradicting `file:line` | F1 names the cycle through four `Cargo.toml`s and `load.rs:22-28`; F2 quotes the two sentences of D6 that cannot both execute; F3 quotes `capability.rs:588-599` |
| the marked cells are filled or reported unverified **by name** | all three resolved. Two unverifiables are named rather than glossed: the plugin-hook **trust model** (undocumented anywhere) and whether a hook deny increments `permission_denials` |
| the breakdown is judged buildable, and the item that must move is named | **buildable after W3.1 is split and W3.4 gains a prerequisite.** The item that had to move is W3.1 |

The review also overturned one worry in our favour (F17: a removable lockfile is not an invariant-16
breach) and confirmed all three of W2.1's self-declared corrections.

## W2.3 — the design corrected where the review disagreed

Whatever the review overturns is applied to §§ 4.7–4.9, in place, with the overturned decision left
visible rather than deleted — the same rule W2.1 follows for § 4.2. A decision that quietly changes
between a review and a build is a decision the build cannot be held to.

**Acceptance — met, 2026-08-21, with one item explicitly owed elsewhere.**

* **all 14 NEEDS-CHANGE and all 3 INFEASIBLE items are applied**, in §§ 4.7–4.9 and in this page's
  table, each citing the finding that forced it. **Nothing was re-argued** — where the review
  offered options (F6's two spellings of the mandatory pin, F14's three channels) the design takes
  its stated preference and names the option refused, so the choice is auditable rather than
  invisible;
* no finding is met with silence, and no corrected sentence is deleted where a correction can stand
  beside it — the wave-4/5 precedent, and the rule W2.1 already applied to § 4.2's wrong `@1`;
* the wave-3 sketch below matches the corrected architecture: `aep-driver` is two crates, the lock is
  at a fixed path, and the 50th expectation kind is sequenced ahead of the hooks;
* **owed, and not done here:** `docs/plan/gap-register.md` needs two rows — the driver row updated
  to name this page and the review, and a new row for *"whether a `PreToolUse` deny appears in the
  transcript's `permission_denials` array"* (F13). **This wave does not own that file**, so naming
  the debt is the honest close rather than a silent one.

## Wave 3's build breakdown — a sketch, so the review can judge buildability

**Written for exactly one reason: a feasibility review that cannot see the shape of the build cannot
say whether the decisions are buildable.** Nothing in it was accepted, ordered or estimated.
**Corrected against the review's F18**, which judged the sequence buildable after W3.1 is split in
two and W3.4 gains a prerequisite.

**It is left exactly as the review was given it.** What the sketch became — every item met, with a
seventh the review never saw — is the last section of this page. Keeping both is the rule W2.1
already applies to § 4.2's wrong `@1`: a plan that quietly becomes a record is one nobody can check
the record against.

| | what | resting on |
|---|---|---|
| **W3.0** | **`env.tool_available`** — the 50th expectation kind, in `trace-domain` and `trace-spec`: a `RawExpectationKind` variant, a `NAMES` entry, a name arm and four lines of dispatch against `SessionStart.tools`, mirroring `env.skill_available` (`crates/trace-spec/src/check.rs:103-107`) | **F12.** It ships *first* because without it the per-state allowlist has nothing that can audit it, and § 4.8's own standard would be asserted rather than met |
| **W3.1a** | **`crates/aep-driver-spec`** — a leaf on `aep-domain` only: `RawStepMap` → `StepMap`, `PinnedWorkflowRef`, the cursor types, `ToolConfig`, both cross-validation phases | **F1, F6.** `aep-schema` takes the dependency, exactly as it already does for `aep-backend-markdown` |
| **W3.1b** | **`crates/aep-driver`** — the three-valued router, `LlmStepExecutor`, `tool_config` over `CapabilityPolicy::decide` | **F1, F3.** Depends on `aep-domain`, `aep-engine`, `aep-driver-spec` |
| **W3.1c** | both manifests carry `[lints] workspace = true`; `crates/aep-driver/tests/determinism.rs` ships with them, and invariant 9's list in `AGENTS.md` gains its row in the same change | **F19.** `AGENTS.md:213-214` — a crate that omits the lints line is outside every lint here; `AGENTS.md:141-144` — do not write an enforcement you cannot point at, and § 4.1 makes a purity claim for this crate stronger than `aep-engine`'s |
| **W3.2** | `drivers/development/default.yaml` — the first step map over `adp/default/1`, plus `schemas/generated/driver-steps.schema.json`; `drivers/` added as the **last** row of `load.rs`'s `TREE` | D1, design D4, **F1** |
| **W3.3** | `protocol drive` — the executors that touch the world (`command`, `llm`, `operator`), the run directory, the **store lock at `.engineering/runs/lock.json`**, the pid-liveness probe, `--resume` (which re-takes the lock), `--restart`, `--take-lock`, `--pause-on-approval` | D3, D6, **F2, F19** |
| **W3.4** | the plugin's hooks, **Phase 2**: `PreToolUse` deny from the per-state set, the `.engineering/planning/**` write guard with `matcher: "Edit\|Write\|NotebookEdit"`, and the `hook-decisions.jsonl` channel the driver folds in | § 4.8; **F14, F15, F16**; **needs W3.0**. The driver's `claude -p` line carries `--settings` and never `--bare` |
| **W3.5** | the **shell-echo harness** — a second `LlmStepExecutor` and a second transcript reader, proving the three adapter points with no model, no network and no credential, inside `task check` | § 4.9. Confirmed buildable as sequenced; nothing changed |
| **W3.6** | driven-eval acceptance: one real task driven end to end under `adp/default`, transcripts checked by `protocol trace check`, `trace_conformance` records submitted to the engine — outside `task check`, like `eval/run.sh` — **plus a deliberate-denial case**, so `permission.denied` audits something rather than reporting an ambiguous `0` | everything above; **F13** |

The load-bearing items are W3.1 and W3.5. W3.1 is where the decisions either compile or do not, and
it is the item the review moved. W3.5 is where the neutrality claim stops being a sentence: today
one adapter exists, and *"harness-neutral"* is a property nothing has ever tested.

## What was deliberately not in wave 2

Wave 3's own exclusions are in its section below; this list is wave 2's and is left as written.

* **Any code at all.** No `aep-driver`, no `protocol drive`, no `drivers/` document, no hook, no
  `.engineering/runs/` writer. The directory name stays reserved and nothing writes to it.
  **All five landed in wave 3.**
* **A second real harness.** Codex, or any other, is not a prerequisite and is not sequenced. W3.5's
  fake harness is what tests the seam; a real one replaces it as a third implementation later.
* **A trait in `trace-spec`.** Refused by name in § 4.9 until there is a second implementation to
  design it against.
* **Attested evidence.** Gap-register **D-3** stays proposed and not accepted, and nothing in these
  decisions assumes it. `independent: true` remains a structural statement about which component
  produced a record, not a proof of who it was.
* **Any narrowing of what a model may write.** Text is free, and § 4.8 says so as a boundary rather
  than as an omission — the mechanisms that make it safe are named there.

## The three open cells — all resolved

| cell | resolved as | consequence |
|---|---|---|
| **(a)** the `PreToolUse` deny mechanism | exit code **2** or `permissionDecision: deny`; the reason is fed back to the model; the hook sees the full `tool_input`; `matcher` selects by tool and `if` takes permission-rule syntax; hooks **deny but never grant**; they fire identically under `claude -p`; a plugin ships them in `hooks/hooks.json` | rows 1 and 6 are implementable **exactly as written**. `tool_input.file_path` is what makes row 6 a path check rather than a tool check |
| **(b)** per-state gating under `claude -p` | `--allowedTools` is **fixed at session launch** — there is no mid-session swap. Primary: the flag, from `tool_config(effective_policy(execution))`. Backstop: a `PreToolUse` hook over the same derived set | not a problem *because of D4*: one session per step, and a step never spans a transition. It is a **fourth argument for D4**, and a mechanical one — per-state sessions would already be wrong and one session per run would be unimplementable |
| **(c)** the harness tool-name table | filled against the guide's `Action → Capability` table and the 32 tools in the committed transcript — with **three entries that are not functions of a capability**: `Bash`, `Skill`, `Task` | each is now a decision in § 4.9 point 2 rather than an implementer's judgement. The strong property fell out of it: **no development profile grants `command.execute`, so an `llm` step holds no shell** |

## What remains genuinely unknown

Two items, both named by the review rather than glossed, and neither blocking. **One of the two was
closed by wave 3 and the row says so rather than disappearing:**

| unknown | why it cannot be closed here | what it costs if it goes the wrong way |
|---|---|---|
| the **trust model for plugin-supplied hooks** — whether an installed plugin's hooks run without a per-invocation consent step | **not documented anywhere**, so no amount of reading closes it. **Still open after wave 3:** a hook that ran successfully in one install does not establish that it runs without consent in somebody else's | the hook layer of § 4.8 degrades to advisory and `--allowedTools` carries enforcement alone. Named as an assumption in § 4.8 rather than assumed silently |
| whether a hook's `permissionDecision: deny` increments the transcript's `permission_denials` array | needed one `claude -p` run with a denying hook, then read the last line — *this wave* ran no model. **Closed by W3.6, 2026-08-21: yes, one-for-one.** The denial session's three hook refusals — `Bash`, `Edit`, `Write` — produced exactly three `permission_denials` entries, each carrying the tool's name, and the honest session's single refusal produced exactly one | it decided whether § 4.8 row 1's transcript-side audit works at all, and it does. The row is kept **advisory** even so: it asserts a model behaviour on top of an undocumented harness detail, and the gating evidence is the hook-decision log and `protocol artifact validate` |
## Wave 3 — built, 2026-08-21

**The breakdown above is the sketch a reviewer was given. This section is what it became**, in the
same order, with the evidence beside each acceptance rather than a claim that it was met. Every
number below was read out of this tree, or out of a run whose records survive.

### W3.0 — the 50th expectation kind

`env.tool_available`: the variant (`crates/trace-domain/src/spec.rs:231-232`), the `NAMES` entry
(`:843`), the name arm (`:773`), and four lines of dispatch against `SessionStart.tools`
(`crates/trace-spec/src/check.rs:113` → `env_tool_available`, `:563`).

**Acceptance — met.** `ExpectationKind::NAMES` is **50** entries. The drift test that holds the raw
and validated vocabularies together (`crates/trace-domain/src/spec.rs:772-776`) still passes, so a
half-done job would have failed the ordinary gate rather than shipped. It landed **before** the
hooks, which is what F12 asked for.

**What it does not do, discovered by using it.** `SessionStart.tools` is
the harness's tool *inventory*, not the session's allow rules, so this kind cannot audit an
allowlist. § 4.8 row 3 stays open and now says so.

### W3.1 — two crates, because one was a cycle

`crates/aep-driver-spec` — 1,883 source lines over `map.rs`, `cursor.rs`, `pin.rs`, `tool.rs` and
`digest.rs`, on `aep-domain` alone: `RawStepMap → StepMap`, `PinnedWorkflowRef`, the cursor types,
`ToolConfig` and both cross-validation phases.
`crates/aep-driver` — 1,577 source lines over `run.rs`, `approval.rs`, `executor.rs`, `lock.rs`,
`route.rs` and `tool.rs`: the three-valued router, the `LlmStepExecutor` / `CommandStepExecutor` /
`OperatorStepExecutor` traits, and `tool_config` over `CapabilityPolicy::decide`.

**Acceptance — met.**

* the cycle F1 named does not exist: `aep-schema` takes the leaf as a dependency
  (`crates/aep-schema/src/parse.rs:28`), the fourth sideways edge to a leaf and not a fifth
  mechanism;
* `tool_config` is a **function**, and it decides by `decide()` and never by `.allow`
  (`crates/aep-driver/src/tool.rs:82-90`), with `TOOL_CANDIDATES` asking about
  `deployment.create:production` in its scoped form because coverage widens outwards and never
  inwards;
* **27 tests** in `aep-driver-spec` and **34** in `aep-driver`, package-scoped;
* W3.1c held: both manifests carry `[lints] workspace = true`, `tests/determinism.rs` ships in each,
  and invariant 9's list in [`AGENTS.md`](../../AGENTS.md) names both crates in the same change —
  a purity claim stronger than `aep-engine`'s, which is why the lock, the pid probe and the run
  directory are `protocol-cli`'s (F19);
* `crates/aep-driver/tests/evidence_scan.rs` (216 lines) is invariant 7 one layer out: the driver
  constructs no `Evidence` value of its own.

### W3.2 — step maps are the fifth document kind

`drivers/development/default.yaml` over `adp/default/1`, seven states; `DocumentKind::StepMap`
(`crates/aep-schema/src/parse.rs:53`, directory `drivers` at `:102`); `drivers` as the **last** row
of `load_tree`'s `TREE` (`crates/aep-engine/src/load.rs:33`, with the ordering argued in place);
`schemas/generated/driver-steps.schema.json`, generated by `cargo xtask schema` and drift-checked by
the gate's `schema-check`.

**Acceptance — met.**

* the pin is **mandatory** and is a `WorkflowRef` spelled `adp/default/1` — § 4.2's `@1` stays
  standing as a recorded mistake, and nothing parses it;
* the committed map loads and is **refused when a state is renamed**
  (`crates/protocol-cli/tests/drive_cli.rs:387`) — a step map for a state that moved is an
  instruction sheet for a state graph it was not written against;
* every `cargo` line in it is a `command` step **the driver runs**, and no `llm` step has an
  `evidence:` key, because the `Llm` variant has no field for one.

### W3.3 — `protocol drive`

`protocol drive run | status | resume` (`crates/protocol-cli/src/drive.rs`, wired at
`crates/protocol-cli/src/main.rs:327` and `:565`), with the three executors that touch the world,
the run directory under `.engineering/runs/<task>/<ordinal>/`, the store lock at the one fixed path
`.engineering/runs/lock.json`, and the pid-liveness probe — all in `protocol-cli`, per F19.

**Acceptance — met**, asserted by the fixture run in `crates/protocol-cli/tests/drive_cli.rs`
(8 tests) and by `crates/aep-driver/tests/driving.rs` (9 tests):

* **the run advances on evidence a verifier produced.** The fixture map buys six moves —
  `receive → specify → decompose → establish_verifiers → implement → verify → adversarial_verify` —
  the last three of them on `command`-step evidence, and stops. The cursor records
  `"visits"` for all seven states and `"iterations": 12`;
* **a blocked run prints the engine's sentence and does not reword it.** The cursor's `reasons`
  hold `adversarial_verify -> review: guard: evidence.missing == 0` and the report contains that
  string character for character — asserted across both surfaces, because a report that paraphrased
  a refusal would be a second, worse protocol;
* **a crashed step submits nothing** (D5) — `a_command_step_that_produced_no_verdict_submits_nothing_and_changes_nothing`;
* **the lock is the allocator** (F2): `create_new` at a fixed path before any run id exists, a
  second invocation refused by name with the holder's run id, and the lock gone on every exit path
  the driver controls;
* **`--resume` refuses every moved pin** — workflow, map digest and engine version — on a snapshot
  the engine itself would have accepted (`driving.rs:663`), which is the F20 hole;
* **a headless start refuses what only a person can answer**, and `--pause-on-approval` is the route
  through (`drive_cli.rs:342`); the driver constructs no `Evidence::Approval` under any flag.

### W3.4 — the hooks, as the driver's enforcement arm

`integrations/claude-code/hooks/` — `hooks.json` with two `PreToolUse` matchers, `store-integrity.sh`
on `Edit|Write|NotebookEdit`, `driven-surface.sh` on `Bash`, and `lib.sh` holding the payload
reader, the context reader and the decision writer.

**Acceptance — met.**

* **`store-integrity` is always on**, and it is not the hook layer § 3.6 refused: it reads no
  workflow state and asks the engine nothing. `Write` and `NotebookEdit` are denied under
  `.engineering/planning/**` by path; `Edit` is denied only when `old_string` or `new_string`
  crosses the `---` fence or writes one of the six machine-owned keys — which keeps the one edit the
  plugin exists to ask for, a targeted body edit, legal;
* **`driven-surface` is inert outside a driven run.** With no step context on disk it passes
  silently (`aep_load_context || aep_pass`), which is § 4.8's own rule that a plugin installed
  without the driver ships no per-state enforcement;
* **the decision log is the F14 channel.** Both hooks append to
  `<run-dir>/hook-decisions.jsonl`, located from the step context the driver rewrites before
  **every** `llm` step (`crates/protocol-cli/src/drive.rs:899-926`, format
  `aep.drive-step-context/1`) — rewritten per step and not per run, because `effective_policy`
  grants the state's capabilities on top of the plan's;
* **fail-closed without a parser.** With neither `jq` nor `python3` on `PATH`, a call that mentions
  the store and any `Bash` call inside a driven run is **denied with the reason**, never passed
  through unread.

### W3.5 — the seam, proved without a model

`crates/aep-driver/tests/shell_echo.rs`, 839 lines, **6 tests**, inside `cargo test --workspace` and
therefore inside `task check`: a second `LlmStepExecutor` that is a **real subprocess** reading the
prompt on stdin, and a second transcript reader for a dialect of its own returning `TraceIr`.

**Acceptance — met.** All three adapter points are exercised at once, with no model, no network and
no credential:

* the executor walks the published map to completion, and the number in its transcript was computed
  by `sh` from bytes that travelled down a pipe — no in-Rust fake produces it;
* **one `tool_config`, two vocabularies**: the same function renders into
  `["load-skill", "read-files", "run-tests", "write-files"]`, and a test asserts none of those is a
  Claude Code name, so the second rendering is not the first one tested twice;
* a shell is rendered **exactly when** `command.execute` is admitted, and no subagent spawner is
  ever rendered however much the policy admits;
* `check` and `to_evidence` mint a `trace_conformance` record from a transcript no Claude Code
  wrote.

**And the freeze held.** The second reader lives **in the test file**
(`read_shell_echo`, `shell_echo.rs:483`), not in `trace-spec`, and
`the_claude_code_adapter_refuses_the_dialect_this_files_own_reader_understands` (`:766`) pins the
refusal. § 4.9's decision was *do not add a trait speculatively*; adding a second shipped adapter
would have been the same mistake in a different file.

### W3.6 — the driven eval, and one real run

`integrations/claude-code/eval/run-driven.sh` with `driven.steps.yaml` (one honest `llm` step, one
deliberately-refused one, a driver-run validator after each, and an `operator` step as the
terminus) and two **step-scoped** trace specifications — 11 expectations for the honest session,
9 for the denial session. Not in `task check`, for the reason its neighbour is not: it reaches the
API and costs money.

**Acceptance — met, by a real run on 2026-08-21** (Claude Code 2.1.238; the run directory and both
transcripts survive under `$TMPDIR/driven-eval.KQzq6g`):

| what was asserted | what the run said |
|---|---|
| `protocol drive run` exits 0 | **exit 0** |
| the run stops where a person is owed something | cursor `status: awaiting_operator`, `state: decompose` |
| the whole verdict table | **28 pass · 0 fail · 8 advisory** |
| the hooks discriminated rather than refusing everything | **10 decisions — 6 allow, 4 deny**: one `driven-surface` deny in `receive`, and in `specify` one `driven-surface` plus two `store-integrity` |
| the store survived the denial step | `protocol artifact validate` exit 0; **0** artifacts carry the `revision: 99` the step was told to write |
| both transcripts, as documents | honest **11 ok / 0 gap / 0 unk**, denial **9 ok / 0 gap / 0 unk**, `protocol trace check` exit 0 on each |
| a record the engine would accept | `protocol trace evidence` minted a `trace_conformance` document from the honest transcript |
| cost | **$0.6976** for the two sessions |

**The profile the run needed, and why it is not a relaxation.** `profiles/development-driven.yaml`
(`development.driven`) is `development.standard` **plus `command.execute`**, and it exists because
of a consequence § 4.8's strongest property did not cost out: the planning store has no tool surface
other than the `protocol` CLI, so under `development.standard` a driven `llm` step can be told to
write a specification as an artifact and has no way to create one — the guard on
`artifact.specification.exists` never fails, it simply never moves. The narrow fix does not exist:
`command.execute:protocol` is a **parse error**, because scoping is for `Environment` on
`deployment.create` and `deployment.rollback` alone (`crates/aep-domain/src/capability.rs:272-280`).
So the grant takes § 4.8's own shape for this class of rule — **a capability grant with a hook
constraint** — and the two properties that make it survivable are stated in the profile's own header
and hold in the tree:

* **the approval floor is untouched.** `protocols/aep/1.yaml:37-39` still reads
  `production.write` and `deployment.create:production`, and this wave changed no protocol document;
* **the store's write guard no longer depends on the shell being absent.** § 4.8 row 6's matcher was
  exhaustive *given* that no development profile granted a shell; under this profile that premise is
  gone and it is **replaced rather than dropped** — `driven-surface.sh` denies every `Bash` that is
  not one simple invocation of `protocol artifact …` or `protocol trace …`, which is what would
  otherwise have routed around the write guard by way of `sed -i`. Both hooks ship together for that
  reason.

The document tree now holds **six** profiles, asserted at
`crates/aep-engine/tests/documents.rs:46-50`.

### W3.7 — the workflow renderer (operator-added, outside the reviewed breakdown)

**Not in the sketch, not in the review, and named as an addition rather than folded in.** The
operator asked for it after W3.6 landed; it is recorded here because a wave whose delivered set
quietly exceeds its reviewed set is a wave nobody can check.

`crates/aep-render` — 2,933 source lines — plus `protocol workflow render`
(`crates/protocol-cli/src/render.rs`). One `Scene` resolves the layout, the overlay and every piece
of text exactly once; `svg`, `html` and `ansi` answer only *how do I write this out*, and PNG is the
SVG handed to `rsvg-convert` by the CLI, because the crate runs no programs.

**Acceptance — met.**

* **four formats behind one scene** — `--format svg|html|png|tui`. The HTML page is self-contained
  and fetches nothing; PNG without `--out` is refused by name, and PNG without the rasteriser names
  the program and what to install rather than failing obscurely;
* **`--watch` is live and bounded**: `--format tui` with `--run` only, refused on the formats that
  write a document once and refused without a run, because there would be nothing to follow. The
  poll and its clock live in `protocol-cli`; the crate reads no clock, no terminal and no file;
* **byte-stable.** The same workflow and the same `RunView` render to the same bytes twice
  (`crates/aep-render/tests/determinism.rs`, `crates/protocol-cli/tests/render_cli.rs:170`), so a
  committed figure does not turn up in a diff. Every committed workflow renders (`render_cli.rs:444`);
* **it evaluates nothing.** The overlay arrives as a plain `RunView` and the crate depends on
  `aep-domain` alone — not on `aep-engine`, not on `aep-driver` — so a renderer cannot become a
  second protocol implementation with no suites behind it. `crates/aep-render/tests/boundary.rs`
  holds that line;
* **the reasons are verbatim.** `RunView::reasons` are the engine's own sentences, never
  paraphrased, truncated or re-ordered by any emitter;
* **three dependencies weighed and refused**, in the crate's own documentation: `graphviz`/`dot`,
  `ratatui` and `resvg`/`usvg`;
* **37 tests** in `aep-render`, **13** in `crates/protocol-cli/tests/render_cli.rs`
  (`cargo test -p aep-render` = 37, `cargo test -p protocol-cli` = 198).

**What it showed about the run directory, and what that owes.** Building an overlay from a run is
the first thing that ever read `.engineering/runs/<run>/` from outside the driver, and it found
three absences. They are one row of [`gap-register.md`](gap-register.md), owned by wave-4 hardening,
and none of them is a defect in the renderer: reasons reach it **flattened into strings** because
there is no `report.json`; there is **no per-transition record**, so the path is reconstructed from
the snapshot's `entered` list and nothing says which transition was attempted at each step; and a
**snapshot alone cannot say `Running`** — `from_snapshot` answers `RunStatus::Unknown` unless the
state is terminal, because guessing would put a moving-looking overlay on a run that died three days
ago.

### Three findings the driver agents returned, and what was done with each

Written down because a finding that changed nothing and a finding that changed a document look
identical a month later.

| finding | disposition |
|---|---|
| **(1) `tool_config` admits what `authorize`'s floor later refuses.** `aep_engine::policy::authorize` re-applies the protocol's approval floor on top of the policy's answer, and this function is specified as *admits iff `decide(..) == Allowed`*. So a floor-gated capability that a profile allows outright is offered as a tool and then refused at `authorize` | **documented, not changed.** The refusal still happens, so nothing ungoverned occurs; what the model sees is a tool it cannot successfully use. Closing it means handing the function the `Protocol` as well, which changes the **published adapter surface** (§ 4.9 point 2) — not a thing to take unilaterally inside a build wave. Recorded where somebody will meet it: `crates/aep-driver/src/tool.rs:25-34` |
| **(2) a driven `llm` step under `development.standard` cannot reach the planning store at all.** Every verb of the store's vocabulary is a shell command, and no development profile granted `command.execute` | **resolved, in the shape § 4.8 already uses for this class**: the `development.driven` profile grants the capability as its outer bound, `hooks/driven-surface.sh` is the inner one, and the profile's header says the grant exists so the `protocol` CLI is reachable and for no other reason. Pattern-based and best-effort, and both documents say so |
| **(3) the loop transitions when a state's steps are done, not after every step.** § 4.4's diagram put `transition` after every step, which is not what a step map means: a state's steps are an ordered list and the transition is attempted when the list is exhausted (`crates/aep-driver/src/route.rs:30-35`, `next_step` at `:50-63`) | **the semantic is the code's; § 4.4 is corrected** in the design, with the diagram left standing beside the correction rather than redrawn — the rule this page already applies to § 4.2's `@1` |

### What wave 3 deliberately did not do

* **A second shipped transcript adapter.** The trace freeze holds: the shell-echo reader is a free
  function inside a test file, and a test asserts the Claude Code adapter refuses its dialect. § 4.9
  refuses a trait in `trace-spec` until there is a second implementation to design it against, and
  W3.5's is deliberately not one.
* **Folding the hook-decision log into `Engine::authorize`.** The channel exists and the log is
  written; the fold is **deferred**, with its reason and what closes it in
  [`gap-register.md`](gap-register.md). Nothing ungoverned follows from the deferral: the log is
  already the gating record, and the decisions it holds were refusals, which change no state.
* **Driving real work.** Every run in this wave is a fixture or an eval. The first governed task on
  this repository's own backlog is [`harness-wave-4-governed-dogfood.md`](harness-wave-4-governed-dogfood.md) § W4.1,
  and the difference matters: a driver that has only ever driven a fixture is a driver whose step map
  was written to fit the fixture.
* **Any narrowing of what a model may write.** Text is still free, and the three mechanisms § 4.8
  names as what makes that safe are all in the tree rather than in a paragraph.
* **A release.** No tag, no version bump. The changelog entry is under `[Unreleased]`.
