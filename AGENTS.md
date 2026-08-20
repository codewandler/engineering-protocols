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
waves 1 to 3 shipped from them. Four proposals are currently open, and none has been accepted:

| proposed design | status |
|---|---|
| [`ess-closed-loop-execution-conformance-design-v0.1.md`](docs/design/ess-closed-loop-execution-conformance-design-v0.1.md) | reviewed, reconciled against the code, and frozen for implementation — except the four open decisions D1–D4 named in its §2. This is ESS wave 4, and it starts when [`docs/plan/ess-wave-3.5-reconciliation.md`](docs/plan/ess-wave-3.5-reconciliation.md) closes its gates |
| [`ess-semantic-diff-impact-evolution-design-v0.1.md`](docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md) | **not reviewed at all.** Sequenced after wave 4 |
| [`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md) | reviewed by [`docs/reviews/2026-08-20-next-waves-feasibility-review.md`](docs/reviews/2026-08-20-next-waves-feasibility-review.md) and **not reconciled**: nothing was folded back in, and that review reads the document as four waves rather than one. Unsequenced |
| [`semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md`](docs/design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md) | **not reviewed at all.** Unsequenced |

Do not implement from an unreviewed design, and do not treat one as evidence of what this repository
is. [`docs/VISION.md`](docs/VISION.md) § *Proposed, not accepted* says what each would add.

## Current state

See the status table in [`README.md`](README.md); keep it accurate when you land work. `git tag -n99`
is the per-wave record of what actually shipped — read it before believing any prose about progress.

**Every crate in the workspace is implemented and gated. There are no skeletons left.** The most
recent tag is `0.3.2-ess-wave-3`; `task check` currently passes 41 suites and 953 tests, with 0
clippy warnings and 0 rustdoc warnings.

* **AEP — the protocol; the v0.2 scope is implemented.** `aep-domain`, `aep-schema`, `aep-engine`,
  `aep-contract`, `aep-backend-memory`, `aep-conformance`, `adp-domain`, `aop-domain`,
  `protocol-cli` and `xtask`, plus the document tree (`protocols/`, `principles/`, `workflows/`,
  `profiles/`, `artifacts/lifecycles/`). `aep-conformance`, `adp-domain` and `aop-domain` all
  shipped in `0.2.0-wave-3`: sixteen black-box suites over the command and query surfaces, three
  conformance levels, and a `FaultyBackend` whose injected defects the suites are checked against
  (45 inline plus 7 integration tests), beside the development and operations typed vocabularies.
  `0.2.1` added project discovery.
* **ESS — executable system specifications; roughly 60% of its design.** Three delivered, gated
  crates: `ess-domain` (the typed model, `0.3.0-ess-wave-1`), `ess-compiler` (resolution, `EssIr`,
  source-aware diagnostics, `0.3.1-ess-wave-2`) and `ess-gen` (Markdown + Mermaid, JSON Schema,
  OpenAPI 3.1 and AsyncAPI 3.0 behind one `Generator` trait, `0.3.2-ess-wave-3`). `generated/`
  holds 27 committed artifacts plus an index, drift-checked by `cargo xtask generate --check`.
* **Not built yet:** test synthesis and the conformance runner (ESS wave 4), Rust structural
  synthesis (ESS wave 5), and any durable backend — the only implementation of the contract is in
  memory.
* Work order: [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) §4 for AEP,
  [`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md) for ESS. Wave 4 does not start until
  [`docs/plan/ess-wave-3.5-reconciliation.md`](docs/plan/ess-wave-3.5-reconciliation.md) closes its
  gates.

## Invariants

These hold across the workspace. Breaking one is a design change, not a refactor.

Each carries what actually enforces it — a lint, a type, a test or a scan — because a rule nothing
checks is a rule that has already drifted somewhere. Three say **nothing**. That is not an oversight
to be papered over; it is the target list for the next mutation review, and it is only useful while it
is honest. Do not write an enforcement here that you cannot point at.

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
   `as_bool`, so there is no boolean to collapse into. The Kleene tables have tests.
