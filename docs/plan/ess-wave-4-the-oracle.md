# ESS wave 4 — the oracle, and what it found in the specification language

> **Delivered.** Goal from [`ess-roadmap.md`](ess-roadmap.md): a generated conformance suite, and
> proof that it bites — checked against an implementation that is deliberately wrong. A specification
> now synthesises its own suite, a runner executes it against a `ConformanceTarget`, twelve
> deliberately wrong implementations say which scenario catches which fault, and a run's evidence
> decides whether an ADP task may complete. `task check` is green with seven steps: 50 suites, 1216
> tests, 0 clippy warnings, 0 rustdoc warnings. What the wave did *not* deliver is at the bottom of
> this page.

Design phase 5. Waves 1 to 3 produced a model, an IR and four projections — all of them *outputs*.
A projection cannot be wrong about the world, only about the model: a generated OpenAPI document is
whatever the specification says, and no running system contradicts it. This wave is the first thing
that can be contradicted, because it is the first thing that makes a claim about software somebody
else wrote.

## What the wave set out to prove, and what it proved instead

It set out to prove that a specification can act as a verdict on an implementation. It does:
[`examples/billing-conformance/`](../../examples/billing-conformance/) walks the loop in both
directions, and the second direction is the one that counts.

What was not expected is where the value landed. **Before the suite failed a single implementation,
it failed the specification language four times.** The synthesizer's refusals were not a backlog of
unimplemented features; each was a construct the model named but did not say enough about to *test*,
and every one of them had been rendered without complaint by all four projections of wave 3.
Documentation, JSON Schema, OpenAPI and AsyncAPI all publish `PayInvoice`, and not one of them needs
to know *which invoice* it settles, because a document does not have to run.

That is the wave's actual result, and it generalises past this repository: a projection checks that a
model is *complete enough to describe*; only an oracle checks that it is complete enough to *decide*.
The two are not the same bar, and the gap between them is exactly four constructs wide here.

## The four model gaps

| gap | what was missing | what closed it | how it was found |
|---|---|---|---|
| an outcome's subject | a command could not say which entity it changed, so a lifecycle scenario had no entity to arrange | `creates:` / `moves:` / `updates:` on the outcome; a transition no outcome takes is `missing_causation` | **reasoned** — wave 3.5 gate G14, ahead of the synthesizer |
| an observable escalation | `on_failure: escalate` promised "surface it to a person", which no target can be asked to prove | `escalate:` with `emits:` naming a declared event; the bare word is refused | **reasoned** — wave 3.5 gate G2 |
| the instance a command acts on | `PayInvoice` settles *an* invoice, and nothing connected its input to that invoice's identity | `instance:` names the input field carrying the identity; `creates:` points at an emitted event instead | **refused** — 28 scenarios would not generate |
| a wrong-state refusal | a scenario could assert that *something* went wrong, never that the right thing did | `wrong_state: true` with an `error:`, a fourth kind of outcome; the states stay derived from the lifecycle | **refused** — 16 scenarios said the specification declared no answer |

Two of the four were predicted. [`ess-wave-3.5-reconciliation.md`](ess-wave-3.5-reconciliation.md)
exists because independent reviews read the wave 4 design against the code and found two model changes
that would be far cheaper before a synthesizer was built around their absence. They were right, and
the reviews earned their cost — twenty gates, closed before the oracle was allowed to start.

They were also not enough, and that is the part worth keeping. G14 gave an outcome a subject and
stopped one question short of the one a test asks; nobody noticed until synthesis refused twenty-eight
lifecycle scenarios across the two examples and said why. Billing went from five scenarios to nineteen
and from fifteen refusals to one on the day `instance:` landed. No review found that. A machine that
had to *construct a step* found it in one run.

### Why the refusals were right to refuse

The tempting repair in both cases is inference, and both times it is wrong for the same reason.

For `instance:`, "the input field whose type matches the entity's identity" is cheap and has no answer
when a command carries two fields of that type — the oracle fixture has such a pair on purpose — and
no answer when it carries none. Worse, an inferred link silently changes *which scenarios exist* when
someone adds an unrelated field, and stored conformance results are keyed on exactly those names. A
synthesizer that fabricated an id would fail a **correct** implementation, which is worse than
generating nothing: the reference against which everything else is measured would be the thing
reporting a defect.

