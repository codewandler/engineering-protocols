---
title: Synthesize code from a specification
sidebar_position: 7
description: The synthesis plan, the three emitters behind it, obligations as the contract with the human, and how the generated code is proven against the generated suite.
---

# Synthesize code from a specification

Structural synthesis generates the part of an implementation that was never yours to write — types,
typestate lifecycles, component ports, one transport — and hands back everything it will not guess
as a **named obligation**. Behaviour is never generated: every algorithm is an obligation someone
implements.

```console
$ protocol ess synthesize --path examples/billing --target rust --out out/
```

`--target` is `rust`, `go` or `web`; `--out` writes the tree, and without it the artifacts are
listed instead of written.

## The plan: every capability gets exactly one disposition

Synthesis starts with a language-neutral **synthesis plan**. Every capability of the specification
receives exactly one of three dispositions, with the reason recorded:

| Disposition | Meaning |
|---|---|
| **generated** | the specification determines it fully; the emitter writes it |
| **obligation** | the specification cannot determine it — a decision or an algorithm — so it is named and left to a person, with a declared seam to implement against |
| **refused** | the specification cannot even state what would be needed; the reason is printed |

For `examples/billing/`: 45 capabilities — 33 generated, 8 obligations, 4 refused. The plan is
rendered as `PLAN.md` and `plan.json` in every emitted tree, and it is **byte-identical across all
three targets**: choosing an emitter never changes the plan, only what is made of it.

What a target holds more weakly or cannot represent at all is declared in a `TARGET.md` beside the
plan — a named weakening, never a silent downgrade. (Example: the browser target cannot carry the
`unsafe_code = "forbid"` lint, because a WebAssembly export requires `#[no_mangle]`; its `TARGET.md`
says so, and a test asserts the property the lint would have closed.)

## The three emitters

| Target | Emits | Dependencies |
|---|---|---|
| `rust` | a cargo workspace: semantic types, typestate lifecycles, component ports, one HTTP transport | none |
| `go` | a Go module with the same system | standard library only |
| `web` | a WebAssembly bridge over the Rust target plus a page built at load time from an emitted `catalog.json` — no model is typed into its HTML | no build tool, no `wasm-bindgen` |

The zero-dependency constraint is deliberate: generated code that pulls third-party crates makes
every downstream build reach the network and inherit someone's version policy.

## Realizations: the human's half

An **obligation** is implemented in a separate, hand-written crate — a *realization* — that plugs
into the generated seams. `examples/billing-realization/` is the model: one implementation per
obligation in the generated plan, linked into the generated workspace. The linker never chooses
between candidate implementations; ambiguity is an error.

## How the output is proven, not assumed

The generated code is judged by the suite the same specification generated
(see [Verify an implementation](./verify-conformance.md)), and the repository's gate runs the whole
argument on every commit:

* The committed billing suite, **unchanged**, passes the generated workspace linked with the
  hand-written realization — 29 of 29 scenarios — and a deliberately corrupted linkage fails
  exactly the scenario that exists to catch it.
* The committed trees under `generated/rust/`, `generated/go/` and `generated/web/` are
  regenerated and compared byte-for-byte, then built: `cargo check`, `gofmt`/`go build`/`go vet`,
  and `cargo build --target wasm32-unknown-unknown` plus a Node-driven boundary test with seventeen
  asserted claims.
* The **dual-target demonstration**: `examples/gatepass/` is synthesized to Rust *and* Go, both
  binaries are started on ephemeral ports, and their startup records, their answers to seven HTTP
  exchanges, and the `/openapi.json` and `/docs` documents they publish are compared — the two
  applications must agree with each other and with the committed artifacts, byte for byte where
  bytes are claimed.

## Where the HTTP surface comes from

The gatepass model gained exactly one word to become a running server: a component may declare
`reached_by: network`, which states where its callers are and names no protocol. HTTP follows
because the one contract this project projects for a command surface is an OpenAPI document — the
transport is *derived*, which is the [semantics-over-transport](../concepts/design-principles.md)
principle doing its job.

## Honest limits

* **Generated code is structural, never behavioural.** Every algorithm is an obligation.
* **Obligations are plan entries, not yet artifacts** a task can own and evidence can close — that
  extension (W7.4) is deferred by decision.
* **The demonstration is not a deployment**: plain HTTP, no auth, no TLS, one connection at a time,
  no `servers` block because the model has no URL.
