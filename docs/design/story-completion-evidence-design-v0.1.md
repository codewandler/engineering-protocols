# Story completion, evidence-gated — Design v0.1

> **Repository:** `codewandler/engineering-protocols`
> **Status:** **proposed, not accepted.** Nothing here is a work order. It is proposed by
> [`harness-wave-4-governed-dogfood.md`](../plan/harness-wave-4-governed-dogfood.md) § W4.3, whose
> acceptance criterion is *a verdict on this document* — accepted, accepted in part, or refused —
> and not a build. Per [`AGENTS.md`](../../AGENTS.md) § *Which documents are normative*, a proposal
> is not a work order however recent it is.
> **Audience:** whoever takes that verdict, and whoever would build it afterwards.
> **Relationship to existing design:** it is `principles/verification/ess-conformance.yaml` applied
> one layer up. That principle joins evidence to a **task's** completion; this one asks what it would
> take to join evidence to a **story's**.
> **Cross-reference:**
> [`harness-planning-and-driver-design-v0.1.md`](harness-planning-and-driver-design-v0.1.md)
> §§ 2.5–2.7 (the store and its deviation register) and §§ 4.7–4.8 (the driver that produces the
> evidence, and the enforce-and-verify standard § 4 applies to itself);
> [`transcript-conformance-design-v0.1.md`](transcript-conformance-design-v0.1.md) § 5 (the record).

---

## 1. Motivation — the claim nobody checks

**Today anyone can move a story to `implemented` with one command, and nothing in the repository can
tell that move from one a run earned.**

`protocol artifact move story:x --to implemented` succeeds exactly when
`artifacts/lifecycles/story.yaml:19` lists `implemented` as a target of `active`. The decision is
`ArtifactLifecycle::permits_transition` — a set lookup over a `BTreeMap<ArtifactStatus,
BTreeSet<ArtifactStatus>>` (`crates/aep-domain/src/artifact.rs:1425-1430`) — reached from
`PlanningDocument::move_status`, whose only argument besides the target status is a
`LifecycleRegistry` (`crates/aep-backend-markdown/src/document.rs:115-142`). There is no fact store
in that call, no execution, no evidence, and nowhere to put one.

So `implemented` is a **claim by whoever typed it**, and the repository already refuses that shape of
claim one layer down. `ess-conformance` says a task does not complete until something other than the
implementing agent ran the specification's own suite, and it states the consequence for a person in
its own header: *"nobody has to read a diff and judge whether it matches the spec. The spec judges
it, and the protocol refuses to call the task done until it has"*
(`principles/verification/ess-conformance.yaml:8-9`).

This document asks the same question one layer up: **what would make a story's `implemented` a fact
rather than a claim?** With harness wave 3 the answer is available for the first time, because for the
first time there is a record of *how the work was done* that the worker cannot mint —
`EvidenceKind::TraceConformance`, established only by `Verifier::TraceChecker`
(`crates/aep-domain/src/evidence.rs:1210-1230`, `crates/aep-domain/src/verification.rs:57-65`), with
a producer that is a constant nothing can set (`crates/trace-spec/src/evidence.rs:72-98`).

## 2. The rule, as a principle over facts

The shape is deliberately `ess-conformance`'s: conditional on the graph, `predicates` +
`evidence` + `artifacts` under one `require:`, `on_failure: block`. Written as prose it would be a
paragraph nothing evaluates; written as a principle it is resolved into the plan and evaluated by the
engine at the completion phase.

```yaml
# principles/verification/story-completion-evidence.yaml   (proposed)
id: story-completion-evidence
version: 1
title: A story is implemented when there is evidence that it was
summary: >-
  Where the work is planned as stories, a task does not complete until the graph holds the
  transcript check for the run that did it and an independent test result for the change it
  produced — so that moving the story to `implemented` records something rather than asserting it.

requires:
  before_completion:
    conditional:
      # Checked against the artifact graph at evaluation time, exactly as ess-conformance's
      # condition is, so adding a story to a project turns the rule on without editing this file.
      - when: artifact.story.exists
        require:
          predicates:
            # One predicate, and the obvious second one is refused below — see § 2.1.
            - trace_conformance.passed
          evidence:
            # The whole point, twice. An agent's account of how it worked is not a check of how it
            # worked, and an agent's account of its own tests is not a test run.
            - kind: trace_conformance
              independent: true
              verifier: trace-checker
            - kind: test_result
              independent: true
              verifier: test-runner
          artifacts:
            # The edge § 3 formalises. Note the limit in § 4.4: this is satisfied graph-wide.
            - kind: task
              relation:
                kind: delivers
                target_kind: story

verification:
  - verifier: trace-checker

on_failure: block
```

