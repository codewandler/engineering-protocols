---
title: Facts, predicates and vocabulary
sidebar_position: 3
description: The declared capabilities, evidence kinds and verifiers, the fact spellings the engine actually projects, and the predicate syntax.
---

# Facts, predicates and vocabulary

The vocabulary is declared by the protocol documents (`protocols/aep/1.yaml` and its extensions);
a document that uses anything outside it fails validation. This page mirrors the repository's
authoring brief.

## Capabilities

Nothing else may appear in any `capabilities:` block:

```text
repository.read  repository.write  tests.execute  command.execute
network.read  network.write  telemetry.read
production.read  production.write
deployment.create[:environment]  deployment.rollback[:environment]
secret.read  artifact.read  artifact.write  planning.read  planning.write
review.request  approval.request
```

Environments: `development`, `test`, `staging`, `production`, or omitted (every environment).

**Approval floor:** `production.write` and `deployment.create:production` may never appear in a
profile's `allow`; resolution refuses otherwise.

## Evidence kinds

```text
test_result  static_analysis  contract_result  property_test_result  deployment_result
metric_observation  health_observation  approval  diff  artifact  review  verification
specification
```

Plus `ess_conformance`, produced by the conformance runner (see
[Verify an implementation](../guides/verify-conformance.md)).

## Verifiers

```text
compiler  test-runner  contract-runner  static-analyzer  property-tester  model-checker
telemetry-query  policy-engine  human-approval  human-review  artifact-validator
```

`aep_engine::engine::kinds_for_verifier` maps each verifier class to the evidence kinds it can
produce.

## Artifact kinds and statuses

Kinds: `vision product-requirements initiative epic story task specification acceptance-criteria
design feature-design component-design architecture-design api-design data-design
architecture-decision-record test-plan evaluation-plan verification-report review-result
approval-record release-plan migration-plan runbook incident-report postmortem`. Requiring `design`
is satisfied by any design subkind.

Statuses: `draft proposed in_review approved accepted rejected active implemented superseded
archived`. Requiring `approved` is satisfied by `accepted`, `active` or `implemented` too.

## Phases

`aep/1`: `intake specification planning implementation verification review learning completion`.
`adp/1` adds `decomposition verification-setup adversarial-verification`.
`aop/1` adds `detection triage diagnosis mitigation recovery qualification staging canary
observation promotion preparation migration`.

## Observable fact families

A predicate may only read these:

```text
task.**  change.**  risk  severity
state.**  workflow.**  principle.**  evidence.**  required_evidence.**
tests.**  test.**  unit_tests.**  contract_tests.**  regression_suite.**
static_analysis.**  contracts.**  property_test.**  coverage.**
specification.**  diff.**  source_diff.**  artifact.**  review.**  verification.**
approval.**  approvals.**  deployment.**  metric.**  service.**

adp/1 adds: mutation.**  differential.**  invariant.**  clean_room.**  build.**  types.**
aop/1 adds: incident.**  blast_radius.**  slo.**  release.**  rollout.**  runbook.**  migration.**
            error_rate  recovery_verified
```

Scales for `>=` on non-numeric values: `risk: low<medium<high<critical`,
`severity: info<low<medium<high<critical`, `health: unhealthy<degraded<healthy`.

## Facts the engine projects

A family being declared is necessary but not sufficient — use these spellings. A fact in a declared
family with a spelling nothing projects passes validation and then never becomes true.

| Fact | From |
|---|---|
| `task.id`, `task.kind`, `task.objective`, `task.profile` | the task |
| `tests.<suite>.{passed,failed,skipped,total,result,exists}` | a `test_result`; suites: `unit integration contract property regression mutation fuzz differential e2e smoke` |
| `test.result`, `test.exists`, `test.first_result` | most recent / first test run |
| `unit_tests.failed`, `contract_tests.failed`, `regression_suite.result` | aliases kept for the design documents' examples; accepted on input, canonical forms are what the engine emits |
| `static_analysis.{errors,warnings,result,exists}` | a `static_analysis` |
| `contracts.{checked,failed,breaking_changes,result,exists}` | a `contract_result` |
| `property_test.<claim>.{result,passed,cases,seed,exists}` | a `property_test_result`; `seed` is what re-runs the search that found a counterexample |
| `deployment.{status,succeeded,environment,revision}`, `deployment.previous_revision.exists`, `deployment.<env>.status` | a `deployment_result` |
| `metric.<name>` and bare `<name>` | a `metric_observation` |
| `service.health`, `service.<service>.health` | a `health_observation` |
| `approval.<id>.{granted,decision,by_human}` | an `approval` |
| `diff.{exists,files_changed,lines_added,lines_removed}`, `source_diff.exists` | a `diff` |
| `artifact.<kind>.{exists,count,approved,approved.count,<status>.count}` | the artifact graph |
| `artifact.<kind>.{schema_valid,sections_present,reviewed,current,relationship_valid}` | an `artifact` observation |
| `review.<subject-kind>.{result,approved,blocking_findings,by_human}`, `review.{result,approved}` | a `review` |
| `verification.<claim>.{status,passed}` | a `verification` |
| `specification.satisfied`, `specification.requirements.{total,satisfied}`, `specification.unsatisfied.count` | a `specification` |
| `state.current`, `state.<id>.entered`, `workflow.terminal` | the engine |
| `evidence.count.<kind>`, `evidence.first_seq.<kind>`, `evidence.last_seq.<kind>` | the engine |
| `evidence.missing`, `required_evidence.missing`, `approvals.granted`, `principle.<id>.active` | the engine |

`evidence.first_seq.<kind>` is submission order — how ordering rules become checkable:
`evidence.first_seq.test_result < evidence.first_seq.diff` says a test ran before any code changed.

Verification claim ids are singular and shared across documents: `precondition postcondition
invariant hypothesis recovery blast-radius clean-room differential mutation migration dry-run`.
Reuse one before inventing another — `invariant` and `invariants` are different claims, and evidence
for one does not satisfy a requirement for the other.

## Predicate syntax

Compact form, one predicate per list item:

```yaml
- tests.unit.failed == 0
- error_rate < service.slo.error_threshold     # dotted right-hand side = another fact
- test.result == failed                        # bare word = a literal
- release.version == "1.2.3"                   # quote a literal containing dots
- specification.satisfied                      # bare path: present and truthy
- defined(deployment.previous_revision)
- not change.architectural
```

Structured form:

```yaml
all: [ ... ]          # a bare list is also an implicit `all`
any: [ ... ]
not: <predicate>
task.kind: {any_of: [feature, bugfix]}
risk: {gte: medium}
change.architectural: true
```

Operators in mapping form: `eq ne lt lte gt gte any_of none_of exists truthy`.

Evaluation is three-valued — see [Evidence and completion](../concepts/evidence.md).
