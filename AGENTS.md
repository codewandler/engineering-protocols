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

**Everything else in `docs/design/` is proposed until a plan page in `docs/plan/` accepts it.** A
proposal is not a work order, however long and however recent it is.
`ess-implementor-design-v0.1.md` and `ess-review-v0.1.md` show what acceptance looks like:
[`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md) and the wave 1–3 plan pages took them up, and
waves 1 to 3 shipped from them. The four proposals filed 2026-08-20 have since been reviewed, and
their acceptance state is:

| proposed design | status |
|---|---|
| [`ess-closed-loop-execution-conformance-design-v0.1.md`](docs/design/ess-closed-loop-execution-conformance-design-v0.1.md) | **implemented** as ESS wave 4 (`0.4.0-ess-wave-4`); its four open decisions D1–D4 were taken at their stated defaults |
| [`ess-semantic-diff-impact-evolution-design-v0.1.md`](docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md) | **core implemented** as ESS wave 5 (`0.5.0-ess-wave-5`). Two of its seventy-eight sections are rejected outright (the proposal-evaluation loop and architecture search); the rest past §31 stays proposed |
| [`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md) | **accepted in part** by [`docs/plan/ess-wave-6-structural-synthesis.md`](docs/plan/ess-wave-6-structural-synthesis.md), which is wave 6, in progress. Its obligation/`Realization` programme stays proposed (W7.4 takes a slice), and its §28 is refused by invariant 6 |
| [`semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md`](docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md) | reviewed and **deferred whole**; two ideas harvested |

Do not implement from an unreviewed design, and do not treat one as evidence of what this repository
is. [`docs/VISION.md`](docs/VISION.md) § *Proposed, not accepted* says what each would add, and
[`docs/plan/gap-register.md`](docs/plan/gap-register.md) holds every open gap with what closes it.

## Current state

See the status table in [`README.md`](README.md); keep it accurate when you land work. `git tag -n99`
is the per-wave record of what actually shipped — read it before believing any prose about progress.

**Every crate in the workspace is implemented and gated. There are no skeletons left.** The most
recent tag is `0.6.1-ess-wave-6.5`; `task check` (nine steps) currently passes 94 suites and 1693
tests, with 0 clippy warnings and 0 rustdoc warnings. The gate now needs three toolchains beside
Rust's own: the **Go toolchain**, the **`wasm32-unknown-unknown` target** and **Node**. Two of the
nine steps build the second and third emitters' committed trees, and none of those checks skips
when its toolchain is absent — it fails and names it, because a skipped check reads exactly like a
passing one.

* **AEP — the protocol; the v0.2 scope is implemented.** `aep-domain`, `aep-schema`, `aep-engine`,
  `aep-contract`, `aep-backend-memory`, `aep-conformance`, `adp-domain`, `aop-domain`,
  `protocol-cli` and `xtask`, plus the document tree (`protocols/`, `principles/`, `workflows/`,
  `profiles/`, `artifacts/lifecycles/`). `aep-conformance`, `adp-domain` and `aop-domain` all
  shipped in `0.2.0-wave-3`: sixteen black-box suites over the command and query surfaces, three
  conformance levels, and a `FaultyBackend` whose injected defects the suites are checked against
  (45 inline plus 7 integration tests), beside the development and operations typed vocabularies.
  `0.2.1` added project discovery.
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
* **Infra — observed infrastructure as a second instance of the ESS pattern.** Four crates:
  `infra-domain` (the k8s observation subset, raw→validated, eleven `INFRA-*` refusal codes,
  secrets only ever as digests — IW1), `infra-compiler` (the content-addressed `infra-ir/1`
  with unresolved references as typed facts, plus the validating read-back of a persisted
  document — IW1/IW2), `infra-analyze` (the typed dependency graph with exact pod ownership,
  twenty `INFRA-DIAG-*` diagnosis rules with registered severities, workload properties,
  invariant candidates and directions — IW2) and `infra-spec` (the authored desired state:
  twelve expectation kinds evaluated three-valued against a snapshot, where a False without a
  gap or an Unknown without a reason is unrepresentable — IW3).
  `protocol infra validate|compile|inspect|graph|diagnose|view|simulate|diff` is the surface;
  the scanner (`infra-scout`) is a separate repository holding the credentials, and nothing
  here reaches a network. Plan pages: `docs/plan/infra-wave-1-observe.md`,
  `docs/plan/infra-wave-2-analyze.md`.
* **Not built yet:** wave 7, scheduled on the roadmap (the wave 6.5 hardening batch is done:
  chunk A closed the three unenforced invariants, the digest widening and `proptest` phase 1;
  chunk B closed the input→event-payload model gap — an outcome's `payload:` declaration — and
  the value-object invariant scenarios);
  attested evidence (gap register D-3, proposed and unaccepted); any durable backend — the only
  implementation of the contract is in memory.
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
   transition. Never collapse unobserved to false.
   *Enforced by* the `Truth` type: three variants, Kleene `and`/`or`, no `From<bool>` and no
   `as_bool`, so there is no boolean to collapse into. The Kleene tables have tests, and the
   algebra's laws are property-checked over generated expressions
   (`crates/aep-domain/tests/truth_laws.rs`).
