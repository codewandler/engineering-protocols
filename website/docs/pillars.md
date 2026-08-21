---
title: What this insists on
sidebar_position: 4
description: Seven commitments, the mechanism enforcing each, and the three that are currently enforced by nothing.
---

# What this insists on

Seven commitments run through both halves. Each is stated here with the mechanism that holds it up,
because a commitment with no mechanism is a sentence, and this project exists to replace sentences.

The repository keeps sixteen invariants with the same discipline — each one carries what actually
enforces it, and **three of them say "nothing"**. Those three are named here too, rather than
smoothed over. They are on [what you still have to trust](./status/what-you-have-to-trust.md).

## 1. Evidence over assertion

A task does not complete because an agent says the work is done. It completes when a predicate over
recorded facts is true.

An evidence requirement can be marked `independent: true`, and an agent's own submission never
satisfies it — `Producer::Agent` and `Producer::Verifier` are different variants, and the requirement
reads which one produced the record. Ordering is a fact too: `evidence.first_seq.test_result <
evidence.first_seq.diff` is how red-before-green is checked.

**Where it holds and where it does not.** The engine evaluates what verifiers and humans produced and
never manufactures a fact. That is a property of how the API is used, not a checked rule: nothing in
the workspace prevents a harness from submitting a `TestResult` it invented, and the harness guide
says so outright.

## 2. `Unknown` is not `False`

`tests.unit.failed == 0` is *false* when a suite failed and *unknown* when nothing ran. A harness
needs different behaviour in each case — fix the code, or go run the tests.

Predicate evaluation is three-valued and only `True` permits a transition, so nothing is loosened by
the third value. The `Truth` type has three variants, Kleene `and`/`or`, no `From<bool>` and no
`as_bool` — there is no boolean to collapse into.

An explanation distinguishes the two on sight: `✗` is a fact that is wrong, `?` is a fact nobody has
observed.

## 3. Capabilities default to deny

A capability no document mentions is not granted. `deny` beats `require_approval` beats `allow`, and
a `deny` cannot be granted back by a later document, so the policy works as a safety envelope rather
than as a starting suggestion.

A principle may restrict; only a profile or a protocol may grant. And some capabilities cannot be
granted outright at all: `aep/1` holds `production.write` and `deployment.create:production` in an
**approval floor**, so a profile that granted them would fail to resolve.

## 4. Every mutation is a command, and a refusal is still recorded

There is one write path, because a second path is a second place to forget validation, authorisation,
idempotency, provenance and audit. A command carries actor *and* executor, correlation and causation,
an idempotency key and an asserted revision — so a retry is recognised, and a stale write is refused
rather than merged.

A refused command changes nothing and is still recorded. `AuditRecord::validate` rejects a rejection
that carries a change record: the audit trail cannot claim a refusal changed something.

Asking is itself an event. Authorising an action takes a mutable execution because the request and
its answer both land in the trail, including the denials.

**Honestly:** the one-write-path rule is a property of the contract's current shape. A second write
path would compile and pass the gate.

## 5. Nothing is deleted

`ArchiveEntity` and `SupersedeEntity` are the vocabulary. There is no delete command to call, and a
test asserts that parsing the command kind `aep.entity.delete/v1` fails, naming the kind it refused.

An engineering record whose history can be erased is not a record.

## 6. Determinism

Same validated state plus same evidence set produces the same decision. Iteration is over ordered
maps and sets, never hash maps, so output ordering is stable; the compiler reads no clock and no
randomness, and compiling the same source twice is byte-identical — asserted by a test rather than
claimed by a comment.

**Honestly:** the banned-token scan that enforces this covers the ESS compiler only. The other twelve
workspace members are unscanned.

## 7. A specification compiles into its own contracts

One model, and everything else derived from it: Markdown with diagrams, JSON Schema per command
input, event and error payload, an OpenAPI 3.1 document per component, an AsyncAPI 3.0 document per
component. Every artifact carries its provenance — specification version, a digest of the resolved
model, compiler and generator versions — and a check fails the build when committed output no longer
matches the specification.

**And, since this page was written, the tests and the structural code.** A generated conformance
suite arrived as ESS wave 4, structural synthesis as wave 6, and wave 7 grew the latter to three
targets and a running HTTP application per compiled emitter. Behaviour is still never generated —
every algorithm is a typed obligation someone has to implement.
[Where this stands](./status/where-this-stands.md) keeps the current numbers.

---

**Sources.** `AGENTS.md` § *Invariants* (each with its stated enforcement, including invariants 7, 8
and 14, which state "nothing", and invariant 9, whose scan covers `crates/ess-compiler/src` only);
`README.md` § *Design decisions worth knowing*; `docs/guide/harness.md` § *Never manufacture
evidence*; `protocols/aep/1.yaml` (`approval_floor`); `crates/aep-domain/src/audit.rs`;
`crates/aep-domain/src/command.rs`.
