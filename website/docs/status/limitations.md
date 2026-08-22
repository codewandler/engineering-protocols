---
title: Limitations and trust assumptions
sidebar_position: 2
description: What you still have to take on faith, and what the missing pieces mean for an adopter — stated so they can be weighed, not discovered.
---

# Limitations and trust assumptions

The point of the project is that what you must take on faith is narrow and named. This page is that
list, with the concrete consequence of each entry for someone adopting the tool. The register behind
it is `docs/plan/gap-register.md`, which holds one rule: a gap leaves it either **by decision** or
**by code**, and never by quietly disappearing.

## The trust assumption that matters most

**`independent: true` is structural, not attested.** It checks that an evidence record's declared
producer is not the agent under review — one comparison over a self-declared field. Nothing binds a
verifier's identity to the evidence it submits: the engine will record a test result the harness
invented, and nothing downstream can tell.

*Consequence:* a harness that misreports the producer of a suite it never ran satisfies every
independence requirement. Which producers may write records is the **harness author's**
responsibility, and the [harness guide](../guides/integrate-a-harness.md) states it as a rule —
which is exactly the shape of rule this project exists to replace, so it is named here as the
central open gap.

*What closing it takes:* attested evidence — a signature over the record and a key the protocol
already knows. There is no signature, no key and no attestation anywhere in the workspace. A
proposed design exists (gap register D-3) and is deliberately unaccepted: it adds a dependency
class, and that acceptance is the operator's to make.

## Storage

**There is a durable store, and it is not an implementation of the contract.** Planning artifacts
live as markdown under `.engineering/planning/` and survive a restart — this repository's own plan is
59 of them. But `aep-backend-markdown` writes through its own `create`/`update` rather than through
`CommandService`, so the 16 `aep-conformance` suites do not run against it, and it has no journal, no
audit join and no history.

*Consequence:* "there is a durable backend" is not yet a claim you can lean on. The only
implementation of the AEP storage contract is still in memory, and it forgets entities, audit trails
and histories when the process exits. Writing a durable one means implementing two traits and
proving the result against the shipped conformance suites — `docs/guide/backend.md` in the
repository covers it. Register rows D-P1 and D-P3.

## The driver, and what a governed run does not yet prove

The reference driver exists and has walked a real story. What it has not yet done is finish one.

* **A driven run mints a `trace_conformance` record and nothing gates on it.**
  `drivers/development/checks.yaml` runs `protocol trace evidence` over the session's own transcript
  and the driver submits the record the checker wrote, so the check is in the audit trail — but no
  workflow, principle or profile requires the kind, so a transcript that contradicts its
  specification stops nothing. *Consequence:* the transcript check is provenance, not enforcement.
* **A story declares its checks by convention rather than in its own document.** The checks map runs
  `bash .engineering/checks/run.sh`, because a planning artifact has no field for a command. A story
  whose checks live elsewhere has to make that path run them.
* **A hook's decision is recorded and is not folded into the audit trail.** Both plugin hooks write
  a decision log and the driver writes the step context, but nothing folds those lines back through
  `Engine::authorize`. *Consequence:* the audit trail is a complete account of what the engine
  decided and not of what the harness stopped.
* **The per-state tool set is enforced twice and audited by nothing.** The allowlist at session
  launch and the `PreToolUse` hook enforce the same derived set, and the expectation kind meant to
  audit it reads the harness's tool *inventory* rather than the session's allow rules.
* **A run directory cannot be read back as a full account of the run.** The engine's reasons arrive
  flattened into strings, there is no per-transition record and no report document, so a snapshot
  alone cannot say a run is still going.
* **A driven session is not hermetic.** A scratch config directory does not exclude account-level
  MCP servers: two of the four sessions in the first run listed three of them, all unauthenticated,
  and nothing asserts their absence.
* **Harness neutrality has never met a second harness.** Every behavioural document here is
  published as harness-neutral, and exactly one adapter exists. The claim is untested.
* **A story's `implemented` is a claim nothing checks.** A status move is validated against the
  kind's lifecycle and nothing else, so an artifact's status is whatever was typed. The rule that
  would fix it exists one layer down — the `ess-conformance` principle gates a *task's* completion on
  independent evidence — and has no analogue for the artifact.

## Evidence horizons: what decay does not yet reach

**A requirement with a horizon and no subject is revived by any fresh record of its kind.** The
matcher checks the subject only when one is given, so a fresh run about an unrelated component
restores a gate about this one. Until it is fixed, a requirement about one subject should say so.

**More generally, evidence does not have to name its subject.** A fact observed of one thing can move
another. This is not hypothetical: an adopter's end-to-end job held a legacy service while the
deployment rolled its successor, and produced weeks of green about a component nobody was shipping.
The approvals rule already refuses a record bound to the wrong revision; there is no analogue for the
wrong subject.

**An execution snapshot written before `observed_at` existed cannot be restored.** It fails to
deserialize and says which field is missing. Decided that way rather than defaulted, because a
default would have invented an observation date — which is the one thing this whole feature exists
to stop.

## Conformance reach