Every name in it is already declared for development work and nowhere else: `trace_conformance` as an
evidence kind, `trace-checker` as a verifier and `trace_conformance.**` as an observable family, all
in `protocols/adp/1.yaml:17-44`. The engine refuses a submission whose kind the protocol does not
declare (`crates/aep-engine/src/engine.rs:320-324`), so this principle is admissible under `adp/1`
and inert anywhere else — which is § 5's **D-S5**.

### 2.1 The second predicate, refused by name

The obvious companion to `trace_conformance.passed` is
`trace_conformance.expectations.gapped == 0`. **It is refused, because it cannot fail.**
`trace_conformance.passed` is projected as `status.is_pass() && expectations_gapped == 0`
(`crates/aep-domain/src/evidence.rs:1865-1867`) — the pessimistic reading, taken there for the same
reason `ess_conformance.passed` takes it. Adding the conjunct would restate a condition already
inside the fact.

This matters more than a redundant line usually does, and it is where this design **deviates from the
document it mirrors** — stated rather than applied quietly, because a deviation from a model is a
thing a reviewer should be able to see.

`ess-conformance` writes the rule against exactly this mistake in its own comments: *"a check that
cannot fail is worse than no check, because it reads as protection"*
(`principles/verification/ess-conformance.yaml:51-56`). And then its `predicates:` block lists both
`ess_conformance.passed` **and** `ess_conformance.scenarios.failed == 0`
(`principles/verification/ess-conformance.yaml:26-30`) — while the projection makes `passed` mean
`status.is_pass() && scenarios_failed == 0` (`crates/aep-domain/src/evidence.rs:1834-1835`). The
second conjunct there cannot fail either.

So the model's **reasoning** and the model's **shape** disagree, and this document follows the
reasoning. The one-predicate form is deliberate. Nothing here proposes changing `ess-conformance`:
that is a different document with its own history, the redundancy costs nothing at evaluation time,
and a design proposal is not the place to edit a principle in force.

And the fact is genuinely strong: `expectations_gapped` counts **every** contradicted expectation
including any the caller downgraded to advisory on the command line
(`crates/aep-domain/src/evidence.rs:939-944`), so `passed` cannot be bought with a flag. That is the
same polarity as everything else here — unproven is not proven.

## 3. The `delivers` relation, formalised

**The vocabulary already has the edge; the shared document does not.**

`RelationKind::Delivers` exists in the domain crate — the variant at
`crates/aep-domain/src/artifact.rs:957`, in `ALL` at `:975`, wire name `delivers` at `:993`, inverse
label *"delivered by"* at `:1034`. `artifacts/relations/relations.yaml` declares **twelve** relations
(`:12-75`) and `delivers` is not among them. The CLI already says so out loud, in a doc comment on
the function that answers `protocol artifact relations`:
*"`delivers` has no entry in that file — the sentence here matches its declaration in `aep-domain`"*
(`crates/protocol-cli/src/planning.rs:855-861`).

Two consequences, both worth stating because they are easy to get backwards:

* **`protocol artifact relate story:x delivers …` works today.** `relate` parses through
  `RelationKind::parse`, which reads `ALL`, so the edge is writable and validatable right now. What
  is missing is the *meaning*, in the one document a human reads to learn the vocabulary.
* **The gap is documentation-shaped, not domain-shaped.** No Rust changes to add the row. That makes
  this the cheapest half of the whole proposal and the half that can land whatever the verdict on the
  rest is.

The proposed row, with the direction that makes the requirement legible — the relations document's
own reason for carrying pairings at all (`artifacts/relations/relations.yaml:8-11`):

```yaml
  - relation: delivers
    meaning: Produces the outcome something asked for.
    source: [task, story]
    target: [story, epic, acceptance-criteria, product-requirements]
    note: >-
      Not `decomposes`, and the difference is the whole reason a completion gate can read this edge.
      Decomposition is a statement about planning — this work is part of that work — and a
      decomposed task that is abandoned breaks nothing. `delivers` is a statement about the result:
      the task whose run produced the outcome, not merely the task that was carved out of the intent.
```