For `wrong_state:`, the states are deliberately *not* written down. A lifecycle already declares which
states each transition may be taken from, so every other state is wrong by construction, and adding a
`from:` narrows the branch without anyone editing a second list. Writing them out would be a second
copy of a fact the model already holds.

The refusal is a first-class output rather than a silent omission, for the reason
[`ess-wave-3-projections.md`](ess-wave-3-projections.md) gave one wave earlier about gaps on a
generated page: a suite quietly holding fewer checks than it used to is the one failure a passing run
cannot show you. There are twelve refusal causes, `ESS-SYNTH-001` to `ESS-SYNTH-012`, each carrying
the construct, the reason and the hint that says what would have to change — and the committed suite
index lists every construct that got no scenario, so a hole is a line in a diff.

## The suite

Five families, from [`examples/billing/`](../../examples/billing/) and
[`examples/oracle-fixture/`](../../examples/oracle-fixture/):

| family | what it asserts | billing | oracle-fixture |
|---|---:|---:|---:|
| command outcomes | the branch is reached, its event is emitted, and every event the branch does not emit is absent | 8 | 9 |
| lifecycle transitions | the move happens, observed through a view rather than merely reported | 3 | 3 |
| wrong-state refusals | the declared error, in a state the transitions do not start from | 8 | 8 |
| entity invariants | checked after each state-changing command | 4 | 0 |
| bindings | the mapping field by field, the delivery guarantee, the failure policy | 4 | 11 |
| **total** | | **27** | **31** |
| constructs with no scenario | recorded in the index with a reason | 1 | 6 |

Both suites are committed under [`suites/generated/`](../../suites/generated/), regenerated by
`task suite` and drift-checked by `task suite-check` — the seventh step of the gate, and a CI job of
its own, *Conformance suites up to date*. They sit beside the projections rather than inside
`generated/`, because that tree has one owner and a recursive orphan scan that deletes what its owner
did not produce; two writers there would each delete the other's committed contract.

The oracle fixture's zero in the invariant row is the honest kind of zero. All five of its invariant
scenarios are refused as `ESS-SYNTH-011`: the invariants read fields no view of that entity publishes,
so there is nothing a black-box target could be asked to show. The hint says to publish the fields or
to state the invariant over what a view already carries. A fixture built to stress the oracle produced
a refusal the normative example does not — which is what a second fixture was added for.

## Does it bite

[`crates/ess-conformance/src/faulty.rs`](../../crates/ess-conformance/src/faulty.rs) ships twelve
implementations that are wrong in exactly one way each, and
[`tests/faults.rs`](../../crates/ess-conformance/tests/faults.rs) asserts two properties per fault
rather than one:

1. the fault fails **the scenario that exists to catch it**, by name — not "the run went red", which a
   single panic would also achieve;
2. the fault **does not break everything**, held to a per-fault blast-radius allowance that defaults to
   one and has to be raised with a reason.

Eleven of the twelve are caught. Six sit at an allowance of one or two. The widest is `wrong-event`
at 22, and the shape of that 22 is the interesting part: **5 failed and 17 error**, asserted as that
split rather than as a total. `InvoiceCreated` is where a new invoice's identity is published, and
fifteen further scenarios read it to arrange themselves; rename it and they cannot be set up at all.
They come back as *nobody found out*, not as *the implementation contradicted the specification
fifteen more times*. That is the whole reason the status vocabulary has four words rather than two,
and it is why a wide radius here is a fact about scenario dependency rather than about over-reaching
assertions.

A control group runs first: both references pass their own suite in full, 27 of 27 and 31 of 31.
Without it every row below would be readable as "the target is broken somehow", and the matrix would
be measuring the reference rather than the suite.

Two faults are injected inside the implementation rather than at the boundary — a dropped binding and
a swapped mapping — because neither is expressible as a perturbation of what goes in and what comes
out. A test asserts that those two and no others are injected that way, so the exception stays an
argued exception.

### The one fault caught by nothing

