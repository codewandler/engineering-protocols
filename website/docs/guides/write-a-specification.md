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
```

One file works too: `protocol ess validate --path spec.yaml` reads a single file carrying the header
and the members, and splitting into a directory later changes nothing about invocation.

The header's `domains:` list is checked in both directions — a listed domain nothing declares is
refused, and so is a declared domain the header does not list.

Point your editor at `schemas/generated/ess.schema.json` and field names are checked as you type.
The schema is generated from the same Rust types the validator runs; CI fails if they drift.

## Validate early, read the refusals

```console
$ protocol ess validate --path examples/billing
billing v3 — 5 file(s): 2 domain(s), 1 entit(ies), 5 command(s), 6 event(s), 3 error(s), 2 view(s), 2 actor(s)
valid
```

Break a reference and the refusal names what was available:

```console
$ protocol ess validate --path /var/tmp/copy
3 file(s)
1 problem(s):
  - [undeclared_reference] command.billing.invoice.CreateInvoice.outcomes.accepted.emits: `billing.invoice.InvoiceRaised` is not a declared event (hint: declared events: `billing.email.EmailSent`, `billing.invoice.InvoiceCreated`)
```

Every problem is reported in one run.

## What the model insists on

These are the authoring decisions that surprise people coming from OpenAPI-first or prose designs.
Each exists to keep a generated test honest.

### A command that can be refused says so

Not an `emits` list — **outcomes**:

```yaml
outcomes:
  - name: accepted
    when: amount.amount > 0
    emits: [billing.invoice.InvoiceCreated]
  - name: rejected
    error: billing.invoice.InvalidAmount
```

A command with a precondition has at least two results. A specification recording only the happy one
generates a suite that never checks the branch where the money does not move.

### An outcome the input cannot decide says that too

Whether a mail provider accepts an address is not a function of the request:

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
  emits: [billing.invoice.InvoiceIssued]

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
  emits: [billing.invoice.InvoiceCreated]
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

`delivery:` and `on_failure:` are required words, not defaults — a binding that can fail silently is
the difference between specifying a system and specifying a demo. `drop` is legal: losing work is a
decision, and the decision has to be findable in the document that made it. `escalate` must name a
declared event, because "surface it to a person" is not something a generated test can observe.

### Crossing contexts takes a declared conversion

A binding's `mapping:` is the one place two independently-written contexts must agree about a type,
so both sides are checked. `billing.invoice.Email` and `billing.email.EmailAddress` are distinct
newtypes, and the model refuses to treat one as the other unless you say so — with a reason:

```yaml
conversions:
  - from: billing.invoice.Email
    to: billing.email.EmailAddress
    because: >-
      An invoice's customer email is a deliverable address; the email context
      validates it again on the way out.
```

`because:` is required, and conversions are directional — declaring `Email → EmailAddress` does not
grant the reverse, which is usually the unsafe one.

## Three layers above the domains

A domain says what the software *means*. Three further layers say how it is put together, kept apart
because conflating them is how a domain model turns into a description of a deployment:

| Layer | Says | Does not say |
|---|---|---|
| **component** | `invoice-service` owns `billing.invoice`; `reached_by: network` states its callers are remote | whether it is a process or a module; which protocol it speaks |
| **binding** | `InvoiceCreated` causes `SendEmail` | which queue carries it |
| **topology** | the system is not correct with one instance | how many pods to start |

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
