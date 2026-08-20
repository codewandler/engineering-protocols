---
title: What a refusal looks like
sidebar_position: 2
description: A refusal names the rule, the path and what would unlock it. Four kinds, with the codes they carry.
---

# What a refusal looks like

A tool that says *no* without saying why teaches nobody anything, and gets routed around. Both halves
of this project treat the refusal as the product: it carries a stable code, the path it is about, and
either what is missing or what was available instead.

## An action refused

`explain` answers one question — may this be done, here, now?

```text
$ protocol explain --task examples/development-passkeys/task.yaml --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
```

Four lines, and each does work. The **reason** names a principle a person can go and read and a rule
inside it with a stable name. The **missing** line says exactly what would unlock it. The **state**
says where in the workflow the question was asked, because the answer depends on it.

Nobody had to write that denial into the task or the profile. `approval-gates` is in force because
the profile includes it, and `aep/1` holds `production.write` in an approval floor — so a profile
that granted it outright would fail to resolve at all, rather than being caught in review.

## A rule that could never fire

The harder failure is not a rule that refuses. It is a rule that looks enforced and does nothing.
`validate` reads the document tree on its own and refuses several ways of writing one:

| Refusal | Because otherwise |
|---|---|
| `unobservable_fact` — a predicate reading a fact the protocol does not declare | the rule can never be satisfied, and nobody finds out until a task hangs |
| `unreachable_state` — a workflow state nothing can reach | the state is decoration |
| `dead_end_state` — a non-terminal state with no way out | execution wedges there |
| `incomplete_rollback_policy` — a rollback with no precondition | "rolled back" describes a wish, not a plan |
| `unknown_phase` — an obligation timed against a phase no state declares | the rule is not strict, it is absent |
| `capability_conflict` — a task needing a capability the resolved policy denies | the agent would find out mid-task |

An `unobservable_fact` refusal reads like this — the fact, the protocol that does not declare it, and
then a list of every family that *is* declared, which is usually enough to find the right spelling:

```text
  - [unobservable_fact] principle migration-has-a-way-back...: `migration.rollback_tested` is not
    declared observable by protocol adp/1 (hint: declared families: ...)
```

There is a quieter version of the same mistake, and it is worth knowing before you meet it: a fact in
a declared family but a spelling nothing projects passes validation and then never becomes true. A
`?` that never becomes `✓` is a task nobody can finish, and it looks like a stuck agent rather than a
typo.

## A specification refused

The ESS compiler's diagnostics are built the other way round from most: the structured form is the
diagnostic, and the text is a projection of it — never the reverse, which is how a message ends up
carrying information the machine-readable form lost. Each one has a stable code such as
`ESS-BINDING-002`, a message, typed details, an optional hint and a source span.

The reason that matters: a coding agent consumes a diagnostic as a **repair instruction**. It needs
the two types and the two paths as fields, not as prose it has to parse back out.

The refusal worth seeing is the type crossing, because it is the one place two independently written
bounded contexts must agree about a type, and so the one place a rename in one of them breaks the
other silently:

```text
billing.invoice.InvoiceCreated.customer_email  has type `billing.invoice.Email`
billing.email.SendEmail.recipient              requires  `billing.email.EmailAddress`
```

Both are a newtype over `String`. Neither converts to the other, because being both strings is not a
reason. To let this crossing through, the specification has to say so — and say why:

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
evaluate. Conversions are directional — declaring `Email → EmailAddress` does not grant the reverse,
and the reverse is usually the unsafe one.

Because the reason is required, the generated documentation can carry it. From
`generated/docs/crossings.md`:

> Declaring a crossing is also the only way to make one. Two newtypes over `String` do not convert
> because they are both strings; they convert because a line in the specification says they may.

## Refusals the model cannot argue with

Three more, chosen because each closes a way of writing a specification that reads fine and means
nothing:

* **`missing_causation`** — a transition no command outcome takes. A state change nothing can trigger
  is the lifecycle equivalent of a type no value can inhabit.
* **`refusal_mutated_state`** — an outcome that reports an error *and* moves an entity. A refused
  command changes nothing, so that branch is refused rather than documented.
* **A binding that escalates without naming an event.** `on_failure: escalate` has to say which
  declared event the escalation emits, because "surface it to a person" is not something a generated
  test can observe. `delivery:` and `on_failure:` are required words, not defaults — a binding that
  can fail silently is the difference between specifying a system and specifying a demo, and the way
  that difference disappears is a default nobody read.

## Every problem, in one run

Validation accumulates. A document with four broken references reports four errors, not the first
one — in both halves, and the tests assert exact counts rather than "is an error".

An author who has to re-run the tool to discover the second error is an author running it ten times
to learn what one pass already knew.

## A refusal is still a record

On the protocol side, refusing is not silence. Asking whether an action is permitted is itself an
event: the request and its answer both land in the audit trail, including the denials. And a refused
command changes nothing while still being recorded — the audit type rejects a rejection that carries
a change record, so the trail cannot claim a refusal changed something.

---

**Sources.** `README.md` and `examples/development-passkeys/README.md` (the `explain` transcript;
the rule name `production-write-requires-approval` is asserted in `crates/aep-engine/src/policy.rs`
and `crates/aep-engine/tests/end_to_end.rs`); `docs/guide/adopting.md` § *Keeping the documents
honest*; `crates/ess-compiler/src/diagnostic.rs`; `docs/guide/specification.md`;
`examples/billing/components.yaml`; `generated/docs/crossings.md`;
`examples/billing/domains/invoice.yaml`; `AGENTS.md` invariants 3 and 15;
`crates/aep-domain/src/audit.rs`.
