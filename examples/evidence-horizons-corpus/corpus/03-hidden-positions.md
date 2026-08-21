# Widget Foundry — annotations in positions a line-anchored parser cannot see

> Three positions, each of which hid real annotations from a real gate for days or weeks. All three
> look completely normal to a human reader, which is exactly why nobody noticed.
> Every annotation in this file is well-formed. The only question is whether the parser finds it.
> Verify: 2026-08-30 — this file's own header-block annotation is well-formed and must be found.
>   (horizon: 7d)
> Last updated: 2026-08-30

## Position 1 — inside a `>` header block

The house convention puts a file's purpose, cross-references and `Last updated:` in a leading quote
block, so an annotation naturally goes there too. A parser anchored on `^[ \t]*Verify:` skips every one
of them — the annotation is present, correct, and unwatched.

The header of this file carries one. It must appear in the parse.

## Position 2 — a quote block mid-document

> Context for the escapement rework, quoted from the design note.
> Verify: 2026-08-29 — the escapement rework is not yet enabled in atlas. (horizon: 3d)
> A bare `>` line below must act as a stop.
>
> This sentence must NOT be absorbed into the annotation's body.

## Position 3 — wrapped inside a quote block

> Verify: 2026-08-28 — the flange-service canary is serving 5% of traffic and its error rate is
>   under the stated objective, measured over a two-hour window rather than a single scrape.
>   (horizon: 5d)

## Position 4 — after a `<br>` inside a markdown table cell

Inside a table cell, `<br>` is the only line break available. An annotation written there is invisible
to a `^`-anchored pattern and perfectly legible to every human reading the table. This position is
worth its own test because the annotations that end up here are disproportionately the *short-horizon*
ones — a ticket row, a pin, a state that someone already knew was volatile.

| Component | State | Checked |
|---|---|---|
| sprocket-api | pinned 4.2.1 | Verify: 2026-08-30 — pin read from the environment's values file. (horizon: 7d) |
| flange-service | pinned 2.11.0 | notes<br>Verify: 2026-08-30 — pin confirmed against the running image. (horizon: 3d) |
| grommet-index | rebuilt | rebuilt overnight<br>Verify: 2026-08-29 — row counts match the primary store. (horizon: 2d) |

## Position 5 — `<br>` in a table cell, and the claim rotted faster than the default

The reason this position matters is not symmetry. A short horizon in a table cell is usually a
deliberate act: somebody shortened it *because* the claim had already rotted once. Losing this row
loses the one annotation whose author had the most information about how fast its subject moves.

| Component | State | Checked |
|---|---|---|
| ratchet-gateway | 1.4.0 | shortened after a same-day rot<br>Verify: 2026-08-30 — pin confirmed against the running image; horizon cut to 1d because this claim was falsified within hours on a previous reading. (horizon: 1d) |
