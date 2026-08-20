# Review: ESS Implementor Design v0.1

Reviewing [`ess-implementor-design-v0.1.md`](ess-implementor-design-v0.1.md) against what
`engineering-protocols` has already learned building the same shape of thing twice.

**Verdict: the architecture is right and the scope is too wide.** The core rule — semantic concepts
are primary, transports are projections — is the correct spine, and §36's invariants are the right
ones. Three modelling gaps will produce generated tests that assert false things, and one of them
(§2) is a hole in the very join between ESS and AEP. Recommended cut at the end.

## What is right, and worth not relitigating

* **Semantic-first with transports as projections** (§3, §6). This is what lets one model compile to
  a monolith or to services, and it is the reason OpenAPI must not be authoritative.
* **The validation pipeline** (§19) is exactly the shape that worked here: parse → raw → resolve →
  validate → IR, with generators forbidden from touching raw YAML. Our equivalent rule — validated
  types do not implement `Deserialize` — is what keeps it honest; ESS should adopt it verbatim.
* **Structured diagnostics as agent feedback** (§29). Underrated. A deterministic diagnostic is a
  repair instruction; prose is a guess.
* **Synthesis levels** (§11) with 0–3 first. Correct, and the discipline to stop at 3 is the hard
  part.
* **Generated code is disposable and regenerable** (§15). The moment generated files are hand-patched
  the model stops being the source of truth.

---

## Findings

### F1 — A command has one outcome, and real commands have several *(blocking for Level 2)*

§4.4 gives a command `preconditions` and `emits`. §13 then generates `CreateInvoice → expect
InvoiceCreated`. §17 promises conformance validates *"rejected commands"* and *"specified error
behavior"* — neither of which the model can express.

Any command with a precondition has at least two outcomes, and the generated test asserts only one.
Worse, it asserts the happy one, so a suite full of green tests says nothing about the branch where
the money does not move.

**Refinement.** Make outcome the unit `emits` hangs off:

```yaml
command: CreateInvoice
input:
  customer_email: Email
  amount: Money

outcomes:
  accepted:
    when: amount.amount > 0
    emits: [InvoiceCreated]
  rejected:
    when: amount.amount <= 0
    error: InvalidAmount
```

with a declared error vocabulary per domain. Test synthesis then derives one scenario per outcome,
and a command whose outcomes do not cover its input space is a validation error rather than a
surprise in production.

### F2 — Views have no consistency semantics, so generated assertions race *(blocking for Level 2)*

§4.6 calls views "stable observables"; §18's scenario asserts a view immediately after the command
that caused it. If the view is a projection — the normal case — that assertion passes on a laptop and
flakes in CI, and the fix everyone reaches for is a sleep.

We solved this exact problem in the protocol half and it is worth copying rather than rediscovering:
every accepted mutation returns an opaque **consistency token**, and a read may demand a view no older
than it. Immediately-consistent implementations satisfy it for free; projected ones block.

**Refinement.** Declare a view's consistency in the model:

```yaml
view: InvoiceById
consistency: read_your_writes    # or: eventual
```

and generate `expect` for the first and `eventually` for the second — with the scenario format
carrying the token, never a delay. §18's `eventually:` block already hints at this; make it a
property of the view rather than a choice the scenario author makes per assertion.

### F3 — Bindings have no failure semantics *(blocking for Level 2, and for honesty)*

§7 gives `on event → invoke command` and a transport of `async` or `local`. §13 generates
"Eventually: SendEmail handled". Nothing says what happens when it is not: no delivery guarantee, no
retry, no dead-letter, no compensation. A binding that can fail silently is the difference between a
specification of a system and a specification of a demo.

**Refinement.** Require a delivery statement per binding, even if v0.1 supports exactly one value:

```yaml
bindings:
  - id: notify-on-invoice-created
    when: {event: invoice.InvoiceCreated}
    invoke: {command: email.SendEmail}
    delivery: at_least_once        # the only value v0.1 accepts
    on_failure: escalate           # retry | escalate | drop — `drop` must be written, never defaulted
```

Requiring the word is the point: `drop` should be something an author typed, not something they got.

### F4 — Invariants are prose, so invariant 6 is unenforceable

§4.7 writes invariants as strings (`Paid cannot transition to Cancelled`), while §36.6 requires that
"invariants reference valid model fields". A string cannot be checked against the model, so the
invariant about invariants cannot hold.

**Refinement.** Reuse the predicate language that already exists here — `aep_domain::predicate` — with
paths resolved against the ESS type registry instead of the fact store. It is a typed expression
language with three-valued evaluation, a compact and a structured form, and minimal-cause
explanations, and it took a while to get right. Transition legality (`Paid cannot transition to
Cancelled`) is not a predicate at all: it is already expressible as the absence of a transition, and
should be validated structurally rather than stated twice.

### F5 — Identity is deferred, and that is how renames become breaking changes

§22 says to "eventually distinguish identity / display name / wire name". Deferring it is
retrospectively expensive: every generated artifact, every conformance scenario and every stored
result will have baked in whichever name existed first.

