---
title: What you still have to trust
sidebar_position: 2
description: One declaration the whole loop rests on, and the limits of what a generated artifact can carry.
---

# What you still have to trust

The point of this project is that what you must take on faith is narrow and *named*. So here is the
list, rather than a page claiming there is nothing on it.

## The one that matters: a producer declaring itself independent

Everything downstream of one declaration is checked. The declaration itself is not.

`independent: true` is a boolean over a self-declared `Producer`. **Nothing binds a verifier's
identity to the evidence it submits** — the engine will record a test result the harness invented,
and nothing downstream can tell. The harness guide states this as a rule for harness authors, which
is exactly the shape of rule this project exists to replace.

**What it means concretely.** A harness that reports a verifier as the producer of a suite it never
ran satisfies every requirement marked `independent: true`. The protocol has no way to know.

**What closing it would take.** Attested evidence: a signature over the record, and a key the
protocol already knows. There is no signature, no key and no attestation anywhere in the workspace.
The gap register records it as D-3, deliberately unscheduled — it adds a dependency class, and that
acceptance is the operator's, not a wave's. It is written down here for the same reason it is
written down in the repository's own vision document: an unnamed assumption is one nobody can
decide to close.

## Three invariants that were enforced by nothing

Until `0.6.1-ess-wave-6.5` this page listed three invariants whose enforcement column said
"nothing": the engine never manufactures evidence, the domain crate is clock-free and
randomness-free, and every mutation is a command. All three are now enforced by tests that fail the
build — a source scan over the engine, determinism scans in every crate that states the property,
and an enumeration of the contract's write surface that fails on any new public mutation path,
naming itself. Every scan carries an inverse assertion, so a scan that silently stops seeing
violations fails instead of passing.

They are kept on this page in the past tense because the list is only useful while it is honest —
in both directions.

## What a generated artifact cannot carry

**A newtype collapses on the wire.** `billing.invoice.Email` and `billing.email.EmailAddress` are
separate definitions in the generated schemas, so a code generator emits two types. But both are a
bare JSON string, and a payload with the two values swapped validates clean. JSON Schema constrains
structure; it cannot carry nominal identity.

**A command's HTTP path is a convention, not a specification.** The model has no `exposures:`
construct yet, so the generator chose the path shape and wrote that choice into the generated
document's own description.

**Envelopes are checked structurally.** Every schema the generated OpenAPI and AsyncAPI documents
*embed* is validated against the real JSON Schema 2020-12 meta-schema by a conforming validator. The
envelope around them is checked key by key, in the dialect the document declares, but not against the
OpenAPI 3.1 or AsyncAPI 3.0 meta-schemas — neither of which is vendored here. Holding them would mean
committing two third-party documents, each with its own licence, pinned version and update path,
inside a repository whose discipline is that committed artifacts derive from sources it owns. That
trade was weighed and declined; what is unchecked is the envelope, not the types.

## What has not been demonstrated

* **No team has been governed by this yet.** A project can be discovered; nothing has run under it.
* **No durable backend exists.** The reference implementation is in memory.

---

**Sources.** `docs/VISION.md` § *The thesis* (the attestation gap); `crates/aep-domain/src/evidence.rs`;
`docs/guide/harness.md` § *Never manufacture evidence*; `AGENTS.md` § *Invariants* 7, 8, 9 and 14;
`CHANGELOG.md` § *0.6.1* (the three invariants becoming enforced); `docs/plan/gap-register.md` (D-3);
`docs/guide/specification.md` § *Two things a projection can quietly destroy*;
`docs/plan/ess-roadmap.md` § *W3.2's first criterion, amended*; `README.md` § *What does not work
yet*.
