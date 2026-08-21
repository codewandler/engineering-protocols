---
title: Evidence and completion
sidebar_position: 3
description: Evidence records, producers and independence, three-valued evaluation, ordering facts, and the join between AEP and ESS.
---

# Evidence and completion

Completion in AEP is not a declaration. It is a predicate over recorded facts, and this page
explains where those facts come from and how they are evaluated.

## Evidence records

Evidence is a typed record submitted to the engine: a test result, a static-analysis run, a
deployment result, an approval, a diff, a review, a verification, a conformance run. Each record
carries:

* a **kind** — one of the kinds the protocol declares; a kind the protocol does not declare is
  refused at submission, with the declared kinds listed;
* a **producer** — who made the observation: `Producer::Agent { id }` or
  `Producer::Verifier { verifier }` are different variants, and requirements read which one it was;
* **provenance** — command, tool, revision, workspace, environment, digest, inputs — so the record
  can be re-derived later by someone who does not trust the submitter;
* a **sequence number** stamped by the engine at submission.

The engine projects facts from records: a `test_result` becomes `tests.unit.failed`,
`tests.unit.total` and so on; an `approval` becomes `approval.<id>.granted`. Predicates read those
facts. The full projection table is in the [vocabulary reference](../reference/vocabulary.md).

## Independence

An evidence requirement can be marked `independent: true`. The agent's own submission never
satisfies it — only a record whose producer is a verifier does. This is how "an agent cannot verify
itself" stops being a principle and becomes a type: the requirement reads the `Producer` variant.

The limit is worth stating as precisely as the mechanism. Independence is **structural, not
attested**: the engine checks who the record *says* produced it. A harness that lies about the
producer defeats the check, and nothing cryptographic prevents that today — see
[Limitations](../status/limitations.md), which treats this as the project's central trust
assumption.

## Three-valued evaluation: `Unknown` is not `False`

`tests.unit.failed == 0` is *false* when a suite ran and failed, and *unknown* when nothing ran.
Those are different situations demanding different responses, so predicate evaluation is
three-valued (Kleene logic), and **only `True` permits a transition**:

| Truth | What happened | What a harness should do |
|---|---|---|
| `True` | observed, and it holds | nothing |
| `False` | observed, and it is wrong | fix the code |
| `Unknown` | nothing observed it | run the verifier that would |

Collapsing `Unknown` into `False` produces an agent that "fixes" code nobody has tested. Collapsing
it into `True` produces one that finishes tasks nothing verified. The `Truth` type in `aep-domain`
has three variants, Kleene `and`/`or`, and deliberately no `From<bool>` and no `as_bool` — there is
no boolean to collapse into. The CLI renders the distinction as `✗` (false) versus `?` (unknown).

## Ordering is a fact

The engine records submission order, and `evidence.first_seq.<kind>` exposes it. Red-before-green is
therefore a checkable predicate rather than an instruction:

```text
evidence.first_seq.test_result < evidence.first_seq.diff
```

A test result was recorded before any code change existed. This only works if the harness submits
evidence as it observes it — batching everything at the end destroys the ordering facts.

## The join: how ESS evidence completes AEP tasks

AEP and ESS meet at one declared point. A task can carry an artifact of kind
`executable-system-specification`; the `ess-conformance` principle then requires, before completion,
evidence of kind `ess_conformance` that is `independent: true` and produced by a
`conformance-runner`:

```yaml
requires:
  before_completion:
    conditional:
      - when: artifact.executable-system-specification.exists
        require:
          predicates:
            - ess_conformance.passed
            - ess_conformance.scenarios.failed == 0
          evidence:
            - kind: ess_conformance
              independent: true
              verifier: conformance-runner
```

Three consequences:

* **Nobody reads a diff to judge conformance.** The specification judges it, and the protocol
  refuses to call the task done until something other than the author ran that judgement.
* **A task with no specification owes nothing here.** The condition reads the artifact graph at
  evaluation time; adding a specification turns the rule on without editing the rule.
* **The evidence is revision-bound and fails closed.** A conformance run carries the digest of the
  specification it ran against, and it must match the `model_digest` the specification artifact
  records. If the artifact records no digest, no run can be shown current and the requirement can
  never be satisfied — evidence that cannot demonstrate its revision is not assumed fresh.

`protocol ess conform evidence` mints the evidence record in the same process that ran the suite,
so no caller authors its own verdict. The repository's `examples/billing-conformance/` walks both
directions: a passing run completes the task; a failing run leaves it blocked, naming the principle
that refused.

---

**Sources.** `crates/aep-domain/src/evidence.rs`; `crates/aep-domain/src/predicate.rs` (the `Truth`
type); `principles/verification/ess-conformance.yaml`; `docs/guide/harness.md`;
`examples/billing-conformance/`; `AGENTS.md` § *Invariants* 5 and 7.
