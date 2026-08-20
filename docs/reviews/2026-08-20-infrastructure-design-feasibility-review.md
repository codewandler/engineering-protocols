# Feasibility review — the infrastructure design against the repository and the vision's boundary

Reviewed at `34cac07` (`main`). Subject:
[`semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md`](../design/semantic-infrastructure-discovery-specification-conformance-multicloud-design-v0.1.md)
— 4,002 lines, 100,559 bytes, 103 numbered sections, filed today, never reviewed.
[`AGENTS.md`](../../AGENTS.md) § *Which documents are normative* records it as "**not reviewed at all.**
Unsequenced".

Read-only. Nothing outside this file was changed. Section numbers prefixed **§** refer to the design;
`docs/design/…:N` and `crates/…:N` are `file:line`.

**On line numbers.** Code citations are taken from `git show 34cac07:<path>` — the committed state —
because the working tree is being edited concurrently by an implementation agent (`git status`:
77 modified files across `aep-domain`, `aep-engine`, `ess-domain`, `ess-compiler`, `ess-gen`,
`examples/billing`, `generated`). Where the working tree already differs in a way that matters, it is
flagged inline.

---

## Verdict

**Buildable in principle; not here, not now; and one structural section contradicts a boundary
`docs/VISION.md` restated today.** The design is unusually careful — it separates observation from
intent, keeps `Unknown` first class, refuses to mint an acronym, and its ESS-side example is the
existing model quoted verbatim rather than a replacement for it. It also proposes, in §80, that
`infra-discovery`, `infra-target-aws` and `infra-target-azure` be workspace crates. An adapter that
satisfies its own feasibility argument (§61: AWS Cloud Control, Azure Resource Graph, GCP Cloud Asset
Inventory, Kubernetes discovery API, Hubble) calls a cloud API and holds a credential, and
`docs/VISION.md:160` says "nothing here calls a cloud API, holds a credential, applies a plan or
watches a rollout." The design argues against itself here: §2.11 and §92.7 both say external systems
act and this repository decides — §80 then puts the actor inside the workspace. **The design should
move, not the boundary**, and the move is cheap: adapters become external verifiers, and
`protocols/aep/1.yaml:57-68` already has the `verifiers:` list to name them in.

On scope: §77 lists **36 numbered work items across six tracks**, and §102 sequences all of them
behind W4–W7, of which **zero are built**. Measured against the last three waves (ESS 1: 16,096
insertions / +200 tests; ESS 2: 10,661 / +135; ESS 3: 14,283 / +139 —
`docs/reviews/2026-08-20-next-waves-feasibility-review.md:44-46`; my own crates-only
`git diff --shortstat`: 12,161 / 9,641 / 11,981), each of which did exactly one *kind* of thing, this
is **roughly eleven waves** (inferred, method in I2). ESS took three waves to carry one example from a
model to four projections and still has no oracle: wave 4 is blocked on three open gates
(`docs/plan/ess-wave-3.5-reconciliation.md:29-49`, 15 of 19 closed, G2/G15/G16 outstanding). A second
subject matter cannot start before the first one closes its loop.

One correction to the framing this review was commissioned under: **the design does not re-scope an
existing wave, because no such wave exists.** Detail in I3.

**Recommendation: defer the whole document as a horizon, and harvest two ideas now** — §17's
required/permitted/observed relation classes, and §13's freshness question, which exposes a real
tension in the current model and has a cheaper answer than the design proposes. Full options and costs
in *Decisions for Timo*.

---

## Findings

| # | severity | finding | design § | evidence |
|---|---|---|---|---|
| I1 | **critical** | §80's workspace crates cross `docs/VISION.md`'s "nothing here calls a cloud API" boundary; §2.11 and §92.7 contradict §80 | §80, §81, §61 | `docs/VISION.md:157-170`; design:3278, 3294-3296, 3305 |
| I2 | **critical** | ~11 waves of work, sequenced behind 4 unbuilt waves, on a repo whose first subject matter has no oracle | §77, §102 | `docs/plan/ess-wave-3.5-reconciliation.md:29-49`; wave sizes below |
| I3 | high | The header says it "refines the planned ESS topology-synthesis boundary". There is no such plan — W8 is one line in an unaccepted document, and the roadmap puts topology generation outside all five waves | header:6, §9, §79 | `…structural-synthesis…:1821`; `docs/plan/ess-roadmap.md:282` |
| I4 | high | `InfraSpec`/`InfraIr` is a **fork** of the ESS pattern, not a second instance: no `Raw*`, no `TryFrom`, no `ValidationErrors`, no `ValidationCode`. `infra-domain` would sit outside invariants 2, 3 and 4 on day one | §21, §22, §81 | `crates/aep-domain/tests/invariants.rs`; design:1663, 1715, 3310 |
| I5 | high | The existing topology model **survives unchanged** — but the six requirement fields §8.3 needs do not exist, so BR2 cannot extract them without guessing, which design:96 forbids | §8.1, §8.3 | `crates/ess-domain/src/topology.rs:86-123`; design:1052-1075 |
| I6 | high | `ClaimStatus` conflates three concepts the repository already types separately (`Producer`, evidence-set agreement, `Truth`); the normalization step sits against invariant 7, which nothing enforces | §12, §2.7, §26 | `crates/aep-domain/src/evidence.rs:714-740`; `predicate.rs:56`; design:1312 |
| I7 | high | §13 introduces the first wall-clock-dependent predicate in the repository. Invariants 8 and 9 both turn on this, and §72 gets close without closing it. **The repository today compares no two timestamps anywhere** | §13, §72 | `crates/aep-domain/src/time.rs:3-6`; `artifact.rs:1175-1185`; design:1362-1366, 3059-3065 |
| I8 | medium | Discovery costs the "nothing in `task check` reaches the network" property, which was defended by an explicit decision three days ago. §85's acceptance criterion reads as a live account | §61, §85, §70 | `AGENTS.md:226`; `docs/plan/ess-wave-3-projections.md:180`; design:3450 |
| I9 | medium | Six of the eight capabilities §58 proposes already exist, and every write it contemplates is already behind the approval floor | §58 | `protocols/aep/1.yaml:14-39` |
| I10 | medium | §48's eight artifact kinds against 26 that exist — but the restraint is stated correctly and `Other(String)` means none is blocking | §48 | `crates/aep-domain/src/artifact.rs:328-388`; design:2472-2474 |
| I11 | low | §29's change engine depends on a design that is itself unreviewed, which depends on wave 4, which is gated. Three levels of unbuilt | §29, §96 | `AGENTS.md` § normative table |

