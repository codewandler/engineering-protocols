# Feasibility review — the semantic diff, impact and evolution design against the code that exists

> **Subject:** [`docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md`](../design/ess-semantic-diff-impact-evolution-design-v0.1.md), 2416 lines, filed 2026-08-20, never reviewed.
> **Reviewed against:** `main` at `34cac07`. **`crates/ess-compiler/` and `crates/ess-domain/` were being edited by an implementation agent throughout this review**, so line numbers in those two crates drift; symbol names are given wherever a number is likely to have moved.
> **Question asked:** not *should we do this next* — that is settled ([`docs/plan/ess-wave-3.5-reconciliation.md`](../plan/ess-wave-3.5-reconciliation.md) § *Decisions taken*, decision 2: **ESS wave 4, then semantic diff**). The question is *is it buildable as written*, and *what must wave 4 not foreclose*.
> **Method:** every claim carries a `file:line`, a section number, or a command's output. Where it does not, it says "I'm guessing" or "inferred".
> **Not reviewed:** the writing. Sections whose subject is a document that does not exist are marked as such rather than assessed.

---

## Verdict

**Buildable, in four waves, not one — and two of its seventy-eight sections belong to a different product.**

The core is sound and unusually well matched to what landed. Of roughly sixty change kinds the taxonomy names across §12–§20, **two name a field the model does not have** (`PortChanged`, `TransportRequirementChanged`) and **one is attached to the wrong construct** (`TransitionTriggerChanged`); the rest map onto real `EssIr` fields. Its central architectural choice — diff the compiled IR, not the source and not the projections (§4) — is the one this repository would have taken anyway, and `EssIr` is already shaped for it: name-keyed `BTreeMap`s, `PartialEq` throughout, canonical JSON with a byte-comparison test (`EssIr` and `EssIr::to_canonical_json`, `crates/ess-compiler/src/ir.rs`). Its refusal to infer renames from similarity (§6) is exactly right and is already the model's position.

Three things are wrong, in descending order of what they cost.

**First, the invalidation model (§32, §33) needs gate G19 as a precondition and, as written, contradicts it.** G19 makes evidence *fail closed* against a specification revision. §33's verdict vocabulary includes "still valid", and §26 counts `evidence_records_invalidated` as a subset — which is evidence *failing open* by default, with a delta engine deciding the subset. A missed dependency edge under G19 costs a re-run. The same missed edge without G19 costs a false conformance claim. §32 refines G19; it cannot replace it, and the design does not say so about evidence records (it says it only about the conformance *gate*).

**Second, this is four waves plus a blocked programme.** Track SD (§9–§39) is roughly the size of ESS waves 1–3 combined. Track VI (§32, §33) is blocked on wave 4 for half and on `contract_digest` for the other half — a symbol that exists in **no code** and only in an unreviewed, unreconciled, unsequenced design (`grep -rn contract_digest crates/` returns nothing; it appears only in `ess-structural-synthesis-obligations-realizations-design-v0.1.md` and this document). Track EV (§43–§52) is blocked on `Realization`, from the same unsequenced design. The header's claim that it "can be introduced as soon as `EssIr` is stable" is true of §9–§26 and false of everything after §31.

**Third, §36 and §38 are a different product wearing this vocabulary.** [`docs/VISION.md:153`](../VISION.md) — "Not an LLM orchestration framework". §36 is an orchestration loop; §38 is a search driver over it. §51/§52 and the roadmap's EV6 cross the other stated line (`docs/VISION.md:157-162`: "nothing here … applies a plan or watches a rollout"). §37 is fine — a deterministic constraint check over deterministic facts is engineering, and it is the only one of the three that is.

One structural note that outranks all of it: `docs/VISION.md:146-149` says that "specified once and compiled" says nothing about a system *changing*, and that absorbing this design into the thesis "is a decision someone has to take deliberately, with a reason." Accepting even track SD is a **vision amendment**, not only a wave.

---

## Findings, by severity

| # | finding | cost to address **now** | cost **after wave 4** |
|---|---|---|---|
| **S1** | §32/§33 need G19 as a precondition and contradict it on evidence records | one paragraph in the design; G19 already scheduled | a selective-invalidation engine that can vouch for stale evidence |
| **S2** | Wave 4's `ScenarioId` may be a **counter** (W4 §37) or a **semantic name** (W4 §21); both satisfy W4 §50, only one survives a spec change | one sentence + one test in wave 4 | re-keying the committed suite, the fault matrix and every committed report |
| **S3** | This is four waves plus a blocked programme, not one | resequence the plan page | a wave that does not land |
| **S4** | §36/§38 are outside `docs/VISION.md:153`; §51/§52/EV6 outside `:157-162` | delete two sections, or amend VISION deliberately | an LLM loop inside the trusted core |
| **S5** | Diff is the **first two-IR consumer**; a cross-IR handle lookup panics by design and nothing typed prevents it | a stated discipline + one test in wave 4's suite work | a panic in a review tool, reported as a crash |
| **S6** | `ConformanceScenario.source` must be **complete**, not just the originating construct, or §32's intersection is unsound | one field's documented semantics in wave 4 | regenerating the whole committed suite |
| **S7** | Name collisions: `Provenance` ×2, `Artifact` ×2, `Digest` vs `SpecDigest`, `Profile`, `Obligation`, `Revision` ×3, `ChangeSet` | rename in the design text | two answers to "which specification produced this" |
| **S8** | §22's graph has a live partial duplicate in `protocol-cli` | move `ess_graph_of` down when the graph lands | two graphs that disagree |
| **S9** | Taxonomy gaps: 2 kinds name absent fields, 1 misattributed, 8 real constructs have no change kind — including the three that decide which scenarios wave 4 generates | edit the taxonomy | a diff that reports no change when the suite changed |
| **S10** | §33 assumes per-element artifact provenance; `ess_gen::Provenance` is per-*model*, so today any change invalidates all 27 artifacts | record the intent in wave 4's D4 | a manifest retrofitted onto committed artifacts |
| **S11** | `Version(u32)` is major-only; `billing/v3` cannot identify a revision, so §35/§41/§66 are under-specified | edit three examples | a proposal `base:` that resolves to two models |
| **S12** | `FreshnessPolicy::BoundToDependencySet` already declares §33's policy and **nothing enforces it** | none — it is a reuse instruction | a second freshness vocabulary |
| **S13** | Two plan pages disagree on when diff starts | one line in `ess-roadmap.md` | an agent building from the stale one |
| **S14** | §5 precondition 3 assumes an IR format version that does not exist; a committed `EssDelta` needs a `Raw*` pair per invariant 2 | two lines in the design | a validated type with `Deserialize` on it |

