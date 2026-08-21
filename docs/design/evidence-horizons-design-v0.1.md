# Evidence horizons — a green result from three weeks ago is not a fact — Design v0.1

> **Repository:** `codewandler/engineering-protocols`
> **Status:** **proposed**, and **corrected by adversarial review** — see § 12.
> **19 CONFIRMED · 15 NEEDS-CHANGE · 3 INFEASIBLE**, every one applied below and cited at the
> place it changed. Not accepted for build by any plan page.
> **Story:** [`story:evidence-horizons`](../../.engineering/planning/story/evidence-horizons.md) —
> item **C1**, first in an early adopter's own ranked fix order.
> **Audience:** an implementor who knows `aep-domain`'s three-valued evaluation and the seven harness
> calls; and anyone deciding whether a decaying fact belongs in the protocol at all.
> **Relationship to existing design:** additive. It replaces nothing in
> [`consolidated-design-v0.2.md`](consolidated-design-v0.2.md); it adds one field to an evidence
> record, one field to an evidence requirement, one accessor to a trait, one derived fact, and one
> module to the markdown backend. It changes no truth value's meaning — it produces `Unknown` where
> the engine already knew how to produce `Unknown`, and withholds a fact where a fact can no longer
> be stated honestly.
> **Regression target:** `examples/evidence-horizons-corpus/` — 42 raw annotations, of which the
> adopter's own reference implementation finds 37 and says so.

---

## 1. What is wrong today

An admitted fact is timeless. `EvidenceRecord` (`crates/aep-domain/src/evidence.rs:1912`) carries
`produced_at`, and `Engine::submit_evidence` (`crates/aep-engine/src/engine.rs:342`) stamps it from
the engine's own clock — *"The engine assigns the identifier and the timestamp. A caller cannot
backdate evidence"* (`engine.rs:47`). That is the right rule for a **log entry** and the wrong answer
to a **question about the world**: it records when the record arrived, never when anybody looked.

Two consequences, both live in the code as it stands.

| | today | consequence |
|---|---|---|
| A deployment check run three weeks ago | `EvidenceRequirement::evaluate` counts it (`requirement.rs:332`) | a transition is permitted on an observation nobody has repeated |
| A record submitted now for a check run last month | `produced_at = now` | the log says the fact is minutes old |

The second is the worse one, and it is what makes a single time field indefensible rather than merely
incomplete: with one field, the *only* honest thing a harness can do with an old observation is lie
about it.

## 2. What is being decided

Twelve decisions. Each is stated as a rule, then argued. Ten were taken before review; **D11 and
D12 were forced by it**, and are marked.

| # | decision |
|---|---|
| **D1** | An evidence record carries **two** times: `observed_at` (the caller's, required) and `produced_at` (the engine's). |
| **D2** | `observed_at` is a **newtype**, `ObservedAt`, not a second bare `Timestamp`. |
| **D3** | A future `observed_at` is a **refusal**, by one comparison, at submission. |
| **D4** | The **horizon is declared on the requirement**. An evidence record has no horizon field, so there is nothing on it to extend. |
| **D5** | Decay happens **inside `EvidenceRequirement::evaluate`**, and yields `Unknown` — never `False`. |
| **D6** | The evaluation clock reaches the domain through `RequirementContext::now() -> Option<Timestamp>`, whose `None` **fails closed**. |
| **D7** | Nothing about decay is stored in a snapshot. A restored execution re-decays against the clock it is restored under. |
| **D8** | The markdown corpus's one-line annotation is **split** into the requirement half and the observation half by the scanner, which is the same split D1 makes. |
| **D9** | A document scanner **reports its own coverage** — raw occurrences seen versus records produced — and a divergence is a finding. |
| **D10** | There is **no horizon-mutation operation** anywhere in the public API, and a source scan holds it that way. |
| **D11** | A lapsed record's **facts are withheld** from the fact store, under the strictest horizon the plan declares for its kind — because a transition's guard reads facts, not requirements. *(added by finding F22)* |
| **D12** | `evidence.missing` counts a lapsed requirement as missing, and a second fact, `evidence.lapsed`, says why. *(added by findings F21 and F38)* |

---

## 3. D1, D2, D3 — the two times

### 3.1 The fields

```rust
pub struct EvidenceEnvelope<T> {
    pub id: EvidenceId,
    pub observed_at: ObservedAt,   // new: when somebody looked. The caller's.
    pub produced_at: Timestamp,    // unchanged: when the record entered the log. The engine's.
    pub producer: Producer,
    pub subject: Option<SubjectRef>,
    pub value: T,
    pub provenance: Provenance,
}
```

`observed_at` is **required**: no serde default, no `Option`. The story's reasoning is mechanical
rather than aesthetic — the adopter found a one-field convention *silently classifying
scheduled-but-never-performed checks as the freshest records in the corpus*, because a future date
under a "when was this checked" reading produces a negative age, and a negative age inflates
remaining horizon. A field that may be absent is a field some caller will leave absent, and the
fallback would have to be `produced_at`, which reintroduces exactly the conflation.

