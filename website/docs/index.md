---
slug: /
title: What this is
sidebar_label: What this is
sidebar_position: 1
description: Two documents every engineering organisation runs on are prose. This project makes both executable.
---

# What this is

Two of the most consequential documents in any engineering organisation are prose: the one that says
**how we work**, and the one that says **what we are building**. Both are read by people who then go
and do something else, and neither can be checked.

```text
"Follow TDD, don't break the API, get approval before touching production."
        → a wiki page nobody consults during the work

"The billing service issues invoices; a paid invoice cannot be cancelled."
        → a ticket, an out-of-date API doc, and an argument six months later
```

`engineering-protocols` makes both executable. Not summarised into a prompt — turned into typed
documents a program resolves, evaluates and refuses.

## Two halves

| | Governs | Answers the question |
|---|---|---|
| **AEP** — Agentic Engineering Protocol | how engineering work is performed | *Was this built properly?* |
| **ESS** — Executable System Specification | what software must exist | *Is this the thing we meant to build?* |

They are not layers of each other. AEP does not know what an invoice is; ESS does not know what a
code review is. They meet at exactly one place — evidence:

```text
ESS                    defines the target
 │
 ▼
ADP (an AEP profile)   governs the work toward it
 │
 ▼
Implementation
 │
 ▼
ESS conformance        checks the result against the target
 │
 ▼
Evidence               a fact, produced by something other than the agent
 │
 ▼
AEP completion         the protocol decides whether that is enough
```

The loop closes because the specification that *generated* the contracts is the same one that
*tests* the implementation. An agent cannot satisfy the test by weakening it, because it did not
write the test — and it cannot declare the work finished, because completion is a predicate over
facts it does not control.

## The thesis

> Describe how work is performed and what must exist **once**, in typed form, and let everything else
> — the checks, the contracts, the tests, the audit trail, the skeleton — be derived from that
> description rather than maintained beside it.

The model reasons. The protocol constrains. The specification defines. The verifiers establish facts.

None of this makes a model reliable. It makes a model's output *checkable*, which is a different and
more achievable thing.

## Where to go from here

| If you want | Read |
|---|---|
| why prose rules stop working once agents write the code | [Why agents change this](./why-agents-change-this.md) |
| what the two halves actually are, and where they join | [The two halves](./two-halves.md) |
| the design commitments, and the mechanism enforcing each | [What this insists on](./pillars.md) |
| the central claim made visible — one specification, and the contracts it produced | [A specification and its contracts](./in-practice/a-specification-and-its-contracts.md) |
| what is built, what is not, and what nobody has agreed to build | [Where this stands](./status/where-this-stands.md) |

The project is Apache-2.0 and lives at
[github.com/codewandler/engineering-protocols](https://github.com/codewandler/engineering-protocols).
Every claim on this site is traceable to a file in that repository, and each page says which.