---

## S1 — §32/§33 need G19, and contradict it on evidence records (critical, decision-relevant)

**This is the most decision-relevant question in the review, so it is answered first and plainly.**

> **The design's invalidation model needs G19 as a precondition. It does not subsume G19, and as written it conflicts with G19's verdict vocabulary.**

### What G11 closed and G19 has not

`EssConformanceResult` now carries a required `SpecDigest` (`crates/aep-domain/src/evidence.rs:812`) and answers `attests(&SpecDigest) -> bool` (`:845`). Its own documentation makes the argument the design would need to make: "`billing/v3` is a label two different resolutions can share; a digest is not" (`:806-811`). That is G11, recorded closed at `docs/plan/ess-wave-3.5-reconciliation.md:43`.

G19 is the other half (`:37`, and § *G19* at `:273-295`): the artifact must carry the digest so `EvidenceRequirement` has something to compare against. Today nothing does. The gate page states the consequence exactly — "a suite run against yesterday's specification produces evidence that satisfies today's requirement" — and names the mirror it copies: `ReviewRequirement::evaluate` calling `review.covers(artifact)` (`crates/aep-domain/src/review.rs:244-258`), which refuses an approval of version 3 for version 7.

### Why the design needs it rather than replacing it

The two mechanisms point in opposite directions.

| | G19 | design §32/§33 |
|---|---|---|
| default verdict for evidence after a spec change | **invalid** | **valid**, unless a change intersects its provenance |
| what a missing dependency edge costs | nothing — the digest already differs | a stale record accepted as current |
| what it is for | safety | scope estimation and review |

Under G19, every conformance record from the previous revision fails, because every one carries the previous digest. Under §32 alone, only the intersecting scenarios are flagged and the rest are silently retained. **A selective model layered on nothing is not a refinement of a coarse model; it is the absence of one.** Without G19 there is no coarse verdict for §32 to refine, and the delta engine becomes the sole authority on whether evidence is current — which is the same defect class as an agent certifying its own work, one level up.

The design half-knows this. §32's "Important rule" says selective analysis "does not replace the final full ESS conformance gate." But it says that about the *gate*, not about *evidence records*, and §33's per-obligation verdict list opens with **"still valid"** while §26 counts `evidence_records_invalidated` as a number that can be less than the total. Those two are precisely what G19 refuses. The design needs one added sentence with the direction of the implication in it:

> Selective invalidation may only **add** invalidations to those the specification digest already produces. It may never mark a record valid that the digest binding invalidated.

### The conflict of vocabulary, and how to avoid it cheaply

G19's fix is **one digest per specification**, on the `Artifact`. §33 wants **many digests** — per generated artifact, per obligation contract. Both are correct at their level, and two digest notions in one repository is the failure the wave 4 design already warns about at its §23 ("two provenance types in one repository are two answers to 'which specification produced this'").

Cheap avoidance, entirely within G19's scope: name the field on `Artifact` `spec_digest` and type it `SpecDigest`, not `digest: Option<String>`. `aep_domain::evidence::Provenance` already has a `digest: Option<String>` field for a *different* thing — a digest of raw tool output for tamper detection (`crates/aep-domain/src/evidence.rs:940-942`). A second bare `digest` on `Artifact` would collide with it in reading, and `SpecDigest` already exists, is validated (16–64 lower-case hex, `:696-707`) and is what `ess-gen` writes.

### Verdict on the question as posed

- **Subsumes G19?** No. Opposite default; different property.
- **Conflicts with G19?** Yes, at the verdict vocabulary — "still valid" evidence across a revision boundary is what G19 exists to refuse.
- **Needs G19 as a precondition?** **Yes.** Track VI must not start before G19 closes, and the design should say so in its §5 preconditions rather than in §32's body.

---

## S2 — Wave 4's scenario id: a counter and a semantic name, and both pass its acceptance (critical, foreclosure)

The wave 4 design contains two incompatible answers.

**§21 (`:952`)** gives `ConformanceScenario { id: ScenarioId, … }` and **§23** shows a *derived semantic* id:

```text
scenario: billing.invoice.CreateInvoice/outcome/rejected
```

**§37 (`:1737` at `34cac07`; `:1743` in the working tree, which is being edited)** gives the runner "an id source — a monotonic counter, seeded from the suite", and lists "correlation ids, **scenario ids** and idempotency keys" as what it mints.

**§50 (`:2305` at `34cac07`; `:2311` in the working tree)** accepts either: "Scenario IDs are stable across unchanged input." A counter satisfies that trivially.

### Why the counter forecloses diff

A monotonic counter is stable across *unchanged* input and unstable across *changed* input in the worst possible way: adding one command renumbers every scenario after it. §32's whole mechanism is intersecting a change set with a scenario set across two revisions. With counter ids there is no scenario set to intersect — the identifiers are not the same identifiers.

The cost lands on things wave 4 commits. §38 of the wave 4 design (`:1767-1790`) commits the suite to `generated/conformance/suite.json`, drift-checked by `cargo xtask generate --check`, and says the point is to let "the wrong-implementation matrix refer to scenario IDs that do not change accidentally." With counter ids, a change to one command makes the drift check go red on scenarios that did not change, and the fault matrix's references silently move.

There is also an internal contradiction worth flagging while wave 4 is still open, independent of diff: §21 (`:986-1004`) says the committed suite is read back "by a runner in a later process on a later checkout, and referred to by scenario id from the fault matrix." If the *runner* mints scenario ids from a counter (§37), the committed suite does not contain them, and §21's `id` field has nothing in it at generation time.

### What to preserve, concretely

1. `ScenarioId` is **derived from the scenario's `Vec<EssSemanticRef>`** at generation time — construct name, plus the branch or transition, exactly as §23's example shows. The runner's counter mints correlation ids and idempotency keys only.
2. Add to wave 4's §50 acceptance: *"a scenario whose source refs are unchanged keeps a byte-identical id across a specification change."*
3. The test that proves it: compile the billing example, generate the suite, add one unrelated command to a second copy, regenerate, and assert every pre-existing scenario id is unchanged. That is the mutation this rule exists to catch, and it is the repository's own standard (`AGENTS.md:188-191`, "verify a guard by breaking it").

