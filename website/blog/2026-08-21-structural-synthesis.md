---
title: "0.6 — the code that was never yours to write"
description: >
  Release 0.6.0-ess-wave-6 adds one verb: protocol ess synthesize. The specification that already
  writes its documentation, its contracts and its tests now writes the structural part of the code —
  and the generated system passes the suite the specification generated before this code existed.
slug: structural-synthesis
tags: [release, ess]
---

Release `0.6.0-ess-wave-6` adds one verb to the CLI. `protocol ess synthesize` turns a compiled
specification into a plan and a Rust workspace: the types, the states, the ports — the part of an
implementation that was never anyone's to write — plus a typed list of exactly what remains. The
specification that already writes its documentation, its contracts and its tests now writes code,
and the claim that matters is executed rather than asserted: the generated system passes the
conformance suite the specification generated two waves earlier, before any of this code existed.

This post is a tutorial, run against the normative example that ships in the repository, so every
output below is reproducible — and checked by tests, not pasted from memory.

{/* truncate */}

## One command, one plan, one workspace

```console
protocol ess synthesize --path examples/billing --out billing
```

```text
billing v3 — 45 capabilities: 33 generated, 8 obligation(s), 4 refused, model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  obligation: command behaviour `billing.email.SendEmail` — decided outside the system: the provider rejects the recipient address
  obligation: command behaviour `billing.invoice.CancelInvoice` — the contract is declared; the algorithm is not
  obligation: command behaviour `billing.invoice.CreateInvoice` — the contract is declared; the algorithm is not
  obligation: command behaviour `billing.invoice.IssueInvoice` — the contract is declared; the algorithm is not
  obligation: command behaviour `billing.invoice.PayInvoice` — the contract is declared; the algorithm is not
  obligation: view query `billing.invoice.InvoiceById` — how the projection is kept current is a storage decision
  obligation: view query `billing.invoice.OutstandingInvoices` — how the projection is kept current is a storage decision
  refused: actor grants `billing.invoice.Auditor` — a grant is checked against a caller identity, which types do not carry
  refused: actor grants `billing.invoice.Customer` — a grant is checked against a caller identity, which types do not carry
  obligation: binding escalation `notify-on-invoice-created` — the contract is declared; the algorithm is not
  refused: workload `email-service` — topology synthesis is deferred with its design
  refused: workload `invoice-service` — topology synthesis is deferred with its design
  Cargo.toml — 339 byte(s)
  PLAN.md — 6033 byte(s)
  crates/billing-system/Cargo.toml — 518 byte(s)
  crates/billing-system/src/lib.rs — 11338 byte(s)
  crates/billing-types/Cargo.toml — 367 byte(s)
  crates/billing-types/src/email.rs — 5106 byte(s)
  crates/billing-types/src/invoice.rs — 26524 byte(s)
  crates/billing-types/src/lib.rs — 1248 byte(s)
  crates/billing-types/src/obligation.rs — 1368 byte(s)
  crates/billing-types/src/primitives.rs — 1783 byte(s)
  crates/email-service/Cargo.toml — 435 byte(s)
  crates/email-service/src/lib.rs — 2986 byte(s)
  crates/invoice-service/Cargo.toml — 439 byte(s)
  crates/invoice-service/src/lib.rs — 6928 byte(s)
  plan.json — 11048 byte(s)
written to billing
```

The output is a standalone, zero-dependency Rust workspace — three crates and a system crate that
wires them — and, before any code, a **plan**. The plan is the hinge of the whole wave, so it comes
first here too.

## The plan never guesses

`PLAN.md` (and its machine twin, `plan.json`) travels inside the workspace and opens with the
disposition count:

> 45 capabilities: **33 generated**, **8 obligations**, **4 refused**. An obligation is yours to
> implement against its contract; a refusal is a fact about this synthesis scope, not about the
> specification.

Every semantic capability of the specification gets exactly one of three dispositions, and there is
no fourth:

