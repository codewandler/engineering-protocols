# ESS wave 7 — the loop closed over generated code

> **In progress. Scheduled 2026-08-20 by operator instruction, re-scoped twice by the same
> authority on 2026-08-21: W7.3 (the Go emitter) was deferred by the sequencing decision that
> pulled the infra waves forward and **revived the same night** (roadmap commit `1e3bbac`), so
> W7.1, W7.2, W7.3 and its browser continuation are delivered, W7.5 — the dual-target
> demonstration — is delivered, and W7.4 (obligations as artifacts) stays deferred and unscheduled
> on the roadmap.** Design:
> [`ess-semantic-diff-impact-evolution-design-v0.1.md`](../design/ess-semantic-diff-impact-evolution-design-v0.1.md)
> §33, which was blocked on `contract_digest` existing in no code. W7.1 makes it exist. Where that
> section's verdict vocabulary contradicts gate G19's polarity — its `still valid` and
> `unchanged` answers — G19 wins, exactly as it did in wave 5.

**Goal: a generated artifact knows which part of the model it derives from, and `ess impact`
narrows "the specification moved, everything is owed" to the artifacts whose slice moved — without
ever saying an artifact still stands.**

## W7.1 — the diff learns about generated artifacts. Delivered.

`contract_digest` became code. Every generated artifact — the 36 projections under `generated/`,
each committed conformance suite, each synthesised workspace — now carries, beside the whole-model
`source_digest`, the digest of the **model slice** it derives from, stamped through the one
provenance mechanism every generator already used (`ess_gen::Provenance`).

### The slice rule, and why it leans big

An artifact's slice is its seed constructs closed over everything they rest on, by the same
dependency graph `ess impact` walks — one graph, one walk, moved from `ess-diff` down into
`ess-compiler` (`graph`, `refs`) so the crate that stamps a digest and the crate that asks "did
this slice move" cannot drift apart. The membership rule resolves every doubt by including more:

| rule | why |
|---|---|
| sub-constructs travel with their parents — a command brings its outcomes, an entity its moves | an artifact derived from a command was derived from its branches; omitting them claims "still current" past a change to an error an outcome refuses with |
| the system header, naming, conversions and workloads are in **every** slice | none of them has an `EssSemanticRef` a slice could name, so a change there cannot be attributed — and the only digest that does not lie about an unattributable change is one that moves for every artifact |
| documents that read by inversion seed wide — `OpenAPI` takes every actor and binding, `AsyncAPI` every binding, command and component, a domain page its whole context plus the bindings and components | "which actors are relevant to this document" is itself an answer that changes; an actor granted its first accepted command tomorrow must move the digest today |
| the suite, the synthesised workspaces, the indexes and the system-wide docs pages are whole-model | legitimately: each renders or obliges something for every construct |

The asymmetry that justifies the lean: a too-big slice costs a regeneration nobody needed; a
too-small one costs a false "still current". Those are not comparable errors.

### The impact answer

`protocol ess impact --from A --to B [--suite S] [--generated DIR]` now writes `ess-impact/2`:
`--suite` became optional, the document gained an `artifacts` section, and the churn carries
`generated_artifacts_total` / `generated_artifacts_owed`. The artifact answer has wave 5's exact
polarity, enforced by the same shapes: `ArtifactAnswer` is `Whole` or `Narrowed` and nothing else,
an artifact absent from the answer was **not reached** — never "still current" — and everything the
analysis cannot follow is owed, stated as such:

* a change to the specification itself (no construct to seed a closure at) owes every artifact;
* a move in a family the delta does not compare owes every artifact;
* a committed artifact whose provenance cannot be read — every pre-wave-7 artifact included — is
  owed as `provenance-unreadable`;
* one whose contract digest is not what its slice computes against `--from` is owed as
  `contract-mismatch`, a false claim about derivation;
* a committed file the model derives nothing at is owed as `unfollowed`;
* a suite whose own contract digest its model does not compute is **refused**, not narrowed — the
  short list it would produce looks exactly like a correct short list.

Every narrowed obligation carries the path from the artifact's seeds to the changed construct, one
hop per line, same as scenario impact.

### The check that bites