**Cost now:** one sentence in the wave 4 design, one derivation rule, one test.
**Cost after wave 4:** re-keying `generated/conformance/suite.json`, the fault matrix, every committed report, and the drift baseline — with the ids referenced from prose in at least two design documents.

---

## S3 — This is four waves plus a blocked programme (critical, scope)

### Measured baseline

```console
$ git diff --shortstat 0.2.1 0.3.0-ess-wave-1        # ESS wave 1
 65 files changed, 18285 insertions(+), 1604 deletions(-)
$ git diff --shortstat 0.3.0-ess-wave-1 0.3.1-ess-wave-2
 33 files changed, 10661 insertions(+), 133 deletions(-)
$ git diff --shortstat 0.3.1-ess-wave-2 0.3.2-ess-wave-3
 54 files changed, 14283 insertions(+), 37 deletions(-)
```

Split by area, and with tests from `git tag -n99` (442 at `0.2.1` → 642 → 777 → 916):

| wave | `crates/` | `generated/`+`examples/` | `docs/` | `schemas/` | tests |
|---|---:|---:|---:|---:|---:|
| ESS 1 — the model | 12 161 | 318 | 2 462 | 2 152 | +200 |
| ESS 2 — the compiler | 9 641 | 82 | 460 | 392 | +135 |
| ESS 3 — the projections | 11 981 | 1 689 | 40 | — | +139 |

A wave in this repository is **10–12k lines of `crates/` and 135–200 tests.**

### Where the seams are

The repository's practice is a demo that bites, then the machinery. Applying it:

| slice | sections | why it is a seam | est. `crates/` | blocked on |
|---|---|---|---:|---|
| **SD-a** — the delta | §5–§21, §59, §60, §63, §67, §71 | one artifact, one CLI verb, no graph. Falsifiable on its own | ~4–6k | nothing beyond wave 4 landing first |
| **SD-b** — the graph and impact | §22–§26, §68 | 13 typed edge kinds, closure, explainable paths, and absorbing the CLI's private graph (S8) | ~8–10k | SD-a |
| **SD-c** — verification impact | §32, VI1 | delta → scenario provenance intersection | ~3–4k | **wave 4** *and* **G19** (S1) |
| **SD-d** — policy and compatibility | §27–§31, §35, §37, §69 | two new document types, two `Raw*`→validated pairs, two schemas, a precedence question | ~8–10k | SD-b |
| **EV** — evolution | §43–§52, §66, §70 | not a wave — a programme | unknown | `Realization`, `SynthesisPlan`, `contract_digest`, **all of which exist only in an unsequenced, unreconciled design** |
| **out** | §36, §38 | see S4 | — | — |
| **unbuildable** | §33, VI2, VI3 | needs a synthesis manifest and `contract_digest` | — | same unsequenced design |

SD-a through SD-d is roughly the whole of ESS waves 1–3 again. `contract_digest` is verified absent from the workspace:

```console
$ grep -rn "contract_digest" crates/
$ grep -rln "contract_digest" docs/
docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md
docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md
docs/reviews/2026-08-20-vision-review.md
docs/reviews/2026-08-20-next-waves-feasibility-review.md
```

`AGENTS.md:33` records that its home design is "reviewed … and **not reconciled**", read by that review as four waves rather than one, and unsequenced. So §33, VI2, VI3, and all of track EV are **transitively blocked on an unaccepted design** — a fact the header's "additive, can start as soon as `EssIr` is stable" does not carry.

### The falsifiable claim in §56

§56 says track SD "can begin once `EssIr` is considered stable enough to compare." That is true of SD-a and SD-b and false of SD-c and SD-d. The design's own roadmap (§76) puts SD1–SD5 under "NOW", which reads as one wave. It is four.

---

## S4 — §36 and §38 are a different product; §51/§52/EV6 cross the other line (high, vision)

Three sections propose judgement rather than computation. They do not have the same answer.

| § | what it is | verdict |
|---|---|---|
| **§37** Change budgets | `ProposalConstraint` checked against deterministic impact facts, returning structured violations | **belongs here.** It is a predicate over facts the engine already produced, and it is the same shape as `CapabilityPolicy::decide` — a rule that names what decided |
| **§36** LLM proposal evaluation loop | an orchestration loop: objective → LLM candidate → compile → delta → policy → revise | **does not belong.** `docs/VISION.md:153`: "Not an LLM orchestration framework" |
| **§38** Multi-candidate architecture search | run §36 N times and expose a Pareto frontier | **does not belong.** It is §36 with a driver |

The distinction is sharp and the design almost draws it itself. §59 already states the correct rule — "no LLM calls inside the diff or impact engine … An LLM may consume the report. It is never part of producing the authoritative report." Everything in §36 and §38 that this repository should own is **already covered by §37 plus §59**: a machine-readable `ImpactReport` (§25), a machine-checkable constraint set (§37), and a refusal to let an LLM near the producer (§59). The loop that consumes them is an agent harness — the thing `docs/guide/harness.md` addresses as somebody else's program.

§62 (security and trust boundaries) is a good section and should stay; it is a statement about capabilities, and capabilities are this repository's subject.

Separately, the evolution tail crosses the *other* stated line. `docs/VISION.md:157-162` scopes out operating a system: "nothing here calls a cloud API, holds a credential, applies a plan or watches a rollout." §50 is safe — it says explicitly "creating a plan must not mutate production", and an inspectable plan is an artifact. But §51 (release as a transition), §52 (evolution conformance over `R3 → R4`), §48 (compatibility windows, dual-write, rolling upgrades) and the roadmap's **EV6 "AOP-governed apply / rollback transition"** are the applying and the watching. `ArtifactKind::MigrationPlan` and `ArtifactKind::ReleasePlan` already exist (`crates/aep-domain/src/artifact.rs:370-372`) — generating one is in scope; executing it is not.

**Inferred, not verified:** that §52's transition oracle would require a live target. The design does not say, and I have not found a way to test `R3 → R4` invariants without running both. Labelled a hypothesis.

---

## S5 — Diff is the first two-IR consumer, and the handle model assumes one (high)

§5 precondition 4 states the hazard: "No handle minted by one `EssIr` is used as a lookup key in the other." It states it and then never returns to it, and it is the design's single most under-weighted engineering risk.