- **generated** — the specification's own words determine the code, so the code is written. Types,
  events, errors, command contracts, view row types, the entity lifecycle, the binding's
  transformation and delivery, both component ports.
- **obligation** — the contract is declared but the behaviour is not, so you get a trait and a
  named entry, never an implementation. From the committed plan, verbatim:

| capability | source | why not generated |
| --- | --- | --- |
| command behaviour | `billing.email.SendEmail` | decided outside the system: the provider rejects the recipient address |
| command behaviour | `billing.invoice.CreateInvoice` | the contract is declared; the algorithm is not |
| view query | `billing.invoice.OutstandingInvoices` | how the projection is kept current is a storage decision |

- **refused** — this synthesis scope cannot represent it, and says so with the reason and the stage
  that refused. Also verbatim:

| capability | source | stage | why |
| --- | --- | --- | --- |
| actor grants | `billing.invoice.Customer` | planning | may invoke `billing.invoice.CreateInvoice`; a grant is checked against a caller identity, which types do not carry, and enforcement belongs to the layer that knows who is calling |
| workload | `invoice-service` | planning | requires at least 2 replica(s); topology synthesis is deferred with its design |

The rule underneath is the same refusal culture the rest of this repository runs on, applied to
codegen: **zero guessed business logic**. There is no disposition that means "generated, roughly".
The `calculate_tax` a code generator would love to invent is unrepresentable, because inventing it
would require a fourth disposition that does not exist. What the generator did not write is a
document, not an absence — and every obligation row carries its contract, in the specification
author's own words where the specification declares one ("the provider rejects the recipient
address" is the billing author's sentence, not the generator's).

Obligation stubs in the workspace refuse by name: each returns a value naming its plan entry —
never `todo!()`, never a panic — so a workspace built entirely on stubs compiles and reports
exactly what it cannot yet do. The plan's obligation list and the workspace's stub set are held to
a bijection by a test.

## The plan has no language in it

`SynthesisPlan` is language-neutral: dispositions, obligations and refusals are phrased against the
model, never against Rust. Rust lives only in the emission stage, behind the same seam a future Go
or TypeScript emitter would consume — `--target` takes `rust` today, a one-variant enum. A second
language is a second emitter, not a second planner, and a target-specific refusal is marked as the
target's, not the plan's. What was deliberately *not* built: a target registry or any
multi-language abstraction. One seam, one target behind it.

## The transition that does not compile

The billing specification declares an invoice lifecycle — `Draft → Issued → Paid`, with
cancellation from `Draft` and `Issued` but not from `Paid`. The emitter renders that as typestate:
one marker type per declared state, and a transition as a method that exists only on the states the
specification lets it start from. Try the transition the specification refuses:

```rust
use billing_types::invoice::{invoice_state, Invoice};

fn refund(invoice: Invoice<invoice_state::Paid>) -> Invoice<invoice_state::Cancelled> {
    invoice.cancel()
}
```

```text
error[E0599]: no method named `cancel` found for struct `Invoice<billing_types::invoice::invoice_state::Paid>` in the current scope
 --> src/main.rs:4:13
  |
4 |     invoice.cancel()
  |             ^^^^^^ method not found in `Invoice<billing_types::invoice::invoice_state::Paid>`
  |
  = note: the method was found for
          - `Invoice<billing_types::invoice::invoice_state::Draft>`
          - `Invoice<billing_types::invoice::invoice_state::Issued>`
```

The compiler's note *is* the state machine. Wave 2 made an unresolved reference unrepresentable in
the IR; this is the same property pushed into the target language — the illegal transition is not a
runtime error with a good message, it is a method that does not exist. The same discipline runs
through the rest of the emitted types: newtypes distinct from their representations, one outcome
enum per command with the refusal branches beside the successes, and the one transport the
specification's own words force — in-process, at-least-once, standard library only.

## The linker never chooses

