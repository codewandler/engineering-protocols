---
title: "0.7 — one specification, two applications"
description: >
  Release 0.7.0-ess-wave-7 closes the loop over generated code: every generated artifact carries the
  digest of the model slice it derives from, ten construct families join the semantic delta — and the
  wave ends with a demonstration rather than a claim: one specification synthesised to Rust and to Go,
  two binaries answering the same seven HTTP exchanges identically and publishing byte-identical
  contracts.
slug: one-spec-two-apps
tags: [release, ess]
---

Release `0.7.0-ess-wave-7` is the wave where the machinery built over six releases stops pointing at
itself. It opens with bookkeeping — every generated artifact now carries the digest of the model
slice it derives from, and the semantic diff learns four more construct families — and it ends with
the demonstration all of that bookkeeping was for: one application specification, synthesised to
Rust *and* to Go, two binaries built and started **in the gate**, writing one startup record,
answering seven HTTP exchanges identically, and publishing the same contract byte for byte. Nothing
below is asserted from memory: every output was produced by running the released code, and the block
that was not run for this post says which committed file it is.

{/* truncate */}

## An artifact now knows what it derives from

Wave 6 left an honest gap, stated on this site at the time: the diff did not know about generated
code, so any change to a specification owed the whole generated tree. W7.1 closes it with one field.
Every generated artifact — the 36 committed projections, each conformance suite, each synthesised
workspace — carries a `contract_digest` beside its whole-model `source_digest`: the digest of the
artifact's *slice*, its seed constructs closed over everything they rest on, by the same dependency
graph `ess impact` walks. The head of the committed OpenAPI document, verbatim:

```yaml title="generated/openapi/invoice-service.yaml"
# generated from billing v3
# model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
# contract digest 349f5a6ba27e3e94aba00b700b07454ac9f44021daa1385d6df99e9d4dc83bdd
# compiler 0.1.0 · generator 0.1.0
# do not edit: regenerate with `protocol ess generate`
```

The slice rule leans big on purpose — a too-big slice costs a regeneration nobody needed, a
too-small one costs a false "still current", and those are not comparable errors. The polarity is
wave 5's, unchanged: an artifact absent from the answer was *not reached*, never "still current",
and everything the analysis cannot follow is owed, stated as such — unreadable provenance, a
contract digest the slice does not compute, a committed file the model derives nothing at. A suite
whose own contract digest its model does not compute is refused outright, because the short list it
would produce looks exactly like a correct short list.

## Ten families in the delta

W7.2 executes gap register D-1: entities, commands, views and bindings join the comparison — ten
construct families now, 74 new typed change kinds. On the fixture pair that shipped with wave 5,
grown to six changes for this wave (`protocol ess impact --from examples/revision-pair/before --to
examples/revision-pair/after`, run for this post):

```text
6 change(s): 2 widening, 2 narrowing, 2 other

  widens   type catalog.pricing.Currency: variant `CHF` added
           type/catalog.pricing.Currency/variant-added/CHF
  narrows  type catalog.pricing.Currency: variant `GBP` removed
           type/catalog.pricing.Currency/variant-removed/GBP
  changes  entity catalog.pricing.PriceList: invariants [floor.amount >= 0] → [floor.amount > 0]
           entity/catalog.pricing.PriceList/invariants-changed
  changes  command catalog.pricing.CreatePriceList: outcome `created` is decided by `when floor.amount >= 1`, was `when floor.amount > 0`
           command/catalog.pricing.CreatePriceList/outcome-condition-changed/created
  narrows  actor catalog.pricing.Auditor: may no longer invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList
  widens   actor catalog.pricing.PricingManager: may invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.PricingManager/grant-added/catalog.pricing.RetirePriceList

15 construct(s) reached: 5 changed, 8 depend on one directly, 2 through another
16 of 22 generated artifact(s) owed regeneration
```

Read the two middle changes. A strengthened invariant and a moved guard used to arrive as an *empty
delta* that put everything back to owed, through the fail-closed catch-all. Now each is a named
change — with both predicates rendered and **no direction**, because predicate comparison here is
conservative canonical equality: two spellings the parser normalises to one form are no change,
anything canonically different is *changed*, and whether the new predicate implies the old stays
refused rather than guessed. And the last line is W7.1 and W7.2 meeting: sixteen of the
twenty-two artifacts this specification derives are owed, each with the path that reached it one
hop per line — and the six that are absent were not reached, which is all the answer ever says.

## Go tells the truth about itself

