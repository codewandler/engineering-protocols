# ESS roadmap — waves 1 to 6

> **Waves 1 to 5 delivered; 6 in progress; 6.5 scheduled behind it.** Numbering restarts: these are
> ESS waves, not a continuation of the protocol's. Tags are `0.3.0-ess-wave-1`, `0.3.1-ess-wave-2`,
> `0.3.2-ess-wave-3`, `0.3.3-ess-wave-3.5`, `0.4.0-ess-wave-4` and `0.5.0-ess-wave-5`, keeping the
> convention that a tag is named after its changelog heading.
>
> Waves 1 to 4 are design §32's seven phases, with 3 and 4 merged (both are projections of the same
> IR, and separating them would mean shipping documentation nothing checks) and 7 left out (behavioural
> synthesis is not worth attempting before structural synthesis has produced something that builds).
> Wave 5 is not from §32 at all — it comes from a design filed later, and it moved ahead of structural
> synthesis by decision because wave 4 left everything derived from a single revision with no way to
> say what a change invalidates:
>
> | wave | design phase |
> |---|---|
> | 1 | 1 — core domain |
> | 2 | 2 — compiler |
> | 3 | 3 + 4 — documentation, OpenAPI, AsyncAPI |
> | 4 | 5 — test synthesis |
> | 5 | — semantic diff, from the later design |
> | 6 | 6 — structural synthesis, Rust first |

Scope and ordering follow [`ess-review-v0.1.md`](../design/ess-review-v0.1.md): the review's findings
are folded into the waves that would otherwise have to unlearn them. Structural synthesis
(design §32 phase 6) is **wave 6 of this roadmap**, and it is last on purpose: it starts only once the
model has survived three projections *and* wave 4's oracle has proven it can fail a wrong
implementation. An earlier version of this paragraph said structural synthesis was "deliberately
outside this roadmap — wave 4 at the earliest", which contradicted the wave 5 section below it and
used the old phase numbering; the sequencing is now stated once, here.

The ordering rule throughout: **each wave must be falsifiable by the one before it.** A generated
artifact nothing can check is a claim, not a deliverable.

Its corollary, learned the hard way in wave 3 and worth writing down: **never call a wave complete
under an acceptance criterion its own tests say is unmet.** When the delivered thing is narrower than
the criterion, amend the criterion to what was delivered and record the remainder as deferred with the
decision that deferred it — visibly, in this file. A criterion quietly reinterpreted is a criterion
nobody can hold the work to, and it is the one failure mode a roadmap is supposed to prevent.

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

> **Delivered**, tagged `0.3.1-ess-wave-2`. See [`ess-wave-2-the-compiler.md`](ess-wave-2-the-compiler.md).

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

> **Delivered**, tagged `0.3.2-ess-wave-3`. See [`ess-wave-3-projections.md`](ess-wave-3-projections.md).
> W3.2's first criterion was amended on delivery; the amendment and what it defers are recorded below.

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

* **Every schema an OpenAPI or AsyncAPI document embeds is validated against the JSON Schema
  2020-12 meta-schema** by a conforming validator — `jsonschema` as an `ess-gen` dev-dependency, with
  `default-features = false` so no test can retrieve a document over the network. The **envelope**
  around those schemas is checked structurally: every required key, in the dialect the document
  declares.
* **Regeneration is byte-identical**, and CI fails on a diff — the mechanism F8 asked for, applied to
  generated artifacts rather than only to the IR.
* **Every construct in `examples/billing/` appears in every projection that should contain it**,
  asserted per construct. A projection silently dropping unions is the bug this catches.

**Deliverable:** `ess generate --kind docs|schema|openapi|asyncapi`, output committed and drift-checked,
and a test per construct per projection.

#### W3.2's first criterion, amended — and what is deferred

That criterion originally read: *"OpenAPI and AsyncAPI are validated against their own schemas, not
merely produced."* That is not what shipped, and the projection tests say so in writing
(`crates/ess-gen/tests/openapi.rs:14`, `crates/ess-gen/tests/schema.rs:277`). Rather than leave the
roadmap as the last place still claiming an unmet gate, the criterion above states what is delivered:
embedded schemas validated for real, envelopes checked structurally.

**Deferred:** validating the OpenAPI 3.1 and AsyncAPI 3.0 *envelopes* against their own meta-schemas.

