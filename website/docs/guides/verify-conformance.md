---
title: Verify an implementation
sidebar_position: 5
description: Generate the conformance suite a specification obliges, run it against an implementation, prove the suite bites, and turn a run into protocol evidence.
---

# Verify an implementation

A specification is also an oracle: it derives the test suite an implementation is held to, and a run
of that suite becomes the evidence that completes — or refuses to complete — an AEP task.

## Generate and run the suite

```console
$ protocol ess conform synthesize --path examples/billing
$ protocol ess conform run --path examples/billing --target billing
```

`synthesize` walks the compiled IR and writes one scenario per obligation the specification states:
each declared outcome, each lifecycle move, each move that must **not** be honoured, entity
invariants after each move, value-object invariants at observable field positions, and each of the
four claims a binding makes. For `examples/billing/` that is 29 scenarios.

A construct the specification does not say enough about to test is **refused, not omitted**, and the
refusal is printed beside the scenarios that exist — a suite quietly holding fewer checks than the
specification requires is the one failure a passing run cannot show.

## Exit codes: wrong is not the same as unverified

| Exit | Meaning |
|---|---|
| `0` | every scenario passed |
| `1` | the implementation contradicted the specification, or could not expose what a required scenario checks |
| `3` | nothing contradicted the specification, and at least one scenario could not be executed |

`1` says the system is wrong; `3` says nobody found out. Running the billing suite against a target
that has never heard of the commands is a `3`, not a pass. Scenarios individually report `passed`,
`failed` or `unsupported`, and an `unsupported` scenario still fails the run — a check the target
could not make is not a check that passed.

## Prove the suite bites

A suite that passes everything tells you nothing, so the runner can break one property on purpose
and name the scenario that exists to catch it:

```console
$ protocol ess conform run --path examples/billing --target billing --inject accept-invalid-amount
billing v3 against billing-reference-accept-invalid-amount 0.1.0 — failed
  ...
  failed billing.invoice.CreateInvoice/outcome/rejected
  ...
  29 scenarios: 28 passed, 1 failed, 0 error, 0 unsupported
injected fault: an input that satisfies no branch's guard is accepted by the guarded one — expected to
be caught by `billing.invoice.CreateInvoice/outcome/rejected`
not conformant: the implementation contradicted the specification (exit 1)
```

The repository holds thirteen implementations that are wrong in exactly one way each, and a matrix
asserts which named scenario catches each fault — plus a blast-radius allowance, so a suite that
starts over-reaching fails rather than looking thorough.

## Turn a run into evidence

`conform run` prints a report. `conform evidence` runs the same suite and writes the record the
protocol decides on:

```console
$ protocol ess conform evidence --path examples/billing --target billing
- kind: ess_conformance
  specification: billing/v3
  spec_digest: 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  implementation: billing-reference 0.1.0
  status: passed
  scenarios_total: 29
  scenarios_failed: 0
  producer:
    producer: verifier
    verifier: conformance-runner
```

That file goes straight into `protocol evaluate --evidence`, and `examples/billing-conformance/`
walks both directions: a passing run completes the task; a run against a faulty implementation
leaves it blocked, naming `ess_conformance.scenarios.failed = 1` and the principle that refused.

Three properties of that record are load-bearing:

* **The record is minted in the process that ran the suite.** `ConformanceReport::to_evidence`
  takes no argument naming who produced it, so there is no call site at which a caller can describe
  itself as the verifier — and no `--report file.json` input exists, because that would be a record
  whose contents came from a file the caller wrote.
* **The digest is what makes it worth anything.** `billing/v3` is a label two resolutions can
  share; `spec_digest` is the content identity of the resolved model. The `ess-conformance`
  principle fails closed: a specification artifact recording no digest can never be shown conformed
  to, and a run against yesterday's model does not close a task built against today's.
* **`independent: true` is structural, not attested.** It says the producer is not the agent under
  review — checked by one comparison, signed by nothing. Which producers a harness lets write
  records is the harness's decision. See [Limitations](../status/limitations.md).

## Requiring conformance in a profile

Add the shipped principle like any other:

```yaml
principles:
  - ess-conformance
```

It is conditional on the task carrying an `executable-system-specification` artifact at all, so a
task with no specification owes nothing, and adding a specification turns the rule on without
editing the rule.

## The limit: in-process targets only

`ConformanceTarget` is a Rust trait, and `protocol ess conform run` reaches only the reference
implementations it was compiled with. **Holding your own system to a specification means depending
on the `ess-conformance` crate from your own tests** and implementing the trait as an adapter.
Nothing speaks to an implementation over a socket yet.

Two commands, one word apart, ask different questions — each `--help` names the other:

| Command | Asks |
|---|---|
| `protocol conformance` | does a storage **backend** implement the AEP contract — commands, queries, audit, idempotency? |
| `protocol ess conform` | does an **implementation** satisfy this specification — is a negative amount refused? |
