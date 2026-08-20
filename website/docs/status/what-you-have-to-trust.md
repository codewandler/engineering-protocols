---
title: What you still have to trust
sidebar_position: 2
description: One declaration the whole loop rests on, three invariants nothing enforces, and the limits of what a generated artifact can carry.
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
protocol already knows. There is no signature, no key and no attestation anywhere in the workspace,
and no plan document proposes one — so this is a gap, not a horizon. It is written down here for the
same reason it is written down in the repository's own vision document: an unnamed assumption is one
nobody can decide to close.

## Three invariants enforced by nothing

The repository keeps sixteen invariants, and each one carries what actually enforces it — a lint, a
type, a test or a scan. Three say "nothing", and they say it in the repository's own working
agreement rather than being quietly dropped:

| Invariant | Enforced by | Consequence |
|---|---|---|
| The engine never manufactures evidence | nothing | it is a property of how the API is used, stated as a rule for harness authors |
| The domain crate is clock-free and randomness-free | nothing | true today by inspection; the scan that would catch a clock being added covers a different crate |
| Every mutation is a command | nothing | one write path is a property of the contract's current shape. A second one would compile and pass the gate |

A fourth is narrower than it sounds: the determinism scan that bans clocks and unordered maps covers
the ESS compiler only. The other twelve workspace members are unscanned.

These are a target list for the next mutation review, and the list is only useful while it is honest.

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
* **No specification has produced a test.** The conformance suite, the runner, and the deliberately
  wrong implementation that would prove the suite bites are all unbuilt.

---

**Sources.** `docs/VISION.md` § *The thesis* (the attestation gap); `crates/aep-domain/src/evidence.rs`;
`docs/guide/harness.md` § *Never manufacture evidence*; `AGENTS.md` § *Invariants* 7, 8, 9 and 14;
`docs/guide/specification.md` § *Two things a projection can quietly destroy*;
`docs/plan/ess-roadmap.md` § *W3.2's first criterion, amended*; `README.md` § *What does not work
yet*.
