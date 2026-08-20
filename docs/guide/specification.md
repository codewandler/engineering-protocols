# Specifying a system

For you if you want the contracts, tests and documentation of a system to be derived from one
document instead of maintained beside it.

An **Executable System Specification** describes a system semantically: `CreateInvoice` is a command,
and `POST /v1/invoices` is one way to expose it. That distinction is the whole design. It is what lets
the same specification compile to a modular monolith or to distributed services without the domain
model changing, and it is what makes a generated test a statement about the system rather than about
its HTTP layer.

## What exists today

| Works | Command |
|---|---|
| Parse a specification from one file or a directory | `protocol ess validate --path <path>` |
| Refuse a malformed one, naming every problem at once | same, exit 1 |
| Resolve every reference into a normalized IR | `protocol ess compile --path <path>` |
| Look one declaration up, resolved | `protocol ess inspect --path <path> <name>` |
| See the actor/command/event graph | `protocol ess graph --path <path> --format dot\|mermaid\|json\|yaml` |
| Derive documentation, schemas and contracts | `protocol ess generate --kind docs\|schema\|openapi\|asyncapi` |
| Derive the conformance suite it obliges | `protocol ess conform synthesize --path <path>` |
| Run that suite against an implementation | `protocol ess conform run --path <path> --target <name>` |
| Turn that run into AEP evidence | `protocol ess conform evidence --path <path> --target <name>` |
| Say what moved between two revisions of one specification | `protocol ess diff --from <path> --to <path> --format text\|json` |
| Say what that move puts back to owed in a committed suite | `protocol ess impact --from <path> --to <path> --suite <suite.json>` |
| Complete a task only once that evidence exists | [`examples/billing-conformance/`](../../examples/billing-conformance/) |
| Read the suites this repository commits | [`suites/generated/`](../../suites/generated/) |
| Validate in an editor as you type | [`schemas/generated/ess.schema.json`](../../schemas/generated/ess.schema.json) |
| Require conformance to one, as a protocol rule | [`principles/verification/ess-conformance.yaml`](../../principles/verification/ess-conformance.yaml) |

**A suite runs against the implementations this repository ships, and not yet against yours.** A
`ConformanceTarget` is a Rust trait, so `protocol ess conform run` reaches only what it was compiled
with: the two hand-written references inside `ess-conformance`. Holding your own system to a
specification means writing that adapter in Rust today — the suite is the same document either way.

### The graph, without generating a documentation tree

`protocol ess graph` prints the same picture the generated `docs/README.md` opens with — the actors,
the commands each component accepts, the events it publishes, and the bindings between them.

| `--format` | output |
|---|---|
| `dot` (default, and `text` still means it) | Graphviz, for `dot -Tsvg` |
| `mermaid` | a `flowchart`, unfenced, to redirect into a Markdown file or paste into a pull request |
| `json`, `yaml` | the nodes, edges and groups themselves |

```console
protocol ess graph --path examples/billing --format mermaid >> notes.md
```

One renderer produces both the CLI's diagram and the page's, so the two cannot drift apart;
`crates/protocol-cli/tests/graph.rs` compares them and fails if they do.

### What gets derived, and what each projection is for

| `--kind` | output | why it exists |
|---|---|---|
| `docs` | Markdown with Mermaid diagrams | the cheapest check that the model is complete: a construct with no rendering is a hole in a page a person reads |
| `schema` | JSON Schema per command input, event and error payload | the type system, projected without losing the distinctions it exists to make |
| `openapi` | one OpenAPI 3.1 document per component | the specification *is* the HTTP contract, not a document beside it |
| `asyncapi` | one AsyncAPI 3.0 document per component | the same for messaging, including what happens when a binding fails |

`--out` writes them; without it you get a listing, because a command that looks read-only should not
write into whatever directory you happened to be in. `cargo xtask generate --check` fails when the
committed output no longer matches the specification — a generated OpenAPI document that has drifted
is a contract someone is already building against.

Every artifact carries its provenance: the specification version, a digest of the resolved model, and
the compiler and generator versions. The digest is over the model rather than the source files, so it
does not change when a comment does — a digest that moves for no reason is one every reader learns to
ignore.

### Two things a projection can quietly destroy

Worth knowing because they are the questions to ask of any generated artifact here.

**A newtype collapsing into its representation.** `Email` and `EmailAddress` are both a `String`
underneath. The generated schemas keep them as separate definitions with separate references, so a
code generator emits two types — but on the wire both are a bare JSON string, and **a payload with
the two values swapped validates clean.** JSON Schema constrains structure; it cannot carry nominal
identity. That is a real limit, stated rather than papered over.

