---
format: aep.planning-md/1
id: story:completion-needs-evidence
kind: story
status: draft
title: A story cannot reach implemented on somebody's word
summary: The move to implemented is refused unless the engine has admitted the evidence the protocol requires, and the refusal names what is missing.
owner: protocol
tags:
- evidence
- store
relations:
- decomposes: epic:evidence-gated-completion
revision: 1
---
# Story: A story cannot reach `implemented` on somebody's word

## Outcome

A reviewer looking at a closed story knows that something the engine admitted stands behind it — and
somebody trying to close one without that is refused, and told exactly what is missing.

## Context

The store checks that a move is **legal** and says nothing about whether it is **earned**. Everywhere
else this repository refuses that gap: the engine never manufactures evidence, an agent's own
statement never satisfies an independence requirement, and the verifier class that can mint a
transcript verdict is a type rather than a convention. Then a person writes `status: implemented` into
a file and none of it applied. `adp.story.complete/v1` already exists and means *record that a story
is done, and what did it*; it has no consumers.

## Acceptance

- `protocol artifact move <story> --to implemented` under a task whose protocol requires evidence is
  refused when none has been admitted, and the refusal names the kinds that would satisfy it — the
  same shape the illegal-transition refusal already has.
- The same move succeeds once a `TestResult` or `trace_conformance` record for that story has been
  admitted by the engine.
- A hand-edited `status: implemented` with no admitted record is reported by `validate`, naming the
  artifact — the store cannot prevent the edit and is honest about catching it afterwards.
- A plan being sketched outside any task is **not** gated: the requirement comes from the protocol in
  force, not from the store.

## Out of Scope

Approval evidence. A person saying *I approve* is already modelled as `Evidence::Approval` with a
`Producer::Human`; this story is the mechanical half.

## Open Questions

Whether the gate is a refusal or a warning in its first release. Decides: protocol owner. Default if
nobody answers: a refusal, because a warning that can be ignored is the state the store is in today.
