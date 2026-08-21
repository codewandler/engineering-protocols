# Working agreement

For humans and agents working in this repository. Read this before changing anything.

## What this repository is

A machine-executable specification of engineering methodology: principles, workflows, capabilities,
evidence and verification, expressed as typed Rust and generated JSON Schema rather than as prose in
a prompt. It is a **library and specification**, not an agent, a CI system or a deployment platform.

## Which documents are normative

Exactly two, and neither is the newest file in `docs/design/`:

* [`docs/design/consolidated-design-v0.2.md`](docs/design/consolidated-design-v0.2.md) — the
  specification for the protocol.
* [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) — what is implemented,
  and §5, the register of deliberate deviations.

When code and the consolidated design disagree, the document wins unless the disagreement is recorded
in the reconciliation register §5. Add to that list rather than diverging silently.

**Everything else in `docs/design/` is proposed until a plan page in `docs/plan/` — or a story in
`.engineering/planning/` — accepts it.** A proposal is not a work order, however long and however
recent it is. `ess-implementor-design-v0.1.md` and `ess-review-v0.1.md` show what acceptance looks
like: [`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md) and the wave 1–3 plan pages took them
up, and waves 1 to 3 shipped from them.

The store is the second acceptance surface, and it is new: `story:evidence-horizons` took up
`evidence-horizons-design-v0.1.md` and shipped it without a plan page ever existing, which is what
the store is for — a triaged item is a story with a status, not a page somebody has to remember to
write. A design accepted that way is accepted; check both surfaces before calling one proposed.

Eight further proposals now sit in `docs/design/`. Their acceptance state is:

| proposed design | status |
|---|---|
| [`ess-closed-loop-execution-conformance-design-v0.1.md`](docs/design/ess-closed-loop-execution-conformance-design-v0.1.md) | **implemented** as ESS wave 4 (`0.4.0-ess-wave-4`); its four open decisions D1–D4 were taken at their stated defaults |
| [`ess-semantic-diff-impact-evolution-design-v0.1.md`](docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md) | **core implemented** as ESS wave 5 (`0.5.0-ess-wave-5`). Two of its seventy-eight sections are rejected outright (the proposal-evaluation loop and architecture search); the rest past §31 stays proposed |
| [`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md) | **accepted in part** by [`docs/plan/ess-wave-6-structural-synthesis.md`](docs/plan/ess-wave-6-structural-synthesis.md), which is wave 6, in progress. Its obligation/`Realization` programme stays proposed (W7.4 takes a slice), and its §28 is refused by invariant 6 |
| [`semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md`](docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md) | reviewed and **deferred whole**; two ideas harvested |
| [`harness-planning-and-driver-design-v0.1.md`](docs/design/harness-planning-and-driver-design-v0.1.md) | **Phase 1 accepted** by [`docs/plan/harness-wave-1-planning-plugin.md`](docs/plan/harness-wave-1-planning-plugin.md), which is harness wave 1: the markdown planning store, `protocol artifact`, and the Claude Code plugin. Its Phase 2 **reference driver** is decided by the operator (`docs/VISION.md` § *What this is deliberately not*, narrowed 2026-08-21), reviewed against the code by harness wave 2, and **built as harness wave 3** — `aep-driver-spec`, `aep-driver`, `drivers/`, `protocol drive`, the plugin's enforcement hooks and a second harness with no model in it. Both halves are recorded in [`docs/plan/harness-wave-2-driver-decision.md`](docs/plan/harness-wave-2-driver-decision.md), which carries wave 2's decisions and wave 3's acceptance. Harness wave 4 — [`docs/plan/harness-wave-4-governed-dogfood.md`](docs/plan/harness-wave-4-governed-dogfood.md) — stays **proposed**, and its W4.1 has been run once: `W4-1/1`, 2026-08-21, **blocked in `establish_verifiers`** for two reasons the engine printed. The run is the finding, and it is on that page rather than repaired into a pass |
| [`evidence-horizons-design-v0.1.md`](docs/design/evidence-horizons-design-v0.1.md) | **implemented**, 2026-08-21, accepted by `story:evidence-horizons` in `.engineering/planning/` rather than by a plan page. Its own header still reads *proposed* and is stale; the gap register's *Closed by code — evidence horizons* section is the record. Follow-ups it left are open rows there: F26, and decisions D-6 and D-7 |
| [`story-completion-evidence-design-v0.1.md`](docs/design/story-completion-evidence-design-v0.1.md) | **proposed, not accepted.** Proposed by [`docs/plan/harness-wave-4-governed-dogfood.md`](docs/plan/harness-wave-4-governed-dogfood.md) § W4.3, whose acceptance criterion is a verdict on it — accepted, accepted in part or refused — and not a build. Both shapes it could take are domain changes |
| [`transcript-conformance-design-v0.1.md`](docs/design/transcript-conformance-design-v0.1.md) | **accepted, in implementation** as trace wave 1 by [`docs/plan/trace-wave-1-transcript-checker.md`](docs/plan/trace-wave-1-transcript-checker.md), which takes up its milestones T1–T3, sequences them and sets their acceptance criteria. Its open decisions D1–D6 are taken at their stated defaults, with one narrowing: the `regex` matcher of § 3.4 is **refused by name** rather than implemented, because the workspace carries no regular-expression engine. What stays proposed is named on the plan page: assertions over the per-request usage *series* (§ 2.7), an expectation kind for the skill's own text entering context (§ 2.8), and a streaming checker (**D5**) |