**The open decision it waits on is vendoring.** Neither meta-schema is in this repository. Holding
them means taking on two third-party documents of roughly 100 KB, each with its own licence, each
needing a pinned version and an update path, inside a repository whose entire discipline is that
committed artifacts are derived from sources it owns. That was weighed while wave 3 was being built
and **declined for now**; nothing since has changed the trade. Reopen it by recording a decision here
— not by adding the files.

---

## ESS wave 4 — the specification as verification oracle

> **Proposed, and gated.** Design:
> [`ess-closed-loop-execution-conformance-design-v0.1.md`](../design/ess-closed-loop-execution-conformance-design-v0.1.md),
> reviewed and reconciled against the code, and frozen for implementation except the open decisions it
> names. Wave 4 does not start until the blocking gates in
> [`ess-wave-3.5-reconciliation.md`](ess-wave-3.5-reconciliation.md) are closed — several of them are
> model changes that are cheaper before a synthesizer is built around their absence.

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

## ESS wave 5 — what changed, and what that invalidates

> **Proposed, and next.** Plan: [`ess-wave-5-semantic-diff.md`](ess-wave-5-semantic-diff.md). Design:
> [`ess-semantic-diff-impact-evolution-design-v0.1.md`](../design/ess-semantic-diff-impact-evolution-design-v0.1.md),
> reviewed in [`2026-08-20-semantic-diff-feasibility-review.md`](../reviews/2026-08-20-semantic-diff-feasibility-review.md).

**Goal: two compiled specifications, and a typed answer to what moved between them.**

Sequenced ahead of structural synthesis by decision, and the argument is what wave 4 left behind.
Everything this repository generates is derived from **one revision**, and gate G19 made conformance
evidence fail closed when the model moves — so today any change to a specification, including one to a
comment in an unrelated domain, sends every conformance requirement back to owed. Correct, and blunt.
A semantic delta is what makes it proportionate. Structural synthesis would instead generate *more*
artifacts from one revision, multiplying the thing that has no change story.

Note this is a **vision amendment** as well as a wave: `docs/VISION.md` describes specifying a system
once and compiling from that description, and says nothing about a system changing over time. The
first slice, its exclusions, and the decision itself are on the plan page.

## ESS wave 6 — structural synthesis

