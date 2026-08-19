# Working agreement

For humans and agents working in this repository. Read this before changing anything.

## What this repository is

A machine-executable specification of engineering methodology: principles, workflows, capabilities,
evidence and verification, expressed as typed Rust and generated JSON Schema rather than as prose in
a prompt. It is a **library and specification**, not an agent, a CI system or a deployment platform.

The authoritative specification is [`docs/design/consolidated-design-v0.2.md`](docs/design/consolidated-design-v0.2.md).
When code and that document disagree, the document wins unless the disagreement is recorded in
[`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) §5 as a deliberate
deviation. Add to that list rather than diverging silently.

## Current state

See the status table in [`README.md`](README.md); keep it accurate when you land work.

* Implemented and gated: `aep-domain`, `aep-schema`, `xtask schema`.
* Skeletons with documented planned surfaces: `aep-engine`, `aep-contract`, `aep-conformance`,
  `adp-domain`, `aop-domain`, `protocol-cli`.
* Work order: [`docs/design/reconciliation-v0.2.md`](docs/design/reconciliation-v0.2.md) §4.

## Invariants

These hold across the workspace. Breaking one is a design change, not a refactor.

1. **Rust is the source of truth.** Schemas are generated. Never hand-edit `schemas/generated/`; run
   `cargo xtask schema`.
2. **Parse, then validate.** Documents deserialize into a `Raw*` type and become a domain type
   through `TryFrom`. Validated types do **not** implement `Deserialize`, so the only way to obtain
   one is to validate. Do not add `Deserialize` to a validated type to save a conversion.
3. **Validation accumulates.** A document with four broken references reports four errors. Push into
   `ValidationErrors`; do not return on the first failure.
4. **Every validation failure carries a stable `ValidationCode`.** Tests match on codes, never on
   message text.
5. **`Unknown` is not `False`.** Predicate evaluation is three-valued; only `True` permits a
   transition. Never collapse unobserved to false.
6. **Capabilities default to deny**, and `deny` beats `require_approval` beats `allow`. A principle
   may restrict; only a profile or protocol may grant.
7. **The engine never manufactures evidence.** It evaluates what verifiers and humans produced.
8. **The domain crate is clock-free and randomness-free.** No `SystemTime::now`, no RNG. The engine
   takes a `Clock` so an execution is replayable.
9. **Determinism.** Same validated state plus same evidence set ⇒ same decision. Iterate over
   `BTreeMap`/`BTreeSet`, never `HashMap`, so output ordering is stable.
10. **Document identity comes from document content**, not from filenames. A workflow's `id` is
    declared inside the file; loaders index by declared id.
11. **Every public item is documented** (`missing_docs = "warn"`) and the workspace is
    clippy-pedantic clean.
12. **No `unsafe`** (`unsafe_code = "forbid"`).

## Gate

```console
task check
```

Format check, `clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, and
`cargo xtask schema --check`. Land nothing that does not pass it. There is no release process yet;
when there is, releases require a green full suite, not component gates.

## Conventions

* **Tests live beside the code** they test, in a `#[cfg(test)] mod tests`. Name a test after the
  behaviour it protects, not the function it calls: `an_approval_of_version_three_does_not_cover_version_seven`.
* **Every test asserts a reason.** Prefer `expect_err` plus a check on the `ValidationCode` over
  `assert!(result.is_err())`.
* **Rust CLIs use `clap`'s derive API.** Hand-rolled argument parsing is not accepted.
* **Task runner is `Taskfile.yml`** (go-task). Do not add a Makefile.
* **Comments explain why**, and only where the reason is not evident from the code. Doc comments on
  public items explain what the type is *for*, and where a design decision is embedded in it, why.
* **Wire-format aliases are deliberate.** `unit_tests.failed` alongside `tests.unit.failed`,
  `test_execution` alongside `test_result`: both spellings appear in the design documents. Canonical
  forms are what the engine emits; aliases are only accepted on input, and each is documented on the
  type that projects it.

## Commits

* Conventional prefixes: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
* Title, blank line, then a body explaining what changed and why. No title-only commits.
* Ticket references go in a `Refs:` tagline at the end of the body, never in the title.
* Write messages through a file or a quoted heredoc (`git commit -F -` with `<<'MSG'`), never
  `-m "…"` with backticks in the text.
