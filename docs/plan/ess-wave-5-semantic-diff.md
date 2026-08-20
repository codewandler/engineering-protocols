# ESS wave 5 — what changed, and what that invalidates

> **Proposed, and gated on wave 4 closing its loop — which it now has.** Design:
> [`ess-semantic-diff-impact-evolution-design-v0.1.md`](../design/ess-semantic-diff-impact-evolution-design-v0.1.md),
> reviewed against the code in
> [`2026-08-20-semantic-diff-feasibility-review.md`](../reviews/2026-08-20-semantic-diff-feasibility-review.md).
> Sequenced ahead of structural synthesis by decision, so this is wave 5 and structural synthesis
> moves out one.

**Goal: two compiled specifications, and a typed answer to what moved between them — then what that
answer invalidates.**

## Why this before structural synthesis

Wave 4 made a specification into an oracle. Every artifact this repository generates is now derived
from a model and checked against it: schemas, contracts, documentation, a conformance suite, and the
evidence that lets a task close.

All of it is derived from **one revision**. Nothing here can answer what happens when the model moves.
A specification changes, and every artifact downstream of it is silently either still true or quietly
wrong — and the only current answer is to regenerate everything and re-run everything, which is both
expensive and, worse, indistinguishable from having checked nothing.

That is the gap wave 4 opened rather than closed. Gate G19 bound conformance evidence to the
specification digest it was produced against, so evidence now **fails closed** when the model moves:
the requirement goes back to owed. Correct, and blunt. Every change invalidates every result,
including a change to a comment in an unrelated domain. A semantic delta is what makes that
proportionate — it is the difference between "the specification moved, re-run everything" and "these
four scenarios depend on what moved".

Structural synthesis, by contrast, generates *more* artifacts from one revision. Doing it first would
multiply the thing that has no change story.

## Two decisions, both taken

**The core is accepted, and the vision is amended to say so.** Accepting this was not only a wave:
[`docs/VISION.md`](../VISION.md)'s thesis described specifying a system once and compiling everything
else from that description, in the present tense. It now carries a second sentence — when the
description changes, what that invalidates is derived too — and the amendment is marked as an
amendment rather than smuggled in as a clarification.

The reason, stated so it can be argued with: a specification that cannot say what changed cannot
govern work on a system that already exists, and every real adoption is brownfield. A model you can
only use on day one is a model nobody adopts on day two. What stays outside the thesis is what a delta
*implies anyone should do* — it says what a revision invalidates, and a person decides what happens
next.

Accepted: the core, §9–§26. Rejected outright rather than deferred: §36's proposal-evaluation loop and
§38's architecture search, which the vision refuses by name. Still blocked: everything after §31,
which depends on symbols that exist in no code.

**Invalidation fails closed.** The design has evidence failing open — §33's verdict vocabulary
includes "still valid", and §26 counts invalidated records as a *subset*, which puts a delta engine in
the position of deciding which prior results survive. Gate G19 made the opposite choice, and it stands.

The argument is what a missed dependency edge costs. Failing closed, it costs a re-run of something
that did not need re-running. Failing open, it costs a false conformance claim — a task closing on
evidence produced against a specification that has since moved. Those are not comparable errors, and a
delta engine is exactly the kind of component whose edge cases are discovered late.

So §32 **refines** G19 and cannot replace it: a delta may narrow what has to be re-established, and it
may never mark something still valid that G19 would have put back to owed. The design is wrong here
and gets corrected before it is implemented from, the way wave 4's design was reconciled before its
first slice.

## What is in, and what is explicitly not

The review measured the whole design at four waves plus a blocked programme. This page takes the
first slice only.

| track | sections | state |
|---|---|---|
| the delta and impact closure | §9–§26 | **this wave** |
| evidence and scenario invalidation | §32, §33 | needs wave 4's evidence, which now exists — but see below |
| generated-artifact and obligation invalidation | §33 | blocked: `contract_digest` and `Realization` exist in no code, only in an unsequenced design |
| change proposals, LLM evaluation, architecture search | §35, §36, §38 | **rejected** — §36 is an orchestration loop and §38 a search driver over it, which `docs/VISION.md` refuses by name. §37, change budgets, is a deterministic check over deterministic facts and is fine |

## W5.1 — the delta, and one fixture pair that bites

Six construct families whose IR coverage is complete and whose comparison needs no unknowns:
**system, types, events, errors, actors, components**. Deliberately excluded from the first slice:
entities and commands, because their invariants and conditions are predicates and predicate
comparison is where an undecidable answer lives; and views, bindings, topology and conversions.

Four mechanically derivable relations and no more: a grant added is a widening, a grant removed is a
narrowing, an enum or union variant added is a widening, one removed is a narrowing. Everything else
is simply *changed*. There is no undecidable answer in this slice, which is the whole reason for
choosing those six families first.

`protocol ess diff --from <dir> --to <dir> --format text|json`, joining the five `ess` verbs that
already exist. Canonical output with a byte-identical test, generalised from the one the compiler
already has. A raw-to-validated pair for the JSON form, because it is read back and invariant 2
applies.

One refusal only: comparing two different systems. Everything else the design proposes to refuse
waits for a reason to exist.

**The fixture is the deliverable, not the type.** A pair of specifications differing by four changes,
one per relation, so the delta has something to be right *about*. The failure mode of a diff is
producing a plausible answer nobody checks, which is the same failure mode as a schema that accepts
everything — and this repository has now shipped that defect three times and caught it three ways.

## W5.2 — impact closure, and what it is allowed to say

A dependency graph over the IR, and a closure from a change to what depends on it, with the path
recoverable — the design is right that an impact nobody can explain is an impact nobody will act on.

The constraint that shapes it: wave 4's suite already records, per scenario, the set of constructs it
depends on. That was built deliberately in the scenario IR rather than recording only what caused a
scenario to exist, because reconstructing it afterwards means regenerating the suite. So the first
consumer of impact closure already exists and already carries the edges it needs.

## What has to be true before this starts

* Wave 4 tagged, and its committed suites drift-checked in CI. **Done.**
* The fail-closed polarity settled — **done**, above — and the design corrected where it contradicts
  G19, which is an edit to `docs/design/` that has not been made yet.
* The vision amendment taken — **done**.
* Two change kinds in the taxonomy name fields the model does not have, and one is attached to the
  wrong construct. Correct them in the design before implementing from it, the way wave 4's design was
  reconciled before its first slice.

## Not in this wave

Structural synthesis and obligations, infrastructure, formal verification, the proposal-evaluation
loop, and anything depending on `contract_digest` or `Realization`. Each is named in a design and none
is accepted.
