# Fact-scoped applicability — Design v0.1

> **Repository:** `codewandler/engineering-protocols`
> **Status:** **proposed, not accepted.** Per [`AGENTS.md`](../../AGENTS.md) § *Which documents are
> normative*, a proposal is not a work order. Its acceptance surface is
> [`harness-wave-4-governed-dogfood.md`](../plan/harness-wave-4-governed-dogfood.md) § W4.2, which
> this wave extends to carry it; **no plan page or store item proposed it before this document
> existed**, and the header of an earlier draft claimed otherwise.
> **Audience:** whoever maintains `principles/`, and whoever writes the next task document.
> **Scope:** two principle documents gain an `applies_when:` clause over one new declared fact.
> No engine change, no protocol change, no new grammar, **no new enforcement mechanism** — it
> narrows an existing one with grammar three shipped documents already use, which is what keeps it
> inside wave 4's *"no new enforcement mechanism in this wave"* constraint
> ([plan page](../plan/harness-wave-4-governed-dogfood.md), *Decisions, taken*, row 3).
> **Cross-reference:** `principles/development/contract-testing.yaml`, whose header argues against
> the narrow form of this idea and which § 5 answers rather than eliding;
> `principles/verification/mutation-testing.yaml:8-11`, which states the repository's doctrine
> *against* this shape of fix and which § 6.2 engages.

---

## 0. Verdict first — necessary, and **not sufficient**

**This change does not unblock run `W4-2/1`, and an earlier draft of this document said it would.**
Measured on the live run:

| | `evidence.missing` at `adversarial_verify -> review` |
|---|---|
| before | **4** |
| after | **2** |

`protocol drive resume W4-2/1 … --max-iterations 60`, exit 1, status `blocked`
(`/home/timo/.cache/claude-tmp/resume-2.log`; the same two numbers reproduce under
`protocol evaluate --state`). The two it removes were **impossible** for a documentation change. The
two it leaves are `specification` and `verification` records, which the `development/checks` step
map never produces for **any** task. § 8 is the full accounting of the seven separate things that
still stand between this run and `complete`; this document closes **two** of them and says so.

It is proposed anyway, because the two it closes cannot be closed any other way that keeps one
profile governing both code work and documentation work, and because the remaining five are a
different defect in a different layer.

## 1. Motivation — the obligation that had no honest producer

Governed run **`W4-2/1`** drove `story:open-vocabulary-audit` — a documentation story — through
`implement` and `verify`. Its check suite reports **119 pass, 0 fail, 0 broken, 0 undeclared**
(`.engineering/runs/W4-2/1/adversarial_verify-1-1.log:206`). It blocked in `adversarial_verify` on
`evidence.missing == 0` (`.engineering/runs/W4-2/1/cursor.json`).

Two of the four missing records were unreachable rather than merely undone:

| principle | required evidence | pinned verifier |
|---|---|---|
| `contract-testing` | `contract_result` | `contract-runner` (`principles/development/contract-testing.yaml:31-32` at HEAD) |
| `property-based-testing` | `property_test_result` | `property-tester` (`principles/verification/property-based-testing.yaml:24-26` at HEAD) |

Both are `independent: true`, so the agent may not assert them. Neither verifier can observe a
document: a contract runner reads a published interface, a property tester generates inputs for a
callable. **The consequence for a person:** a writer who changes only documents was being asked for
a contract run that cannot exist, and the only ways out were to forge the record or to strip the
rule from every task the profile governs, including the ones that do change code.

## 2. The mechanism already exists

Nothing is built. `Principle::applies` is one line:

```rust
self.applicability.evaluate(facts) != crate::predicate::Truth::False
```

(`crates/aep-domain/src/principle.rs:688-692`, body at `:691`.) Resolution filters on it
(`crates/aep-engine/src/resolve.rs:162`). The posture is deliberate and **this document does not
touch it**: only an explicit `False` removes a rule. An absent fact evaluates `Unknown`
(`Predicate::Truthy`, `crates/aep-domain/src/predicate.rs:399-401`), and `Unknown != False`, so a
task that says nothing carries every obligation. **Silence is not an exemption.**

The fact reaches the predicate through `Task::facts`, which seeds `task.*` and then copies whatever
the document declared in `constraints.facts` (`crates/aep-domain/src/task.rs:357-369`).

## 3. The fact: `change.code`

