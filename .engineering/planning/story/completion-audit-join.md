---
format: aep.planning-md/1
id: story:completion-audit-join
kind: story
status: draft
title: What made this done, answerable from the store
summary: The admitted record is joined to the artifact through the journal, so the question a reviewer asks three months later is answered by the store rather than by git archaeology.
owner: protocol
tags:
- evidence
- store
relations:
- decomposes: epic:evidence-gated-completion
- depends_on: story:journal-backed-store
- depends_on: story:completion-needs-evidence
revision: 1
---
# Story: What made this done, answerable from the store

## Outcome

Somebody auditing a closed story three months later types one command and gets the record that closed
it — the suite, the transcript digest, the verifier class — instead of reconstructing it from commit
archaeology.

## Context

The gate makes closing a story require evidence; this makes the evidence findable afterwards. The
store has no audit join today (**D-P3**), so the only trace of *why* is the commit that changed the
line, and a commit message is not a record the engine admitted. P3's journal is where the join
belongs, which is why this depends on it rather than inventing a second place to keep history.

## Acceptance

- The admitted record is joined to the artifact through the journal and is retrievable by artifact id.
- The join names the revision the artifact was at when the record was admitted, so a later edit cannot
  make an old record look like it was about the new text.
- `protocol explain` can answer *what made this done* for a story in this store.
- Removing the record's source file does not silently unlink the join — the join is a stored fact, not
  a path.

## Out of Scope

Reconstructing joins for stories closed before the journal existed. A rule applied backwards to
records made under a different rule reports noise.

## Open Questions

Whether the join is one-to-one or many. Decides: protocol owner. Default if nobody answers: many —
a story satisfied by a suite and a transcript check has two records, and forcing a choice between them
would lose one.
