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
$ protocol ess generate --path examples/billing --kind openapi --out generated/openapi
```

| `--kind` | Output | Why it exists |
|---|---|---|
| `docs` | Markdown with Mermaid diagrams (lifecycles as state diagrams, bindings as flowcharts) | the cheapest completeness check: a construct with no rendering is a hole in a page a person reads |
| `schema` | one JSON Schema per command input, event and error payload, plus the named types | the type system projected without losing its distinctions — newtypes stay separate definitions |
| `openapi` | one OpenAPI 3.1 document per component | the specification *is* the HTTP contract, not a document beside it |
| `asyncapi` | one AsyncAPI 3.0 document per component | the same for messaging, including what happens when a binding fails |

Without `--out` you get a listing and nothing is written — a command that looks read-only does not
write into whatever directory you happened to be in.

Every artifact carries provenance: specification version, a digest of the resolved model, compiler
and generator versions, and the regeneration command. The digest is over the *model*, not the source
files, so it does not move when a comment does — a digest that moves for no reason is one every
reader learns to ignore.

See [the worked example](../examples/specification-to-contracts.md) for one command's source next to
each generated document.

## The graph, without generating a tree

`protocol ess graph` prints the actor/command/event picture the generated docs open with:

| `--format` | Output |
|---|---|
| `dot` (default) | Graphviz, for `dot -Tsvg` |
| `mermaid` | a `flowchart`, unfenced — redirect into a Markdown file or paste into a PR |
| `json`, `yaml` | the nodes, edges and groups themselves |

One renderer produces both the CLI's diagram and the documentation's, and a test compares them, so
the two cannot drift.

## Drift-checking in CI

Commit the generated output and regenerate in CI:

```console
$ cargo xtask generate --check    # committed projections still match the specification?
$ cargo xtask suite --check       # committed conformance suites still match?
```

Both fail on any byte of difference. A generated OpenAPI document that has drifted is a contract
someone is already building against; a stale conformance suite certifies the wrong thing.

## What a projection can quietly destroy

Two questions to ask of any generated artifact, answered honestly for these:

* **A newtype collapses on the wire.** `billing.invoice.Email` and `billing.email.EmailAddress`
  stay separate schema definitions, so code generators emit two types — but both are a bare JSON
  string on the wire, and a payload with the two values swapped validates clean. JSON Schema
  constrains structure; it cannot carry nominal identity.
* **A command's HTTP path is a convention.** The model has no `exposures:` construct yet, so
  `/invoices/commands/create-invoice` is a shape the generator chose — written into the generated
  document's own description rather than left for a reader to infer.

And one check that is scoped rather than total: every schema the generated OpenAPI and AsyncAPI
documents *embed* is validated against the real JSON Schema 2020-12 meta-schema, but the envelopes
around them are checked structurally, not against the OpenAPI/AsyncAPI meta-schemas — neither is
vendored here. What is unchecked is the envelope, not the types.