6. **Capabilities default to deny**, and `deny` beats `require_approval` beats `allow`. A principle
   may restrict; only a profile or protocol may grant.
   *Enforced by* `CapabilityPolicy::decide` plus tests that first construct the state where each link
   decides anything: `a_denied_capability_is_not_downgraded_to_requiring_an_approval`
   (`crates/aep-domain/src/capability.rs`) asserts its fixture holds one capability in all three sets
   before asserting the outcome, and `crates/aep-domain/tests/safety_envelope.rs` covers the approval
   floor. Verified by mutation, not by reading.
7. **The engine never manufactures evidence.** It evaluates what verifiers and humans produced.
   *Enforced by* **nothing**. It is a property of how the API is used, and `docs/guide/harness.md`
   states it as a rule for harness authors — which is exactly the shape of rule this repository exists
   to replace.
8. **The domain crate is clock-free and randomness-free.** No `SystemTime::now`, no RNG. The engine
   takes a `Clock` so an execution is replayable.
   *Enforced by* **nothing**. `crates/aep-domain/src/` contains no `SystemTime` and no RNG today; the
   scan that would catch one being added covers `ess-compiler` only (invariant 9).
9. **Determinism.** Same validated state plus same evidence set ⇒ same decision. Iterate over
   `BTreeMap`/`BTreeSet`, never `HashMap`, so output ordering is stable.
   *Enforced by* a banned-token scan over `crates/ess-compiler/src` **only**
   (`no_source_file_in_the_compiler_reads_a_clock_or_an_unordered_map`, in
   `crates/ess-compiler/tests/billing.rs`), beside a test that compiles the same source twice and
   compares bytes. The other eleven crates and `xtask` are unscanned.
10. **Document identity comes from document content**, not from filenames. A workflow's `id` is
    declared inside the file; loaders index by declared id.
    *Enforced by* the registry's signatures: `Registry::insert_*` takes a validated document and no
    path (`crates/aep-engine/src/registry.rs`), so there is no filename available to index by.
11. **Every public item is documented** (`missing_docs = "warn"`) and the workspace is
    clippy-pedantic clean.
    *Enforced by* `missing_docs` and `clippy::pedantic` in `[workspace.lints]`, raised to errors by
    the `clippy` step's `-D warnings`, plus the `doc-check` step (`RUSTDOCFLAGS=-D warnings`) for
    broken intra-doc links. All fourteen workspace members opt in with `[lints] workspace = true`; a
    new crate that omits that line is outside every lint here.
12. **No `unsafe`** (`unsafe_code = "forbid"`).
    *Enforced by* that lint in `[workspace.lints.rust]`. `forbid` cannot be lifted by an inner
    `allow`, so this one is closed rather than merely checked — again, for the thirteen members that
    opt in.
13. **Identity is opaque.** An `EntityId` is never parsed for meaning. A human-readable key belongs in
    the `EntityLocator`; the moment code reads structure out of an id, identity has become a key again.
    *Enforced by* the type: `EntityId(String)` has a private field and no structural accessor, and
    `EntityId::new` refuses anything under twelve characters, which is what catches `AUTH-142` going
    in as identity. Nothing stops code parsing the `Display` output back out.
14. **Every mutation is a command.** There is no second write path, because a second path is a second
    place to forget validation, authorisation, idempotency, provenance and audit.
    *Enforced by* **nothing**. One write path is a property of the contract's current shape; adding a
    second would compile and pass the gate.
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

Eight steps, all eight of which CI also runs, in this order:

1. `fmt-check` — `cargo fmt --all -- --check`.
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
8. `synth-check` — `cargo xtask synth --check`, which fails if the committed synthesised workspaces
   under `generated/rust/` differ from what the specifications determine, or if a matching tree no
   longer passes `cargo check` — the latter being a defect in `ess-synth`, not in any specification.

Land nothing that does not pass all seven.

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

* **The workspace has nine direct third-party crates.** Seven are declared once in
  `[workspace.dependencies]` — `serde`, `serde_json`, `serde_yaml`, `schemars`, `thiserror`, `clap`,
  `anyhow` — and two are crate-local: `sha2` in `ess-gen`, and `jsonschema` as a dev-dependency of
  `ess-gen` and `aep-schema`. Reach for the workspace list before adding to it.
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
  reason. Keep it that way: a gate that needs the network is a gate that goes red for reasons that
  have nothing to do with the change.

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