**Refinement.** Split at v0.1, exactly as the protocol half does: an opaque stable identity, a logical
locator, and a wire name that may change without changing either. Also reconcile the two locator
schemes — §2 writes `ess://billing/v3` while AEP uses `ep://org/space/kind/key`. Two schemes for one
idea is a translation table somebody maintains forever; pick one.

### F6 — The join to AEP is described but not buildable *(one hour of work, high value)*

§2 says ESS "should be represented in AEP as a first-class artifact" as
`ArtifactKind::ExecutableSystemSpecification`, and §35 wants `Evidence<EssConformanceResult>` with a
completion predicate `ess_conformance.status == passed`. None of the three exists on our side, so the
loop the vision describes cannot currently be closed even by hand.

**Refinement — concrete, and independent of the rest of ESS:**

| Change | Where |
|---|---|
| `ArtifactKind::ExecutableSystemSpecification` (`executable-system-specification`) | `aep-domain/src/artifact.rs` |
| `EvidenceKind::EssConformance` with fields: spec version, implementation identity, suite version, compiler version, generator version, status | `aep-domain/src/evidence.rs` |
| fact projection `ess_conformance.{status,passed,spec_version}` | same |
| `'ess_conformance.**'` in `protocols/adp/1.yaml` observables | document |
| an `ess-conformance` principle requiring that evidence, `independent: true` | `principles/verification/` |

Doing this first makes the vision testable before the compiler exists: a task can already require ESS
conformance evidence and refuse to complete without it, even while a human produces that evidence by
hand.

### F7 — The reference example contradicts the model it is meant to exercise

§4.7 defines `Invoice` states as `Draft / Issued / Paid / Cancelled` with named transitions. §31's
example — the one Phase 1 must "parse + validate" — declares `state: [created, paid]`, and §18's
conformance scenario asserts `status: created`. Three spellings of one machine.

**Refinement.** Make §31 the normative fixture and derive every other snippet from it, mechanically if
possible. We had precisely this drift between a design document's examples and the shipped documents,
and the thing that fixed it was a test that loads the real fixture rather than a copy.

### F8 — Determinism is asserted without a mechanism

§27 requires generation to be deterministic given the same inputs, and §36.9 makes it an invariant.
Nothing says how, and the three things that break it are always the same: unordered maps, timestamps,
and formatter drift.

**Refinement.** Adopt the rules that already hold here, explicitly: `BTreeMap`/`BTreeSet` only, no
clock or RNG anywhere in the compiler, canonical serialisation with a trailing newline, and a CI job
that regenerates every artifact and fails on any diff. The last one is what makes the first three
true rather than aspirational.

### F9 — Eleven crates before one of them has a user

§26 lists eleven ESS crates. We started with six for the protocol and several sat empty through two
waves, which cost review attention every time someone opened the tree.

**Refinement.** Start with three — `ess-domain` (model, raw types, validation), `ess-compiler`
(resolution, IR, diagnostics), `ess-gen` (every generator behind one trait) — and split when a
boundary has been argued about twice.

### F10 — Test synthesis lands before anything can run the tests

Phase 5 generates contract, integration and E2E tests; Phase 6 generates the code they run against.
Between them, the suite's own correctness is unfalsifiable.

**Refinement.** Hand-write a small reference billing implementation *before* Phase 5, and — the lesson
that most improved the protocol's conformance work — ship a deliberately **wrong** one beside it. A
generated suite that has never failed is a suite nobody has any reason to trust. Assert per property
that the wrong implementation fails the check that exists to catch it.

### F11 — Union semantics unspecified

§21 lists `Union` among composite forms without saying whether it is tagged. Untagged unions do not
round-trip through JSON Schema, OpenAPI or Serde without ambiguity.

**Refinement.** Tagged only, with the tag field named in the model. If untagged is wanted later it can
be added as a distinct form with its own name and its own warnings.

---

## Recommended v0.1 scope

Narrower than §38, and ordered so each step is falsifiable by the one before it:

1. **F6 first** — the AEP-side artifact kind, evidence kind and principle. Independent of everything
   else, closes the loop in the vision, and is a day's work.
2. **Core model + validation** (§32 Phase 1–2) with F1, F3, F5 and F11 folded in from the start —
   they are cheap now and structural later.
3. **The billing fixture** as the single normative example (F7), parsed by a test in the repository.
4. **Docs generation** (Phase 3), because it is the cheapest possible check on model completeness: a
   model that cannot be described cannot be compiled.
5. **OpenAPI/AsyncAPI** (Phase 4).
6. **A hand-written reference implementation, plus a deliberately wrong one** (F10).
7. **Test synthesis** (Phase 5), proven against both.

Levels 4 and 5 — behavioural and verified synthesis — stay out of v0.1. So does topology generation:
§8 is already marked "intentionally narrow", and the narrowest useful version is to model it and
generate nothing.

## What I would not change

The non-goals list (§37) is unusually good and should survive contact with enthusiasm. In particular
"do not make OpenAPI or AsyncAPI authoritative" and "do not mandate microservices, CQRS or event
sourcing" are the two that a project like this loses first, and losing either turns a specification
language into a framework.