**A command becoming an endpoint.** `CreateInvoice` is a command; `POST /invoice/commands/create-invoice`
is one way to expose it. The model has no `exposures:` construct yet (design §6 sketches one), so that
path is a *convention the generator chose*, written down in the generated document's own description.
When `exposures:` lands it should override the convention, not replace it.

### Closing the loop: the suite a specification obliges

`generate` derives what an implementation is built *from*. `conform` derives what it is held *to*.

```console
protocol ess conform synthesize --path examples/billing
protocol ess conform run --path examples/billing --target billing
```

The first walks the IR and writes one scenario per thing the specification obliges: each declared
outcome, each lifecycle move, each move that must **not** be honoured, what must still hold of an
entity afterwards, what a value object declares of every value at each observable field position
that holds one, and each of the four claims a binding makes. 29 of them for `examples/billing/`.
The second executes them against an implementation and reports scenario by scenario.

**A construct the specification does not say enough about to test is refused, not omitted.** Billing
had one — a value object's own invariants are declared over a type rather than over an instance —
until wave 6.5 delivered the slice its refusal promised: `Money`'s `amount >= 0` is now read at each
view field position that holds a `Money`, rebased onto the position (`total.amount >= 0`, over every
row, with at least one row demanded). What genuinely has no witness keeps a refusal — the oracle
fixture prints six. A refusal is printed beside the scenarios that exist, because a suite quietly
holding fewer checks than the specification requires is the one failure a passing run cannot show.

| exit | what it means |
|---|---|
| `0` | every scenario passed |
| `1` | the implementation contradicted the specification, or could not expose what a required scenario checks |
| `3` | nothing contradicted the specification and at least one scenario could not be executed |

The third one is the distinction worth having: `1` says the system is wrong, `3` says nobody found
out. `protocol ess conform run --path examples/oracle-fixture --target billing` is `3`, because the
billing implementation has never heard of `oracle.order.PlaceOrder`.

`--untraced` shows the fourth word. §16 refuses to require every implementation to trace the commands
its bindings invoke, so a target that cannot is legitimate — and the one scenario that needs it comes
back `unsupported` rather than passing, and the run still fails. A check the target could not make is
not a check that passed.

`--inject <fault>` breaks one property on purpose and names the scenario that exists to catch it,
which is how a reader checks in one command that the suite bites:

```console
protocol ess conform run --path examples/billing --target billing --inject accept-invalid-amount
```

`cargo xtask suite --check` fails when the committed suites under
[`suites/generated/`](../../suites/generated/) differ from what the specifications oblige — the same
guard `generated/` has, in its own CI job, because a suite is a contract too. It lives beside
`generated/` rather than inside it: each committed tree has exactly one generator, and both orphan
scans delete what their own task did not produce.

### Not `protocol conformance`

Two verbs, one word apart, two questions:

| command | asks |
|---|---|
| `protocol conformance` | does a storage **backend** implement the AEP contract — commands, queries, audit, idempotency, consistency? |
| `protocol ess conform` | does an **implementation** satisfy this specification — is a negative amount refused, can a paid invoice still be cancelled? |

Design §42 calls the first contract conformance and the second semantic conformance. Neither implies
the other, and each command's `--help` names the other one.

### What changed, and which way

`protocol ess diff` compares two revisions of one specification and reports what moved semantically.
Not a text diff — the comparison is over the two compiled IRs, so moving a declaration between files,
renaming a file, reordering blocks or rewriting every comment reports **nothing**, and a single line
that removes a currency reports one narrowing.

```console
$ protocol ess diff --from examples/revision-pair/before --to examples/revision-pair/after
catalog v2 → v2
  before  bc6f70b3dc81a99d67c95510139c121d21bbef19f229f46ac7887551b31811d8
  after   3e5ba8c16baf2d7d7316fd64fab88b6706cd3d6020562bd602ba1def8c196180

4 change(s): 2 widening, 2 narrowing, 0 other

  widens   type catalog.pricing.Currency: variant `CHF` added
           type/catalog.pricing.Currency/variant-added/CHF
  narrows  type catalog.pricing.Currency: variant `GBP` removed
           type/catalog.pricing.Currency/variant-removed/GBP
  narrows  actor catalog.pricing.Auditor: may no longer invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList
  widens   actor catalog.pricing.PricingManager: may invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.PricingManager/grant-added/catalog.pricing.RetirePriceList
```

