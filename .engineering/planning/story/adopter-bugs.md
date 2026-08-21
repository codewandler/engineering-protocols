---
format: aep.planning-md/1
id: story:adopter-bugs
kind: story
status: draft
title: 'The unambiguous ones: the fallback lifecycle, the kind ladder, the project merge'
summary: A1, A2, A3 plus B2's compile-time directory name and G2's untyped failure policy — five defects that need no decision.
owner: protocol
tags:
- adoption
- bug
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: The unambiguous ones — the fallback lifecycle, the kind ladder, the project merge

## Outcome

The five defects that need no decision are gone, and each one's fix is asserted by a test rather than
observed once. An adopter following `docs/guide/adopting.md` gets the behaviour the guide describes.

## Context

An early adopter's review, round 1 — **items A1, A2, A3, plus B2 and G2** — last in the
adopter's ranked order and first in cost. They are grouped as one story because they are one kind of
work: no design question, no vocabulary widening, five small corrections with a test each.

- **A1** — the fallback lifecycle is documented and unreachable. `artifact.rs:1387` documents *"absent
  for the fallback lifecycle"* and `LifecycleRegistry::for_kind` implements the fallback, but a
  lifecycle document that omits `kind:` fails validation with `empty_declaration`. Either a document
  may omit `kind:` to register the fallback, or the comment goes.
- **A2** — kind-hierarchy fallback is dead for custom kinds. `ArtifactKind` accepts any kebab-case
  name; `parent()` is defined over the built-in variants only, so `digest`, `briefing` and `insight`
  cannot share a ladder. Their suggested rule: last-hyphen-segment parent for custom kinds, which is
  what `feature-design → design` already does.
- **A3** — the guide implies project-local workflows; `ProjectPaths` merges only principles and
  profiles. A team whose work has a new shape must vendor a protocol tree, and the guide does not say
  so. Their workaround was `protocols: .`.
- **B2** — `PROJECT_DIRECTORY` is a compile-time constant (`.engineering`): no config, no env, no
  flag, so renaming to `.workflow` kills walk-up discovery. Minor on its own; it rides here because it
  is a one-line surface and its *class* is `story:open-vocabulary-audit`.
- **G2** — `FailurePolicy` is loosely typed (`additionalProperties`), so an invented parameter
  validates silently: `on_failure: {action: retry, retry: {to: write}}` passed and meant nothing. **A
  policy that validates and does nothing is a gate that cannot fire**, which is the same defect class
  as a muted gate with no exit criterion.

**A1 and A2 interact, and the precedence is stated rather than emergent.** With last-segment
parenting for custom kinds *and* a kindless document registering the fallback, a lookup for
`weekly-digest` could resolve through the `digest` parent chain **or** through the global fallback,
and whichever the code happened to try first would become the rule nobody wrote down. The required
order is **exact kind → parent chain, nearest ancestor first → global fallback last**: the most
specific ladder an adopter declared always wins, and the fallback is what is left when nothing
matched. It is implemented in `for_kind`, documented on the method *and* in the lifecycle-document
guidance, and pinned by a disambiguation test rather than by reading the implementation.

**Being worked in parallel, 2026-08-21.** Another agent is fixing these in `crates/` on the same day
this story was written, so it may reach `implemented` almost immediately. That is not a reason to skip
the record: the story is what the fix is reviewed against, and A3's half — deciding whether the
project merge gains workflows or the guide states the vendoring rule plainly — is a documentation
decision that a code fix does not make on its own.

## Acceptance

- A lifecycle document that omits `kind:` either registers the fallback or is refused for a reason
  that matches the code's own comment; the comment and the behaviour agree, asserted by a test.
- A custom kind with a hyphen resolves a parent by the last-segment rule, and `digest`, `briefing` and
  `insight` can be given one ladder in a fixture tree.
- **Resolution precedence is exact kind, then the parent chain nearest-ancestor-first, then the global
  fallback** — stated on `for_kind` and in the lifecycle-document guidance, and asserted by a
  disambiguation test: a tree registering both a `digest` lifecycle and a kindless fallback resolves
  `weekly-digest` to the `digest` ladder, and a kind with no chain match resolves to the fallback.
- `docs/guide/adopting.md` and `ProjectPaths` agree about what a project may merge — either the merge
  widens or the guide states the vendoring rule, and the guide's example runs as written.
- The project directory name is resolvable without recompiling, or the constant is documented as fixed
  with the reason.
- An unknown key in a `FailurePolicy` is refused at validation, with the adopter's exact invented
  policy as the fixture.

## Out of Scope

Everything the report calls a design question: B1's status vocabulary, the evidence model, the
lifecycle concepts and the enforcement tier all have their own stories and none of them ride here.

## Open Questions

A3's direction — widen the project merge to workflows and protocols, or state the vendoring rule.
Decides: protocol owner. Default if nobody answers: **state the vendoring rule**, because widening the
merge changes what a tree means for every adopter and the guide being wrong is the defect actually
reported.