---

## I1 — §80 puts the actor inside the workspace (critical)

**Verified.**

`docs/VISION.md:151-170` § *What this is deliberately not*, restated today and naming this design by
name:

> **Not a deployment platform**, and the infrastructure design does not change that — it makes the
> line worth drawing precisely. Generating an artifact is in scope: this project may compile a
> specification into the file that describes an infrastructure, and decide whether an infrastructure's
> observed state conforms to what was specified. *Operating* a system is not: nothing here calls a
> cloud API, holds a credential, applies a plan or watches a rollout.
> […]
> External systems do the work. This project decides what the results permit.

The design agrees with that in prose, twice:

* §2.11 (design:194-198): "Independent scanners, compilers, planners, and verifiers remain
  authoritative for facts and acceptance."
* §92.7 (design:3568-3570), *Building an orchestrator by accident*: "Mitigation: external cloud/IaC/
  deployment systems perform actions; AEP/AOP decide what is permitted and what evidence is required."

And then contradicts it in structure. §80 (design:3264-3299) lists as "potential future crates":

```text
infra-discovery
    adapter traits
    snapshot normalization
    identity correlation
…
infra-gen-kubernetes
infra-target-aws
infra-target-azure
```

§81 (design:3305-3307) gives the trait:

```rust
pub trait ObservationAdapter {
    fn observe(&self, scope: &ObservationScope) -> Result<Vec<ProviderObservation>, ObservationError>;
}
```

`observe` against the sources §61 (design:2776-2806) names — AWS Cloud Control, AWS Config, VPC Flow
Logs, Azure Resource Graph, GCP Cloud Asset Inventory, the Kubernetes discovery API, Cilium Hubble —
is a network call under a credential. §10.2 (design:1179-1181) says "Discovery should be read-only by
default" and "Scanning production is an observation operation", which addresses *what* the call does,
not *who makes it*. Read-only is not the boundary `VISION.md` drew; **making the call at all** is.

### What each side costs

| move | cost |
|---|---|
| **the boundary moves** — adapters are workspace members | `AGENTS.md:226` ("Nothing in `task check` reaches the network") stops being true, or discovery ships untested in the gate. Direct third-party dependencies go from nine (`AGENTS.md:214`, `Cargo.toml:40-46`) to nine plus an AWS SDK, a Kubernetes client and a TLS backend — I did not measure the transitive count and am not guessing at it, but `unsafe_code = "forbid"` (`Cargo.toml:50`) binds only the thirteen workspace members and says nothing about what they pull in. Credential handling, rate limits and retry enter a repository that currently has no I/O beyond reading files. `docs/VISION.md:157-170` has to be rewritten. |
| **the design moves** — adapters are external verifiers | Zero. `protocols/aep/1.yaml:57-68` already declares a `verifiers:` list (`compiler`, `test-runner`, `telemetry-query`, `policy-engine`, …); `telemetry-query` is precisely this shape — something outside the repository that observes a running system and hands back a fact. The repository defines `ProviderObservation` and `ObservedInfraSnapshot` as **input schemas** and a normalization/conformance path over them. Everything the design wants from §12 onward still works. §50 (design:2519) already proposes exactly this: "a new evidence kind such as `infra_conformance` could mirror `ess_conformance`, with an independent infrastructure conformance runner." |

The second is also what the repository already does for its *own* half: `ess-gen` emits a suite;
something else runs it; the result arrives as `EvidenceKind::EssConformance`
(`crates/aep-domain/src/evidence.rs:830`) with `independent: true` demanded mechanically
(`principles/verification/ess-conformance.yaml:31-36`). Nothing in the ESS loop requires this
repository to run the implementation it judges, and nothing in the infra loop requires it to scan the
estate it judges.

**Recommendation:** keep §12–§30, §33–§46, §59, §60, §71, §84; drop `infra-discovery`,
`infra-target-aws`, `infra-target-azure` from §80 and restate the adapter as an external verifier
producing a documented input schema. This is a one-paragraph edit to the design, not a redesign.

---

## I2 — scope: roughly eleven waves, behind four unbuilt ones (critical)

**Measured, then inferred; the inference is labelled.**

Measured wave sizes:

| wave | insertions (whole repo) | insertions (`crates/` only) | tests |
|---|---|---|---|
| ESS 1 — the model | 18,285 (17,436 excl. `Cargo.lock`) | 12,161 | 442 → 642 (+200) |
| ESS 2 — the compiler | 10,661 | 9,641 | 642 → 777 (+135) |
| ESS 3 — projections | 14,283 | 11,981 | 777 → 916 (+139) |

Commands: `git diff --shortstat 0.2.1 0.3.0-ess-wave-1`, `… 0.3.0-ess-wave-1 0.3.1-ess-wave-2`,
`… 0.3.1-ess-wave-2 0.3.2-ess-wave-3`; test counts from `git tag -n99`. These agree with
`docs/reviews/2026-08-20-next-waves-feasibility-review.md:44-46` (16,096 / 10,661 / 14,283), which
measured wave 1 on a narrower file set.

The shape that matters more than the size: **each wave did one kind of thing.** Wave 1 a model, wave 2
a resolver, wave 3 four projections of one IR. The prior review used exactly this to conclude that the
wave-4 document "as written is three waves" and the wave-5 document "is four waves"
(`docs/reviews/2026-08-20-next-waves-feasibility-review.md:44-56`).

§77 (design:3144-3214) proposes 36 items:

| track | items | kinds of thing |
|---|---|---|
| ID — discovery | 9 (ID1–ID9) | evidence model, 2 inventory adapters, normalization, correlation, projections, 2 flow adapters, diff |
| IS — specification | 6 (IS1–IS6) | domain model, compiler, invariants, conformance, extraction, diff |
| BR — ESS bridge | 4 (BR1–BR4) | requirement model, extraction, satisfaction, cross-layer conformance |
| TG — target generation | 6 (TG1–TG6) | capabilities, plan, 2 targets, falsification, realization |
| MC — portability | 5 (MC1–MC5) | Azure adapter, realizability, profile, cross-target conformance, GCP |
| EV — evolution | 6 (EV1–EV6) | plan, fixture, obligations, transition conformance, migration fixture, AOP execution |