> **Accepted for implementation, 2026-08-20** — the gate it waited on is met, both halves: a correct
> target passes 27 of 27 and completes the task, a faulty one is refused by name (`0.4.0-ess-wave-4`).
> Design:
> [`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](../design/ess-structural-synthesis-obligations-realizations-design-v0.1.md);
> decisions and slices on the plan page,
> [`ess-wave-6-structural-synthesis.md`](ess-wave-6-structural-synthesis.md).

**Goal: a generated invoice service and email service that compile, and that pass the suite wave 4
generated.**

Design phase 6, and §38's last success criterion. It is last for a reason: a generator built on a
model that has not survived three projections and a conformance suite generates confident nonsense,
and every wave before this one exists to make that claim falsifiable before the code is written.

### W6.1 Domain types, commands, events, views

Rust from the IR: newtypes that stay distinct from their representations, tagged unions as enums,
lifecycles as state types whose illegal transitions do not compile, commands as traits with an outcome
type per declared outcome.

### W6.2 Component skeletons and one transport adapter

A component's inner domain generated in full; its outer surface generated as a port, with exactly one
transport implemented — the one the billing example needs. Every other transport is a later wave, and
pretending otherwise is how a generator acquires six half-adapters.

### W6.3 The generated code passes the generated tests

The whole point, and the only acceptance criterion that matters: `cargo test` in the generated
workspace runs wave 4's suite against wave 6's code and passes — with the deliberately-wrong
implementation still failing the checks it should.

**Deliverable:** `ess synthesize`, a generated buildable workspace, and CI that regenerates it and
runs the suite.

---

## ESS wave 6.5 — hardening, the register emptied

> **Scheduled, 2026-08-20.** Starts when wave 6 tags. Contents and evidence, one row per gap:
> [`gap-register.md`](gap-register.md) § "Open, owned by the post-wave-6 hardening batch". This is
> the reconciliation shape wave 3.5 had: no new capability, the existing claims made mechanical.

In order: the three invariants enforced by nothing become enforced (7 by a source scan over the
engine, 8 by extending the banned-token scan to every crate that states the property, 14 by a
write-surface enumeration test); the model digest widens to full SHA-256 with one regeneration
commit (register D-4); the input→event-payload model gap closes, so the one fault caught by nothing
gets a scenario that catches it; value-object invariant scenarios synthesise, as the refusal text
already promises; and `proptest` phase 1 lands with the model changes it was queued behind.

## ESS wave 7 — the loop closed over generated code

> **Scheduled, 2026-08-20, by operator instruction: run 7.1 through 7.4, then stop.** Re-scoped
> since: 7.3 acquired a browser continuation, 7.4 stays deferred, and **7.5 is the dual-target
> demonstration**, scheduled and delivered by operator instruction on 2026-08-21. Attested evidence
> — what an earlier draft of this page called 7.5 — is not scheduled at all: it lives on the gap
> register as D-3, it adds a dependency class, and that acceptance is Timo's, not a wave's.

### W7.1 The diff learns about generated artifacts

> **Delivered.** See [`ess-wave-7-closing-the-loop.md`](ess-wave-7-closing-the-loop.md); W7.3 and
> W7.4 are deferred there by operator decision.

`contract_digest` becomes code: generated artifacts — projections, suites, synthesised workspaces —
carry the digest of the model slice they derive from, and `ess impact` narrows "the specification
moved, the whole workspace is owed" to the artifacts whose slice moved. Same polarity as everything:
narrow, never survive. This closes the loop wave 6 opens, by the same argument that put the diff
ahead of synthesis.

### W7.2 Entities and commands join the delta

Register D-1 executed: predicate comparison as conservative canonical equality. Equal says nothing;
different says *changed* with no direction; implication stays refused. Entities, commands, views and
bindings enter the six-family comparison, which stops being six.

### W7.3 A second emitter, and the seam proves itself

> **Delivered.** See [`ess-wave-7-closing-the-loop.md`](ess-wave-7-closing-the-loop.md) § W7.3:
> `generated/go/` is committed and gated (`gofmt -l`, `go build`, `go vet`), the plan's two
> renderings are byte-identical in both trees, and what Go holds more weakly or cannot represent
> at all is in each module's `TARGET.md`. A third emitter followed under § W7.3b — the browser,
> which is not a language at all. The dual-target *demonstration* the paragraph below anticipated
> was **not** part of either, and became the slice of its own it deserved: W7.5, delivered.
> W7.4 stays deferred.

**Go, by decision** — reversible, and chosen *because* it lacks sum types: every outcome enum and
tagged union forces the emitter to either encode honestly or refuse at the target stage, which is
the first real test of "a target-specific refusal never masquerades as unsynthesizable". The plan
does not change; that is the whole point of the seam, and W7.3 fails if it has to.

### W7.5 One specification, two applications, one surface

> **Delivered.** See [`ess-wave-7-closing-the-loop.md`](ess-wave-7-closing-the-loop.md) § W7.5.
> `examples/gatepass/` is the application, and the model gained one word to make its transport
> derivable: a component may say `reached_by: network`, which states where its callers are and
> names no protocol. What follows is a derivation rather than a choice — this repository projects
> exactly one contract for a command surface, the `OpenAPI` document, and that is an HTTP
> contract. Both synthesised applications serve exactly the routes it declares, publish it and the
> committed documentation at `/openapi.json` and `/docs`, and write the same startup record outside
> a `runtime` member that is each process's own. `cargo xtask synth` starts both on ephemeral ports
> and compares all of it.

### W7.4 Obligations become artifacts, and tasks

The deferred half of the synthesis design's §13–§23, now safe to take: an obligation from the plan
becomes an AEP artifact with identity and a contract digest (which W7.1 provides), and an ADP task
can require its resolution the way it already requires conformance. Invalidation reuses
revision-bound approval (`ReviewResult::covers`) rather than mirroring it — review finding 9. §28
stays refused; obligations never grant capabilities.

## What is not in these six waves

Behavioural synthesis (§32 phase 7), formal verification, topology generation, the compatibility
classification `ess diff` will eventually carry, and every transport beyond the one the billing example needs. Each is a wave of its
own. None is worth starting before generated code has compiled and passed a suite it did not write.

### The two proposals that arrived after these waves were written

Both were filed as designs on 2026-08-20 and both have now been reviewed against the code. Neither is
accepted. What follows is what the reviews concluded, so the next reader does not have to open them to
learn the answer.

**Semantic diff** ([design](../design/ess-semantic-diff-impact-evolution-design-v0.1.md),
[review](../reviews/2026-08-20-semantic-diff-feasibility-review.md)) — buildable, and sequenced after
wave 4 by decision. It is four waves, not one: its own smallest useful slice is an `EssDelta` over six
construct families with `protocol ess diff`, which is already the size of a delivered wave. Two of its
seventy-eight sections — an LLM proposal-evaluation loop and multi-candidate architecture search — are a
different product, and collide with `docs/VISION.md`'s "Not an LLM orchestration framework"; the
recommendation is to cut them rather than sequence them.

Two of its findings were acted on immediately, because they cost a sentence now and a regeneration
later, and both are folded into the wave 4 design: a `ScenarioId` is a semantic name and never a
counter, and a scenario's `source` is the set of constructs it *depends on* rather than the one that
spawned it. A third is why gate G19 exists: this design has evidence failing **open**, where the
repository fails closed, and it needs G19 as a precondition rather than replacing it.

**Infrastructure** ([design](../design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md),
[review](../reviews/2026-08-20-infrastructure-design-feasibility-review.md)) — the *design* is deferred
whole, and three waves have since shipped from plan pages that took none of it as a work order. It is
roughly eleven waves at this repository's measured rate, sequenced behind four that do not exist yet. It
would also put cloud discovery adapters inside this workspace, making live API calls under a credential,
which `docs/VISION.md` refuses — and the design argues against itself on that point in two of its own
sections, so the fix is to move the design rather than the boundary. The scanner therefore lives in its
own repository (`infra-scout`), holds the credentials, and nothing in this workspace reaches a network.

What has shipped, as a second instance of the ESS pattern rather than as that design:

| wave | plan page | delivers |
|---|---|---|
| IW1 | [`infra-wave-1-observe.md`](infra-wave-1-observe.md) | `infra-domain` and `infra-compiler`: a scanned bundle validated, refused with stable `INFRA-*` codes, compiled into the content-addressed `infra-ir/1` |
| IW2 (+2.5) | [`infra-wave-2-analyze.md`](infra-wave-2-analyze.md) | `infra-analyze`: the typed dependency graph, twenty `INFRA-DIAG-*` rules, workload properties, invariant candidates, directions, the HTML view |
| IW3 | [`infra-wave-3-desired-state.md`](infra-wave-3-desired-state.md) | `infra-spec`: an authored desired state, its three-valued simulation against a snapshot, and typed drift between two snapshots |
| IW4 | [`infra-wave-4-project-back.md`](infra-wave-4-project-back.md) | `infra-project`: every gap projected back as a reviewable diff — per-object merge and strategic patches, generated manifests, and an obligation for every gap whose value is a decision |

**IW4 delivered ends the scheduled infrastructure waves.** IW5 — *propose* and *apply*, the verbs
that consult a registry and reach a cluster — stays unscheduled and adapter-side: both need a
network and a credential, which is the boundary `docs/VISION.md` draws and the reason the scanner
lives in its own repository. What this workspace ships is the whole non-acting half: observe,
analyze, measure against a declared state, and hand back a diff a person applies with their own
hands.

The two ideas the review harvested are both honoured in IW3 rather than in the design's own shape.
Its §2.7 rule — "a fact that cannot be established is not a pass" — is invariant 5, and `simulate`
makes it structural: an `Outcome` is `Holds`, `Gap(what would have to change)` or
`Undecidable(why)`, with no fourth shape and no boolean to collapse into. Its freshness model,
stated as `now() − t < D`, is refused outright: no expectation compares a timestamp, there is no
duration vocabulary to write one in, and a banned-token test in `infra-spec` keeps it that way
(review finding I7).

Worth taking from it if the vocabulary ever grows: its required / permitted / observed split is a
better answer than `Truth::Unknown` for *"we looked and saw nothing"*. IW4 did not need it — a
projection reads gaps, and a gap is already the `False` arm — so it stays an idea rather than a
debt.
And its freshness model is stated as `now() − t < D`, which this repository cannot express — the domain
is clock-free, nothing here compares two timestamps, and the existing `FreshnessPolicy` variants are all
causal. `BoundToDependencySet` already exists, already means what both designs need, and currently has
no readers at all.
