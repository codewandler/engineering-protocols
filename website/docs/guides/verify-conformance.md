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
$ protocol ess conform synthesize --path examples/billing | head -4
billing v3 — 29 scenario(s), 0 refusal(s), model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  billing.email.SendEmail/outcome/failed
  billing.email.SendEmail/outcome/sent
  billing.invoice.CancelInvoice/outcome/cancelled
$ protocol ess conform run --path examples/billing --target billing | tail -2
  29 scenarios: 29 passed, 0 failed, 0 error, 0 unsupported
conformant: every scenario the specification obliges passed (exit 0)
```

`synthesize` walks the compiled IR and derives one scenario per obligation the specification states:
each declared outcome, each lifecycle move, each move that must **not** be honoured, entity
invariants after each move, value-object invariants at observable field positions, and each of the
four claims a binding makes. For `examples/billing/` that is 29 scenarios.

It writes nothing unless you ask it to. The line `head -4` cut above is `nothing written: pass --out
to write suite.json, or --format json for its contents` — a verb that scatters files over a working
tree the first time someone tries it is a verb nobody tries twice. `--out <dir>` writes `suite.json`
there, and `conform run --suite <file>` runs a written suite instead of re-deriving one. The suites
committed under `suites/generated/` are maintained by `cargo xtask suite`, which reads
`--format json`.

A construct the specification does not say enough about to test is **refused, not omitted**, and the
refusal is printed beside the scenarios that exist — a suite quietly holding fewer checks than the
specification requires is the one failure a passing run cannot show. Billing has none. The
`examples/revision-pair/` fixture has four, and they read like this:

```console
$ protocol ess conform synthesize --path examples/revision-pair/before | grep -A 3 ESS-SYNTH-013
refusal[ESS-SYNTH-013]: type catalog.pricing.Money has no scenario
  no view publishes a field position that can answer what `catalog.pricing.Money` declares of every value
    - `amount >= 0`
  help: publish a field that holds a value of this type in some view — outside a list, a map, a union and an `Optional` — or state the claim as an entity invariant over what a view already publishes
```

The claim is in the specification and no view exposes anything that could answer it. The suite says
so, with the code, the reason and what would change it — instead of arriving four checks shorter
with nothing to indicate that it had.

## Exit codes: wrong is not the same as unverified

| Exit | Meaning |
|---|---|
| `0` | every scenario passed |
| `1` | the implementation contradicted the specification, or could not expose what a required scenario checks |
| `3` | nothing contradicted the specification, and at least one scenario could not be executed |

`1` says the system is wrong; `3` says nobody found out. Running the billing suite against a target
that has never heard of the commands is a `3`, not a pass:

```console
$ protocol ess conform run --path examples/billing --target oracle-fixture | tail -2
  29 scenarios: 0 passed, 0 failed, 29 error, 0 unsupported
undecided: nothing contradicted the specification and at least one scenario could not be executed — the target could not answer, so there is no verdict about it (exit 3)
```

Scenarios individually report `passed`, `failed`, `error` or `unsupported`, and an `unsupported`
scenario still fails the run — a check the target could not make is not a check that passed. `--untraced`
is that rule with a name: it hides the one observation the model refuses to require of every
implementation, and the run fails at `1` on a single scenario rather than passing at 28 of 29.

```console
$ protocol ess conform run --path examples/billing --target billing --untraced | tail -2
  29 scenarios: 28 passed, 0 failed, 0 error, 1 unsupported
not conformant: the implementation contradicted the specification, or could not expose what 1 required scenario(s) check — an unsupported required scenario is a failure and not a skip (exit 1)
```

## Prove the suite bites

A suite that passes everything tells you nothing, so the runner can break one property on purpose
and name the scenario that exists to catch it:

```console
$ protocol ess conform run --path examples/billing --target billing --inject accept-invalid-amount \
    | grep -E '^billing|^  failed|scenarios:|^injected|^not conformant'
billing v3 against billing-reference-accept-invalid-amount 0.1.0 — failed
  failed billing.invoice.CreateInvoice/outcome/rejected
  29 scenarios: 28 passed, 1 failed, 0 error, 0 unsupported
injected fault: an input that satisfies no branch's guard is accepted by the guarded one — expected to be caught by `billing.invoice.CreateInvoice/outcome/rejected`
not conformant: the implementation contradicted the specification (exit 1)
```

The other 28 scenarios pass, and the `grep` above is what hides them.

There are thirteen faults, each an implementation wrong in exactly one way, and a matrix asserts
which named scenario catches each — plus a blast-radius allowance, so a suite that starts
over-reaching fails rather than looking thorough. `--inject` with a name that is not one of them
prints the list:

```console
$ protocol ess conform run --path examples/billing --target billing --inject nope
error: `nope` is not a fault; known faults are wrong-event (--target billing), accept-invalid-amount (--target billing), allow-illegal-transition (--target billing), wrong-refusal-error (--target billing), drop-binding (--target oracle-fixture), wrong-mapping (--target oracle-fixture), stale-read-your-writes (--target billing), ignore-external-outcome (--target billing), wrong-event-payload (--target billing), partial-event-payload (--target billing), extra-event (--target billing), drop-consistency-token (--target billing), negative-projected-total (--target billing)
```

## Turn a run into evidence

`conform run` prints a report. `conform evidence` runs the same suite and writes the record the
protocol decides on:

```console
$ protocol ess conform evidence --path examples/billing --target billing --observed-at 2023-11-14
- kind: ess_conformance
  specification: billing/v3
  spec_digest: 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  implementation: billing-reference 0.1.0
  status: passed
  scenarios_total: 29
  scenarios_failed: 0
  suite_version: ess-conformance/1
  compiler_version: 0.1.0
  generator_version: 0.1.0
  observed_at: 1699920000000
  producer:
    producer: verifier
    verifier: conformance-runner
  provenance:
    command: protocol ess conform evidence --path examples/billing --target billing
    inputs:
    - examples/billing
