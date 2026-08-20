# ESS wave 3 — projections that pay for themselves

> **Delivered.** Goal from [`ess-roadmap.md`](ess-roadmap.md): one model, three projections, and
> documentation that fails the build when the model outgrows it. Four generators behind one trait —
> the roadmap's three rows, with `OpenAPI` and `AsyncAPI` counted apart — the projections of
> `examples/billing/` committed under [`generated/`](../../generated/), and
> `cargo xtask generate --check` in `task check` and as a CI job of its own. 97 tests in `ess-gen`.
> What the wave did *not* deliver is at the bottom of this page.

Design phases 3 and 4. Wave 2 produced an IR whose type guarantees every reference resolves. This
wave is the first thing that reads it — and the first evidence that the model is worth having, because
until something is derived from it, "one source of truth" is a claim.

## Documentation first, and not because it is easy

It is the cheapest possible check on model completeness. A construct with no rendering shows up as a
hole in a page a person reads. The same construct missing from a JSON Schema shows up as a subtly
permissive schema that validates everything and nobody notices for a month.

So the acceptance criterion for the documentation generator is not "the output reads well" — it is
**every construct in the model appears somewhere in the output**, asserted per construct against
`examples/billing/`. A projection that silently drops tagged unions is the bug that test exists to
catch, and it is a bug every generator in this wave can have.

## Three projections, one trait

| projection | what it proves |
|---|---|
| Markdown + Mermaid | the model can be described — every construct has a rendering |
| JSON Schema | the type system is projectable without losing the distinctions it exists to make |
| OpenAPI + AsyncAPI | the specification is the contract, not a document beside it |

Three crates total, not eleven (review F9): `ess-domain`, `ess-compiler`, `ess-gen`. Each projection
is an implementation of one `Generator` trait rather than a crate of its own, because what differs
between OpenAPI and Markdown is the body, not how it is invoked.

## The two places a projection destroys the model

Both are worth naming in advance, because both are easy to do accidentally and hard to notice later.

**A newtype collapsing into its representation.** `Email` and `EmailAddress` are both a `String`
underneath. If a generated JSON Schema renders both as `{"type": "string"}`, the projection has
thrown away the one distinction the specification exists to make — and a consumer of that schema can
now put an invoice's email where a delivery address belongs, which is exactly what the model refuses.

**A command becoming an endpoint, or an event becoming a topic.** `CreateInvoice` is a command and
`POST /v1/invoices` is one way to expose it; `InvoiceCreated` is a fact and a Kafka topic is one way
to carry it. Design §6 and §7 both say the transport is a separate realization — and the model has no
`exposures:` or `transport:` construct yet. So the OpenAPI and AsyncAPI generators must either derive
the mapping by a **stated** convention, or report that the model needs the construct first. Inventing
a convention silently is the failure; either explicit answer is fine.

## Provenance, because an artifact nobody can attribute is an artifact nobody can audit

Design §10: every generated file carries the specification version, a digest of the resolved model,
the compiler version and the generator version. Four facts because each moves independently — the
same specification through two generator versions produces different output, and the moment there are
two checkouts that is the only question anyone asks.

The digest is over the IR rather than the source files, deliberately: two trees differing only in
comments and file layout mean the same system, and a digest that changed when a comment did would be
a digest every reader learns to ignore.

## Generated output is checked, not eyeballed

The failure mode a generator invites is output that looks right.

* **Regeneration is byte-identical**, and CI fails on a diff. Review F8's point applied one level up:
  the test is what makes determinism true.
* **An orphan is caught** — a committed artifact no generator produces any more is stale contract, and
  the check that only looks at what *is* generated will never see it.
* **Every construct, in every projection that should contain it**, asserted individually.
* **The output is validated, not merely produced.** A schema that accepts everything is not a schema,
  so each generated schema is checked against a hand-written valid instance and a hand-written
  invalid one.

## Deliverable

`protocol ess generate --kind docs|schema|openapi|asyncapi`, a committed generated tree,
`cargo xtask generate --check` in the gate, and a test per construct per projection.

## What shipped, against what this page asked for

| asked for | what shipped |
|---|---|
| Markdown + Mermaid, every construct rendered **or the generator refuses** | six pages; the generator does not refuse — see below |
| JSON Schema, the type system projectable without loss | 17 documents: every command input, event payload, error payload and named type. Draft 2020-12, one self-contained file each, no cross-file `$ref` |
| `OpenAPI` + `AsyncAPI`, the specification *is* the contract | one document per component for each: `OpenAPI` 3.1 × 2, `AsyncAPI` 3.0 × 2 |
| provenance, four facts (§10) | on every artifact, twice — as a comment a person reads and as `x-ess-provenance` a tool reads |
| regeneration byte-identical, CI fails on a diff | `task generate-check`, the CI job "Projections up to date", and a generate-twice-and-compare test per projection |
| an orphan is caught | `cargo xtask generate --check` scans the committed tree and names files no generator produces; `cargo xtask generate` deletes them |
| every construct, in every projection that should contain it | asserted per construct, and the IR now carries every construct the specification language has — the gap allowlist is empty and a test says so |
| output validated, not merely produced | met for JSON Schema, and for every schema the two contracts *embed* (39 fragments against the 2020-12 meta-schema). **Not met** for the `OpenAPI`/`AsyncAPI` envelopes — the open decision below |

### The documentation generator does not refuse, deliberately

The roadmap's wording was "every construct has a rendering, or the generator refuses". `generate` is
infallible instead, and a gap is made loud three other ways: a new variant of an enum this projection
matches on stops the build, because no `match` in `docs.rs` has a wildcard arm; a construct the IR
carries that no page mentions fails `tests/docs.rs`; and a construct the IR does not carry at all is
printed as a named gap on the page where a reader went looking for it.

