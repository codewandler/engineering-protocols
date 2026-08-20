---
title: "0.5 — what moved, and what that invalidates"
description: >
  Release 0.5.0-ess-wave-5 adds two verbs: a semantic diff over two revisions of a specification,
  and an impact closure that says which conformance results a change puts back to owed — with the
  path that explains each one.
slug: semantic-diff
tags: [release, ess]
---

Release `0.5.0-ess-wave-5` adds two verbs to the CLI. `protocol ess diff` answers *what actually
moved* between two revisions of a specification — as typed changes over the compiled models, not as
text. `protocol ess impact` answers the question that makes the first one worth asking: *which
verification results does that movement invalidate*, and by exactly what path.

This post is a tutorial on both, run against a fixture that ships in the repository, so every output
below is reproducible — and checked by tests, not pasted from memory.

{/* truncate */}

## The problem, in one commit

Somebody — increasingly, some *agent* — revises a specification. Between the two revisions,
`git diff` reports:

```text
 components.yaml                 |   4 +-
 domains/catalog-pricing.yaml    | 192 +++++++++++----------
 system.yaml                     |  12 +-
 3 files changed, 109 insertions(+), 99 deletions(-)
```

Two hundred changed lines across three files, one of which git can only pair up by rename detection.
A reviewer now owes an answer to two questions, and the text diff helps with neither:

1. **What did this change about the system?** Not about the files — about what the system means.
2. **What do we have to re-verify?** Every conformance result was produced against the old revision.
   Which of them still stand?

Before 0.5 this repository had exactly one honest answer to the second question: *none of them* —
evidence is bound to the specification digest it was produced against, so any change puts every
result back to owed. Correct, and blunt. A change to one comment cost a full re-run, and a full
re-run after every change is indistinguishable from never having checked anything.

## What the semantic diff says