Do not implement from an unreviewed design, and do not treat one as evidence of what this repository
is. [`docs/VISION.md`](docs/VISION.md) § *Proposed, not accepted* says what each would add, and
[`docs/plan/gap-register.md`](docs/plan/gap-register.md) holds every open gap with what closes it.

## Current state

The status report is [`docs/status.md`](docs/status.md); keep it accurate when you land work. Its
delivered-waves table is generated from the annotated tags (`cargo xtask status`) and drift-checked
in the gate, and prose here states **no count of the gate's own suites or tests** — four hand-written
ones drifted apart in the repository's first 48 hours, so that number lives in exactly one place, the
gate's output. Counts of things a command prints on demand — documents in the tree, artifacts in the
store, records in a corpus — are written down with the command that produces them, so a reader can
re-run it. `git tag -n99` is the per-wave record of what actually shipped, and `task check` is the
measurement; read those before believing any prose about progress.

**Every crate in the workspace is implemented and gated. There are no skeletons left.** Twenty-nine
workspace members: **twenty-six crates** under `crates/`, two realization examples and `xtask`. The
latest wave is `0.10.0-horizons-dogfood-lab`; `git tag -n99` says what each one delivered.
`protocol validate` reads the document tree and reports what it holds — at that tag, **44 files:
3 protocols, 22 principles, 4 workflows, 6 profiles, 8 lifecycles and 1 step map** — so run it
rather than copying that line forward. The gate
(`task check`, ten steps) needs three toolchains beside
Rust's own: the **Go toolchain**, the **`wasm32-unknown-unknown` target** and **Node**. Two of the
ten steps build the second and third emitters' committed trees, and none of those checks skips
when its toolchain is absent — it fails and names it, because a skipped check reads exactly like a
passing one.

* **AEP — the protocol; the v0.2 scope is implemented.** `aep-domain`, `aep-schema`, `aep-engine`,
  `aep-contract`, `aep-backend-memory`, `aep-conformance`, `adp-domain`, `aop-domain`,
  `protocol-cli` and `xtask`, plus the document tree (`protocols/`, `principles/`, `workflows/`,
  `profiles/`, `artifacts/lifecycles/`). `aep-conformance`, `adp-domain` and `aop-domain` all
  shipped in `0.2.0-wave-3`: sixteen black-box suites over the command and query surfaces, three
  conformance levels, and a `FaultyBackend` whose injected defects the suites are checked against,
  beside the development and operations typed vocabularies. `0.2.1` added project discovery.
  **Evidence carries a clock as of `0.10.0`:** a record states `observed_at`, a requirement may
  declare a `horizon`, and past it the fact reads `Unknown` — see invariants 5, 7 and 17.
* **ESS — executable system specifications.** Six delivered, gated crates: `ess-domain` (the typed
  model, `0.3.0-ess-wave-1`), `ess-compiler` (resolution, `EssIr`, source-aware diagnostics,
  `0.3.1-ess-wave-2`), `ess-gen` (four projections behind one `Generator` trait,
  `0.3.2-ess-wave-3`), `ess-conformance` (the specification as oracle: synthesis, runner, evidence,
  `0.4.0-ess-wave-4`), `ess-diff` (semantic delta and impact closure, `0.5.0-ess-wave-5`) and
  `ess-synth` (language-neutral synthesis plan, **three** emitters behind one seam: wave 6's Rust
  workspace — whose linkage with the hand-written realization in `examples/billing-realization`
  passes the committed billing suite unchanged, and fails the deliberately corrupted linkage at
  the one scenario that exists to catch it — wave 7's Go module, W7.3, which is the test of
  the neutrality claim, and wave 7's browser realization, W7.3b, which is the harder test of it
  because it is not a language at all: a `WebAssembly` bridge over the Rust target's system, JSON
  over linear memory with three exports and no build tool, beside a page whose command forms,
  event log, view tables and lifecycles are built at load time from an emitted `catalog.json` —
  nothing about any system is typed into its HTML. The plan's two renderings are byte-identical in
  all three trees, and what a target holds more weakly or cannot represent at all is stated in a
  `TARGET.md` beside the plan, never folded into it). W7.5 is the demonstration those three
  emitters existed for: **one specification, two running applications, one surface** —
  `examples/gatepass/` synthesised to Rust and to Go, both serving the routes the committed
  `OpenAPI` document declares plus `/openapi.json` and `/docs`, both writing the same startup
  record outside a declared `runtime` member, and the gate starting both on ephemeral ports to
  compare records, statuses, bodies and published bytes. Its transport is **derived**, as wave 6
  requires: a component may say `reached_by: network`, which states where its callers are and
  names no protocol, and HTTP follows because the one contract this repository projects for a
  command surface is an `OpenAPI` document. `generated/` holds the committed projections and all
  three synthesised trees, `suites/generated/` the committed conformance suites; all drift-checked
  in the gate.
