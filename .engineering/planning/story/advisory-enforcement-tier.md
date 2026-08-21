---
format: aep.planning-md/1
id: story:advisory-enforcement-tier
kind: story
status: draft
title: A gate that reports and counts, and its written route back to blocking
summary: 'A second enforcement tier: advisory findings with an owner and an exit criterion, so a muted gate is a typed state rather than a deleted one.'
owner: protocol
tags:
- adoption
- enforcement
relations:
- decomposes: epic:adopter-feedback-round-1
revision: 1
---
# Story: A gate that reports and counts, and its written route back to blocking

## Outcome

A check that is not ready to block is a **typed state with an owner and an exit criterion**, not a
deleted check and not a muted one. Anybody can list what is currently advisory, who owns each one, and
what has to be true for it to bite.

## Context

An early adopter's review, round 1 — **item F1** — third in the adopter's
ranked order, with the sharpest claim attached: adoption fails here otherwise. Their repository runs
**15 blocking and 4 advisory** checkers, and the advisory ones carry standing findings of
**48 / 26 / 9 / 2** — numbers for which, in their words, *"blocking would be disabled within a day"*.
The rule they attach to each advisory gate is the part worth stealing: it carries its **written route
back to blocking**, because *"an advisory gate with no exit criterion is just a muted gate"*.

The pattern was independently invented a third time in their CI (`allow_failure: true` plus a comment
saying *remove once the first push succeeds*), and their sample says where the muting lands: **6 of 19**
configs mute a check, skewed toward conformance and e2e — precisely the checks AEP wants to gate.

**This repository has the same tier already, in one place only:** the trace spec's gate/advisory
split, where `--advisory` moves the checker's exit code and `trace_conformance.passed` deliberately
ignores it (gap register, *Closed by code — transcript conformance, phase 2*). That is the precedent
and the shape to generalise: the downgrade is a property of the invocation, the record names every
downgraded id, and the fact stays strictly stronger than exit 0. The protocol layer has no such tier
at all.

Two constraints the report attaches and this story keeps: **bypass must be cheap and loud**, and **a
gate outside the artifact tree is not deployed** — `.git/hooks/` is unversioned, so a fresh clone has
no gate.

## Acceptance

- An enforcement declaration admits at least two tiers, and a check in the advisory tier reports and
  counts without changing the run's verdict.
- An advisory declaration without an owner and an exit criterion is **refused** at validation — the
  route back to blocking is a required field, not a comment.
- The advisory findings are countable from the record: a run reports how many findings each advisory
  check produced, so a standing 48 is visible as a standing 48.
- A bypass leaves a record naming who bypassed what, and a gate that lives outside the versioned tree
  is reported as not deployed.

## Out of Scope

Automatically promoting an advisory gate to blocking when its findings reach zero. The exit criterion
is written by a person and read by a person; a gate that promoted itself would surprise a green build,
which is the one thing an advisory tier exists to avoid.

## Open Questions

Whether the tier is a property of the check or of the invocation. Decides: protocol owner. Default if
nobody answers: **of the check, declared in the tree**, with the invocation able to downgrade further
and never to upgrade — the same polarity the trace checker already has, where a caller's own flag can
never satisfy a requirement.
