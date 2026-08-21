---
title: Write a specification
sidebar_position: 4
description: Author an ESS document — the layout, the constructs the model insists on, and the validation errors that teach the model fastest.
---

# Write a specification

This guide covers authoring an Executable System Specification. The normative example is
`examples/billing/` in the repository — deliberately the smallest system that exercises everything
the model has. Concepts are covered in [ESS](../concepts/ess.md); this page is about writing one.

## Layout

```text
system.yaml            format version, the system's name, which domains it has
domains/invoice.yaml   one bounded context: types, entities, commands, events, errors, views
domains/email.yaml     a second, so cross-domain references are real rather than assumed
components.yaml        which component owns which domain, the bindings between them, conversions
topology.yaml          what the system needs at runtime to be correct
```

Those five files are `examples/billing/`, and the `5 file(s)` in every validate line below is them.

One file works too: `protocol ess validate --path spec.yaml` reads a single file carrying the header
and the members, and splitting into a directory later changes nothing about invocation. A file names
at most one `domain:`, so the one-file form is for a one-domain system; `components:`, `bindings:`
and `topology:` can sit in it as well.

The header's `domains:` list is checked in both directions, and both refusals name the fix:

```text
- [undeclared_reference] system.domains: `tiny.audit` is listed as a domain of the system, and no source declares it (hint: declared domains: tiny.core)
- [conflicting_declaration] domain tiny.core: `tiny.core` is declared, and the system header does not list it (hint: the header's `domains:` says what the system has; add it there, or drop the source that declares it)
```

Point your editor at `schemas/generated/ess.schema.json` and field names are checked as you type.
The schema is generated from the same Rust types the validator runs, and `cargo xtask schema
--check` runs in CI as its own step, so drift fails by name rather than inside a test log.

## Validate early, read the refusals

```console
$ protocol ess validate --path examples/billing
billing v3 — 5 file(s): 2 domain(s), 1 entit(ies), 5 command(s), 6 event(s), 3 error(s), 2 view(s), 2 actor(s)
valid
```

Break a reference and the refusal names what was available. Take a copy of `examples/billing/`,
misspell `InvoiceCreated` as `InvoiceRaised` in the `accepted` branch's `emits:` and `payload:` —
those two occurrences only, not every one in the file — and run it:

```console
$ COPY=$(mktemp -d)/billing && cp -r examples/billing "$COPY"
$ # in $COPY/domains/invoice.yaml, rename `InvoiceCreated` to `InvoiceRaised` in the
$ # `accepted` outcome's `emits:` list and its `payload:` key
$ protocol ess validate --path "$COPY"
5 file(s)
2 problem(s):
  - [undeclared_reference] command.billing.invoice.CreateInvoice.outcomes.accepted.emits: `billing.invoice.InvoiceRaised` is not a declared event (hint: declared events: `billing.email.DeliveryEscalated`, `billing.email.EmailSent`, `billing.invoice.InvoiceCancelled`, `billing.invoice.InvoiceCreated`, `billing.invoice.InvoiceIssued`, `billing.invoice.InvoicePaid`)
  - [undeclared_reference] command.billing.invoice.CreateInvoice.outcomes.accepted.instance: outcome `accepted` of `billing.invoice.CreateInvoice` acts on the instance named by `invoice_id`, which is no field of an emitted event of it (hint: the field of an emitted event must be typed `billing.invoice.InvoiceId` — declared: none are declared)
```

Every problem is reported in one run — one typo, two consequences, both stated, and the exit code is
`1`. The second is the more useful of the two: the branch says it creates an invoice and publishes
its identity in an emitted event, and the misspelling took away the event that was carrying it.

## What the model insists on

These are the authoring decisions that surprise people coming from OpenAPI-first or prose designs.
Each exists to keep a generated test honest. Every block below is an excerpt of
`examples/billing/`, abridged only by dropping comments and `summary:` lines.

### A command that can be refused says so

Not an `emits` list — **outcomes**. From `domains/invoice.yaml`:

```yaml
outcomes:
  - name: accepted
    when: amount.amount > 0
    creates: billing.invoice.Invoice
    instance: invoice_id
    emits:
      - billing.invoice.InvoiceCreated

  - name: rejected
    error: billing.invoice.InvalidAmount
```

A command with a precondition has at least two results. A specification recording only the happy one
generates a suite that never checks the branch where the money does not move.

### An outcome the input cannot decide says that too

Whether a mail provider accepts an address is not a function of the request. From
`domains/email.yaml`:

```yaml
- name: failed
  external: the provider rejects the recipient address
  error: billing.email.Undeliverable
```

Writing `when: false` would claim the branch is unreachable — a different statement, and a false
one. A generator reads `external` and injects a fault instead of trying to construct an input.

### Illegal lifecycle moves are illegal by absence

`Paid` cannot become `Cancelled` because no transition says it can. There is no forbidding rule,
because a rule would be a second place for the same truth to live, and two places eventually
disagree. The generated documentation lists the absent pairs, derived from the same transitions.

### A command says what it answers when invoked in the wrong state

One key and one error name — everything else is derived:

```yaml
- name: issued
  moves: billing.invoice.Invoice.issue      # `issue` runs from [Draft]
  instance: invoice_id
  emits:
    - billing.invoice.InvoiceIssued

- name: wrong-state
  wrong_state: true
  error: billing.invoice.InvoiceStateConflict
```

`wrong_state:` names no state: `issue` already declares it runs from `Draft`, so the refused states
are derived. The `error:` is required — without it a generated scenario could only assert that
*nothing happened*, which also passes against an implementation refusing for the wrong reason.

### An event's values need a declared source

`emits:` declares which facts a branch announces; `payload:` declares what fills their fields:

