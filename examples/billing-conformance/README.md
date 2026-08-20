# Worked example: a task that finishes because the specification says so

The claim this repository exists to make, run end to end. `examples/development-passkeys/` shows a
task governed by principles a person wrote down; this one shows a task governed by a **machine**:
nobody reads the diff and decides whether it matches the specification. The specification generates
its own suite, something other than the agent runs that suite against the implementation, and the
protocol refuses to call the work done until the run says it holds.

Both directions are here, and the second is the one that matters. A demo where the happy path works
proves nothing a hardcoded `true` would not.

```console
$ cargo build -p protocol-cli
$ B=target/debug/protocol
```

## The three documents

| File | What it is |
|---|---|
| `task.yaml` | BILL-301, under `development.critical` — the shipped profile that carries `ess-conformance` |
| `artifacts.yaml` | the artifact graph, including `ess:billing` with the **model digest** of the resolved specification |
| `evidence/01`–`05` | the ordinary evidence a critical task owes: red test, diff, verifier findings, a human approval, five verification claims |
| `evidence/06-conformance.yaml` | **produced by the runner**, not written by hand |
| `evidence/06-conformance-faulty.yaml` | the same, for an implementation that is wrong in one way |

The `ess-conformance` rule is conditional: a task with no executable system specification owes
nothing extra. What turns it on is one entry in `artifacts.yaml`, checked against the artifact graph
at evaluation time rather than guessed at when the profile resolves:

```yaml
  - id: ess:billing
    kind: executable-system-specification
    model_digest: e19d384dac86219a
```

## 1. Run the specification's own suite against the implementation

```console
$ $B ess conform run --path examples/billing --target billing | tail -3
  27 scenarios: 27 passed, 0 failed, 0 error, 0 unsupported
  1 construct(s) of the specification got no scenario — run `protocol ess conform synthesize` to see which
conformant: every scenario the specification obliges passed (exit 0)
```

## 2. Turn the run into evidence

`run` prints a report about an implementation. The protocol decides on an evidence record, which is
a different document: it carries who produced it.

```console
$ $B ess conform evidence --path examples/billing --target billing
- kind: ess_conformance
  specification: billing/v3
  spec_digest: e19d384dac86219a
  implementation: billing-reference 0.1.0
  status: passed
  scenarios_total: 27
  scenarios_failed: 0
  suite_version: ess-conformance/1
  compiler_version: 0.1.0
  generator_version: 0.1.0
  producer:
    producer: verifier
    verifier: conformance-runner
  provenance:
    command: protocol ess conform evidence --path examples/billing --target billing
    inputs:
    - examples/billing
```

Three fields carry the weight:

* **`producer: verifier / conformance-runner`.** The rule requires `independent: true`, which means
  the producer is not the agent under review. That word is stamped by the crate that ran the suite
  and there is no argument through which a caller can set it — see
  `crates/ess-conformance/src/evidence.rs` for what it does and does not buy.
* **`spec_digest`.** Not the label `billing/v3`, which two different resolutions can share. Without
  a digest a record says that *some* implementation passed *some* suite, and a run against
  yesterday's model would close a task built against today's. The rule fails closed: a specification
  artifact recording no digest can never be conformed to.
* **`status`.** `passed`, `failed`, or `inconclusive` — the last for a run that could not be carried
  out. "Nobody found out" is not "the implementation is wrong", and neither is a pass.

The command that produced the committed record is in the record. It is regenerated and compared byte
for byte by `crates/protocol-cli/tests/cli.rs`, so nobody can edit these two files by hand without a
test noticing.

## 3. The task completes — because of the run, and only because of it

Submit everything **except** the conformance record and the task stops one state short:

```console
$ $B evaluate --task examples/billing-conformance/task.yaml \
    --artifacts examples/billing-conformance/artifacts.yaml \
    --evidence examples/billing-conformance/evidence/01-red-test.yaml \
    --evidence examples/billing-conformance/evidence/02-implementation.yaml \
    --evidence examples/billing-conformance/evidence/03-verification.yaml \
    --evidence examples/billing-conformance/evidence/04-review.yaml \
    --evidence examples/billing-conformance/evidence/05-verifications.yaml \
    --advance
state       adversarial_verify (Adversarial verify)
...
  ✗ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
      evidence.missing = 1
  ? ess_conformance.passed                                        [principle ess-conformance]
      unobserved: ess_conformance.passed
  ? evidence ess_conformance from conformance-runner (independent)  [principle ess-conformance]
      0 of 1 required record(s) submitted
  ? conformance-runner must run                                   [principle ess-conformance]
      no evidence from conformance-runner has been recorded
```