Every handle is a newtype over a name — `TypeHandle(QualifiedName)`, `EntityHandle(QualifiedName)`, `ComponentHandle(ComponentName)` (`crates/ess-compiler/src/ir.rs`, `handles!` block). Two compilations of two revisions therefore mint **structurally identical, freely interchangeable** handles, and the accessor's failure mode is a panic:

> "`{handle}` is not a {} this IR declares: a handle belongs to the IR that minted it" — `crates/ess-compiler/src/ir.rs:141` — the line two repository documents already cite for it; the working tree has since shifted it a few lines

The repository already knows this. The wave 4 design cites the same line for why a committed suite carries semantic names rather than handles (§21, `:986-1004`), and the reconciliation page lists it as a hazard for the property-testing work: "a generator that mixes two compilations will look like a crash rather than a mistake" (§ *The property-based work*).

For every consumer so far this has been an edge case. **For a diff engine it is the normal case** — holding two IRs is the entire job, and the natural code is `after.entity(&before_binding.event)`. Nothing in the type system stops it. The panic message is good, but a review tool that panics on a specification pair is reported to Timo as a crash, not as a programming mistake.

Three options, cheapest first:

1. **Discipline plus a test.** The diff engine never calls an `EssIr` accessor with a handle it did not obtain from that same `EssIr`; it resolves through `handle.name()` and the target IR's map. Enforced by a source scan in the diff crate's `tests/`, on the model of `crates/ess-compiler/tests/billing.rs`'s existing banned-token scan (`AGENTS.md:120-123`). **Cost: hours.**
2. **A `Side` wrapper in the diff crate** — `Before(&EssIr)` / `After(&EssIr)` newtypes, so the accessor is only reachable through the right side. Cost: a day, no change to `ess-compiler`.
3. **Brand the handles with a lifetime.** Correct and expensive; changes every signature in `ess-gen` and every projection. **Not recommended** — the projections do not need it and would pay for it.

Take (1) and note (2) as the fallback. Nothing here needs to happen before wave 4; it needs to be in the diff wave's plan rather than discovered.

---

## S6 — `ConformanceScenario.source` must be complete, or §32 is unsound (high, foreclosure)

Wave 4 §21 (`:955`) gives `ConformanceScenario { …, source: Vec<EssSemanticRef> }`. §23 illustrates it:

```text
derived_from:
    command billing.invoice.CreateInvoice
    outcome rejected
    error   billing.invoice.InvalidAmount
```

Read as **provenance** — "which construct caused this scenario to exist" — that list is right and complete. Read as the input to §32's intersection — "which constructs, if changed, make this scenario stale" — it is **not**, and the difference is silent.

A scenario that executes `CreateInvoice.accepted` and then asserts a view also depends on: the view's `filter`, its `consistency`, its `assertion_style`, the entity it projects, that entity's `invariants`, the entity's `lifecycle` transition the outcome takes, and every named type reachable from the command input. Change the view's `consistency` from `read_your_writes` to `eventual` (`crates/ess-domain/src/view.rs:67-79`) and that scenario's expected behaviour changes while its `derived_from` list does not mention the view at all.

§32's mechanism is `change → source semantic refs → scenario provenance intersection`. If `source` is the causing construct rather than the dependency set, the intersection under-reports, and under-reporting in an invalidation model means **keeping evidence that should have been invalidated** — S1 again, from a different direction.

**What to preserve in wave 4:** document `source` as *every ESS construct the scenario reads or asserts against*, not only the one that generated it, and generate it from the same walk the synthesizer already does to build the scenario. Add one acceptance line: *"a scenario's `source` names every construct whose change would change the scenario."*

**Cost now:** it is the same walk; recording what it visits is nearly free while the walk is being written.
**Cost after wave 4:** re-deriving provenance for a committed suite, which means regenerating it, which means the drift check and every committed report move.

---

## S7 — Name collisions and duplicate abstractions (high)

Two `Provenance` types already exist in this workspace and the design proposes vocabulary that lands on both.

| the design names (§) | the real one | what it means there |
|---|---|---|
| `Provenance` on every change (§11), report provenance (§61) | `ess_gen::Provenance` — `crates/ess-gen/src/provenance.rs:13` | *which specification produced this generated file*: system, spec version, model digest, compiler version, generator version |
| same | `aep_domain::evidence::Provenance` — `crates/aep-domain/src/evidence.rs:924` | *how an observation was made*: command, tool, revision, workspace, environment, output digest, inputs |
| same | `ArtifactProvenance` — `crates/aep-domain/src/artifact.rs:1144` | *who made an artifact and from what* |
| same | `EntityProvenance` — `crates/aep-domain/src/entity.rs:776` | provenance on a graph entity |
| `Digest` in `EssRevisionRef` (§9), report identity (§61) | **`SpecDigest`** — `crates/aep-domain/src/evidence.rs:696-707` | validated 16–64 lower-case hex; refuses upper case so one model has one spelling. Already the workspace's answer. Use it; do not declare `Digest` |
| `EssDelta` artifact kind (§34); "generated artifact" (§33) | **two** `Artifact` types: `aep_domain::artifact::Artifact` (`:1215` — the graph node, with `id`/`kind`/`version`/`freshness`) and `ess_gen::Artifact` (`crates/ess-gen/src/artifact.rs:11` — `{ path, contents }`, one generated file) | §33 and §34 use the word for both in adjacent sections without distinguishing them |
| `CompatibilityProfile` (§30) | `aep_domain::profile::Profile` — `crates/aep-domain/src/profile.rs:41`, plus the `profiles/` document tree | in this repository a **profile grants capabilities** (`AGENTS.md:104-105`: "only a profile or protocol may grant"). A second unrelated "profile" is a real ambiguity |
| `EvolutionObligation` (§46) | `Obligation` / `ObligationId` / `ResolvedObligation` — `crates/aep-domain/src/plan.rs:37`, `crates/aep-domain/src/principle.rs` | an obligation here is a **phase-timed duty derived from a principle**. The synthesis design adds `ImplementationObligation`; this adds a third |
| "revision" throughout (§5, §9, §34) | `Revision(String)` (`crates/aep-domain/src/artifact.rs:191`) — a source revision; `EntityRevision` — a monotonic counter for optimistic concurrency; `ArtifactVersion(String)` (`:157`) | three existing meanings before this one |
| `ChangeKind { Added, Removed, Modified }` (§11) | `ChangeSet` — `crates/aep-domain/src/evidence.rs:534` | already the evidence vocabulary for "what changed": files, lines, revision before/after, paths |