[`examples/revision-pair/`](../../examples/revision-pair/) is that pair. Its two halves differ by
exactly those four changes and by a great deal of text that means nothing: the domain file has a
different name, every top-level block is in a different order, every comment is rewritten, and one
naming default is written out on one side and left implicit on the other.

**Four kinds of change carry a direction, and no others.** A grant added widens what the system
permits and a grant removed narrows it; an enum or union variant added widens what a type accepts and
one removed narrows it. Each is decided by set membership, so none of it is a guess. Everything else
is reported as *changed*, which says the revisions differ here and that no direction follows from the
difference — a rewritten invariant is `changed`, even when the new one is strictly stronger, because
saying so would be a proof rather than a comparison.

**Six construct families are compared:** the system header, types, events, errors, actors and
components. Entities, commands, views, bindings, topology and conversions are not, yet — their
invariants and conditions are predicates, and that is where an undecidable answer starts.

**Nothing is inferred to be a rename.** `InvoiceCreated` removed and `InvoiceIssued` added is reported
as a removal and an addition, however similar the names look: a rename and a delete-plus-create have
different consequences for everything already deployed, and a report that guesses between them is
wrong in the direction nobody checks.

There is one refusal: two specifications that name different systems. Comparing `billing` with
`ordering` would produce a delta — every construct of one added, every construct of the other removed
— and it would be an enormous, plausible answer to a question nobody asked.

`--format json` writes the `ess-diff/1` document: canonical, byte-identical for the same pair, with
each change carrying an id derived from its own content, so a review comment or a later tool can quote
one and still mean the same change after a sibling is inserted.

### What that invalidates, and why

A delta says what moved. `protocol ess impact` says what **stood on** what moved — and the first
thing that does is a conformance suite, because every scenario in one records the set of constructs
its result depends on.

```console
$ protocol ess impact --from examples/billing --to billing-with-one-grant-moved/ \
    --suite suites/generated/billing/suite.json
billing v3 → v3
  before  13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  after   fd52355634fd35e7401e371f924e806f9f30b69e95f98bd5d5fcd8c0ef504f5a

2 change(s): 1 widening, 1 narrowing, 0 other

  widens   actor billing.invoice.Auditor: may invoke `billing.invoice.CreateInvoice`
           actor/billing.invoice.Auditor/grant-added/billing.invoice.CreateInvoice
  narrows  actor billing.invoice.Customer: may no longer invoke `billing.invoice.CreateInvoice`
           actor/billing.invoice.Customer/grant-removed/billing.invoice.CreateInvoice

suite billing v3 (13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861): 7 of 29 scenario(s) owed again
2 construct(s) reached: 2 changed, 0 depend on one directly, 0 through another

  billing.invoice.CreateInvoice/outcome/accepted
    directly-changed actor billing.invoice.Customer — actor/…/grant-removed/…CreateInvoice
  …
```

Without this, moving one grant means re-running all twenty-seven: gate G19 binds conformance
evidence to the specification digest it was produced against, so the moment the model moves, every
requirement it satisfied goes back to owed. That is correct and it is blunt. Seven is the same
answer, proportionate.

**Every impact carries the path that explains it.** Not "these eleven things are affected" but
*this is affected because it references that, which references the thing you changed*:

```text
  catalog.pricing.PublishPriceList/outcome/published
    transitively-impacted entity catalog.pricing.PriceList — type/…/variant-removed/GBP
      -> type catalog.pricing.Money has a field of type type catalog.pricing.Currency
      -> type catalog.pricing.Headline wraps type catalog.pricing.Money
      -> entity catalog.pricing.PriceList has a field of type type catalog.pricing.Headline
```

The scenario never mentions `Currency`. It is reached three declarations away, and the three lines
are what make that checkable rather than assertable. An impact nobody can explain is an impact
nobody will act on.

**It narrows; it never says a result still holds.** A scenario absent from the output was not
reached by this analysis — which is *not* a claim that its evidence still stands. Impact analysis
refines G19 and cannot replace it, because the two errors are not comparable: failing closed costs a
re-run that was not needed, and failing open costs a task closing on evidence produced against a
specification that has since moved. So the report has no vocabulary for a survival, and three things
put the **whole** suite back to owed rather than narrowing it:

| what happened | why nothing can be narrowed away |
|---|---|
| a change to the specification itself — its version, its summary | no scenario names the system as a dependency, so no closure can start there |
| the suite depends on a construct the dependency graph has no node for | a closure could never reach it, and leaving it out of every answer is the one way a narrowing is wrong and looks right |
| the suite was produced from another revision, or another system | **refused** rather than answered: prior results were produced against the model the suite records, and narrowing against any other one answers a question about a specification nobody has |

**Why a verb and not `ess diff --suite`.** It takes a third input and writes a different document —
`ess-impact/1` rather than `ess-diff/1` — and a `--format json` that means one of two documents
depending on another flag is the shape `ess conform`'s two verbs were split apart to avoid. The
counter-argument is that a diff nobody can act on is not much of a diff; that is answered by
printing the delta first and in full, so nobody has to run both commands to see both halves.

### Why compile at all, when validate already refuses a bad specification

`validate` answers "is this document consistent". `compile` answers the question a generator has:
**what does every reference point at?**

A validated specification holds *names*. `CreateInvoice` emits `billing.invoice.InvoiceCreated`, and
that is a name which probably resolves. Anything reading it either re-checks every reference or trusts
that something else did — and both of those are how a generator emits code for a type that does not
exist. The IR holds resolved handles instead, and compiling is the only way to get one, so a
projection reading it cannot ask a question the IR cannot answer.

The same source compiled twice is byte-identical, and there is a test that says so rather than a
comment claiming it.

## The shortest thing that works

```console
$ cargo build -p protocol-cli
$ target/debug/protocol ess validate --path examples/billing
billing v3 — 5 file(s): 2 domain(s), 1 entit(ies), 5 command(s), 6 event(s), 3 error(s), 2 view(s), 2 actor(s)
valid
```

[`examples/billing/`](../../examples/billing/) is the normative example, and a test in `ess-domain`
parses it — so a change to the model that the example can no longer express fails the build rather
than quietly making the documentation wrong.

Break a reference and the refusal says what was available:

```console
$ cp -r examples/billing /var/tmp/copy
$ vi /var/tmp/copy/domains/invoice.yaml          # rename the emitted event, not its declaration
$ target/debug/protocol ess validate --path /var/tmp/copy
3 file(s)
1 problem(s):
  - [undeclared_reference] command.billing.invoice.CreateInvoice.outcomes.accepted.emits: `billing.invoice.InvoiceRaised` is not a declared event (hint: declared events: `billing.email.EmailSent`, `billing.invoice.InvoiceCreated`)
```

Every problem is reported in one run. An author who has to re-run the tool to discover the second
error is an author running it ten times to learn what one pass already knew.

## Six things the model insists on

**A command that can be refused says so.** Not an `emits` list — *outcomes*:

```yaml
outcomes:
  - name: accepted
    when: amount.amount > 0
    emits: [billing.invoice.InvoiceCreated]
  - name: rejected
    error: billing.invoice.InvalidAmount
```

A command with a precondition has at least two results. A specification recording only the happy one
generates a suite that never checks the branch where the money does not move — and the branch where
the money does not move is the one that matters.

**An outcome the input cannot decide says that too.** Whether a mail provider accepts an address is
not a function of the request:

```yaml
- name: failed
  external: the provider rejects the recipient address
  error: billing.email.Undeliverable
```

Writing `when: false` there would claim the branch is unreachable, which is a different statement and
a false one. A generator reads `external` and injects a fault instead of trying to construct an input.

**A projection declares its consistency.** `consistency: eventual` on a view is what decides that a
generated assertion is `eventually` rather than immediate. Getting this wrong produces a suite that
passes on a laptop and flakes in CI — and the usual fix, a sleep, makes the suite test the machine it
runs on.

**An illegal move is illegal because nobody wrote it.** `Paid` cannot become `Cancelled` because no
transition says it can. There is no rule forbidding it, because a rule would be a second place for
the same truth to live, and two places eventually disagree.

**And a command says what it answers when it is asked anyway.** That is one key and one name — the
error — because everything else is already written down:

```yaml
- name: issued
  moves: billing.invoice.Invoice.issue      # `issue` runs from [Draft]
  instance: invoice_id
  emits: [billing.invoice.InvoiceIssued]

- name: wrong-state
  wrong_state: true
  error: billing.invoice.InvoiceStateConflict
```

`wrong_state:` names no state. `issue` already declares it runs from `Draft`, so `Issued`, `Paid` and
`Cancelled` are the states `IssueInvoice` refuses in — derived, printed on the generated page, and
never authored, which is the same reason there is no rule forbidding `Paid → Cancelled`. Add a
`from:` to a transition and the branch narrows with it; nobody has to remember a second list.

