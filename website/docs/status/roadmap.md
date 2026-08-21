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
| Harness wave 1, trace wave 1 | the markdown planning store and the `protocol artifact` verbs, the Claude Code plugin, and the transcript checker | `0.8.0-harness-wave-1-trace-wave-1` |
| Harness waves 2–3, trace wave 2 | the reference driver — decided, then built: `protocol drive`, the driven shell, two enforcement hooks, `protocol workflow render`; and transcript conformance as an evidence kind the engine admits | `0.9.0-harness-waves-2-3` |
| Evidence horizons, the first governed run | records carry `observed_at` and requirements carry a `horizon`; `protocol drive` walked a real story out of this repository's own backlog, and blocked | `0.10.0-horizons-dogfood-lab` |

## Deferred by decision

* **W7.4 — obligations become artifacts and tasks.** An obligation would become a typed artifact a
  task can own and evidence can close. Its precondition (a contract digest that exists in code) is
  met; the wave stays unscheduled by decision.
* **D-3 — attested evidence.** A proposed design exists and is deliberately unaccepted; see
  [Limitations](./limitations.md).
* **A durable backend.** No wave claims it; the contract and its conformance suites are the
  intended path for whoever builds one. The markdown planning store is durable and is deliberately
  not that backend.
* **W4.3 — a story's completion gated on evidence.** Its acceptance criterion is a *verdict* on the
  design — accepted, accepted in part, or refused — and not a build. Until then, a story's
  `implemented` is a claim nothing checks.
* **W4.4 — a second harness on the same step map.** Unscheduled, which is why harness neutrality is
  listed as an untested claim rather than a delivered property.

The full register of open gaps, each with what closes it, is `docs/plan/gap-register.md` in the
repository. Twenty rows are open there today — ten from building the harness, ten from the first
outside adopter — and each names either the story or the decision that would close it.

## The extension proposals

Eight designs proposed extending the project. Their status, so that reading the newest file in
`docs/design/` is never mistaken for reading the plan:

| Proposal | Status |
|---|---|
| **Closed-loop execution and conformance** — the specification as an oracle | **built** (ESS wave 4); its four open decisions taken at their stated defaults |
| **Semantic diff, impact and evolution** | **core built** (wave 5, extended by wave 7) after being accepted as a stated amendment to the project's thesis. Two of its seventy-eight sections — the proposal-evaluation loop and architecture search — were rejected outright |
| **Structural synthesis, obligations and realizations** | **delivered through wave 7**: plan, three emitters, dual-target demonstration. Rejected outright: behavioural synthesis (§36), the agent loop (§41), obligation-derived grants (§28). Still proposed with the design: `Realization` as a first-class object, topology synthesis, formal verification |
| **Infrastructure discovery and multi-cloud realization** | the design as written stays **deferred whole** — it would put cloud discovery adapters inside the workspace, which the vision refuses. A Kubernetes-scoped subset was built instead as infra waves 1–4, with the scanner kept outside the boundary |
| **A planning store, and a reference driver** | **phase 1 accepted and built** (harness wave 1: the store, the `protocol artifact` verbs, the plugin); the driver's six open holes became taken decisions in wave 2 and shipped in wave 3. Phase 2 — the store as a contract implementation — is designed and not accepted |
| **Transcript conformance** | **built** across trace waves 1 and 2, and the design document still carries *proposed, not accepted* at its head. That is the repository's rule working as intended: the plan pages sequence work, and a design does not |
| **Evidence horizons** | **built** and released in `0.10.0-horizons-dogfood-lab`, after an adversarial review of the design returned 19 confirmed, 15 needs-change and 3 infeasible — all applied before any code landed |
| **Story completion, evidence-gated** | **proposed**; what W4.3 owes is a verdict, not an implementation |

The semantic-diff acceptance is the instructive one: it was **not** implied by the original thesis
("specified once and compiled" says nothing about a system *changing*), so absorbing it meant
amending the thesis in writing, with the reason — conformance evidence binds to a specification
digest, and without a delta any edit sends every requirement back to owed, which is correct and
blunt.

## The next honest milestones

* **AEP:** a driven run that reaches the operator. The first one blocked four states short, for two
  correct reasons about the step map — so the milestone is no longer *has it ever governed anything*
  but *can it carry one story all the way through*.
* **ESS:** wave 7 closed the loop it named; what remains (W7.4) is deferred rather than scheduled.
* **Infrastructure:** waves 1–4 delivered the observe → desire → project loop; no further wave is
  currently sequenced on a plan page.
* **The harness:** a second one. Every behavioural document is published as harness-neutral and
  exactly one adapter has ever read them, so the claim is untested rather than proven.
* **Adoption:** the first outside adopter's review is triaged into thirteen stories, twelve of them
  still drafts and none sequenced. Sequencing them is what turns *somebody could write a tree
  against this* into *somebody keeps one*.

---

**Sources.** `git tag -l`; `docs/plan/ess-roadmap.md`; `docs/plan/gap-register.md` (row counts);
`docs/plan/harness-wave-4-governed-dogfood.md` § *W4.1*; `docs/design/` (each proposal's own status
header); `AGENTS.md` § *Which documents are normative*; `docs/VISION.md` § *Proposed, not accepted*
and § *The thesis*; `CHANGELOG.md` §§ *0.7.0*, *0.8.0*, *0.9.0*, *0.10.0*.
