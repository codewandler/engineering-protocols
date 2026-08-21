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
listed instead of written, for the same reason `protocol ess generate` behaves that way — a verb
that scatters files over a working tree the first time someone tries it is a verb nobody tries
twice.

## The plan: every capability gets exactly one disposition

Synthesis starts with a language-neutral **synthesis plan**. Every capability of the specification
receives exactly one of three dispositions, with the reason recorded:

| Disposition | Meaning |
|---|---|
| **generated** | the specification determines it fully; the emitter writes it |
| **obligation** | the specification cannot determine it — a decision or an algorithm — so it is named and left to a person, with a declared seam to implement against |
| **refused** | the specification cannot even state what would be needed; the reason is printed |

The first line of the run is the plan in one sentence, and the reasons follow it:

```console
$ protocol ess synthesize --path examples/billing --target rust | head -4
billing v3 — 45 capabilities: 33 generated, 8 obligation(s), 4 refused, model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  obligation: command behaviour `billing.email.SendEmail` — decided outside the system: the provider rejects the recipient address
  obligation: command behaviour `billing.invoice.CancelInvoice` — the contract is declared; the algorithm is not
  obligation: command behaviour `billing.invoice.CreateInvoice` — the contract is declared; the algorithm is not
```

A refusal reads the same way and says what it cannot state: *"actor grants
`billing.invoice.Auditor` — a grant is checked against a caller identity, which types do not
carry"*.

The plan is rendered as `PLAN.md` and `plan.json` in every emitted tree, and it is
**byte-identical across all three targets** — the same 45/33/8/4 line and the same `plan.json`
digest come back from `--target rust`, `--target go` and `--target web`. Choosing an emitter never
changes the plan, only what is made of it.

What a target holds more weakly or cannot represent at all is declared in a `TARGET.md` beside the
plan — a named weakening, never a silent downgrade. The Rust target is the one the others are
measured against and emits no such file; Go's names four weakenings and the browser's six, each
with the capabilities it touches. The browser tree's first row is the worked example: it cannot
carry `#![forbid(unsafe_code)]`, because a WebAssembly export is a `#[no_mangle]` item and rustc's
own `unsafe_code` lint flags one. The file says so, states that the crate contains no `unsafe`
block, no `unsafe fn` and no raw-pointer dereference, and a test asserts the property the lint
would have closed. What is lost is the compiler closing the question, not the property.

## The three emitters

| Target | Emits | Dependencies |
|---|---|---|
| `rust` | a cargo workspace: semantic types, typestate lifecycles, component ports, one HTTP transport | none |
| `go` | a Go module with the same system | standard library only |
| `web` | a WebAssembly bridge over the Rust target plus a page built at load time from an emitted `catalog.json` — no model is typed into its HTML | no build tool, no `wasm-bindgen` |

The zero-dependency constraint is deliberate: generated code that pulls third-party crates makes
every downstream build reach the network and inherit someone's version policy.

`examples/gatepass/` is emitted to Rust and Go and deliberately not to the browser: it is a
component whose own words say its callers are not deployed with it, and a surface reached over a
network is one a page would *call* rather than contain.

## Realizations: the human's half

An **obligation** is implemented in a separate, hand-written crate or module — a *realization* —
that plugs into the generated seams. Three ship here: `examples/billing-realization/`,
`examples/gatepass-realization/` and `examples/gatepass-go-realization/`, one implementation per
obligation in the generated plan, linked into the generated tree. The linker never chooses between
candidate implementations; ambiguity is an error.

## How the output is proven, not assumed

The generated code is judged by the suite the same specification generated
(see [Verify an implementation](./verify-conformance.md)), and the repository's gate runs the whole
argument on every commit — `cargo xtask synth --check`, one of the ten steps of `task check`:

* The committed billing suite, **unchanged**, passes the generated workspace linked with the
  hand-written realization — 29 of 29 scenarios — and a deliberately corrupted linkage fails
  exactly the scenario that exists to catch it.
* The committed trees under `generated/rust/`, `generated/go/` and `generated/web/` are
  regenerated and compared byte-for-byte, then built: `cargo check`, `gofmt`/`go build`/`go vet`,
  and `cargo build --target wasm32-unknown-unknown` plus a Node-driven boundary test that loads the
  committed module outside a browser and drives it through the page's own glue. Its last line is
  `browser boundary: 17 claims held — catalogue, dispatch, transport, view, refusal, redelivery`.
  None of those three checks skips when its toolchain is missing: it fails and names it, because a
  skipped check reads exactly like a passing one.
* The **dual-target demonstration**: `examples/gatepass/` is synthesized to Rust *and* Go, both
  binaries are started on ephemeral ports, and their startup records, their answers to seven HTTP
  exchanges, and the `/openapi.json` and `/docs` documents they publish are compared. The seven are
  chosen to separate the kinds of "no": a registered visit (202), a visit of no length refused on
  domain grounds (422), two view reads (200), a body the schema refuses (400), an undeclared path
  (404) and a declared path under an undeclared method (405). The two applications must agree with
  each other and with the committed artifacts, byte for byte where bytes are claimed.

## Where the HTTP surface comes from

The gatepass model gained exactly one word to become a running server: a component may declare
`reached_by: network`, which states where its callers are and names no protocol. HTTP follows
because the one contract this project projects for a command surface is an OpenAPI document — the
transport is *derived*, which is the [semantics-over-transport](../concepts/design-principles.md)
principle doing its job.

## Honest limits

* **Generated code is structural, never behavioural.** Every algorithm is an obligation.
* **Obligations are plan entries, not yet artifacts** a task can own and evidence can close. That
  extension is W7.4, and it is deferred by operator decision rather than blocked:
  `docs/plan/ess-wave-7-closing-the-loop.md` § *W7.4 — deferred by operator decision* records that
  nothing else in wave 7 depends on it and that its one precondition — a contract digest that
  exists in code — is now met. What closes it is scheduling it, which is a decision somebody takes,
  not a build somebody is waiting on. It is on the [roadmap](../status/roadmap.md) under *Deferred
  by decision*.
* **The demonstration is not a deployment**: plain HTTP, no auth, no TLS, one connection at a time,
  no `servers` block because the model has no URL.
