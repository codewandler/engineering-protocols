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
| Validate in an editor as you type | [`schemas/generated/ess.schema.json`](../../schemas/generated/ess.schema.json) |
| Require conformance to one, as a protocol rule | [`principles/verification/ess-conformance.yaml`](../../principles/verification/ess-conformance.yaml) |

**Nothing is generated yet.** There is no compiler, no OpenAPI, no test synthesis — those are ESS
waves 2 and 3 in [`docs/plan/ess-roadmap.md`](../plan/ess-roadmap.md). What exists is the model and
the join: a task can already be blocked until something proves an implementation conforms, with a
human producing that evidence by hand.

## The shortest thing that works

```console
$ cargo build -p protocol-cli
$ target/debug/protocol ess validate --path examples/billing
billing v3 — 3 file(s): 2 domain(s), 1 entit(ies), 2 command(s), 2 event(s), 2 error(s), 2 view(s), 2 actor(s)
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

## Four things the model insists on

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

Until a compiler exists, a person produces that evidence by hand. The point is that the *shape* is
already right: when the runner arrives, nothing about the protocol side changes.

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
