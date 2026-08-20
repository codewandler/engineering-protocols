# ESS roadmap — waves 1 to 5

> **Waves 1 and 2 delivered; 3 to 5 proposed.** Numbering restarts: these are ESS waves, not a
> continuation of the protocol's. Tags are `0.3.0-ess-wave-1` and so on, keeping the convention that
> a tag is named after its changelog heading.
>
> The five waves are design §32's seven phases, with 3 and 4 merged (both are projections of the same
> IR, and separating them would mean shipping documentation nothing checks) and 7 left out (behavioural
> synthesis is not worth attempting before structural synthesis has produced something that builds):
>
> | wave | design phase |
> |---|---|
> | 1 | 1 — core domain |
> | 2 | 2 — compiler |
> | 3 | 3 + 4 — documentation, OpenAPI, AsyncAPI |
> | 4 | 5 — test synthesis |
> | 5 | 6 — Rust structural synthesis |

Scope and ordering follow [`ess-review-v0.1.md`](../design/ess-review-v0.1.md): the review's findings
are folded into the waves that would otherwise have to unlearn them. Rust structural synthesis
(design §32 phase 6) is deliberately outside this roadmap — it is wave 4 at the earliest, and only
once the model has survived three projections.

The ordering rule throughout: **each wave must be falsifiable by the one before it.** A generated
artifact nothing can check is a claim, not a deliverable.

---

## ESS wave 1 — the join, and the model

> **Delivered.** See [`ess-wave-1-the-model.md`](ess-wave-1-the-model.md).

**Goal: an ESS is a document this repository can parse, validate and refuse — and the protocol can
already require conformance to one.**

### W1.1 Close the loop first (review F6)

Independent of every line of ESS code, and it makes the vision testable before the compiler exists:

| Change | Where |
|---|---|
| `ArtifactKind::ExecutableSystemSpecification` | `aep-domain/src/artifact.rs` |
| `EvidenceKind::EssConformance` — spec version, implementation identity, suite version, compiler version, generator version, status | `aep-domain/src/evidence.rs` |
| facts `ess_conformance.{status,passed,spec_version}` | same |
| `'ess_conformance.**'` observable | `protocols/adp/1.yaml` |
| principle `ess-conformance` requiring that evidence, `independent: true` | `principles/verification/` |

A task can then require ESS conformance and refuse to complete without it, with a human producing the
evidence by hand. That is the whole vision, working, before anything is compiled.

### W1.2 `ess-domain` — the model

Identity split at the start, not "eventually" (F5): opaque id, logical locator, wire name — reusing
`ep://` rather than inventing `ess://` (one scheme, not a translation table).

Type system: primitives, `Struct`, `Enum`, `Optional`, `List`, `Map`, and **tagged** `Union` only
(F11). Domain wrappers stay distinct from their representations — `Email` is not `String`.

Domain layer: domains, entities, value objects, events, actors and roles, and:

* **Commands carry outcomes** (F1), not a bare `emits`:

  ```yaml
  outcomes:
    accepted: {when: amount.amount > 0, emits: [InvoiceCreated]}
    rejected: {when: amount.amount <= 0, error: InvalidAmount}
  ```

  with a declared error vocabulary per domain. A command whose outcomes do not cover its input space
  is a validation error.
* **Views declare consistency** (F2): `read_your_writes` or `eventual`, which is what decides whether
  a generated assertion is `expect` or `eventually`.
* **Invariants are typed predicates** (F4), reusing `aep_domain::predicate` with paths resolved
  against the ESS type registry. Transition legality is structural — the absence of a transition —
  not a sentence.

Raw → validated with accumulating errors and stable codes, exactly as the protocol half does;
validated types do not implement `Deserialize`.

### W1.3 The billing fixture, and schemas

§31's example becomes the single normative fixture (F7), parsed by a test in this repository, with
every other snippet in the design derived from it. `cargo xtask schema` publishes ESS's own JSON
Schema alongside the protocol's, drift-checked in CI.

**Deliverable:** `ess validate` accepts the billing example and rejects a catalogue of malformed ones,
each with a code and a test. Roughly 120 tests.

---

## ESS wave 2 — the compiler

**Goal: source becomes a normalized IR with no unresolved references, and every rejection in §20 has
a test.**

### W2.1 Resolution and semantic validation

The §20 list, each with its own code, message and failing fixture: missing types, commands
referencing undefined events, components accepting undefined commands, invalid binding mappings,
unreachable states, invalid transitions, views exposing missing fields, topology references to
missing components, contradictory invariants, forbidden dependency cycles.

Binding mapping validation is where this earns its keep: `InvoiceCreated.customer_email: Email` into
`SendEmail.recipient: Email` must typecheck, and `Email` into `VerifiedEmail` must fail with the
diagnostic §29 already drafts.

### W2.2 Components, bindings, topology

Components with inner domain and outer surface (§6). **Bindings state delivery and failure** (F3) —
`delivery: at_least_once`, `on_failure: retry | escalate | drop`, with `drop` a word someone typed.
Topology is modelled and generates nothing this wave.