```yaml
- name: accepted
  when: amount.amount > 0
  creates: billing.invoice.Invoice
  instance: invoice_id
  emits:
    - billing.invoice.InvoiceCreated
  payload:
    billing.invoice.InvoiceCreated:
      customer_email: input.customer_email
      amount: input.amount
```

Without this, an implementation announcing an amount nobody submitted contradicts nothing. The block
is optional per field, and an absence means something: `invoice_id` has no line because the identity
is the implementation's to assign.

### A view declares its consistency

`consistency: eventual` on a view is what decides that a generated assertion is `eventually` rather
than immediate. Getting it wrong produces a suite that passes on a laptop and flakes in CI.

### A binding says what happens when it fails

Bindings live above the domains, in `components.yaml`:

```yaml
bindings:
  - id: notify-on-invoice-created
    when:
      event: billing.invoice.InvoiceCreated
    invoke:
      command: billing.email.SendEmail
    mapping:
      recipient: event.customer_email
      template: invoice-created
    delivery: at_least_once
    on_failure:                   # retry | drop | escalate
      escalate:
        emits: billing.email.DeliveryEscalated
```

`delivery:` and `on_failure:` are required words, not defaults — a binding that can fail silently is
the difference between specifying a system and specifying a demo. `drop` is legal: losing work is a
decision, and the decision has to be findable in the document that made it. `escalate` must name a
declared event, because "surface it to a person" is not something a generated test can observe.

`at_least_once` is the only value `delivery:` accepts today. It is spelled out rather than assumed
because "exactly once" is what everyone believes they have until a retry proves otherwise; a second
guarantee would take an implementation to derive from, not a keyword.

### Crossing contexts takes a declared conversion

A binding's `mapping:` is the one place two independently-written contexts must agree about a type,
so both sides are checked. `billing.invoice.Email` and `billing.email.EmailAddress` are distinct
newtypes, and the model refuses to treat one as the other unless you say so — with a reason:

```yaml
conversions:
  - from: billing.invoice.Email
    to: billing.email.EmailAddress
    because: >-
      An invoice's customer email is a deliverable address; the email context validates it again on
      the way out, so the invoice context does not have to know how.
```

`because:` is required, and conversions are directional — declaring `Email → EmailAddress` does not
grant the reverse, which is usually the unsafe one. The reason is not decoration: it is what
`ess inspect` prints back at the crossing, so the person reading the binding a year later reads the
argument for it rather than reconstructing one.

## Three layers above the domains

A domain says what the software *means*. Three further layers say how it is put together, kept apart
because conflating them is how a domain model turns into a description of a deployment:

| Layer | Says | Does not say |
|---|---|---|
| **component** | `invoice-service` owns `billing.invoice` and accepts four commands | whether it is a process or a module; which protocol it speaks |
| **binding** | `InvoiceCreated` causes `SendEmail` | which queue carries it |
| **topology** | the system is not correct with one instance | how many pods to start |

One component word does reach further than the rest: `reached_by:`, a closed set of `in_process` —
the default, and what silence has always meant — and `network`. It says where a component's callers
are, and that is enough for the generators to derive an HTTP surface rather than a document beside
one. `examples/billing/` declares neither and gets the default; `examples/gatepass/components.yaml`
declares `reached_by: network`, which is why that example has a served contract and billing does not.

## Check what you just wrote resolved

`ess validate` says the document holds together. `ess inspect` shows what one declaration *became*,
with every reference in it resolved — the fastest way to find out whether the thing you meant is the
thing the model read:

```console
$ protocol ess inspect billing.invoice.CreateInvoice --path examples/billing --kind command
command    billing.invoice.CreateInvoice
  domain     billing.invoice
  input      customer_email: billing.invoice.Email
  input      amount: billing.invoice.Money
  outcome    accepted — when amount.amount > 0 (test: construct_input)
    emits      billing.invoice.InvoiceCreated
  outcome    rejected — otherwise (test: default_branch)
    reports    billing.invoice.InvalidAmount
  wire       create-invoice
  display    Create invoice
```

`rejected — otherwise` is the line to read: the specification names no condition for that branch, and
the model derived one. `(test: construct_input)` and `(test: default_branch)` say how a generated
scenario will reach each branch, before any suite exists.

On a binding it resolves the crossing as well, reason included:

```console
$ protocol ess inspect notify-on-invoice-created --path examples/billing --kind binding
binding    notify-on-invoice-created
  when       billing.invoice.InvoiceCreated occurs
  invoke     billing.email.SendEmail
  mapping    recipient: billing.email.EmailAddress = event.customer_email: billing.invoice.Email
    converted  An invoice's customer email is a deliverable address; the email context validates it again on the way out, so the invoice context does not have to know how.
  mapping    template: billing.email.TemplateId = `invoice-created` (literal)
  delivery   at_least_once
  on failure escalate
  summary    Tell the customer an invoice exists.
```

`--kind` is only needed when one name is used in two namespaces; the seven it accepts are `domain`,
`type`, `command`, `event`, `error`, `binding` and `component`.

## Names

| Name | Example | Who reads it |
|---|---|---|
| qualified name | `billing.invoice.CreateInvoice` | the specification, and only it |
| wire name | `create-invoice` | HTTP paths, topics, generated JSON |
| display name | `Create invoice` | generated documentation, a UI |
| locator | `ep://acme/billing/ess-command/billing.invoice.CreateInvoice` | anything outside |

Conflating any two costs a rename later: an HTTP path that changes because someone improved a domain
term is an outage caused by a wording fix.

## Next

* [Verify an implementation](./verify-conformance.md) — generate the suite this specification
  obliges, run it, and turn the result into evidence.
* [A specification and its contracts](../examples/specification-to-contracts.md) — the billing
  example's source next to its generated output.