**Spelling:** `change.code`, boolean, declared in a task's `constraints.facts` exactly like the
existing `change.public_contract` and `change.architectural`
(`examples/development-passkeys/task.yaml:27`).

**Meaning, in one sentence:** *this task's product includes a change to executable source.* Not "is
this task important", not "did I break a consumer" — a categorical statement about what the task
produces, settled before the diff exists.

**No protocol change is needed, and none is smuggled in.** `protocols/aep/1.yaml:113` declares the
family `change.**`; `FactPattern::matches` lets `**` match a one-segment leaf
(`crates/aep-domain/src/facts.rs:471-485`, asserted for `tests.unit` at `:746-751`), so
`check_predicate` accepts `change.code` (`crates/aep-engine/src/registry.rs:664-692`). The
unobservable-fact refusal — what catches `tests.unit.faild` — does not fire for a leaf under a
declared family. That openness is graded, in the audit this very run produced: the row *"A protocol
document's `observables:` block"* is **open**, citing `website/docs/reference/vocabulary.md:84`
(`docs/guide/open-vocabulary.md:93`, in the run's worktree).

So: no new validation, no vocabulary registry, no schema. § 7 is the honest statement of what that
costs.

## 4. The criterion, and its blind spot

> **Does this principle pin a verifier that, by construction, can only observe code?**

A pinned code-only verifier makes the obligation *impossible* for a non-code change, not merely
expensive. A generic verifier (`test-runner`) or an unpinned kind makes it satisfiable by any suite
— and a documentation project can produce those records, as this run did.

**The criterion is framed on required *evidence*, and that is a blind spot worth stating before the
table rather than after it.** A principle can oblige through *predicates* with no `evidence:` block
at all; such an obligation never appears in `evidence.missing`, and the criterion is structurally
unable to see it. `design-by-contract` is the live example — § 4.2.

### 4.1 Applied to the nine principles actually in force for this run

These are the nine `protocol resolve` reports for `W4-2/1` under `development.driven`
(`snapshot.json`, `protocol_resolved`, seq 2). An earlier draft judged five principles that this
profile never resolves and skipped six that it does; this table is the correction.

| principle | required evidence | verdict | reason |
|---|---|---|---|
| **`contract-testing`** | `contract_result` / `contract-runner` | **scoped** | Compares published interface surfaces. A document publishes none, so `contracts.checked > 0` has no honest producer. |
| **`property-based-testing`** | `property_test_result` / `property-tester` | **scoped** | Generated inputs need something to call. |
| `spec-driven` | `specification` (verifier unpinned) | **not scoped** | A documentation task has a specification — this run wrote one, `.engineering/planning/specification/open-vocabulary-audit.md`. That no *record* exists is a map gap (§ 8), not an impossibility. |
| `test-driven` | `test_result` / `test-runner` | **not scoped** | The run's 119 checks are its tests, red before green, and the ordering fact `evidence.first_seq.test_result < evidence.first_seq.diff` evaluates **✓**. |
| `static-analysis` | `static_analysis`, unpinned | **not scoped** | Refuted by the run: `protocol artifact validate` produced a real record, `errors: 0` (`snapshot.json`, evidence 6). |
| `least-privilege` | capability policy, no evidence | **not scoped** | Nothing about code shape. Its own header: a privilege rule with exceptions is not a privilege rule. |
| `provenance-tracking` | `verification` (independent), `diff` | **not scoped** | Provenance is owed by every change. The missing `verification` record is a map gap (§ 8). |
| `approval-gates` | conditional on deployment facts | **not scoped** | Already conditional, and correctly: reads `does not apply` for this run. |
| `reversible-changes` | conditional on `deployment.revision` | **not scoped** | Same. |

### 4.2 Applied to the principles only `development.critical` resolves

| principle | verdict | reason |
|---|---|---|
| `mutation-testing` | **not scoped** | Mutation testing over documents is real and this repository has now done it: W4-2's `mutation-proof` unit applies **nine** mutations to the audit and asserts each reddens a named check, on a copy, tree byte-identical afterwards (checks M1–M14, all PASS). Its header also places applicability with the profile — § 6.2. |
| `differential-testing` | **not scoped** | Old-versus-new needs a runner, not a compiler. **Note the reason is *not* the one an earlier draft gave** — see § 4.3, its escape hatch is broken. |
| `invariant-checking` | **not scoped** | Verifier unpinned, and a document invariant is machine-checkable: *"every open row's three trailing cells hold an em dash"* is one of the audit's own row invariants, machine-checked by the suite. |
| `ess-conformance` | **not scoped** | Already conditional on `artifact.executable-system-specification.exists`, checked against the **artifact graph at evaluation time** (`ess-conformance.yaml:22-26`) — strictly better than a declared fact, because nobody can mistype it into being false. |
| `design-by-contract` | **not scoped, and the criterion cannot say why** | § 4's blind spot, made concrete. It pins no verifier and declares no `evidence:` block, so the criterion answers "not scoped" for the wrong reason. It obliges `verification.<claim>.passed` **or** `property_test.<claim>.result == passed` for three claims (`principles/development/design-by-contract.yaml:14-16, 40-48`); for prose *both* disjuncts are code-shaped, and scoping `property-based-testing` removes one of the two routes without touching this rule. It is left alone because `development.driven` never resolves it and because a predicate-shaped fix is a different design. **Recorded as a limitation, not settled.** |

Two scoped out of fourteen. The twelve refusals are the answer to *"this is a loophole factory"* —
including two, `static-analysis` and `mutation-testing`, whose evidence this very run produced.

### 4.3 A latent defect found while arguing this, in a shipped document

`principles/verification/differential-testing.yaml:12-16` tells a task how to rule the principle out:
*"`change.behaviour_preserving: false` among its constraint facts"*. **That is false for the case the
principle exists for.** `applies_when` is `any:` (`:10`), Kleene disjunction has `True` dominate
(`crates/aep-domain/src/predicate.rs:76-84`), so for `task.kind: refactor` the first disjunct is
`True` and the declared `false` changes nothing. Verified by resolving a `refactor` task with
`change.behaviour_preserving: false` under `development.critical`: `differential-testing` is still in
force. Not fixed here — this document does not own that file's semantics — and filed as a
gap-register row.

## 5. The objection `contract-testing` already wrote against itself

Quoted in full, including the clause an earlier draft elided (`contract-testing.yaml:3-6` at HEAD):

> *The narrow form — apply only when a task declares `change.public_contract` — hands the switch to
> the party most likely to be wrong about it. "Did I change a published interface?" is exactly the
> question an agent answers by eyeballing its own diff, **and a task that answers `false` turns the
> check off entirely.** The person who finds out is whoever's client breaks on the next release.*

The elided clause is the stronger half and it applies to `change.code` too: a task that answers
`false` turns the check off entirely. The rebuttal is therefore **partial, and only on the first
half**:

* `change.public_contract` asks *did this diff break a consumer?* — a judgement about **effect**,
  which the contract runner exists to answer and which the author is worst placed to pre-empt. It
  stays unread.
* `change.code` asks *does this task change source at all?* — a statement about the task's
  **product**, visible in the task document, and wrong in a way a reviewer sees at a glance.

On the second half — *the switch is still in the author's hand* — the header is simply right, and
§ 7 concedes it rather than arguing. What bounds the damage is that this switch is categorical and
pre-diff, so a reviewer checking it needs no contract runner, only the ability to notice that a task
declaring `change.code: false` has a `.rs` file in its diff.

### 5.1 Dropping the principle removes the anti-vacuity guard and keeps the vacuous one

Worth stating because it is counter-intuitive. `contract-testing` adds `contracts.checked > 0`
(`:24-26` at HEAD) precisely so an empty contract suite cannot satisfy it. `development.standard`'s
completion block keeps `contracts.failed == 0` (`profiles/development-standard.yaml:38`), and
**applicability does not touch a completion condition** (§ 8, row 5). So for a task declaring
`change.code: false`, a `contract_result` with `checked: 0, failed: 0` would satisfy what remains.
Reaching that state takes a deliberately vacuous record *and* a false declaration; it is recorded
rather than mitigated, and it is an argument for § 6.1's alternative rather than against this one.

## 6. Alternatives considered

### 6.1 A profile for work whose contract surface is private

`development.fast` already excludes both principles **and** carries no `contracts.failed == 0`; its
own summary names this class of work — *"internal tooling, scripts, tests, docs generators"*
(`profiles/development-fast.yaml:8-9`). A `development.driven-fast` — `extends: development.fast`
plus `command.execute` — would reach the same `evidence.missing = 2` as this proposal **and** clear
two of the completion conditions in § 8 that this proposal cannot touch. On the run in front of us
it is strictly better, and an earlier draft omitted it from its "three honest exits".

It is not taken here for two reasons, both structural rather than about this run:

1. **It multiplies profiles rather than facts.** The change-shape question is orthogonal to the
   fast / standard / critical risk scale and to the driven capability grant. Answering it with a
   profile means a `driven-fast`, and then a `driven-standard-docs`, and so on across the cross
   product; answering it with one fact composes with every point on the scale.
2. **It moves the decision away from the task that knows it.** A profile is chosen for a body of
   work; a change shape is a property of one task. Choosing `development.fast` for a documentation
   task also silently lowers its risk class, which is a second, unrelated claim.

**Both are open to being overruled**, and if this repository decides the profile route is the right
one, the two `applies_when` clauses should be reverted rather than kept alongside it.

### 6.2 The repository's own doctrine says the opposite

`principles/verification/mutation-testing.yaml:8-11`:

> *whether a mutation run is worth its cost is a **profile's call, not a task's**, and the profile
> expresses that by dropping the principle (`without_principles:`) rather than by leaving it in force
> with a condition that never fires.*

That is a direct statement against this shape of fix, in a shipped document, and it must be answered
rather than quoted for support (an earlier draft did the latter). The answer is that the doctrine is
about **cost** and this is about **possibility**:

* *"a mutation run is expensive for the value it returns here"* is a judgement about worth, it
  varies with a team's budget, and a profile is the right place for it.
* *"no contract runner can observe prose"* is not a judgement at all. The obligation is unsatisfiable
  by construction, and a rule that can only ever be satisfied by forging its evidence is not a
  strict rule — it is a rule that trains people to forge evidence.

The doctrine's own remedy — `without_principles:` — is available and is what § 6.1 describes. This
document claims the fact is better *for the impossibility case only*, and it does not propose
extending `change.code` to any cost case.

## 7. The trap: a task can lie, and nothing stops it

**Stated plainly, because the alternative is a design document that pretends otherwise.** A task
declaring `change.code: false` that then changes code turns both principles off, and **no mechanism
in this repository refuses that.** Resolution reads the fact; it never compares it to the diff.

| claim | status |
|---|---|
| The fact is in a **reviewed, version-controlled document**, in the diff a reviewer reads. | true |
| `protocol resolve --format yaml` **echoes the declared facts** under `facts:`. | true — but only for someone re-running resolve against the same tree, not from the run's own record |
| Silence is safe: an **undeclared** `change.code` leaves both principles in force. | true, and tested |
| The **run directory records the facts**. | **false.** `cursor.json`, `snapshot.json` and `step-context.json` hold no copy of `constraints.facts`; the only matches under `.engineering/runs/W4-2/1/` are inside model transcripts, incidental to a model having read the file |
| The run records that two principles **were dropped**. | **false, and this is the sharp one.** `dropped_principles` is populated only from task removals and `profile.without_principles` (`crates/aep-engine/src/resolve.rs:97-122, 325`). An applicability drop leaves it **empty**, so the record shows a seven-principle plan, an empty drop list, and no facts. A reviewer must re-derive the deletion by diffing against the profile |
| Something **cross-checks the fact against the diff**. | **false**, and out of scope |

The mitigation is *review*, not enforcement, and it is weaker than an earlier draft claimed: the
thing a reviewer would have to notice is currently absent from every artefact the run leaves behind.
That is the strongest argument in this document for treating § 9's F-W4.2-1 and F-W4.2-3 as owed
rather than optional.

## 8. What this does **not** fix

Seven things stand between `W4-2/1` and `complete`. Rows 3 and 4 are what this document removes half
of; it touches none of the others. `is_complete` is `terminal && completion_met`
(`crates/aep-engine/src/evaluate.rs:252-254`), and `completion_requirements` folds in every in-force
principle's before-completion obligations as well as the profile's block (`:313-345`) — so a
completion condition is **not** removed by a principle falling away.

| # | what is owed | source | after this change |
|---|---|---|---|
| 1 | `tests.unit.failed == 0` | profile completion | ✓ satisfied |
| 2 | `static_analysis.errors == 0` | profile completion | ✓ satisfied |
| 3 | `evidence.missing == 0` | profile completion | **2**, was 4 — the two this closes |
| 4 | a `specification` record and an independent `verification` record | `spec-driven`, `provenance-tracking` | still owed; **the `development/checks` map declares only `test_result`, `trace_conformance`, `diff` and `static_analysis`** (`drivers/development/checks.yaml:128, 175, 183, 192, 205, 215, 233`), and neither kind is in `EvidenceMapping::MINTABLE` (`crates/aep-driver-spec/src/map.rs:531-536`), so no step can produce either |
| 5 | `contracts.failed == 0` | profile completion, `development.standard:38` | still owed and **unreachable for a document**: only a `ContractResult` projects it (`crates/aep-domain/src/evidence.rs:1609-1612`), and applicability cannot remove a completion condition |
| 6 | `regression_suite.result == passed` | `test-driven:31` | still owed; projected only by a `test_result` with suite `regression`, which the map never submits |
| 7 | `review.approved` | `review -> complete` guard | a person's, correctly — and no CLI verb writes the record the operator step asks for |

**The load-bearing structural finding is row 4's parenthesis, and it is bigger than this design.**
`StepMap::check_run` validates map → protocol (every kind a step declares is declared by the
protocol) and never plan → map (every kind the plan requires is produced by some step)
— `crates/aep-driver-spec/src/map.rs:710-750`. A map that can never satisfy its plan therefore
loads, runs, and blocks only at the guard, **after every model token has been spent**. For
`W4-2/1` that was $31.46 across ten sessions.

## 9. Follow-ups this leaves open

* **F-W4.2-1 — the run does not persist the facts it was governed by.** Small fix (write the resolved
  plan's `facts:` beside `snapshot.json`); not taken here because it changes the driver.
* **F-W4.2-2 — no `change.code` ↔ diff cross-check.** A `source_diff` record carrying changed paths
  would make a declared `false` refutable rather than merely reviewable.
* **F-W4.2-3 — an applicability drop is invisible in `dropped_principles`.** § 7. Arguably the
  cheapest of the three and the one that most improves review.
* **F-W4.2-4 — the step-map coverage check is one-directional.** § 8. The expensive one, and the
  reason a wave can burn a model budget to learn something a load-time check could have said.
* **F-W4.2-5 — `differential-testing`'s documented escape hatch does not work for `kind: refactor`.**
  § 4.3.
* **F-W4.2-6 — the criterion in § 4 cannot see predicate-only obligations.** § 4.2,
  `design-by-contract`.

## 10. What changes

```yaml
# principles/development/contract-testing.yaml
applies_when:
  all:
    - task.kind: {any_of: [feature, bugfix, refactor]}
    - change.code

# principles/verification/property-based-testing.yaml
applies_when: change.code
```

Both parse today, and `protocol validate --root .` on the amended tree is exit 0, `45 file(s) …
valid`. **The precedent claimed by an earlier draft does not exist**, and the accurate statement is
narrower: a bare truthy path is used by `differential-testing.yaml:17`, and `all:` with a nested
`any_of:` map is new to this repository, reaching `Predicate::from_node` through the single-key-map
route (`crates/aep-domain/src/predicate.rs:647-655` for `all:`, `:709-715` for the nested operator map).
Kleene conjunction makes the composite behave: `False` dominates (`Truth::and`, `:65-74`), so `change.code: false` removes `contract-testing` whatever
`task.kind` says, and an absent `change.code` leaves it `Unknown`, which is in force.

**Tests** (`crates/aep-engine/tests/documents.rs`, against the real document tree):

1. `a_task_that_declares_no_code_change_owes_no_contract_or_property_evidence` — a
   `development.driven` feature with `change.code: false` resolves to exactly the seven other
   principles. Verified by breaking it: commenting out the `applies_when` fails the test with a
   message naming the principle.
2. `a_task_that_declares_nothing_still_owes_contract_and_property_evidence` — the Kleene half, and
   the one that fails silently if anybody ever "simplifies" `applies` to `== Truth::True`. Verified
   by breaking it: that exact mutation fails the test.

## 11. What this is deliberately not

* **Not a new capability, protocol version, schema or enforcement mechanism.**
* **Not a change to `Principle::applies`.** The three-valued posture is why this is safe.
* **Not an exemption for documentation work.** Seven principles stay in force for this task,
  including static analysis, and the run still owes the six other things in § 8.
* **Not a claim that `change.code` is enforceable.** § 7.
* **Not sufficient to finish the run it was written for.** § 0.
