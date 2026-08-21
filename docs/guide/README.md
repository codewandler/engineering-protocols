# Adopter's guide

`engineering-protocols` turns the rules an engineering team already works by — write the spec first,
watch the test fail before you implement, get a human to approve a production change — into typed
documents a program can execute. A task resolves against those documents into a plan: which rules are
in force, what the agent may do, what evidence is owed and what counts as finished. The agent still
reasons; the protocol decides what the resulting facts permit.

## What is not built yet

From the status report, [`docs/status.md`](../status.md). Everything else there is at 100% and
gated by `task check`.

| Missing | Consequence for you |
|---|---|
| A durable backend | A durable store exists — [`aep-backend-markdown`](../../crates/aep-backend-markdown/) keeps planning artifacts as files — but it is not an implementation of the storage contract, so the only thing the suites have ever certified is [`aep-backend-memory`](../../crates/aep-backend-memory/), which forgets everything when the process exits. [`backend.md`](backend.md) is about writing one that does not, and proving it. |
| A remote conformance runner | `protocol conformance` runs the suites against the in-memory backend. Proving *your* backend means calling `aep_conformance::run` from your own test suite. |
| Federated artifact graphs | An artifact manifest describes one project. Cross-repository architecture ([`consolidated-design-v0.2.md`](../design/consolidated-design-v0.2.md) §92) resolves references by hand today. |
| An attestation behind `independent: true` | The engine checks that the producer is not the agent under review. Nothing signs the record, so which producers you let write one is your harness's decision, not the protocol's. |

## Which guide

| Guide | For you if |
|---|---|
| [`adopting.md`](adopting.md) | You have engineering rules you want enforced, and a repository to put them in |
| [`harness.md`](harness.md) | You are building an agent harness and want the protocol to decide what it may do |
| [`backend.md`](backend.md) | You are storing engineering entities — designs, reviews, approvals — and want them to survive an audit |
| [`specification.md`](specification.md) | You want a system's contracts, tests and documentation derived from one document instead of maintained beside it |

For the full document vocabulary — every capability, evidence kind, fact path and predicate operator —
see the [document authoring brief](../plan/document-authoring-brief.md). This guide is the narrative
version; the brief is the reference.

## The shortest thing that works

Build the CLI, check the documents, watch a refusal.

```console
$ cargo build -p protocol-cli
$ B=target/debug/protocol
$ $B validate
44 file(s): 3 protocol(s), 22 principle(s), 4 workflow(s), 6 profile(s), 8 lifecycle(s), 1 step map(s)
valid
```

`validate` is not a schema check. It refuses a predicate reading a fact nothing observes, a workflow
state nothing can reach or leave, and a rollback policy that cannot say what it rolls back to — the
ways a rule ends up looking enforced and doing nothing.

Now resolve the [worked example](../../examples/development-passkeys/) — one task, one profile,
nothing else stated:

```console
$ $B resolve --task examples/development-passkeys/task.yaml
inputs      . and examples/development-passkeys/task.yaml
task        AUTH-142 (feature)
objective   add-passkey-support
protocol    adp/1
profile     development.standard
workflow    adp/default (initial: receive)
principles  spec-driven, test-driven, static-analysis, least-privilege, provenance-tracking, contract-testing, property-based-testing, approval-gates, reversible-changes
obligations 10
capabilities
  allowed            approval.request
  allowed            artifact.read
  allowed            artifact.write
  requires_approval  deployment.create
  requires_approval  deployment.create:production
  requires_approval  network.write
  requires_approval  production.write
  allowed            repository.read
  allowed            repository.write
  allowed            review.request
  denied             secret.read
  allowed            tests.execute
```

The task names two things. The nine principles, the workflow and the twelve capability decisions are
derived, so none of them can drift out of step with the profile.

Ask whether the agent may change production:

```console
$ $B explain --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --action production.write
production.write denied
  operation: change production state
  reason:    principle approval-gates rule production-write-requires-approval
  missing:   approval for capability production.write
  state:     receive
$ echo $?
1
```

The refusal names a principle someone can go and read, and says what would unlock it. Nobody had to
write that denial into the task or the profile: `approval-gates` is in force because
`development.standard` includes it, and the rule it applied has a name.

Then walk the task on its evidence:

```console
$ $B evaluate --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/01-red-test.yaml \
    --advance
state       implement (Implement)
transitions
  implement -> verify [blocked]
      guard: diff.exists
Task incomplete in `implement`:
  ✗ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
      tests.unit.failed = 1; unobserved: static_analysis.errors; evidence.missing = 7
  ? specification.satisfied                                       [principle spec-driven]
      unobserved: specification.satisfied
  ...
```

One failing test and an approved specification are enough to reach implementation — and only a
failing one. `✗` and `?` mean different things and want different responses: `✗` is a fact that is
wrong, `?` is a fact nobody has observed. Submit all five evidence files and the same command reaches
`complete`.

Every record above carries an `observed_at`, and a requirement may put a horizon on it — after which
the fact goes back to `?` rather than to `✗`, because an old answer is not a wrong one.
[`adopting.md`](adopting.md) § *A rule with a clock on it* is the worked version.