`generate-check`, `suite-check` and `synth-check` all flow through one `sync` comparison in
`xtask`, and that one place now reads the contract digest out of both the committed and the freshly
generated artifact: a mismatch fails the gate with its own sentence — *a stale contract digest is a
false claim about the model slice it derives from* — beside the plain byte-drift message. No tenth
step; the same three steps got sharper teeth.

### Proven on the fixture pair

`crates/ess-diff/tests/artifacts.rs`, by name: the revision pair's four-change delta narrows the
owed artifacts to a strict subset — the `Currency` changes reach the `Money`, `Headline` and
command schemas, the domain page and both API documents, while the `PriceListId` and event schemas
are absent; a grant change reaches the `OpenAPI` document (which renders grants) and not the
`AsyncAPI` one (which reads no actor); and a system-header change owes every artifact. The paths
are asserted hop by hop where they are load-bearing.

## W7.2 — entities and commands join the delta. Delivered.

Gap register D-1, executed. Entities, commands, views and bindings entered the comparison — ten
families now, in design §60's own order (`system, type, entity, command, event, error, view,
actor, component, binding`) — as 74 new typed change kinds in the established taxonomy, with no
untyped catch-all and each family documented complete over the `Resolved*` struct it compares.
The directional relations stay exactly the four that existed; every new kind's `relation` is a
`const fn` returning *changed* without reading the content, so a direction cannot be smuggled in.

### The canonical form, precisely

A predicate's canonical form is the parsed
[`Predicate`](../../crates/aep-domain/src/predicate.rs) exactly as the compiler resolves it: the
parser's own simplifications applied (`not not p` → `p`, an empty `all` → *always*, a singleton
`all`/`any` unwrapped, whitespace and YAML formatting gone), and **nothing else** — no reordering
of `all`/`any` children, no algebraic rewriting, no implication. Two predicates are canonically
equal iff the parsed values are structurally equal (`Predicate`'s derived `Eq`). Equal says
nothing; different says *changed*; no third answer exists. Where the model also keeps the author's
statement beside the parsed predicate — an entity's or a type's invariants — the statement is part
of the canonical form, because a documentation projection quotes it: a respaced `when:` is
therefore silence (the IR keeps only the AST) while a respaced invariant statement is *changed*
(the model keeps the spelling), and both directions are proven on fixtures
(`a_guard_respaced_is_the_same_predicate_and_no_change`,
`an_invariant_statement_reworded_without_moving_the_predicate_is_still_a_change`). The asymmetry
is D-1's: over-recognition would be a false "nothing moved", under-recognition costs a re-run.

### The fail-closed arm shrank honestly — and closed a hole

Mechanism 6's uncompared-family equality check lost entities, commands, views and bindings and now
reads what still has no family: the **conversions**, the **workloads**, and — newly — each
**domain's naming**. The domain arm closes a fail-open gap found while shrinking the list: a
domain document can set a wire name, a display name and a summary, no family compares a domain,
and the wave-5 list did not serialise domains either, so that exact edit produced an empty delta
*and* an empty narrowing. Only the naming is read, deliberately: a domain's membership sets are
derived from the constructs' own declarations, each compared by its own family, and folding them
in would send every added type to `Whole` and erase the narrowing. The mechanism itself stays as
the backstop for the construct the model gains next.

### Proven on the fixture pair

`examples/revision-pair/` grew to six changes: an entity invariant strengthened
(`floor.amount >= 0` → `> 0`) and an outcome guard moved (`floor.amount > 0` → `>= 1`), each
reported as *changed* with both predicates rendered and no direction. The suite is ten scenarios;
the invariant edit owes the nine that touch an instance and not the rejected creation, the guard
edit owes all ten through the command every scenario creates through, and the artifact answer
narrows differently per edit — the entity edit reaches every command schema (each outcome acts on
the entity), the guard edit reaches only the guarded command's own schema, and neither reaches a
type schema. The mutation that used to prove the catch-all (erasing a payload mapping) now proves
its shrink: it arrives as `command/billing.invoice.PayInvoice/outcome-payload-changed/settled` and
narrows, while a workload flip and a domain-naming edit still fall to `Whole`.

`ess-diff/1` is unchanged: the document's shape did not move, the new change kinds are additive
rows in the existing list, and the old six categories keep their relative order under §60 — so a
delta this build wrote before W7.2 still reads back, and the format label keeps meaning what it
meant.

## W7.3 — a second emitter, and the seam proves itself. Delivered.

**Go, and the plan did not move.** `crates/ess-synth/src/go/` consumes a finished `SynthesisPlan`
through exactly the surface `src/rust/` uses — `is_generated`, `obligation_of`, `generated`,
`obligations` — and `src/plan.rs` gained not one line to admit it. The claim is checked rather than
asserted, twice: `the_plan_is_byte_identical_in_both_targets_trees` compares the emitter's output,
and `the_two_targets_commit_the_same_plan_byte_for_byte` compares the **committed** files, so
`generated/go/billing/PLAN.md` and `generated/rust/billing/PLAN.md` are the same bytes and so are
the two `plan.json`.

Go was the right adversary because it has no sum type. Every tagged union, every enum, every
command outcome set and the transport's own event log had to be encoded by hand or refused out
loud, and there was no `enum` keyword to hide behind.

### The encoding decisions, one row each

| construct | Go | why not the obvious alternative |
|---|---|---|
| enum, tagged union, command outcome, outbox, event log | **sealed interface**: one unexported marker method (`isPayee()`), one exported struct per variant | `type Channel string` with constants admits `Channel("whatever")`, so the set is not closed. An unexported method cannot be implemented from another package, so it is |
| typestate lifecycle | **one type per state** (`InvoiceInDraft`), transitions as methods on exactly the states that declare them, returning the next state's type | Go cannot declare a method for one instantiation of a generic type, so `Invoice[Draft].Issue()` has no spelling. The guarantee survives the change of construction: an undeclared move is a method that does not exist |
| newtype | **struct with an unexported field**, a constructor and a `Value()` accessor | `type Email string` accepts `var e Email = "anything"` — an untyped constant assigns straight in, and the distinctness the newtype exists for is gone. The accessor is not decoration: a conversion in another package has to read the representation |
| obligation | **interface** per owed capability, one `Unimplemented` per package returning the typed refusal, bijective with the plan | identical to Rust in substance. Two file moves are forced by Go's rules, not by the plan: `UnmetObligation` lives in a package that imports nothing (Go refuses an import cycle where Rust allows a module cycle), and owed conversions get their own package because they name both ends |
| transport | the same in-process at-least-once log, cursor, pump, `Redeliver`, retry hold-back and escalation-through-its-obligation | derived from the same declarations, so both targets write down the same conclusion. One shape differs: each generated delivery is its own method, because Go declares a variable once per block and two bindings reacting to one event would redeclare `input` |
| the typed refusal | `(T, *obligation.UnmetObligation)`, not `(T, error)` | `error` is an open interface: anything could be returned through it. Rust's `Result<T, UnmetObligation>` says exactly one thing can go wrong here, and the concrete pointer keeps that |
| identifiers | allocated once, in one fixed sweep, with a deterministic `_` repair | Rust has several namespaces — a type, a function, a module and an enum's variants may all be `Invoice` — and Go has exactly one per package, so `NewEmail`, `InvoiceData` and `PayeeCompany` all compete with every declared type name |

### Parity, honestly — where the two targets differ

Per capability class. **Generated** means the emitted code carries what the first target's carries;
**weakened** means it is emitted with a named guarantee the language cannot hold, stated in the
generated doc comment *and* in `TARGET.md`; **target-refused** means the plan marks it generated
and this language cannot represent it, so it is not emitted and says so.

| capability class | Rust | Go |
|---|---|---|
| domain type — newtype | generated | generated, **weakened**: Go's zero value needs no constructor, and `==` is undefined where the representation is a list, map or bytes |
| domain type — struct | generated | generated, **weakened**: the same two |
| domain type — enum, tagged union | generated (`enum`) | generated, **weakened**: the set is closed, a `switch` over it is not exhaustively checked |
| entity lifecycle | generated (typestate over `S`) | generated, **weakened**: illegal moves are refused identically; `Refine` answers `(value, ok)` where Rust's is total, because the state is a sealed interface whose zero value names no state |
| command contract | generated | generated, **weakened**: the outcome set is closed, its `switch` unchecked |
| event type, error type, view type | generated | generated, **weakened**: zero value, equality |
| binding delivery | generated | generated, **weakened**: lifting a component's outbox entry onto the log is a `switch` Go cannot prove total, so a `nil` answer is dropped rather than logged, and the generated code says so |
| component port | generated | generated, **weakened** (exhaustiveness) — or **target-refused**, below |
| command behaviour, view query, binding escalation | obligation | obligation — same contract, same stub, same bijection |
| conversion, mechanical | generated (`impl From`) | generated — a package-level function, because Go has no `From` and a method on the source type would live in the package that must not know the destination |
| conversion, owed | obligation | obligation — in its own package, and the method names both ends |
| binding transformation | generated | generated |
| actor grants, workload | refused (**planning**) | refused (**planning**) — the same refusal, for every target, not a fact about Go |

Two things Go cannot represent at all, each a **target-stage** refusal marked
`RefusalStage::Target` so it can never read as a fact about the model, and each with a fixture that
exercises it:

| refused | why | fixture |
|---|---|---|
| `Map<Bytes, _>`, and everything that rests on it | a Go map key must be comparable and `[]byte` is not; rendering the key as text would claim the bytes are text. Rust's `BTreeMap<Vec<u8>, V>` is ordinary | `a_map_keyed_by_bytes_is_refused_at_the_target_stage_and_never_emitted` |
| a component port whose obligation seams derive one method name | Go gives a type one method set, so the interface bundling a component's obligations cannot embed two seams that both declare `Place`. A rename would be the emitter choosing a name the specification did not | `two_seams_of_one_component_that_derive_one_method_name_are_refused_not_renamed` |

A refusal **travels the way dependence does**: the command whose input holds the unrepresentable
type is refused, the port that accepts that command is refused, and every binding landing on that
port with it — rather than emitting a port with one handler quietly missing, which is the absence
this repository refuses to ship. The emitter's coverage assertion is widened by exactly that set
(`emitted == plan-generated minus target-refused`, and `stubs == plan-obligations minus
target-refused`), so a refusal cannot be used to hide a forgotten emission.

The billing specification has **zero** target refusals and four weakenings; both refusal classes
are reachable and are proven on fixtures rather than on prose.

### The gate grew Go teeth

`cargo xtask synth` writes and checks **both** trees from one plan, and then builds what it wrote:
`cargo check` inside the Rust workspace, and `gofmt -l` (empty), `go build ./...` and `go vet
./...` inside the Go module. The emitter writes already-`gofmt`-clean source rather than shelling a
formatter into the emission, so a file `gofmt` would rewrite is a defect in `ess-synth` and fails
the gate saying so. **The check never skips**: a missing Go toolchain is a failure that names it,
because a check that quietly passes without its toolchain reads exactly like one that passed. CI
installs Go on the `gate` job (whose `cargo test` runs `xtask`'s own tests, which write and build
both trees) and on the `synthesis` job, pinned to the `go` directive the emitter writes into every
generated `go.mod`.

`generated/go/` is a committed tree with an owner of its own, carved out of the projection task's
orphan scan the way `generated/rust/` is, and the ownership test refuses an uncovered nesting or an
unowned exclusion.

Three emitted shapes the billing example never reaches — an **owed** crossing (its own package,
because Go refuses the import cycle Rust's modules allow), an **owed transformation**, and the
`retry` failure policy — are covered by a second fixture rather than shipped untested
(`an_owed_crossing_gets_its_own_package_because_go_refuses_an_import_cycle`,
`an_owed_transformation_and_a_retry_policy_are_emitted_the_way_the_binding_declares_them`).

### What is deliberately not here

The dual-target demonstration the roadmap appends — one application specified in `examples/`,
synthesised to Go *and* Rust, both binaries starting with semantically identical log output and
serving the same API. It needs an application specification that does not exist yet and a runtime
neither emitter produces, and it is a slice of its own rather than a criterion this one silently
fails. Nothing in W7.3 blocks it: the two trees exist, both build, and both are drift-checked.

## W7.3b — the browser is the third target, and the page holds no model. Delivered.

Filed under W7.3's number rather than its own because it is the same claim continued: a third
emitter behind the same seam, and the plan gained not one line to admit it. `PLAN.md` and
`plan.json` are byte-identical in all three trees. `crates/ess-synth/src/web/` emits a
`WebAssembly` bridge over the Rust target's system — three `#[no_mangle]` exports passing JSON over
linear memory, no `wasm-bindgen` and no build tool, because a gate step that resolves a crate is a
gate step that reaches a network — beside one page whose command forms, event log, view tables and
lifecycles are built at load time from an emitted `catalog.json`. A test asserts the page names no
construct of any specification: the UI cannot drift from the model because it never contained it.
Six weakenings are in its `TARGET.md`, the gate builds the module for `wasm32-unknown-unknown`,
checks the page's export references against the compiled module's own export table, and drives one
round trip through the page's own `bridge.js` under Node, holding seventeen claims.

