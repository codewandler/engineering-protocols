# Evidence-horizon conformance fixture (C1)

A synthesized regression corpus for **evidence that decays from `true` back to `?`** — the annotation
form, the parse edge cases, the classification boundaries, and the two things a horizon cannot do.

**It carries no real fact about any organisation.** Every subject is an invented mechanism in an
invented system ("Widget Foundry"). The *shape* is drawn from a production corpus of 160 annotations
maintained over several months; the content is not. Free to commit anywhere, including a public repo.

## Layout

| Path | What |
|---|---|
| `corpus/01-forms.md` | 12 parse-form cases: wrapping, list bullets, token case/spacing, stop-line boundaries, adjacent annotations, one negative (hyphen not em-dash), one near-miss token |
| `corpus/02-malformed.md` | 7 cases of the class that matters most — annotations that *look* annotated and are not watched |
| `corpus/03-hidden-positions.md` | 7 annotations in positions a line-anchored parser cannot see: quote blocks, wrapped quote blocks, table cells |
| `corpus/04-classification.md` | 8 arithmetic cases at a fixed reference date, including the `age == horizon` boundary |
| `corpus/05-traps.md` | 4 rows encoding the two limits: false-within-horizon, and re-check-versus-extend |
| `corpus/06-reference-gaps.md` | 4 well-formed annotations the reference implementation misses — see below |
| `expected.json` | Generated, not hand-written. Records + per-file coverage + known reference gaps |
| `generate_expected.py` | Regenerates `expected.json` from a reference implementation |

Reference date is **2026-09-01**, warning window 2 days, default horizon when malformed 14 days.

```console
$ python3 generate_expected.py --impl /path/to/check-verify.py
expected.json: 37 record(s) — 14 ok, 16 expiring, 7 expired, 8 malformed
```

## ⚠️ `expected.json` is a baseline, not ground truth

The expectations were produced by running a real implementation over the corpus, deliberately, so they
encode rules that survived months of contact with human-written documents rather than my reading of a
spec. But that implementation is **not correct**, and the fixture says so in `known_reference_gaps`:

- **42 raw annotations in the corpus, 37 parsed.** The five-record gap is the conformance target.
- A correct implementation finds all 42 and still rejects the two deliberate negatives in
  `01-forms.md` (a hyphen separator, and a space between `(` and `horizon:`).

## What the corpus is actually for

Three levels, in rising order of what they will teach an implementer.

**1. The arithmetic.** Easy, and the boundary is the only interesting part: `age == horizon` is **not**
expired. An off-by-one there fires the gate a day early on every record in a corpus, which is how a
gate gets muted and then deleted.

**2. The malformed class.** Harder, and worth more test weight than expiry. An annotation with no
horizon token must get a stated default **and** be marked malformed — otherwise the convention decays
into undated prose one line at a time, invisibly. The two variants in `02-malformed.md` that survive a
first fix are the token replaced by prose, and prose placed *inside* the token. The second one appears
when someone has been told to keep the token and does exactly that.

**3. The positions.** This is where a real implementation will actually fail, and the fixture's most
useful content is the evidence that it keeps failing. Six positions have now been found where an
annotation is present, correct, legible to a human, and invisible to the parser — because the parser's
idea of "a line" disagrees with the document's:

| # | Position | How it was found |
|---|---|---|
| 1 | Horizon token on a wrapped continuation line | first seeded batch |
| 2 | Annotation inside a `>` header/quote block | 2 real annotations sat unwatched |
| 3 | Annotation after `<br>` inside a table cell | 5 real annotations sat unwatched |
| 4 | Annotation ending a table cell with **no** `<br>` | this fixture |
| 5 | Second of two consecutive `<br>` table rows — absorbed into the first's body, date **and** horizon lost | this fixture |
| 6 | Annotation wrapped in inline-code backticks | this fixture |

Positions 1–3 were each found in production, fixed, and believed complete. Positions 4–6 were found in
an afternoon by writing this fixture and comparing raw occurrences against parsed records — on a corpus
where the same comparison then showed **15 of 160 annotations (9.4%) unwatched by the gate whose whole
job is to make an unchecked claim visible.**

The generalisable lesson is not "handle six positions". It is that **a scanner over human-written
documents needs a coverage claim of its own**, and that the comparison which produces it — raw
occurrences versus parsed records — is cheap enough to be part of the gate rather than part of an
investigation. `expected.json`'s `coverage` block is that comparison, per file.

## The two traps (`corpus/05-traps.md`)

Both rows classify **ok**. That is the point: neither is something a parser can catch.

**False while inside its horizon.** A horizon is a guess about volatility, not a guarantee — a
seven-day claim can be false on day five, and the gate will say it is fine. Nothing automatic follows.
What must follow is that *shortening* a horizon is cheap, normal, and attributable to the reading that
justified it; if the only way to say "this subject moves faster than I thought" is prose, nobody says
it and the corpus keeps a horizon already proven wrong.

**Re-check versus extend.** There is one correct refresh: observe again, write a new observation date.
Growing the horizon while leaving the date alone yields a record that reports fresh and has not been
looked at since. Both rows in the fixture are the same subject refreshed the two ways, and no parser
can tell them apart from a single reading.

Design consequence, and it is an API-shape decision rather than a validation rule: **make the
observation date the identity of the fact, and offer no operation that mutates a horizon in place.** If
`extend` is as easy to call as `re-check`, it is the one that gets called — every time, under pressure,
by whoever is trying to get a gate green. A useful diagnostic to build on top: flag any record whose
horizon grew while its observation date did not. That needs history rather than one reading, which is
why it belongs in the store rather than the parser.

## Horizon distribution

`distribution.json` carries the measured shape of the production corpus (167 tokens), for anyone
generating at volume. Short horizons dominate deployment state; long ones cover slow external facts. An
implementation that assumes one sensible default is wrong at both ends.
