# `oracle` — a fixture, not a system

`examples/billing/` is the normative example. It is written to be read as the specification of a
real system, and it stays that way. This directory is the other thing: the smallest specification
that holds the constructs billing deliberately does not, so that the conformance oracle described in
[`docs/design/ess-closed-loop-execution-conformance-design-v0.1.md`](../../docs/design/ess-closed-loop-execution-conformance-design-v0.1.md)
has an input for every check it must be able to fail.

Nothing is generated from here. `cargo xtask generate` is pinned to `examples/billing` (the
`NORMATIVE_EXAMPLE` constant in `xtask/src/main.rs`), so a corner added here costs no committed
output.

## What is here that billing does not have, and why

| construct | here | why billing does not have it |
|---|---|---|
| `on_failure: retry` | `handoff-on-placed` | billing's one binding escalates |
| `on_failure: drop` | `handoff-on-shipped` | the one policy with nothing to observe, so the only input that makes synthesis **refuse** a check (§18) rather than emit one |
| three bindings on three events | `components.yaml` | with one binding, a dropped-binding fault fails every binding scenario there is; §26 wants the unrelated ones green |
| a mapping source with a same-typed sibling | `OrderPlaced.contact` beside `alternate_contact` | billing's `InvoiceCreated` carries one address, so a wrong mapping can only be a value the target invented, never a swap the document could have made |
| `updates:` | `AmendOrder` | billing uses `creates:` and `moves:` only, so §20's "evaluate invariants after a state-changing command" has no instance without a transition to hang on |
| a filtered `eventual` view | `HeldOrders`, `state == Held` | billing's eventual view has no filter, so its convergence is satisfied the moment the entity exists |
| a `read_your_writes` view filled by one command | `OpenOrders`, `state == Placed`, the initial state | billing's is two commands away, which is a weaker race to lose |

What billing already covers and this fixture does not duplicate: unions, enums, maps, optionals,
actors, a topology, and a wrong-event fault. Both examples carry an externally decided outcome,
because forcing a downstream failure is what makes the three `on_failure` policies distinguishable
at all.

## Running it

```console
protocol ess validate --path examples/oracle-fixture
protocol ess compile  --path examples/oracle-fixture
```

`crates/ess-compiler/tests/oracle_fixture.rs` asserts each row of the table above against the
compiled IR, so a corner cannot be deleted from these files without a named test failing.
