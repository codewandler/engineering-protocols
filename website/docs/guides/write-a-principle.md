---
title: Write a principle
sidebar_position: 2
description: Encode one of your team's rules as a principle document, put it in force through a profile, and learn the two authoring failures worth meeting early.
---

# Write a principle

A principle is one enforceable rule: when it applies, what it requires, by when, and who may attest
it. This guide encodes a real rule end to end:

> A change that rewrites persisted data needs an approved migration plan before the code is written,
> and something other than the agent has to have run the recovery path before it is finished.

For the full syntax, see the [document reference](../reference/documents.md).

Commands assume `B=target/debug/protocol` after `cargo build -p protocol-cli`. The tree they run
against is this repository's `protocols/`, `principles/`, `workflows/`, `profiles/` and `artifacts/`
with the two documents below added to it — which is what every count in the output lines is a count
of.

## Four decisions, in order

| Decision | Question | In the document |
|---|---|---|
| Applicability | when is this rule even about this task? | `applies_when:` |
| Timing | at which point does it bite? | `before_<phase>:` keys under `requires:` |
| Obligation | what must be true or exist? | `predicates:`, `artifacts:` |
| Evidence | who is allowed to say so? | `evidence:` with `independent: true` |

## The document

`principles/migration-has-a-way-back.yaml`:

```yaml
id: migration-has-a-way-back
version: 1
title: A schema migration has a way back
summary: >-
  A change that rewrites persisted data is not finished until something other
  than the agent has run the down path. Without it the recovery plan gets
  written during the incident, by whoever is awake.
applies_when:
  # A fact nothing can observe, so the task declares it. The alternative —
  # applying the rule to every task — gets it removed within a month.
  change.database_schema: true
requires:
  before_implementation:
    artifacts:
      - kind: migration-plan
        status: approved
  before_completion:
    predicates:
      - verification.recovery.passed
    evidence:
      # The agent's own report of a successful rollback rehearsal does not
      # satisfy this.
      - kind: verification
        independent: true
```

And a profile that puts it in force:

```yaml
id: acme.service
version: 1
title: Acme service development
summary: Standard development, plus Acme's migration rule.
protocol: adp/1
extends: development.standard
principles:
  - migration-has-a-way-back
```

## Verify it does what you meant

**Applicability.** Two tasks under the same profile — one declares `change.database_schema: true`,
the other declares it `false`:

```console
$ $B resolve --root . --task task.yaml | grep -E '^(task|principles|obligations)'
task        BILL-88 (feature)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking, contract-testing, property-based-testing, approval-gates, reversible-changes, migration-has-a-way-back
obligations 12
$ $B resolve --root . --task task-rename-a-button.yaml | grep -E '^(task|principles|obligations)'
task        BILL-89 (feature)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking, contract-testing, property-based-testing, approval-gates, reversible-changes
obligations 10
```

The rule is **absent** from the second task, not present and vacuously satisfied — so nobody reads a
green report and wonders which ticks meant anything.

**Saying nothing is not the same as saying no.** Leave `change.database_schema` out of the second
task entirely and the rule stays in force, all twelve obligations with it. An applicability condition
the engine cannot evaluate resolves to *applies*
(`crates/aep-domain/src/principle.rs:688`), because a rule that can rule itself out by silence is a
rule nobody can rely on. Opting out is a declaration a reviewer can see.

**Timing.** With the migration plan in the manifest, a failing test carries the task into
implementation. Remove the plan from the manifest and the same evidence stops one state short:

```console
$ $B evaluate --root . --task task.yaml --artifacts artifacts-without-the-plan.yaml \
    --evidence red-test.yaml --advance | grep -E '^(state|transitions)| -> implement|migration-plan'
state       establish_verifiers (Establish verifiers)
transitions
  establish_verifiers -> implement [blocked]
      ? artifact migration-plan (approved) — no migration-plan artifact is declared [principle migration-has-a-way-back]
```

The block names the principle. Somebody can read it and either write the plan or argue with the
rule — both better outcomes than an agent quietly writing the migration.

## The two authoring failures worth meeting early

**1. A fact the protocol does not declare.** Suppose the rule had reached for
`migration.rollback_tested`, which reads perfectly well in English:

```console
$ $B validate --root .
46 file(s): 3 protocol(s), 23 principle(s), 4 workflow(s), 7 profile(s), 8 lifecycle(s), 1 step map(s)
1 problem(s):
  - [unobservable_fact] principle migration-has-a-way-back.obligations.migration-has-a-way-back/before-completion: `migration.rollback_tested` is not declared observable by protocol adp/1 (hint: declared families: ess_conformance.**, trace_conformance.**, mutation.**, differential.**, invariant.**, clean_room.**, build.**, types.**, task.**, change.**, risk, severity, state.**, workflow.**, principle.**, evidence.**, required_evidence.**, tests.**, test.**, unit_tests.**, contract_tests.**, regression_suite.**, static_analysis.**, contracts.**, property_test.**, coverage.**, specification.**, diff.**, source_diff.**, artifact.**, review.**, verification.**, approval.**, approvals.**, deployment.**, metric.**, service.**)
$ echo $?
1
```

`migration.**` does exist — but only under `aop/1` (operations), and this principle runs under
`adp/1`. The error lists every family that *is* declared, which is usually enough to find the right
spelling — here, `verification.recovery.passed`.

**2. A declared family, but a spelling nothing projects.** This one passes validation and then never
becomes true:

```console
$ $B validate --root .
46 file(s): 3 protocol(s), 23 principle(s), 4 workflow(s), 7 profile(s), 8 lifecycle(s), 1 step map(s)
valid
$ $B evaluate --root . --task task.yaml | grep passsed
  ? verification.recovery.passsed                                 [principle migration-has-a-way-back]
      unobserved: verification.recovery.passsed
```

A `?` that never becomes `✓` is a task nobody can finish, and it looks like a stuck agent rather
than a typo. Check every new predicate against the
[projected-facts table](../reference/vocabulary.md#facts-the-engine-projects) before writing the
rest of the rule.

## Rules you are held to, and where each one bites

* `before_<phase>` keys use `_` for `-` in phase names: `before_verification_setup` → phase
  `verification-setup`. A phase no state declares is caught at **resolve**, not at validate — no
  workflow is known until a profile picks one — as ``[unknown_phase] workflow adp/default: an
  obligation is timed against phase `nonsense-phase`, which no state declares``, with the declared
  phases listed beside it.
* Requirements with no stated timing default to **before completion**.
* A principle must enforce something. A document with only a title is refused at validate as
  `[empty_declaration] principle empty-rule: declares no obligations, evidence, verification or
  capability policy, so it cannot change any outcome`.
* A principle's `capabilities:` may only take away. `deny` and `require_approval` are applied; an
  `allow:` parses, validates and is then **ignored** — a principle restricts, and granting is a
  profile's or a protocol's job (`crates/aep-domain/src/capability.rs:630`). Adding
  `allow: [secret.read]` to the rule above leaves `denied secret.read` in the resolved plan.
* Extending a profile can only make completion harder: conditions are conjoined — `acme.service`
  inherits `development.fast`'s completion condition through `development.standard` and reports both,
  separately, in `evaluate` — and a denial cannot be granted back.
