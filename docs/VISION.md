# Vision

Two of the most consequential documents in any engineering organisation are prose: the one that says
**how we work**, and the one that says **what we are building**. Both are read by people who then do
something else, and neither can be checked.

```text
"Follow TDD, don't break the API, get approval before touching production."
        → a wiki page nobody consults during the work

"The billing service issues invoices; a paid invoice cannot be cancelled."
        → a ticket, an out-of-date API doc, and an argument six months later
```

This project makes both executable.

## Two halves of one problem

| | Governs | Answers |
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

## Why this matters more with agents than without

A person who ignores the wiki page can be asked why. An agent given the same page in a prompt will
produce something that reads as though it followed it, at whatever scale you run it. Prose
instructions do not fail loudly; they fail silently and plausibly.

What changes when the rules are typed and executable:

* **"An agent cannot verify itself" stops being a principle and becomes a type.** An evidence
  requirement marked `independent: true` is not satisfied by the agent's own report of a green suite.
* **"Get approval before touching production" stops being a reminder.** The protocol refuses to
  resolve a profile that grants production access outright — so the mistake cannot be made, rather
  than being noticed in review.
* **"Write the test first" stops being a convention.** Submission order is a fact:
  `evidence.first_seq.test_result < evidence.first_seq.diff`.
* **"Ada approved the design" stops being ambiguous.** The approval names the revision it approved,
  so version 7 is not covered by a review of version 3.
* **"Build what we specified" stops being a matter of interpretation.** The contracts, the tests and
  the skeleton all come from one model, and conformance is a suite the implementation runs against
  itself.

None of this makes the model reliable. It makes the model's output *checkable*, which is a different
and more achievable thing.

## What each half is for

**AEP** turns methodology into documents a program executes: principles with timed obligations,
workflows whose transitions are guarded by evidence, capabilities that default to denied, artifacts
with lifecycles and revision-bound approvals, and an audit trail that records refusals as carefully as
it records changes. A harness asks the protocol what is owed, what is permitted and whether the work
is done; the protocol answers deterministically, and can always say why.

**ESS** turns a system design into a model a compiler consumes: domains, entities, commands, events,
views, state machines, components, bindings, topology. From one model it derives documentation,
OpenAPI, AsyncAPI, JSON Schema, contract and integration tests, service skeletons, and — where the
specification is complete enough to determine behaviour — the behaviour itself. Transports are
projections, not the model: the same specification compiles to a modular monolith or to distributed
services without the domain changing.

## The thesis

> Describe how work is performed and what must exist **once**, in typed form, and let everything else
> — the checks, the contracts, the tests, the audit trail, the skeleton — be derived from that
> description rather than maintained beside it.

The model reasons. The protocol constrains. The specification defines. The verifiers establish facts.
Nothing in the loop asks anyone to be trusted.

## Where this stands

Honest as of `0.2.1`. See [`README.md`](../README.md) for the measured status table.

| | State |
|---|---|
| AEP — domain model, engine, documents, CLI | implemented, 442 tests |
| AEP — interaction contract, identity, audit | implemented, with an in-memory reference backend |
| AEP — conformance suites for backends | implemented, 16 suites checked against a deliberately broken backend |
| AEP — running on a real project | in progress: a project can now be discovered, but nothing has been governed by it yet |
| ESS — everything | specified, not built. [`docs/design/`](design/) |

The next honest milestone for AEP is not a feature; it is a team whose work it actually governs. The
first for ESS is parsing and validating one small system end to end — the billing example — because a
model that cannot be validated cannot be compiled.

## What this is deliberately not

Not an LLM orchestration framework, a CI system, a deployment platform, an incident-management
product, a workflow engine, a message broker, or a policy language meant to replace OPA. Not a
universal ontology of software engineering, and not a mandate for microservices, CQRS or event
sourcing.

The responsibility is narrower and stated twice, once per half:

> Define the semantics by which engineering work can be constrained, evidenced, verified and
> progressed — and the semantics by which a software system can be specified once and compiled into
> its contracts, its tests and as much of itself as the specification safely determines.

External systems do the work. This project decides what the results permit.