The `error:` is required, and it is the whole point. Without it a generated suite could only assert
that *nothing happened*, which passes against an implementation that refuses for the wrong reason or
fails with an untyped infrastructure error. With it, the illegal-move scenarios require the branch
and the error by name. A command that moves nothing has no state to be wrong in, so declaring the
branch there is refused as `unreachable_branch`; two of them on one command is
`conflicting_declaration`, because an instance is in one state and both would claim it.

**An event's values are a guess until the outcome says where they come from.** `emits:` declares
*which* facts a branch announces; `payload:` declares what fills their fields:

```yaml
- name: accepted
  when: amount.amount > 0
  creates: billing.invoice.Invoice
  instance: invoice_id
  emits: [billing.invoice.InvoiceCreated]
  payload:
    billing.invoice.InvoiceCreated:
      customer_email: input.customer_email
      amount: input.amount
```

Without it, an implementation announcing an amount nobody submitted contradicts nothing: every field
is present and well-typed, and asserting `InvoiceCreated.amount == CreateInvoice.amount` from the
shared spelling would be a guess that fails a correct implementation naming its fields differently.
With it, the generated scenario holds the announced amount to the submitted one — a reading of the
declaration, not an inference.

The block is **optional per field**, and the absence means something: `invoice_id` has no line
because the identity is the implementation's to assign. An undetermined field stays covered by the
suite's presence-and-type assertion and by no value, which is how a reader of the suite sees exactly
what the specification leaves open. A source is `input.<field>` — checked against the command's
input, with the same type discipline and the same declared-conversion escape hatch a binding's
`mapping:` has — or literal text, checked as far as text can be. The two mappings are one idea read
in two directions: a binding fills a command's input *from an event*, an outcome fills an event's
payload *from the input*.

Over HTTP the same branch is a `409`, where an input-decided refusal is a `422`: the caller sent
nothing wrong, so there is nothing for it to correct.

## Three layers above the domains

A domain says what the software *means*. Three further layers say how it is put together, and the
model keeps them apart on purpose: conflating them is how a domain model turns into a description of
a deployment.

| layer | says | does not say |
|---|---|---|
| **component** | `invoice-service` owns `billing.invoice` | whether it is a process or a module |
| **binding** | `InvoiceCreated` causes `SendEmail` | which queue carries it |
| **topology** | the system is not correct with one instance | how many pods to start |

### A binding says what happens when it fails

```yaml
bindings:
  - id: notify-on-invoice-created
    when: {event: billing.invoice.InvoiceCreated}
    invoke: {command: billing.email.SendEmail}
    mapping:
      recipient: event.customer_email
      template: invoice-created
    delivery: at_least_once
    on_failure:                   # retry | drop | escalate
      escalate:
        emits: billing.email.DeliveryEscalated
```

`retry` and `drop` are single words. `escalate` is not: it has to say which declared event the
escalation emits, because "surface it to a person" is not something a generated test can observe.
A binding that escalates without naming an event is refused — the failure policy would otherwise be a
promise nothing can be asked to keep.

`delivery:` and `on_failure:` are **required words, not defaults**. A binding that can fail silently
is the difference between specifying a system and specifying a demo, and the way that difference
disappears is a default nobody read. `drop` is legal — a system that loses work is a decision, and
the decision has to be findable in the document that made it.

`at_least_once` is the only guarantee this build accepts, and stating it is still worth doing:
"exactly once" is what everyone believes they have until a retry proves otherwise, so the word on the
page is what tells a generator the command must be idempotent.

### A mapping is where two contexts have to agree

`mapping` is the one place in the model where two independently-written bounded contexts must agree
about a type — so it is the one place a rename in one of them breaks the other silently. Both sides
are checked:

```text
billing.invoice.InvoiceCreated.customer_email  has type `billing.invoice.Email`
billing.email.SendEmail.recipient              requires  `billing.email.EmailAddress`
```

Those are two distinct newtypes, both a `String` underneath, and the entire value of naming them
apart is that the model refuses to treat one as the other. To let this particular crossing through,
say so — and say why:

```yaml
conversions:
  - from: billing.invoice.Email
    to: billing.email.EmailAddress
    because: >-
      An invoice's customer email is a deliverable address; the email context validates it again on
      the way out, so the invoice context does not have to know how.
```

`because:` is required. A conversion with no reason is exactly what this declaration exists to
prevent: a widening someone added to make a build pass, which the next reader finds and cannot
evaluate. Conversions are **directional** — declaring `Email → EmailAddress` does not grant the
reverse, and the reverse is usually the unsafe one.

