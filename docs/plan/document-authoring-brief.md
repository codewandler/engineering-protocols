# Document authoring brief

Everything needed to write a valid AEP document without reading the Rust. The declared vocabulary
below is the contract: a document that uses anything outside it fails validation.

Voice: these documents are read by engineers deciding whether to trust the rule. Comment on **why** a
rule exists where the reason is not obvious; never restate the YAML in prose.

---

## 1. Declared vocabulary (`protocols/aep/1.yaml`)

**Capabilities** — nothing else may be mentioned in any `capabilities:` block:

```text
repository.read  repository.write  tests.execute  command.execute
network.read  network.write  telemetry.read
production.read  production.write
deployment.create[:environment]  deployment.rollback[:environment]
secret.read  artifact.read  artifact.write  planning.read  planning.write
review.request  approval.request
```

Environments: `development`, `test`, `staging`, `production`, or omitted (meaning every environment).

**Approval floor** — `production.write` and `deployment.create:production` may never appear in a
profile's `allow`. Put them in `require_approval` or `deny`. Resolution refuses otherwise.

**Evidence kinds**:

```text
test_result  static_analysis  contract_result  property_test_result  deployment_result
metric_observation  health_observation  approval  diff  artifact  review  verification  specification
```

**Verifiers**:

```text
compiler  test-runner  contract-runner  static-analyzer  property-tester  model-checker
telemetry-query  policy-engine  human-approval  human-review  artifact-validator
```

**Artifact kinds**: `vision  product-requirements  initiative  epic  story  task  specification
acceptance-criteria  design  feature-design  component-design  architecture-design  api-design
data-design  architecture-decision-record  test-plan  evaluation-plan  verification-report
review-result  approval-record  release-plan  migration-plan  runbook  incident-report  postmortem`.
Requiring `design` is satisfied by any design subkind.

**Artifact statuses**: `draft  proposed  in_review  approved  accepted  rejected  active
implemented  superseded  archived`. Requiring `approved` is satisfied by `accepted`, `active` or
`implemented` too.

**Phases** — `aep/1`: `intake  specification  planning  implementation  verification  review
learning  completion`. `adp/1` adds `decomposition  verification-setup  adversarial-verification`.
`aop/1` adds `detection  triage  diagnosis  mitigation  recovery  qualification  staging  canary
observation  promotion  preparation  migration`.

**Observable fact families** — a predicate may only read these:

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

**Scales** (for `>=` on non-numeric values): `risk: low<medium<high<critical`,
`severity: info<low<medium<high<critical`, `health: unhealthy<degraded<healthy`.

## 2. Facts the engine actually projects

Use these spellings. Anything else is unobservable in practice even if the family is declared.

| Fact | From |
|---|---|
| `task.id`, `task.kind`, `task.objective`, `task.profile` | the task |
| `tests.<suite>.{passed,failed,skipped,total,result,exists}` | a `test_result`; suites: `unit integration contract property regression mutation fuzz differential e2e smoke` |
| `test.result`, `test.exists`, `test.first_result` | most recent / first test run |
| `unit_tests.failed`, `contract_tests.failed`, `regression_suite.result` | aliases kept for the design documents' examples |
| `static_analysis.{errors,warnings,result,exists}` | a `static_analysis` |
| `contracts.{checked,failed,breaking_changes,result,exists}` | a `contract_result` |
| `property_test.<claim>.{result,passed,cases,seed,exists}` | a `property_test_result`. `seed` is what re-runs the search that found the counterexample, so a result carrying one is reproducible and a result without one says so rather than looking reproducible |
| `deployment.{status,succeeded,environment,revision}`, `deployment.previous_revision.exists`, `deployment.<env>.status` | a `deployment_result` |
| `metric.<name>` and bare `<name>` | a `metric_observation` |
| `service.health`, `service.<service>.health` | a `health_observation` |
| `approval.<id>.{granted,decision,by_human}` | an `approval` |
| `diff.{exists,files_changed,lines_added,lines_removed}`, `source_diff.exists` | a `diff` |
| `artifact.<kind>.{exists,count,approved,approved.count,<status>.count}` | the artifact graph |
| `artifact.<kind>.{schema_valid,sections_present,reviewed,current,relationship_valid}` | an `artifact` observation |
| `review.<subject-kind>.{result,approved,blocking_findings,by_human}`, `review.{result,approved}` | a `review` |
| `verification.<claim>.{status,passed}`, `<claim>_verified` | a `verification` |
| `specification.satisfied`, `specification.requirements.{total,satisfied}`, `specification.unsatisfied.count` | a `specification` |
| `state.current`, `state.<id>.entered`, `workflow.terminal` | the engine |
| `evidence.count.<kind>`, `evidence.first_seq.<kind>`, `evidence.last_seq.<kind>` | the engine |
| `evidence.missing`, `required_evidence.missing`, `approvals.granted`, `principle.<id>.active` | the engine |

`evidence.first_seq.<kind>` is submission order. It is how ordering rules become checkable:
`evidence.first_seq.test_result < evidence.first_seq.diff` says a test ran before any code changed.

## 3. Predicate syntax

Compact form, one per list item:

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

## 4. Requirement sets

Wherever `requires:` or `completion:` appears:

```yaml
requires:
  predicates:
    - tests.unit.failed == 0
  evidence:
    - test_result                    # shorthand
    - kind: test_result              # or the full form
      at_least: 1
      independent: true              # an agent's own assertion does not satisfy it
      verifier: test-runner
  artifacts:
    - kind: design
      status: approved
      fresh: true                    # default; excludes superseded/rejected
      relation: {kind: designs, target_kind: specification}
  reviews:
    - subject_kind: design
      result: approved               # approved | changes_requested | rejected
      human: true
      fresh: true                    # the review must cover the artifact's current version
  approvals:
    - security-review
  conditional:
    - when: {change.architectural: true}
      require:
        artifacts:
          - kind: architecture-design
            status: approved
          - kind: architecture-decision-record
```

A bare list under `requires:` is read as predicates. An unrecognised mapping key is read as a fact
predicate, so `requires: {change.architectural: true}` works — which also means a misspelt key
becomes an unobservable-fact error rather than being ignored.

## 5. Principle document

```yaml
id: test-driven                  # lower-case kebab-case
version: 1
title: Test-driven development
summary: >-
  One or two lines: what this enforces, and what goes wrong without it.
applies_when:                    # omitted means always
  task.kind: {any_of: [feature, bugfix]}
requires:                        # phase-keyed form
  before_implementation:
    - test.exists
  before_completion:
    - tests.unit.failed == 0
  during_verification:
    - ...
  always:
    - ...
evidence:                        # must exist by completion
  - kind: test_result
    independent: true
verification:                    # verifiers that must have spoken
  - verifier: test-runner
  - verifier: human-review
    subject_kind: design
    before: {phase: implementation}
capabilities:                    # a principle may only take away
  deny: [secret.read]
  require_approval: [production.write]
on_failure: block                # block | abort | {action: retry, max_attempts: 2, then: block}
                                 # | {action: escalate, to: oncall}
                                 # | {action: rollback, rollback: {require: [<predicate>]}}
```

The alternative `requires` form, when a rule is about a specific state:

```yaml
requires:
  before: {state: implement}     # or {phase: implementation}
  artifacts:
    - kind: specification
      status: approved
```

Rules:

* `before_<phase>` keys use `_` for `-` in phase names: `before_verification_setup` → phase
  `verification-setup`.
* Requirements with no stated timing default to **before completion**.
* A principle must enforce something: obligations, evidence, verification or a capability policy.
  A document with only a title is rejected.
* Every phase named must exist in the workflow the profile uses, or resolution fails.

## 6. Workflow document

```yaml
id: adp/default                  # namespaced with `/`; the last segment must not be a number
version: 1
title: Standard development workflow
summary: ...
initial: receive
states:
  receive:
    title: Receive
    summary: ...
    phases: [intake]
    requires: { ... }            # what must hold to enter this state
    capabilities: { ... }        # adjustments while here
    irreversible: false
    on_failure: block
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: verify
    to: review
    description: ...
    when:
      all:
        - tests.unit.failed == 0
        - static_analysis.errors == 0
    requires: { ... }
    on_failure: { ... }
allow_unreachable_states: false
```

Rules, all enforced:

* the initial state must exist; every `from`/`to` must exist;
* every non-terminal state needs at least one outgoing transition;
* every state must be reachable from `initial` unless `allow_unreachable_states: true`;
* at most one transition per `from`/`to` pair — combine guards with `any`;
* a state marked `irreversible: true` must not have a rollback failure policy;
* a rollback policy must state its precondition: `{action: rollback, rollback: {require: [...]}}`;
* a workflow whose states declare no `completion` phase will fail resolution, because obligations
  default to being owed before completion.

## 7. Profile document

```yaml
id: development.standard         # dotted kebab-case
version: 1
title: Standard development
summary: >-
  When to choose this over its neighbours.
protocol: adp/1                  # development profiles use adp/1, operations profiles aop/1
extends: development.fast        # optional; inherits workflow, principles, capabilities, completion
workflow: adp/default            # required unless inherited
principles: [spec-driven, test-driven]
without_principles: [mutation-testing]   # drop something inherited
capabilities:
  allow: [repository.read, repository.write, tests.execute]
  require_approval: [production.write]
  deny: [secret.read]
completion:
  all:
    - specification.satisfied
    - tests.unit.failed == 0
    - evidence.missing == 0
facts:                           # profile-level context facts
  risk: medium
```

Extending can only make completion harder: conditions are conjoined, and a principle may be added or
dropped but a denial cannot be granted back.

## 8. Artifact lifecycle document (`artifacts/lifecycles/<kind>.yaml`)

```yaml
kind: architecture-decision-record
initial: proposed
transitions:
  proposed: [accepted, rejected]
  accepted: [superseded]
  rejected: []
```

An artifact whose status is not in its kind's lifecycle is a validation error. A `superseded`
artifact must have a successor declaring `supersedes:` it.

## 9. Identifier rules

| Kind | Shape | Example |
|---|---|---|
| principle, phase, approval, claim | lower-case kebab | `test-driven` |
| profile, workflow | kebab segments joined by `.` or `/`, last segment not a number | `development.standard`, `adp/default` |
| state | kebab or snake segments | `adversarial_verify` |
| artifact id | `<namespace>:<name>` | `design:passkeys-auth` |

## 10. Checking your work

```console
python3 -c "import sys,yaml; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" <your files>
```

The full loader and cross-document validation run after all documents are in place; the reviewer will
report anything the brief could not catch. Do not modify files you do not own — if you spot a problem
in someone else's document, report it instead.