`wrong-event-payload` publishes `InvoiceCreated` with an amount nobody submitted. Every field the
event declares is present, and every one is of its declared type: `999` is as well-formed a `Money` as
the amount the caller sent. The check that would catch it is
`InvoiceCreated.amount == CreateInvoice.amount`, and **no construct in the model licenses it**. An
outcome says which events it emits, and never what fills their fields. Asserting the equality would be
a match on a shared field name — the inference this crate refuses everywhere else, and one that would
fail an implementation that names the field differently for a reason.

So it stays recorded as uncaught, with its reason, and the matrix asserts it is *still* uncaught. The
day the gap closes, that row fails and has to be rewritten rather than being quietly forgotten. Two
rows that sat beside it have already moved this way: an event a branch does not declare is now
asserted absent, and a read-your-writes read that could not be demanded is now `unsupported` instead
of quietly weakened.

Closing it is a model change, in the shape `mapping:` already has for a binding: some way for an
outcome to say where a payload field comes from. That is a construct, a resolution rule, a rendering
in every projection and an edit to the normative example — the same cost as `instance:`, and it is the
clearest candidate the wave produced for the next model change.

*(Wave 6.5 closed it, in exactly that shape: a `payload:` declaration on a command outcome, and the
row now designates the scenario that catches it. This section stays as the record of why the gap
existed and what the closure had to cost.)*

What synthesis *can* ask for, it now asks. `partial-event-payload` is the same event with a declared
field missing, and that one is caught, because the type is declared. Presence and shape are a row in
the matrix rather than a sentence in a doc comment.

## The runner refuses to guess, and refuses to sleep

`ConformanceTarget` has nine methods, and every one of them traces to a construct the specification
declares. Seven come straight from design §7; the other two exist because a step in the closed
thirteen-step scenario vocabulary could not otherwise be executed at all, and each is argued on its
own method rather than in a table. There is **no assertion method** — no `assert_the_binding_worked`, no
`tell_me_whether_escalation_happened`. A target reports what it observed and the runner decides
whether the specification is satisfied, because the moment a method answers a question the suite is
supposed to ask, the suite has stopped checking the implementation and started asking it for its
verdict. It follows that a step the model cannot express through declared concepts is a finding about
the model rather than a new method on the trait, and that is the rule that turned all four gaps above
into refusals instead of hooks.

Determinism is structural, not a convention. A `Runner` is constructed with its clock and its id
source and nothing beneath it reaches for an ambient one — no `SystemTime::now`, no RNG. The clock's
`now` takes `&mut self`, because a clock that must advance is a clock that mutates and the type should
say so; a fixed clock handed to a bounded assertion would ask its target forever. Nothing sleeps: a
`Deadline` is an instant in the runner's own clock, not a duration to wait out, and the waiting is the
target's job where §15 puts it. Two identically constructed runners produce byte-identical reports.

`unsupported` is a fourth status beside `passed`, `failed` and `error`, and a required scenario that
ends in it fails the run. A skip that reads as a pass is how a suite comes to certify what it never
checked — and the degradation is real, not hypothetical: the `Untraced` reference is a legitimate
implementation that cannot say which command a binding invoked, and a run against it reports
`unsupported` on exactly the mapping scenario and still fails.

## The loop, closed in both directions

`protocol ess conform synthesize | run | evidence`. `run` exits `0` conformant, `1` contradicted or
unable to expose something required, and `3` when the run could not be carried out at all — because
telling a harness the system is wrong when nobody found out is its own kind of lie.

`evidence` is the handoff. It mints the record in the same process that executed the suite, and takes
no argument naming who produced it, so there is no call site at which a caller can describe itself as
the verifier. A verb that converted a saved `--report report.json` into evidence would let the caller
author the record, which is the forbidden shape with a JSON file in the middle of it.

Both directions are tested, and the second is what makes the first mean anything: a correct
implementation passes 27 of 27, the record satisfies `ess-conformance`, and the task advances to
`complete`; the same implementation with `accept-invalid-amount` injected fails the one scenario that
exists to catch it, produces a record saying so, and the task stops at review naming
`ess_conformance.passed = false`, `ess_conformance.scenarios.failed = 1` and the principle that
refused. Withholding the record entirely is a third, different refusal, and it is tested too. The
shipped principle was not touched to make any of this work.

### What `independent: true` does and does not mean

