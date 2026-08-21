---
format: aep.planning-md/1
id: story:external-clock-obligations
kind: story
status: draft
title: An obligation never gates a transition — it raises
summary: 'A commitment with owner, due date and escalation, open|met|slipped, deliberately unable to block: a commit cannot close a statutory clock.'
owner: protocol
tags:
- adoption
- obligations
relations:
- decomposes: epic:adopter-feedback-round-1
- informed_by: story:time-based-transitions
revision: 2
---
# Story: An obligation never gates a transition — it raises

## Outcome

A commitment that fires on a date nobody controls and is satisfied by a person is modelled as such:
visible, owned, escalating, and structurally incapable of blocking a commit — because blocking a
commit cannot close one.

## Context

An early adopter's review, round 1 — **item E1**, with the largest count in the report behind it:
**425 open, 106 met, 83 slipped** in the adopter's store. Their checker for these is **deliberately
advisory**, and the reason is the design constraint, not a compromise: *"blocking a commit cannot
close one"* — 10 of their 11 overdue items run on a statutory clock that no engineering action moves.

The model asked for: an obligation with an **owner**, a **due date** and an **escalation**, in states
`open | met | slipped`. The line that must survive into the implementation is that **an obligation
never gates a transition — it raises**. That makes it categorically different from an approval, and
the report says plainly not to conflate the two: an approval is a gate a person opens; an obligation
is a debt a person owes, and a debt that could block the work would just be deleted.

This is the same tier question as `story:advisory-enforcement-tier` seen from the other side: F1 asks
for a check that reports and counts, E1 asks for a commitment that raises and never stops. Both are
the report's evidence that one enforcement level is not enough. `story:time-based-transitions` is the
clock this needs; the edge is `informed_by` because an obligation with a due date that nothing
evaluates is still worth recording.

## Acceptance

- An obligation declares owner, due date and escalation, and moves through `open | met | slipped`.
- **No obligation can gate a transition**, and that is asserted structurally — a construct that tried
  to make one a precondition is refused, rather than merely being absent from the examples.
- A past-due obligation raises: it is visible in the store's own reporting with its owner and its age,
  and the escalation names who hears about it.
- The report is countable — open, met and slipped totals come out of the store, because the number is
  the thing anybody actually looks at.

## Out of Scope

Notifying anyone. The escalation names a route; delivering on it is the adopter's system, exactly as
the outbound-claim story leaves transport alone.

## Open Questions

Whether `slipped` is terminal or an obligation can still reach `met` after slipping. Decides: protocol
owner. Default if nobody answers: **not terminal — a slipped obligation can be met late**, and the
record keeps both facts, because an obligation that becomes unsatisfiable the moment it is late would
teach everybody to stop recording the due date.