`task delivers story` reads correctly in both directions — *"story:x delivered by task:y"* — and it
is the only pairing this design's rule needs. `story delivers epic` is included because it is the
same statement one level up and excluding it would make the vocabulary asymmetric for no reason.

**Enforcing the pairing is out of scope and stays that way.** The lists in that file are advisory
until the artifact validator reads them, and turning them into refusals is harness design open
decision **D2**, deferred on the grounds that it changes a shared document's meaning for every
consumer of the artifact graph. Adding a row to an advisory list is additive; making the list binding
is not.

## 4. What actually enforces it

This is the section the design exists for, because the obvious answer is wrong and the wrong answer
is one sentence long.

### 4.1 Why `artifact move` cannot simply consult the principle

Three separate reasons, each independently sufficient:

1. **Lifecycles are structural.** The move consults a `LifecycleRegistry` and a set of legal targets
   (`crates/aep-backend-markdown/src/document.rs:115-142`,
   `crates/aep-domain/src/artifact.rs:1425-1430`). No facts, no evidence, no execution.
2. **`ArtifactLifecycle` cannot express the rule.** Its three fields are `kind`, `initial` and
   `transitions` (`crates/aep-domain/src/artifact.rs:1386-1395`), under `deny_unknown_fields`. A
   `requires:` clause on a transition is a new field on a document type that six committed schemas
   describe (`schemas/generated/{artifact-lifecycle,planning-document,artifact-manifest,profile,principle,workflow}.schema.json`),
   so it is a domain change wearing a document change's clothes.
3. **A principle's `before_completion` gates a task, not an artifact.**
   `ObligationTiming::default_timing()` is `Before { target: phase "completion" }`
   (`crates/aep-domain/src/principle.rs:119-126`, `COMPLETION_PHASE` at `:48`), and that phase belongs
   to a workflow state — `complete`, terminal, `phases: [completion]`
   (`workflows/development/default.yaml:74-78`). So § 2's principle gates `Engine::transition` into
   `complete` (`crates/aep-engine/src/engine.rs:385-388`) and has no opinion whatsoever about a
   markdown file's frontmatter.

**The spine of the problem, stated once:** the planning store holds artifacts and the engine holds
evidence, and the engine reads the graph as an input and never writes to it — the crate says so on
the constructor (`crates/aep-engine/src/engine.rs:198-200`). *Any* mechanism that gates a store move
on evidence must therefore reach outside the store. There is no arrangement of documents that avoids
this.

### 4.2 Taken: enforce at the move, audit at validate

The repository's own standard for this is already written, in § 4.8 of the driver design: *"an
enforcement mechanism nobody audits is a claim, and an audit with no enforcement is a report about a
horse that has already left."* Applied here it decides the shape — one mechanism that refuses, and a
different one that catches what the first did not see.

**Enforce — `protocol artifact move --task <task>`.** The move verb grows an optional mode: given the
task, it builds the execution the way `protocol evaluate` does and refuses a move to `implemented`
when the completion obligations are not satisfied, printing the `CompletionExplanation` — one line per
unmet requirement — as the refusal. The dependency edge is not new: `planning.rs` already reaches
into the engine for project discovery and the document registry
(`crates/protocol-cli/src/planning.rs:90-114`); what is new is evaluating rather than loading.

Its limit, stated rather than discovered: **it is opt-in.** A move without the flag is unguarded, and
a gate somebody has to ask for is the shape D3 refused for approvals. The mitigation is that the one
actor for which the rule matters most — the driver, closing out a governed run — always passes it,
because the driver already holds the execution.