No snapshot is **committed** anywhere in this repository —
`grep -rl next_seq --include='*.yaml' --include='*.json'` returns nothing outside
`.claude/worktrees/` — so nothing in the tree needs migrating.

**An in-flight run directory is a different claim, and finding F33 is right that the first draft
conflated them.** `aep-driver` persists `execution.snapshot()` into `.engineering/runs/<id>/`
(`crates/aep-driver/src/run.rs:212`, `:740`), and a run directory written before this field
**cannot be restored**: deserialization fails with `missing field observed_at`. That is the decided
outcome rather than an oversight. A record from before the field existed cannot say when it was
observed, and the two alternatives are both worse — inventing a time is the defect this design
exists to remove, and a nullable field is the defect one paragraph up. The refusal names the field,
which is what tells an operator that the remedy is to start the run again.

### 3.2 Why a newtype (D2)

`EvidenceEnvelope::new(id, produced_at, producer, value)` becomes
`EvidenceEnvelope::new(id, observed_at, produced_at, producer, value)`. Two adjacent parameters of
the same type is a swap waiting to happen, and a swapped pair here is silent: both are plausible
epoch values, and the wrong one decays on the wrong clock. `ObservedAt(Timestamp)` makes the swap a
compile error.

It also gives the age computation and the future-date comparison one home:

```rust
impl ObservedAt {
    pub fn new(at: Timestamp) -> Self;
    pub fn timestamp(self) -> Timestamp;
    pub fn is_after(self, now: Timestamp) -> bool;        // D3's one comparison
    pub fn age_millis(self, now: Timestamp) -> u64;       // saturating; never negative
}
```

### 3.3 The future-date gate (D3)

One comparison, at the one place a record enters the engine:

```rust
// crates/aep-engine/src/engine.rs, in submit_evidence, before the record is built
if submission.observed_at.is_after(now) {
    return Err(ProtocolError::ObservationInFuture { observed_at, now });
}
```

The comparison lives in `ObservedAt::is_after` (domain, clock-free); the clock that feeds it lives in
the engine, which is invariant 8. The refusal is a new `ProtocolError` variant with code
`observation_in_future`, so a harness branches on the reason rather than on message text.

**Why a refusal and not a clamp.** A planned re-check is a different object from a decaying
observation. Accepting a future date and treating it as "now" would store a scheduled check as a
performed one; accepting it and treating it as future would make the model answer "yes, somebody has
looked at this" for something nobody has looked at. The model must be able to answer *has anyone
ever looked at this?*, and the only representation under which it can is one where every stored
observation is in the past.

The scanner (§ 6) applies the same rule at its own boundary: an annotation dated after the reference
date is a **rejection with a reason**, not a record.

---

## 4. D4 — the horizon is on the requirement

The story leaves this as its one open question and states the default: **on the requirement**, on the
grounds that *a record that could set its own expiry is a record that can extend itself*. This design
takes that default and does not weaken it with a "stricter of the two" rule.

```yaml
requires:
  evidence:
    - kind: deployment_result
      horizon: 3d          # new
      independent: true
```

```rust
pub struct EvidenceRequirement {
    pub kind: EvidenceKind,
    pub at_least: usize,
    pub subject: Option<SubjectRef>,
    pub verifier: Option<Verifier>,
    pub independent: bool,
    pub horizon: Option<Horizon>,   // new. `None` = the present behaviour, exactly.
}
```

`Horizon` is a day count with one spelling on the way out (`7d`) and a tolerant parse on the way in
(`7d`, `7 d`, `7D`, bare `7`), because `corpus/01-forms.md` establishes that a convention which
rejects `(Horizon: 7D)` reports a correctly annotated claim as undated prose.

**Why not on the record, and why not "both, stricter wins".** Both alternatives put a number that
lengthens the life of a fact under the control of whoever produces the fact. `corpus/05-traps.md`
names the failure precisely: *"if `extend` is as easy to call as `re-check`, it is the one that gets
called — every time, under pressure, by whoever is trying to get a gate green."* Putting the horizon
on the requirement is not a defence against a malicious producer; it is a defence against an
ordinary one at 18:00 on a Friday. `Option<Horizon>` on the requirement also makes the change
strictly additive: every requirement in the repository parses to `horizon: None` and evaluates
exactly as it does today.

**Consequence, stated rather than hidden.** Because the horizon lives on the requirement, two
requirements may read the same record on different clocks — a 3d deployment gate and a 30d audit
gate over one deployment record. That is correct and is the point: how long an observation is worth
something is a property of *the question being asked*, not of the observation.

**A weakening, also stated rather than hidden (finding F26).** `EvidenceRequirement::matches`
(`requirement.rs:243`) checks `subject` and `verifier` only when they are given, so a requirement
that names a horizon and no subject is satisfied by *any* fresh record of that kind — a fresh test
run for an unrelated component revives a gate about this one. The design does **not** require
`subject` alongside `horizon`, because a kind-level horizon is a real and useful thing to write
(*"a test run within seven days"*), and forcing a subject onto it would make the honest form of the
requirement unwritable. The weakening is inherited from the matcher and is the matcher's to fix; a
requirement that must be about one subject says so, as it does today.

