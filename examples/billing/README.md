# The billing specification

The single normative example. Every snippet in the ESS design document is meant to be derivable from
this directory, and a test in `ess-domain` parses it — so a change here that the model cannot express
fails the build rather than quietly making the document wrong.

The system it describes is deliberately the smallest one that exercises everything wave 1 must model:

```text
billing.invoice                      billing.email
  CreateInvoice ──▶ InvoiceCreated     SendEmail ──▶ EmailSent
        │                                  │
        ▼                                  ▼
  Invoice (Draft → Issued → Paid | Cancelled)
        │
        ▼
  InvoiceById (eventual)
```

Two bounded contexts, a command with **two outcomes**, a command with an outcome the input cannot
decide, events, both consistency levels, a filtered view, actors, and a state machine whose illegal
transitions are illegal by absence.

A test — `the_example_exercises_every_construct_the_model_has` — asserts that every type body, every
primitive, `Optional`/`List`/`Map`, both consistency levels, an actor with grants and one without, an
error carrying a payload and a command with an overridden wire name all appear here. **A construct
added to the model without reaching this directory fails the build**, because what the normative
example leaves out is what nothing checks.

**A component is not a deployment.** `invoice-service` owning `billing.invoice` says the invoice
context is one unit of ownership; whether it ships as its own process or as a module inside one binary
is `topology.yaml`'s business, and changing that answer changes nothing in `domains/`. That separation
is the point of specifying a system semantically, and it is why the three layers are three files.

## What each file is for

| File | Why it exists |
|---|---|
| `system.yaml` | the format version, the system's identity, and which domains it has |
| `domains/invoice.yaml` | the invoice bounded context: every type kind, an entity with a lifecycle, actors, a refusable command, an event, and both kinds of view |
| `domains/email.yaml` | the second context, so cross-domain references are exercised rather than assumed, and the command whose failure the input cannot decide |
| `components.yaml` | who owns which context, the binding between them, and the one type crossing that binding needs |
| `topology.yaml` | what the system needs in order to run — modelled, and deployed by nothing |

## Three things worth reading closely

**`CreateInvoice` has outcomes, not an `emits` list.** A command with a precondition has at least two
results, and a specification that records only the happy one generates tests that say nothing about
the branch where the money does not move.

**`InvoiceById` declares its consistency.** It is a projection, so it is `eventual`, so a generated
scenario must assert it with `eventually` rather than immediately. Getting this wrong produces a suite
that passes on a laptop and flakes in CI, and the usual fix — a sleep — makes the suite test the
machine it runs on.

**`Paid` cannot become `Cancelled`, and no rule says so.** There is simply no transition. A rule
would be a second place for the truth to live, and the two would eventually disagree.
