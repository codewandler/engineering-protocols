---
title: Limitations and trust assumptions
sidebar_position: 2
description: What you still have to take on faith, and what the missing pieces mean for an adopter — stated so they can be weighed, not discovered.
---

# Limitations and trust assumptions

The point of the project is that what you must take on faith is narrow and named. This page is that
list, with the concrete consequence of each entry for someone adopting the tool.

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

**No durable backend.** The only implementation of the AEP storage contract is in memory; it forgets
everything when the process exits. Persisting an execution is possible (snapshots serialise), but
entities, audit trails and histories do not survive a restart. Writing a durable backend means
implementing two traits and proving the result against the shipped conformance suites — the
repository's backend guide (`docs/guide/backend.md`) covers it.

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

## Scope limits that are boundaries, not gaps

* **No federated artifact graphs.** A manifest describes one project; cross-repository references
  are resolved by hand.
* **Infrastructure scanning lives outside.** Raw cluster scans are trusted to the external scanner;
  this workspace begins at the observation file.
* **No team has been governed by this yet.** The protocol runs, the documents validate, the suites
  bite — and the next honest milestone for AEP is not a feature but a team whose work it actually
  governs, which has not happened.

---

**Sources.** `docs/VISION.md` § *The thesis* (the attestation gap);
`crates/aep-domain/src/evidence.rs`; `docs/guide/harness.md`; `docs/plan/gap-register.md` (D-3);
`README.md` § *What does not work yet*; `CHANGELOG.md` § *0.7.0* (the demonstration's stated
limits); `docs/guide/specification.md` § *Two things a projection can quietly destroy*.
