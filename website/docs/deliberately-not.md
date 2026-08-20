---
title: What this is not
sidebar_position: 5
description: Not an orchestration framework, not a CI system, not a deployment platform. The boundary is stated so it can be held.
---

# What this is not

Scope stated in the negative, because a project that cannot say what it refuses to become will
eventually become all of it.

**Not an LLM orchestration framework.** Nothing here calls a model, holds a prompt or schedules an
agent. The harness does that; the protocol answers questions the harness asks.

**Not a CI system, an incident-management product, a workflow engine or a message broker.** External
systems do the work. This project decides what the results permit.

**Not a policy language meant to replace OPA.** The subject is engineering work and the software it
produces, not general authorisation.

**Not a universal ontology of software engineering**, and not a mandate for microservices, CQRS or
event sourcing. A component is a unit of ownership; whether it ships as a process or as a module
inside one binary is a separate file and a separate decision.

**Not a deployment platform.** This is the line worth drawing precisely, because a proposed design
for infrastructure would sit closest to it. Generating an artifact is in scope: compiling a
specification into a file that describes an infrastructure, and deciding whether an observed state
conforms to what was specified. *Operating* a system is not. Nothing here calls a cloud API, holds a
credential, applies a plan or watches a rollout. Actually deploying something is optional, later, and
somebody else's process.

## The responsibility, stated twice — once per half

> Define the semantics by which engineering work can be constrained, evidenced, verified and
> progressed — and the semantics by which a software system can be specified once and compiled into
> its contracts, its tests and as much of itself as the specification safely determines.

## And two things the project is not claiming

**It does not make a model reliable.** It makes a model's output checkable, which is a different and
more achievable thing.

**It has not yet governed a team.** The protocol runs, the documents validate, the conformance suites
bite. The next honest milestone for AEP is not a feature; it is a team whose work it actually
governs — and that has not happened. A project can now be discovered, and nothing has been governed
by it yet.

---

**Sources.** `docs/VISION.md` § *What this is deliberately not* and § *Where this stands*;
`AGENTS.md` § *What this repository is*.