**And a refusal (finding F27).** `at_least: 0` parses, and it satisfies `matching >= self.at_least`
before any horizon branch can fire — a gate that reads as guarded and is not. `at_least: 0` beside a
`horizon` is therefore refused at parse, with a reason that says what it means. `Horizon::days(0)` is
refused for the same reason, and the two are kept consistent on purpose.

---

## 5. D5, D6, D7 — decay inside evaluate

### 5.1 The mechanism

`EvidenceRequirement::evaluate` gains one filter, in **three stages in this order** — finding F20
caught the first draft applying the horizon before the revision check, which would have made a
record that is both stale and lapsed report `Unknown` where it must report `False`:

```
for record in evidence where self.matches(record):
    1. if unbound_revision(record)      -> does not count; remember the reason; continue
    2. if self.horizon is Some:
         if context.now() is None       -> does not count; remember "no clock"; continue
         if not horizon.covers(..)      -> does not count; remember the lapse; continue
    3. counts
```

The lapse remembered is the one with the **greatest `observed_at`**, by comparison and not by
arrival: evidence is stored in submission order, not observation order, and the freshest lapsed
record is the one whose date a reader will recognise and whose re-check is cheapest (finding F35).

The outcome is then one `match` over the two remembered reasons, so a record that is both stale and
lapsed reports the revision mismatch — `False`, something was observed and it contradicts the
requirement — and re-running against the model that exists now is the move either way:

| situation | truth | detail |
|---|---|---|
| enough live records | `True` | — |
| a matching record is past its horizon | `Unknown` | `the last observation was at <observed_at>, the horizon is 7d, and it lapsed at <expiry>` |
| horizon declared, no clock available | `Unknown` | `horizon 7d cannot be checked: this context has no evaluation time` |
| no matching record at all | `Unknown` | unchanged: `0 of 1 required record(s) submitted` |

`Unknown`, never `False`, is invariant 5 and it is the whole decay direction: *the failure mode is a
refused transition with the reason "nobody knows", never a wrong one.* A lapsed deployment check does
not mean the deployment failed. It means nobody has looked.

The reason string is **required by acceptance** to name both the horizon and the observation time,
and it reaches the operator through the path that already exists:
`TransitionEvaluation::unmet` → `Requirement::line` → `RequirementOutcome`'s `Display`, which renders
`? evidence deployment_result — the last observation was at …`.

### 5.2 The boundary

`age == horizon` is **not** expired. `corpus/04-classification.md` is explicit about the cost of
getting this wrong: *"An off-by-one there fires the gate a day early on every record in a corpus,
which is how a gate gets muted and then deleted."* So `Horizon::covers` is
`now.saturating_sub(observed_at) <= days * 86_400_000`, and there is a test named for the boundary.

`saturating_sub` never produces a negative age, and D3 means it never has to: a future observation
cannot be stored.

**Two age computations exist on purpose, and finding F34 is right that they must be named
together.** The engine compares *instants* (`Horizon::covers`, milliseconds); the document scanner
compares *whole days* (`CivilDate::days_since`). They agree exactly for an observation at midnight,
which every scanned date is, and deliberately differ for one at noon — a record observed at midday
is six days old on the sixth morning by the day count and six-and-a-half by the instant. The engine
takes the instant because an engine has one; a document convention takes the day because that is
what the author wrote. Both boundaries have a test.

### 5.3 The clock reaches the domain by trait accessor (D6)

```rust
pub trait RequirementContext {
    fn facts(&self) -> &dyn FactSource;
    fn artifacts(&self) -> &ArtifactGraph;
    fn evidence(&self) -> &[EvidenceRecord];
    /// When this evaluation is happening. `None` when the context has no clock.
    fn now(&self) -> Option<Timestamp> { None }   // new, defaulted
}
```

Three properties this shape buys:

* **The domain stays clock-free** (invariant 8). It receives an instant; it never reads one. The
  banned-token scan in `crates/aep-domain/tests/determinism.rs` continues to pass unchanged.
* **It fails closed.** A context that cannot say what time it is cannot satisfy a requirement that
  declares a horizon — it yields `Unknown`, which permits nothing. The opposite polarity (treat a
  missing clock as "no decay") would mean any caller who forgot to wire a clock silently got the old,
  wrong behaviour, and would get it on the green path where nobody looks.
* **It breaks nothing.** A defaulted method leaves the two existing test contexts
  (`requirement.rs:1626`, `crates/aep-domain/tests/safety_envelope.rs:109`) compiling, and every
  requirement without a horizon is unaffected by `now` being `None`.

`Execution` implements it from `Execution::evaluated_at`, an `Option<Timestamp>` initialised
`None` — which is why the trait's default is the failing-closed one: an execution nobody has handed
a clock to is exactly the case that must not quietly pass.

**Finding F37 found the first draft's answer INFEASIBLE and it was right.** It said the field is set
in `Engine::evaluate`, and both evaluate entry points take `&Execution` — `engine.rs:162` and
`evaluate.rs:182`. The review offered two ways out; the design takes a third, and states the cost:

> `Execution::observe_at(now)` is called at **every entry point that holds `&mut`** — initialising,
> restoring, submitting evidence, transitioning. `evaluate` is a pure read and uses the instant of
> the last engine interaction.

