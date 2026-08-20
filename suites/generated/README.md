# Generated conformance suites

**Do not edit these files.** They are generated from the specifications under
[`examples/`](../../examples) by `cargo xtask suite`, and CI fails if they differ from what
those specifications oblige.

A suite is the other half of a specification: every check an implementation has to pass for
the word *conformant* to mean anything about it. One JSON document per specification, keyed by
scenario id, holding no handle into any particular compilation — so a runner in another
language can read it, and a fault matrix can name a scenario by an id that does not move when
a sibling is added.

```console
protocol ess conform run --suite suites/generated/billing/suite.json --target billing
```

| suite | checks | scenarios | no scenario | generated from |
| --- | --- | --- | --- | --- |
| [`billing/suite.json`](billing/suite.json) | billing v3 (model digest e19d384dac86219a) | 27 | 1 | [`examples/billing`](../../examples/billing) |
| [`oracle-fixture/suite.json`](oracle-fixture/suite.json) | oracle v1 (model digest 4288d50a003fa7d5) | 31 | 6 | [`examples/oracle-fixture`](../../examples/oracle-fixture) |

## What no scenario covers

A construct the specification does not say enough about to test is refused rather than quietly
omitted (design §36). A refusal is a fact about the specification, not a gap in this file — and
it is listed here rather than left in a command's output because a suite holding fewer checks
than the specification requires is the one failure a passing run cannot show. Here it is a line
in a diff instead.

### `billing`

| code | element | the scenario that is missing |
| --- | --- | --- |
| `ESS-SYNTH-006` | `type billing.invoice.Money` | — |

What would close them:

* a later slice of `ess-conformance` synthesises this

### `oracle-fixture`

| code | element | the scenario that is missing |
| --- | --- | --- |
| `ESS-SYNTH-011` | `entity oracle.order.Order` | `oracle.order.Order/invariant/after/oracle.order.AmendOrder/amended` |
| `ESS-SYNTH-011` | `entity oracle.order.Order` | `oracle.order.Order/invariant/after/oracle.order.CancelOrder/cancelled` |
| `ESS-SYNTH-011` | `entity oracle.order.Order` | `oracle.order.Order/invariant/after/oracle.order.HoldOrder/held` |
| `ESS-SYNTH-011` | `entity oracle.order.Order` | `oracle.order.Order/invariant/after/oracle.order.PlaceOrder/accepted` |
| `ESS-SYNTH-011` | `entity oracle.order.Order` | `oracle.order.Order/invariant/after/oracle.order.ShipOrder/shipped` |
| `ESS-SYNTH-010` | `binding handoff-on-shipped` | `handoff-on-shipped/binding/on-failure` |

What would close them:

* `drop` is unobservable by design; write `escalate:` with an event if the failure has to be provable
* publish the fields the invariant reads in a view of this entity, or state the invariant over what one already publishes
