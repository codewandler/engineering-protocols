# Gap register — every open question, and what closes it

Started 2026-08-20, after wave 5 shipped and wave 6 was scheduled. One row per gap that some
document records honestly but nothing yet closes. The rule for this page: a gap leaves it either
**by decision** (recorded here, implemented where stated) or **by code** (the row names the commit
or wave that closed it). A gap that quietly disappears from this page without either is the failure
mode this page exists to prevent.

Sources are the feasibility review (`docs/reviews/2026-08-20-next-waves-feasibility-review.md`),
the per-invariant *Enforced by* lines in `AGENTS.md`, and the honest-limits sections of the wave 4
and 5 records.

## Open, from 2026-08-21 — the harness family

The rows on this page that are neither closed by decision nor closed by code. The two the operator
tracks — **the driver**, and **the store's durability** — were opened deliberately by
[`harness-wave-1-planning-plugin.md`](harness-wave-1-planning-plugin.md) and its design,
[`harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md),
which is the correct way for a wave to leave something owed: name it here on the way in, rather than
have a reader find it by its absence. **The driver row is closed by code as of 2026-08-21** and has
moved to its own section below; the store's durability is still open here. The others were opened by
the waves that discovered them and each says which wave takes which slice.

**Harness wave 3 closed two rows and opened three**, which is the shape a build wave should have:
the three new ones were all found by using what the wave built, and each names what closes it rather
than who is annoyed by it.

**[`harness-wave-4-governed-dogfood.md`](harness-wave-4-governed-dogfood.md) takes a slice of the
store's durability and closes none of it**, and it is the named owner of two of the three rows wave 3
opened. It is proposed, not sequenced, and it opens behind wave 3, which is **delivered**. What it
takes, and what is left, is written into the rows themselves rather than summarised here, because a
summary above a table is the second copy that drifts.

| gap | what closes it |
|---|---|
| **The planning store is durable and is not a contract implementation.** `aep-backend-markdown` writes through its own `create`/`update` rather than through `CommandService` — deviation **D-P1** against invariant 14 — so the sixteen `aep-conformance` suites do not run against it, and it has no journal, no audit join and no history (**D-P3**). Until then, "there is a durable backend" is a claim the suites do not support | **P3**, the journal-backed `CommandService`/`QueryService` for the markdown store: the two write functions reroute through command envelopes, the journal becomes the history, and the store runs the sixteen suites. `AGENTS.md` § *Current state* states both halves in the meantime. **The slice harness wave 4 takes:** W4.0 puts real content in a real store — this repository's own `.engineering/planning/`, 33 artifacts, `protocol artifact validate` exit 0 on 2026-08-21, governing the plan that governs it — which is what turns D-P3's missing history from a register entry into something an operator notices. `story:completion-audit-join` in that store already carries `depends_on: story:journal-backed-store`, so the dependency is recorded where the work is, not only here. **What stays open after it:** all of P3. Wave 4 adds no journal, no envelopes and no audit join, and running the sixteen suites against the store is untouched by anything it does |
| **The harness-neutrality claim has never met a second real harness.** Every behavioural document here is published as harness-neutral and one adapter exists; trace wave 1 says the claim is untested (`trace-wave-1-transcript-checker.md:263-265`), and W3.5's shell-echo harness tests the *seam* with no model rather than the *portability*. Opened by [`harness-wave-4-governed-dogfood.md`](harness-wave-4-governed-dogfood.md) § W4.4, whose research input is [`../reviews/2026-08-21-codex-harness-research.md`](../reviews/2026-08-21-codex-harness-research.md) | **W4.4**, in one of three named tiers — a live Codex run decided by `protocol trace check` against the same specification; a reader tested against a recorded rollout; or a written refusal naming what the harness cannot express. The research already narrows it: Codex ships a stable `PreToolUse` hook with the same decision contract, so the plausible refusal is now about **rollout-format drift**, not about enforcement. Whichever tier lands is named here. Tracked in the store as `story:codex-adapter` under `epic:cross-harness-portability` |
| **A story's `implemented` is a claim nothing checks.** `protocol artifact move` consults a `LifecycleRegistry` and nothing else (`crates/aep-backend-markdown/src/document.rs:115-142`), so a status is whatever was typed. The rule that would fix it exists one layer down — `ess-conformance` gates a *task's* completion on independent evidence — and has no analogue for the artifact. Opened by [`harness-wave-4-governed-dogfood.md`](harness-wave-4-governed-dogfood.md) § W4.3 | a **verdict**, not a build: [`story-completion-evidence-design-v0.1.md`](../design/story-completion-evidence-design-v0.1.md) is proposed-not-accepted and W4.3's acceptance is accepting, accepting in part, or refusing it — with the reason recorded. Refused closes this row exactly as accepted does. The `delivers` row for `artifacts/relations/relations.yaml` (§ 3 of that design) is separable and lands whatever the verdict, because the relation is already in the binary (`crates/aep-domain/src/artifact.rs:957`) and only the shared document is short of it. Tracked in the store as `story:completion-needs-evidence` under `epic:evidence-gated-completion` |
| **A hook's decision is recorded and is not folded into the audit trail.** Design § 4.8 decided (F14) that the driver folds each `hook-decisions.jsonl` line into the execution through `Engine::authorize` after the step exits, because a hook is a separate process and `authorize` takes `&mut Execution`. Wave 3 built the **channel** — both hooks write the log, the driver writes the `step-context.json` they locate it from, and the driven eval reads it as gating evidence (10 decisions, 6 allow and 4 deny, on the run of 2026-08-21) — and **deferred the fold**. Opened by [`harness-wave-2-driver-decision.md`](harness-wave-2-driver-decision.md) § *Wave 3 — built* | the deferral has a stated reason rather than a discovered one: **every decision the log has ever held is a refusal, and a refusal changes no engine state**, so the fold adds *provenance to the audit trail* and not enforcement — and doing it wrong is worse than not doing it, because a fold that replayed a hook's deny as the driver's own would put the driver's name on somebody else's refusal, when the trail's whole value is that it says who asked. **What closes it:** an `authorize` ingestion that preserves the hook as the deciding party, together with the first case where a hook's decision would change what the engine does — which does not exist while hooks **deny and never grant** |
| **The per-state tool set is enforced twice and audited by nothing.** Design § 4.8 row 3 — the allowlist at session launch plus a `PreToolUse` hook over the same derived set — was to be audited by the 50th expectation kind. `env.tool_available` shipped (W3.0) and then showed that it reads the wrong list: **`SessionStart.tools` is the harness's tool *inventory*, not the session's allow rules.** The committed fixture `crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl` was launched with **nine** allowed tools and lists **thirty-two**; the driven runs pass **eight** and list **twenty-eight**. The kind stays and is load-bearing — it rules out *"the tool did not exist"* as an explanation for a refusal — but § 4.8 row 3 no longer claims an audit | **either of two, and the first is not this repository's to build:** (a) a **harness-side record of the effective allowlist** in the transcript — the launch flag as the session received it, distinct from the inventory — which an expectation kind then reads exactly as `env.tool_available` reads `tools` today; or (b) a **driver-written expectation**: `protocol drive` emits, per `llm` step, a trace specification carrying a `tool.absent` row for every tool `tool_config` did **not** admit, and `protocol trace check` decides the transcript against it. (b) is buildable here today and is **strictly weaker** — it catches a tool that was offered *and called*, never one that was merely offered — so it is an audit of the leak that mattered rather than of the allowlist |
| **A run directory cannot be read back as a full account of the run.** `protocol workflow render --run` (W3.7) is the first thing to read `.engineering/runs/<run>/` from outside the driver, and it found three absences at once: the engine's reasons arrive **flattened into strings**, because the cursor's `reasons` array is the only record and there is no `report.json`; there is **no per-transition record**, so the path is reconstructed from the snapshot's `entered` list and nothing says which transition was attempted where, or why the one that failed did; and a **snapshot alone cannot say `Running`** — `from_snapshot` answers `RunStatus::Unknown` unless the state is terminal, because guessing would put a moving-looking overlay on a run that died three days ago. None of the three is a defect in the renderer, and each is a thing the run did not write down | **harness wave 4's hardening item**, [`harness-wave-4-governed-dogfood.md`](harness-wave-4-governed-dogfood.md) § W4.2 — already the wave that puts observations behind wave 3's guessed numbers, and the one whose sibling item W4.1 accepts on *"the run is readable by somebody who was not there"*. Three writes close it: a `report.json` per run carrying the reasons structured as the engine produced them, a per-transition record beside the snapshot, and a status the **run** writes so a reader holding one document needs no second one to know whether anything is still moving |

## Closed by code, 2026-08-21 — harness wave 3, the reference driver

Two rows, both opened deliberately on the way in — the first by harness wave 1's design, the second
by the feasibility review that judged it — and both closed the same day by the wave that was
sequenced to close them. The build's record, with the acceptance for every item, is
[`harness-wave-2-driver-decision.md`](harness-wave-2-driver-decision.md) § *Wave 3 — built,
2026-08-21*.

| gap | what closed it |
|---|---|
| **The reference driver was decided and not built.** `docs/VISION.md` § *What this is deliberately not* says the repository ships one, and no crate implemented the harness contract. The six § 4 holes became **taken decisions** in wave 2 (`harness-wave-2-driver-decision.md`), the feasibility review judged them against the code (`../reviews/2026-08-21-driver-feasibility-review.md` — 23 confirmed, 14 needs-change, 3 infeasible, all 17 applied in W2.3), and wave 3 built them | **harness wave 3, 2026-08-21**, in the order the review required. **W3.0** `env.tool_available`, the 50th expectation kind, before the hooks. **W3.1** `crates/aep-driver-spec` (1,883 source lines, `aep-domain` only: `RawStepMap → StepMap`, `PinnedWorkflowRef`, the cursor, `ToolConfig`, both cross-validation phases) and `crates/aep-driver` (1,577 lines: the three-valued router, the executor traits, `tool_config` over `decide`) — two crates because one was the cycle F1 named. **W3.2** `drivers/development/default.yaml` over `adp/default/1`, `DocumentKind::StepMap`, `drivers` as the last `TREE` row, `schemas/generated/driver-steps.schema.json`. **W3.3** `protocol drive run\|status\|resume` with the three executors, the run directory, the lock at the one fixed path `.engineering/runs/lock.json` and the pid-liveness probe; the fixture run makes six moves on command-step evidence and stops with the engine's sentence verbatim. **W3.4** the plugin's hooks as the driver's enforcement arm — `store-integrity` always on, `driven-surface` inert outside a driven run, both writing `hook-decisions.jsonl`, both fail-closed without `jq` or `python3`. **W3.5** the shell-echo second harness inside `task check`: a second `LlmStepExecutor` that is a real subprocess and a second transcript reader, proving all three adapter points with no model, no network and no credential — and the trace freeze held, the second reader living in the test file with the Claude Code adapter's refusal of its dialect pinned by a test. **W3.6** the driven eval, run for real: drive exit 0, `awaiting_operator` at `decompose`, 28 pass / 0 fail / 8 advisory, 10 hook decisions, the store unforged, $0.6976. **What stays open after it:** *built* is not *adopted*. Harness wave 4 W4.1 drives one real story of this repository's own backlog end to end; one story driven once says the mechanism holds on real work and does not say driven runs are how work happens here. Three new rows above were opened by using what this wave built |
| **Does a hook deny increment `permission_denials`?** The review (F13) could not verify whether a `PreToolUse` deny shows up in the transcript's `permission_denials` count — the enforcement audit reads that field, and an ambiguous `0` reads as "nothing was denied" when it may mean "denials are counted elsewhere" | **W3.6's deliberate-denial case, 2026-08-21: yes, one-for-one.** On Claude Code 2.1.238, the denial session's three hook refusals — `Bash`, `Edit`, `Write` — produced exactly three `permission_denials` entries, each carrying the tool's name; the honest session's single refusal produced exactly one. Recorded in three places rather than one, because a fact that lives only in a register is a fact nobody reads at the moment they need it: design § 4.8 (*F13, answered*), `integrations/claude-code/eval/README.md`, and the unknowns table of `harness-wave-2-driver-decision.md`. **The row it supports is kept advisory**, and that is a decision rather than a hedge: it asserts a *model behaviour* — that something forbidden was attempted at all — on top of an *undocumented harness detail*, and the gating evidence stays on disk in the hook-decision log and in `protocol artifact validate`. Getting the case right took two runs: the first asked for a hand-edited `status:` and the model correctly used `protocol artifact move` instead, so the guard was never exercised. **A deliberate-denial case has to ask for something with no legal alternative**, which is why the target is now `revision:`, a field with no CLI verb at all |

## Closed by code, 2026-08-21 — transcript conformance, phase 2

One row, opened on the way in by phase 1 and closed the same day by the phase that had to wait for
the checker's real types. It is on this page rather than absent from it because a vocabulary with
no producer is exactly the shape of defect the page exists to catch — and it was named here while
it was still true.

| gap | what closed it |
|---|---|
| **The vocabulary admitted transcript conformance and nothing could produce it.** `EvidenceKind::TraceConformance` existed, `Verifier::TraceChecker` could establish it and `protocols/adp/1.yaml` declared both — but `Evidence` had no `TraceConformance` payload, so `Evidence::kind()` could never return the kind and the engine's admission check could never be reached with it (`crates/aep-engine/src/engine.rs:320-321` reads the kind off the payload). A protocol could require the kind and nothing could satisfy it | `Evidence::TraceConformance(TraceConformanceResult)` — the verdict, the three counts, every gapped expectation's id, the ids downgraded on the command line and the digest pair, typed so the two digests cannot be transposed; `trace_conformance.**` declared observable; `CheckReport::to_evidence` in `crates/trace-spec/src/evidence.rs`, on the producing side, with a producer nobody can set; and `protocol trace evidence`. The loop is asserted end to end rather than by inspection: `crates/protocol-cli/tests/trace_cli.rs` writes the document and feeds the file back to `protocol evaluate --evidence`, in both renderings |

Three decisions inside it, each taken deliberately and each recoverable from here rather than from
a diff:

- **The record is a summary, not the report.** An expectation's citation quotes the transcript —
  the prompt, the model's reasoning, file contents it read — and an evidence record is a thing
  people paste into pull requests. Counts, ids and two digests cross the boundary; the rows do not,
  and `--redact` is therefore not an option on the evidence verb because there is nothing left in
  the record for it to remove.
- **`trace_conformance.passed` ignores `--advisory`.** A downgrade moves the checker's exit code so
  a cost bound that drifted with model routing cannot turn a CI job red (design D6). It is a
  property of the *invocation*, not of the protocol's requirement, and a requirement a caller's own
  flag could satisfy would not be a requirement. The record names every downgraded id so the
  narrowing is visible, and the fact stays strictly stronger than exit 0 — the same polarity as
  everything else here: unproven is not proven.
- **`Evidence::spec_digest` does not opt in.** That accessor is the *resolved-model* digest the ESS
  revision binding compares against an artifact. A trace specification's digest is the digest of an
  authored YAML document about behaviour, and no ESS artifact will ever pin one — returning it
  would make every trace record fail the revision comparison for a reason unrelated to the
  revision. The match arm says so where a reader will look for it.

## Closed by decision, 2026-08-21

### D-5 — transcript conformance is its own evidence kind, not a `Verification`

The transcript-conformance design (§ 5.1) flags that `EvidenceKind` is a closed enum and that a
`TraceConformance` variant is therefore a **domain change**, belonging in the acceptance decision
rather than being discovered during implementation. Accepted, and the alternative refused for the
reason the design gives: reusing `Verification` would make a claim about *how an agent worked*
indistinguishable from every other verifier statement, and being distinguishable is the entire value
of the record.

**Executed by code, 2026-08-21 (trace-evidence phases 1 and 2).** Phase 1 is the vocabulary below;
phase 2 added the payload, the builder and the verb, and closed the row it opened — see *Closed by
code, 2026-08-21 — transcript conformance, phase 2* above.

- `EvidenceKind::TraceConformance`, wire name `trace_conformance`. No alias: the list of aliases
  exists for documents written against earlier drafts, and this kind has none.
- `Verifier::TraceChecker`, wire name `trace-checker`, the only class that can establish it — named
  separately from `conformance-runner` for the same reason that class was named separately in the
  first place, that an agent reporting on its own run is not a check of it, and the type says so.
  Deliberately **not** `artifact-validator`, which the design's own § 5.2 step example writes: a
  transcript is a record of the worker rather than an artifact of the work, and letting any artifact
  validator mint the claim gives away exactly the distinguishability this decision is about. The
  design's § 5.2 step example was the one place left disagreeing with the code, and phase 2
  corrected it in place (`docs/design/transcript-conformance-design-v0.1.md:782`).
- Declaration in a protocol document is **required**, not optional: the engine refuses a submission
  whose kind the protocol does not declare (`crates/aep-engine/src/engine.rs:321`, stated for
  harness authors at `docs/guide/harness.md:18`). Both spellings therefore go into
  `protocols/adp/1.yaml` beside `ess_conformance`, and not into the base protocol — development is
  the reversible direction, because widening a declaration to every profile later is additive and
  narrowing one is not.
- No new observable family. The engine projects `evidence.count.trace_conformance` from the kind's
  own name, and `aep/1` already declares `evidence.**` (`protocols/aep/1.yaml:119`). The
  `trace_conformance.**` family belongs with the payload that projects facts into it, which is the
  open row above.

## Closed by decision, 2026-08-20

### D-1 — predicate comparison in the diff: conservative canonical equality

The wave 5 record excludes entities and commands from the delta because their invariants and
conditions are predicates, "and predicate comparison is where an undecidable answer lives". That
sentence conflates two questions. Predicate *implication* — does the new `when` accept everything
the old one did — is undecidable in general and stays refused. Predicate *equality after
canonicalisation* is decidable and cheap, and it is all the delta needs:

- canonically equal ⇒ the construct did not change, and the delta says nothing
- canonically different ⇒ **changed**, no direction derived, and the impact closure invalidates
  everything that depends on it — the same fail-closed polarity as everything else

No "still valid" claim is ever produced from predicate reasoning; a rewritten-but-equivalent
predicate that canonicalisation cannot recognise reports as *changed* and costs a re-run, which is
the cheap error. This unblocks entities, commands, views and bindings joining the delta as a later
slice of `ess-diff`, with the four directional relations staying exactly where they are.

**Executed by code, 2026-08-21 (ESS W7.2).** The four families are compared; the canonical form is
the parsed `Predicate` exactly as the compiler resolves it (the parser's own simplifications, no
reordering, no rewrite rules), plus the author's statement where the model keeps one. See
`docs/plan/ess-wave-7-closing-the-loop.md` § W7.2.

### D-2 — linking two implementations that claim one obligation is an error

The synthesis design (§20–§21) proposes `link` over obligation implementations and separately
proposes multiple implementations per obligation, and the review (finding 8 under "What is
missing") notes nothing says how `link` chooses. Decided: it does not choose. In wave 6's linker,
zero implementations for an obligation is an unsatisfied obligation and two is an **ambiguity
error naming both**; selection among alternatives is `Realization` material (§30–§34) and stays
proposed with it. Recorded on the wave 6 plan page as a constraint on W6.3.

### D-3 — attested evidence: the proposal now exists, and is not accepted

`docs/VISION.md` names the gap: what the loop asks you to trust is that a producer declaring
itself independent is. Review finding M6 says neither design closes it, and nothing has proposed
closing it — which made it a gap with no owner. It now has a proposed shape, so accepting or
rejecting it is a decision rather than an omission:

- the conformance runner holds a keypair; the report carries a signature over the canonical report
  bytes plus the suite and specification digests
- `independent: true` stops being a self-declaration and becomes *derived*: present and valid
  signature from a registered runner key ⇒ independent; anything else ⇒ not
- key registration is deliberately out of scope of the proposal's first slice (a file of trusted
  keys beside the protocol documents is enough to make the property mechanical; rotation and
  revocation are real and later)

Status: **proposed, not accepted.** It adds a dependency class (signatures) to a workspace with
nine third-party crates and a written policy about that, which is exactly the kind of cost the
acceptance decision is for. Until accepted, the VISION's trust sentence stays as written — narrow
and named.

### D-4 — the model digest widens before anything else rests on it

`SuiteProvenance`/`Provenance` carry a 16-hex (64-bit) truncation of the model digest
(`crates/ess-gen/src/provenance.rs`), and review M5 said to widen it "if a completion decision is
going to rest on it". Gate G19 then made completion decisions rest on it, and wave 5 made suite
acceptance rest on it (`ess impact` refuses a suite whose digest mismatches). A 64-bit digest is
fine against drift and weak against construction. Decided: widen to the full SHA-256 hex in the
next model-touching batch (below), regenerating committed artifacts once. Not done live because
every committed suite and projection embeds the digest and the regeneration belongs in one commit.

## Closed by code, 2026-08-21 — wave 7, W7.5

One gap, closed by one word, and it had never been on this list because nothing had needed it: the
model could say what a component accepts and publishes and could not say **where its callers are**.
Every synthesised system therefore had exactly one derivable transport — the in-process log a
binding's `at_least_once` determines — and a second one could only have been chosen by preference,
which the wave-6 rule forbids.

Each new guard was verified by mutation before being trusted: the one-line violation it exists to
catch was applied, the failure was watched naming the defect, and the mutation reverted.

| gap | what closes it now |
|---|---|
| the model cannot state that a component's surface is reached from outside the process, so no specification can *derive* a network transport | `reached_by:` on a component (`ess-domain`), a closed two-word set — `in_process` (the default, and what silence has always meant) and `network` — validated by the raw→validated pair and refused as `EmptyDeclaration` when a network surface has neither an accepted command nor an owned domain that projects a view. Skipped from the resolved model's serialisation when unstated, so **no existing specification's digest moved**. Mutation: the rule made to return no errors, caught by `a_component_reached_over_a_network_that_serves_nothing_is_refused` |
| `openapi.rs`'s "what this refuses to guess" carried *pagination, filtering, sorting* — a view is in the IR and nothing said how one is read | the row is closed by a declaration rather than by a generator's opinion: where a component says `network`, each view its domains declare gets `GET /{domain}/views/{view}`, its rows under one key, its declared filter in the response description and its consistency as `x-ess-consistency`. Still no page size, no cursor, no ordering and no filter parameter, because the specification states none. Mutation: view paths published unconditionally, caught by `a_view_is_served_only_where_the_specification_says_something_outside_reads_it` **and** by `generate-check` on `openapi/invoice-service.yaml` |
| a synthesised server and its published contract could disagree about a path or a status | `ess_gen::http` holds one route mapping and one status mapping, read by the `OpenAPI` projection, the Rust emitter and the Go emitter. Mutation: two rows dropped from the emitted route table, caught by `the_routes_a_server_answers_are_the_routes_the_contract_declares` with both sets printed |

## Closed by code, 2026-08-21 — wave 6.5 chunk B

The last two rows of the post-wave-6 hardening batch. As with chunk A, each new guard was verified
by mutation before being trusted: the one-line violation it exists to catch was applied, the
failure was watched naming the defect, and the mutation reverted.

| gap | what closes it now |
|---|---|
| nothing relates a command's input to an emitted event payload — the one fault caught by nothing | a `payload:` declaration on a command outcome (`ess-domain`), resolved and type-checked with the binding mapping's own discipline (`ess-compiler`, `ESS-COMMAND-003` and the shared `ESS-COMMAND-002`); synthesis asserts the declared values in `ExpectEvent`, and `wrong-event-payload` moved to the caught side of the matrix — designated by `billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled`, blast radius 2. A field with no declared source stays *undetermined* by decision: the suite asserts its presence and type and never a value, and there is no `unmapped_payload_field` refusal |
| value-object invariant scenarios not synthesised (design §20) | `ScenarioId::ValueInvariant` — `<type>/invariant/at/<view>/<field>` — one scenario per observable field position that holds a value of the type, the type's own predicate rebased onto the position and required of every row with at least one row demanded. Billing gains two (`Money` at `InvoiceById.total` and `OutstandingInvoices.total`, 27→29 scenarios, refusals 1→0); what has no witness keeps a refusal under the new honest cause `ESS-SYNTH-013` rather than "not synthesised yet". The family's own fault, `negative-projected-total`, is caught by the position it corrupts with blast radius 2 |

A change to either construct lands in the command family, which the semantic delta deliberately
does not compare until W7.2 — so `ess impact` gained fail-closed mechanism 6: the uncompared
families are checked for canonical equality, and any difference owes the whole suite
(`WholeSuite::UncomparedFamilyChanged`, with the test that a payload-only change is `Whole` over an
empty delta).

## Closed by code, 2026-08-20 — wave 6.5 chunk A

Each guard was verified by mutation before being trusted: the one-line violation it exists to catch
was applied, the failure was watched naming the defect, and the mutation reverted.

| gap | what closes it now |
|---|---|
| invariant 7 — "engine never manufactures evidence" | `crates/aep-engine/tests/evidence_scan.rs`: payload types read off `Evidence` itself, every construction in shipped engine code refused, destructuring and the `submit_evidence` envelope allowed. Mutation: a fabricated `Evidence::TestResult` in `submit_evidence`, caught at file:line |
| invariant 8 — clock/RNG-free domain crate | `crates/aep-domain/tests/determinism.rs` and `crates/ess-gen/tests/determinism.rs` extend the banned-token scan to both crates that stated the property unscanned (`ess-diff` and `ess-synth` already scanned themselves; `ess-domain` states no claim). Mutations: a `SystemTime::now` in `time.rs`, a `HashMap` import in `types.rs`, both caught |
| invariant 14 — one write path | `crates/aep-contract/tests/write_surface.rs`: every method of every public trait enumerated and pinned; `CommandService::execute` is the one write path. Mutation: a default-bodied `fn purge` on `CommandService`, caught by name |
| digest widening (D-4) | `crates/ess-gen/src/provenance.rs` writes the full 64-hex SHA-256; every committed projection, suite and synthesised workspace regenerated once; `SpecDigest` still parses 16–64 so a pre-widening record fails at the comparison that names both digests, not at parse |
| property-based testing phase 1 (`proptest`) | `crates/aep-domain/tests/truth_laws.rs` (Kleene laws over generated expressions) and `crates/ess-compiler/tests/adversarial.rs` (the recorded property: any generated document is refused with reasons or compiles byte-identically twice). Fixed seeds, so the gate cannot be flaky. Mutations: `and` collapsing `Unknown` to `False`, a clock read in `to_canonical_json`, both caught with shrunk counterexamples |

## Not gaps, verified closed

Recorded so nobody re-opens them: command↔transition and command↔entity exist in the model
(wave 3.5, gates); witness synthesis refuses on `Truth::Unknown` and blames the specification
(`crates/ess-conformance/src/input.rs:165`, `:332`); scenario synthesis refuses as data rather
than failing the build (suites list every construct that got no scenario, with the reason); the
wave 3.5 page's gate count no longer disagrees with itself.
