# A revision pair

Two revisions of one specification, for `protocol ess diff` and `protocol ess impact`.

```console
protocol ess diff --from examples/revision-pair/before --to examples/revision-pair/after
```

Not a system anybody would run — a fixture, like [`oracle-fixture/`](../oracle-fixture/). It exists so
that the semantic diff has something to be **right about**, because the failure mode of a diff is
producing a plausible answer nobody checks.

## The four changes, one per relation

`before/` and `after/` differ by exactly four semantic changes, and each one is a different answer to
"which way did this go":

| what an author did | what the delta says | relation |
|---|---|---|
| `Currency` gained `CHF` | `type/catalog.pricing.Currency/variant-added/CHF` | **widens** — a value that used not to parse now does |
| `Currency` lost `GBP` | `type/catalog.pricing.Currency/variant-removed/GBP` | **narrows** — a value that used to parse no longer does |
| `PricingManager` may now retire a price list | `actor/catalog.pricing.PricingManager/grant-added/…` | **widens** — a caller that could not do something now can |
| `Auditor` may no longer | `actor/catalog.pricing.Auditor/grant-removed/…` | **narrows** — an authorization that now fails |

Those four are the only relations this repository claims to derive mechanically. Everything else a
delta reports is *changed*, which says the two revisions differ and that no direction follows from the
difference alone.

## The rest of the diff is a lie, and that is the point

A text diff between the two directories reports most of three files, one of which it cannot even pair
up because it was renamed. The semantic delta reports **four changes**. None of the rest is a change
to the system, and all of it is there deliberately:

| what moved in the text | why the delta says nothing |
|---|---|
| `domains/pricing.yaml` → `domains/catalog-pricing.yaml` | a document's identity is its `domain:` key, not its filename (invariant 10) |
| every top-level block is in a different order | every declaration is indexed by the name it declares |
| every comment is rewritten | comments are not in the model |
| `display: Auditor` written out on one side, left out on the other | the model falls back to the declaration's own last segment, so both spellings are the same display name |

`crates/ess-diff/tests/revision_pair.rs` asserts the delta's contents change by change, and asserts by
name that none of the four rows above reaches it. `crates/ess-diff/tests/impact.rs` asserts which of
the nine scenarios each change puts back to owed, and the exact path that explains one of them.

## What is in the fixture and why

One domain, one entity, three commands, three events, one error, two actors, one component: the
smallest specification that carries one of each construct the first slice of the semantic diff
compares — system, types, events, errors, actors, components — and nothing it does not.

Two parts of it are there for the **impact** closure rather than for the delta, and both are
byte-for-byte identical in meaning across the two revisions, so neither reaches the delta:

| what | why it is here |
|---|---|
| `CreatePriceList`, the one outcome that `creates:` a price list | without it the specification obliges **no conformance suite at all** — every other scenario needs an instance to act on, and the synthesiser refuses each one for want of one. With it, the pair obliges nine scenarios, which is what makes "which scenarios does this change invalidate" a question with an answer |
| `Headline`, a newtype wrapping the `Money` struct | it puts `Currency` three declarations away from the entity that ends up holding it, so a scenario that never mentions `Currency` is reached transitively — and the path, not the answer, is what a reader checks |

```console
protocol ess conform synthesize --path examples/revision-pair/before --out /tmp/catalog
protocol ess impact --from examples/revision-pair/before --to examples/revision-pair/after \
  --suite /tmp/catalog/suite.json
```

All nine are owed again, and the counts per change are not the same: the two `Currency` changes reach
every scenario, through the entity every one of them creates or moves, while each grant change
reaches only the four or five that act as *that* actor. That asymmetry is the fixture's second job —
a closure that reported nine for all four changes would be indistinguishable from one that reported
nothing at all.

Nothing generates from this directory. `cargo xtask generate` is pinned to `examples/billing`, and
`cargo xtask suite` to `examples/billing` and `examples/oracle-fixture`, so a construct added here
costs no committed output.