```

`--observed-at` is why that record is quotable. `observed_at` is required, and without the flag it is
now, in epoch milliseconds — so the command above is byte-for-byte
`examples/billing-conformance/evidence/06-conformance.yaml`, and `diff` proves it. Pinning an
observation time is legitimate for exactly this, regenerating a committed record, which is why it is
an explicit flag rather than anything inferred. `--out` writes the record to a file instead of
standard output.

Adding `--inject accept-invalid-amount` gives the record for the other direction —
`examples/billing-conformance/evidence/06-conformance-faulty.yaml`, reproducible the same way. It
carries `status: failed` and `scenarios_failed: 1`, and one key the passing record does not have at
all:

```yaml
  failed_scenarios:
  - failed billing.invoice.CreateInvoice/outcome/rejected
```

The verdict names the scenario, so a reader does not have to re-run anything to know which one.

Either record goes straight into `protocol evaluate --evidence`, and `examples/billing-conformance/`
walks both directions. The passing run completes the task; the faulty one leaves it blocked:

```console
$ protocol evaluate --task examples/billing-conformance/task.yaml \
    --artifacts examples/billing-conformance/artifacts.yaml \
    --evidence examples/billing-conformance/evidence/06-conformance-faulty.yaml \
    --advance | grep ess_conformance
  ✗ ess_conformance.passed                                        [principle ess-conformance]
      ess_conformance.passed = false
  ✗ ess_conformance.scenarios.failed == 0                         [principle ess-conformance]
      ess_conformance.scenarios.failed = 1
  ✓ evidence ess_conformance from conformance-runner (independent)  [principle ess-conformance]
```

The refusal names the fact, the value, and the principle a person can go and argue with. The last
line is the one worth noticing: the record is accepted as independent and still refuses the task.

Three properties of that record are load-bearing:

* **The record is minted in the process that ran the suite.**
  `ConformanceReport::to_evidence(&self, observed_at)` takes one argument and it is a timestamp, so
  there is no call site at which a caller can describe itself as the verifier or name the
  implementation that answered. No `--report file.json` input exists either, because that would be a
  record whose contents came from a file the caller wrote — and the caller is often the agent under
  review.
* **The digest is what makes it worth anything.** `billing/v3` is a label two resolutions can
  share; `spec_digest` is the content identity of the resolved model, carried from the compiled
  model with no step at which it could be typed in. The `ess-conformance` principle fails closed: a
  specification artifact recording no `model_digest` can never be shown conformed to, and a run
  against yesterday's model does not close a task built against today's.
* **`independent: true` is structural, not attested.** It says the producer is not the agent under
  review — checked by one comparison, signed by nothing. A record naming `conformance-runner` is
  taken at its word. What would close it is written down and not yet decided: the runner holds a
  keypair, the report carries a signature over its canonical bytes plus the suite and specification
  digests, and `independent` becomes derived from a valid signature by a registered key rather than
  declared. That proposal is recorded as **proposed, not accepted** in `docs/plan/gap-register.md`
  § D-3, because it adds a signature dependency to a workspace with a written policy about
  dependencies. Until it is decided, which producers a harness lets write records is the harness's
  decision. See [Limitations](../status/limitations.md).

## Requiring conformance in a profile

Add the shipped principle like any other:

```yaml
principles:
  - ess-conformance
```

It is conditional on the task carrying an `executable-system-specification` artifact at all, so a
task with no specification owes nothing, and adding a specification turns the rule on without
editing the rule. `development.critical` is the one shipped profile that already carries it, which
is why `examples/billing-conformance/` is governed by it.

## The limit: in-process targets only

`ConformanceTarget` is a Rust trait, and `protocol ess conform run` reaches only the two reference
implementations it was compiled with: `--target billing` and `--target oracle-fixture`. Nothing in
this build speaks to an implementation over a socket.

**Holding your own system to a specification today** means depending on the `ess-conformance` crate
from your own tests: implement `ConformanceTarget` for it, read the committed
`suites/generated/<system>/suite.json` with `ConformanceSuite::from_json`, and call
`Runner::for_suite(&suite).run(&suite, &target)`. That is the whole adapter — the suite is the same
document either way. What would close the gap is a runner that speaks to a target out of process,
which is a design decision about transport that the model deliberately does not carry today; the
open form is in [Limitations](../status/limitations.md).

Two commands, one word apart, ask different questions — each `--help` names the other:

| Command | Asks |
|---|---|
| `protocol conformance` | does a storage **backend** implement the AEP contract — commands, queries, audit, idempotency? |
| `protocol ess conform` | does an **implementation** satisfy this specification — is a negative amount refused? |
