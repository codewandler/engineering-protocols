---
title: Proposed, not accepted
sidebar_position: 3
description: Four designs proposed extending this. One has been built, one was absorbed into the thesis by a deliberate amendment, one is delivered in its first slice — and the fourth is planned only as a scoped subset, with its adapter kept outside the boundary.
---

# Proposed, not accepted

A project that keeps design documents accumulates a hazard: the newest and longest file starts
reading like the plan. It is not, and the repository is explicit about it — a proposal is not a work
order, however long and however recent it is.

Four designs proposed extending this project. **Three have since been taken up** — one built, one
absorbed into the thesis by an amendment somebody had to argue for and write down, and one delivered
in its first slice with its remaining sections named rather than implied. **The fourth is planned
only as a scoped subset**, and the page still exists for the same reason it always did: nothing in
that directory should be mistaken for what has been agreed.

| What it would add | Status |
|---|---|
| **Closed-loop execution and conformance** — the specification becomes an *oracle*: a verdict on an implementation, not only a projection of a model | **built.** Delivered as ESS wave 4, `0.4.0-ess-wave-4`. Its four open decisions were each taken at the default the design named |
| **Semantic diff, impact and evolution** — the system changing over time, impact closure, what a revision invalidates | **core implemented** as ESS wave 5 (`0.5.0-ess-wave-5`), after being accepted as a stated amendment to the thesis. Two of its seventy-eight sections were rejected outright rather than deferred, being a different product |
| **Structural synthesis, obligations and realizations** — generated applications, and human or agent work carried as typed obligations | **delivered in part** as ESS wave 6 (`0.6.0-ess-wave-6`), hardened by wave 6.5. The review read the design as four waves rather than one, and the first is built: the plan, the Rust emission, obligations as plan entries. §36's behavioural synthesis, §41's agent loop and §28's obligation-derived grants were rejected outright; `Realization`, topology synthesis and formal verification stay proposed with the design; obligations-as-artifacts is sequenced as W7.4 and deferred by decision |
| **Infrastructure discovery and multi-cloud realization** — a fourth domain, with infrastructure specified and checked beside the existing pair | the design as written stays deferred — roughly eleven waves at this repository's measured rate, and it would put cloud discovery adapters inside this workspace, which the vision refuses. What changed: **a k8s-scoped subset is now planned as infrastructure waves**, with the discovery adapter external to this workspace, per that same vision boundary |

## What "accepted" had to cost

The first and the third rows are horizons the thesis already implied: "specified once and compiled"
promises the tests and the skeleton. Both halves are now delivered — the tests as wave 4, the
skeleton as wave 6 — and it is worth being precise about what that bought, because a design is not
the same thing as the code that came out of it. Four of the oracle design's constructs turned out to
be untestable as written, and the repository's account of that wave says which four and how each was
found. The synthesis design paid a different price: of its programme, wave 6 took exactly the slice
whose criterion the existing oracle could execute — and the generated suite promptly caught a real
defect in the code generator, a delivery policy that conflated a declared refusal with an unmet
obligation, before any human reviewer did.

The second row was **not** implied. "Specified once and compiled" describes a system in the present
tense and says nothing about one *changing*, so absorbing it meant amending the thesis rather than
extending a roadmap — and the amendment is written down, with the reason, in the vision itself. What
forced it was the oracle: conformance evidence is bound to the specification revision it attests, so
without a semantic delta a change to a comment in an unrelated domain sends every conformance
requirement back to owed. Correct, and blunt.

Infrastructure is still a second subject matter rather than a further projection of the first, and
the design as written remains deferred whole. The change is narrower than an acceptance: a
Kubernetes-scoped subset is planned as infrastructure waves, and the part the vision refuses — a
discovery adapter making live API calls under a credential — stays outside this workspace. That is
the fix the review named: move the design, not the boundary, because operating a system is the one
thing this project [says it does not do](../deliberately-not.md).

## What is actually sequenced

| Wave | Goal | State |
|---|---|---|
| ESS wave 4 | a generated conformance suite, and proof that it bites — checked against an implementation that is deliberately wrong | **delivered**, `0.4.0-ess-wave-4`. Twelve deliberately wrong implementations, eleven caught by the named scenario that exists to catch each; the twelfth recorded as caught by nothing, with the reason — a record wave 6.5 has since closed |
| ESS wave 5 | two compiled specifications, and a typed answer to what moved between them | **delivered**, `0.5.0-ess-wave-5`. It moved ahead of code generation by decision, because wave 4 left everything derived from a single revision with no way to say what a change invalidates |
| ESS wave 6 | a generated Rust workspace from the billing specification that compiles, and that passes the suite wave 4 generated — with a deliberately faulty implementation still failing it | **delivered**, `0.6.0-ess-wave-6`. On billing: 45 capabilities — 33 generated, 8 obligations, 4 refused — and the committed wave-4 suite, unchanged, passes 27 of 27 against the linked workspace while the corrupted linkage fails exactly the scenario that exists to catch it |
| ESS wave 6.5 | no new capability — the existing claims made mechanical | **delivered**, `0.6.1-ess-wave-6.5`. Three invariants that were enforced by nothing now fail the build when violated, the model digest is the full SHA-256, `proptest` phase 1 landed, an event payload's declared provenance is asserted (closing the one fault caught by nothing), and value-object invariant scenarios grew the billing suite 27→29 |
| ESS wave 7 | the loop closed over generated code: artifacts carry the digest of the model slice they derive from, and entities and commands join the delta | **scheduled** — W7.1 and W7.2. The wave's other two slices, W7.3 (a second emitter, Go) and W7.4 (obligations become artifacts and tasks), are **deferred by decision** |
| infrastructure waves | a Kubernetes-scoped subset of the infrastructure design, specified and checked beside the existing pair, with the discovery adapter external to this workspace | **planned.** The fourth domain as a whole stays deferred |

The ordering rule underneath all of them: **each wave must be falsifiable by the one before it.** A
generated artifact nothing can check is a claim, not a deliverable. Generating code judged by an
oracle nobody has seen fail is the exact mistake that ordering exists to prevent — the oracle was
made to fail on purpose, once per fault, before wave 6 started, and wave 6's generated code was then
judged by it on its first day.

---

**Sources.** `docs/VISION.md` § *Proposed, not accepted*; `AGENTS.md` § *Which documents are
normative*; `docs/plan/ess-roadmap.md` (the wave sequence, the ordering rule, and the review outcomes
for the proposals); `docs/plan/ess-wave-6-structural-synthesis.md` (the decisions taken, and what was
rejected or deferred by name); `CHANGELOG.md` §§ *0.6.0* and *0.6.1*; `docs/reviews/` for the reviews
the status column refers to.