* **Infra — observed infrastructure as a second instance of the ESS pattern.** Five crates:
  `infra-domain` (the k8s observation subset, raw→validated, eleven `INFRA-*` refusal codes,
  secrets only ever as digests — IW1), `infra-compiler` (the content-addressed `infra-ir/1`
  with unresolved references as typed facts, plus the validating read-back of a persisted
  document — IW1/IW2), `infra-analyze` (the typed dependency graph with exact pod ownership,
  twenty `INFRA-DIAG-*` diagnosis rules with registered severities, workload properties,
  invariant candidates and directions — IW2) and `infra-spec` (the authored desired state:
  twelve expectation kinds evaluated three-valued against a snapshot, where a False without a
  gap or an Unknown without a reason is unrepresentable — IW3) and `infra-project` (a gap
  becomes a reviewable patch tree where a patch is mechanically safe, an obligation where a
  value is a human's to choose, and a refusal where the gap is not a field — with the
  round-trip asserted: applying the tree closes what it claims and moves nothing else — IW4).
  `protocol infra validate|compile|inspect|graph|diagnose|view|simulate|diff|project` is the
  surface;
  the scanner (`infra-scout`) is a separate repository holding the credentials, and nothing
  here reaches a network. Plan pages: `docs/plan/infra-wave-1-observe.md`,
  `docs/plan/infra-wave-2-analyze.md`.
* **Trace — an agent run as a third observation domain.** Two crates: `trace-domain` (the
  harness-neutral event IR `trace-ir/1`, content-addressed by a digest over the transcript's raw
  bytes, with an event the adapter cannot read kept opaque rather than dropped; and the
  `trace-spec/1` expectation vocabulary, **fifty kinds** — `env.tool_available` was the fiftieth,
  built first in harness wave 3 because the hooks lean on it — raw→validated with ten `TRACE-*`
  refusal codes) and `trace-spec` (the Claude Code `stream-json` adapter and the checker, whose
  `ok`/`gap`/`unk` verdicts each cite the event indices that produced them). `protocol trace
  check|inspect|evidence` is the surface, with `ess conform`'s exit codes — `0` conformant, `1`
  contradicted, `3` nobody found out. `evidence` runs the check and writes the AEP record it
  produced, so a verdict about a run enters the engine as a fact rather than as a claim. Nothing
  here calls a model or reads a clock: every duration and every cost comes out of the transcript,
  which is what lets a report be committed and diffed. Plan page:
  `docs/plan/trace-wave-1-transcript-checker.md`.
* **Harness — the planning store, and the reference driver that walks a workflow.** Wave 1 built
  `aep-backend-markdown` and `protocol artifact`; wave 3 built the driver. Three crates:
  `aep-driver-spec` (the leaf — step maps raw→validated, the mandatory workflow pin, the run cursor,
  `ToolConfig`), `aep-driver` (the deterministic three-valued router, the executor traits and
  `tool_config`, both clock-free and randomness-free under invariant 9) and `aep-render` (a workflow
  and a run over it as SVG, HTML, PNG or a terminal frame, byte-stable, depending on `aep-domain`
  alone so a renderer cannot become a second protocol implementation). `protocol drive
  run|status|resume` and `protocol workflow render` are the surfaces; step maps are the fifth
  document kind, under `drivers/`, and `development.driven` is the sixth profile — the only one that
  grants a shell, held to the `protocol` CLI by the plugin's own hook. **Gates are evaluated only by
  the engine**: the driver asks and does what it is told, and enforcement sits at two layers — the
  per-state tool set at session launch and two `PreToolUse` hooks, `store-integrity` and
  `driven-surface`, both shipped in `integrations/claude-code/hooks/` — with `protocol trace check`
  reading the transcript afterwards to say whether it held. Record:
  `docs/plan/harness-wave-2-driver-decision.md`; design
  `docs/design/harness-planning-and-driver-design-v0.1.md` §§ 4.1–4.9.
  The store this repository runs on is `.engineering/planning/`: **59 artifacts** — one initiative,
  seven epics, forty stories, ten tasks and one specification — and `protocol artifact validate`
  exits 0 on it. **It has been driven once.** `W4-1/1`, 2026-08-21, walked a real story from that
  store under `development.driven` and **blocked in `establish_verifiers`**, because the
  specification it wrote was still `draft` and the suite it ran passed where the rule wants a
  failing one. Four sessions, 80 hook decisions of which 11 were denials, no tracked file touched.
  The record is `docs/plan/harness-wave-4-governed-dogfood.md` § W4.1, and it stands as run: a
  dogfood wave that reports only its successes measures nothing. *Built* is not *adopted* — one
  story driven once says the mechanism holds on real work, and does not say driven runs are how
  work happens here.
