---
title: Roadmap and proposals
sidebar_position: 3
description: What has been delivered in which order, what is deferred by decision, and which design proposals are accepted, rejected or still only proposed.
---

# Roadmap and proposals

The repository keeps design proposals in `docs/design/`, and holds a strict line: **a proposal is
not a work order**, however long and however recent. Work is sequenced by plan pages in
`docs/plan/`, and each wave must be falsifiable by the one before it — a generated artifact nothing
can check is a claim, not a deliverable. (The oracle was made to fail on purpose, once per fault,
before code generation was allowed to be judged by it.)

## Delivered, in order

| Wave | Delivered | Tag |
|---|---|---|
| AEP waves 1–4 | the protocol: engine, contract, identity and audit, backend conformance suites, document tree | `0.1.0` … `0.2.1` |
| ESS wave 1 | the typed model | `0.3.0-ess-wave-1` |
| ESS wave 2 | the compiler and IR | `0.3.1-ess-wave-2` |
| ESS wave 3 | four projections, committed and drift-checked | `0.3.2-ess-wave-3` |
| ESS wave 3.5 | reconciliation: twenty gates closed before the oracle was allowed to start | `0.3.3-ess-wave-3.5` |
| ESS wave 4 | the specification as oracle: generated suites, runner, evidence | `0.4.0-ess-wave-4` |
| ESS wave 5 | semantic diff and impact closure | `0.5.0-ess-wave-5` |
| ESS wave 6 | structural synthesis: the plan and the Rust emitter, proven against the generated suite | `0.6.0-ess-wave-6` |
| ESS wave 6.5 | hardening: three unenforced invariants gained build-failing enforcement, full SHA-256 digests, property tests | `0.6.1-ess-wave-6.5` |
| ESS wave 7 | contract digests and artifact-level impact, ten diff families, the Go and browser emitters, and the dual-target demonstration | `0.7.0-ess-wave-7` |
| Infra waves 1–4 | observation → IR → graph/diagnosis → desired state and simulation → gaps projected back as patches | `0.7.0` and `0.7.1-infra-waves-1-4` |

## Deferred by decision

* **W7.4 — obligations become artifacts and tasks.** An obligation would become a typed artifact a
  task can own and evidence can close. Its precondition (a contract digest that exists in code) is
  met; the wave stays unscheduled by decision.
* **D-3 — attested evidence.** A proposed design exists and is deliberately unaccepted; see
  [Limitations](./limitations.md).
* **A durable backend.** No wave claims it; the contract and its conformance suites are the
  intended path for whoever builds one.

The full register of open gaps, each with what closes it, is `docs/plan/gap-register.md` in the
repository.

## The four extension proposals

Four designs proposed extending the project. Their status, so that reading the newest file in
`docs/design/` is never mistaken for reading the plan:

| Proposal | Status |
|---|---|
| **Closed-loop execution and conformance** — the specification as an oracle | **built** (ESS wave 4); its four open decisions taken at their stated defaults |
| **Semantic diff, impact and evolution** | **core built** (wave 5, extended by wave 7) after being accepted as a stated amendment to the project's thesis. Two of its seventy-eight sections — the proposal-evaluation loop and architecture search — were rejected outright |
| **Structural synthesis, obligations and realizations** | **delivered through wave 7**: plan, three emitters, dual-target demonstration. Rejected outright: behavioural synthesis (§36), the agent loop (§41), obligation-derived grants (§28). Still proposed with the design: `Realization` as a first-class object, topology synthesis, formal verification |
| **Infrastructure discovery and multi-cloud realization** | the design as written stays **deferred whole** — it would put cloud discovery adapters inside the workspace, which the vision refuses. A Kubernetes-scoped subset was built instead as infra waves 1–4, with the scanner kept outside the boundary |

The semantic-diff acceptance is the instructive one: it was **not** implied by the original thesis
("specified once and compiled" says nothing about a system *changing*), so absorbing it meant
amending the thesis in writing, with the reason — conformance evidence binds to a specification
digest, and without a delta any edit sends every requirement back to owed, which is correct and
blunt.

## The next honest milestones

* **AEP:** not a feature — a team whose work the protocol actually governs. That has not happened.
* **ESS:** wave 7 closed the loop it named; what remains (W7.4) is deferred rather than scheduled.
* **Infrastructure:** waves 1–4 delivered the observe → desire → project loop; no further wave is
  currently sequenced on a plan page.

---

**Sources.** `docs/plan/ess-roadmap.md`; `docs/plan/gap-register.md`; `AGENTS.md` § *Which
documents are normative*; `docs/VISION.md` § *Proposed, not accepted* and § *The thesis*;
`git tag -n99`; `CHANGELOG.md` §§ *0.7.0*, *0.7.1*.
