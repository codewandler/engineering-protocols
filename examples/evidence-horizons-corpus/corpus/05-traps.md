# Widget Foundry — the two traps

> These are not parser tests. They encode the two things a horizon-based implementation cannot do, so
> that an adopter meets them in a fixture rather than in production. Both rows are expected to pass the
> gate. That is the point.
> Last updated: 2026-08-30

## Trap 1 — false while still inside its horizon

A horizon is a guess about how fast a subject moves. It is not a guarantee, and the tail is where it
fails: a claim written with a seven-day horizon can be false on day five, and the gate will report it
as fine, because the gate can only see the clock.

Below, the same subject is annotated twice. The first claim was made on 2026-08-25 with a seven-day
horizon. The second, five days later, contradicts it. At the reference date the FIRST record still
classifies **ok** — it has two days left.

Verify: 2026-08-25 — the flange-service pin in atlas is 2.11.0. (horizon: 7d)

Verify: 2026-08-30 — the flange-service pin in atlas is 2.12.0; the 2026-08-25 reading above was
  correct when written and was overtaken on 2026-08-28. (horizon: 3d)

**What an implementation is expected to do:** nothing automatic. Both records parse; the first is ok;
the second is ok. A conforming implementation must NOT invent a contradiction check it cannot ground.

**What it must make possible:** shortening a horizon must be a normal, cheap, first-class response to a
subject that proves volatile — and the shortening must be attributable to the reading that justified
it. If the only way to express "this subject moves faster than I thought" is prose, nobody will express
it, and the corpus will keep the horizon that was already proven wrong.

## Trap 2 — re-check versus extend

There is exactly one correct way to refresh a claim: observe it again and write a new observation date.
Growing the horizon while leaving the date alone produces a record that reports as fresh and has not
been looked at since the original reading.

The two rows below are the same subject, refreshed the two ways.

**Correct — re-checked, new date, horizon unchanged:**

Verify: 2026-08-30 — the grommet index row count matches the primary store. (horizon: 7d)

**Forbidden — same observation date, horizon grown to cover the gap:**

Verify: 2026-08-04 — the grommet index row count matches the primary store. (horizon: 60d)

Both classify **ok** at the reference date, and no parser can tell them apart from one record alone.
That is the finding.

**What an implementation is expected to do:** make the observation date the identity of the fact, and
offer no operation that mutates a horizon in place. If "extend" is as easy to call as "re-check", it is
the one that gets called — every time, under pressure, by whoever is trying to get a gate green.

A useful diagnostic an adopter can implement on top: flag any record whose horizon grew while its
observation date did not. It needs history rather than a single reading, which is why it belongs in the
store rather than the parser — but the fixture carries the pair so the behaviour can be tested.
