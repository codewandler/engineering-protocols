# Widget Foundry — deployment notes

> Parse-form cases. Every annotation below is about an invented system. What matters is the SHAPE of
> the line, not what it says.
> Last updated: 2026-08-30

## Baseline

The simplest possible annotation: one line, an em-dash, a horizon token at the end.

Verify: 2026-08-30 — sprocket-api is running image v4.2.1 in the atlas namespace. (horizon: 7d)

## Wrapping

Prose is hard-wrapped, so the horizon token routinely lands on a continuation line. An implementation
anchored to end-of-line reads this as having no horizon at all and silently falls back to its default —
which means the claim is watched on the wrong clock while looking perfectly annotated.

Verify: 2026-08-29 — the flange-service chart tag published to the registry is 2.11.0, and no higher
  tag exists on the release branch as of this reading. (horizon: 3d)

## Wrapping across three lines

Verify: 2026-08-28 — the nightly reconciliation job for the grommet index completed with exit 0, and
  the row count it reported matches the count in the primary store to within the tolerance the runbook
  states. (horizon: 14d)

## List-bullet prefix

- Verify: 2026-08-30 — cogwheel-worker has 4 replicas ready in atlas. (horizon: 2d)

## Case and spacing variance in the token

The first two are well-formed. An implementation that matches the token case-sensitively, or that
requires exactly one space around the number, rejects a valid annotation and reports it as undated
prose.

Verify: 2026-08-30 — the escapement queue depth is under 100. (Horizon: 7D)

Verify: 2026-08-30 — the ratchet-gateway health endpoint returns 200. (horizon:  5 d )

The third is a NEAR MISS and must be treated as malformed: a space between the opening parenthesis and
the keyword. This is a judgement call worth stating rather than leaving implicit — the token is
deliberately strict, because the whole convention rests on it being a token and not a phrase. Loosening
the left edge is the first step toward accepting `(horizon: 5d — but see below)`, which is the failure
this strictness exists to prevent.

Verify: 2026-08-30 — the pallet-fork latency is under 50ms. ( horizon: 5d)

## Stop-line boundaries

The body absorbs continuation lines, and must stop at the next annotation keyword. If it does not, the
`Due:` line below is swallowed into the first annotation's body and disappears from its own gate.

Verify: 2026-08-30 — the mainspring migration has not yet been applied to atlas.
Due: 2026-09-10 — apply the mainspring migration (owner: A. Fitter; to: platform).

The body must also stop at a heading.

Verify: 2026-08-30 — pinion-cache eviction rate is within its objective. (horizon: 3d)

### A heading that must not be absorbed

The body must stop at a list bullet, too.

Verify: 2026-08-30 — the balance-wheel scheduler is enabled in atlas. (horizon: 3d)

- an unrelated bullet that belongs to nothing

## Two adjacent annotations

The second must be parsed as its own record, not absorbed into the first's body. Absorbing it produces
one annotation with a stale date and the wrong horizon, and loses the other entirely.

Verify: 2026-08-30 — the detent-service deployment succeeded. (horizon: 7d)
Verify: 2026-08-24 — the detent-service previous revision is still retained. (horizon: 7d)

## Negative case — not an annotation

The convention requires an em-dash separator. This line uses a hyphen and must NOT be picked up; if an
implementation accepts it, the corpus count silently inflates and a real malformed line hides in the
noise.

Verify: 2026-08-30 - hyphen instead of em-dash, so this is not an annotation at all. (horizon: 7d)
