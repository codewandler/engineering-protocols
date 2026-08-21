---
format: aep.planning-md/1
id: story:operator-resume-ux
kind: story
status: draft
title: A refused run tells the operator which of two things to type
summary: The lock refusal names the holder and the two routes out of it; --take-lock supersedes rather than erases; --resume re-acquires before it writes.
owner: driver
tags:
- driver
- operator
relations:
- decomposes: epic:reference-driver
- depends_on: story:protocol-drive-verb
revision: 1
---
# Story: A refused run tells the operator which of two things to type

## Outcome

An operator whose run is refused by a lock does not go and read a design document. The refusal names
who holds it and the exactly two commands that resolve it, and stealing a lock is something a person
did on purpose, recorded in the run that took it.

## Context

A paused run holds no lock, because an `operator` step waiting for a person has no bound and any age
threshold would break exactly the runs that paused correctly. That makes re-acquisition on resume
load-bearing: a resume that writes without re-taking the lock is how two live runs happen. The
refusal follows the shape `artifact move` already uses for an illegal transition — refuse, and name
where you can actually go.

## Acceptance

- A lock whose pid is alive is refused; the message carries run id, pid, host and the cursor's state,
  and names both `--resume` and `--take-lock`.
- A lock whose pid is **not** alive on the same host is reported stale and **still refused** without
  `--take-lock`.
- A lock naming a different host is never stale, whatever the local pid table says.
- `--take-lock` writes the stolen lock's contents into the new run's cursor, so *this run took the
  lock from pid 4711 of run `<task>/2`* is in the record.
- `--resume` against a store whose lock another run now holds refuses.
- The lock is absent after an approval pause while `current` still points at the run.

## Out of Scope

Waiting. There is no queue and no blocking acquire — a driver that waits on a lock is a driver
holding a session open for an unbounded time.

## Open Questions

None. The age-threshold question was asked and answered: there is deliberately no threshold.
