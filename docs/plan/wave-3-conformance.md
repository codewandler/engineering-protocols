# Wave 3 — conformance, and the domain profiles

> **Delivered.** Status after: the v0.2 scope is implemented. 424 tests.

Goal: **let a backend nobody in this repository wrote prove it implements the contract.** Wave 2
delivered the contract and one implementation of it. A contract with exactly one implementation is a
description of that implementation.

Projected status: **≈83% → 100% of the v0.2 initial scope.**

## Why conformance is the last thing and not the first

A conformance suite written before an implementation tests what its author imagined. Written after
one, it tests what the contract actually requires — and the in-memory backend from wave 2 exists
precisely so the suite has something to be checked against while it is being written.

The suite's own acceptance test: it must **fail** against a backend deliberately broken in each of the
ways the suite exists to catch. A suite that passes everything is not a suite.

---

## W3.1 `aep-conformance` — the suites

Sixteen suites from §78, each a function generic over `S: CommandService + QueryService`:

| suite | what it refuses to accept |
|---|---|
| identity | ids that are keys; identity reused after archival |
| command execution | a mutation that leaves no revision or no audit record |
| idempotency | a replay that applies twice, or returns a different result |
| optimistic concurrency | a stale write that merges instead of failing |
| query | a filter that is silently ignored |
| consistency | `AtLeast(token)` answered from an older view |
| relations | an edge that resolves to nothing; a missing inverse |
| history | a revision missing from the record |
| immutability | an edited review result |
| audit | a mutation with no record |
| rejected-action audit | a refusal with no record, or one carrying a change |
| correlation | an activity that cannot be reassembled from one id |
| causation | a chain with a broken link |
| provenance | an entity with no creator |
| events | an event caused by a command that does not name it |
| type registry | a type that cannot be described |

Each returns a structured `SuiteReport { suite, checks: Vec<Check{name, passed, detail}> }`, so a
failure says which property broke rather than which assertion fired.

## W3.2 Conformance fixtures and levels

`conformance/fixtures/`, `conformance/scenarios/`, `conformance/expected/` — language-neutral, so a
backend written in another language can be driven from the same inputs (§84).

Conformance **levels** (§85): `core` (identity, commands, idempotency, concurrency, query),
`audited` (core plus audit, correlation, causation, provenance), `full` (everything). A backend
states the level it claims and the suite proves or refutes it.

## W3.3 `protocol conformance`

`protocol conformance --level core|audited|full` runs the suites against the in-memory backend, and
`--backend <url>` against a remote one once an adapter exists. Output: one line per check, exit 1 on
any failure.

## W3.4 A deliberately broken backend

`aep-conformance` ships `FaultyBackend`, a wrapper that injects exactly one fault at a time —
duplicate-applies a replay, merges a stale write, drops the audit record for a refusal. The suite's
own tests assert that each fault is caught by the suite that exists to catch it. This is what stops
the suite from quietly becoming a smoke test.

## W3.5 `adp-domain` and `aop-domain`

The development and operations entity types and commands (§4.2, §4.3): `adp.test-plan/v1`,
`adp.specification/v1`; `aop.incident/v1`, `aop.runbook/v1`, `aop.release/v1`, with the commands
those types need (`AcknowledgeIncident`, `MitigateIncident`, `ResolveIncident`, `PromoteRelease`,
`RollbackRelease`).

Sized last on purpose: they are the least load-bearing part of the design and the easiest to get
wrong without a real user.

## W3.6 Documentation pass

A `docs/guide/` written for someone adopting this: how to write a profile for their organisation, how
to wire a harness to the engine, how to implement and prove a backend. Currently that knowledge is
spread across `AGENTS.md`, the authoring brief and the design specification, which is three places
too many.

---

## Risks

| Risk | Handling |
|---|---|
| The suite tests the in-memory backend's habits rather than the contract | `FaultyBackend` (W3.4), and every check phrased as a property of the contract, not an equality against a known output |
| Conformance levels multiply into a matrix nobody runs | three levels, named after what they let you trust, and the CLI runs `full` by default |
| ADP/AOP types are invented rather than needed | derive them from the incident and release profiles that already exist; anything no document references does not get written |