Everything else this profile asks for is already satisfied. Add the run and the task finishes:

```console
$ $B evaluate --task examples/billing-conformance/task.yaml \
    --artifacts examples/billing-conformance/artifacts.yaml \
    --evidence examples/billing-conformance/evidence/01-red-test.yaml \
    ... \
    --evidence examples/billing-conformance/evidence/06-conformance.yaml \
    --advance
state       complete (Complete)
transitions
  (none: this state is terminal)
Task complete in `complete`:
```

## 4. And it refuses — the half that makes the rest mean anything

Same task, same everything else. One implementation that accepts an invoice the specification
refuses: `--inject accept-invalid-amount`, one of twelve deliberate faults.

```console
$ $B ess conform run --path examples/billing --target billing --inject accept-invalid-amount
billing v3 against billing-reference-accept-invalid-amount 0.1.0 — failed
  ...
  failed billing.invoice.CreateInvoice/outcome/rejected
  ...
  27 scenarios: 26 passed, 1 failed, 0 error, 0 unsupported
injected fault: an input that satisfies no branch's guard is accepted by the guarded one — expected to be caught by `billing.invoice.CreateInvoice/outcome/rejected`
not conformant: the implementation contradicted the specification (exit 1)
$ echo $?
1
```

That run produces a record too — `evidence/06-conformance-faulty.yaml`, `status: failed`, naming the
scenario. Submit it in place of the passing one:

```console
$ $B evaluate --task examples/billing-conformance/task.yaml \
    --artifacts examples/billing-conformance/artifacts.yaml \
    ... \
    --evidence examples/billing-conformance/evidence/06-conformance-faulty.yaml \
    --advance
state       review (Review)
transitions
  review -> complete [blocked]
      ✗ ess_conformance.passed — ess_conformance.passed = false [principle ess-conformance]
      ✗ ess_conformance.scenarios.failed == 0 — ess_conformance.scenarios.failed = 1 [principle ess-conformance]
Task incomplete in `review`:
```

Read what the refusal is **not**. The evidence is not missing —
`✓ evidence ess_conformance from conformance-runner (independent)` — it was submitted and accepted
as independent. What refuses the task is what the record *says*, and the refusal names both the rule
(`principle ess-conformance`) and the observation (`ess_conformance.scenarios.failed = 1`). The
record itself names the scenario, so the repair is a lookup rather than a re-run.

## 5. A passing run against a different revision does not count either

Change one character of `model_digest` in the manifest and the same passing record stops closing the
task: it is a true report about a different resolution of the specification. That is the same rule
as `examples/development-passkeys/`'s stale approval, one layer down — an approval of version 3 does
not cover version 7, and a suite run against yesterday's model does not attest today's.
`a_conformance_run_against_another_revision_of_the_model_does_not_close_the_task` in
`crates/protocol-cli/tests/cli.rs` is that check.

## What is not proven here

* **Attestation.** `independent: true` says the record was produced by the conformance runner rather
  than by the agent under review, checked structurally. Nothing signs the file. A person can type
  one, and closing that gap is the harness's job, not the protocol's.
* **`artifact.design.sections_present`** is declared in `task.yaml` under protest, with the reason
  written there: the `Evidence::Artifact` record that should carry it cannot currently be written in
  an evidence document at all. It is unrelated to conformance, and it is the one place this example
  asserts rather than observes.

## Files

| File | What it is |
|---|---|
| `task.yaml` | BILL-301 under `development.critical` |
| `artifacts.yaml` | the graph, with the executable system specification and its model digest |
| `evidence/01-red-test.yaml` | the failing test, before any code |
| `evidence/02-implementation.yaml` | the change, produced by an agent and recorded as such |
| `evidence/03-verification.yaml` | unit, regression, mutation, differential, contracts, static analysis, a property, the specification |
| `evidence/04-review.yaml` | a human approval of the design version the manifest holds |
| `evidence/05-verifications.yaml` | the mutation, differential, invariant, pre/postcondition and provenance claims |
| `evidence/06-conformance.yaml` | the conformance run — **generated**, byte-checked against the runner |
| `evidence/06-conformance-faulty.yaml` | the same run against a deliberately wrong implementation |
