---
title: Generate contracts and documentation
sidebar_position: 5
description: Derive docs, JSON Schema, OpenAPI and AsyncAPI from a specification, keep the committed output drift-checked, and know what a projection cannot carry.
---

# Generate contracts and documentation

Once a specification validates, four projections derive from it. Everything on this page is
deterministic: the same source produces byte-identical output, so generated artifacts can be
committed, reviewed and drift-checked like source.

## The four projections

```console
$ protocol ess generate --path examples/billing --kind openapi
billing v3 (13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861) — 1 projection(s), 2 artifact(s)
  openapi/email-service.yaml — 4710 byte(s)
  openapi/invoice-service.yaml — 15984 byte(s)
nothing written: pass --out to write these, or --format json for their contents
```

| `--kind` | Output | Why it exists |
|---|---|---|
| `docs` | Markdown with Mermaid diagrams (lifecycles as state diagrams, bindings as flowcharts) | the cheapest completeness check: a construct with no rendering is a hole in a page a person reads |
| `schema` | one JSON Schema per command input, event and error payload, plus the named types | the type system projected without losing its distinctions — newtypes stay separate definitions |
| `openapi` | one OpenAPI 3.1 document per component | the specification *is* the HTTP contract, not a document beside it |
| `asyncapi` | one AsyncAPI 3.0 document per component | the same for messaging, including what happens when a binding fails |

Omit `--kind` and all four are produced together: 35 artifacts for `examples/billing`.

Without `--out` you get a listing and nothing is written — a command that looks read-only does not
write into whatever directory you happened to be in. **`--out` names the root of the tree, not one
projection's directory**: each artifact's path already begins with its projection, so the committed
output of the whole set is one command.

```console
$ protocol ess generate --path examples/billing --out generated
```

Every artifact carries provenance: specification version, the digest of the resolved model, a
separate digest of the contract surface, compiler and generator versions, and the regeneration
command. The model digest is over the *model*, not the source files, so it does not move when a
comment does — a digest that moves for no reason is one every reader learns to ignore.

See [the worked example](../examples/specification-to-contracts.md) for one command's source next to
each generated document.

## The graph, without generating a tree

`protocol ess graph` prints the actor/command/event picture the generated docs open with:

| `--format` | Output |
|---|---|
| `dot` (default) | Graphviz, for `dot -Tsvg` |
| `mermaid` | a `flowchart`, unfenced — redirect into a Markdown file or paste into a PR |
| `json`, `yaml` | the nodes, edges and groups themselves — 13 nodes, 7 edges and 3 groups for `examples/billing` |

`text` is still accepted as `dot`'s old name.

One renderer produces both the CLI's diagram and the documentation's: `protocol ess graph --path
examples/billing --format mermaid` emits exactly the bytes fenced under *The system as a graph* in
`generated/docs/README.md`, and a test compares them, so the two cannot drift.

## Drift-checking in CI

Commit the generated output and regenerate in CI:

```console
$ cargo xtask generate --check    # committed projections still match the specification?
projections are up to date
$ cargo xtask suite --check       # committed conformance suites still match?
suites are up to date
```

Both fail on any byte of difference. A generated OpenAPI document that has drifted is a contract
someone is already building against; a stale conformance suite certifies the wrong thing. The same
`--check` flag is on `xtask schema`, `xtask synth` and `xtask infra`, which cover the published JSON
Schemas, the synthesised trees and the example cluster's committed IR.

## What a projection can quietly destroy

Two questions to ask of any generated artifact, answered honestly for these:

* **A newtype collapses on the wire.** `billing.invoice.Email` and `billing.email.EmailAddress`
  stay separate schema definitions — each carries `"x-ess-kind": "newtype"` and its own name — so
  code generators emit two types. But both are `"type": "string"` on the wire, and a payload with
  the two values swapped validates clean. JSON Schema constrains structure; it cannot carry nominal
  identity.
* **A command's HTTP path is a convention.** The model has no `exposures:` construct, so
  `/invoices/commands/create-invoice` is a shape the generator chose — written into the generated
  document's own `info.description` rather than left for a reader to infer.

And one check that is scoped rather than total, stated per projection:

| projection | what is checked | what is not |
|---|---|---|
| `schema` | every document is validated against the real JSON Schema 2020-12 meta-schema and built into a validator | — |
| `openapi` | every **embedded** schema is validated against the same meta-schema, because OpenAPI 3.1's dialect *is* 2020-12 | the envelope, checked against an enumerated list by hand |
| `asyncapi` | the envelope is checked as a skeleton: version, `info`, `channels`, `operations`, and every operation's `action` | the payloads. They are AsyncAPI Schema Objects and declare no `schemaFormat`, so validating them against 2020-12 would assert a dialect the document does not claim |

What closes the two gaps is vendoring the OpenAPI 3.1 and AsyncAPI 3.0 meta-schemas, which is an
open decision rather than an oversight: neither ships with anything here, and a test may not fetch
one — the validator is built with `default-features = false` and has no retriever, so it could not
reach the network if it tried.