### W2.3 `EssIr`, diagnostics, determinism

A normalized IR whose type prevents unresolved references. Source-aware diagnostics carrying code,
span and a machine-readable body, because a coding agent consumes them as repair instructions.

Determinism made true rather than asserted (F8): `BTreeMap`/`BTreeSet` only, no clock or RNG in the
compiler, canonical serialisation, and a CI job that regenerates everything and fails on a diff.

**Deliverable:** `ess compile`, `ess inspect`, `ess graph` over billing; every §20 rejection covered.

---

## ESS wave 3 — projections that pay for themselves

**Goal: one model, three projections, and documentation that fails the build when the model outgrows
it.**

Design phases 3 and 4. Documentation first, deliberately: it is the cheapest check on model
completeness, because a model that cannot be described cannot be compiled. Then the contracts, which
is where the specification stops being a document about the system and becomes the system's source of
truth.

### W3.1 `ess-gen` — one trait, three generators

Three crates total, not eleven (F9): `ess-domain`, `ess-compiler`, `ess-gen`.

| projection | what it proves |
|---|---|
| Markdown + Mermaid | the model can be described — every construct has a rendering, or the generator refuses |
| JSON Schema per command input and event payload | the type system is projectable without loss |
| OpenAPI + AsyncAPI | the specification is the contract, not a document beside it |

Every artifact carries provenance (§10): spec version, source digest, compiler version, generator
version. An artifact that cannot say which specification produced it is an artifact nobody can audit.

### W3.2 Generated output is checked, not eyeballed

The failure mode a generator invites is output that looks right. So:

* **OpenAPI and AsyncAPI are validated against their own schemas**, not merely produced.
* **Regeneration is byte-identical**, and CI fails on a diff — the mechanism F8 asked for, applied to
  generated artifacts rather than only to the IR.
* **Every construct in `examples/billing/` appears in every projection that should contain it**,
  asserted per construct. A projection silently dropping unions is the bug this catches.

**Deliverable:** `ess generate --kind docs|schema|openapi|asyncapi`, output committed and drift-checked,
and a test per construct per projection.

---

## ESS wave 4 — the specification as verification oracle

**Goal: a generated conformance suite, and proof that it bites — checked against an implementation
that is deliberately wrong.**

Design phase 5, and the wave where review F10 is load-bearing.

### W4.1 Scenarios generated from the model

The §18 scenario format, derived from the IR: a command's outcomes become cases, `external` outcomes
become injected faults, a view's consistency decides `expect` against `eventually`, a lifecycle's
transitions become a state-transition suite, and a binding becomes a flow test with its `delivery` and
`on_failure` semantics.

### W4.2 Two reference implementations, one of them wrong

A small hand-written billing implementation, and beside it a deliberately **wrong** one: drops the
binding, emits the wrong event, lets a paid invoice be cancelled, answers a view before the projection
catches up.

**The suite must fail the specific check each fault exists to break** — not merely fail. This is the
lesson the protocol's own conformance work cost us: a generated suite that has never failed is a suite
nobody has a reason to trust, and one that fails for the wrong reason is worse, because it looks like
evidence.

### W4.3 The loop closed

The run produces `EvidenceKind::EssConformance` — the type wave 1 added — submitted to the protocol
engine, satisfying a completion predicate on a real task. A specification generates its own tests, an
implementation is checked against them, and the protocol decides whether that is enough.

**Deliverable:** `ess test --generate`, a conformance runner, both implementations, and a matrix
asserting which fault breaks which check.

---

## ESS wave 5 — structural synthesis

**Goal: a generated invoice service and email service that compile, and that pass the suite wave 4
generated.**

Design phase 6, and §38's last success criterion. It is last for a reason: a generator built on a
model that has not survived three projections and a conformance suite generates confident nonsense,
and every wave before this one exists to make that claim falsifiable before the code is written.

### W5.1 Domain types, commands, events, views

Rust from the IR: newtypes that stay distinct from their representations, tagged unions as enums,
lifecycles as state types whose illegal transitions do not compile, commands as traits with an outcome
type per declared outcome.

### W5.2 Component skeletons and one transport adapter

A component's inner domain generated in full; its outer surface generated as a port, with exactly one
transport implemented — the one the billing example needs. Every other transport is a later wave, and
pretending otherwise is how a generator acquires six half-adapters.

### W5.3 The generated code passes the generated tests

The whole point, and the only acceptance criterion that matters: `cargo test` in the generated
workspace runs wave 4's suite against wave 5's code and passes — with the deliberately-wrong
implementation still failing the checks it should.

**Deliverable:** `ess synthesize`, a generated buildable workspace, and CI that regenerates it and
runs the suite.

---

## What is not in these five waves

Behavioural synthesis (§32 phase 7), formal verification, topology generation, `ess diff` compatibility
classification, and every transport beyond the one the billing example needs. Each is a wave of its
own. None is worth starting before generated code has compiled and passed a suite it did not write.