Why this one. It breaks no published trait and touches no caller in the three crates that make the
seven calls. It gives the fact store and requirement evaluation **one** instant rather than two,
which is what § 5.4 needs. And it is safe where it matters: a transition is a mutation, so no gate
can be passed on a stale instant. The cost, named rather than hidden: a process that holds an
execution open for a week and only *reads* it sees a frozen clock. `observe_at` re-reads it, and
every mutation calls it.

### 5.4 `evidence.missing` must decay too

`Execution::satisfies_evidence` (`execution.rs:448`) is a **second** implementation of "does this
requirement have enough records", feeding the derived facts `evidence.missing` and
`required_evidence.missing`, and `evidence.missing == 0` guards a real transition in a shipped
workflow (`workflows/development/default.yaml:134`). If only `evaluate` decayed, that document would
keep passing while the evaluation beside it said `?` — the failure `evaluate.rs:2-3` exists to
prevent. So `satisfies_evidence` applies the same horizon filter against the same `evaluated_at`.

**They are consistent, not equal, and findings F21 and F38 are right that the first draft blurred
it.** `evidence.missing` is a **count**, not a truth value. A lapse makes it `1`, so
`evidence.missing == 0` reads `False` where the requirement reads `Unknown`. That is not a breach of
D5: it is the pre-existing polarity of a count, which already reads `False` for a requirement
*nobody has met yet*, and a count that said `?` because one of its inputs was unknown would be a
count nothing could compare.

What the design adds instead is **D12**: a second derived fact, `evidence.lapsed`, counting records
past their horizon. Collapsed into one number, a stale gate is indistinguishable from an empty one
on the surface an operator actually reads; separated, `missing 1 / lapsed 1` says *somebody looked
and nobody has looked since*, and `missing 1 / lapsed 0` says *nobody has looked at all*.

The parity test therefore asserts two things that can both hold — the review's correction, taken
verbatim: the requirement reads `Unknown`, **and** the count reports the same requirement as
missing.

### 5.4a Facts decay too, because a guard reads facts (D11 — finding F22)

The first draft said facts do not decay, and § 10 listed it as a deliberate non-goal. Finding F22
showed that this leaves the story's second acceptance bullet unmet: a transition's guard is
`transition.when.outcome(execution.fact_store())` (`evaluate.rs:212`), a predicate over the **fact
store**, which requirement evaluation never touches. A workflow guarded on
`deployment.status == succeeded` would still fire on a deployment nobody had looked at since — the
requirement beside it reading `?` and the guard waving it through.

So a lapsed record's facts are **withheld** from the store:

```rust
// Execution::refresh_facts
for recorded in &self.evidence {
    if self.has_lapsed(&recorded.record) { continue; }   // its facts are not bound
    observed.extend_facts(recorded.record.facts());
}
```

The horizon is on a requirement and a fact is not a requirement, so the question *how long is an
observation of this kind worth something?* is answered by **the plan as a whole, strictest answer
winning** — `Execution::strictest_horizon(kind)`, over every requirement set the plan holds
(obligations, completion, principles, state and transition requirements, and inside conditionals).
Two requirements over one deployment record, at 3d and 30d, mean the record's facts stand for three
days: the shorter is the one somebody wrote down because they knew how fast the subject moves.

**A withheld fact is absent, and an absent fact is `Unknown`** (`predicate.rs`, and invariant 5).
The guard does not read `False`; it does not claim the tests failed. It reads `?`, which permits
nothing and says nobody knows. That is the whole polarity of the design arriving at the one surface
the story names.

Two consequences the review asked to have written down:

* **A conditional's `when` is covered by this (finding F24).** `ConditionalRequirement::evaluate`
  maps `Unknown` to `Unknown` (`requirement.rs:912`) rather than to *does not apply*, and
  `count_missing_evidence` treats a conditional it cannot evaluate as in force
  (`execution.rs:429`). So a lapse cannot switch a requirement block off by making its condition
  unreadable — it makes the block unresolved, which refuses.
* **The verifier check is covered too (finding F23).** `principle_verification` (`evaluate.rs:382`)
  asked *has anyone ever?* and would have been the one surface where a lapsed record still read
  `True`. It now skips lapsed records through the same `Execution::has_lapsed`, so a verification
  requirement and an evidence requirement cannot disagree about one record.

### 5.5 Snapshots re-decay; they do not carry a verdict (D7)

`Snapshot` gains nothing. It carries `RecordedEvidence`, which carries the record, which now carries
`observed_at` — so the observation time round-trips, and **no derived staleness does**.

This is the same argument the snapshot already makes about the plan (`execution.rs:52`): *"a snapshot
that carried its own copy could outlive a change to them without anyone noticing."* A snapshot that
carried "this requirement was satisfied" would be a green verdict with a shelf life of forever. The
property, and its test:

> An execution snapshotted while a 3d requirement was satisfied, and restored six days later,
> evaluates to `Unknown` — from the same bytes.

`evaluated_at` is deliberately **not** in the snapshot for the same reason.

### 5.6 Re-submitting the identical record restores nothing