Full detail is in `CHANGELOG.md`; what belongs here is the number and the reason it is this one.

## W7.5 — one specification, two applications, one surface. Delivered.

The demonstration W7.3 named and deliberately did not attempt, executed rather than asserted:
**one application specification, synthesised to Rust and to Go, both binaries starting with the
same log and serving the same API.**

### What forces HTTP, and why it is not a preference

The wave-6 rule is that a system has exactly one transport and the transport is *derived from what
the specification states*. Billing's `delivery: at_least_once` on a binding derives an in-process
log. The demonstration needed a specification whose own words derive something else, and the model
could not say anything at all about how a component's surface is reached — so the model gained one
word, in the wave-4 tradition: raw → validated, a closed set, a code, a regenerated schema, and one
construct rather than a transport DSL.

```yaml
components:
  - component: pass-service
    reached_by: network        # or `in_process`, which is what silence means
```

`network` names no protocol. What follows is a *derivation*: a surface whose callers are not
deployed with it has to exist on a wire, and this repository projects exactly one contract for a
component's command surface — the `OpenAPI` document under `generated/openapi/` — which is an HTTP
contract. A synthesised server speaking anything else would contradict the document committed
beside it. Neither variant of [`Reach`](../../crates/ess-domain/src/component.rs) says `http`, and
adding one would be design §7's transport DSL, which this model has not taken.