## What is a name, and what is three names

| name | example | who reads it |
|---|---|---|
| qualified name | `billing.invoice.CreateInvoice` | the specification, and only it |
| wire name | `create-invoice` | HTTP paths, topics, generated JSON |
| display name | `Create invoice` | generated documentation, a UI |
| locator | `ep://acme/billing/ess-command/billing.invoice.CreateInvoice` | anything outside |

Conflating any two of these costs a rename later: an HTTP path that changes because someone improved
a domain term is an outage caused by a wording fix. The locator reuses the protocol's own `ep://`
scheme rather than inventing `ess://`, so an approval recorded against a command in a specification
addresses it the same way an approval against a design document does.

## Requiring conformance

The protocol half can already demand it. [`ess-conformance`](../../principles/verification/ess-conformance.yaml)
is conditional on the project having an ESS artifact at all, and when it does:

* `ess_conformance.passed` must be true,
* `ess_conformance.scenarios.failed` must be zero,
* the evidence must be `independent: true` and come from a `conformance-runner`.

The last one is the load-bearing part. An agent's own report that its implementation matches the
specification is not evidence that it does.

Add it to a profile the same way as any other principle:

```yaml
principles:
  - ess-conformance
```

### The handoff, end to end

`protocol ess conform run` prints a report about an implementation.
`protocol ess conform evidence` produces the record the protocol decides on — the same run, written
as evidence:

```console
$ protocol ess conform evidence --path examples/billing --target billing
- kind: ess_conformance
  specification: billing/v3
  spec_digest: 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  implementation: billing-reference 0.1.0
  status: passed
  scenarios_total: 29
  scenarios_failed: 0
  producer:
    producer: verifier
    verifier: conformance-runner
```

That file goes straight into `protocol evaluate --evidence`, and
[`examples/billing-conformance/`](../../examples/billing-conformance/) walks the whole sequence:
a passing run completes the task, and a run against an implementation with one deliberate fault
leaves it blocked, naming `ess_conformance.scenarios.failed = 1` and the principle that refused it.

Three things about that record are worth knowing before you rely on it.

**The conversion happens in the runner, not in the engine.** Invariant 7 — the engine never
manufactures evidence — and design §32: an agent may trigger a run, read the report and repair what
it says, and may not construct the record by assertion.
[`ConformanceReport::to_evidence`](../../crates/ess-conformance/src/evidence.rs) takes no argument
naming who produced it, so there is no call site at which a caller can describe itself as the
verifier. That is also why the verb runs the suite rather than converting a saved report: a
`--report report.json` input would be a record whose contents came from a file the caller wrote.

**The digest is what the record is worth anything for.** `billing/v3` is a label two different
resolutions can share; `spec_digest` is the content identity of the resolved model, and the rule
fails closed — an ESS artifact recording no `model_digest` can never be shown to have been conformed
to, and a run against yesterday's model does not close a task built against today's.

**`independent: true` is structural, not attested.** It says the producer is not the agent under
review, checked by one comparison. Nothing signs the file, and a person can type one. Which
producers a harness lets write records is the harness's decision, exactly as it is for a test
runner's `tests.unit.failed == 0`.

**A run that could not be carried out is not a failure.** `passed`, `failed` and `inconclusive` are
three different findings; the third means nobody found out, which is a target to go and reach rather
than a defect to fix. None of the last two is a pass, so the requirement stays owed either way.

## Writing one

Start from [`examples/billing/`](../../examples/billing/) — it is deliberately the smallest system
that exercises everything the model has. The layout:

```text
system.yaml            format version, the system's name, which domains it has
domains/invoice.yaml   one bounded context: types, entities, commands, events, errors, views
domains/email.yaml     a second, so cross-domain references are exercised rather than assumed
```

The header's `domains:` list is checked in both directions: a domain listed there that nothing
declares is refused, and so is a domain some file contributes that the header does not list. A
misspelling in either place is the kind of thing that otherwise reads as "that context is not
finished yet".

One file works too. `protocol ess validate --path spec.yaml` reads a single file carrying both the
header and the members, which is what a small system should look like; splitting into a directory
later changes nothing about how the tool is invoked.

Point your editor at [`schemas/generated/ess.schema.json`](../../schemas/generated/ess.schema.json)
and field names are checked as you type, rather than by a build somewhere else. The schema is
generated from the same Rust types the validator runs, and CI fails if the two drift.