6. **Capabilities default to deny**, and `deny` beats `require_approval` beats `allow`. A principle
   may restrict; only a profile or protocol may grant.
   *Enforced by* `CapabilityPolicy::decide` plus tests that first construct the state where each link
   decides anything: `a_denied_capability_is_not_downgraded_to_requiring_an_approval`
   (`crates/aep-domain/src/capability.rs`) asserts its fixture holds one capability in all three sets
   before asserting the outcome, and `crates/aep-domain/tests/safety_envelope.rs` covers the approval
   floor. Verified by mutation, not by reading.
7. **The engine never manufactures evidence.** It evaluates what verifiers and humans produced.
   *Enforced by* a source scan, `crates/aep-engine/tests/evidence_scan.rs`, which reads the payload
   types off `Evidence` itself and refuses any construction of one in shipped engine code — struct
   literal, constructor path, variant expression or variant-as-function. Destructuring and the
   envelope stamp in `submit_evidence` stay allowed: reading evidence and stamping the id, clock
   time and producer onto a caller's payload are the engine's job. The scan's extractor is checked
   against the engine's own test modules, which construct evidence constantly, so a scan that has
   stopped seeing constructions fails on them instead of passing on everything.
8. **The domain crate is clock-free and randomness-free.** No `SystemTime::now`, no RNG. The engine
   takes a `Clock` so an execution is replayable.
   *Enforced by* a banned-token scan, `crates/aep-domain/tests/determinism.rs` — boundary-aware,
   because `Operand::` contains `rand::`, and comment-skipping, because prose about the rule is not
   a breach of it. `aep-engine` is deliberately unscanned: `src/clock.rs` is the one place
   `SystemTime::now` is allowed to live, behind the `Clock` trait.
9. **Determinism.** Same validated state plus same evidence set ⇒ same decision. Iterate over
   `BTreeMap`/`BTreeSet`, never `HashMap`, so output ordering is stable.
   *Enforced by* banned-token scans over nine crates that claim the property or feed one that
   does — `ess-compiler` (`tests/billing.rs`), `ess-diff` (`tests/canonical.rs`), `ess-synth`
   (`tests/synthesis.rs`), `aep-domain`, `ess-gen`, `infra-domain`, `infra-compiler` and
   `infra-analyze` (`tests/determinism.rs` in each) — beside tests that compile, diff, generate
   or render twice and compare bytes, and a seeded property test that does the same for every
   generated adversarial specification (`crates/ess-compiler/tests/adversarial.rs`).
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
    broken intra-doc links. All fifteen workspace members opt in with `[lints] workspace = true`; a
    new crate that omits that line is outside every lint here.
12. **No `unsafe`** (`unsafe_code = "forbid"`).
    *Enforced by* that lint in `[workspace.lints.rust]`. `forbid` cannot be lifted by an inner
    `allow`, so this one is closed rather than merely checked — again, for the fourteen members that
    opt in. **One crate cannot declare it and says so**: a `WebAssembly` export is a `#[no_mangle]`
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

## Gate

```console
task check
```

Nine steps, all nine of which CI also runs, in this order:

1. `fmt-check` — `cargo xtask fmt --check`, which formats exactly the workspace members. Not
   `cargo fmt --all`: that flag also reaches every member's local path dependencies, which since
   `examples/billing-realization` would hand the synthesised workspaces under `generated/rust/`
   to rustfmt — and their bytes are the emitter's, held byte-identical by `synth-check`.
2. `clippy` — `--workspace --all-targets -D warnings`, which is also what turns `missing_docs` and
   `clippy::pedantic` from warnings into failures.
3. `test` — `cargo test --workspace`.
4. `doc-check` — `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings`. Doc comments carry
   the design reasoning here, so a broken intra-doc link loses an argument, not a hyperlink.
5. `schema-check` — `cargo xtask schema --check`.
6. `generate-check` — `cargo xtask generate --check`, which fails if the committed projections under
   `generated/` differ from what the specification produces.
7. `suite-check` — `cargo xtask suite --check`, which fails if the committed conformance suites under
   `suites/generated/` differ from what the specifications produce. A suite is a contract an
   implementation is checked against, so a stale one certifies the wrong thing.
8. `synth-check` — `cargo xtask synth --check`, which fails if any committed synthesised tree —
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

Land nothing that does not pass all nine.

**A green local gate does not guarantee a green CI.** The steps mirror each other exactly, but the
*toolchain* does not: CI installs whatever `stable` is on the day, and a newer clippy can introduce a
lint that fails a commit which passed locally on an older one. That is how `clippy::unused_async`
turned `main` red on a commit whose gate was green. Before pushing something you cannot easily revisit,
`rustup update` first — and when CI fails on a lint that did not exist locally, that is the cause, not
a flaky gate. There is no release process yet; when there is, releases
require a green full suite, not component gates.

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
  `anyhow` — and three are crate-local: `sha2` in `ess-gen`, `jsonschema` as a dev-dependency of
  `ess-gen` and `aep-schema`, and `proptest` as a dev-dependency of `aep-domain` and `ess-compiler`
  (`default-features = false`, and every property runs under a fixed seed so the gate cannot be
  flaky — the seed and the way to widen locally are documented where each is used). Reach for the
  workspace list before adding to it.
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
