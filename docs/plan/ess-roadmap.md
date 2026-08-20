# ESS roadmap — waves 1 to 3

> **Wave 1 delivered; waves 2 and 3 proposed.** Numbering restarts: these are ESS waves, not a
> continuation of the protocol's. Tags are `0.3.0-ess-wave-1` and so on, keeping the convention that
> a tag is named after its changelog heading.

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

**Goal: one model, four projections, and a conformance suite proven against an implementation that is
deliberately wrong.**

### W3.1 `ess-gen` — one trait, four generators

Documentation (Markdown plus Mermaid diagrams) first, because it is the cheapest check on model
completeness: a model that cannot be described cannot be compiled. Then JSON Schema, OpenAPI,
AsyncAPI. Every artifact carries provenance — spec version, source digest, compiler and generator
versions (§10).

Three crates total, not eleven (F9): `ess-domain`, `ess-compiler`, `ess-gen`.

### W3.2 Two reference implementations, one of them wrong

A small hand-written billing implementation, and beside it a deliberately **wrong** one (F10): drops
the binding, emits the wrong event, lets a paid invoice be cancelled, answers a view before the
projection catches up. The generated suite must fail the specific check each fault exists to break.

This is the lesson wave 3 of the protocol cost us: a generated suite that has never failed is one
nobody has a reason to trust.

### W3.3 Conformance, and the loop closed

The §18 scenario format, generated from the model, executed against both implementations. The result
becomes `EvidenceKind::EssConformance` — the type added in wave 1 — submitted to the protocol engine,
satisfying a completion predicate on a real task.

**Deliverable:** design §38's success criteria except Rust synthesis, and the vision demonstrated end
to end: a specification generates its own contracts and its own tests, an implementation is checked
against them, and the protocol decides whether that is enough.

---

## What is not in these three waves

Rust structural synthesis, behavioural synthesis, formal verification, topology generation, `ess diff`
compatibility classification, and every transport beyond the one the billing example needs. Each is a
wave of its own, and none of them is worth starting before the model has survived being projected
three different ways.