The fixture is [`examples/revision-pair/`](https://github.com/codewandler/engineering-protocols/tree/main/examples/revision-pair)
— two revisions of a small pricing system, built so the diff has something to be *right about*:

```console
protocol ess diff --from examples/revision-pair/before --to examples/revision-pair/after
```

```text
catalog v2 → v2
  before  bc6f70b3dc81a99d
  after   3e5ba8c16baf2d7d

4 change(s): 2 widening, 2 narrowing, 0 other

  widens   type catalog.pricing.Currency: variant `CHF` added
           type/catalog.pricing.Currency/variant-added/CHF
  narrows  type catalog.pricing.Currency: variant `GBP` removed
           type/catalog.pricing.Currency/variant-removed/GBP
  narrows  actor catalog.pricing.Auditor: may no longer invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList
  widens   actor catalog.pricing.PricingManager: may invoke `catalog.pricing.RetirePriceList`
           actor/catalog.pricing.PricingManager/grant-added/catalog.pricing.RetirePriceList
```

Two hundred changed lines of text are **four changes to the system**. Both sides are compiled first,
and the comparison runs over the two intermediate representations — so everything that changes the
files without changing the model reaches nothing, and each of those non-changes is in the fixture
deliberately, asserted by name in a test:

| what moved in the text | why the delta says nothing |
|---|---|
| `domains/pricing.yaml` renamed to `domains/catalog-pricing.yaml` | a document's identity is its `domain:` key, not its filename |
| every top-level block reordered | declarations are indexed by the name they declare |
| every comment rewritten | comments are not in the model |
| a display name written out on one side, defaulted on the other | both spellings resolve to the same name |

### A change carries a direction — where one can be derived

The four changes above are not merely *listed*; each carries a relation, and the relations are the
point:

| what an author did | relation | what that means for a running system |
|---|---|---|
| `Currency` gained `CHF` | **widens** | a value that used not to parse now does |
| `Currency` lost `GBP` | **narrows** | a value that used to parse no longer does — existing data may now be refused |
| `PricingManager` may now retire a price list | **widens** | a caller that could not do something now can |
| `Auditor` may no longer | **narrows** | an authorization that used to succeed now fails |

A grant added is a *security-relevant fact*, and it is now a typed one. A reviewer reading a text
diff has to reconstruct "this widens who can do what" from YAML hunks; here it is the first thing on
the line. These four relations — grant added/removed, variant added/removed — are the only ones the
tool claims to derive mechanically. Everything else is reported as *changed*, which is an answer,
not a shrug: the revisions differ, and no direction follows from the difference alone.

## What that invalidates

The diff alone is a better code review. The second verb is what makes it *operational*. This
repository generates a conformance suite from a specification — scenarios an implementation must
pass, with results recorded as evidence. Synthesize the suite for the old revision:

```console
protocol ess conform synthesize --path examples/revision-pair/before --out /tmp/catalog
protocol ess impact --from examples/revision-pair/before --to examples/revision-pair/after \
  --suite /tmp/catalog/suite.json
```

The output opens with the same four-change delta, then answers for each of the suite's nine
scenarios. One of them, in full:

```text
  catalog.pricing.PriceList/transition/retire/by/catalog.pricing.RetirePriceList/retired
    transitively-impacted entity catalog.pricing.PriceList — type/catalog.pricing.Currency/variant-removed/GBP
      -> type catalog.pricing.Money has a field of type type catalog.pricing.Currency
      -> type catalog.pricing.Headline wraps type catalog.pricing.Money
      -> entity catalog.pricing.PriceList has a field of type type catalog.pricing.Headline
    directly-changed actor catalog.pricing.Auditor — actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList
```

Read the middle three lines again. The scenario never mentions `Currency`. It is invalidated anyway,
because the entity it acts on holds a `Headline`, which wraps `Money`, which has a `Currency` field
— three declarations away. That chain is the deliverable:

```mermaid
flowchart LR
    S["scenario:<br/>retire a PriceList"] --> P[entity PriceList]
    P -->|has a field of type| H[type Headline]
    H -->|wraps| M[type Money]
    M -->|has a field of type| C[type Currency]
    C === X(["changed: variant GBP removed"])
```

An impact nobody can explain is an impact nobody will act on. Every invalidation carries the path
that produced it, one hop per line, so an answer you disbelieve is an answer you can inspect —
and a wrong edge in the graph is a bug you can point at, not a vibe.

## What it will never say

The verb's most important property is a sentence it cannot form: **"this result is still valid."**

Invalidation here *fails closed*. A scenario absent from the output was not reached by this
analysis — which is not a claim that its evidence still stands. The base rule is unchanged: any
change to the specification digest puts every conformance requirement back to owed. Impact *narrows*
what has to be re-established; it never marks anything as surviving. That polarity is structural,
not a convention someone has to remember:

- there is no "still valid" verdict in the vocabulary, and no method that returns one
- the only way to combine two answers is a join whose top element is *invalidate everything*
- a change to the system header invalidates everything
- a dependency the graph does not recognise invalidates everything
- a suite whose digest does not match the `--from` revision is refused, not narrowed

The asymmetry behind the design: a missed dependency edge under fail-closed costs a re-run that was
not needed. The same missed edge under fail-open costs a **false conformance claim** — a task closed
on evidence produced against a specification that has since moved. Those are not comparable errors,
and a dependency graph is exactly the kind of component whose missing edges are discovered late.

## The honest numbers

On the repository's normative example — a billing system obliging 27 conformance scenarios — the
narrowing is real and *uneven*, and the unevenness is worth publishing rather than hiding:

| change | scenarios owed again |
|---|---|
| any change at all, before 0.5 | 27 of 27 |
| move an actor's grant | **7 of 27** |
| change an enum variant | 23 of 27 |

Authority changes are where the narrowing pays. Type changes barely narrow — and that is a true fact
about the system, not a defect in the analysis: nearly every scenario acts on an entity, and a type
that most entities reach is genuinely reached by most scenarios. A closure that reported dramatic
narrowing for every change would be describing a system nobody specified.

Two more limits, stated so they can be held against us:

- **Entities and commands are not yet compared.** Their invariants and conditions are predicates,
  and predicate comparison is where undecidable answers live. This slice compares the six construct
  families whose comparison needs no unknowns — system, types, events, errors, actors, components —
  and a change it cannot follow invalidates the whole suite, per the polarity above.
- **The diff refuses exactly one input:** two specifications that name different systems. That is a
  comparison with no meaning, and the tool says so instead of producing a plausible-looking answer.

## Why this matters for agent-driven engineering

The premise of this project is that agents produce the change volume of a much larger team, and that
review-by-reading does not survive that volume. Wave 5 is that premise applied to the specification
itself:

- an agent revises the model; the *reviewable object* is four typed changes, two of them
  authority-relevant, not two hundred lines of YAML
- verification re-runs in proportion to what moved, on evidence, with the reasoning attached
- and the one thing the machinery never does is quietly decide that old evidence still counts

The full record for the wave — what was accepted, what was rejected by name, and the two decisions
it forced — is in the repository under
[`docs/plan/ess-wave-5-semantic-diff.md`](https://github.com/codewandler/engineering-protocols/blob/main/docs/plan/ess-wave-5-semantic-diff.md).
Tag: [`0.5.0-ess-wave-5`](https://github.com/codewandler/engineering-protocols/releases/tag/0.5.0-ess-wave-5).