This falls out of D1 + D4 rather than needing a rule of its own, and the story requires it *asserted,
not documented*:

* the record's identity as a fact is `observed_at`, which the caller supplies;
* the engine stamps a new `id` and a new `produced_at`, and neither is read by the horizon filter;
* so re-submitting a byte-identical payload with the same `observed_at` produces a second lapsed
  record and the requirement still reads `Unknown`.

Only a record with a **new** `observed_at` — a new observation — restores it. The test submits the
same payload twice past the horizon, asserts `Unknown` both times, then submits with a fresh
`observed_at` and asserts `True`.

---

## 6. D8, D9 — the corpus, and what a scan owes

### 6.1 The 42 annotations, and what they map onto

The corpus is human-written markdown. Each annotation is one line of the adopter's convention:

```
Verify: 2026-08-30 — sprocket-api is running image v4.2.1 in the atlas namespace. (horizon: 7d)
        ^ observed_at   ^ the claim                                                ^ horizon
```

One line carries **both halves** of the D1/D4 split, and the scanner separates them — this is the
same split, arriving from the other direction:

| the line says | which half | where it lands |
|---|---|---|
| `2026-08-30` | the observation | `ClaimRecord::observed_at`, an `ObservedAt` |
| the prose | what was observed | `ClaimRecord::claim` |
| `(horizon: 7d)` | the **requirement** — how often this must be re-checked | `ClaimRecord::horizon` |
| the token being absent or unparseable | a carried flag | `ClaimRecord::malformed` |

**The horizon on a scanned line is the requirement half, not a record-side horizon.** It is the
author declaring a re-check interval for their own claim in the only place they have to declare it.
It binds the scanner's report. It never travels into the engine as a record-side expiry, because
`EvidenceRecord` has no field for it to travel in — which is D4 doing its work at the type level.

The classification, at a reference date, is the corpus's own vocabulary:

| state | rule | Kleene reading of the fact |
|---|---|---|
| `ok` | `remaining > warn_days` | `True` |
| `expiring` | `0 <= remaining <= warn_days` | `True` — still inside the horizon |
| `expired` | `age > horizon` | **`Unknown`** |

`expiring` is a **report** state, not a gate state: it exists for a human reading a list, and it
permits everything `ok` permits. Only `expired` changes a truth value, and it changes it to
`Unknown`.

### 6.2 The malformed default is carried, never inferred

A record with no parseable horizon token gets the stated default of **14 days** *and*
`malformed: true`. The two are independent, and `distribution.json` says why in one line: 14d is the
second most common deliberately chosen horizon in the source corpus (33 of 167 tokens). An
implementation that recovered "malformed" by testing `horizon == 14` would mark a third of a healthy
corpus as decayed convention. So `malformed` is a field on the record, set by the parse, and no code
anywhere infers it from the value.

### 6.3 Six positions, and the coverage claim (D9)

The corpus's most valuable content is the evidence that a line-anchored parser *keeps* failing:
six positions where an annotation is present, correct, legible to a human, and invisible — three of
them found in production, three found in an afternoon by comparing counts. The scanner handles all
six (a wrapped continuation line; a `>` quote block; after `<br>` in a table cell; at the end of a
table cell with no `<br>`; the second of two consecutive `<br>` rows; inside inline-code backticks).

Handling six positions is not the design. **This is:**

```rust
pub struct ClaimScan {
    pub records: Vec<ClaimRecord>,
    pub raw_occurrences: usize,     // `Verify:\s*\d{4}-\d{2}-\d{2}\s*—`, counted independently
    pub rejections: Vec<ClaimRejection>,
}

impl ClaimScan {
    /// `raw_occurrences - records.len()`. Non-zero is a finding, not a warning.
    pub fn divergence(&self) -> usize;
}
```

The raw count is a lower bound on what a human would call an annotation, computed **without the
parser**. The comparison is one line and it is what surfaced 15 unwatched annotations in 160 on a
live corpus — 9.4%, in a gate whose entire job was making unchecked claims visible. It belongs in the
gate rather than in an investigation, so `protocol evidence scan --strict` exits non-zero on a
non-zero divergence, and the corpus regression test asserts divergence 0 on all six files.

**The stop lines are binding, and finding F29 is right that a first draft leaving them implicit
would have left every `claim` in `expected.json` resting on nothing.** A body absorbs continuation
lines and stops at: another `Verify:`, a `Due:`, a heading, a list bullet, a blank line, a bare `>`,
a new table row, or the end of a table cell. `Last updated:` is **body text**, not a stop — the
reference absorbs it into `03-hidden-positions.md`'s header record, and following it there keeps the
42-record set agreeing with `expected.json` on every binding field.

**Which fields bind (finding F31).** `expected.json` holds 37 records and two of them are
known-wrong: `03-hidden-positions.md` record 4 and `06-reference-gaps.md` record 1 each end with an
embedded `<br>Verify: 2026-08-29 — …  |`, a neighbour the reference swallowed. So the regression
binds `date`, `horizon`, `malformed`, `state` and `days`, and **not** `what`. It asserts positively
that no record's claim text contains `Verify:` — the swallow bug, made checkable.

