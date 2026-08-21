# Widget Foundry — three positions a production parser was still blind to

> ⚠️ All four annotations in this file are well-formed and SHOULD be found. The reference
> implementation this fixture's expectations were generated from finds exactly ONE — the first row of
> Gap 2, whose neighbour it then swallows.
>
> These are not invented edge cases. All three were discovered by running that implementation over
> this fixture, and then confirmed against its real corpus: **15 of 160 live-state annotations —
> 9.4% — were invisible to the gate whose entire job is to make an unchecked claim visible.**
> That gate had already been fixed three times for three other positions in the same class.
> Last updated: 2026-08-30

## Gap 1 — a `Verify:` at the end of a long table cell, with no `<br>` before it

The known fix for table cells keys on `<br>` as a synthetic line start. But a cell whose prose simply
ends with an annotation has no `<br>` at all — the annotation is the last sentence of a paragraph that
happens to live inside a cell. To a reader it is identical to any other annotation. To a `^`-anchored
parser it does not exist.

This is the highest-volume gap in the real corpus, and it self-selects for the worst case: long cells
are long because the subject is contested, and a contested subject is exactly the one whose state
someone bothered to re-check.

| Ticket | State | Detail |
|---|---|---|
| WF-401 | open | The pallet-fork regression is reproduced on the atlas environment and a fix is in review; nothing here yet records which release carries it, so do not report it as shipped. Verify: 2026-08-30 — read from the environment's running image and the open review. (horizon: 5d) |

## Gap 2 — two consecutive table rows, each carrying a `<br>Verify:`

The first row's annotation absorbs continuation lines until a stop line. A table row is not a stop
line, so the second row is swallowed into the first's body — and vanishes.

The failure is asymmetric in the worst direction. The swallowed record keeps neither its date nor its
horizon, so a SHORTER-horizon, OLDER claim disappears behind a fresher neighbour, and the corpus count
silently drops by one with nothing to indicate it. Inserting a blank line between the rows makes both
parse, which is how the mechanism was confirmed.

| Component | State | Checked |
|---|---|---|
| mainspring | pinned 3.0.1 | notes<br>Verify: 2026-08-30 — pin read from the running image. (horizon: 7d) |
| balance-wheel | pinned 1.9.4 | notes<br>Verify: 2026-08-29 — pin read from the running image; horizon shortened because this component redeploys daily. (horizon: 2d) |

## Gap 3 — an annotation wrapped in inline-code backticks

Writing the annotation inside backticks is a natural thing to do when a document is showing the reader
what the convention looks like — and just as natural when someone is recording a real claim and wants
it to stand out. The two are indistinguishable to the parser, which sees a line starting with a
backtick and moves on.

`Verify: 2026-08-30 — the escapement rework is enabled in atlas; read from the deployment. (horizon: 3d)`

## What a conforming implementation must do

Find all four annotations above. More usefully, note the shape shared by all three gaps and by the
three positions already fixed before them: **every one is a case where the annotation is present,
correct, and legible to a human, and the parser's idea of "a line" disagrees with the document's.**

An adopter is going to keep meeting this class. The lesson the corpus supports is not "handle these
four positions" — it is that a scanner over human-written documents needs a coverage claim of its own.
Six positions have now been found this way, three of them after the gate was already believed
complete, and the only reason the last three surfaced is that somebody built a fixture and compared
raw occurrences against parsed records. That comparison should be part of the gate, not part of an
investigation.
