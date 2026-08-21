# engineering-protocols

> A strongly typed, portable and machine-executable specification for how autonomous engineering
> work is performed and proven correct.

Coding and operations agents are usually governed by prose:

> *Follow TDD, don't break existing APIs, verify your work, and ask before deploying.*

That reads well and enforces nothing. It leaves every operative question open: what counts as
following TDD, what evidence proves a test failed *before* the implementation existed, which
operations need approval, what "verify your work" means, when the task is actually finished, and what
happens when verification fails.

`engineering-protocols` moves those rules out of prompts and into typed, executable protocol
definitions. The model still reasons. The protocol decides what the resulting facts permit. The
agent may be probabilistic; the protocol semantics are not.

## Two halves, one seam

| | Governs | Answers |
|---|---|---|
| **AEP** — Agentic Engineering Protocol | how engineering work is performed | *Was this built properly?* |
| **ESS** — Executable System Specification | what software must exist | *Is this the thing we meant to build?* |

They are not layers: AEP does not know what an invoice is, and ESS does not know what a code review
is. They meet at exactly one point — evidence. ESS defines the target, work proceeds under AEP, ESS
conformance checks the result, and that verdict is a fact AEP's completion predicate reads. The loop
closes because the specification that *generated* the contracts is the one that *tests* the
implementation: an agent cannot pass by weakening a test it did not write, and cannot declare itself
done, because completion is a predicate over facts it does not control.
[`docs/VISION.md`](docs/VISION.md) is the full argument.

## What that looks like

```console
$ protocol explain --task examples/development-passkeys/task.yaml --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
```

The refusal has an address: it names a principle someone can go and read, and says what would unlock
the operation. Nobody wrote that denial into the task — the task names a profile and an objective,
and nine principles, a workflow and twelve capability decisions are derived from the document tree.

* **Red-before-green is a fact, not an instruction:**
  `evidence.first_seq.test_result < evidence.first_seq.diff`.
* **An agent cannot verify itself.** An evidence requirement marked `independent: true` is never
  satisfied by the agent's own report of a green suite.
* **An approval names the revision it approved.** Approving design version 3 stops satisfying the
  requirement at version 7 — otherwise a reviewer's name ends up attached to a decision they never saw.
* **Unknown is not false.** `✗` is a fact that is wrong — fix the code; `?` is a fact nobody
  observed — go run the tests. Only `true` permits a transition.
* **Capabilities default to deny**, and `deny` cannot be granted back by a later document.
* **Nothing is deleted.** Archive and supersede are the vocabulary; every mutation crosses one
  boundary carrying actor, executor, correlation, causation and an idempotency key.

## Is this for you

| Read | If |
|---|---|
| [`docs/guide/adopting.md`](docs/guide/adopting.md) | you have engineering rules you want enforced, and a repository to put them in |
| [`docs/guide/harness.md`](docs/guide/harness.md) | you are building an agent harness and want the protocol to decide what it may do |
| [`docs/guide/backend.md`](docs/guide/backend.md) | you are storing designs, reviews and approvals, and want them to survive an audit |
| [`docs/guide/specification.md`](docs/guide/specification.md) | you want a system's contracts, tests and documentation derived from one document instead of maintained beside it |

It is not a tool for making an agent ship features faster — and deliberately not an LLM orchestration
framework, a CI system or a deployment platform: nothing here calls a cloud API or holds a
credential. External systems do the work; this project decides what the results permit.

## What works, and what does not

Working today, all gated by `task check`: the AEP document tree, resolution, evidence-guarded
workflows and the `protocol` CLI; ESS specifications compiling to documentation, OpenAPI, AsyncAPI,
JSON Schema, generated conformance suites, and structural skeletons in Rust, Go and a browser
realization — with one specification run as two applications and compared in every gate run; a
Kubernetes observation checked three-valued against a desired state, with gaps projected back as
reviewable patches; a durable markdown planning store; and a
[Claude Code plugin](integrations/claude-code/) that plans through it.

Not working yet, stated plainly: no durable backend implements the storage contract — the reference
implementation is in memory; generated code is structural, never behavioural — every algorithm
remains a typed obligation; the conformance runner cannot reach an out-of-process implementation —
holding your own system to a specification means depending on `ess-conformance` from your own tests;
`independent: true` is self-declared, with no signature or attestation binding a verifier to its
evidence; and no team's work is governed by this yet — the repository is its own first user.

[`docs/status.md`](docs/status.md) is the full status report: the delivered waves (derived from the
tags, drift-checked in the gate), the component tables, and every limitation with its consequence.
The gate is the measurement — run `task check` rather than trusting a number written in prose.

## Where everything is

| | |
|---|---|
| [`docs/guide/`](docs/guide/) | the adopter's guide — start here |
| [`docs/VISION.md`](docs/VISION.md) | why this exists, and how the two halves compose |
| [`docs/status.md`](docs/status.md) | the status report: waves, components, limitations, and the full document index |
| [`AGENTS.md`](AGENTS.md) | the working agreement, including the invariant register — every rule names the check that enforces it |
| [`docs/design/`](docs/design/), [`docs/plan/`](docs/plan/) | designs — proposed until a plan page accepts them — and the plan pages that did |
| [`CHANGELOG.md`](CHANGELOG.md) | what changed, per release |

## Build

Requires a recent stable Rust, [go-task](https://taskfile.dev), the Go toolchain, the
`wasm32-unknown-unknown` target and Node. A check whose toolchain is missing fails and names it
rather than skipping.

```console
task check     # the ten-step gate: format, status, lint, tests, rustdoc, and five more drift checks
```

## Licence

Apache-2.0. See [LICENSE](LICENSE).