A `ClaimRejection` is the other half of the same honesty: the hyphen-separator line in
`01-forms.md` is *deliberately* not an annotation, and it is not counted raw either, so it must
produce neither a record nor a divergence. The two deliberate negatives:

| negative | raw-counted? | record? | why |
|---|---|---|---|
| `Verify: 2026-08-30 - hyphen …` | no (no em-dash) | **no** | accepting it inflates the count and hides a real malformed line in the noise |
| `… ( horizon: 5d)` — space after `(` | yes | **yes, `malformed: true`, horizon 14d** | the token is deliberately strict; the annotation is still an annotation |

The second is the subtle one and the corpus states the reasoning: loosening the left edge of the
token is the first step toward accepting `(horizon: 5d — but see below)`, which is the failure the
strictness exists to prevent.

### 6.4 Round-trip through the markdown backend

`ClaimRecord::render()` returns the canonical one-line form for a well-formed record, and **the
original source span verbatim for a malformed one**. Rendering a malformed record canonically would
write its 14d default into the document as though it had been chosen — turning a carried flag into
an inferred one and destroying § 6.2.

**Finding F30 caught the first draft asserting one property where there are two**, and it was right:
stated as a round trip, the malformed half is `scan(source) == scan(source)`, which is a tautology
over 8 of the 42 records. Split:

| records | property | what it is |
|---|---|---|
| the 34 well-formed | `scan(render(r))` yields exactly one record equal to `r` on `(date, horizon, malformed, claim)` | a round trip |
| the 8 malformed | `render(r)` is byte-equal to the source span, and is **not** the canonical form | a normalisation *refusal* |

The span may be multi-line (`02-malformed.md:33-34`), and `render` returns it whole.

### 6.5 The two traps classify `ok`, on purpose

`corpus/05-traps.md` holds four records; all four classify `ok` or `expiring` at the reference date,
and the regression test asserts that **none is rejected and none is expired**. A conforming
implementation must not invent a contradiction check it cannot ground: a claim can be false inside
its horizon, because a horizon is a volatility guess and not a guarantee.

What the model owes instead is two things it *can* do:

1. **Shortening a horizon is cheap and attributable.** It is an edit to the requirement — one line,
   in a reviewed document, with the reading that justified it in the same diff.
   `02-malformed.md` shows the correct form: the reason goes *before* the token.
2. **A horizon that grew while its observation date did not is detectable from history.** That needs
   two readings, not one, which is why it lives in the store rather than in the parser:

```rust
pub fn horizon_growth(before: &ClaimScan, after: &ClaimScan) -> Vec<HorizonGrowth>;
```

matched on `(claim text, observed_at)` — **not on text alone**, which finding F39 showed is
ambiguous here: `05-traps.md:41` and `:45` carry byte-identical claim text at two different dates,
and they are two different facts. Where one scan holds two records under the same key, the
diagnostic reports *I could not tell* rather than pairing arbitrarily, which is why `HorizonGrowth`
is an enum with a `Ambiguous` arm and not a struct.

**The fixture cannot supply the `before` reading, and the first draft claimed it could — INFEASIBLE,
corrected.** `05-traps.md` holds `(2026-08-30, 7d)` and `(2026-08-04, 60d)`: different dates *and*
different horizons, so neither ordering is *the horizon grew while the date stood still*. The test
therefore builds `before` from the trap-2 claim text at `2026-08-04` with a 7d horizon and uses the
fixture's own `(2026-08-04, 60d)` row as `after` — one true positive — and asserts that the correct
refresh, the `(2026-08-30, 7d)` row, is **not** flagged.

---

## 7. D10 — no horizon mutation, and a scan that says so

The story's requirement is absolute: *"the API offers no horizon mutation at all."* Three mechanisms,
in order of strength:

| # | mechanism | strength |
|---|---|---|
| 1 | `EvidenceRecord` has **no horizon field** | absolute — there is nothing on a record to mutate |
| 2 | a requirement's horizon comes from a parsed document and is re-read on every resolve | strong — an in-memory change does not survive |
| 3 | a source scan, over five crates | a guard on future edits |

**Finding F25 was right that mechanism 1 was overstated.** The *record* has no horizon; the
**requirement** does, its fields are `pub`, and it derives `Clone` — so `requirement.horizon =
Some(longer)` is a one-line extension, and a scan that looked only for `fn *horizon(&mut self)`
would not see it. The scan therefore refuses **two** constructs, across `aep-domain`, `aep-engine`,
`aep-backend-markdown`, `aep-driver` and `protocol-cli`:

* assignment — `.horizon =`, and not `.horizon ==`, and not the struct-literal `horizon:` that
  *constructs* a requirement from a document, which is the one way a horizon is ever set at all;
* any `fn` taking `&mut self` whose name contains `horizon`.

Making the field private was considered and not taken: `from_node` builds the value in the same
module, but every requirement-shaped fixture in the workspace would need a constructor, and the
mechanism that actually holds the rule is the one that reads the whole workspace rather than one
type's visibility.