**Inferred** (scaling by the measured one-kind-per-wave shape, not by line count): ID ≈ 3 waves,
IS ≈ 2, BR ≈ 1, TG ≈ 2, MC ≈ 1, EV ≈ 2 → **≈ 11 waves**. I am not estimating insertions; the
adapter tracks have a cost profile this repository has never paid (network, credentials, recorded
fixtures) and I have no measured basis for it.

§102 (design:3902-3946) sequences all of it after W4–W7. Of those:

* **W4** (closed-loop conformance) is designed, reviewed, frozen, and **not started**. It is gated on
  `docs/plan/ess-wave-3.5-reconciliation.md` closing G2, G15 and G16 (`:29-49`; 15 of 19 gates closed).
* **W5** (structural synthesis) is proposed and gated on W4 closing both halves of its loop
  (`docs/plan/ess-roadmap.md:241-249`).
* **W6, W7** exist only in the structural-synthesis design's §43 list and in this design's §78/§102.
  They are not in `docs/plan/ess-roadmap.md`, which stops at wave 5.

So the design is 36 items behind 2 unbuilt waves and 2 waves that have never been written down outside
an unaccepted proposal.

The load-bearing sentence is `docs/VISION.md:147-149`:

> "specified once and compiled" says nothing about a system *changing*, and infrastructure is a second
> subject matter rather than a further projection of the first. Absorbing either of those into the
> thesis is a decision someone has to take deliberately, with a reason.

**Can the repository carry a second subject matter before the first closes its loop? No — and the
reason is measurable rather than aesthetic.** ESS's first subject matter has produced four projections
of one example and **not one verdict**. `docs/VISION.md:129` lists "ESS — the specification as an
oracle, and generated code" as "specified, not built". Every claim in this design about conformance,
drift, counterexamples and falsifiability is a claim about an oracle. Building a second oracle before
the first has ever failed a wrong implementation is the exact mistake `docs/plan/ess-roadmap.md:27`
exists to prevent: "**each wave must be falsifiable by the one before it.**"

---

## I3 — the header re-scopes a wave that does not exist (high)

**Verified, and this is better news than the framing suggests.**

The design's header (design:6):

> **Relationship to existing work:** Additive, but it refines the planned ESS topology-synthesis
> boundary. It must not disrupt the current Wave 4/5 gating sequence.

What "the planned ESS topology-synthesis boundary" actually is:

1. **One line.** `docs/design/ess-structural-synthesis-obligations-realizations-design-v0.1.md:1821`
   — `W8   Topology synthesis` — inside a §43 "Later Roadmap" fenced block of five one-line entries
   (W8 through W12), introduced by "After realizations are proven:". `grep -n 'W8'` over that file
   returns exactly one hit.
2. **In a document nothing has accepted.** `AGENTS.md` § *Which documents are normative* records that
   design as "reviewed by `docs/reviews/2026-08-20-next-waves-feasibility-review.md` and **not
   reconciled**: nothing was folded back in, and that review reads the document as four waves rather
   than one. Unsequenced".
3. **Explicitly outside the roadmap.** `docs/plan/ess-roadmap.md:280-284`, § *What is not in these
   five waves*: "Behavioural synthesis (§32 phase 7), formal verification, **topology generation**,
   `ess diff` compatibility classification, and every transport beyond the one the billing example
   needs. Each is a wave of its own."

Wave 5 is structural synthesis, W5.1–W5.3 (`docs/plan/ess-roadmap.md:241-279`). **W8 is not wave 5's
anything.** There is no planned topology-synthesis boundary to refine — there is a placeholder in an
unaccepted proposal, and a roadmap line saying the topic is out of scope.

Two consequences, opposite in sign:

* **Against the document:** the header's wording is what makes it read as a change to an accepted
  wave. It should be corrected, because "refines the planned X" is how a proposal acquires the
  authority of a plan without a plan page ever accepting it — the precise failure mode `AGENTS.md`
  § *Which documents are normative* was written to stop ("A proposal is not a work order, however long
  and however recent it is").
* **For the document:** §79 (design:3247-3262), *Recommended W8 Revision*, is therefore **the cheapest
  true thing in the whole design.** It costs nothing today, because there is nothing to revise. It is a
  ten-line note in `docs/plan/ess-roadmap.md` saying that when topology synthesis is eventually
  designed, ESS should emit a provider-neutral `InfrastructureRequirementSet` rather than growing a
  Kubernetes ontology. That note is worth taking even if the other 3,990 lines are deferred, because
  it is a constraint on an unwritten wave and constraints are cheapest before the code exists.

---

## I4 — `InfraSpec`/`InfraIr` is a fork of the ESS pattern, not a second instance (high)

**Verified.**

The question posed was: parallel stack, second instance of the same pattern, or fork? It is a **fork**
— the same shape with none of the mechanics that make the shape hold here.

What the ESS pair actually is:

| stage | ESS | enforced by |
|---|---|---|
| document | `RawTopology`, `RawWorkload`, `RawResource` (`crates/ess-domain/src/topology.rs:39, 48, 82`), each `Deserialize` + `deny_unknown_fields` | invariant 2 |
| validated | `Topology`, `Workload`, `Resource` via `TryFrom<RawTopology>` (`:308`), which do **not** implement `Deserialize` | a source scan over ten `Raw*`→validated pairs, `crates/aep-domain/tests/invariants.rs` |
| errors | `ValidationErrors` accumulated, never early-returned; every error carries a `ValidationCode` | invariants 3 and 4; `validation_codes!` in `crates/aep-domain/src/error.rs` generates `ValidationCode::ALL` |
| resolved | `EssIr` (`crates/ess-compiler/src/ir.rs:792`), whose members are **handles**, so a dangling reference is unrepresentable | the type |

What the design specifies:

* §21 (design:1663-1713) gives an illustrative YAML and closes with "The syntax is illustrative. The
  semantic types matter more than YAML shape." No `RawInfraSpec`. No conversion.
* §22 (design:1715-1732) lists nine properties `InfraIr` "should" have — stable identity, resolved
  references, normalized ordering, canonical digests. All correct, all matching `EssIr`. None named as
  a type.
* §81 (design:3310-3312): `pub fn compile_infra_spec(spec: InfraSpec) -> Result<InfraIr, InfraDiagnostics>`
  — **one** error type, undefined, taking an already-constructed `InfraSpec`. So `InfraSpec` is
  constructible without validating, which is the thing invariant 2 exists to make impossible.

Concretely, if built as written, `infra-domain` lands outside four invariants on day one:

| invariant | why it would not cover `infra-domain` |
|---|---|
| 2 — parse, then validate | no `Raw*`→`TryFrom` pair; the scan in `crates/aep-domain/tests/invariants.rs` enumerates ten pairs by name and would not see infra types |
| 3 — validation accumulates | `InfraDiagnostics` is undefined; `AGENTS.md` already records "There is no workspace-wide check: a new validator that returns early passes the gate" |
| 4 — stable `ValidationCode` | not mentioned anywhere in 4,002 lines |
| 11 — lints | a new crate that omits `[lints] workspace = true` is outside every lint; `AGENTS.md` names this failure mode explicitly |

**This is a one-sentence fix in the design, and it should be made before the document is sequenced,
not after.** "`InfraDiagnostics` is `aep_domain::error::ValidationErrors`; `InfraSpec` is obtained only
through `TryFrom<RawInfraSpec>`; codes come from the existing `validation_codes!` macro." The design
already reaches for reuse elsewhere (§25: "Use the same principle as ESS conformance"; §35: "Use the
existing synthesis algebra"; §96: "reuse the previously proposed semantic-change architecture rather
than invent another change engine"). It just does not do it for the part where the repository's
mechanics actually live.

**Is it a parallel stack?** Yes, and legitimately so — §2.2 (design:112-124) argues for two IRs
because "Observation is partial, time-bounded, possibly conflicting, and evidence-backed. Normative
specification is validated intent." That argument is sound and matches how the repository already
separates `Evidence` from `Specification`. The parallelism is not the problem. The missing mechanics
are.

---

## I5 — the existing topology model survives; the requirement fields it needs do not exist (high)

**Verified, including the part the design does not say.**

### The design's ESS example is the repository's own file

§8.1 (design:1003-1013):

```yaml
topology:
  workloads:
    invoice-service:
      replicas:
        min: 2
      stateless: true
      requires:
        - postgres: invoice-store
        - publish: invoice-events
```

`examples/billing/topology.yaml:5-13` — identical, character for character, minus the `email-service`
block below it. So §8 is not proposing an ESS-side model. It is quoting one.

### Does the existing topology model survive, get subsumed, or get contradicted?

| existing type | location | fate under this design |
|---|---|---|
| `Topology { workloads: BTreeMap<ComponentName, Workload> }` | `crates/ess-domain/src/topology.rs:224` | **survives unchanged** |
| `Workload { component, replicas, stateless, requires }` | `:114` | **survives unchanged** — §8.1 names exactly these four |
| `Replicas { min: u32, max: Option<u32> }` | `:95` | **survives unchanged**; §8.2 puts *availability/failure-domain* on the InfraSpec side, not replica counts |
| `Resource { kind: String, name: String }` | `:86` | **survives**, but see below |
| `ResolvedWorkload` | `crates/ess-compiler/src/ir.rs:747` | **survives unchanged** |
| `validate_topology` (workload names an undeclared component) | `crates/ess-domain/src/topology.rs:403` | **survives** — it is a software-side rule and stays software-side |

**Subsumed: nothing. Contradicted: nothing in the types.** §5.7 (design:841-849) explicitly argues
*against* absorbing the estate into ESS: "Putting cloud accounts, IAM policies, VPC routes, Kubernetes
operators, backup vaults, shared clusters, organization hierarchy, and multi-tenant platform resources
directly into ESS would turn ESS into a universal ontology". That is the same reasoning
`crates/ess-domain/src/topology.rs:8-10` gives for the model it has:

> a component is a unit of ownership, a workload is a statement about running it, and conflating them
> is how a domain model turns into a description of a deployment.

Two doc claims stay true under the design and are worth confirming, because they read as though they
might not: `crates/ess-domain/src/topology.rs:4-6` ("nothing in this wave generates one") and
`generated/docs/topology.md` ("None of this is a deployment and nothing generates a manifest from
it"). Under §8.3 the manifest comes from `InfraIr`, not from `Topology`, so both survive verbatim.

### The gap the design does not name

`Resource` (`crates/ess-domain/src/topology.rs:86-91`) is two free strings:

```rust
pub struct Resource {
    pub kind: String,   // `postgres`, `publish`, `cache`
    pub name: String,   // `invoice-store`, `invoice-events`
}
```

Nothing validates `kind` against a vocabulary — `Topology::validate` (`:358`) checks only that neither
string is blank (`:187-218`).

§8.3's requirement contract (design:1052-1075) needs six things `Resource` does not have:

```yaml
requirement:
  id: billing.invoice-store
  kind: relational-database        # ← not `postgres`; a typed family
  engine: postgres
  connectivity:
    exposure: private              # ← does not exist
  availability:
    failure_domains: {minimum: 2}  # ← does not exist
  durability:
    backup: {required: true}       # ← does not exist
  security:
    encryption_at_rest: required   # ← does not exist
```

`kind: postgres` → `kind: relational-database, engine: postgres` is a vocabulary the model does not
have. The other four fields have no representation anywhere in `ess-domain`.

So **BR2** ("ESS topology -> requirement extraction", design:3177) cannot be built against the model
as it stands: it would have to invent exposure, availability, durability and encryption requirements
that the specification never states. The design's own central rule (design:96) forbids exactly that:

> **Never guess. Generate it, create an explicit obligation, record an unknown, or refuse.**

This is a **wave-1-territory model change**, the same shape as the command/transition gap the prior
review found and called "one model change, in wave-1 territory, and it blocks both waves"
(`docs/reviews/2026-08-20-next-waves-feasibility-review.md:20-30`). The design does not mention it.
§93's open question 6 ("What is the minimal `InfrastructureRequirementSet` ESS must emit at W8?",
design:3589) circles it without landing on it.

**Consequence, stated plainly:** every requirement in §8.3's example would arrive as an
`InfrastructureObligation` — "the contract is known but a decision cannot be safely derived" (§35,
design:2114-2116) — because ESS never stated it. That is the *correct* behaviour of the design's own
algebra, and it means BR3 would produce a plan of obligations and no generated capabilities until ESS
grows the vocabulary. Worth knowing before, not after.

---

## I6 — observation duplicates the evidence model rather than reusing it (high)

**Verified for the duplication; the invariant-7 part is labelled a hypothesis.**

### Three status enums against what exists

| design | variants | what the repository already has |
|---|---|---|
| `ClaimStatus` (design:1312-1319) | `Declared, Derived, Observed, Corroborated, Conflicting, Unknown` | — |
| `InfraCheckStatus` (design:1830-1836) | `Passed, Failed, Unknown, ConflictingEvidence, NotApplicable` | `VerificationStatus`; `Truth` (`crates/aep-domain/src/predicate.rs:56`) |
| §2.7 (design:170-176) | `Pass, Fail, Unknown, Conflict, NotApplicable` | same, spelled differently in the same document |

**What is right, and should be kept:** §2.7's rule (design:178) — "A fact that cannot be established is
not a pass" — is invariant 5 arrived at independently. `crates/aep-domain/src/predicate.rs:50-51`:
"`Unknown` means no observation has been made yet; it is distinct from `False`, which means…". The
design got there without being told, which is a good sign about the author's instincts.

**What is wrong:** `ClaimStatus` is three orthogonal concepts in one enum, and two of them already have
types.

| `ClaimStatus` variant | what it actually says | the repository's type for that |
|---|---|---|
| `Declared`, `Derived`, `Observed` | *who produced it* | `Producer::{Agent, Human, Tool, Harness, Verifier}` (`crates/aep-domain/src/evidence.rs:714-740`), plus `Provenance` (`:774-796`) with `command`, `tool`, `revision`, `environment`, `digest`, `inputs` |
| `Corroborated`, `Conflicting` | *how many independent producers agree* — a property of the evidence **set**, not of one claim | nothing, and this is a real gap (see *What this design gets right*, item 3) |
| `Unknown` | a truth value | `Truth::Unknown` (`predicate.rs:62`) |

The repository already makes the first axis mechanically consequential rather than descriptive:
`Producer::is_agent` (`evidence.rs:752`) carries the doc comment "Agent-produced evidence is not
thereby untrustworthy; what it means is that a principle requiring independent verification is not
satisfied by it alone", and `principles/verification/ess-conformance.yaml:31-36` turns that into a
refusal. A `ClaimStatus::Observed` with no producer attached loses that.

`InfraCheckStatus` is closer to legitimate: it is a *report* type, and `EssConformanceResult`
(`evidence.rs:673-698`) is the precedent — it has `status: VerificationStatus` plus counts plus
`failed_scenarios`. The one genuinely new variant is `ConflictingEvidence`, and it earns its place.
But §26's result shape (design:1840-1850) and `EssConformanceResult` should be reconciled explicitly,
because `EssConformanceResult` already carries six of the eight fields §26 asks for
(subject, status, counts, verifier versions, failed items) and the working tree is currently adding a
seventh, a `SpecDigest` binding the result to the specification revision it attests (gate G19,
`docs/plan/ess-wave-3.5-reconciliation.md:47` — in flight, uncommitted at `34cac07`).

### Invariant 7 — hypothesis, labelled

`AGENTS.md` invariant 7: "**The engine never manufactures evidence.** It evaluates what verifiers and
humans produced. *Enforced by* **nothing**."

§81 (design:3308) proposes
`normalize_observations(&[ProviderObservation]) -> Result<ObservedInfraSnapshot, ObservationDiagnostics>`,
and §12 (design:1327-1330) says a normalized claim "may be `Corroborated` while retaining both
evidence paths". So normalization takes two producers' claims and emits a third claim that no producer
made.

**Hypothesis, not a finding:** that is a *derivation* over evidence rather than a manufacture of it,
which is what the engine legitimately does — `EssIr::reactions()` (`crates/ess-compiler/src/ir.rs:834`)
derives a graph nobody wrote, and derived facts already exist in the engine. Whether
`Corroborated` is a derivation or a manufacture depends on whether the corroboration is recomputable
from the retained evidence paths, and the design says it retains them (design:1329). So this is
probably fine. What is *not* fine is that the design never draws the line, and invariant 7 is one of
the three the repository admits nothing checks. If any of this is ever built, "a normalized claim is a
pure function of the evidence set it cites" needs to be a stated invariant with a test, not an
inference a reviewer had to make.

---

## I7 — §13 introduces the first wall-clock predicate in the repository (high)

**Verified, and this is the finding most worth acting on.**

### What the repository's position actually is

`crates/aep-domain/src/time.rs:3-6`:

> The domain crate is deliberately clock-free: a [`Timestamp`] can be constructed from an epoch value
> but never read from the system clock here. Wall-clock access belongs to the engine, behind a `Clock`
> it can swap for a fixed one in tests, which is what makes an execution replayable.

`Clock` is `crates/aep-engine/src/clock.rs:12`, with three implementations: `SystemClock` (`:19`,
the only `SystemTime::now` in the workspace, `:23`), `FixedClock` (`:34`) and `SteppingClock` (`:61`).
`Engine<C: Clock = SystemClock>` (`crates/aep-engine/src/engine.rs:170`); the end-to-end test pins it
(`crates/aep-engine/tests/end_to_end.rs:76`, `FixedClock::new(1_700_000_000_000)`).

**And here is the measurement the design needs and does not have: the repository compares no two
timestamps anywhere, and has no duration type.**

* `Timestamp` (`time.rs:25`) is a `u64` with `from_epoch_millis`/`epoch_millis` and derived
  `PartialOrd`. No arithmetic, no `since`, no `elapsed`.
* `git grep 'Duration' -- crates/` returns three kinds of hit: `SystemClock`'s own `duration_since`
  (`clock.rs:24`), and `Duration` as the *name of an ESS primitive type* in
  `crates/ess-domain/src/types.rs:54` and its projections — a type name in a specification language,
  not a value anything computes with.
* `FreshnessPolicy` (`crates/aep-domain/src/artifact.rs:1175-1185`) has four variants and **not one is
  a duration**: `AlwaysValid`, `UntilSuperseded` (the default), `BoundToRevision`,
  `BoundToDependencySet`. Freshness in this repository is **causal**, never temporal.
* `MetricObservation.window: Option<String>` (`evidence.rs:386`, "The window it was measured over, such
  as `5m`") is free-form text and is compared to nothing.
* "Principles with timed obligations" (`git tag -n99`, `0.1.0`) means *phase*-timed:
  `ResolvedObligation::applies_within(state)` (`crates/aep-domain/src/plan.rs:70`) scopes an obligation
  to a workflow state, not to a duration.

### What §13 proposes

§13 (design:1349-1371), *Time and Freshness Are Semantic*, with three example rules (design:1362-1366):

```text
security-group state <= 15 minutes old
runtime-flow evidence covers >= 24 hours
backup restore evidence <= 90 days old
```

Every one is `now() − observed_at < D`. That is the first predicate in this repository whose value
depends on when it is asked.

### The three consequences, concretely

1. **A verdict changes when nothing changed.** Re-run the same conformance suite over the same
   `ObservedInfraSnapshot` an hour later and `Pass` becomes `Unknown`. Invariant 9 — "Same validated
   state plus same evidence set ⇒ same decision" — is then **false**, unless the evaluation instant is
   part of the evidence set. The gate's `generate-check` step (`cargo xtask generate --check`) has the
   same problem in a worse form: a committed artifact that encodes a freshness verdict goes red
   overnight without a commit.
2. **§72 gets close and does not close it.** §72 (design:3051-3066) asks for "explicit timestamps
   excluded from semantic digest **where appropriate**" and "Observation itself is time-varying;
   processing of the same observation set should not be." The second sentence is exactly right and the
   first is exactly the wording an invariant cannot be held to. What is missing is one sentence: **the
   evaluation instant is an injected input, not a read.** The repository already made this move once,
   in `crates/aep-engine/src/clock.rs`, and gate G12 made it a second time for the conformance runner
   ("a conformance run is reproducible, not merely unslept … the runner's clock and id source are
   injected, as the engine's already are", `docs/plan/ess-wave-3.5-reconciliation.md:47`). The design
   cites neither.
3. **There is a cheaper answer the design does not consider.** `FreshnessPolicy::UntilSuperseded`
   already exists and needs no clock: *evidence is stale when a newer observation of the same scope
   exists.* That answers most of §13 — "is this snapshot the current one for this account/region?" is
   a causal question, and the coverage/scope machinery of §60 (design:2743) already carries the
   information to answer it. Of the three example rules, only `<= 15 minutes old` genuinely needs a
   duration; `covers >= 24 hours` is a property of the observation window itself (a fact about the
   evidence, not about now), and `<= 90 days old` is a policy threshold that belongs above the
   deterministic core by the design's own §2.9 ("Scoring is policy, not infrastructure truth",
   design:186-188).

**Did the design notice? No.** §13 is 23 lines, cites nothing in the repository, and its conclusion
(design:1369) — "A stale observation can produce `Unknown` rather than a false pass" — is correct about
`Unknown` and silent about where `now` comes from. Neither invariant 8 nor invariant 9 appears anywhere
in 4,002 lines. This is the single highest-value correction available, and it is one paragraph.

---

## I8 — what discovery costs the gate (medium)

**Verified for the property; the dependency count is explicitly not measured.**

`AGENTS.md:226-229`:

> **Nothing in `task check` reaches the network.** No step downloads a schema, resolves a remote `$ref`
> or calls an API — `jsonschema` is built with `default-features = false` for exactly that reason. Keep
> it that way: a gate that needs the network is a gate that goes red for reasons that have nothing to
> do with the change.

This is not an aspiration; it is a decision the repository already defended under pressure.
`docs/plan/ess-wave-3-projections.md:180` weighs three ways to validate the OpenAPI/AsyncAPI envelopes
and rejects the network one in a table row: "fetch the meta-schemas in a test | a gate that fails when
someone else's CDN does; nothing in `task check` reaches the network today". It took the option that
left an acceptance criterion **unmet, in writing** (`:183`) rather than reach the network.
`crates/ess-gen/Cargo.toml:21-26` carries the same reasoning in the manifest.

Costs, in order of certainty:

| cost | certainty |
|---|---|
| The property is lost, or discovery is untested in the gate | **certain**, if adapters are workspace members |
| Direct third-party dependencies exceed the current nine (`AGENTS.md:214`, `Cargo.toml:40-46`) | **certain** — an AWS SDK, a Kubernetes client and a TLS backend are three at minimum |
| Transitive crate count, build time, audit surface | **not measured. I'm not guessing.** `unsafe_code = "forbid"` (`Cargo.toml:50`) binds the thirteen workspace members and says nothing about their dependencies |
| Credential handling enters a repository with no I/O beyond file reads | **certain** |
| Rate limits and pagination make an adapter's output non-deterministic across runs unless recorded | **certain** for live accounts; avoidable with fixtures |

**The design has the mitigation and does not commit to it.** §70 (design:2974-3018) proposes a
reference fixture, and §94 (design:3597-3637) says "Do not start with multi-cloud generation. Build one
narrow discovery/conformance vertical slice." Both are right. But §85 (design:3450) states as an
acceptance criterion: "one AWS account/region reference estate can be scanned deterministically" —
which reads as a live account, and "deterministically" against a live account is a claim that cannot
hold.

**Fix, one line:** state that the gate runs against recorded fixtures only, and that any live-account
run is an out-of-gate operation producing evidence, never a `task check` step. That is consistent with
I1's recommendation and costs nothing.

---

## I9 — most of §58's capabilities already exist (medium)

**Verified.**

`protocols/aep/1.yaml:12-39` already declares, and puts a floor under:

```yaml
capabilities:
  - repository.read / repository.write
  - tests.execute
  - command.execute
  - network.read / network.write
  - telemetry.read
  - production.read / production.write
  - deployment.create / deployment.rollback
  - secret.read
  - artifact.read / artifact.write
  - planning.read / planning.write
  - review.request
  - approval.request

approval_floor:
  - production.write
  - deployment.create:production
```

§58 (design:2714-2721) proposes eight more:

| proposed | already covered by |
|---|---|
| `cloud.inventory.read` | `production.read` |
| `cluster.inventory.read` | `production.read` |
| `identity.policy.read` | `production.read` (+ `secret.read` for secret metadata) |
| `network.flow.read` | `telemetry.read` / `network.read` |
| `cloud.resource.write` | `production.write` — **already behind the approval floor** |
| `cluster.resource.write` | `production.write` — same |
| `network.route.write` | `production.write` — same |
| `identity.policy.write` | `production.write` — same |

So read-only discovery is already expressible today, and every write the design contemplates is
already impossible to grant outright (invariant 6: "Capabilities default to deny… A principle may
restrict; only a profile or protocol may grant", enforced by `CapabilityPolicy::decide` and
`crates/aep-domain/tests/safety_envelope.rs`).

The design half-notices: design:2707 ("Existing AEP/AOP concepts such as production read, network
read, telemetry read, repository read, and least privilege should be reused where they fit") and
design:2723 ("Do not add new capability names merely to mirror every provider API. Add them only when
they create meaningful governance boundaries"). That instinct is correct and matches this
repository's taste. **The four `.write` capabilities should be dropped from the design outright** —
they are provider verbs behind a floor that already exists, and adding them would weaken the floor by
giving a profile four new names to grant that are not `production.write`.

---

## I10 — §48's artifact kinds (medium, and mostly fine)

`ArtifactKind` (`crates/aep-domain/src/artifact.rs:328-388`) has 26 named variants plus
`Other(String)` (`:387`). §48 (design:2458-2479) proposes eight more and then states the restraint
itself (design:2472-2474):

> Do not add all of these immediately. Introduce an artifact kind only when it has stable identity,
> lifecycle, relations, and governance value.

That is how `ExecutableSystemSpecification` was added — exactly one kind, in W1.1, before any ESS code
existed (`docs/plan/ess-roadmap.md:44-52`), specifically so the loop could be tested with a human
producing the evidence by hand. The precedent is good and the design follows it.

Two notes: `Other(String)` means none of the eight blocks anything, so this is not a gating decision;
and design:2477 ("Large observation snapshots may be content-addressed external artifacts referenced by
AEP rather than embedded directly") is correct and matches `ArtifactLocation` already being a location
rather than a body.

---

## I11 — §29's change engine depends on an unreviewed design (low)

§29 (design:1907-1951) and §96 (design:3660-3698) both say infrastructure should reuse the ESS
semantic-diff architecture rather than build a second change engine. That is the right direction. The
dependency chain, per `AGENTS.md` § *Which documents are normative*:

```text
this design  →  ess-semantic-diff-impact-evolution-design-v0.1.md   ("not reviewed at all")
             →  ess-closed-loop-execution-conformance-design-v0.1.md ("frozen", not started)
             →  docs/plan/ess-wave-3.5-reconciliation.md             (G2, G15, G16 open)
```

A review of the semantic-diff design is being written concurrently
(`docs/reviews/2026-08-20-semantic-diff-feasibility-review.md`). I have not read it, and its
conclusions may change the cost of §29. Flagged rather than assessed.

---

## What this design gets right

Read for its good ideas rather than its risks, and several are portable to this repository **today,
with no infrastructure at all**.

### 1. It refuses to mint a top-level acronym (header:7)

> **Naming:** This document deliberately uses `InfraSpec`, `InfraIr`, and related names rather than
> introducing an `ISS` protocol/specification acronym in v0.1. The architectural boundary should prove
> itself before a new top-level acronym is made normative.

This is `docs/plan/ess-roadmap.md:27` — "each wave must be falsifiable by the one before it" — applied
to naming, and it is a discipline the repository would recognise instantly. AEP and ESS each earned
their acronym by shipping. **Keep this sentence regardless of what happens to the rest**, and make it
the model for how the next proposal opens.

### 2. Observation is not specification (§2.1, §5.3, §68)

design:804: "A scan cannot know which is intentional" — followed by the list that makes it concrete
(deliberate architecture, temporary migration infrastructure, manual drift, abandoned resources,
emergency incident changes, provider defaults, historical leftovers). §68 (design:2936-2954) refuses to
regenerate an estate from a scan and gives the staged path instead.

This is invariant 7's argument arriving independently in a different domain. It is also the argument
`docs/VISION.md:112-120` makes about `independent: true` being the one thing the loop asks you to
trust. Same instinct, same rigour.

### 3. Required / permitted / observed as three separate relations (§17) — **the best idea in the document**

design:1545-1574 splits communication into three relation classes and reads off the four cells:

| cell | design's reading (design:1558-1572) |
|---|---|
| required but not permitted | "Likely deployment/configuration defect" |
| required and permitted but never observed | "May be normal, dormant, or suspicious. **Not automatically a failure without a liveness expectation.**" |
| observed but not required | "Potential undocumented dependency or unexpected communication" |
| permitted but neither required nor observed | "Potentially excessive network authorization" |

**This is a better answer than the current evidence model has for "we looked and saw nothing."**
`Truth::Unknown` (`crates/aep-domain/src/predicate.rs:50-51`) means "no observation has been made yet"
and cannot distinguish that from *an observation was made and found nothing*. Invariant 5 stops
`Unknown` collapsing to `False`, which is the important half — but it leaves the repository unable to
say "we ran the check, it came back empty, and that is a fact about the world rather than a gap in our
knowledge." The second cell is exactly that distinction, and the qualifier — a liveness expectation has
to be *declared* before absence means anything — is the part that makes it sound rather than merely
appealing.

**Portable now, without infrastructure.** An evidence requirement that distinguishes "no evidence" from
"evidence of absence" is a refinement of `Truth` and `EvidenceRequirement`, not a cloud feature. It
would matter immediately for wave 4: a conformance scenario that ran and found no violation is not the
same as a scenario that did not run, and `EssConformanceResult` (`crates/aep-domain/src/evidence.rs:673`)
currently carries `scenarios_total` and `scenarios_failed` with no way to say a scenario was skipped.
This is the one idea I would lift out of the document and put in front of wave 4.

### 4. Coverage is a first-class output, and a permission failure is a gap, not an omission (§59, §60)

design:2739: "A failed permission should produce a coverage gap, not silently omit the resource and
claim completeness." design:2775: "A complete-looking graph with hidden coverage holes is dangerous."
§60 (design:2743-2774) gives the shape — accounts discovered vs scanned vs inaccessible, regions
requested vs completed, flow-log coverage as `9/12`.

This is the same discipline `AGENTS.md` § *Conventions* states as "**Verify a guard by breaking it**"
and `AGENTS.md` § *Invariants* practises by listing "*Enforced by* **nothing**" three times rather than
papering over it. §71's fault matrix (design:3020-3049) — including "stale observation causing Unknown
rather than Pass" (design:3036) — is `crates/aep-conformance/src/faulty.rs` for infrastructure, and the
repository already knows that a suite which passes everything tells you nothing (`git tag -n99`,
`0.2.0-wave-3`).

### 5. Portability is capability satisfaction, not name mapping (§2.8, §5.4, §39)

design:182: "AWS RDS is not semantically equivalent to an Azure resource merely because both vendors
call something a managed database." design:806-827 kills the translation table explicitly.

This is `conversions:` in `examples/billing/components.yaml:8-13`, generalised: two values that are
both a `String` underneath are still not the same type, and the crossing gets written down **with the
reason someone had for allowing it**. The design reached the repository's own rule from a different
direction, which is the best evidence available that the rule is real.

### 6. Secrets: metadata and references, never values (§20)

design:1644-1661. Should be an *invariant* wherever this lands, in the sense `AGENTS.md` uses the word —
with the thing that enforces it named — not a section. It is the one requirement in the document whose
violation is unrecoverable.

### 7. Endpoint conformance does not prove transition safety (§2.10, §5.6, §45)

design:837-839: "An AWS and Azure realization may independently satisfy the same InfraSpec while an
attempted migration between them loses data, breaks identity, or violates availability."

The repository already holds the precedent and the design cites it correctly (§53, design:2579-2593):
`workflows/migrations/forward-only.yaml` models an irreversible state and demands preparation evidence
before the point of no return. design:2593 — "Infrastructure evolution should reuse this principle
rather than introducing a universal rollback fiction" — is the right conclusion drawn from the right
file.

### 8. Its ESS-side example is the existing model, unchanged

Covered in I5. Worth restating as a positive: this author read `examples/billing/topology.yaml` and
built the boundary around what is there rather than around what would have been convenient. §5.7 then
argues *against* growing ESS, which is the opposite of what a proposal for a fourth domain usually
does.

---

## Decisions for Timo

| # | decision | options | cost | default if nobody answers |
|---|---|---|---|---|
| D1 | Do provider adapters live in this workspace? | (a) yes — `infra-discovery`, `infra-target-aws` as members; (b) no — adapters are **external verifiers**, this repo defines the input schema and consumes it | (a) `AGENTS.md:226` becomes false or discovery is ungated; >9 direct deps + TLS; credentials in-repo; `docs/VISION.md:157-170` must be rewritten. (b) **zero** — `protocols/aep/1.yaml:57-68` already has the `verifiers:` list, and `telemetry-query` is this shape | **(b)**. The design's own §2.11 and §92.7 argue for it; only §80 disagrees |
| D2 | May wall-clock freshness decide a verdict? | (a) yes, via an **injected** `Clock` as the engine and the wave-4 runner already do; (b) no — freshness stays causal (`FreshnessPolicy::UntilSuperseded`); (c) yes, read the clock at evaluation | (a) one type parameter; invariants 8 and 9 stay true. (b) **free**, answers most of §13, cannot express `<= 15 minutes old`. (c) invariants 8 **and** 9 break, and `generate-check` goes red overnight | **(b)**, escalating to (a) the first time a rule genuinely needs a duration |
| D3 | Accept §79's W8 revision? | (a) accept — record in `docs/plan/ess-roadmap.md` that when topology synthesis is designed, ESS emits a provider-neutral `InfrastructureRequirementSet` rather than a Kubernetes ontology; (b) leave W8 as one line in an unaccepted document | (a) ~10 lines of roadmap, **no code** — the constraint is free because the wave is unwritten. (b) free, and the note is lost | **(a)**. The cheapest true thing in the document |
| D4 | Sequencing | (a) accept whole, after wave 5; (b) narrow slice now — §17's relation classes + an `infra_conformance` evidence kind, no discovery, no realization; (c) defer entirely, keep as horizon; (d) accept minus the boundary-crossing parts | (a) ≈11 waves behind 2–4 unbuilt ones. (b) ≈1 wave, and §17 helps wave 4 rather than waiting on it. (c) free; the document ages but its ideas are dated, not perishable. (d) still ≈8 waves | **(c) + (b)**: keep the document as a horizon in `docs/VISION.md` § *Proposed, not accepted*, and lift §17's distinction into wave 4's evidence model now |
| D5 | Correct the header's claim? | (a) rewrite design:6 to name what it actually refines (one line in an unaccepted §43 list); (b) leave | (a) one line. (b) every future reader believes an accepted wave is being re-scoped — the failure mode `AGENTS.md` § *Which documents are normative* exists to stop | **(a)** |
| D6 | Does `InfraSpec` reuse the validation mechanics? | (a) yes — `InfraDiagnostics` **is** `ValidationErrors`, `InfraSpec` only via `TryFrom<RawInfraSpec>`, codes from `validation_codes!`; (b) leave `InfraDiagnostics` undefined | (a) one sentence in the design. (b) a new crate outside invariants 2, 3, 4 and possibly 11 on day one | **(a)**, and make it a precondition of ever sequencing the document |
| D7 | Does ESS grow the six requirement fields §8.3 needs? | (a) yes, as part of whatever wave takes BR2; (b) no — every requirement arrives as an `InfrastructureObligation` | (a) wave-1-territory model change to `Resource` (`crates/ess-domain/src/topology.rs:86`); touches the schema, the projections and `generated/`. (b) free, and correct under the design's own algebra, but BR3 generates nothing until it happens | **(b)**, stated openly in the design rather than discovered at BR2 |

---

## Method

* Every claim above carries a `file:line`, a design section number, or a command's output. Where I did
  not verify something, it says so: the transitive dependency count in I8, and the invariant-7
  reasoning in I6, which is labelled a hypothesis.
* Code line numbers are from `git show 34cac07:<path>`, because the working tree is being edited
  concurrently (77 modified files at the time of review, including gate G11/G19 work adding
  `SpecDigest` to `crates/aep-domain/src/evidence.rs`). Working-tree divergence is flagged where it
  matters.
* Wave sizes measured with `git diff --shortstat` between tags; test counts from `git tag -n99`. They
  agree with `docs/reviews/2026-08-20-next-waves-feasibility-review.md:44-46`.
* Writing quality was not assessed. The question answered is whether this can be built here, and
  whether it should be.
* I did not read `docs/reviews/2026-08-20-semantic-diff-feasibility-review.md`, which was being written
  concurrently; see I11.
