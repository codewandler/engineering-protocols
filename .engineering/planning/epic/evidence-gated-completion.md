---
format: aep.planning-md/1
id: epic:evidence-gated-completion
kind: epic
status: draft
title: Done is a claim with evidence behind it
summary: A story reaches implemented because the engine admitted evidence for it, not because somebody typed a status.
owner: protocol
tags:
- evidence
- store
relations:
- decomposes: initiative:the-repo-governs-itself
revision: 1
---
# Epic: Done is a claim with evidence behind it

## Outcome

Nobody can close a story here by typing a word. `implemented` is reachable only when the engine has
already admitted the evidence the protocol requires for it, and a reviewer three months later can ask
the store *what made this done* and be answered from the store.

## Why Now

The store validates that a status move is **legal**; it says nothing about whether it is **earned**.
That is the same asymmetry the repository refuses everywhere else: the engine never manufactures
evidence, an agent's own statement never satisfies an independence requirement, and a verifier class
is a type rather than a convention — and then a person edits `status: implemented` and none of it
applied. `adp-domain` already declares `adp.story.complete/v1` as *"record that a story is done, and
what did it"*, with zero consumers.

## Scope

The gate on the move, and the join that survives it. The gate refuses the move and names what is
missing, in the same voice `artifact move` already uses for an illegal transition. The join is what
P3's journal makes possible: the admitted record beside the artifact, not in a commit message.

## Out of Scope

Approval. A person saying *I approve* is `Evidence::Approval` and a `Producer::Human`, and it is
already modelled; this epic is about the mechanical half. Also out: retrofitting the gate onto
artifacts that were moved before it existed — a rule applied backwards to records made under a
different rule is a rule that reports noise.

## Risks

A gate that fires on every move makes the store unusable for planning, which is what it is for
today. The mitigation is that the requirement comes from the protocol in force for the task, not from
the store: a plan being sketched under no task is not gated, and a story being driven under
`adp/default` is.

## Done When

A story in this store reached `implemented` because a `trace_conformance` or `TestResult` record was
admitted for it, a hand-edited `status: implemented` is caught by `validate` and named, and
`adp.story.complete/v1` has its first consumer.
