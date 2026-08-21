# A revision pair

Two revisions of one specification, for `protocol ess diff` and `protocol ess impact`.

```console
protocol ess diff --from examples/revision-pair/before --to examples/revision-pair/after
```

Not a system anybody would run — a fixture, like [`oracle-fixture/`](../oracle-fixture/). It exists so
that the semantic diff has something to be **right about**, because the failure mode of a diff is
producing a plausible answer nobody checks.

## The six changes

`before/` and `after/` differ by exactly six semantic changes. Four are one per direction the
mechanical relations can take, and two — added by ESS wave 7.2 — are predicate edits, which can
only ever be *changed*:

| what an author did | what the delta says | relation |
|---|---|---|
| `Currency` gained `CHF` | `type/catalog.pricing.Currency/variant-added/CHF` | **widens** — a value that used not to parse now does |
| `Currency` lost `GBP` | `type/catalog.pricing.Currency/variant-removed/GBP` | **narrows** — a value that used to parse no longer does |
| the `PriceList` invariant became `floor.amount > 0` | `entity/catalog.pricing.PriceList/invariants-changed` | **changed** — strictly stronger, and saying so would be a proof, not a comparison |
| `created`'s guard became `floor.amount >= 1` | `command/catalog.pricing.CreatePriceList/outcome-condition-changed/created` | **changed** — canonically different predicates, no direction derived |
| `PricingManager` may now retire a price list | `actor/catalog.pricing.PricingManager/grant-added/…` | **widens** — a caller that could not do something now can |
| `Auditor` may no longer | `actor/catalog.pricing.Auditor/grant-removed/…` | **narrows** — an authorization that now fails |

The grant and variant rows are the only relations this repository claims to derive mechanically.
Everything else a delta reports is *changed*, which says the two revisions differ and that no
direction follows from the difference alone — a guard rewritten with different spacing, by
contrast, resolves to the same canonical predicate and is not a change at all
(`crates/ess-diff/tests/families.rs` proves that on a witness specification).

## The rest of the diff is a lie, and that is the point

A text diff between the two directories reports most of three files, one of which it cannot even pair
up because it was renamed. The semantic delta reports **six changes**. None of the rest is a change
to the system, and all of it is there deliberately:

| what moved in the text | why the delta says nothing |
|---|---|
| `domains/pricing.yaml` → `domains/catalog-pricing.yaml` | a document's identity is its `domain:` key, not its filename (invariant 10) |
| every top-level block is in a different order | every declaration is indexed by the name it declares |
| every comment is rewritten | comments are not in the model |
| `display: Auditor` written out on one side, left out on the other | the model falls back to the declaration's own last segment, so both spellings are the same display name |

`crates/ess-diff/tests/revision_pair.rs` asserts the delta's contents change by change, and asserts by
name that none of the four rows above reaches it. `crates/ess-diff/tests/impact.rs` asserts which of
the ten scenarios each change puts back to owed, and the exact path that explains one of them.

## What is in the fixture and why

One domain, one entity, three commands, three events, two errors, two actors, one component: the
smallest specification that carries one of each construct whose comparison the fixture is there to
prove — and nothing it does not. There is deliberately no view and no binding here: those families
are proven on the witness specification in `crates/ess-diff/tests/families.rs`, and the entity
invariant's conformance scenarios are refused by the synthesiser (no view publishes `floor.amount`),
which is itself the honest answer and part of what the fixture shows.

Two parts of it are there for the **impact** closure rather than for the delta, and both are
byte-for-byte identical in meaning across the two revisions, so neither reaches the delta:

| what | why it is here |
|---|---|
| `CreatePriceList`, the one outcome that `creates:` a price list | without it the specification obliges **no conformance suite at all** — every other scenario needs an instance to act on, and the synthesiser refuses each one for want of one. With it, the pair obliges nine scenarios, which is what makes "which scenarios does this change invalidate" a question with an answer |
| `Headline`, a newtype wrapping the `Money` struct | it keeps `Currency` out of everything the entity names directly, so a scenario that never mentions `Currency` is reached transitively — and the path, not the answer, is what a reader checks. Since the `floor` field arrived the *shortest* such path runs `Currency ← Money ← PriceList`, two hops, and that is the one the report explains |

```console
protocol ess conform synthesize --path examples/revision-pair/before --out /tmp/catalog
protocol ess impact --from examples/revision-pair/before --to examples/revision-pair/after \
  --suite /tmp/catalog/suite.json
```

All ten are owed again, and the counts per change are not the same: the two `Currency` changes and
the guard edit reach every scenario, the invariant edit reaches the nine that touch an instance (the
rejected creation touches none), and each grant change reaches only the four that act as *that*
actor. That asymmetry is the fixture's second job — a closure that reported ten for all six changes
would be indistinguishable from one that reported nothing at all.

Nothing generates from this directory. `cargo xtask generate` is pinned to `examples/billing`, and
`cargo xtask suite` to `examples/billing` and `examples/oracle-fixture`, so a construct added here
costs no committed output.
