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
> description rather than maintained beside it. When the description changes, let what that
> invalidates be derived too.

The second sentence was added deliberately, in August 2026, and it is an amendment rather than a
clarification. "Specified once and compiled" describes a system in the present tense, and every real
adoption is brownfield: a model you can only use on day one is a model nobody adopts on day two. The
concrete pressure came from the oracle. Binding conformance evidence to the specification revision it
attests is correct and blunt — without a semantic delta, changing a comment in an unrelated domain
sends every conformance requirement back to owed, which is indistinguishable from having checked
nothing.

So *what changed* is now part of what this project derives, alongside the contracts and the tests.
What is still deliberately outside it is what any of that implies anyone should **do**: a delta says
what a revision invalidates, and a person decides what happens next.

The model reasons. The protocol constrains. The specification defines. The verifiers establish facts.
What the loop asks you to trust is narrow and named: that a producer declaring itself independent is.
Everything downstream of that declaration is checked.

That one declaration is a gap, and it is worth naming as one. `independent: true` is a boolean over a
self-declared `Producer` (`crates/aep-domain/src/evidence.rs`); nothing binds a verifier's identity to
the evidence it submits, and the harness guide says so outright — the engine will record a test result
the harness invented. Closing it means attested
evidence: a signature over the record and a key the protocol already knows. There is no signature, no
key and no attestation anywhere in the workspace. A proposed shape now exists —
[`docs/plan/gap-register.md`](plan/gap-register.md) D-3 — and is not accepted, so this is a gap with
an owner rather than a horizon.

## Where this stands

Stale numbers were this section's failure mode: in the repository's first 48 hours the hand-written
gate count here drifted against three other documents' counts, every copy true when written and
wrong within hours. So this section no longer carries counts or a tag pin. The delivered-waves
table in [`docs/status.md`](status.md) is generated from the annotated tags and drift-checked in
the gate; `git tag -n99` is the per-wave record; `task check` is the measurement.

| | State |
|---|---|
| AEP — domain model, engine, documents, CLI | implemented |
| AEP — interaction contract, identity, audit | implemented, with an in-memory reference backend |
| AEP — conformance suites for backends | implemented, 16 suites checked against a deliberately broken backend |
| AEP — running on a real project | in progress: a project can be discovered and planning artifacts have a durable store, but no team's work has been governed by it yet |
| ESS — model, compiler, four projections | implemented, waves 1–3 |
| ESS — the specification as an oracle | implemented, wave 4: the generated suites are the verdict |
| ESS — semantic diff, what a revision invalidates | implemented, wave 5, first slice; extended by wave 7 down to the generated artifact |
| ESS — structural synthesis: three emitters, one plan, the dual-target demonstration | implemented, waves 6–7 |
| Infra — observation, diagnosis, desired state, gaps projected back as patches | implemented, infrastructure waves 1–4 |
| Harness — the planning store and the Claude Code plugin | implemented, harness wave 1; the driver is decided and not built |

The next honest milestone for AEP is not a feature; it is a team whose work it actually governs.
ESS closed its loop — one specification into contracts, generated suites, three structural
skeletons and a verdict on two running applications — and what remains open there is behavioural:
every algorithm is still a typed obligation a person implements. The open gaps, each with what
closes it, are in [`docs/plan/gap-register.md`](plan/gap-register.md).

## Proposed, not accepted

Six design documents in [`docs/design/`](design/) propose extending this. They are listed here with
their status so that reading the newest file in that directory cannot be mistaken for reading what
this project has agreed to build. The tally is deliberately not stated in a sentence: a count in a
preamble is the first thing to go stale, and the status column is the answer.

