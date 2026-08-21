---
title: Track specification change
sidebar_position: 6
description: Compare two revisions semantically with ess diff, and compute what the change invalidates — scenarios and generated artifacts — with ess impact.
---

# Track specification change

Conformance evidence is bound to the specification digest it attested, so the moment a specification
moves, every requirement it satisfied goes back to owed. That is correct and blunt. `ess diff` and
`ess impact` make it proportionate: a typed statement of what moved, and a narrowing of what the
move actually invalidates.

## What moved: `ess diff`

The comparison is over two **compiled** models, not text. Moving declarations between files,
renaming files, reordering blocks and rewriting every comment report nothing; one line that removes
a currency reports one narrowing:

```console
$ protocol ess diff --from examples/revision-pair/before --to examples/revision-pair/after
catalog v2 → v2
  before  9aa886fb68a2447af40c92cf53ed260af0d102507ac87e73a8e31fb7d20a0916
  after   2dcf59ba04dd2fb953218bf8c60146d4efd4fca8282af8cd53c2063f4f4616be

6 change(s): 2 widening, 2 narrowing, 2 other

  widens   type catalog.pricing.Currency: variant `CHF` added
           type/catalog.pricing.Currency/variant-added/CHF
  narrows  type catalog.pricing.Currency: variant `GBP` removed
           type/catalog.pricing.Currency/variant-removed/GBP
  changes  entity catalog.pricing.PriceList: invariants [floor.amount >= 0] → [floor.amount > 0]
           entity/catalog.pricing.PriceList/invariants-changed
  changes  command catalog.pricing.CreatePriceList: outcome `created` is decided by `when floor.amount >= 1`, was `when floor.amount > 0`
           command/catalog.pricing.CreatePriceList/outcome-condition-changed/created
  narrows  actor catalog.pricing.Auditor: may no longer invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList
  widens   actor catalog.pricing.PricingManager: may invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.PricingManager/grant-added/catalog.pricing.RetirePriceList
```

`examples/revision-pair/` is that pair: exactly these six semantic changes buried under renamed
files, reordered blocks and rewritten comments.

Rules the report follows:

* **Ten construct families are compared:** system header, types, entities, commands, events,
  errors, views, actors, components, bindings.
* **Only set-membership changes carry a direction.** A grant or variant added *widens*, one removed
  *narrows*. Everything else is *changed* — a rewritten invariant is `changed` even when the new one
  is strictly stronger, because deriving that would be a proof, not a comparison. Predicates are
  compared for canonical equality only; implication is refused.
* **Nothing is inferred to be a rename.** `InvoiceCreated` removed and `InvoiceIssued` added is a
  removal and an addition, because a rename and a delete-plus-create have different consequences for
  everything already deployed.
* **One refusal:** two specifications naming different systems. The delta would be enormous,
  plausible, and an answer to a question nobody asked.

`--format json` writes the canonical `ess-diff/1` document — byte-identical for the same pair, each
change carrying an id derived from its own content, so a review comment can quote one and still mean
the same change later.

## What that invalidates: `ess impact`

A delta says what moved; `impact` says what **stood on** what moved — which conformance scenarios
are owed again, and which generated artifacts are owed regeneration:

```console
$ protocol ess impact --from examples/billing --to billing-with-one-grant-moved/ \
    --suite suites/generated/billing/suite.json

2 change(s): 1 widening, 1 narrowing, 0 other
  ...
suite billing v3 (13577b3c…): 7 of 29 scenario(s) owed again
2 construct(s) reached: 2 changed, 0 depend on one directly, 0 through another
9 of 37 generated artifact(s) owed regeneration

  billing.invoice.CreateInvoice/outcome/accepted
    directly-changed actor billing.invoice.Customer — actor/…/grant-removed/…CreateInvoice
  ...
```

Without this, moving one grant re-runs all 29 scenarios. Seven is the same answer, proportionate.

**Every impact carries the path that explains it** — not "these eleven things are affected" but
*this is affected because it references that, which references what you changed*:

```text
  catalog.pricing.PublishPriceList/outcome/published
    transitively-impacted entity catalog.pricing.PriceList — type/…/variant-removed/GBP
      -> type catalog.pricing.Money has a field of type type catalog.pricing.Currency
      -> type catalog.pricing.Headline wraps type catalog.pricing.Money
      -> entity catalog.pricing.PriceList has a field of type type catalog.pricing.Headline
```

`--suite` narrows scenarios; `--generated generated/` additionally checks each committed artifact's
stamped `contract_digest` against what its model slice computes — an artifact whose claim cannot be
read or does not hold is owed outright, stated as such.

## What it will never tell you

The analysis **narrows; it never says a result still holds**. A scenario absent from the output was
not reached by the closure — which is not a claim that its evidence stands. The two error directions
are not comparable: failing closed costs a re-run that was not needed; failing open costs a task
closing on evidence produced against a specification that has since moved. So the report has no
vocabulary for survival, and three situations put the whole suite back to owed:

| Situation | Why nothing narrows |
|---|---|
| the specification header itself changed — version, summary | no scenario names the system as a dependency, so no closure can start |
| the suite depends on a construct the dependency graph has no node for (conversions, workloads, a domain's naming have no compared family yet) | a closure could never reach it, and silently dropping it is the one wrong narrowing that looks right |
| the suite was produced from another revision or another system | **refused** rather than answered |