Mechanism 3 is the house pattern (invariants 2, 7, 8 and 9 are all held by source scans). Its own
failure mode — a scan that has silently stopped matching — is handled the way `evidence_scan.rs`
handles it: the test asserts each extractor **finds** a planted positive, and rejects every near
miss, before asserting it finds nothing real.

---

## 8. CLI surface

| verb | what it does | exit |
|---|---|---|
| `protocol evaluate --evidence <file>` | unchanged path; the file's records now carry a required `observed_at` | as today |
| `protocol evidence scan <paths…> [--at DATE] [--warn-days N] [--strict] [--fail-on-expired]` | scans markdown, prints records + per-file coverage + classification | `1` under `--strict` when coverage diverges; `1` under `--fail-on-expired` when any record has lapsed |
| `protocol evidence inspect <files…> [--at DATE] [--horizon 7d]` | reads submission files, prints each record's `observed_at`, age and state; refuses a future observation | `1` on a future observation |
| `protocol ess conform evidence [--observed-at DATE]` | existing verb; the record it writes now states when the run happened | unchanged |
| `protocol trace evidence [--observed-at DATE]` | the same, for a transcript check | unchanged |

**Two flags, not one (finding F32).** The first draft gave `--strict` two definitions in one
document. They are separate because they answer different questions — *is the gate blind?* and *is
the claim stale?* — and because the vendored corpus deliberately carries nine expired records at its
reference date: a verb that conflated them could never be run over the corpus as a pass condition.

**`inspect --horizon` is report-only (finding F36).** It is a what-if applied to a printed table. It
reaches no requirement, no evaluation and no document, and nothing it prints can extend the life of
a record. The horizon that decides a gate is declared on a requirement, in a reviewed document, and
is re-read on every resolve.

**`--observed-at` on the two minting verbs defaults to now, and that is not an inference.** Those
verbs *perform* the observation they record — the suite runs in that process, in that second. The
flag exists so a committed record can be regenerated byte for byte, which is the one legitimate
reason to pin an observation time, and it is an explicit flag precisely so that pinning is visible.

Every verb takes `--format text|json|yaml`. `--at` defaults to today and every test passes it
explicitly, which is what makes the corpus regression reproducible on any machine on any day. `scan`
and `inspect` are **reads**: neither writes a document, so `protocol artifact new`'s no-`--out`
argument does not arise.

## 9. Schema impact

Invariant 1: Rust is the source of truth, `cargo xtask schema` regenerates.

| schema | change |
|---|---|
| `principle.schema.json`, `profile.schema.json`, `workflow.schema.json` | `EvidenceRequirement` gains an optional `horizon`, published through the hand-written `JsonSchema` impl at `requirement.rs:1381`, in its properties **and** in the `either(..)` description string |
| `evidence.schema.json` | **unchanged** — it publishes `Evidence`, the payload, not the envelope |
| `event.schema.json`, `protocol.schema.json`, `task.schema.json` | unchanged |

Finding F28 corrected this list in both directions: `protocol` and `task` publish no
`EvidenceRequirement`, and `workflow` does — a state's and a transition's `requires`. `cargo xtask
schema` writes exactly the three named, which is the check that settled it.

`EvidenceInput` (`aep-schema/src/parse.rs:283`), the shape of an `--evidence` file, gains a required
`observed_at`. It is not a published schema today; that is a pre-existing gap and this design does
not widen it.

## 10. What this does not do

* **It does not decay the evidence *counts*.** `evidence.count.*`, `evidence.first_seq.*` and
  `evidence.last_seq.*` remain facts about the **log**: how many records arrived, and in what order.
  A count that decayed would break `evidence.first_seq.test_result < evidence.first_seq.diff`, the
  fact that makes red-before-green checkable, by rewriting the past. The *observations* a record
  projects do decay — that is D11 — and the two are different things.
* **It does not check whether a verifier's stated observation time is honest.** That is § C5's
  territory and belongs to a later round; the story puts it out of scope explicitly. `ObservedAt` is
  as trustworthy as the producer, and the audit trail records who said it.
* **It does not detect contradiction inside a horizon.** § 6.5 — deliberately, because it cannot be
  grounded.
* **It does not schedule anything.** A planned re-check is a different object; D3 refuses to store one
  as an observation, and this design does not add the object. `story:time-based-transitions` (§ D2)
  is where a clock the protocol can *plan* against belongs.

## 11. Build order

| # | crate | what |
|---|---|---|
| 1 | `aep-domain` | `CivilDate`, `Horizon`, `ObservedAt` in `time.rs`; `observed_at` on the envelope; `horizon` on `EvidenceRequirement`; decay in `evaluate`; `RequirementContext::now`; the D10 scan |
| 2 | `aep-backend-markdown` | `claim.rs` — the scanner, the six positions, coverage, the two properties of § 6.4, `horizon_growth` |
| 3 | `aep-engine` | `evaluated_at` and `observe_at`; the D3 refusal; `satisfies_evidence` parity; D11's fact withholding; the verifier check |
| 4 | `aep-schema`, `ess-conformance`, `trace-spec` | `observed_at` on `EvidenceInput` and on the two records the CLI mints; `cargo xtask schema` |
| 5 | `protocol-cli` | `protocol evidence scan` / `inspect`; `--observed-at` on the two minting verbs |
| 6 | tests | the corpus regression: 42 records, per-file counts, both negatives, divergence 0 |

