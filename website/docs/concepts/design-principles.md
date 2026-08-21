---
title: Design principles
sidebar_position: 5
description: The rules the whole project is built on — what each one means in practice, what enforces it, and where its limits are.
---

# Design principles

These are the design rules both halves of the project are built on. Each is stated with the concrete
mechanism that enforces it, because a rule nothing checks has already drifted somewhere — and where
a principle is only partially held, that limit is stated too. The repository's `AGENTS.md` keeps the
full invariant register with the enforcing test or lint named for each entry.

## 1. Evidence over assertion

A task does not complete because an agent says the work is done. It completes when a predicate over
recorded facts evaluates true. Facts come from evidence records; each record names its producer, and
a requirement marked `independent: true` is not satisfied by the agent's own submission. Submission
order is recorded too, so "the test failed before the implementation existed" is a checkable fact
(`evidence.first_seq.test_result < evidence.first_seq.diff`), not a claim in a report.

**Enforced by:** the `Producer` type on every evidence record, requirement evaluation reading it,
and a source scan (`crates/aep-engine/tests/evidence_scan.rs`) that refuses any construction of
evidence in shipped engine code — the engine evaluates evidence, it never manufactures it.

**Limit:** independence is structural, not attested. A harness that misreports the producer defeats
it. See [Limitations](../status/limitations.md).

## 2. `Unknown` is not `False`

Predicate evaluation is three-valued. "The tests failed" and "nothing ran the tests" are different
situations demanding different responses — fix the code, or go run the tests — and only `True`
permits a transition, so the third value loosens nothing while preventing "we did not look" from
reading as "we looked and it was fine". That conflation is the failure agents produce most often and
the one hardest to see in a summary.

**Enforced by:** the `Truth` type — three variants, Kleene `and`/`or`, no `From<bool>`, no
`as_bool`. There is no boolean to collapse into. The algebra's laws are property-checked over
generated expressions.

## 3. Deny by default, and a denial cannot be undone

A capability no document mentions is not granted. Precedence is fixed — `deny` beats
`require_approval` beats `allow` — and a later document cannot grant a denial back, so a `deny` is a
safety envelope, not an opening bid. A principle may only restrict; only a profile or protocol may
grant. Above all of it sits the approval floor: `aep/1` refuses to *resolve* a profile that grants
`production.write` outright, so the misconfiguration cannot exist, rather than being caught in
review.

**Enforced by:** `CapabilityPolicy::decide` and tests that construct the state where each precedence
link is load-bearing before asserting the outcome — verified by mutation, not by reading.

## 4. One write path, and refusals leave records

Every mutation of an engineering entity is a command through one boundary, carrying actor and
executor, correlation and causation, an idempotency key and an asserted revision. A second write
path is a second place to forget validation, authorisation, idempotency, provenance and audit. A
refused command changes nothing and is still recorded; nothing is physically deleted — archive and
supersede are the vocabulary, because a record whose history can be erased is not a record.

**Enforced by:** a test that enumerates every public method of every contract trait and pins the
list to one write path (`crates/aep-contract/tests/write_surface.rs`); `AuditRecord::validate`
rejecting a refusal that claims a change; and the command vocabulary having no delete variant — a
test asserts that parsing `aep.entity.delete/v1` fails.

## 5. Determinism, so decisions can be replayed and audited

Same validated state plus same evidence set produces the same decision; compiling or generating the
same source twice is byte-identical. The domain crates read no clock and no randomness; the engine
takes an injected `Clock`, so an execution replays exactly. This is what makes an audit trail
diffable and a generated tree drift-checkable at all.

**Enforced by:** banned-token scans over the ten crates that claim the property (no `SystemTime`, no
RNG, ordered maps instead of hash maps), plus tests that compile, diff, generate or render twice and
compare bytes. Crates that legitimately own a clock or a terminal are named as unscanned rather than
quietly exempted.

## 6. Parse, then validate — and refuse loudly, all at once

Documents deserialize into raw types and become domain types only through validation; validated
types do not implement `Deserialize`, so there is no path around validation. Validation accumulates:
a document with four broken references reports four errors, each with a stable code
(`unobservable_fact`, `dead_end_state`, …) that tests match on instead of message text. The target
of the design is the quiet failure — a rule that looks enforced and can never fire. A predicate
reading a fact nothing declares, a state nothing can reach, a rollback that cannot say what it rolls
back to: all refused at validation time.

**Enforced by:** a source scan asserting every raw/validated type pair keeps the split; non-optional
`ValidationCode` on every error; per-type tests asserting exact error counts.

## 7. One source of truth; everything else is generated and drift-checked

Rust types are the source of truth for schemas; the specification is the source of truth for
projections, suites and synthesized code. Derived output is committed for review — and CI regenerates
all of it and fails on any byte of difference, so a generated artifact cannot quietly diverge from
what would be generated today. A check whose toolchain is missing **fails and names the toolchain**
rather than skipping, because a skipped check reads exactly like a passing one.

**Enforced by:** the gate's drift steps — `schema-check`, `generate-check`, `suite-check`,
`synth-check` — all of which CI runs.

## 8. Semantics over transport

`CreateInvoice` is a command; `POST /invoices/commands/create-invoice` is one way to expose it. The
ESS model declares meaning — commands, outcomes, events, lifecycles — and transports are derived
projections. A component says where its callers are (`reached_by: network`), never which protocol to
speak; HTTP follows because an OpenAPI document is the one contract projected for a command surface.
This is what lets one specification compile to a monolith or to services, and to Rust, Go and a
browser target, without the domain changing — and what makes a generated test a statement about the
system rather than its HTTP layer.

**Enforced by:** the model's vocabulary (there is no way to write an endpoint into a domain), and by
the dual-target demonstration run in every gate: two applications synthesized from one
specification, compared on live exchanges and published contracts.

## 9. Fail closed, and say what you refused

Where the system cannot show something is current, it treats it as owed: conformance evidence binds
to the specification digest it attested, impact analysis has no vocabulary for "still valid", and a
specification artifact without a digest can never satisfy a conformance requirement. Symmetrically,
every refusal is treated as a product: it carries a stable code, names the rule and the path it is
about, and states what would unlock it — because a tool that says *no* without saying why teaches
nobody anything and gets routed around. The same discipline applies to generated output: a scenario
the specification cannot support is a printed refusal, an ungenerated capability is a named
obligation, and a target's weakness is a declared entry in its `TARGET.md`.

**Enforced by:** the digest checks in the `ess-conformance` principle and gate G19; the impact
report's construction (narrowing only); and refusal codes across every validator and runner.

---

Three principles here (1, 4, and the scans behind 5) were, for a time, stated but enforced by
nothing. They gained their enforcement in release `0.6.1-ess-wave-6.5`, and the register records
that history rather than smoothing it over — a claims register is only useful while it is honest in
both directions.

**Sources.** `AGENTS.md` § *Invariants* (each entry names its enforcing test, lint or scan);
`README.md` § *Design decisions worth knowing*; `CHANGELOG.md` § *0.6.1*;
`protocols/aep/1.yaml` (`approval_floor`).