* **Evidence horizons — a fact knows when somebody looked.** An evidence record carries a required
  `observed_at`; a requirement may carry a `horizon` in whole days; past it the fact decays to
  `Unknown` and never to `False`, and the lapsed record's facts are withheld, so a guard reading
  them refuses too. `evidence.lapsed` sits beside `evidence.missing` because *nobody produced it*
  and *somebody did and nobody has looked since* want different responses. `protocol evidence
  scan|inspect` is the observation half — `scan` reads the annotation convention out of
  human-written markdown and reports coverage beside the classification, `inspect` reads an
  evidence file. Neither writes anything and neither decides a gate. Ground truth is
  `examples/evidence-horizons-corpus/`, vendored from an outside adopter: **43 occurrences, 43
  records, 0 unparsed** at its reference date. Design:
  `docs/design/evidence-horizons-design-v0.1.md`.
* **Adopted by one tree that is not ours.** On 2026-08-21 somebody who did not write this
  specification wrote a document tree against it — a protocol extending `aep/1`, four workflows,
  six principles, four profiles, four lifecycles, 26 files — and it validates: `resolve`, `explain`
  and `evaluate` all work on it. Their ranked-first finding, evidence horizons, is closed by code.
  The rest of their review is triaged as `epic:adopter-feedback-round-1` in the store and as the
  gap register's *first adopter's report* section. The review itself is held by the operator and is
  **not in this tree** — nothing adopter-internal is written into a file here.
* **Not built yet:** W7.4 — obligations as artifacts a task can own — deferred by decision;
  attested evidence (gap register D-3, proposed and unaccepted); a **contract implementation that
  survives a process exit**. That last one is now half true and worth stating as two facts rather
  than one: a durable markdown **store** exists (`aep-backend-markdown`, harness wave 1) and holds
  planning artifacts as files, but it is a store and not a `CommandService`/`QueryService`
  implementation — it writes through its own two functions, which is deviation D-P1 against
  invariant 14 — so the **contract** still has exactly one implementor, `aep-backend-memory`, and the
  sixteen conformance suites run against that and nothing else. The journal-backed milestone (P3)
  is what makes the store answer as a backend; until then, "there is a durable backend" is a claim
  the suites do not support.