W7.3 was the seam proving itself: a second emitter behind the same language-neutral plan, and
`PLAN.md` and `plan.json` byte-identical in both trees — the planner gained not one line. Go was
the right adversary because it has no sum type, so every closed set had to be encoded honestly or
refused out loud. From the committed tree, verbatim:

```go title="generated/go/gatepass/types/visit/visit.go"
// Building is Building — `gatepass.visit.Building`: one of a closed set of names.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type Building interface {
	isBuilding()
}
```

That doc comment is the wave's discipline in four lines. What Go holds more weakly than Rust is
never silently downgraded: each module carries a `TARGET.md` beside the plan — for billing, **four
weakenings, zero target refusals** — naming exactly what is lost (no exhaustiveness check on a
`switch`, a zero value no constructor produced, refinement answering `(value, ok)` where Rust's is
total, `==` undefined where the representation holds a list, a map or bytes), each also stated in
the generated doc comment where a reader meets it. The two things Go cannot represent at all — a
`Map<Bytes, _>`, and two obligation seams deriving one method name — are **target-stage refusals**,
marked as the target's so they can never read as facts about the model, and each is proven on a
fixture rather than on prose.

## The billing system, in a page that holds no model

W7.3b is the same claim continued to a third target: `protocol ess synthesize --target web` emits a
`WebAssembly` bridge over the Rust target's crates — three exported functions passing JSON over
linear memory, no `wasm-bindgen`, no build tool, no third-party crate, because a gate step that
resolves a crate is a gate step that reaches the network — and one page that drives it. Open the
committed `generated/web/billing/` and you can send any declared command from a typed form, watch
the outcome it took, read the event log, redeliver an occurrence to see the duplicate
`at_least_once` explicitly permits, and read every declared view's rows.

The realization underneath: **nothing about any system is typed into the HTML.** The command list,
the input controls, the event names, the views and the lifecycles are built at load time from a
`catalog.json` the module carries, and a test asserts the page names no construct of any
specification — the UI cannot drift from the model because it never contained it. Built without a
realization, every command answers the typed refusal naming the obligation the plan owes, and the
page shows that obligation's contract beside it: the honest empty state, not an empty screen. Six
weakenings are in this target's `TARGET.md`, none of them about a language — an `Integer` past 2^53
is rounded by the browser, redelivery is a request a person makes because nothing in a page
advances a clock. The gate builds the module for `wasm32-unknown-unknown`, checks the page's export
references against the module's own export table, and drives one round trip through the page's own
`bridge.js` under Node, holding seventeen claims.

## One specification, two applications, one surface