**No out-of-process runner, on either side.** `protocol ess conform run` reaches only the reference
implementations it was compiled with — `ConformanceTarget` is a Rust trait. Holding your own system
to a specification means depending on the `ess-conformance` crate from your own tests. The same
holds for backend conformance. Nothing speaks to an implementation over a socket.

## Generated code

**Structural, never behavioural.** A specification synthesizes types, typestate lifecycles, ports,
transports and a plan; every algorithm is a typed obligation someone still implements. Behavioural
synthesis is rejected in the roadmap, not pending.

**Obligations are plan entries, not artifacts.** An obligation cannot yet be owned by a task or
closed by evidence — that extension (W7.4) is deferred by decision, its precondition now met.

**The dual-target demonstration is not a deployment.** The generated servers speak plain HTTP with
no authentication and no TLS, take one connection at a time, and publish no `servers` block because
the model has no URL. The committed gatepass conformance suite and the wire demonstration are two
separate proofs — the suite is not run against the two live applications.

## The semantic diff

**A fail-closed arm remains.** Conversions, workloads and a domain's naming have no compared
construct family: a change there owes the whole suite rather than a narrowed set, stated as such.
Predicates are compared for canonical equality only — a provably weaker rewrite still reads as
*changed*, because implication would be a proof and is refused.

## What a projection cannot carry

* **Newtypes collapse on the wire.** `Email` and `EmailAddress` stay separate schema definitions,
  but both are a bare JSON string, and a payload with the two swapped validates clean. JSON Schema
  constrains structure, not nominal identity.
* **HTTP paths are a generator convention.** The model has no `exposures:` construct yet; the
  chosen path shape is written into each generated document's own description.
* **Envelopes are checked structurally.** Every embedded schema is validated against the real JSON
  Schema 2020-12 meta-schema; the OpenAPI 3.1 / AsyncAPI 3.0 envelopes around them are checked key
  by key but not against their own meta-schemas, which are not vendored here. What is unchecked is
  the envelope, not the types.

## What the first outside adopter found, and what is still open

On 2026-08-21 somebody who did not write this specification wrote a document tree against it — a
protocol extending `aep/1`, four workflows, six principles, four profiles, four lifecycles, 26 files
— and it validates. `resolve`, `explain` and `evaluate` all work on it. It arrived with a written
review of everything that got in the way, triaged into thirteen stories. One is implemented —
evidence horizons, which they ranked first and which shipped in `0.10.0` — and four unambiguous bugs
they found were fixed in the same release. The other twelve stories are drafts, and none is
scheduled.

Every row below was found by writing a tree, not by reading the guide. That is what makes the list
worth its space here: none of it was visible to the people who built the thing.

| Still open | Why it matters to an adopter |
|---|---|
| Nothing models a claim leaving the boundary | An assertion handed to a customer is near-irreversible and has no lifecycle here. `ArtifactStatus` is a closed ten-variant enum, so *sent, known wrong, audience not yet told* has no rung to stand on |
| One enforcement level | A check blocks or it is deleted. There is no state for *not ready to block yet* — invented independently three times in the adopter's stack. This repository has that tier in exactly one place, the transcript checker's `advisory` severity |
| Evidence does not name its subject | See above |
| Four lifecycle concepts the protocol cannot express | A decision with a declared default and an expiry; time-based transitions of any kind; a blocker typed by what clears it, without which *parked on a credential* is indistinguishable from *parked on a person* |
| A commitment on a clock nobody controls | It fires on a date the repository does not set, is satisfied by a person, and must never block a commit — blocking a commit cannot close one |
| `release.progressive`'s `promote` is one step | A real fleet is a set. A release live in some targets and deliberately held in others cannot be said, and the adopter's hold-back was implemented as a revert that a downstream force-push silently undid |
| Vocabularies that look open and are not | Three instances in one afternoon: a closed status enum, a project directory name that was a compile-time constant, and a kind ladder defined over built-in variants only. The last two are now fixed; what is owed is the audit that says which of the rest are open |

## Scope limits that are boundaries, not gaps

* **No federated artifact graphs.** A manifest describes one project; cross-repository references
  are resolved by hand.
* **Infrastructure scanning lives outside.** Raw cluster scans are trusted to the external scanner;
  this workspace begins at the observation file.
* **One governed run, and it stopped.** The protocol has now driven a real story out of a real
  backlog, and it blocked four states short of the person it was meant to stop at, for two reasons
  it printed. That is a limitation of the step map, not of the engine — and it is stated here rather
  than buried, because a dogfood wave that reports only its successes is marketing. No team's
  ongoing work is governed by this yet.

---

**Sources.** `docs/plan/gap-register.md` (every row above, with the story that closes it);
`docs/plan/harness-wave-4-governed-dogfood.md` § *W4.1*; `docs/VISION.md` § *The thesis* (the
attestation gap); `crates/aep-domain/src/evidence.rs` and `crates/aep-domain/src/requirement.rs`;
`docs/guide/backend.md`; `docs/guide/harness.md`; `CHANGELOG.md` §§ *0.7.0*, *0.10.0*;
`docs/guide/specification.md` § *Two things a projection can quietly destroy*.