It means the producer is not the agent under review, checked by one comparison, and stamped by the
crate that ran the suite rather than by anything the caller can set. That is worth having and it is
all it is worth.

It is **structural, not attested**. Nothing signs the record, no key exists anywhere in this
workspace, and a person can type the word. [`docs/VISION.md`](../VISION.md) already names this as a
gap rather than a horizon, and the loop this wave closed does not narrow it — it raises the stakes,
because there is now a machine verdict resting on that one declaration. The provenance digest is left
empty rather than filled with something that looks like tamper evidence, which is the right call: a
fake one would be worse than none.

## What shipped, against what this page's goal asked for

| asked for | what shipped |
|---|---|
| scenarios generated from the model — outcomes, external faults, view consistency, transitions, bindings | all five families, 27 + 31 scenarios, committed and drift-checked |
| two reference implementations, one of them wrong | two references and **twelve** faults, not one wrong implementation |
| the suite fails the specific check each fault exists to break | asserted by name for eleven of twelve, with a blast-radius allowance per fault |
| the run produces `EvidenceKind::EssConformance` and closes a real ADP task | `protocol ess conform evidence`, and `examples/billing-conformance/` in both directions |
| `ess test --generate` | spelled `protocol ess conform synthesize`, beside `run` and `evidence` — three verbs because the report and the evidence record are different documents |
| a matrix asserting which fault breaks which check | `tests/faults.rs`, machine-derived from the fault list so a new fault cannot be left out of the table |

The design's four open decisions were all taken at their stated defaults, which is worth recording
because a default that was never chosen and a default that was chosen look identical six weeks later:
D1 the runner is its own crate, D2 the runner is synchronous, D3 the target waits and the runner
passes a deadline, D4 `synthesizer_version` sits beside `generator_version` in the suite's provenance.

## What is still open

**An out-of-process implementation cannot be run.** `ConformanceTarget` is a Rust trait, and the
binary can only reach an implementation it was compiled with. Nothing here speaks to a target over a
socket. `protocol ess conform run` says this outright in its own help rather than implying more is
there, and gives the four-line adapter recipe: depend on `ess-conformance`, implement the trait, read
the committed `suites/generated/<system>/suite.json` and call `Runner::for_suite(&suite).run(...)`.
The suite document is the same either way, and it holds no handle into any particular compilation, so
a runner in another language can read it. That is the half of the portability claim that holds
today; a transport-level target is not.

**A payload field's value is unassertable**, and the construct that would fix it is described above.

**A value object's invariants get no scenario.** `billing.invoice.Money` says `amount >= 0` of every
`Money` in the system — a claim about a *type* rather than about an instance at rest — and rebasing it
onto every entity field that reaches that type is a walk this slice does not do. It is the normative
example's single refusal, `ESS-SYNTH-006`, and unlike the others it is a gap in this crate rather than
in the model. It is recorded as a refusal anyway, because a reader of a suite cannot otherwise tell an
unimplemented slice from a specification with nothing to check.

**`on_failure: drop` generates a refusal, not a scenario**, and that stays correct. A policy that
gives up silently publishes nothing, so there is nothing to assert; the hint says to write `escalate:`
if the failure has to be provable. The refusal is the honest output — a scenario would have to invent
an observation the specification declines to make.

**Attestation is still a gap**, unchanged by this wave and now load-bearing in one more place.

**One number in the crate's own prose is stale.** `crates/ess-conformance/src/lib.rs` and
`examples/billing-conformance/README.md` both say *eleven* deliberate faults where `Fault::ALL` holds
twelve and a test asserts the length. The count moved when `wrong-refusal-error` was added and two
sentences did not follow it. Trivial, and recorded here rather than fixed silently, because this
repository's standing complaint about itself is documents that drift behind the code they describe.

## Not in this wave

Deep predicate solving and a constraint solver; witness synthesis by property-based generation, which
[`ess-wave-3.5-reconciliation.md`](ess-wave-3.5-reconciliation.md) sequences after the model changes
rather than beside them; transport-level conformance targets; and anything about a specification
*changing* — what a revision invalidates is the next wave's subject, and it is next precisely because
gate G19 made conformance evidence fail closed when the model moves, which is correct and blunt in
equal measure.
