---
slug: /
title: Introduction
sidebar_label: Introduction
sidebar_position: 1
description: Typed, executable rules for agent-performed engineering work, and executable specifications for the software it produces.
---

# Engineering Protocols

`engineering-protocols` is a Rust library, a CLI and a set of typed document formats for running
engineering work — especially agent-performed engineering work — under rules a program can execute,
instead of prose a model may or may not follow.

It has two halves:

| Half | Governs | The question it answers |
|---|---|---|
| **AEP** — Agentic Engineering Protocol | how engineering work is performed | *Was this built properly?* |
| **ESS** — Executable System Specification | what software must exist | *Is this the thing we meant to build?* |

With **AEP**, you write your team's rules — "write the test first", "production changes need
approval", "an agent cannot verify its own work" — as typed YAML documents. A deterministic engine
resolves a task against them and answers: which rules are in force, what the agent may do, what
evidence is owed, and whether the work is done. The agent reasons; the protocol decides what the
recorded facts permit.

With **ESS**, you write a system's design — commands, entities, events, state machines, components —
as a typed specification. A compiler derives the documentation, the JSON Schema, OpenAPI and
AsyncAPI contracts, a conformance test suite, and the structural part of the implementation from
that one document. When the specification changes, a semantic diff derives what the change
invalidates.

## Why prose rules fail with agents

A person who ignores the wiki page can be asked why. An agent given the same page in a prompt
produces output that *reads* as though it followed it, at whatever scale you run it. Prose
instructions fail silently and plausibly: "the tests pass" and "I ran the tests" are assertions, and
reading them tells you nothing a competent text generator could not produce without running
anything.

Reviewing everything does not scale either — the cost of producing a change fell, the cost of
reading one did not. So the operative question becomes: **which facts, produced by whom, permit
calling the work done without reading all of it?** That is the question this project turns into
types:

| Prose rule | What it becomes here |
|---|---|
| "An agent cannot verify itself" | an evidence requirement marked `independent: true`, which an agent's own report never satisfies — the producer is part of every evidence record |
| "Get approval before touching production" | a capability held in an approval floor; a profile that grants `production.write` outright fails to resolve |
| "Write the test first" | an ordering fact: `evidence.first_seq.test_result < evidence.first_seq.diff`, checked against recorded submission order |
| "Ada approved the design" | an approval bound to the revision it approved; version 7 is not covered by a review of version 3 |
| "Build what we specified" | a conformance suite generated from the same specification the contracts came from, run by something other than the author |

None of this makes a model reliable. It makes a model's output **checkable**, which is a different
and more achievable property.

## How the two halves connect

AEP does not know what an invoice is; ESS does not know what a code review is. They meet at exactly
one place — evidence:

```text
ESS specification      defines the target system
        │
        ▼
AEP profile            governs the work toward it
        │
        ▼
Implementation         written by an agent or a person
        │
        ▼
ESS conformance run    checks the result against the specification
        │
        ▼
Evidence record        a fact, produced by something other than the author
        │
        ▼
AEP completion         the protocol decides whether that is enough
```

The loop closes because the specification that *generated* the contracts is the same one that
*tests* the implementation. The agent cannot weaken the test, because it did not write the test —
and it cannot declare the work finished, because completion is a predicate over facts it does not
control.

## Where to go

| You want to | Read |
|---|---|
| run it in ten minutes | [Getting started](./getting-started.md) |
| understand the model before adopting it | [Architecture overview](./concepts/overview.md), then [AEP](./concepts/aep.md) and [ESS](./concepts/ess.md) |
| put your team's rules under the protocol | [Govern a task](./guides/govern-a-task.md) and [Write a principle](./guides/write-a-principle.md) |
| make the protocol govern your agent | [Integrate an agent harness](./guides/integrate-a-harness.md) |
| derive contracts and tests from a specification | [Write a specification](./guides/write-a-specification.md) |
| see real input and output before reading anything else | [A specification and its contracts](./examples/specification-to-contracts.md) |
| know what is built, what is not, and what you have to trust | [Status](./status/where-this-stands.md) and [Limitations](./status/limitations.md) |

The project is Apache-2.0 and lives at
[github.com/codewandler/engineering-protocols](https://github.com/codewandler/engineering-protocols).
Every claim on this site is traceable to the repository: concept, example and status pages name
their source files, and every command a guide shows is runnable from a checkout.