## 12. Adversarial review — verdicts

One reviewer, briefed to break this document rather than to appreciate it, against the code that
exists. **19 CONFIRMED · 15 NEEDS-CHANGE · 3 INFEASIBLE · 0 unresolved.** Every NEEDS-CHANGE and
INFEASIBLE item is applied above and cites the finding that forced it, in the house pattern of
`harness-planning-and-driver-design-v0.1.md` § 4.7. **Nothing was re-argued.**

### The three that were INFEASIBLE as written

| # | what was impossible | resolution |
|---|---|---|
| **F37** | `evaluated_at` "set in `Engine::evaluate`" — both evaluate entry points take `&Execution` (`engine.rs:162`, `evaluate.rs:182`) | § 5.3 — a third option the review did not list: set at every `&mut` entry point, `evaluate` reads the last one. No trait breaks; the cost is named |
| **F38** | "a test asserts the two agree" — one is a truth value, the other a count | § 5.4 — two assertions that can both hold, plus `evidence.lapsed` (D12) so the two causes stay distinguishable |
| **F39** | `horizon_growth` over "the trap-2 pair in the fixture" — the pair differs in date *and* horizon, so neither ordering is growth | § 6.5 — the test builds `before`; matching keys on `(text, observed_at)`; a duplicate key reports ambiguity |

### The fifteen corrections

| # | verdict | what changed |
|---|---|---|
| **F20** | NEEDS-CHANGE | § 5.1's loop is now three ordered stages; a record that is both stale and lapsed reports `False` |
| **F21** | NEEDS-CHANGE | § 5.4 states that `evidence.missing` is a count and reads `False` on a lapse; D12 adds `evidence.lapsed` |
| **F22** | NEEDS-CHANGE | **D11** — facts decay, because a guard reads facts. § 5.4a. The story's second acceptance bullet was otherwise unmet |
| **F23** | NEEDS-CHANGE | `principle_verification` skips lapsed records; it was the one surface still reading `True` |
| **F24** | NEEDS-CHANGE | § 5.4a states that a conditional's `when` reads `Unknown` on a lapse and therefore applies, rather than switching a block off |
| **F25** | NEEDS-CHANGE | § 7 — mechanism 1 was overstated; the scan now refuses `.horizon =` assignment across five crates |
| **F26** | NEEDS-CHANGE | § 4 states the subject-less matcher weakening rather than leaving it implicit |
| **F27** | NEEDS-CHANGE | `at_least: 0` beside a `horizon` is refused at parse |
| **F28** | NEEDS-CHANGE | § 9's schema list corrected in both directions: principle, profile, **workflow** |
| **F29** | NEEDS-CHANGE | § 6.3 makes the stop lines binding and decides `Last updated:` explicitly |
| **F30** | NEEDS-CHANGE | § 6.4 splits one tautological property into a round trip and a normalisation refusal |
| **F31** | NEEDS-CHANGE | § 6.3 names the binding fields and the two known-wrong reference records |
| **F32** | NEEDS-CHANGE | `--strict` has one meaning; `--fail-on-expired` is its own flag |
| **F33** | NEEDS-CHANGE | § 3.1 separates *no committed snapshot* from *an in-flight run directory*, which cannot be restored |
| **F34** | NEEDS-CHANGE | § 5.2 names both age computations — instants in the engine, whole days in the scanner — and both have a boundary test |
| **F35** | NEEDS-CHANGE | the lapse reported is the one with the greatest `observed_at`, by comparison |
| **F36** | NEEDS-CHANGE | `inspect --horizon` is marked report-only, in the document and in the flag's own help |

*(F35 and F36 are listed with the fifteen; the count of NEEDS-CHANGE items is 15 and F14 — a wrong
citation, `evaluate.rs:4` for `evaluate.rs:2-3` — was folded in as a CONFIRMED-with-correction.)*

### What the review confirmed

The nineteen CONFIRMED findings are load-bearing and worth keeping visible, because each is a
premise this design rests on that somebody checked rather than assumed:

* the raw regex yields **exactly 42**, per file 12/7/7/8/4/4, matching `expected.json`'s own
  `coverage` block;
* the hyphen negative is not raw-counted, and `( horizon: 5d)` is a record with `malformed: true`
  and the 14d default;
* `age == horizon` is not expired, and `expected.json` agrees;
* both `05-traps.md` pairs classify ok/expiring with nothing invented — and the corpus README's
  *"Both rows classify **ok**"* is loose: with a warning window of 2, two of the four are `expiring`;
* `EvidenceRecord` has no horizon field, and `engine.rs:340` is the only place one is constructed in
  shipped engine code;
* `Unknown` propagates correctly through `and`/`or`/`not`, `RequirementReport::extend` and
  `permitted = guard.is_satisfied() && items.all(..)`;
* a defaulted `RequirementContext::now` leaves all three implementors compiling;
* invariant 2 forces no `Raw*`/validated split on `Horizon` or `ObservedAt`, and invariant 8's
  banned-token scan is untouched by a date parser.