| proposed design | what it would add | status |
|---|---|---|
| [closed-loop execution and conformance](design/ess-closed-loop-execution-conformance-design-v0.1.md) | the specification becomes an *oracle* — a verdict on an implementation, not only a projection of a model | **delivered** as ESS wave 4 |
| [semantic diff, impact and evolution](design/ess-semantic-diff-impact-evolution-design-v0.1.md) | the system changing over time, impact closure, what a revision invalidates | **core accepted** into the thesis above and sequenced as ESS wave 5; reviewed, and its proposal-evaluation and architecture-search sections rejected rather than deferred |
| [structural synthesis, obligations and realizations](design/ess-structural-synthesis-obligations-realizations-design-v0.1.md) | generated applications, and human or agent work carried as typed obligations | proposed; reviewed once and not reconciled — that review reads it as four waves, not one; unsequenced |
| [infrastructure discovery and multi-cloud realization](design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md) | a **fourth domain**: infrastructure, with `InfraSpec` and `InfraIr` beside the ESS pair | reviewed, and deferred whole with two ideas harvested; unsequenced |
| [the planning store and the reference driver](design/harness-planning-and-driver-design-v0.1.md) | a durable store for planning artifacts, a Claude Code plugin that uses it, and the first in-repository implementation of the harness contract | **Phase 1 accepted as harness wave 1; driver decided and sketched, build unsequenced.** The narrowing it rests on is the one recorded above |
| [transcript conformance](design/transcript-conformance-design-v0.1.md) | a typed, executable specification over an *agent-run transcript* — the `infra-spec/1` pattern pointed at a third observation domain, with the same three verdicts and a checker that contains no model | **accepted, in implementation** as trace wave 1 (`docs/plan/trace-wave-1-transcript-checker.md`). It is what makes a behavioural claim about an agent admissible as evidence, which the driver's `llm` step needs and no other mechanism supplies. Its per-request *series* assertions and its streaming checker stay proposed, by name |

Closed-loop conformance and structural synthesis are horizons the two halves already implied — the
thesis promises the tests and the skeleton, and wave 4 delivered the first of them. Semantic diff was
not implied, and absorbing it was taken as a deliberate amendment with the reason recorded above.

Infrastructure remains outside. It is a second subject matter rather than a further projection of the
first, and a review of it recommended deferring the whole and harvesting two ideas. Absorbing it would
be another amendment, and nobody has made the argument for one.

The planning store and the driver are a different case again. Neither adds a subject matter: the
store holds artifact kinds the protocol has always modelled and never had anywhere to put, and the
driver implements a contract this repository already published. What the second of them needed was
not an amendment to the thesis but a narrowing of one refusal, and that narrowing is above, with the
reason beside it.

## What this is deliberately not

Not an LLM orchestration framework, a CI system, an incident-management product, a message broker, or
a policy language meant to replace OPA. Not a universal ontology of software engineering, and not a
mandate for microservices, CQRS or event sourcing. One earlier entry in this list has been narrowed,
deliberately: this is still not *a workflow engine* in the sense refused before — a general-purpose
orchestrator other systems are built on — but the repository now ships one **reference driver**, a
default harness that walks its own workflows by asking the engine the seven questions and doing,
outside the engine, only what the answers permit.

The narrowing has an argument, because a refusal that quietly differs from last month's is
indistinguishable from a refusal somebody forgot. The harness contract — seven calls, three rules —
is published in [`docs/guide/harness.md`](guide/harness.md) and mandated by the normative
consolidated design, and it is implemented by no program in this repository. A contract with zero
implementations is the same defect as an invariant enforced by nothing. The driver is that
implementation, and the line it does not move is the one that mattered: the engine still evaluates
and never acts (invariant 7 unchanged — an agent's own statement never satisfies an independence
requirement), gates are still evaluated by the engine and never by the driver, and "external systems
do the work; this project decides what the results permit" remains true — the driver is the first of
those external systems, kept in-tree the way the website is: a deliverable beside the specification,
consuming only its public surface. Decided by the operator, 2026-08-21; designed, not yet built —
building it waits behind its own reviewed design.

**Not a deployment platform**, and the infrastructure design does not change that — it makes the line
worth drawing precisely. Generating an artifact is in scope: this project may compile a specification
into the file that describes an infrastructure, and decide whether an infrastructure's observed state
conforms to what was specified. *Operating* a system is not: nothing here calls a cloud API, holds a
credential, applies a plan or watches a rollout. Actually deploying something is optional, later, and
somebody else's process.

The responsibility is narrower and stated twice, once per half:

> Define the semantics by which engineering work can be constrained, evidenced, verified and
> progressed — and the semantics by which a software system can be specified once and compiled into
> its contracts, its tests and as much of itself as the specification safely determines.

External systems do the work. This project decides what the results permit.
