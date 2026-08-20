# ESS wave 2 — the compiler

> **Delivered.** Goal from [`ess-roadmap.md`](ess-roadmap.md): source becomes a normalized IR with
> no unresolved references, and every rejection in design §20 has a test.

Wave 1 made a specification a document that parses and refuses. It stopped one step short of being
useful: nothing downstream can consume `Specification`, because a reference in it is a
`QualifiedName` that *probably* resolves. A generator holding one has to re-check every reference or
trust that someone else did, and both are how a generator emits code for a type that does not exist.

Wave 2's answer is a second representation whose *type* carries the guarantee.

## The rule this wave follows

**An unresolved reference must be unrepresentable, not merely absent.** `EssIr` holds resolved
handles, not names. Constructing one is the only way to get one, and construction is what runs the
checks — so a projection reading the IR cannot ask a question the IR cannot answer.

That is the same discipline as wave 1's `Raw*` → validated split, applied one level up: `Raw*` is
"what a document may say", `Specification` is "what it means, locally consistent", and `EssIr` is
"what it means, globally resolved".

## W2.1 — Three layers the model does not have

Wave 1 deliberately modelled no deployment topology, and `examples/billing/README.md` said so. Half
of §20's rejection list is about those layers, so they arrive now.

| Layer | Design | What it adds |
|---|---|---|
| Components | §5, §6 | `owns.domains`, `accepts.commands`, `publishes.events` — logical decomposition, not a deployment decision |
| Bindings | §7 | `when.event` → `invoke.command`, with a typed `mapping` |
| Topology | §8 | workloads, replica floors, required resources |

**A binding states what happens when it fails** (review F3). `delivery:` and `on_failure:` are
required words, not defaults:

```yaml
delivery: at_least_once       # the only value this build accepts
on_failure: escalate          # retry | escalate | drop
```

`drop` has to be typed. A binding that silently drops is the difference between specifying a system
and specifying a demo, and the way that difference disappears is a default nobody read.

## W2.2 — Mapping validation is where this earns its keep

A binding maps an event's fields onto a command's input. That is the one place in the model where two
independently-written declarations have to agree about a type, and the only place a rename in one
context can break another silently.

```text
InvoiceCreated.customer_email : Email  →  SendEmail.recipient : Email        ✓
InvoiceCreated.customer_email : Email  →  SendEmail.recipient : VerifiedEmail ✗
```

The second is refused with design §29's diagnostic, because `Email` and `VerifiedEmail` are distinct
types and the value of naming them separately is entirely in the conversions the model refuses.

## W2.3 — Diagnostics a coding agent can act on

§29 wants a code, a span and a structured body. The structured body is the part that matters: an
agent consuming a diagnostic as a repair instruction needs the two types and the two paths as
*fields*, not as a sentence it has to parse back out.

```text
error[ESS-BINDING-002]: binding `notify-on-invoice-created` is invalid
  billing.invoice.InvoiceCreated.customer_email  has type `billing.invoice.Email`
  billing.email.SendEmail.recipient              requires  `billing.email.VerifiedEmail`
  no conversion is declared
  --> domains/bindings.yaml:14:18
```

The `line:column` is resolved by locating the document path in the source text. `serde_yaml` gives
spans for syntax errors and nothing for semantic ones, and a diagnostic pointing at the top of the
file is a diagnostic someone has to search from.

## W2.4 — Determinism made true (review F8)

Asserted in wave 1, mechanised here: `BTreeMap`/`BTreeSet` only, no clock and no RNG anywhere in the
compiler, canonical serialisation with a trailing newline, and a test that compiles the example twice
and compares bytes. The last one is what makes the first three true rather than aspirational.

## Deliverable

| Command | What it does |
|---|---|
| `protocol ess compile` | source → IR, or every diagnostic |
| `protocol ess inspect` | what one declaration is, resolved |
| `protocol ess graph` | the event/command graph, as DOT |

Plus: every §20 rejection has a code, a message and a failing fixture.

## What the wave taught us about where validation belongs

The plan above says `Specification` is "locally consistent" and `EssIr` is "globally resolved". The
implementation found that line is not where the plan put it.

Wave 1 already did cross-cutting reference validation inside `Specification::validate` — undeclared
events, view sources, ownership — and this wave added bindings, components and topology to it. So by
the time a `Specification` exists, every reference in it has already been checked, and the compiler's
own §20 codes are unreachable through the normal pipeline. `ess compile` reported no diagnostics not
because the specification was clean but because nothing could get that far while broken.

Two ways out, and only one of them is honest:

* **Move the rules into the compiler.** Conceptually tidier, and it makes `ess validate` weaker than
  it is today — a user-visible regression, to fix a diagram.
* **Bridge, not duplicate.** One implementation per rule, in `ess-domain` where it is tested, and the
  compiler maps each `ValidationError` onto a diagnostic code, a `Detail` and a `file:line`. Every
  existing rule gains design §29's structured form; nothing gets checked twice; no rule moves.

The second. Which means the compiler's contribution to this wave is **the IR, the diagnostic format
and the spans** — not a second set of checks. Saying that plainly is better than shipping two code
paths that report one defect under two codes, and it is the kind of thing that is only visible once
both halves exist.

## Not in this wave

Projections — OpenAPI, AsyncAPI, documentation, test synthesis — are wave 3. Topology is *modelled*
and generates nothing: the point of writing it down now is that §20's "topology references a missing
component" becomes checkable, not that anything is deployed.