**Audit — `protocol artifact validate`.** A story in `implemented` with no `delivers` edge from a task
is a **validation error**, accumulated with the others rather than reported alone (accumulation is
wave 1's stated acceptance behaviour). This is the compensating control the store design already
leans on twice: deviation **D-P2** says an out-of-band edit is a permanent property of a file store
and names `validate` as the control, and § 4.8 row 6 calls `validate` *"the strongest audit in this
table"* precisely because it catches an illegal status **whether or not the hook fired**.

What the audit does **not** do, and this is the trade rather than an oversight: it does not stop the
move. It converts an unbacked `implemented` from an invisible claim into a red gate, and it leaves a
window between the move and the next `validate` in which the store says something nothing has
checked. That is the same trade the file-store design already made, one layer up.

### 4.3 Refused, and why: the driver moves the story and nothing else may

The third option is real and is refused for now. The `.engineering/planning/**` write-guard hook
already refuses `Edit`, `Write` and `NotebookEdit` under the store (§ 4.8 row 6). If the *only* actor
permitted to move a story to `implemented` were the driver, at the end of a run whose transition into
`complete` the engine permitted, the evidence gate would be the engine's and the move a mere
consequence — no new verb, no new validator rule.

It is refused because it makes the store's rule depend on a **process** rather than on a **document**:
a store checked out on a machine with no driver would have no rule at all, and the rule would be
invisible to anyone reading the repository. That is the same objection § 3.6 raised against a hook
layer standing in for a driver, pointed the other way. It is worth revisiting when driven runs are the
normal way work happens rather than the new way.

### 4.4 The limit none of the three closes

**An artifact requirement's relation clause is satisfied graph-wide, not per subject.**
`ArtifactRequirement::matches` checks that the artifact is of the kind, is not retired, satisfies the
status, and has *at least one* edge of the relation kind — optionally to *some* artifact of the target
kind (`crates/aep-domain/src/requirement.rs:516-544`). It never asks whether the target is the story
in question, because it has no notion of a subject: `EvidenceRequirement` carries a `SubjectRef`
(`crates/aep-domain/src/requirement.rs:185-187`) and `ArtifactRequirement` has no analogue.

So § 2's `artifacts:` clause is satisfied by **any** task in the graph delivering **any** story. In a
store with one story that is the rule; in a store with forty it is not the rule anybody meant.

There is a second, related limit that constrains the `when:` condition. The graph's fact projection
emits counts and existence flags per kind and per status and **nothing about edges** — the six shapes
are listed on the function itself (`crates/aep-domain/src/artifact.rs:1818-1829`). A condition can
therefore never say *"where a task delivers this story"*; only the `artifacts:` requirement can
mention an edge at all, and that one is graph-wide. `artifact.story.exists` is the strongest condition
the fact vocabulary can express here.

**This is why § 4.2's audit half is not a consolation prize.** `validate` walks every document and
resolves every edge already, so it is the only one of the three mechanisms that can do the join *per
story*. The enforcement half establishes that the obligations were met by the run; the audit half
establishes that *this* story is the one the run delivered. Neither claim is the other, and stating
which is which is the point of this section.

## 5. Deviations and limits

Numbered, because these are decisions and not oversights, and because the honest list is short enough
to read.

**D-S1 — the escape is the profile, not the condition.**
The rule cannot ask *"was this story driven?"*, because being driven is a property of the runner and
the fact families describe artifacts and evidence. So the condition is `artifact.story.exists`, and
the way a project opts out is by not listing the principle in its profile — a principle no profile
names is loaded into the registry and never in force (`crates/aep-engine/src/resolve.rs:87`).
`ess-conformance` is scoped exactly this way in practice: it appears in one profile,
`profiles/development-critical.yaml:20`. The cost is real and is not mitigated here: **a project
cannot mix driven and hand-worked stories under one profile.** Adopting the rule means every story
under that profile owes the evidence, including a documentation story and a story closed by deleting
code.

**D-S2 — the move is not prevented, only invalidated, unless the flag is passed.**
§ 4.2. The window between an unguarded move and the next `validate` is a window in which the store
asserts something unchecked. Inherent to a file store whose whole argument is that it stays readable
and editable; closed only by the driver being the actor that moves things.

**D-S3 — the join is a statement in a file, not a binding.**
The evidence lives in an execution and the story lives in the store, and what connects them is a
`delivers` edge somebody wrote. Anyone who can write the file can write the edge. This is **D-P2 one
layer up**, it is closed by git and `validate` to the same partial extent, and it is not closed by
anything in this document. The contrast is deliberate: the ESS and trace records bind to their subject
by **digest** — a transcript digest and a specification digest, typed so they cannot be transposed
(`crates/aep-domain/src/evidence.rs:906-911`) — and no comparable handle exists between an evidence
record and a planning artifact.

**D-S4 — `independent: true` is structural, not attested.**
Both evidence requirements in § 2 use it, and today it means the producer is not `Producer::Agent`.
Gap-register **D-3** (attested evidence, signatures) stays proposed and nothing here assumes it.

**D-S5 — this rule is development-only, by construction.**
`trace_conformance` and `trace-checker` are declared in `adp/1` and deliberately not in the base
protocol, because widening a declaration later is additive and narrowing one is not
(`protocols/adp/1.yaml:17-33`). A story in an operations project cannot carry this rule.

**D-S6 — the `delivers` pairing stays advisory.**
§ 3. Adding the row does not make an unusual pairing a refusal; that is open decision **D2** of the
harness design, deferred on its own terms.

**D-S7 — the edge has no writer yet, and the one store that exists holds nothing to write it from.**
This repository's own `.engineering/planning/` holds `initiative`, `epic` and `story` artifacts and
**no `task` artifacts at all** — the store's own directory listing is the evidence. So § 2's
`artifacts:` clause is unsatisfiable there today, and it stays unsatisfiable until something creates
the task artifact and relates it. That something is the driven run: a task the driver executed is
exactly the thing a `delivers` edge should point from, and nothing else in the system has a reason to
mint one. Stated here because it changes what an acceptance decision is agreeing to — **the rule
cannot hold before the first driven run creates the first task artifact**, which makes it strictly
downstream of harness wave 4's W4.1 rather than merely proposed alongside it.

## 6. The lifecycle question

**Two shapes, both real, and the recommendation is for the cheaper one — with the other named as
sequenced rather than refused.**

### 6.1 Option A — a new `released` rung

`story.yaml` today is `active: [implemented, archived]`, `implemented: [archived]`
(`artifacts/lifecycles/story.yaml:19-20`). Option A adds `released` after `implemented` and puts the
evidence gate on the **new** move.

*What it buys.* Nothing that works today changes meaning. Every existing lifecycle document, every
existing store and every requirement written `status: implemented` keeps working untouched. And it
says something true that the ladder cannot say at all today: **implemented and released are different
facts**, and only the second is one a target environment can observe (§ 7).

*What it costs.* `ArtifactStatus` is a **closed enum with ten variants**
(`crates/aep-domain/src/artifact.rs:651-672`). A new variant is not a mechanical edit: it touches
`ALL` (`:676-687`), `as_str` (`:690-703`), `parse`, and then three functions that are each a semantic
decision — `is_approved` (`:709-714`), `is_retired` (`:717-719`) and `satisfies` (`:722-728`).
`is_approved` in particular must be decided rather than defaulted, because a requirement written
`status: approved` that stopped being satisfied by the most finished thing in the store would be a
silent narrowing of every existing profile. It regenerates **six** committed schemas, and it forces
four separate decisions on the four planning lifecycles — `story`, `epic`, `task`, `initiative` —
about whether each grows the rung. A kind that grows a rung nothing ever moves to has a ladder with a
dead top.

This is the shape gap-register **D-5** already went through for `EvidenceKind`, and the lesson
recorded there is where the decision belongs: *in the acceptance decision rather than being discovered
during implementation*.

### 6.2 Option B — an evidence gate on the existing terminal move

`active → implemented` stays where it is and acquires a condition. No domain change, no schema
regeneration, nothing new for a reader to learn.

*What it costs.* It **changes the meaning of a move that already exists**, which is the direction that
is not additive. Every store that already moves stories to `implemented` — including, from harness
wave 4's W4.0, this repository's own — starts being refused for a reason that did not exist when the
store was written.

### 6.3 Recommended: B, with the condition carrying the weight

Three reasons.

1. **The cheap error is the reversible one.** B is a principle document, a relations row and one CLI
   mode; A is a closed-enum change across six schemas that cannot be un-shipped quietly. Taking B and
   later needing A costs one extra decision. Taking A and later finding the rule wrong costs a
   migration of every store that adopted it.
2. **The condition is what makes B survivable, and it is `ess-conformance`'s own trick.** That
   principle is conditional on `artifact.executable-system-specification.exists` so that *"a task with
   no ESS owes nothing here"* (`principles/verification/ess-conformance.yaml:21-24`). B takes the same
   shape — with the honest caveat of **D-S1**, that the condition available here is coarser than the
   one available there.
3. **A is sequenced, not refused.** § 7's milestone needs a rung meaning *observed running*, and
   `implemented` cannot be made to mean that without lying. When that milestone is taken up, `released`
   arrives with a reason and a second evidence kind behind it — a far better acceptance decision than
   adding a rung now and looking for something to put on it.

### 6.4 What closes this question

Not an argument. Two observations, neither of which can be made before harness wave 4's W4.1 has run:

* **how many stories in a real store reach `implemented` by a route the driver did not drive** —
  measured on this repository's own store, not estimated. If that number is large, B's coarse
  condition is carrying more weight than a condition should, and A becomes the better shape;
* **whether the released-target-environment milestone (§ 7) is taken up at all.** If it is, `released`
  arrives for its own reasons and B becomes a temporary spelling of a rule that ends up on the new
  rung anyway.

That is why this document is proposed and not accepted, and why W4.3's acceptance criterion is a
verdict rather than a build.

## 7. Later milestone — the released target environment

**Explicitly not v0.1 scope.** It is written down so that § 6.3's recommendation reads as *"not yet"*
rather than *"no"*, and so that whoever takes it finds the gaps already named.

The full shape: `implemented` says a change exists and was checked. It says nothing about whether it
is *running anywhere*. The infrastructure family already produces the observation that would close
that — a cluster read into an IR, decided against an authored desired state, with every gap projected
back as a reviewable diff (infra waves 1–4). And the evidence vocabulary is there:
`EvidenceKind::HealthObservation`, `MetricObservation` and `DeploymentResult`
(`crates/aep-domain/src/evidence.rs:1190-1195`), with `Verifier::TelemetryQuery`
(`crates/aep-domain/src/verification.rs:45-46`).

What it would say: **a story is `released` when a named target environment was observed to be running
the change that delivered it.**

Three things are missing, named rather than waved at:

1. **No evidence kind joins an infrastructure observation to a story.** There is no
   `infra_conformance`, and minting one is the D-5 decision shape again — a closed enum, a verifier
   class, a protocol declaration and an observable family, decided together or not at all.
2. **Nothing binds a deployed artefact to a diff.** The ESS and trace families bind by digest;
   infrastructure observation has no comparable handle on the change, so *"running the change that
   delivered it"* is currently a sentence with no mechanism under it.
3. **`released` is the rung § 6 declined to add now** — and this is the milestone that would give it a
   reason, which is exactly the order those two decisions should be taken in.

## 8. Open decisions

Each with the default taken if nobody decides otherwise.

**S1 — where the principle lives, and which profile lists it.**
*Default:* the document ships at `principles/verification/story-completion-evidence.yaml` and is
listed by **no profile** until the verdict is taken; an unlisted principle is loaded and never in
force (`crates/aep-engine/src/resolve.rs:87`), which is the reversible half. On acceptance, list it in
`development.standard` — the profile harness wave 4's W4.1 drives under — rather than in
`development.critical`, where `ess-conformance` sits, because trace conformance is cheap where a
conformance run is not.

**S2 — the `delivers` row in `relations.yaml`.**
*Default:* add it, as § 3 writes it, advisory. This is separable from the rest of the design and is
worth landing whatever the verdict, because the vocabulary is already in the binary and the shared
document is a row short of describing it.

**S3 — how a story points at the run that delivered it.**
*Default:* the delivering `task` artifact's `location` is the run directory —
`ArtifactLocation::RepositoryPath` at `.engineering/runs/<run-id>/`, which needs **no** change to the
`aep.planning-md/1` frontmatter format. *Refused alternative:* a new frontmatter field, which is a
format change to a published schema for something an existing field expresses.

**S4 — a story whose run wedged.**
*Default:* nothing happens. It stays `active`. A story that cannot reach `implemented` because the
evidence is not there is the rule working, not a bug in it.

**S5 — whether `validate`'s new rule is an error or a warning.**
*Default:* an error, accumulated with the others. `validate` has no warning level today, and inventing
one so that the first rule to need it can be ignored is how a check becomes decorative.

## 9. What this is not

* **Not a replacement for review.** `review.approved` is a separate transition guard
  (`workflows/development/default.yaml:136-139`) and stays exactly where it is. This rule says the
  work was done and checked; it says nothing about whether it was the right work.
* **Not attestation.** See **D-S4**.
* **Not a bridge from `protocol artifact` to `protocol entity`.** That is harness design open decision
  **D3**, deferred until the store implements the contract, and nothing here brings it forward.
* **Not a claim that a driven story is a better story.** The rule is about whether a status records
  something. Whether driving produces better work is a measurement nobody has taken, and harness
  wave 4 says so on its own page.