| what the word closed | how |
|---|---|
| the model could not state where a component's callers are | `reached_by:` on a component, defaulting to `in_process`, skipped from the IR's serialisation when unstated — so **every committed artifact of every existing specification keeps its digest**, and billing's model digest is the same string it was before this wave |
| the `OpenAPI` projection refused to expose a view, because "nothing in the model says how one is read" | it still refuses — *unless* the component says something outside the process reads it. A view then gets `GET /{domain}/views/{view}`, its rows under one key, its declared filter in the description and its consistency as `x-ess-consistency`. No page size, no cursor, no ordering, no filter parameter: the specification states none |
| a server and a contract could disagree about a path | they cannot: `ess_gen::http::routes` is the one mapping, and `openapi.rs`, the Rust emitter and the Go emitter all read it. So is `ess_gen::http::status`, which is where 202, 409, 422 and 502 are decided once |
| a component could claim a surface with nothing on it | `reached_by: network` with no accepted command and no owned domain projecting a view is refused as `EmptyDeclaration`, naming what is missing |

### The application

[`examples/gatepass/`](../../examples/gatepass) — visitor passes for a building. One domain, one
component, **no binding**, so the one transport it has is the one its component's own words force.

| construct | what it holds |
|---|---|
| types | a newtype over `Uuid` and one over `String`, an enum, a tagged union, and two structs — one carrying `Optional<Timestamp>` and `Bytes`, one carrying `Decimal` |
| entity | `Visit`, ten fields including a `List`, a `Map` and an `Optional`, two invariants, a three-state lifecycle with two transitions |
| commands | three, each with a refusal: one guarded (`expected_minutes > 0`) and two `wrong_state:` branches |
| events | three, each with its payload declared from the command's input |
| views | two — one `read_your_writes` with a declared filter, one `eventual` carrying every kind the wire has |
| actors | a receptionist who may invoke all three, and a security auditor who may invoke nothing |