* Work order: [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) §4 for AEP,
  [`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md) for ESS, and
  [`docs/plan/gap-register.md`](docs/plan/gap-register.md) for what is owed outside any wave.

## Invariants

These hold across the workspace. Breaking one is a design change, not a refactor.

Each carries what actually enforces it — a lint, a type, a test or a scan — because a rule nothing
checks is a rule that has already drifted somewhere. Three said **nothing** until the wave 6.5
hardening batch; none does now, and the register is only useful while it is honest. Do not write an
enforcement here that you cannot point at.

1. **Rust is the source of truth.** Schemas are generated. Never hand-edit `schemas/generated/`; run
   `cargo xtask schema`.
   *Enforced by* `schema-check` in the gate (`cargo xtask schema --check`), which fails if the
   committed schemas differ from the types.
2. **Parse, then validate.** Documents deserialize into a `Raw*` type and become a domain type
   through `TryFrom`. Validated types do **not** implement `Deserialize`, so the only way to obtain
   one is to validate. Do not add `Deserialize` to a validated type to save a conversion.
   *Enforced by* a source scan, `crates/aep-domain/tests/invariants.rs`, over ten
   `Raw*`→validated pairs. It asserts the inverse too — the same extractor must *find* `Deserialize`
   on each `Raw*` — so a scan that has silently stopped working fails instead of passing.
3. **Validation accumulates.** A document with four broken references reports four errors. Push into
   `ValidationErrors`; do not return on the first failure.
   *Enforced by* per-type tests that assert an exact count rather than "is an error" — for example
   `crates/ess-domain/src/component.rs` expects four from one pass and
   `crates/aep-domain/src/domain_event.rs` expects three. There is no workspace-wide check: a new
   validator that returns early passes the gate.
4. **Every validation failure carries a stable `ValidationCode`.** Tests match on codes, never on
   message text.
   *Enforced by* the type — `ValidationError.code` is not optional — and by the `validation_codes!`
   macro in `crates/aep-domain/src/error.rs`, which generates `ValidationCode::ALL` from the same line
   as the variant, after five codes had fallen out of a hand-maintained list.
5. **`Unknown` is not `False`.** Predicate evaluation is three-valued; only `True` permits a
   transition. Never collapse unobserved to false. **A lapsed fact takes the same road**: past its
   requirement's horizon an observation stops counting and the requirement reads `Unknown`, never
   `False`, and the lapsed record's facts are withheld so a guard reading them refuses too. Nobody
   has looked lately is not the same finding as it is broken, and only one of them is fixed by
   changing code.
   *Enforced by* the `Truth` type: three variants, Kleene `and`/`or`, no `From<bool>` and no
   `as_bool`, so there is no boolean to collapse into. The Kleene tables have tests, the algebra's
   laws are property-checked over generated expressions
   (`crates/aep-domain/tests/truth_laws.rs`), and the decay is covered by
   `crates/aep-engine/tests/evidence_horizons.rs`. One deliberate exception is recorded rather than
   hidden: `evidence.missing` is a count, so `evidence.missing == 0` reads `False` on a lapse —
   pre-existing polarity of a count, and the reason `evidence.lapsed` exists beside it (gap
   register D-7).
6. **Capabilities default to deny**, and `deny` beats `require_approval` beats `allow`. A principle
   may restrict; only a profile or protocol may grant.
   *Enforced by* `CapabilityPolicy::decide` plus tests that first construct the state where each link
   decides anything: `a_denied_capability_is_not_downgraded_to_requiring_an_approval`
   (`crates/aep-domain/src/capability.rs`) asserts its fixture holds one capability in all three sets
   before asserting the outcome, and `crates/aep-domain/tests/safety_envelope.rs` covers the approval
   floor. Verified by mutation, not by reading.
7. **The engine never manufactures evidence, and it does not decide when the observation happened.**
   It evaluates what verifiers and humans produced. `observed_at` is the caller's and is required —
   there is no default, because a caller who has to write down when they looked cannot back-date by
   omission — and a submission claiming a future observation is refused rather than accepted as a
   fresh record. What the engine still stamps is the record's own envelope: the id, the clock time
   it was submitted at, and the producer.
   *Enforced by* a source scan, `crates/aep-engine/tests/evidence_scan.rs`, which reads the payload
   types off `Evidence` itself and refuses any construction of one in shipped engine code — struct
   literal, constructor path, variant expression or variant-as-function. Destructuring and the
   envelope stamp in `submit_evidence` stay allowed: reading evidence and stamping the id, clock
   time and producer onto a caller's payload are the engine's job. The scan's extractor is checked
   against the engine's own test modules, which construct evidence constantly, so a scan that has
   stopped seeing constructions fails on them instead of passing on everything. The rule reaches the
   driver too, one layer up and with a narrower ban:
   `crates/aep-driver/tests/evidence_scan.rs` refuses any construction of an `Evidence::Approval` or
   a `Producer::Human` in shipped driver code, because nothing below the driver would stop a harness
   from writing its own approval and unlocking a capability with it.
8. **The domain crate is clock-free and randomness-free.** No `SystemTime::now`, no RNG. The engine
   takes a `Clock` so an execution is replayable.
   *Enforced by* a banned-token scan, `crates/aep-domain/tests/determinism.rs` — boundary-aware,
   because `Operand::` contains `rand::`, and comment-skipping, because prose about the rule is not
   a breach of it. `aep-engine` is deliberately unscanned: `src/clock.rs` is the one place
   `SystemTime::now` is allowed to live, behind the `Clock` trait.
9. **Determinism.** Same validated state plus same evidence set ⇒ same decision. Iterate over
   `BTreeMap`/`BTreeSet`, never `HashMap`, so output ordering is stable.
   *Enforced by* banned-token scans over thirteen crates that claim the property or feed one that
   does — `ess-compiler` (`tests/billing.rs`), `ess-diff` (`tests/canonical.rs`), `ess-synth`
   (`tests/synthesis.rs`), `aep-domain`, `ess-gen`, `infra-domain`, `infra-compiler`,
   `infra-analyze`, `infra-project`, `infra-spec`, `aep-driver-spec`, `aep-driver` and `aep-render`
   (`tests/determinism.rs` in each) — beside tests that compile, diff, generate
   or render twice and compare bytes, and a seeded property test that does the same for every
   generated adversarial specification (`crates/ess-compiler/tests/adversarial.rs`).
   Three of the thirteen are the harness's. § 4.1 makes a purity claim for the two driver crates
   stronger than `aep-engine`'s: the routing core is clock-free and randomness-free, and the store
   lock, the pid-liveness probe and the run directory are `protocol-cli`'s precisely because a probe
   reads ambient OS state and would slip past this scan. `aep-render`'s scan is stronger again and
   bans **floats** as well, because its criterion is not *the same decision twice* but *the same
   bytes twice* — a committed figure that regenerates differently is a diff nobody chose — and the
   `--watch` loop, its poll interval and the terminal live in `protocol-cli` for the same reason the
   lock does.
   Deliberately unscanned, because each owns a clock or a terminal: `aep-engine` (invariant 8),
   `ess-conformance` (the runner takes a clock, wave 3.5 decision 3), the backends, the CLI and
   `xtask`. `ess-domain` states no determinism claim of its own.
10. **Document identity comes from document content**, not from filenames. A workflow's `id` is
    declared inside the file; loaders index by declared id.
    *Enforced by* the registry's signatures: `Registry::insert_*` takes a validated document and no
    path (`crates/aep-engine/src/registry.rs`), so there is no filename available to index by.
11. **Every public item is documented** (`missing_docs = "warn"`) and the workspace is
    clippy-pedantic clean.
    *Enforced by* `missing_docs` and `clippy::pedantic` in `[workspace.lints]`, raised to errors by
    the `clippy` step's `-D warnings`, plus the `doc-check` step (`RUSTDOCFLAGS=-D warnings`) for
    broken intra-doc links. All twenty-nine workspace members opt in with `[lints] workspace = true`;
    a new crate that omits that line is outside every lint here.
12. **No `unsafe`** (`unsafe_code = "forbid"`).
    *Enforced by* that lint in `[workspace.lints.rust]`. `forbid` cannot be lifted by an inner
    `allow`, so this one is closed rather than merely checked — again, for the twenty-nine members
    that opt in. **One crate cannot declare it and says so**: a `WebAssembly` export is a `#[no_mangle]`
    item, which rustc's own `unsafe_code` lint flags, so the emitted browser bridge under
    `generated/web/` and the host that links a realization into it (`examples/billing-web`, excluded
    from the workspace for exactly this reason) declare `#![deny(missing_docs)]` alone. Neither
    contains an `unsafe` block, an `unsafe fn` or a raw-pointer dereference; the property holds and
    the compiler is no longer the thing closing it, which is a named weakening in the bridge's own
    `TARGET.md` and a test in `crates/ess-synth/tests/web.rs`.
13. **Identity is opaque.** An `EntityId` is never parsed for meaning. A human-readable key belongs in
    the `EntityLocator`; the moment code reads structure out of an id, identity has become a key again.
    *Enforced by* the type: `EntityId(String)` has a private field and no structural accessor, and
    `EntityId::new` refuses anything under twelve characters, which is what catches `AUTH-142` going
    in as identity. Nothing stops code parsing the `Display` output back out.
14. **Every mutation is a command.** There is no second write path, because a second path is a second
    place to forget validation, authorisation, idempotency, provenance and audit.
    *Enforced by* `crates/aep-contract/tests/write_surface.rs`, which enumerates every method of
    every public trait in the contract and pins the list: `CommandService::execute` is the one
    write path. A new trait or method — required or default-bodied — fails the test with
    instructions to model the mutation as a command payload, or to change this invariant first.
15. **A refused command changes nothing and is still recorded.** `AuditRecord::validate` rejects a
    rejection that carries a change record.
    *Enforced by* `AuditRecord::validate` (`crates/aep-domain/src/audit.rs`) and its tests.
16. **Nothing is physically deleted.** `ArchiveEntity` and `SupersedeEntity` are the vocabulary.
    *Enforced by* the command vocabulary — there is no delete variant to call — and by a test that
    `CommandKind::parse("aep.entity.delete/v1")` fails, naming the kind it refused
    (`crates/aep-domain/src/command.rs`).
17. **A horizon lives on the requirement, and nothing mutates one.** How long an observation is
    worth something is a property of the question being asked, not of the observation — two
    requirements may legitimately read one record on different clocks. The refresh that is allowed
    is *observe again and write a new date*; there is deliberately no `extend`, because if
    extending were as easy to call as re-checking it is the one that gets called, every time, by
    whoever is trying to get a gate green.
    *Enforced by* three mechanisms in decreasing order of strength: an evidence record has **no
    horizon field**, so there is nothing on a record to mutate; a requirement's horizon comes from
    a parsed document and is re-read on every resolve, so an in-memory change does not survive; and
    a source scan over the five crates a horizon can be reached from,
    `crates/aep-domain/tests/horizon_immutability.rs`, refuses a mutator a later edit would
    otherwise add without argument. `Horizon::days` also refuses zero and anything over ten years,
    so a typo cannot become a horizon nothing will outlive.

## Gate

```console
task check
```

Ten steps, all ten of which CI also runs, in this order:

1. `fmt-check` — `cargo xtask fmt --check`, which formats exactly the workspace members. Not
   `cargo fmt --all`: that flag also reaches every member's local path dependencies, which since
   `examples/billing-realization` would hand the synthesised workspaces under `generated/rust/`
   to rustfmt — and their bytes are the emitter's, held byte-identical by `synth-check`.
2. `status-check` — `cargo xtask status --check`, which fails if the delivered-waves table in
   `docs/status.md` no longer matches what the annotated tags record. The one status surface that
   kept going stale by hand is derived instead; the fix is `cargo xtask status`.
3. `clippy` — `--workspace --all-targets -D warnings`, which is also what turns `missing_docs` and
   `clippy::pedantic` from warnings into failures.
4. `test` — `cargo test --workspace`.
5. `doc-check` — `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings`. Doc comments carry
   the design reasoning here, so a broken intra-doc link loses an argument, not a hyperlink.
6. `schema-check` — `cargo xtask schema --check`.
7. `generate-check` — `cargo xtask generate --check`, which fails if the committed projections under
   `generated/` differ from what the specification produces.
8. `suite-check` — `cargo xtask suite --check`, which fails if the committed conformance suites under
   `suites/generated/` differ from what the specifications produce. A suite is a contract an
   implementation is checked against, so a stale one certifies the wrong thing.
9. `infra-check` — `cargo xtask infra --check`, which fails if the committed observation IR,
   simulation, drift or projection tree under `examples/k3d-dev-cluster/` differs from what its
   inputs produce — including a projection file nothing generates any more. (This step was in the
   Taskfile and CI before it was in this list; the list was itself a stale copy.)
10. `synth-check` — `cargo xtask synth --check`, which fails if any committed synthesised tree —
   `generated/rust/`, `generated/go/` and `generated/web/`, three emitters behind one
   language-neutral plan, for two specifications — differs from what the specifications determine; if a matching tree no
   longer builds (`cargo check` for the Rust workspace; `gofmt -l` empty, `go build ./...` and
   `go vet ./...` for the Go module; `cargo build --release --target wasm32-unknown-unknown` for
   the browser bridge and for the host that links a realization into it); if the emitted page calls
   an export its module does not have, or the module exports one no page names — HTML's version of
   a dangling reference, checked against the compiled module's own export table because nothing in
   a browser would refuse it; if the committed billing suite no longer holds against the workspace
   linked with `examples/billing-realization`, where the honest linkage must pass all 27 scenarios
   and the deliberately corrupted one must fail exactly the scenario that exists to catch it; or if
   the browser boundary no longer holds — the realized module is loaded outside a browser through
   the page's own `bridge.js` and driven through one round trip, and seventeen claims about it must
   stand; or if the **dual-target demonstration** stops holding — the two applications synthesised
   from `examples/gatepass/` are built from the committed trees plus their hand-written
   realizations, started on ephemeral ports, and compared on their startup records outside
   `runtime`, on the status and the body of seven exchanges, and on the two documents they publish
   about themselves byte for byte. A tree that matches its specification and still fails here is a
   defect in `ess-synth` or in the realization, not in any specification.

   **It needs the Go toolchain, the `wasm32-unknown-unknown` target and Node**, and says which is
   missing rather than skipping — a check that quietly passes without its toolchain reads exactly
   like a check that passed. `cargo test` needs all three too, because `xtask`'s own tests write
   all three trees and build them.

Land nothing that does not pass all ten.

**A green local gate does not guarantee a green CI.** The steps mirror each other exactly, but the
*toolchain* does not: CI installs whatever `stable` is on the day, and a newer clippy can introduce a
lint that fails a commit which passed locally on an older one. That is how `clippy::unused_async`
turned `main` red on a commit whose gate was green. Run `rustup update` before pushing anything you
will not get a second chance at — and when CI fails on a lint that did not exist locally, that is the
cause, not a flaky gate.

**A release is the procedure in § *Tags*, and nothing mechanical enforces it.** A wave ships,
`CHANGELOG.md` is cut under its heading, `cargo xtask status` regenerates the delivered-waves record
and the annotated tag is written at the commit that delivered the work. The full gate comes first —
component gates are not enough — and no hook, task or CI job checks that it did, which makes it a
discipline rather than a guarantee. It has already slipped once, and the mechanism is worth
knowing: `task check 2>&1 | tail` reports **`tail`'s** exit status, not the gate's, so two runs that
aborted at the first step read as green and two commits were pushed claiming a gate that had never
run past `fmt-check`. Read the gate's own status, not a pipeline's.

## Conventions

* **Tests live beside the code** they test, in a `#[cfg(test)] mod tests`. Name a test after the
  behaviour it protects, not the function it calls: `an_approval_of_version_three_does_not_cover_version_seven`.
* **Every test asserts a reason.** Prefer `expect_err` plus a check on the `ValidationCode` over
  `assert!(result.is_err())`.
* **A test must reach the state where the rule is load-bearing.** A precedence rule needs a fixture
  that populates both sides; a refusal rule needs a refusal in the fixture. A test that would pass
  whether or not the rule holds is not a test of the rule, whatever its name says. Where reaching that
  state takes work, assert that the fixture reached it before asserting the outcome — see
  `a_denied_capability_is_not_downgraded_to_requiring_an_approval` in
  `crates/aep-domain/src/capability.rs`.
* **Verify a guard by breaking it.** Before trusting a new test, apply the one-line mutation it is
  meant to catch, watch it fail with a message that names the defect, and revert. A test that still
  passes under the mutation was never guarding anything; a test that fails with an unreadable message
  costs the next reader an hour.
* **Rust CLIs use `clap`'s derive API.** Hand-rolled argument parsing is not accepted.
* **Task runner is `Taskfile.yml`** (go-task). Do not add a Makefile.
* **Comments explain why**, and only where the reason is not evident from the code. Doc comments on
  public items explain what the type is *for*, and where a design decision is embedded in it, why.
* **Claim ids are singular and shared.** A verification claim is a fact path segment
  (`verification.<claim>.passed`), so `invariant` and `invariants` are different claims and evidence
  for one does not satisfy a requirement for the other. Existing claims: `precondition`,
  `postcondition`, `invariant`, `hypothesis`, `recovery`, `blast-radius`, `clean-room`,
  `differential`, `mutation`, `migration`, `dry-run`. Reuse one before inventing another.
* **`<claim>_verified` is projected but not observable.** The engine emits it, but no protocol
  declares the bare namespace, so a predicate cannot read it — except `recovery_verified`, which
  `aop/1` declares explicitly for the incident profile. Write `verification.<claim>.passed` instead.
* **Wire-format aliases are deliberate.** `unit_tests.failed` alongside `tests.unit.failed`,
  `test_execution` alongside `test_result`: both spellings appear in the design documents. Canonical
  forms are what the engine emits; aliases are only accepted on input, and each is documented on the
  type that projects it.

## Dependencies

Written down because it is already practised, and an unwritten standard is one the next agent meets
only by violating it.

* **The workspace has ten direct third-party crates.** Seven are declared once in
  `[workspace.dependencies]` — `serde`, `serde_json`, `serde_yaml`, `schemars`, `thiserror`, `clap`,
  `anyhow` — and three are crate-local: `sha2` wherever a document is content-addressed
  (`ess-gen`, `trace-domain`, `infra-domain`, `infra-compiler`, `aep-driver-spec`), `jsonschema` as
  a dev-dependency of `ess-gen` and `aep-schema`, and `proptest` in `aep-domain` and as a
  dev-dependency of `ess-compiler` (`default-features = false`, and every property runs under a
  fixed seed so the gate cannot be flaky — the seed and the way to widen locally are documented
  where each is used). Reach for the workspace list before adding to it.
* **A non-workspace dependency carries its justification in the manifest**, beside the line that adds
  it: what it buys, which features are dropped and why that is safe here, and why the version matches
  the other crate that uses it. `crates/ess-gen/Cargo.toml` is the model.
* **Prefer no dependency, and record the refusal.** `crates/aep-domain/tests/invariants.rs` opens by
  weighing three mechanisms and taking the one that needs no new crate, saying what `trybuild` would
  have cost; `crates/ess-compiler/tests/billing.rs` scans its own sources on the same reasoning. Where
  a crate is taken, its surface is cut to what is used — `jsonschema` runs with
  `default-features = false`, which drops `resolve-http`, `resolve-file` and the TLS backend.
* **Nothing in `task check` reaches the network.** No step downloads a schema, resolves a remote
  `$ref` or calls an API — `jsonschema` is built with `default-features = false` for exactly that
  reason. The Go steps hold the same line by construction: the generated module has no
  dependencies, and every `go` invocation runs with `GOPROXY=off` and `GOTOOLCHAIN=local`, so
  neither a dependency nor a `go` directive can make the toolchain fetch anything. The browser
  target holds it the same way, and it is the reason that target has **no `wasm-bindgen`**: that
  crate needs a cargo-installed CLI pinned to its own version, and the emitted tree would then
  resolve third-party crates inside a gate step. It emits its own JSON reader, writer and base64
  codec instead — about seven hundred fixed lines, the same bytes for every specification — and its
  manifest carries nothing but path dependencies into the Rust target's tree, which a test asserts.
  Keep it that way: a gate that needs the network is a gate that goes red for reasons that have
  nothing to do with the change.

## Changelog

`CHANGELOG.md` is maintained with the work, not reconstructed before a release. Every change that
alters what a *user of the protocol* sees — a new document type, a changed fact spelling, a rule that
now refuses something it used to allow — gets a line under `## [Unreleased]` in the same commit that
makes the change. Internal refactors that change nothing observable do not.

Write the entry for the person hitting the behaviour, not for the person who wrote it: "an approval
of version 3 no longer satisfies a review requirement for version 7", not "added freshness check".

## Tags

Each delivered wave gets an annotated tag named after its `CHANGELOG.md` heading — `0.1.0`,
`0.2.0-wave-1`, `0.2.0-wave-2` — pointing at the commit that delivered the work, not at the
changelog housekeeping that follows it. The tag message states what the wave delivered and the
implementation percentage after it, so `git tag -n99` reads as a project history without opening a
browser.

## Commits

* Conventional prefixes: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
* Title, blank line, then a body explaining what changed and why. No title-only commits.
* Ticket references go in a `Refs:` tagline at the end of the body, never in the title.
* Write messages through a file or a quoted heredoc (`git commit -F -` with `<<'MSG'`), never
  `-m "…"` with backticks in the text.