**None of these is fatal and all of them are free to fix now** — they are words in a design document. Left alone, they are what the wave 4 design already calls out at its §23: two answers to one question, in one repository.

### Identity: §6 against invariants 10 and 13

Checked, and **§6 does not violate either.**

- **Invariant 13** (`AGENTS.md:138-142`, "identity is opaque; an `EntityId` is never parsed for meaning") governs `aep_domain::entity::EntityId` (`crates/aep-domain/src/entity.rs:47`), whose whole doc comment says the representation is not interpreted. §6 is about `QualifiedName`, which is the opposite kind of thing by design: it has segments, a `local()`, a `namespace()`, and the naming model's entire point is that logical identity is readable and stable while wire and display names move (`crates/ess-domain/src/name.rs:9`, `:200-210`). Two identity models, correctly separated. §6 uses the right one.
- **Invariant 10** (`AGENTS.md:124-127`, "document identity comes from document content, not from filenames") is likewise satisfied in substance: §5's precondition 2 compares "the same logical system identity", which comes from `EssIr.system` (`crates/ess-compiler/src/ir.rs:910`), read out of `system.yaml`, not from the directory name — even though §58's CLI takes two directories.

Two places where the design drifts from invariant 10's *spirit* are covered in S11.

**One residual hazard, from §11:** `ChangeId` "deterministic from canonical change content". A content-derived id is fine — it is not being parsed. But if an `EssDelta` ever becomes an addressable AEP artifact (§34), it needs an `ArtifactId`, and the design should not use `EssRevisionRef { system, version, digest }` as one. That struct is a **key**, and the moment something looks up an artifact by reading `system` out of it, identity has become a key again — which is exactly the sentence invariant 13 is made of.

---

## S8 — §22's dependency graph already exists, partially, in the wrong place (medium)

`protocol ess graph` shipped in wave 2/3 and builds a semantic dependency graph today, in the CLI:

- `EssGraph` / `EssGraphNode` / `EssGraphEdge` — `crates/protocol-cli/src/main.rs:1605`, `:1614`, `:1624`
- `ess_graph_of(ir: &EssIr) -> EssGraph` — `:1663`

It has **2 node kinds** (command, event) and **2 edge kinds** (`emits` from outcome to event, `invokes` from event through a binding to a command). §22 wants roughly **13** edge kinds. So the existing graph is a proper subset, not a competitor — but it is a subset built in `protocol-cli`, private, with no library home.

The reason this matters is written in the IR's own doc comments. `EssIr::reactions` exists precisely so two consumers cannot disagree — "the IR defines this half of the graph, and a second reading of it here would disagree the first time an event grew a second binding" (`crates/protocol-cli/src/main.rs:1700-1701`). Building `SemanticDependencyGraph` in a new crate while `ess_graph_of` stays in the CLI recreates exactly the disagreement that comment exists to prevent, on a bigger surface.

**Recommendation for SD-b:** `SemanticDependencyGraph` lands in `ess-compiler` beside `reactions`/`projections`/`drivers`/`grants` (`EssIr::reactions`, `::projections`, `::drivers`, `::grants` in `crates/ess-compiler/src/ir.rs`), which are already four hand-rolled projections of the same graph, and `ess_graph_of` becomes a rendering of it. That is a net deletion in the CLI and it removes a fifth. **Cost: within SD-b, nothing extra.**

---

## S9 — Taxonomy: two kinds have no field, one is misattributed, eight constructs have no kind (medium)

Checked every change kind in §12–§20 against `crates/ess-compiler/src/ir.rs` and `crates/ess-domain/`. Most land. These do not.

### Names a field the model does not have

| § | change kind | why not |
|---|---|---|
| §19 | `PortChanged` | `ResolvedComponent` is `{ name, owns, accepts, publishes, naming }` (`ir.rs:820-830`). There are no ports |
| §19 | `TransportRequirementChanged` | `ResolvedWorkload` is `{ component, replicas, stateless, requires }` (`ir.rs:840-848`). There is no transport, deliberately: `crates/ess-domain/src/lib.rs:21` — "Semantic concepts are primary; transports are projections", and wave 4's §41 is titled *No Hidden Transport Assumptions* |

### Attached to the wrong construct

| § | change kind | where it actually lives |
|---|---|---|
| §13 | `TransitionTriggerChanged`, listed under *entity* changes | `Transition` is `{ name, from, to }` (`crates/ess-domain/src/entity.rs:160-167`) — no trigger. Since **G14 closed this afternoon**, the trigger is on the command side: `ResolvedOutcome::subject` → `ResolvedSubject { entity, effect }` → `ResolvedEffect::{Creates, Moves{transition}, Updates}` (`ir.rs:538-548`, `:498-512`). It is a **command** change, not an entity change |

### Real constructs with no change kind

Each of these changes what the system means, and several change what wave 4 generates:

| construct | where | consequence of a change |
|---|---|---|
| `ResolvedOutcome::subject` | `ir.rs:565` | **new since G14.** Which entity an outcome acts on, and whether it creates / moves / updates. A change here changes the lifecycle and invariant scenarios |
| `ResolvedOutcome::test_strategy` | `ir.rs:570` | `ConstructInput` / `DefaultBranch` / `InjectFault` (`crates/ess-domain/src/command.rs:258-265`). **Directly decides which scenario wave 4 generates.** A change here is a suite change with no visible contract change |
| `ResolvedView::assertion_style` | `ir.rs:691` | `expect` vs `eventually` (`crates/ess-domain/src/view.rs:113-118`). Same — decides the generated assertion |
| `ResolvedBinding::escalation` | `ir.rs:813` | the event published on escalation. This is gate **G2**'s construct; it will be load-bearing before diff exists |
| `StateMachine::terminal` | `crates/ess-domain/src/entity.rs:224` | declared, not inferred. Adding a terminal state changes what "stuck" means |
| `ResolvedEntity::state_type` | `ir.rs:367` | the enum the state is projected as |
| `ResolvedBody::{Newtype,Struct}::invariants` | `ir.rs:301-315` | §13 covers **entity** invariants; **type-level** invariants have no change kind |
| `Naming::summary` | `crates/ess-domain/src/name.rs:207-209` | §12 has `WireNameChanged` and `DisplayNameChanged` and no `SummaryChanged`. It is documentation-only, so it belongs in §8's "presentation-only" bucket — but it must be *named* there, or it silently becomes a `TypeBodyChanged` |

