# Widget Foundry — classification boundaries

> Every annotation here is well-formed. What is being tested is the arithmetic: at a fixed reference
> date of **2026-09-01**, which records are ok, which are expiring, and which are expired.
> The boundary case is the one to get right — `age == horizon` is NOT expired.
> Last updated: 2026-08-30

## Comfortably fresh

age 2, horizon 7, 5 days left.

Verify: 2026-08-30 — sprocket-api is running 4.2.1 in atlas. (horizon: 7d)

## Exactly at the boundary

age 7, horizon 7, 0 days left. This must classify **ok**, not expired. An off-by-one here makes the
gate fire a day early on every annotation in a corpus, which is how a gate gets muted.

Verify: 2026-08-25 — the flange-service canary is healthy. (horizon: 7d)

## One day over

age 8, horizon 7, 1 day over. The first record that must fail.

Verify: 2026-08-24 — the grommet index rebuild completed. (horizon: 7d)

## Long expired

age 62, horizon 3, 59 days over.

Verify: 2026-07-01 — the escapement queue is drained. (horizon: 3d)

## Inside a warning window

age 5, horizon 7, 2 days left. With a warning window of 2 this is "expiring"; with the default of 0 it
is plain ok. Both behaviours must be reachable — a pure pass/fail run is the gate, and the warning
window is for a human reading a list.

Verify: 2026-08-27 — cogwheel-worker has 4 replicas ready. (horizon: 7d)

## The long tail

Real corpora are not uniform. Short horizons dominate for deployment state and long ones for slow
external facts, and an implementation that assumes a single sensible default will be wrong at both
ends. See `distribution.json` for the measured shape this is drawn from.

Verify: 2026-08-31 — the mainspring migration lock is released. (horizon: 1d)

Verify: 2026-08-20 — the vendor's published support window for the ratchet firmware still ends
  2027-03-31. (horizon: 90d)

Verify: 2026-06-20 — the balance-wheel licence count purchased is 250 seats. (horizon: 30d)