Then the demonstration, W7.5. [`examples/gatepass/`](https://github.com/codewandler/engineering-protocols/tree/main/examples/gatepass)
is a new application specification — visitor passes for a building: one domain, one component,
three commands each with a declared refusal, two views, a three-state lifecycle, and every
primitive the model has reaching the wire. Its plan: 29 capabilities — 22 generated, 5
obligations, 2 refused.

The model gained exactly one word for it:

```yaml
components:
  - component: pass-service
    reached_by: network        # or `in_process`, which is what silence has always meant
```

`network` names no protocol. What follows is a *derivation*, not a preference: a surface whose
callers are not deployed with it has to exist on a wire, and the only contract this repository
projects for a component's command surface is the `OpenAPI` document — an HTTP contract. A
synthesised server speaking anything else would contradict the document committed beside it. A
specification that says nothing keeps everything it had: the word is skipped from serialisation
when unstated, so billing's digest is the same string it was and all 36 committed projections have
zero drift.

Both emitters grow the transport with **zero dependencies** — Rust hand-rolls HTTP/1.1 over
`std::net::TcpListener`, Go serves over `net/http` — and neither invents a route: the route table
and the status table live once, in `ess_gen::http`, and the published document, the Rust server and
the Go server all read them. Start the Rust application (run for this post, against the committed
tree linked with its hand-written realization; the record below is the process's own three lines,
pretty-printed, with the seven-entry route list of the second line elided):

```json
{"log": "ess/1", "event": "system.starting", "system": "gatepass", "version": "v1",
 "model_digest": "f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61",
 "contract_digest": "e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e",
 "components": ["pass-service"],
 "capabilities": {"generated": 22, "obligations": 5, "refused": 2},
 "runtime": {"address": "127.0.0.1:38593", "language": "rust", "port": 38593}}
{"log": "ess/1", "event": "surface.serving", "component": "pass-service",
 "reached_by": "network", "transport": "http/1.1", "routes": 7, "paths": ["…"],
 "runtime": {"address": "127.0.0.1:38593", "language": "rust", "port": 38593}}
{"log": "ess/1", "event": "system.ready", "system": "gatepass", "surfaces": 1,
 "runtime": {"address": "127.0.0.1:38593", "language": "rust", "port": 38593}}
```

Everything outside `runtime` is derived from the specification. The Go application's record differs
in `runtime` alone — `{"address": "127.0.0.1:43559", "language": "go", "port": 43559}` on that run
— and the gate does not compare a list of members: it **removes** `runtime` and refuses a line that
has none, so a member the record gains tomorrow is compared without anyone editing the comparison.

Then both applications are driven through the same seven exchanges. The table is the gate's own
list ([`xtask/src/main.rs`](https://github.com/codewandler/engineering-protocols/blob/main/xtask/src/main.rs),
`DEMONSTRATIONS`); the statuses and the body equalities were re-verified for this post by starting
both binaries and comparing every answer as a value, because a JSON object is unordered and the two
languages build one through two writers:

| what the exchange proves | request | both answer |
|---|---|---:|
| a visit is registered | `POST /visits/commands/register-visit` | **202** |
| a visit of no length is refused, on domain grounds | `POST /visits/commands/register-visit` | **422** |
| the read-your-writes projection holds the visit just registered | `GET /visits/views/expected` | **200** |
| and so does the unfiltered one, with every field the row declares | `GET /visits/views/by-id` | **200** |
| a body the schema refuses is a bad request, not a domain refusal | `POST /visits/commands/register-visit` | **400** |
| a path the contract does not declare is answered by neither | `GET /visits/commands/cancel-visit` | **404** |
| a declared path under an undeclared method | `GET /visits/commands/register-visit` | **405** |

Identical status *and* identical body, all seven, from both languages. The refusals are worth
reading — this is the Go binary's 405, byte-equal to the Rust one's:

```json
{"refused":"this path answers `POST`, and the contract declares no other method for it"}
```

None of 400, 404, 405 or 501 is a status the contract declares, and none should be: each is a fact
about a transport rather than about a command. And the surface cannot drift from its paperwork,
because the paperwork *is* the surface: `GET /openapi.json` serves the committed contract and
`GET /docs` the committed Markdown domain page, both embedded at emission rather than rebuilt at
run time — a server that regenerated its own contract could publish one nobody reviewed. On the
verification run, both documents were byte-identical between the two applications and to the
committed files. The two realizations were written from the specification, not from each other —
which is what makes "they answer the same way" a claim about the specification rather than about a
copy.

All of this runs on every gate: both binaries built, started on ephemeral ports, compared, killed
and reaped. Six mutations were held against the step while it was built; six named failures, two of
them caught by the demonstration itself disagreeing.

## What the demonstration leaves out

Said here so it cannot be inferred away. **W7.4 — obligations as artifacts and tasks — stays
deferred by operator decision**; its precondition, a contract digest that exists in code, is now
met, so scheduling it is a decision rather than a build. The demonstration has **no
authentication and no TLS** — the model states none, `x-ess-may-invoke` still says who may invoke
what, and enforcement belongs to the layer that knows who is calling. The generated servers take
**one connection at a time**; there is no `servers` block because the model has no URL. The
committed gatepass conformance suite — 12 scenarios, generated and drift-checked like every other —
is **not** run against the two applications: the conformance runner is in-process, nothing here
speaks to an implementation over a socket, and the wire demonstration and the suite are two
separate proofs rather than one. And the browser target refuses this specification out loud rather
than emitting a page for it: a tab binds no socket, so a network surface is one a page would call
rather than contain — a fourth target, not this one.

The gate behind all of it, re-run for this post: nine steps, **94 suites and 1693 tests, 0
failures**, with 0 clippy warnings and 0 rustdoc warnings — and `task check` now needs the Go
toolchain, the `wasm32-unknown-unknown` target and Node beside Rust's own, and says which is
missing rather than skipping.

The full record for the wave — the two re-scopings, the encoding decisions row by row, and what
is deliberately not in it — is in the repository under
[`docs/plan/ess-wave-7-closing-the-loop.md`](https://github.com/codewandler/engineering-protocols/blob/main/docs/plan/ess-wave-7-closing-the-loop.md).
Tag: [`0.7.0-ess-wave-7`](https://github.com/codewandler/engineering-protocols/releases/tag/0.7.0-ess-wave-7).