**The first three are the sharpest instance of the header's optimism.** They landed today (G14) or in wave 3, after the design was written, and they are the fields that decide the suite. A diff engine built to this taxonomy would report **no change** for a specification edit that rewrites half the conformance suite.

### Two more, smaller

- **Conversions have no identity.** `EssIr.conversions` is a `Vec<ResolvedConversion>` (`ir.rs:923`), and `ResolvedConversion` is `{ from, to, because }` (`:456-462`) — no name. §20's `ConversionAdded/Removed/ConversionContractChanged` needs a synthetic key, presumably `(from, to)`. Worth saying, because it is also the **one** collection in the IR where §7's "file splitting should not become a semantic change" is false: conversions keep declaration order (`crates/ess-compiler/src/resolve.rs:1053-1054`), so moving one between files reorders the `Vec`. Everything else is a `BTreeMap` keyed by name and genuinely immune.
- **§26 counts `public_contracts_changed`.** Nothing in the model marks a construct public or internal. `ResolvedComponent::publishes` is the nearest thing. Either derive it from that and say so, or drop the fact.

---

## S10 — §33 assumes per-element artifact provenance; today the digest is per-model (medium)

§33: "Once structural synthesis emits a manifest relating semantic elements to generated artifacts … `SemanticChange → generated artifact provenance → regenerate`."

What exists: `Generator::generate(&self, ir: &EssIr, provenance: &Provenance) -> Vec<Artifact>` (`crates/ess-gen/src/artifact.rs:49`), where `Artifact` is `{ path, contents }` (`:11`) and `Provenance` is **one value for the whole run**, derived from the whole IR:

```rust
// crates/ess-gen/src/provenance.rs:116-127
let json = serde_json::to_vec(ir).unwrap_or_default();
let hash = Sha256::digest(&json);
```

So today, changing one field of one event changes `source_digest`, which appears in the header of **all 27 committed artifacts** under `generated/`. §33's selective regeneration has nothing to be selective with. This is not a defect — it is correct for what the digest is for — but it is a precondition the design does not name.

Two related notes:

- The digest is **truncated to 16 hex characters** and its own doc says why: "this is for telling two models apart in a comment header, not for resisting an adversary" (`crates/ess-gen/src/provenance.rs:110-115`). §62 is a section about trust boundaries. If a policy gate ever refuses a change based on a digest comparison, 64 bits of a design-time convenience hash is doing security work it was explicitly not built for. `SpecDigest` already accepts up to 64 characters (`crates/aep-domain/src/evidence.rs:702-703`), so nothing forecloses — but which length wave 4 writes should be a recorded choice, not a default inherited from a comment header.
- `Provenance::VERSION` is `env!("CARGO_PKG_VERSION")` of **`ess-gen`**, used for both `compiler_version` and `generator_version` — "One number while the two ship together" (`:31-35`). §61 wants compiler version, diff format version, diff engine version and impact analysis version as four distinct things. That is wave 4's **open decision D4** arriving early, and it argues for settling D4 as its default (two fields) rather than deferring.

---

## S11 — `Version` is major-only, so `billing/v3` cannot name a revision (medium)

```rust
// crates/ess-domain/src/name.rs:229-235
/// Only the major part exists on purpose: a minor version that consumers are expected to ignore is
/// not something the model should carry, and one they are not expected to ignore is a major version.
pub struct Version(u32);
```

Consequences for the design:

- **§9 `EssRevisionRef { system, version, digest }`** — for the ordinary case (two edits to a system that has not had a breaking change), `before.version == after.version`. The `version` field is a label; the **digest is the identity**. That is fine, and it is what `EssConformanceResult` already says in almost these words (`crates/aep-domain/src/evidence.rs:806-811`). The design should say it too, so nobody builds an ordering on `version`.
- **§35 `proposal: { base: billing/v3 }`** — not resolvable. Two different resolutions share that label. The proposal's `base` must be a digest, or a digest plus a label for humans.
- **§41 and §66** use `billing v3 → billing v4` as the illustrative revision pair, which reads as though the version distinguishes them. It will not for most real changes.

**This is invariant 10's spirit applied one level up**: identity comes from content. The repository has already had this argument once and won it; the design should inherit the answer rather than re-open it.

---

## S12 — `FreshnessPolicy::BoundToDependencySet` already declares §33's policy, and nothing enforces it (medium, good news)

```rust
// crates/aep-domain/src/artifact.rs:1178-1185
pub enum FreshnessPolicy {
    AlwaysValid,
    UntilSuperseded,      // default
    BoundToRevision,
    BoundToDependencySet, // "Valid only while its dependencies are unchanged."
}
```

`BoundToRevision` is enforced — `ReviewRequirement`'s `covers` reads it (`crates/aep-domain/src/review.rs:254`). `BoundToDependencySet` is enforced by **nothing**:

```console
$ grep -rn "BoundToDependencySet" crates/ docs/
crates/aep-domain/src/artifact.rs:1184
docs/design/archive/artifact-model-extension-v0.1.md:1554
docs/design/consolidated-design-v0.2.md:3988
```

One declaration and two design documents. No reader.

This is the best news in the review, and the design misses it. §33 is not a new concept needing a new vocabulary — **it is the missing mechanism for a policy the normative design already specified and the model already carries.** Track VI should be framed as "make `BoundToDependencySet` mean something", which is a smaller and much more defensible piece of work than "introduce obligation invalidation".

It also settles part of S7: the verdict vocabulary in §33 ("still valid / must be reimplemented / must be reverified / no longer required / newly required") should be expressed as freshness outcomes over the existing policy, not as a parallel enum.

---

## S13 — Two plan pages disagree on when diff starts (low, but control-plane)

| document | says |
|---|---|
| `docs/plan/ess-wave-3.5-reconciliation.md` § *Decisions taken*, decision 2 | "**ESS wave 4, then semantic diff** — the semantic-diff design says it can start as soon as `EssIr` is stable, which is now — but the oracle is what makes every other claim checkable, so it goes first" |
| `docs/plan/ess-roadmap.md:280-284` at `34cac07`; `:282` in the working tree, which is being edited | "What is not in these five waves: … **`ess diff` compatibility classification** … None is worth starting before generated code has compiled and passed a suite it did not write" — which is **wave 5's** acceptance, not wave 4's |