Refusing would have been worse than any of the three. A specification that has already resolved is
not at fault for a hole in this crate, so failing would report the wrong thing — and it would destroy
the very pages that say what is missing, for a reader who cannot fix either.

### Entities, views and actors reached the IR, and the gap list emptied

This page was written while `EssIr` carried only the set of an entity's state names, and it said so:
three entries in `Docs::known_gaps`, each naming the IR change that would close it. All three landed.
`ResolvedEntity` carries the identity field with its name, the fields in declaration order, the
invariants and `ess-domain`'s own `StateMachine`; `ResolvedView` carries the source entity, the filter
as a parsed predicate, the exposed fields, the consistency level and the assertion style; and
`ResolvedActor` carries `may:` as a set of command handles, so a grant naming a command nobody
declares cannot be represented. The documentation renders all three, and `GAPS` is now empty.

The allowlist stays, empty, and that is the point of having built it as an allowlist: a *new* gap is a
failing test rather than a page that quietly omits something, and the emptiness is asserted rather
than assumed. One construct is deliberately not an entry — `SystemSpec::format` (`format: ess/1`) is a
fact about the document, not about the system, and `Provenance` already carries the specification
version.

What no contract projection derives from them yet is a separate matter and a candidate for the next
wave: a view is a read model an `OpenAPI` document could expose, and an actor's grants are the
authorization half of a contract. The `OpenAPI` projection's missing `security` block, though, is no
longer explained by the IR — `may:` says who may invoke a command, while a `securityScheme` describes
how a caller proves who it is, and the model states nothing about authentication. Emitting one would
invent a mechanism no specification backs.

### The two places a projection destroys the model, as they turned out

**A newtype does not collapse.** `Email` and `EmailAddress` keep separate definitions, separate
references and their own names in the schemas and in both contracts, so a code generator emits two
types. The residue is stated rather than papered over: on the wire both are a bare JSON string, so an
instance with the two values swapped validates clean. JSON Schema constrains structure and cannot
carry nominal identity.

**A command does not silently become an endpoint.** The model still has no `exposures:` or
`transport:`, so each generator writes its convention into the document it produces — `POST` at
`/{domain wire name}/commands/{command wire name}`, and a channel address that is the event's
declared `naming.wire` or else its full qualified name, tagged with `x-ess-address-source` so a reader
can tell a chosen address from a derived one. That is the second of the two answers this page called
acceptable: a stated convention, not a silent one. When `exposures:` lands it should override the
convention rather than replace it.

### The third place, which this page did not predict

Both risks above are about one projection getting the model wrong. The one that actually happened was
three projections getting it *differently*: each carried its own copy of the type mapping, and every
one of the 17 comparable projection pairs disagreed. `AsyncAPI` was the permissive side — no
`additionalProperties: false`, no `Decimal` pattern, no `Uuid` pattern — so a service validating an
event against the published `AsyncAPI` document accepted a `Money` with a non-numeric amount and
unknown extra fields that the JSON Schema tree refused. Two documents generated from one model,
describing the same bytes, disagreeing about what was valid.

The copies are gone: `openapi` and `asyncapi` now build their `components.schemas` from
`schema::types` and retarget the pointer. `tests/agreement.rs` is what keeps it that way, and it
classifies every difference as an assertion (what a document accepts) or an annotation (a fact the
model states) — both fail, and a keyword neither list names fails too, so the next keyword added
cannot slip in unclassified. The lesson worth carrying to wave 4: "the same value, deliberately,
without importing it" is not a property, it is a hope, and only a test comparing the outputs makes it
one.

## Open decision — `OpenAPI` and `AsyncAPI` are not validated against their meta-schemas

W3.2 asked for these documents to be "validated against their own schemas". They are not. What ships
is hand-written structural validation: `assert_valid` in `crates/ess-gen/tests/openapi.rs` and
`a_document_is_a_valid_asyncapi_three_skeleton` in `crates/ess-gen/tests/asyncapi.rs` check the
version string, `info.title` and `info.version`, that every path starts with `/`, that every operation
has an `operationId`, that every response key is three digits and carries a non-empty `description`,
that every parameter names a location the specification format knows, and that every action is `send`
or `receive`. That is the part which actually breaks, and it is **weaker than the criterion**: nothing
here would notice a keyword `OpenAPI` 3.1 does not permit where this generator put it.

The reason is mechanical: neither meta-schema is vendored in this repository. Three ways to close it:

| option | cost |
|---|---|
| vendor the `OpenAPI` 3.1 and `AsyncAPI` 3.0 meta-schemas and validate with a JSON Schema validator | two large third-party documents in the tree, each needing a provenance note and an update path |
| fetch the meta-schemas in a test | a gate that fails when someone else's CDN does; nothing in `task check` reaches the network today |
| keep the structural assertions and state the limit | the criterion stays unmet, in writing |

**Default if nobody decides:** the third — what is in the tree now, with this section as the record.

That loose end is now half closed, which narrows the decision. `jsonschema` is a dev-dependency of
`ess-gen` and is used: the hand-rolled `Subset` keyword checker is gone, `pattern` is checked for
real, and every schema each `OpenAPI` and `AsyncAPI` document *embeds* is validated against the
2020-12 meta-schema the crate bundles — 39 fragments. So a validator is no longer the obstacle and
`default-features = false` keeps every test off the network. What is missing is only the two envelope
meta-schemas, which is a vendoring decision rather than a dependency one.

## Not in this wave

Test synthesis is wave 4 and Rust structural synthesis is wave 5, per
[`ess-roadmap.md`](ess-roadmap.md); wave 1's plan page lists test synthesis under wave 3, and the
roadmap is the one to follow.