Every primitive the model has reaches the wire through it, which is what makes the comparison
below worth making: `Bytes` as base64, `Decimal`, `Timestamp`, `Duration` and `Uuid` as strings, an
`Integer` as a number, an absent optional member omitted rather than sent as `null`.

### The surface both applications serve

Seven routes, from one mapping: three commands as `POST`, two views as `GET`, plus
`GET /openapi.json` and `GET /docs`. The two documents are **embedded at emission** — the committed
`OpenAPI` document and the committed Markdown domain page, byte for byte — because a server that
regenerated its own contract could publish one the repository never reviewed. `/docs` is served as
`text/markdown; charset=utf-8` rather than rendered: rendering would be a second rendering of the
documentation, and the two would differ the first time either moved.

A path the contract does not declare is a `404`; a declared path under another method is a `405`; a
body the schema refuses is a `400`; an obligation nothing has satisfied is a `501` naming it.
None of those is a status the contract declares, and none should be — each is a fact about a
transport rather than about a command, which is the row `openapi.rs` has always carried.

Neither tree takes a dependency. Rust serves over `std::net::TcpListener` with about two hundred
fixed lines of HTTP/1.1 and the JSON reader the browser target already needed — now shared, one
emitter, two targets. Go serves over `net/http` and `encoding/json`, with generated codecs beside
the types, because a generated Go type carries an unexported field that `encoding/json` cannot see
and exporting it would undo the distinctness the newtype encoding exists for.