`AGENTS.md:23-24` makes plan pages the thing that accepts a design, so a contradiction between two of them is control-plane drift, not documentation debt. One line in `ess-roadmap.md` closes it. Note also that the roadmap's phrasing scopes only "`ess diff` **compatibility classification**" — §29–§31, i.e. SD-d — which is arguably compatible with decision 2 if SD-a/SD-b come first. Say which was meant.

---

## S14 — Two small preconditions that are false (low)

- **§5 precondition 3**: "The diff implementation understands both IR format versions." `EssIr` has **no format version field** — it is `{ system, version, naming, summary, domains, types, conversions, entities, commands, events, errors, views, actors, bindings, components, workloads }` (the `EssIr` struct in `crates/ess-compiler/src/ir.rs`). `version` is the *specification's* version, not the IR's. §63's `DiffRefusal::UnsupportedIrVersion` has nothing to read. Either add the field in the diff wave or delete the precondition and the refusal; do not leave a refusal that cannot fire — that is the defect class the guard-efficacy review spent a day on (`docs/reviews/2026-08-20-guard-efficacy-review.md`).
- **Invariant 2 applies to `EssDelta`.** `EssIr` derives `Serialize` only — `#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]`, no `Deserialize` — correctly: `AGENTS.md:82-84` forbids `Deserialize` on a validated type. A committed, machine-readable `ess.delta/v1` (§58) that anything reads back therefore needs a `RawEssDelta` → `EssDelta` `TryFrom` pair, and it will be picked up by the source scan in `crates/aep-domain/tests/invariants.rs` that G17 added. Two lines in the design; a surprise otherwise.

---

## What this design gets right

Said plainly, because most of it is right and a findings list buries that.

1. **Diff the compiled IR, not the source and not the projections** (§4). Correct, and the IR is already built for it: name-keyed `BTreeMap`s everywhere, `PartialEq` on every `Resolved*`, canonical JSON with a byte-comparison test (`compiling_the_billing_example_twice_produces_byte_identical_json`, `crates/ess-compiler/tests/billing.rs:295`). "Identical semantic IR produces an empty delta" (§67) is `before == after` and costs nothing.
2. **No heuristic rename inference** (§6, §41). This is the single most important call in the document, it is right, and it is already the model's position — `crates/ess-domain/src/name.rs:9` separates logical identity from wire name precisely because "a transport is reshaped" and "every consumer already deployed" are different consequences.
3. **A typed change model rather than a JSON patch** (§10), with the argument for it: a new ESS construct should *force* the diff engine to decide how it compares. That is the same argument as the `validation_codes!` macro and the empty gap allowlist in wave 3 — a list that cannot silently fall behind.
4. **`SemanticRelation::Unknown` as a first-class answer** (§21, §63), refusing to call a predicate change "stronger" without a proof. This is invariant 5 (`Unknown` is not `False`) applied one level up, and the design reaches it independently.
5. **Refusing a universal risk score** (§27), and splitting deterministic facts from versioned organisational policy from an explicitly advisory estimator. The three-layer split is right and the middle layer is the shape of `CapabilityPolicy` — a rule that names what decided.
6. **Explainable impact paths** (§24), refusing to emit `email-component risk = high` without the path. Same standard as `CapabilityDecision` naming the rule that decided it.
7. **Determinism** (§59, §60): no clock, no RNG, no LLM in the producer, canonical serialisation, trailing newline, stable category ordering as a format contract. Byte-for-byte identical to what the compiler and `ess-gen` already promise and test.
8. **Falsifiability** (§71) with named mutations — "classify a removed actor grant as expansion → relation test fails". That is `AGENTS.md:188-191` verbatim, arrived at independently.
9. **A useful accident:** §30's `CompatibilityProfile` assumptions ("JSON clients ignore unknown response fields", "command inputs reject unknown fields") are **partly derivable rather than declared**. Wave 3 unified one type mapping across the three projections and its tag records why: the AsyncAPI documents had been the permissive side, accepting unknown extra fields the JSON Schema tree refused. Those `additionalProperties` decisions are in `generated/` today. A profile that reads them instead of asserting them is smaller and cannot drift.

---

## Decisions for Timo

| # | decision | options | cost | **default if nobody answers** |
|---|---|---|---|---|
| **D-a** | Does track VI wait for G19? | (a) yes, VI cannot start until G19 closes · (b) build VI first and bind later | (a) none — G19 is already scheduled · (b) a selective-invalidation engine layered on nothing | **(a).** Add G19 to the design's §5 preconditions |
| **D-b** | Scenario id: derived from semantic refs, or the §37 counter? | (a) derived, counter is runtime-only · (b) counter, accept re-keying later | (a) one sentence + one test in wave 4 · (b) re-key the committed suite, the fault matrix and every report | **(a).** Amend wave 4 §37 and add the acceptance line to §50 |
| **D-c** | Is `ConformanceScenario.source` provenance, or the dependency set? | (a) dependency set — every construct whose change changes the scenario · (b) originating construct only | (a) ~free while the walk is being written · (b) regenerate the suite later, or accept an unsound intersection | **(a)** |
| **D-d** | §36 and §38 — in or out? | (a) cut both; keep §37 and §59 · (b) keep, and amend `docs/VISION.md:153` deliberately · (c) move to a separate harness document | (a) two sections deleted · (b) a vision change with a stated reason | **(a).** §37 + §59 already give an external harness everything it needs |
| **D-e** | Track EV (§43–§52) — sequence, or park? | (a) park until `Realization` is accepted work · (b) sequence now | (a) none · (b) a wave built on an unsequenced design | **(a).** It is transitively blocked on `contract_digest`, which exists in no code |
| **D-f** | Accept SD as a vision amendment? | (a) amend `docs/VISION.md`'s thesis to include the system changing over time · (b) leave it in *Proposed, not accepted* until SD-a lands | (a) a paragraph, deliberately · (b) a wave whose home document says it is not part of the vision | **(b)** — amend when SD-a lands and has proved itself, not before. `docs/VISION.md:146-149` asks for this to be deliberate |
| **D-g** | Where does the diff engine live? | (a) `ess-compiler` (it holds the graph projections already) · (b) a new `ess-diff` crate · (c) `ess-gen` | (b) is one manifest and thirteen lint lines | **(b).** Same reasoning as wave 4's D1: `ess-compiler` documents itself as clock-free and reads only its own sources; a diff engine that later takes a policy document is fallible in a way the compiler's contract is not. `ess-compiler` keeps the *graph* (S8); the diff keeps the delta |
| **D-h** | Which digest length does wave 4 write? | (a) 16 (what `ess-gen` writes today) · (b) full 64 | either is accepted by `SpecDigest` | **(a)**, recorded as a choice rather than inherited, with the note that a policy gate would want (b) |