The generated system crate assembles components, bindings and obligation implementations — and the
linking rule is the plan's no-guessing rule again. For every obligation it takes exactly one
offered implementation: **zero** offers is an unsatisfied obligation, **two** is an ambiguity error
naming both claimants, and refusals accumulate, so a linker with three empty slots reports three.
Selection among alternatives is `Realization` material from the design, and stays proposed with it.
Hand-written code lives outside the generated tree — the repository commits
`examples/billing-realization`, one implementation per obligation — and satisfies generated
interfaces by import, so `generated/` stays fully disposable.

## The payoff is executed, not asserted

Wave 4's criterion was that the generated suite must be seen to fail before anything gets to trust
it passing. Wave 6 inherits that: the committed billing suite — the same
`suites/generated/billing/suite.json` wave 4 wrote, unchanged, digest-checked against the
workspace's plan — runs against the synthesised workspace linked with the hand-written obligations,
and passes 27 of 27. Then the falsifiability half: one obligation implementation deliberately
corrupted (`accepts-any-amount`, the `CreateInvoice` guard dropped), and the same unchanged suite
fails exactly `billing.invoice.CreateInvoice/outcome/rejected` — blast radius one. Both halves are
CI, as the gate's `synth-check` step:

```console
cargo test -p billing-realization --test conformance
```

```text
running 3 tests
test the_same_suite_fails_the_corrupted_linkage_exactly_where_the_lie_is ... ok
test the_committed_suite_unchanged_passes_the_linked_synthesized_system ... ok
test two_runs_against_the_linked_system_produce_byte_identical_reports ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

And the suite earned its keep before any human reviewer did: it caught a real generator defect. The
generated delivery arm matched `is_err()`, which conflated a provider *refusing* an address — the
binding's declared `failed` outcome, whose policy is to escalate — with the behaviour behind the
port simply not being implemented yet. Under that shape a forced `SendEmail` failure produced no
`DeliveryEscalated`, and `notify-on-invoice-created/binding/on-failure` failed. The fix
distinguishes them: a declared failure takes the declared policy; an unmet obligation propagates,
because escalating it would publish a domain event no domain fact caused. That is the wave's
ordering rule paying out — code generated *after* the oracle exists gets judged by a suite that has
already been seen to bite, on its first day.

## 0.6.1 hardened the claims

`0.6.1-ess-wave-6.5` adds no capability; it makes the existing claims mechanical. Three invariants
that were enforced by nothing are now enforced by scans that fail the build — and every scan
carries an inverse assertion, so a scan that silently stops seeing violations fails instead of
passing. The model digest widened from 16 hex characters to the full SHA-256, everywhere at once,
because since a task's completion can rest on a digest comparison, the width has to follow the
responsibility. Property-based testing landed (`proptest`, fixed seed): any generated adversarial
specification is either refused with a reason or compiles to byte-identical canonical JSON twice —
no panic, no hang, no third outcome. The one deliberate fault that was recorded as *caught by
nothing* since wave 4 is now caught: an outcome can declare where an emitted event's payload comes
from, synthesis asserts the declared values, and `wrong-event-payload` fails
`billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled` with a blast
radius of two. Every event payload a command determines is asserted — and a field the model leaves
undetermined is asserted for presence and type, never for a value, because a minted identity is not
the command's to predict. The gate behind all of it: 8 steps, 69 suites, 1397 tests, 0 failures.

The full record for the wave — the decisions taken, what was rejected by name, and what is
deliberately not in it — is in the repository under
[`docs/plan/ess-wave-6-structural-synthesis.md`](https://github.com/codewandler/engineering-protocols/blob/main/docs/plan/ess-wave-6-structural-synthesis.md).
Tags: [`0.6.0-ess-wave-6`](https://github.com/codewandler/engineering-protocols/releases/tag/0.6.0-ess-wave-6),
[`0.6.1-ess-wave-6.5`](https://github.com/codewandler/engineering-protocols/releases/tag/0.6.1-ess-wave-6.5).
