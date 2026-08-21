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
  plausible, and an answer to a question nobody asked, so the answer is exit 1 and one line:

  ```text
  refused: these are two systems, not two revisions: `billing` and `catalog`
  ```

`--format` takes `text` or `json`, and nothing else. `--format json` writes the canonical
`ess-diff/1` document — byte-identical for the same pair, each change carrying an id derived from its
own content, so a review comment can quote one and still mean the same change later.

## What that invalidates: `ess impact`

A delta says what moved; `impact` says what **stood on** what moved — which conformance scenarios
are owed again, and which generated artifacts are owed regeneration. This repository ships one
revision of billing, so make the second one: copy it and move a single grant from one actor to
another.

```console
$ NEXT=$(mktemp -d)/billing && cp -r examples/billing "$NEXT"
$ # in $NEXT/domains/invoice.yaml, move `billing.invoice.CreateInvoice`
$ # from actor `billing.invoice.Customer`'s `may:` list to `billing.invoice.Auditor`'s
$ protocol ess impact --from examples/billing --to "$NEXT" \
    --suite suites/generated/billing/suite.json | head -18
billing v3 → v3
  before  13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  after   c37415bc4c4d2dc113af19f38be2affc909fdc443fe7b080dbc4a9ef757cfab8

2 change(s): 1 widening, 1 narrowing, 0 other

  widens   actor billing.invoice.Auditor: may invoke `billing.invoice.CreateInvoice`
           actor/billing.invoice.Auditor/grant-added/billing.invoice.CreateInvoice
  narrows  actor billing.invoice.Customer: may no longer invoke `billing.invoice.CreateInvoice`
           actor/billing.invoice.Customer/grant-removed/billing.invoice.CreateInvoice

suite billing v3 (13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861): 7 of 29 scenario(s) owed again
2 construct(s) reached: 2 changed, 0 depend on one directly, 0 through another
9 of 37 generated artifact(s) owed regeneration

  billing.invoice.CreateInvoice/outcome/accepted
    directly-changed actor billing.invoice.Customer — actor/billing.invoice.Customer/grant-removed/billing.invoice.CreateInvoice
  billing.invoice.CreateInvoice/outcome/rejected
```

The delta comes first and in full; the suite section follows it. `head -18` above cuts the remaining
five scenarios and the artifact section, each of which reads like the two shown.

Without this, moving one grant re-runs all 29 scenarios. Seven is the same answer, proportionate.

`--suite` is what adds the scenario section. **Without it, the report answers for the generated
artifacts alone** — the same two-line count and the same explained paths, and no claim about any
suite. `--generated generated/` goes further and checks each committed artifact's stamped contract
digest against what its model slice computes; an artifact whose claim cannot be read or does not hold
is owed outright — *its committed contract digest is `e6e58e0…`, and its slice computes `d2b4806…`: a
false claim about derivation* — rather than counted as reached.

`--format json` writes the canonical `ess-impact/2` document — a different document from `diff`'s,
which is why `impact` is a verb and not a flag.

**Every impact carries the path that explains it** — not "these eleven things are affected" but
*this is affected because it references that, which references what you changed*. The
`examples/revision-pair/` pair shows it without any editing, because its `before/` obliges a suite
you can synthesise on the spot:

```console
$ SUITE=$(mktemp -d)
$ protocol ess conform synthesize --path examples/revision-pair/before --out "$SUITE" >/dev/null
$ protocol ess impact --from examples/revision-pair/before --to examples/revision-pair/after \
    --suite "$SUITE/suite.json" | grep -A 3 'PublishPriceList/outcome/published'
  catalog.pricing.PublishPriceList/outcome/published
    transitively-impacted entity catalog.pricing.PriceList — type/catalog.pricing.Currency/variant-added/CHF
      -> type catalog.pricing.Money has a field of type type catalog.pricing.Currency
      -> entity catalog.pricing.PriceList has a field of type type catalog.pricing.Money
```

That scenario never mentions `Currency`. The two hops are why it is owed again anyway, and they are
what a reviewer checks — the path, not the verdict.

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
| the suite was produced from another revision or another system | **refused** rather than answered: *the suite checks `billing` and these are two revisions of `catalog`* — exit 1, no report |

And one thing it does not look at all: **prose**. A design doc, a runbook or a README that quotes a
specification is not in the model, so no closure reaches it and no count includes it. The verb for
that class of claim is a different one — `protocol evidence scan` reads dated claims out of markdown
and says which of them nobody has looked at since.