### The startup record, and the one member that may differ

Three lines of JSON on standard output before either application answers anything. Every member is
derived from the specification — **except `runtime`**, which is the process's own: the language it
was synthesised into, the address it bound, the port it took.

| line | carries |
|---|---|
| `system.starting` | the system, its version, the model digest, the contract digest, every component, and the plan's disposition counts |
| `surface.serving` | the served component, its declared reach, the transport, the route count, and every route as method, path, what it serves and the construct it serves |
| `system.ready` | the system, and how many surfaces this process serves |

The split is the whole comparison, and it is deliberately not a list of members to compare: the
gate **removes** `runtime` and refuses a line that has none, so a member the record gains tomorrow
is compared without anyone editing the comparison, and a member moved into `runtime` to make a
comparison pass would be a member that visibly stopped being compared.

### The proof, in the gate

`cargo xtask synth` builds both applications from the committed trees plus their hand-written
realizations — [`examples/gatepass-realization/`](../../examples/gatepass-realization) for Rust,
[`examples/gatepass-go-realization/`](../../examples/gatepass-go-realization) for Go, each with a
linker that resolves exactly one implementation per obligation and never chooses between two (gap
register D-2) — starts each on an **ephemeral** port, and:

* reads three startup lines from each, strips `runtime`, and compares;
* drives seven exchanges through both — a registration, a domain refusal, both projections, a body
  the schema refuses, an undeclared path and a declared path under the wrong method — comparing the
  status and the body **as a value**, because a JSON object is unordered and the two languages
  build one through two writers;
* fetches `/openapi.json` and `/docs` from both and compares them to the committed bytes and to
  each other;
* kills and reaps both, from a guard whose `Drop` runs on every path out.

The two realizations are deliberately not translations of each other. Both were written from the
specification, in the language of the tree each links into, which is what makes "they answer the
same way" a claim about the specification rather than about a copy.

### What is deliberately not here

The browser target is **not** emitted for this specification, and refuses its transport at the
target stage rather than silently: a page holds the system in one tab and binds no socket, so a
network surface is one a page would *call* rather than contain — a fourth target rather than this
one. There is no authentication (the model states none; `x-ess-may-invoke` still says who may
invoke what), no concurrency beyond one connection at a time, no TLS and no `servers` block,
because the model has no URL. And a command naming a subject that was never created still has no
declared outcome — `wrong_state:` demands a state the subject does not have — so both realizations
answer the typed refusal and the surface reports `501`. That is a gap in the *model*, recorded
against the billing realization before this wave and unchanged by it.

## W7.4 — deferred by operator decision

Obligations-as-artifacts stays described on [`ess-roadmap.md`](ess-roadmap.md) and is not
scheduled. Nothing in W7.1, W7.2, W7.3 or W7.5 depends on it; its precondition — a contract digest
that exists in code — is now met, so scheduling it later is a decision, not a build.
