# Widget Foundry — the malformed class

> The failure mode that matters most. A malformed annotation still LOOKS annotated to every human
> reader, so nobody re-checks it and nothing says so. This is the class an adopter's parser will get
> wrong, and it is worth more test weight than the expired class.
> Last updated: 2026-08-30

## No horizon token anywhere

The convention decays into undated prose one annotation at a time. An implementation must not treat
"no token" as "no expiry" — it must apply a stated default AND mark the record malformed, so the decay
is visible rather than silent.

Verify: 2026-08-25 — the escapement-service rollout reached all three zones.

Verify: 2026-08-10 — the mainspring index rebuild finished.

## The token replaced by prose

This is the shape that appears when someone supersedes an annotation and writes the reason where the
token was. It reads as a careful, updated annotation and is invisible to the gate.

Verify: 2026-08-20 — the flange-service pin is 2.11.0. (superseded by the 2026-08-30 re-read above)

## Prose INSIDE the token

The subtler variant, and the one that survives a first fix. Someone is told "keep the horizon token",
so they keep it — and add the reason inside the parentheses. It complies with the letter and still does
not parse.

Verify: 2026-08-20 — the grommet queue is drained. (horizon: 5d — superseded by the re-read above)

Verify: 2026-08-20 — the ratchet-gateway pin is 1.4.0. (horizon: 3d — this namespace moves faster
  than 5d)

## The correct way to write the same thing

The reason goes BEFORE the token. The token is a token, not a sentence.

Verify: 2026-08-30 — the ratchet-gateway pin is 1.4.0. Horizon shortened from 5d because this
  namespace demonstrably moves faster. (horizon: 3d)

## Malformed and long past its default

A record with no token, old enough that any sane default has elapsed. It must surface as BOTH expired
and malformed — an implementation that reports only one of the two loses half the signal.

Verify: 2026-07-15 — the cogwheel-worker autoscaler is enabled in atlas.