---

## Proposed smallest first slice

**SD-a: the delta, and one fixture pair that bites.** After wave 4 lands, before anything else in this document.

### What ships

1. **`EssDelta`** over `(before: &EssIr, after: &EssIr)`, covering the six construct families whose IR coverage is complete and whose comparators need no unknowns: **system, types, events, errors, actors, components**. Deliberately excluded from the first slice: entities and commands (their invariant and condition predicates are where `SemanticRelation::Unknown` lives, §21), views, bindings, topology, conversions.
2. **Four mechanically-derivable relations only**, per §21's own list: grant added → `Expanded`, grant removed → `Narrowed`, enum/union variant added → `Expanded`, removed → `Narrowed`. Every other relation is `Changed`. **No `Unknown` yet**, because there is no predicate comparison in this slice — which is the point of choosing these six families.
3. **`DiffRefusal::DifferentSystem`**, comparing `EssIr.system`. Nothing else refuses. §5's precondition 3 is dropped until there is an IR format version to check (S14).
4. **`protocol ess diff --from <dir> --to <dir> --format text|json`**, joining the five existing `ess` verbs (`validate`, `compile`, `inspect`, `generate`, `graph` — `crates/protocol-cli/src/main.rs:346-362`). No collision.
5. **Canonical output**: the §60 category order as a format contract, `ChangeId` derived from canonical change content, trailing newline, and the byte-identical test generalised from `compiling_the_billing_example_twice_produces_byte_identical_json`, `crates/ess-compiler/tests/billing.rs:295`.
6. **A `RawEssDelta` → `EssDelta` pair**, because the JSON form is read back (S14, invariant 2).

### The demo that bites

One committed revision pair beside the billing example, small enough to audit by hand, containing exactly four changes — two that make a text diff lie in each direction:

| change | what `git diff` shows | what `EssDelta` shows |
|---|---|---|
| move one domain's events into a second file, reflow comments | ~200 lines | **empty delta** |
| `InvoiceCreated` wire name `invoices.created.v1` → `.v2` | one line | `EventWireNameChanged` — same logical identity |
| `InvoiceCreated` → `InvoiceIssued` | one line, adjacent to the above | `EventRemoved` + `EventAdded`, **no rename inferred** |
| add `billing.support` may `RefundInvoice` | three lines | `GrantAdded`, relation `Expanded` |

Rows 2 and 3 are the whole argument for the feature, side by side, in a diff a person can read. Row 1 is the claim in §7 made checkable.

**Note the one exception to row 1** (S9): if the fixture moves a `conversions:` entry between files, the delta will not be empty, because conversions keep declaration order (`crates/ess-compiler/src/resolve.rs:1053-1054`). Either keep conversions in one file in the fixture, or key the conversion comparator on `(from, to)` in this slice and say so.

### Falsifiability, before it is trusted

§71's first four mutations, each applied, watched fail with a message that names the defect, and reverted:

```text
ignore an event-field type change            -> the delta test fails
classify a removed actor grant as Expanded   -> the relation test fails
merge remove+add into a rename               -> the identity test fails
emit changes in hash order                   -> the byte comparison fails
```

### The handle discipline, from line one

A source scan in the diff crate's `tests/`, on the model of `no_source_file_in_the_compiler_reads_a_clock_or_an_unordered_map` (`crates/ess-compiler/tests/billing.rs:315`), asserting that no source file in the diff crate calls an `EssIr` handle accessor. Resolution goes through `handle.name()` and the target IR's map (S5). It costs an hour now and it is the difference between a mistake and a panic.

### Size and shape

| | estimate |
|---|---|
| `crates/` insertions | 4–6k |
| tests | 60–90 |
| new workspace members | 1 (`ess-diff`) |
| new third-party dependencies | **0** |
| gate steps touched | `generate-check` only if the fixture is committed under `generated/` |

Roughly **half a wave** by this repository's measured rate. That is deliberate: it is the smallest thing that produces the argument, and everything above SD-a — the graph, impact closure, policy, compatibility, evidence invalidation — is easier to design once one delta exists to look at.

### What SD-a explicitly does not do

No dependency graph. No impact closure. No `ChangePolicy`, no `CompatibilityProfile`, no proposal constraints. No scenario or evidence invalidation. No LLM anything. No `EvolutionPlan`. Each is named here so that its absence reads as a decision rather than an omission.

---

## Method note

**Verified** against the working tree at `34cac07`: every `file:line` above; the wave insertion counts (`git diff --shortstat` between tags); the test counts (`git tag -n99`); the absence of `contract_digest`, `EssSemanticRef` and `Realization` from `crates/`; the absence of any reader of `BoundToDependencySet`; the absence of ports and transports from the ESS model.

**Inferred, and labelled as such in place:** that §52's transition oracle needs a live target; the slice size estimate for SD-a, which is extrapolated from the three measured waves and not from a prototype.

**Held by another agent during this review**, so line numbers there drift and were re-checked at the end: `crates/ess-domain/`, `crates/ess-compiler/`, `crates/ess-gen/`, `crates/aep-domain/`, `examples/billing/`, `generated/`, `docs/plan/ess-roadmap.md` (+61 lines vs `34cac07`) and `docs/design/ess-closed-loop-execution-conformance-design-v0.1.md` (+579/-104 lines vs `34cac07`).

**Re-verified against the working tree after those edits**, because S2 and S13 turn on them: the §37 monotonic-counter line still names scenario ids, §50 still says only "stable across unchanged input", and `ess-roadmap.md` still lists `ess diff` under what is not in the five waves. **Both findings stand.**

**Not checked:** whether the in-flight edits to those crates change any *behaviour* this review relies on. Every code citation was read from the working tree, not from `34cac07`.
