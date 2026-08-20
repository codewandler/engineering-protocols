# Vision review — design, concept and ambition — 2026-08-20

Reviewing the **overall design, concept and vision** of `engineering-protocols` at `3647f80`
(`main`, CI green). Not the code's correctness: two other reviewers own that. The question here is
whether this thing is worth building as designed, whether its parts cohere, and whether its own
stated ambitions are in tension with each other.

Sources read in full: `docs/VISION.md`, `AGENTS.md`, `README.md`,
`docs/design/reconciliation-v0.2.md`, `docs/design/ess-review-v0.1.md`,
`docs/plan/ess-roadmap.md`, `docs/plan/ess-wave-3-projections.md`, `docs/plan/wave-4-dogfooding.md`,
`docs/guide/harness.md`, and both new designs
(`ess-closed-loop-execution-conformance-design-v0.1.md`,
`ess-structural-synthesis-obligations-realizations-design-v0.1.md`) end to end;
`docs/design/consolidated-design-v0.2.md` and `docs/design/ess-implementor-design-v0.1.md` by
section. Deliberately no overlap with `docs/reviews/2026-08-20-full-repo-review.md`, which covered
gate health and the cycle/inhabitability rule ownership.

Every claim below carries a `file:line`, a document section, or a command's output. Where a claim
is a hypothesis it says so. Two findings rest on another agent's verified observation and are
attributed as such.

---

## Verdict

The central bet is sound in the half almost nobody attempts, and oversold at exactly one seam.
Turning *submission order*, *revision-binding* and *unobserved-versus-false* into machine-checked
facts is genuinely stronger than prose, and this repository is more honest about what it has not
built than most shipped products are. But the design's mechanism of action — *make the violation
unrepresentable* — is unevenly achieved, and nothing records where. Where it is achieved (unresolved
references absent from the IR, validated types that cannot be deserialised, deny-wins composition)
it is excellent. Where it falls back to a hand-written test it is exactly as strong as the prose the
project was built to replace, and in the last two waves that gap produced two real failures nobody
noticed: three projections publishing disagreeing contracts from one model, and three breaks of the
capability envelope that leave all 916 tests green. `docs/VISION.md`'s closing claim — *"Nothing in
the loop asks anyone to be trusted"* — is false as written; the harness is trusted absolutely and
unverifiably, and `docs/guide/harness.md` is prose. ESS belongs here: the join is real,
one-directional and tested. But it is thinner than the vision's diagram implies, and the project is
converging fast on the half it can prove with tests it writes for itself while the half that needs
contact with real work has not started. The two new designs were written against the roadmap rather
than the shipped code, and re-derive three model decisions the code already makes better.

---

## Findings

| # | Severity | Finding | Cost to fix now | Cost later |
|---|---|---|---|---|
| V1 | high | The typed guarantee is thinner than the design implies, and nothing records which invariant is held by a type and which by a test | small: an invariant register with a mechanism column | the confidence is unearned in an unknown subset |
| V2 | high | Independence is a self-declared label; the vision claims a property the architecture lacks | small: one field plus one refusal | public API, and every stored evidence record |
| V3 | high | ESS conformance evidence is not bound to the specification it claims to check | small: a digest plus a `covers`-style check | migration of every stored record |
| V4 | high | Vision-versus-practice drift: the vision's own next AEP milestone is unstarted; ESS took three waves and eight more are proposed | free — it is one wave, and it needs no new code | compounding |
| V5 | med-high | Both new designs were written against the roadmap, not the shipped code | one hour of doc edits | a second source of truth in code |
| V6 | med-high | Wave 5's premise — the agent as residual synthesiser — is an empirical claim with no experiment, no baseline, no metric | a day of experiment design | eight waves premised on an untested claim |
| V7 | medium | `may:` is the new unenforceable invariant — review finding F4 one level up | a model addition while the format is `ess/1` | the oracle certifies authorisation by silence |
| V8 | medium | Wave 5 §28 makes the synthesis planner a capability-granting authority | doc edit | a fourth granting authority nobody audits |
| V9 | medium | The join is real, but thinner and more asymmetric than the vision's diagram | — (assessment) | — |
| V10 | medium | The oracle requires a forced-outcome test backdoor in every conformant implementation | state the rule | every adopter ships the switch |
| V11 | low-med | Two incompatible obligation vocabularies in one commit; invented relation names duplicate existing ones | doc edit | the roadmap cites both |
| V12 | low | Fact spellings in wave 5 that the engine cannot evaluate | doc edit | copied into code |
| V13 | low | `async_trait` re-decides a question `aep-contract` settled deliberately | doc edit | a boxing dependency in a specification crate |
| V14 | low | `docs/VISION.md` is stale by three waves; the roadmap contradicts its own table; neither has a maintenance rule | minutes | the vision is the worst document to be stale |
| V15 | note | Over-built for the stage: `aop-domain`, conformance levels. Under-built and expensive later: ESS specification evolution | — | `ess diff` retrofit touches the evidence type and the drift check at once |

---

## V1 — The typed guarantee is real, but unevenly distributed, and nothing says where

**Severity: high. Judged against the project's own stated mechanism first.**

The thesis is not merely "write the rules down". It is stronger and better than that:

> **"An agent cannot verify itself" stops being a principle and becomes a type.**
> — `docs/VISION.md`, § "Why this matters more with agents than without"

That sentence names the mechanism of action for the entire project: a violation should become
*unrepresentable* or *refused*, not merely *tested for*. Where the repository achieves it, the result
is genuinely excellent, and the wave-2 compiler is the best example — an unresolved reference cannot
be held by `EssIr` at all. Where it does not achieve it, the guard is an ordinary hand-written test,
which is precisely as fallible as the prose the project exists to replace. **The design does not
distinguish the two cases anywhere**, and `AGENTS.md` § Invariants presents all sixteen as one flat
list, so a reader cannot tell which of them is held by the compiler and which by somebody having
remembered to write a test.

Two independent observations from today say the gap is not theoretical.

**Instance 1 — one source of truth was not one source of truth, for a whole wave.**
`docs/plan/ess-wave-3-projections.md:147-163` records it in the project's own words:

> "The one that actually happened was three projections getting it *differently*: each carried its
> own copy of the type mapping, and every one of the 17 comparable projection pairs disagreed.
> `AsyncAPI` was the permissive side … so a service validating an event against the published
> `AsyncAPI` document accepted a `Money` with a non-numeric amount and unknown extra fields that the
> JSON Schema tree refused."

and draws the right conclusion:

> "'the same value, deliberately, without importing it' is not a property, it is a hope, and only a
> test comparing the outputs makes it one."

This is the thesis failing inside its own implementation, at the layer the thesis is about. The
single-source-of-truth property — *"let everything else be derived from that description rather than
maintained beside it"* (`docs/VISION.md`, § "The thesis") — was architecturally intended, was stated
in the wave plan, and was false in the shipped artifacts until `crates/ess-gen/tests/agreement.rs`
was written. Worse, the failure was *asymmetric in the dangerous direction*: the derived contract was
more permissive than the model, so an adopter trusting the published AsyncAPI document would have
accepted values the specification refuses. Nothing about the architecture prevented this; a test
caught it.

**Instance 2 — the capability envelope is guarded by tests, and the tests do not bite.**
Reported by the concurrent mutation-testing reviewer and cited here as **verified by another agent,
not re-derived by me**: three separate breaks of the capability / approval-floor safety envelope
leave all 916 tests passing, including one in which a *refused* approval authorises the action. The
capability model is the one the README leads with as a *safety envelope* ("`deny` cannot be granted
back by a later document, so it works as a safety envelope") and the one `docs/VISION.md` offers as
its second headline example ("the protocol refuses to resolve a profile that grants production
access outright — so the mistake cannot be made, rather than being noticed in review"). For the
parts of that envelope held by a test rather than by the type lattice, "cannot be made" is not what
the code delivers.

**Why this is a vision finding and not a test-coverage finding.** The project's confidence — in the
README, in the tag messages, in the vision — is drawn from the architecture's *intent*. 916 green
tests and 16 stated invariants read as though the invariants are structural. Some are:
`unsafe_code = "forbid"` and `missing_docs = "warn"` (`Cargo.toml` `[workspace.lints]`) are compiler
facts; invariant 2's "validated types do not implement `Deserialize`" is a type fact; invariant 9's
`BTreeMap`-only is checkable by grep. Most are not. A reader has no way to tell, and neither, on
today's evidence, did the authors.

**The missing construct its own logic demands: an invariant register with a mechanism column.**
The repository already invented exactly the right institution for exactly this problem — the
deliberate-deviation register at `docs/design/reconciliation-v0.2.md` §5, which converts drift into
recorded decisions. The same move applied to the sixteen invariants would name, per invariant, what
holds it:

| mechanism | meaning |
|---|---|
| *type* | the violation does not compile or cannot be constructed |
| *refusal* | validation or resolution rejects it, with a `ValidationCode` |
| *test* | a hand-written test would catch it — as strong as the test |
| *convention* | nothing checks it |

This is cheap (a column and an afternoon of honest classification), it is mechanically checkable
against the tree, and it turns "916 tests pass" into a claim a reader can calibrate. It would also
have made both instances above visible before they shipped: the type-mapping agreement was
*convention* until wave 3 ended, and the approval floor is *test*.

**Cost now:** an afternoon, and the honesty is free — the deviations register shows the project can
do this. **Cost later:** every subsequent wave inherits confidence it has not earned in a subset
nobody can name, and each new "the type prevents it" claim in a tag message compounds it.

---

## V2 — "Nothing in the loop asks anyone to be trusted" is false; the harness is trusted absolutely

**Severity: high. Judged against the vision's own claim.**

`docs/VISION.md` § "The thesis" closes with: *"The model reasons. The protocol constrains. The
specification defines. The verifiers establish facts. Nothing in the loop asks anyone to be
trusted."*

The last sentence is not true, and the size of the exception is the whole enforcement boundary.

- `crates/aep-domain/src/requirement.rs:254` is the entire independence mechanism:
  `if self.independent && record.producer.is_agent() { return false; }`. One boolean over a
  **self-declared** enum.
- `crates/aep-engine/src/engine.rs:50-65` — `EvidenceSubmission { evidence, producer, subject,
  provenance, entity }`. There is no *submitter* field. Whoever calls `submit_evidence` states who
  produced the observation, and nothing compares that statement to anything.
- `crates/aep-domain/src/evidence.rs:790-792` — `Provenance.digest` is documented "for tamper
  detection", and is supplied by the same caller.
- Verified absence: grep for `signature|signed|attestation|cryptograph` across `docs/design/*.md`
  and `docs/VISION.md` → **0 hits**. There is no attestation concept anywhere in the design.

The repository is honest about this at the level of the guide — `docs/guide/harness.md:112`: *"The
engine will happily record a `TestResult` you invented. Nothing downstream can tell — which is
exactly why this is the harness's job and not the engine's."* But that honesty is filed where only
an implementor reads it, while the vision states the opposite to everyone else.

**The sharp version of the problem.** The project's own argument against prose is:

> "A person who ignores the wiki page can be asked why. An agent given the same page in a prompt
> will produce something that reads as though it followed it … Prose instructions do not fail
> loudly; they fail silently and plausibly."
> — `docs/VISION.md`, § "Why this matters more with agents than without"

Now read `docs/guide/harness.md:144`: *"Do the mapping once, at tool-registration time, and check
before each call."* The innermost enforcement boundary of the entire system — whether the agent's
tools are actually restricted to the capabilities the protocol granted — is a prose instruction to a
harness author, in a document a harness author reads once. Every argument the vision makes about
wiki pages applies to it verbatim. `harness.md:110-126` and `:128-149` are, structurally, the wiki
page the project was built to replace.

**What is actually available to fix it, cheaply.** The design already distinguishes actor from
executor (§58 "Actor and Execution Identity"), and `aep_engine::trail::command_context` already
builds a context carrying both. The engine therefore *can* know who submitted a piece of evidence,
separately from who is claimed to have produced it. One field and one refusal — reject
`independent: true` evidence whose submitting executor is the execution's own executor — turns the
label into a check for the most common case (an agent relaying its own test output). It does not
solve a determined liar, and nothing in-process can; but it converts the default failure mode from
silent to refused, which is the project's entire thesis.

**The larger missing construct: a harness conformance suite.** `aep-conformance` exists for a stated
reason — a backend must *prove* it implements the contract instead of claiming it (tag
`0.2.0-wave-3`: "A backend can now prove it implements the contract instead of claiming it"). The
harness makes a structurally identical claim — *I asked before acting, I labelled producers
honestly, I submitted evidence as I observed it* — and there is no suite, no level, and no faulty
reference harness. Sixteen suites defend the storage contract, which is the half where the risk is
low; zero defend the interaction contract with the agent, which is the half where the entire thesis
lives. That asymmetry is the single clearest structural gap in the vision.

**Cost now:** reword the vision today (minutes); the submitter field is roughly half a day and is a
public-API change, so it is much cheaper before an external harness exists. **Cost later:** every
stored evidence record lacks the field, and the API is published.

---

## V3 — ESS conformance evidence is not bound to the specification it claims to check

**Severity: high. Judged against the project's own signature design decision.**

The README leads with this decision, and it is the right one:

> **An approval names the revision it approved.** An approval of version 3 of a design does not
> silently authorise version 7 — otherwise a reviewer's name ends up attached to a decision they
> never saw.

Implemented properly for reviews: `crates/aep-domain/src/review.rs:243-260` (`ReviewResult::covers`)
with `FreshnessPolicy`, and tests at `review.rs:294-305` including
`an_approval_of_version_three_does_not_cover_version_seven`.

The join to ESS does not do this.

- `crates/aep-domain/src/evidence.rs:675` — `EssConformanceResult.specification: String`. Free-form
  text. There is no digest field on the type.
- `crates/ess-gen/src/provenance.rs:23` — every *generated artifact* carries `source_digest`, a
  digest of the resolved IR, with good stated reasoning (`:18-22`). **The two ends of the join do not
  speak the same identity.** The generator knows exactly which model it derived from; the evidence
  records a string.
- `principles/verification/ess-conformance.yaml` requires `ess_conformance.passed`,
  `ess_conformance.scenarios.failed == 0`, an independent producer, and the presence of an
  `executable-system-specification` artifact — and never relates the evidence's `specification`
  string to that artifact, its version, or its digest.
- The passing end-to-end test demonstrates the gap without noticing it:
  `crates/aep-engine/tests/end_to_end.rs:285-296` submits `specification: "billing/v3"` against an
  artifact registered as `ess:billing` carrying no version at all. Nothing checks the correspondence,
  because nothing can.
- Verified absence: no freshness or staleness concept exists on evidence at all — grep
  `freshness|Freshness` in `crates/aep-domain/src/evidence.rs` → 0 hits. Reviews have one
  (`requirement.rs:641`); evidence does not.

**Consequence for a real person.** A conformance run against `billing/v3` passes. Somebody edits the
specification — adds a required field to a command input, tightens an invariant, adds an outcome. The
task's completion predicate is still satisfied by yesterday's evidence, and the protocol reports the
work done and conformant against a specification that has moved. That is exactly the situation the
review-freshness rule exists to prevent, reproduced in the newer half, for the artifact the project
is in the process of making authoritative over generated code and generated tests.

**The fix is mechanical and the pieces already exist.** `crates/ess-domain/src/system.rs:370-372`
already produces the qualified identity (`billing/v3`), and `ess_gen::Provenance` already computes
the IR digest. Add `spec_digest` to `EssConformanceResult`, project it as a fact, and have the
principle require it to match the specification artifact in the graph — a `covers` for conformance.

**Cost now:** small, and it is the cheap half of the specification-evolution problem in V15.
**Cost later:** the evidence type is published in `schemas/generated/`, so this becomes a schema
migration plus a rewrite of every stored record, and any evidence produced before the fix is
permanently unattributable.

---

## V4 — The project is drifting from the milestone its own vision names as next

**Severity: high. This is the answer to question 7 (vision versus practice).**

`docs/VISION.md` names two next milestones, and is explicit about which is the honest one:

> "The next honest milestone for AEP is not a feature; it is a team whose work it actually governs.
> The first for ESS is parsing and validating one small system end to end — the billing example."
> — `docs/VISION.md`, § "Where this stands"

ESS's milestone has been delivered three times over: parse and validate (`0.3.0-ess-wave-1`),
compile to an IR (`0.3.1-ess-wave-2`), project into four artifact families (`0.3.2-ess-wave-3`).

AEP's has not started.

- `docs/plan/wave-4-dogfooding.md:3` — still **"In progress."** W4.1 (project discovery) shipped as
  `0.2.1`. W4.2 ("`.engineering/` for this repository") and W4.3 ("What it says about the last three
  waves") have not begun.
- `ls -a .engineering` → **no such directory.** The capability to be governed shipped; the
  governance did not.
- The reach of the join into practice is correspondingly small: `ess-conformance` appears in **one**
  of five profiles (`profiles/development-critical.yaml:20`), and no example task uses that profile
  (`examples/development-passkeys/task.yaml` → `development.standard`).

**Measured, not adjectival.** ESS is 22,427 of 74,690 lines of Rust (30%) and 411 of 916 tests
(45%): `ess-domain` 13,787 lines / 243 tests, `ess-compiler` 3,432 / 45, `ess-gen` 5,208 / 123. The
AEP milestone the vision calls "the next honest" one has **0** lines. The two new designs propose
**W5 through W12** (`ess-structural-…-v0.1.md:1638-1830`) — eight further waves, all ESS — with no
matching AEP work and no wiring into `docs/plan/ess-roadmap.md`.

**Context that makes this sharper, not softer.** `git log --format="%h %ad %s"` shows the entire
repository was built between `08-19 23:37` and `08-20 10:51` — about eleven hours, roughly 7,000
lines an hour. The waves are hours apart, not weeks. At that velocity the *choice of next wave* is
the only meaningful steering input in the project, and it has gone to ESS four times running.

**Verdict: drifting, and in a specific direction** — toward the half whose claims can be discharged
by tests it writes for itself, away from the half whose claims can only be discharged by contact with
work somebody actually did. That is not an accident of preference; it is the gradient. ESS work
produces green tests and tag messages. Dogfooding produces findings, most of them uncomfortable, and
`docs/plan/wave-4-dogfooding.md:41` says so in advance: *"The interesting output is what it
**refuses**. A rule we cannot express, or one that fires when it should not, is a finding about the
protocol rather than about the repository."*

That sentence is the best paragraph in the planning tree, and it has been true and unexecuted for
nine hours and four waves.

**Cost now:** one wave, and it requires no new code — the discovery mechanism shipped in `0.2.1`.
**Cost later:** every wave of ESS increases the amount of protocol surface that has never met real
work, and V1's uneven-enforcement problem is exactly the class of defect that dogfooding finds and
unit tests do not.

---

## V5 — Both new designs describe a repository that no longer exists

**Severity: med-high.**

- `ess-closed-loop-…-v0.1.md:8` — *"`ess-gen` exists locally but is not yet pushed."* Lines `91-93`
  list the ESS side as `ess-domain` and `ess-compiler` only; `:131` repeats it. `ess-gen` landed in
  `05c3d04`, and the design was committed *after* it, in `3647f80`.
- Measured: `grep -c "TestStrategy|assertion_style|OutcomeCondition|construct_input|inject_fault"`
  over both new designs → **0** and **0**.

The shipped model already makes the three decisions the wave-4 design spends its middle third
re-deriving, and makes them better, with the reasoning recorded:

| Shipped | Wave-4 doc |
|---|---|
| `crates/ess-domain/src/command.rs:177-215` — `OutcomeCondition::{When, Otherwise, External{cause}}`, three cases *"rather than an `Option<Predicate>`, because a generated conformance scenario has to treat them differently"*; `cause` required because *"'it can fail' without saying how is not something a test author can act on"* | §12 (`:464-501`) proposes `ExternalOutcomeControl` from scratch |
| `command.rs:233-240` — `TestStrategy::{ConstructInput, DefaultBranch, InjectFault}`, *"exposed on the model rather than decided in each generator … where it can be wrong once instead of once per target"* | §11 (`:431-460`) proposes a four-step witness ladder |
| `crates/ess-domain/src/view.rs:10-21, 91-96` — `Consistency::assertion_style()` → `Expect`/`Eventually`, with the asymmetry argued (*"the cheap mistake is the default"*) | §14 (`:534-589`) re-derives expect-versus-eventually |

**Why this matters beyond tidiness.** An implementor who follows §11 builds a second answer to "how
does a generated test reach this outcome". That is finding F2 from
`docs/reviews/2026-08-20-full-repo-review.md:43` — one rule, two implementations, which genuinely
disagreed — reproduced at design level, before any code is written. And V1's instance 1 is the same
failure class a third time: three copies of one mapping. This repository has now produced the
one-rule-two-implementations defect three times in two waves. It is the project's characteristic
bug, and the new designs invite the fourth.

**Engage with the recorded reason.** The wave-4 doc is not careless about this; line 8 says
*"Reconcile names with the actual local API rather than creating duplicate abstractions."* The
instruction is right. The doc was committed without anyone following it. The cheapest fix is the
institution the repository already has: a short reconciliation header per doc, in the shape of
`reconciliation-v0.2.md` §5, naming what shipped and which paragraphs it supersedes.

**Cost now:** an hour. **Cost later:** a duplicate scenario-derivation path in code, discovered by a
review after it ships.

---

## V6 — Wave 5 rests on an empirical claim with no experiment, no baseline and no metric

**Severity: med-high. Labelled a hypothesis, mine and theirs.**

The deepest claim in the newer design is not about types at all:

> "Deterministic synthesis handles the specified portion of the program; agentic synthesis handles
> the residual implementation obligations. The LLM becomes a residual synthesizer rather than the
> primary generator of the entire software system."
> — `ess-structural-…-v0.1.md:1570-1592` (§38)

and:

> "This reduces agent scope dramatically. The agent receives: *Implement this missing semantic
> contract* instead of: *Implement the email service*." — §23 (`:1057-1069`)

Everything else in this repository is falsifiable by a test. This is falsifiable only by
**measurement**: it is a claim about how well a model performs when its scope is narrowed and its
context is a computed closure rather than a repository. It may well be true — it matches how
constrained synthesis works elsewhere, and §27's context-closure argument (`:1173-1199`) is a good
one. But §44 and §45, the two acceptance-criteria sections, contain no comparison, no baseline arm,
and no metric. Eight proposed waves rest on it and none of them tests it.

**What would falsify it, and why wave 4 makes the experiment possible.** Once the oracle exists,
the experiment is nearly free and entirely in the project's idiom: run two arms against the billing
obligations — one agent given the obligation contract and its context closure, one given the
repository and the specification — and compare conformance pass rate and iterations to green. The
oracle is the measuring instrument, which is a further and rather elegant argument for the wave-4
before wave-5 ordering the roadmap already chose.

**Cost now:** a day, folded into wave 4's acceptance criteria. **Cost later:** the scaffolding for
waves 5–12 gets built on an assumed payoff, and the first honest measurement arrives after the
investment.

---

## V7 — `may:` is the new unenforceable invariant

**Severity: medium. This is review finding F4 reappearing one level up.**

`ess-review-v0.1.md:101-112` (F4) killed prose invariants for the right reason: *"A string cannot be
checked against the model, so the invariant about invariants cannot hold."* The fix — typed
predicates reusing `aep_domain::predicate` — shipped, and is one of the better decisions in the
tree.

Authorisation now occupies the position prose invariants used to.

- `crates/ess-domain/src/actor.rs:55` — `ActorSpec.may: BTreeSet<QualifiedName>`; `:62-67` —
  `may_invoke(command)`. A machine-checkable authorisation statement, keyed on identity so *"renaming
  a command's wire form … cannot silently move a permission"*.
- The module doc anticipates the projection precisely (`actor.rs:14-22`): a generator may compile
  `may` into an RBAC binding, a permission matrix, an OpenAPI security requirement **"or a test that
  asserts a refusal"**.
- The normative fixture already contains the negative case:
  `examples/billing/domains/invoice.yaml:113-119` — `Customer` may only `CreateInvoice`, and
  `Auditor` is declared with **no** `may` at all.
- Neither new design generates it. Wave 4's fault matrix (§25, `:953-961`) has no authorisation
  fault; §50's acceptance criteria never mention actors; `SemanticCommandRequest` carries
  `actor: Option<ActorRef>` (§9, `:348-354`) and nothing asserts on it.
- Verified gap in the model: no authorisation-refusal vocabulary exists. Grep
  `unauthor|forbidden|denied|permission` over `crates/ess-domain/src/*.rs` returns only doc comments
  and test names; `CommandSpec` has outcomes and declared errors, with no notion of a refusal caused
  by *who* invoked it.

**So by wave 4's own rule, the oracle must refuse to check `may:`** — §18 (`:682-707`): *"If the ESS
does not yet define an observable representation of `escalate`, scenario synthesis must refuse that
check rather than invent one."* Applied consistently, that means an implementation in which any
actor may issue any command **passes conformance**, and the specification's authorisation statements
are certified by silence. For a project whose design devotes §97 to least privilege and whose
protocol half treats default-deny as its headline safety property, that is the wrong hole to have.

Credit where due: tag `0.3.2-ess-wave-3` already records the adjacent honesty — *"`may:` is
published as an annotation rather than as a security scheme, because a scheme would describe an
authentication mechanism the specification says nothing about."* That reasoning is correct. The
finding is that annotation is where it stops, and the oracle is about to inherit the gap.

**Cost now:** a declared refusal form plus one generated scenario per (actor, command) pair outside
`may` — cheap while the format is `ess/1`. **Cost later:** a format-version change, and every
conformance report issued in between overstates what it checked.

---

## V8 — Wave 5 §28 makes the synthesis planner a capability-granting authority

**Severity: medium.**

`ess-structural-…-v0.1.md:1203-1231` (§28, "Capability Derivation") has obligation kinds "inform
safe capability defaults", listing grants: `repository.read`, `repository.write`, `tests.execute`,
and `repository.*`.

This contradicts a shipped rule that is commented in three places:

- `crates/aep-engine/src/resolve.rs:13` — *"capabilities: profile grants, principles restrict, task
  restricts."*
- `crates/aep-domain/src/capability.rs:19` — *"Deny wins unconditionally: a profile cannot grant back
  something a principle denied."*
- `AGENTS.md` invariant 6 — *"A principle may restrict; only a profile or protocol may grant."*

A synthesis planner that derives grants is a fourth authority in a composition designed to have
exactly one, and it is the one no document is responsible for. The repository's own resolution
machinery records, for every capability entry, *the document responsible for it* (README: "capabilities
composed with the document responsible recorded for every entry") — a planner-derived grant has no
document to name.

Second, smaller problem: `repository.*` is not a spelling this vocabulary accepts.
`crates/aep-domain/src/capability.rs:238` — *"Environment wildcards are the only widening."*

**The fix keeps the intent and drops the contradiction:** express §28 as *profile selection* — an
obligation kind names an existing profile that already grants what that class of work needs. The
mechanism exists (`profiles/development-{fast,standard,critical}.yaml`), the grant stays attributable
to a document, and the deny floor still cannot be lifted. Doc-only change.

---

## V9 — Does ESS belong in this repository? Yes, but the join is thinner than the diagram

**Severity: medium — an assessment, argued both ways, which is what question 2 asked for.**

### The case for "two products sharing a git remote"

- **The dependency edges are one-way and shallow.** `ess-*` depend on `aep-domain`; `aep-domain`
  knows ESS through exactly one artifact-kind variant (`crates/aep-domain/src/artifact.rs:385`) and
  one evidence variant (`evidence.rs:673-698`, `:830`). Nothing else.
- **What ESS imports is a validation toolkit, not a methodology.** Measured import sites across
  `ess-domain`, `ess-compiler` and `ess-gen`: `error::{ValidationCode, ValidationError,
  ValidationErrors, ParseError}` 18, `predicate::Predicate` 5, `node::Node` 4, `facts::FactPath` 3,
  `entity::{EntityLocator, EntityType}` 2, `protocol::is_supported_major` 1. Not one import touches
  workflows, principles, capabilities, obligations, evidence or audit. Everything ESS reuses would
  sit as comfortably in a crate called `spec-kit`.
- **Reach into practice is minimal**, per V4: one profile of five, no example task, no
  `.engineering/`.
- **The naming is asymmetric, against the project's own recorded reasoning.**
  `ArtifactKind::entity_type()` (`artifact.rs:530-533`) stamps every kind `aep.<kind>/v1`, so an
  executable system specification becomes `aep.executable-system-specification/v1`. Recorded
  deviation 8 (`reconciliation-v0.2.md` §5.8) refused to make exactly that claim for ADP's own types,
  on the grounds that *"a base-protocol name would claim the base protocol defines that shape"*. AEP
  demonstrably does not define ESS's shape — `docs/VISION.md` says so twice ("AEP does not know what
  an invoice is; ESS does not know what a code review is") — and yet the type name says it does.

### The case for a real join

- **It is tested at the point that matters**, including the discrimination the vision is built on:
  `crates/aep-engine/tests/end_to_end.rs:250-334` — with a specification in the graph, conformance is
  owed; an agent's own report leaves it owed; a `ConformanceRunner`'s closes it. And `:336-352`
  proves the negative: a task without a specification owes nothing, so the rule does not fire where
  it does not apply.
- **Reuse prevented two genuine duplications.** Adopting `aep_domain::predicate` (review F4) avoided
  a second expression language with its own three-valued semantics and its own minimal-cause
  explanations — the review's own words: *"it took a while to get right."* Adopting `ep://` (F5)
  avoided a translation table between two locator schemes.
- **Conceptually the two halves are one predicate.** "Was this built properly" and "is this the thing
  we meant" are both conditions on the same completion decision, and a task holding only one of them
  is under-specified. A repository that owned only the first would keep re-inventing the second in
  every profile.

### Verdict

Keep it, and **stop growing the type surface of the join**. The coupling is one artifact kind, one
evidence kind, one principle and a validation toolkit — which is the right amount, and the right
direction (ESS depends on AEP; AEP stays ignorant). What would make it more than a shared remote is
not more types: it is **one task in this repository governed by both halves at once**, which is
precisely V4's unstarted milestone. Questions 2, 3 and 7 converge on the same recommendation, which
is the most useful thing this review found.

Two cheap consequences: stop adding AEP artifact kinds and relations on ESS's behalf (V11), and fix
`entity_type()` for ESS-owned kinds while there are no stored entities to migrate.

---

## V10 — The oracle requires a forced-outcome backdoor in every conformant implementation

**Severity: medium.**

`ess-closed-loop-…-v0.1.md:464-501` (§12) requires the conformance target to expose
`configure_external_outcome` / `ExternalOutcomeControl { command, force_outcome }`, and suggests
*"swapping a test double; controlling an external fake; setting a fixture; injecting a port
implementation."* §8 (`:317-341`) additionally requires scenario isolation, offering "tenant
namespace" and "temporary schema" as mechanisms.

**Consequence for a real person.** §6 (`:231-263`) claims broadly that *"A Rust service, Java
service, serverless implementation, modular monolith, or distributed system may all be conformant."*
What is actually being claimed is narrower: *any implementation that ships a switch which forces
declared domain outcomes and can reset its own state per scenario*. In a production binary, that
switch is a privilege-escalation primitive — force `SendEmail` to report `sent`, force
`CreateInvoice` to `accepted`. No section of the design mentions the risk, requires the surface to be
compiled out of release builds, or scopes it to a test profile.

The billing fixture makes this non-optional rather than hypothetical: `examples/billing/system.yaml`
advertises *"a command with an outcome its input cannot decide"*, and
`crates/ess-domain/src/command.rs:198-208` (`OutcomeCondition::External`) exists for it, so full
conformance cannot be reached without the control surface.

**Cheapest honest fix, using machinery the design already has:** §28 (`:1068-1106`) already defines
an `unsupported` scenario status and the rule that an unsupported *required* scenario fails overall
conformance. State that the control surface is a test-build capability; a target that will not expose
it reports those scenarios `unsupported` and gets a conformance result that says so, rather than
being required to ship the switch. That also preserves the design's good instinct at §28 —
*"Do not silently skip required semantics."*

---

## V11 — Two obligation vocabularies in one commit, and invented relation names

**Severity: low-med.**

- `ess-closed-loop-…-v0.1.md:1727-1738` (§46) defines `obligation: {id, kind: implementation,
  contract: {input, output}, required_by}`.
- `ess-structural-…-v0.1.md:653-729` (§13, §14) defines `type: ess.implementation-obligation/v1`
  with `reason.kind: external-effect`, plus an `ObligationKind` enum of seven variants.

Two incompatible spellings of one concept, committed together in `3647f80`. The concept belongs to
wave 5; §46 in the wave-4 doc is a preview that will be cited as a specification.

- `ess-structural-…-v0.1.md:1007-1019` (§22) names relations `derives` and `satisfies`.
  `crates/aep-domain/src/artifact.rs:900-914` already has `DerivedFrom`, `Implements`, `Verifies`,
  `Specifies`, `Delivers`, `Decides`, `Reviews`, `Blocks`, `DependsOn`, `Supersedes`, `Decomposes`,
  `InformedBy`. The two new names duplicate existing ones — which is the translation table review F5
  (`ess-review-v0.1.md:114-123`) told the project not to build, in the same paragraph that fixed the
  locator schemes.

**Fix:** delete §46 from the wave-4 doc; map §22 onto existing relation kinds. Minutes.

---

## V12 — Fact spellings wave 5 uses that the engine cannot evaluate

**Severity: low.**

- `ess-structural-…-v0.1.md:1050-1055` (§23) — completion `require: [obligation_tests_passed,
  ess_conformance_passed]`.
- `:1364-1370` (§32) — `realization.ess_conformance == passed`,
  `realization.obligations_unresolved == 0`, `realization.build_reproducible == true`.

The shipped projection is `ess_conformance.{status, passed, spec_version, scenarios.total,
scenarios.failed}` (`crates/aep-domain/src/evidence.rs:1377-1398`), and `AGENTS.md` § Conventions
states outright that the `<claim>_verified` shape *"is projected but not observable"* — no protocol
declares the bare namespace, so a predicate cannot read it. `realization.*` is an undeclared
namespace entirely, and `build_reproducible` names a claim no verifier in the vocabulary produces.

Low severity because it is a documentation defect — but it is the kind that gets copied into a
principle document by an implementor who trusts the design over the code, and then fails at
resolution time with a confusing message.

---

## V13 — `async_trait` re-decides a question `aep-contract` settled deliberately

**Severity: low.**

`ess-closed-loop-…-v0.1.md:272-309` (§7) writes `#[async_trait] pub trait ConformanceTarget`.

`crates/aep-contract/src/testing.rs:1-9` records the opposite decision with its reason: *"A
specification crate should not choose anyone's async runtime. The traits here use `async fn` because
the contract has to be implementable by a backend that talks to a network; a backend that talks to a
`BTreeMap` implements them with futures that complete on the first poll, and this drives one to
completion without a dependency."* `aep-contract/src/command.rs:286` uses
`impl std::future::Future`, and the whole workspace has **seven** external dependencies and no async
runtime (`Cargo.toml` `[workspace.dependencies]`).

Credit where due: the same design draws the determinism boundary correctly at §37 (`:1428-1456`) —
*"Execution reports may contain timestamps. Generated definitions may not."* That is exactly right and
resolves the concern I expected to raise. What remains is the dependency, and the unremarked fact
that §15's `timeout: 5s` / `poll_interval: 25ms` (`:593-618`) introduces the first wall clock into a
workspace whose invariant 8 is "the domain crate is clock-free". The boundary is defensible; it
should be stated where invariant 8 is stated.

---

## V14 — The vision document is the one stale status claim in the tree

**Severity: low, but badly placed.**

- `docs/VISION.md:101` — *"Honest as of `0.2.1`."*
- `docs/VISION.md:109` — *"ESS — everything | specified, not built."*

Three ESS waves have shipped since `0.2.1` (`0.3.0-ess-wave-1`, `0.3.1-ess-wave-2`,
`0.3.2-ess-wave-3`), and ESS is now 30% of the codebase. `README.md`'s status table is accurate;
`AGENTS.md` § "Current state" binds only the README (*"keep the status table accurate when you land
work"*), and `grep -n VISION AGENTS.md` returns **0 hits**. The vision has no maintenance rule, so it
is the only document in the tree making a false status claim — in the document an outsider reads
first, and the one that says "Honest as of".

Second instance, same class: `docs/plan/ess-roadmap.md:21` — *"Rust structural synthesis (design §32
phase 6) is deliberately outside this roadmap — it is wave 4 at the earliest"* — contradicts the
table nine lines above it, which maps wave 5 to design phase 6, and contradicts the page's own
§ "ESS wave 5 — structural synthesis".

**Fix:** one line in `AGENTS.md` adding VISION's status table to the per-wave checklist, and delete
the stale sentence in the roadmap. Minutes.

---

## V15 — Over-built for the stage, and the one under-build that gets expensive

**Severity: note. Question 6, with numbers.**

### Paying abstraction cost for a requirement that has not arrived

- **`aop-domain`** — 2,236 lines, 49 tests, a typed vocabulary for incidents and releases, for a
  protocol half that has never executed anything. Its own recorded deviation
  (`reconciliation-v0.2.md` §5.9) states that `LifecycleDescriptor` *cannot express* the operations
  ladders, so it publishes `lifecycle: None` and exposes its transitions as free functions. A typed
  vocabulary whose central type does not fit the domain it was written for, built because §4.3 named
  it. Deferring it would have cost approximately nothing (the README weights it at 5%). The recorded
  deviation is the right response to the situation; the situation was avoidable.
- **`aep-conformance`** — 5,012 lines, 16 suites, **three** conformance levels, serving exactly one
  backend (in-memory) and zero external adopters. The suites themselves are the best-justified code
  in the tree: they caught real defects, and the deliberately-broken backend is the correct
  discipline. The *level system* is the over-build — three certification tiers calibrated for a
  market of third-party backends that does not exist, and cannot exist before the vision's own
  dogfooding milestone.
- **Four projections, 27 committed artifacts, one example specification, no consumer.** The
  projections earn their keep as completeness checks on the model (the wave plan argues this well, and
  V1's instance 1 proves the argument). The count is nonetheless ahead of demand.

Common shape: each is a faithful implementation of a design section, built because the section
existed. For a pre-1.0 repository with no external users, "the design names it" is a weaker reason
than "something needs it", and `reconciliation-v0.2.md` §5 is the right place to record a *deferral*
as well as a deviation.

### Under-built, and expensive to retrofit

**Specification evolution.** `docs/plan/ess-roadmap.md` § "What is not in these five waves" places
`ess diff` compatibility classification outside all five waves. But wave 4's evidence and wave 5's
`contract_digest` invalidation (§29, `:1235-1260`) both require an answer to *"is this specification
change breaking?"* — and generated artifacts are already committed under `generated/` and
drift-checked, so every specification edit already needs classifying by hand today.

This is the same problem `FreshnessPolicy` and `ReviewResult::covers` solved for designs, unsolved
for the artifact the project is making authoritative over contracts, tests and eventually code.
Retrofitting it later touches the evidence type (V3) and the drift-check contract at the same time,
which is why V3's digest is worth doing now: it is the cheap half of this, and it is the half that
prevents silently stale conformance claims in the meantime.

**Not a finding, deliberately:** the absent durable backend is *correctly* unbuilt. `aep-contract`
plus 16 black-box suites is exactly the shape that makes that retrofit cheap, and choosing to defer
it while building the contract first is the best sequencing decision in the repository.

---

## Load-bearing assumptions, and what would falsify each

| # | Assumption | Falsifying observation | Status |
|---|---|---|---|
| A1 | A harness reports producers honestly, and restricts its tools to the granted capabilities | Instrument one real harness: count submissions whose claimed `Producer` differs from the process that produced the bytes, and tool calls made without a prior `authorize`. One counterexample falsifies `independent: true` as a guarantee. | untested. V2 |
| A2 | Real engineering work fits the workflow, principle and capability vocabulary | Write `.engineering/` for this repository, resolve the last four waves against the evidence that exists, and count rules that cannot be expressed plus rules that fire when they should not. `wave-4-dogfooding.md:41` already names this the interesting output. | untested, and needs no new code. V4 |
| A3 | Each stated invariant is held by the mechanism the design implies | Mutate the guard and re-run the gate. Already falsified in three places by the concurrent mutation reviewer (verified by another agent), and once in wave 3 by `tests/agreement.rs`. | **falsified**, partially. V1 |
| A4 | A specification can be complete enough that its derived suite bites | Wave 4's fault matrix. Falsified if a fault exists that no generated scenario catches, or if a scenario fails for the wrong reason. | wave 4 tests it — and this is the best feature of that design |
| A5 | Generated suites are genuinely transport-independent | Wave 5 §34's two realizations. Falsified if the suite needs one line of change between in-process and HTTP+Kafka. | wave 5 tests it |
| A6 | A model constrained to residual obligations outperforms one given the repository | Two arms against the billing obligations; compare conformance pass rate and iterations to green. | untested and unplanned. V6 |
| A7 | Determinism holds outside one toolchain | Regenerate under a different Rust minor and a `schemars` bump. CI pins `dtolnay/rust-toolchain@stable` on `ubuntu-latest` with no matrix (`.github/workflows/ci.yml:23-72`), and the declared MSRV `rust-version = "1.85"` is never exercised. | untested; cheap to add |

A3 is the one worth dwelling on: it is the only assumption on this list that has already been
falsified, it was falsified twice in one day by two independent methods, and it is the assumption the
project's confidence is drawn from.

---

## What holds up well

A review that only lists problems cannot be calibrated. These are the parts I would not change, and
several I would copy.

- **The three signature decisions are genuinely stronger than prose, and each is a fact rather than a
  reminder.** Submission order (`evidence.first_seq.test_result < evidence.first_seq.diff`) turns
  "write the test first" into an observation. `ReviewResult::covers`
  (`crates/aep-domain/src/review.rs:243-260`, tested at `:294-305`) turns approval freshness into a
  computation. Three-valued truth (`docs/guide/harness.md:151-181`) distinguishes "nobody looked"
  from "it is broken" and prescribes a different next action for each. These are the parts a
  competent team would otherwise re-argue every quarter, and they are settled here.
- **Refusal is a first-class output, which is rare.** Denials become audit records
  (`harness.md:229`). A `rollback` with no precondition is rejected at validation time. An empty
  test suite is `inconclusive`, not green (`reconciliation-v0.2.md` §5.6). A configuration containing
  a rule that could never fire is refused outright. That last one is a genuine feedback loop from the
  document layer back to its author, and most policy systems have nothing like it.
- **The deliberate-deviation register is the right institution, and it is used honestly.** Twelve
  entries, each with the reason. Entries 9, 10 and 12 record places the model does *not* fit — which
  is what makes the other nine believable. This is the mechanism V1 asks to be extended, not
  invented.
- **The ESS model out-designs its own design documents.** `OutcomeCondition`, `TestStrategy`,
  `Consistency::assertion_style` and `External.cause` each solve a generator problem once, in the
  model, with the reasoning written on the type — *"where it can be wrong once instead of once per
  target"* (`command.rs:226-228`). Conceptually this is the strongest code in the repository, and it
  is why V5 is a finding at all.
- **Wave 3 caught its own worst defect and generalised the fix.** The type-mapping divergence
  (`docs/plan/ess-wave-3-projections.md:147-163`) was found, recorded in a section honestly headed
  *"The third place, which this page did not predict"*, fixed by removing the copies, and locked by a
  test that classifies every keyword as an assertion or an annotation *and fails on a keyword neither
  list names* — so the next addition cannot slip in unclassified. That last detail is the difference
  between fixing a bug and closing a class of bug.
- **Honesty is load-bearing and consistent, and it is the reason this review can be short.** README
  § "What does not work yet" names the unvalidated OpenAPI and AsyncAPI envelopes. The `0.3.2` tag
  message carries a section headed *"Not met, in writing"*. CHANGELOG has `### Not built` sections.
  Tag messages read as a project history. I did not find a single overstatement in the README or the
  CHANGELOG — every one of my findings is either in a design document, in the vision, or in the gap
  between an architecture's intent and its enforcement. For a repository eleven hours old that is
  remarkable.
- **Dependency discipline.** Seven external crates for 74,690 lines, no async runtime, no `unsafe`,
  and a nine-line `block_on` with a written justification instead of tokio in a specification crate.
- **The join is tested at exactly the point that matters** (`end_to_end.rs:250-352`): an agent's own
  conformance claim leaves the requirement owed, a runner's closes it, and a task without a
  specification owes nothing.
- **The ordering rule for waves is the best single sentence in the planning tree**:
  *"each wave must be falsifiable by the one before it. A generated artifact nothing can check is a
  claim, not a deliverable"* (`docs/plan/ess-roadmap.md`). Wave 4 before wave 5 is right for exactly
  this reason, and I would not reorder them.

---

## Decisions for Timo

| # | Decision | Options | Cost of each | Default if nobody answers |
|---|---|---|---|---|
| 1 | Does an invariant get to be called an invariant when a test holds it? | (a) add a mechanism column to `AGENTS.md`'s sixteen — *type / refusal / test / convention* — and classify honestly; (b) leave the flat list | (a) an afternoon, and it is mechanically checkable; (b) free now, and the confidence stays unearned in a subset nobody can name | **(a)** — it is the cheapest finding here and it is what makes the other reviewers' results interpretable |
| 2 | Fix the vision's trust claim, or fix the architecture | (a) reword `docs/VISION.md` to claim only what the protocol can check; (b) add a submitter identity distinct from the claimed producer and refuse self-submitted independent evidence; (c) both | (a) minutes; (b) half a day and a public-API change, far cheaper before an external harness exists | **(c)** — (a) today, (b) before anyone else's harness |
| 3 | Bind ESS conformance evidence to the specification it checked | (a) add `spec_digest` to `EssConformanceResult`, project it, and require the match in `principles/verification/ess-conformance.yaml`; (b) leave it | (a) small now, and it is the cheap half of specification evolution; (b) a schema migration plus permanently unattributable records | **(a)** |
| 4 | Next wave: dogfooding or the ESS oracle | (a) finish W4.2/W4.3 — `.engineering/` for this repository; (b) ESS wave 4; (c) both, dogfooding first | (a) days, and it can only produce findings — which is the point; (b) about a wave; (c) sequencing cost only | **(c), (a) first** — the vision names it as the next honest milestone, it needs no new code, and it is the only test of the assumption V1 just falsified |
| 5 | What to do with the two unreviewed designs | (a) merge as-is; (b) add a reconciliation header per doc naming what shipped (`TestStrategy`, `OutcomeCondition`, `assertion_style`, `impl Future`), delete wave 4 §46, fix §28 and the fact spellings; (c) rewrite against HEAD | (a) free now, a fourth one-rule-two-implementations defect later; (b) an hour; (c) a day | **(b)** |
| 6 | Authorisation in ESS | (a) declare a refusal form and generate a negative scenario per (actor, command) outside `may`; (b) say in the README that `may:` is documentation and the oracle does not check it | (a) a model addition, cheap at `ess/1`; (b) free, but conformance reports overstate what they cover | **(b) now, (a) before wave 4 ships** — silence must not certify an authorisation rule |
| 7 | Measure wave 5's premise before building eight waves on it | (a) fold the two-arm experiment into wave 4's acceptance criteria; (b) proceed on the assumption | (a) a day of design, and the oracle is the instrument; (b) the first honest measurement arrives after the investment | **(a)** |
| 8 | Roadmap and vision hygiene | (a) wire W5–W12 into `ess-roadmap.md`, delete the self-contradicting line 21, add VISION's status table to the per-wave checklist in `AGENTS.md`; (b) leave | (a) minutes; (b) the vision stays the only false status claim in the tree | **(a)** |

---

## Method notes

- Judged against the project's own stated goals first (`docs/VISION.md`,
  `consolidated-design-v0.2.md` §106–107, the roadmap's falsifiability rule), then against outside
  practice; each finding says which. The one outside-practice comparison worth naming: V2's
  submitter/producer split and `Provenance.digest` are one signature short of the shape
  in-toto/SLSA settled on for exactly this problem, which is a reason to believe the gap is closable
  rather than a reason to redesign.
- Verified versus inferred is labelled per claim. V6 is explicitly a hypothesis about model
  behaviour, not a finding about code. V1's instance 2 is attributed to the concurrent
  mutation-testing reviewer and was not re-derived here.
- Recorded deviations are engaged with rather than relitigated: §5.8 in V9, §5.9 in V15, §5.5–5.6 in
  "What holds up well".
- No overlap with `docs/reviews/2026-08-20-full-repo-review.md`, which is referenced only as the
  precedent V5 compares against.
- Not covered, by scope: code correctness, gate health, and the two subjects owned by the concurrent
  reviewers.
