# Wiring a harness to the engine

For someone building the thing that runs the agent. The engine does no work: it holds no tools, calls
no model, touches no repository. It answers seven questions, and every answer is a function of the
validated documents plus the evidence you submitted — never of anything it observed for itself.

Add `aep-engine` and `aep-domain`; `aep-schema` if you read tasks and manifests from YAML, and
`serde_json` if you persist executions.

## Before you write one: there is a reference driver

`protocol drive run|status|resume` walks a workflow instead of suggesting it, and it is worth
reading before you build the same loop again — either as the thing you use, or as the worked
answer to the questions below.

What it does not do is the interesting part. **It evaluates no gate.** A driver that could evaluate
a gate would be a second protocol implementation with none of the conformance suites behind it, and
the first time the two disagreed the untested one would win. It makes the engine's calls in order,
executes the three kinds of step that touch the world, and records what happened.

| It supplies | You supply |
|---|---|
| the loop: initialize, ask, act, submit, transition | the model, the shell and the person |
| a **step map** — what a harness *does* in each state — as the fifth document kind, under `drivers/` | a step map for your repository, or the shipped `drivers/development/default.yaml` |
| a run directory under `.engineering/runs/<run-id>/`, and a store lock with a liveness probe | nothing; `resume` re-takes it |
| three step kinds: `llm` (a model session), `command` (a program) and `operator` (a person) | which of them each state needs |

The step map is where the split lands: a workflow says which states exist and what evidence each
transition needs, and deliberately does not say how to obtain it — that is what lets one workflow
govern a Rust repository and a Terraform one. A step map is pinned to a workflow *version*, and an
orphaned pin is refused at load rather than applied to a state graph it was never written against.

One property is worth copying whether or not you use the driver: **an `llm` step has no `evidence:`
key and cannot be given one.** Anything a model is supposed to have achieved that is checkable is
observed by the command step after it, so `producer: verifier` is a fact about who ran the suite
rather than the model's opinion of a suite it says it ran. That is a type doing the work a rule
would otherwise have to.

Enforcement is one policy with one enforcer, since `epic:metaharness-migration` (2026-08-22):
every `llm` step is spawned through `metaharness run claude` in ask mode, and the driver's own
per-call policy — `decide_tool` in `crates/protocol-cli/src/drive.rs`, the retired shell hooks
ported to Rust plus the per-state allowlist — answers each `tool.requested` event before the call
runs. It is the only layer that sees a call's **arguments**, and its decisions are `tool.decided`
events in the run's own event stream rather than a side-channel log. Afterwards the record says
whether it held — a verdict a program reads from the stream, not one the agent reported about
itself.

`protocol workflow render --run <id>` draws where a run got to, what it produced and why it
stopped, with the engine's own sentences on the arrows.

## The loop

| Call | Answers | Returns |
|---|---|---|
| `initialize(task)` | What is this task held to? | `Execution`, positioned at the workflow's initial state |
| `requirements(&execution)` | What is owed *here*, and by which document? | `Vec<Requirement>` |
| `capabilities(&execution)` | What may be done at all? | `CapabilityPolicy` — `allow` / `approval_required` / `deny` |
| `authorize(&mut execution, &request)` | May this specific action proceed? | `Decision`, with the rule that produced it |
| `submit_evidence(&mut execution, submission)` | Record what a verifier found | `EvidenceId`, or a rejection if the protocol does not declare that kind |
| `evaluate(&execution)` | What is permitted now, what is missing, is it finished? | `Evaluation` |
| `transition(&mut execution)` | Move | `Moved` / `Completed` / `Blocked { reasons }` |

`initialize` takes ownership of the task, so nothing can mutate it out from under an execution that
is already being evaluated against it. `authorize` takes `&mut` because asking is itself an event:
the request and its answer both land in the trail, including the denials.

```rust
use std::fs;
use std::path::Path;

use aep_domain::action::{Action, ActionRequest, RepositoryWrite};
use aep_domain::evidence::{Evidence, Producer, TestResult, TestSuite};
use aep_domain::time::{ObservedAt, Timestamp};
use aep_domain::verification::Verifier;
use aep_engine::{
    audit_trail, load_tree, DecisionExplanation, Engine, EvidenceSubmission, ProtocolEngine,
    Registry, TransitionResult,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The documents in force. One tree, loaded once, shared by every execution.
    let registry: Registry = load_tree(Path::new("."))?;
    let engine = Engine::new(registry);

    let task = aep_schema::parse::task(&fs::read_to_string("task.yaml")?, None)?;
    let artifacts =
        aep_schema::parse::artifact_manifest(&fs::read_to_string("artifacts.yaml")?, None)?;
    let mut execution = engine.initialize_with_artifacts(task, artifacts)?;

    // What is owed here. Each line names the document that asked for it.
    for requirement in engine.requirements(&execution) {
        println!("{}", requirement.line());
    }

    // What may be done. Expose exactly these as tools, and nothing else.
    let policy = engine.capabilities(&execution);
    for capability in &policy.allow {
        println!("allowed           {capability}");
    }
    for capability in &policy.approval_required {
        println!("requires_approval {capability}");
    }

    // Ask before acting, never after.
    let request = ActionRequest::new(Action::RepositoryWrite(RepositoryWrite {
        paths: vec!["src/auth/passkey.rs".to_owned()],
        intent: Some("implement the credential store".to_owned()),
    }));
    let decision = engine.authorize(&mut execution, &request);
    if !decision.is_allowed() {
        println!("{}", DecisionExplanation::from(&decision));
        return Ok(());
    }

    // Submit what a verifier produced. The producer is the verifier's, not yours, and
    // `observed_at` is when the run actually happened — not when you got round to submitting it.
    engine.submit_evidence(
        &mut execution,
        EvidenceSubmission::new(
            Evidence::TestResult(TestResult::failing(TestSuite::Unit, 0, 1)),
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            ObservedAt::new(Timestamp::from_epoch_millis(1_699_999_940_000)),
        ),
    )?;

    match engine.transition(&mut execution)? {
        TransitionResult::Moved { from, to, .. } => println!("{from} -> {to}"),
        TransitionResult::Completed { state } => println!("complete in {state}"),
        TransitionResult::Blocked { state, .. } => {
            println!("blocked in {state}");
            // Verbatim, to the user. See below.
            print!("{}", engine.explain_completion(&execution));
        }
    }

    for record in audit_trail(&execution) {
        println!("{} {}", record.audit_id, record.kind.as_str());
    }

    let snapshot = execution.snapshot();
    fs::write("execution.json", serde_json::to_vec_pretty(&snapshot)?)?;
    Ok(())
}
```

`Moved` carries `also_permitted`: when more than one transition was legal, the first in document
order is taken and the rest are reported, so a workflow author sees that a choice existed rather than
watching a coin flip.

## Three things to get right

### 1. Never manufacture evidence

The engine will happily record a `TestResult` you invented. Nothing downstream can tell — which is
exactly why this is the harness's job and not the engine's. Two mechanisms exist to help, and neither
works if you route around them:

* `Producer` is part of every submission. `Producer::Agent { id }` and `Producer::Verifier {
  verifier }` are different, and an evidence requirement marked `independent: true` is not satisfied
  by the first. Report what actually produced the observation.
* Evidence of a kind the protocol does not declare is refused at submission with
  `ProtocolError::EvidenceRejected`, listing the kinds that are declared. Fill in `Provenance` —
  `command`, `tool`, `revision`, `workspace`, `environment`, `digest`, `inputs` — so the record can be
  re-derived later by someone who does not trust you.

* `observed_at` is yours and is required. It is when the verifier ran, not when you got round to
  submitting the record, and there is no default — a caller who has to write the date down cannot
  back-date by omission. A submission claiming a time in the future is refused rather than accepted
  as an unusually fresh record.

Order matters too, and the engine records it: `evidence.first_seq.test_result <
evidence.first_seq.diff` is how red-before-green is checked. Submit as you observe. Batching a task's
evidence at the end destroys the ordering facts, and the failure looks like a broken rule.

A requirement may also carry a `horizon`, and then the date does real work: past it the observation
stops counting and the requirement reads `Unknown` again. Nothing you can call extends a horizon —
the only refresh is to run the verifier again and submit a record with a newer `observed_at`. A
harness that re-submits an identical record to quiet a lapsed gate will find the gate still lapsed,
which is the intended answer.

### 2. Map capabilities onto the tools you actually have

`capabilities()` returns the policy in force in the current state — the profile's, adjusted by the
state, and never granted past a `deny`. Every `Action` maps to exactly one `Capability`, so
authorisation is a lookup rather than a judgement:

| Action | Capability |
|---|---|
| `RepositoryRead` / `RepositoryWrite` | `repository.read` / `repository.write` |
| `TestExecute` | `tests.execute` |
| `CommandExecute`, `ToolInvoke` | `command.execute` |
| `NetworkRequest { intent }` | `network.read` or `network.write`, by intent |
| `Deploy { environment }` | `deployment.create[:environment]` |
| `ProductionMutate` | `production.write` |
| `SecretRead` | `secret.read` |

Do the mapping once, at tool-registration time, and check before each call. A capability no document
mentions is not granted — the default is deny — so a tool with no `Action` to describe it is a tool
the protocol cannot govern. Ask what evidence the profile will need before the task starts:
`aep_engine::engine::kinds_for_verifier(&verifier)` says which evidence kinds a given verifier class
can produce, which is how you find out you have no contract runner at plan time rather than at the
transition that needs one.

### 3. `Unknown` is not `False`

Predicate evaluation is three-valued and only `True` permits a transition. The two failing values
mean opposite things and want opposite responses:

| Truth | What happened | What the agent should do next |
|---|---|---|
| `True` | Observed, and it holds | nothing |
| `False` | Observed, and it is wrong | fix the code |
| `Unknown` | Nothing observed it, or nobody has looked since its horizon | go and run the verifier that would |

Collapsing `Unknown` into `False` produces an agent that tries to fix code nobody has tested.
Collapsing it into `True` produces one that finishes tasks nothing verified.

A lapsed observation lands in the third row on purpose: an old green result is not a wrong answer,
so a harness that treated it as `False` would send an agent to repair working code. The facts a
lapsed record projected are withheld along with it, so a guard reading `tests.unit.failed == 0` off
a stale suite refuses instead of passing on it. `evidence.lapsed` counts the records in that state
and sits beside `evidence.missing`, because *nobody produced it* and *somebody did and nobody has
looked since* want different next moves from the agent.

```rust
use aep_domain::predicate::Truth;
use aep_engine::{Engine, Execution, ProtocolEngine};

/// What a harness does with each of the three truth values.
fn next_step(engine: &Engine, execution: &Execution) -> Vec<String> {
    let mut actions = Vec::new();
    for requirement in engine.evaluate(execution).completion {
        match requirement.outcome.truth {
            Truth::True => {}
            Truth::Unknown => actions.push(format!("observe: {}", requirement.outcome.requirement)),
            Truth::False => actions.push(format!("fix: {}", requirement.outcome.requirement)),
        }
    }
    actions
}
```

The CLI prints the same distinction as `✓` / `✗` / `?`, and the detail line says which it is —
`tests.unit.failed = 1` against `unobserved: static_analysis.errors`.

## Persisting an execution

`Execution::snapshot()` returns a `Snapshot`: `serde`-serialisable, holding the current state, the
states entered, evidence in submission order, the event stream and the actor. What it deliberately
does **not** hold is the plan.

```rust
use aep_engine::{Engine, Execution, Snapshot};

/// Restoring an execution: the plan is re-resolved, never stored.
fn resume(
    engine: &Engine,
    task_yaml: &str,
    manifest_yaml: &str,
    snapshot: Snapshot,
) -> Result<Execution, Box<dyn std::error::Error>> {
    let task = aep_schema::parse::task(task_yaml, None)?;
    let artifacts = aep_schema::parse::artifact_manifest(manifest_yaml, None)?;
    Ok(engine.restore(task, artifacts, snapshot)?)
}
```

The plan is derived from the documents. A snapshot carrying its own copy would outlive a change to
them without anyone noticing — an execution still enforcing last month's rules while the repository
says otherwise. Restoring re-resolves, and refuses a snapshot whose task does not match the plan.

For replay, construct the engine with `Engine::with_clock(registry, FixedClock::new(millis))`. With
an injected clock the event stream reproduces exactly, which is what makes an audit trail diffable.
`SystemClock` is the default and is right in production.

## Reaching the audit trail

`aep_engine::trail` is the join between what the protocol decided and what the storage contract
records, so a refusal by the protocol and a refusal by a backend land in the *same* trail, queryable
the same way.

| Function | Use |
|---|---|
| `audit_trail(&execution)` | every `AuditRecord` the event stream implies, in order |
| `decision_record(&execution, &decision, at)` | one authorisation decision as a record |
| `command_context(&execution, request_id, idempotency_key, at)` | the `CommandContext` a command issued during this execution should carry |
| `correlation_id(&execution)` | the execution id, doubling as the correlation id |

Denials become records too. "An agent tried to change production and was stopped, by this rule" is
only worth anything written down next to the changes that succeeded. Bookkeeping events — entering a
state, resolving a profile — deliberately do not become audit records: nothing was decided and
nothing changed, and a trail padded with them is a trail nobody reads.

`command_context` takes the request id and idempotency key from you, because only you know whether
this is a first attempt or a retry. Guessing would defeat the point of both.

## What you owe the user when a transition is blocked

Show `CompletionExplanation` verbatim. It is the protocol's own account of the situation, one line
per requirement, each naming the document that asked for it, and it is already written for a person.
Summarising it into "some checks failed" throws away the only part that tells anyone what to do.

```console
$ $B evaluate --task examples/development-passkeys/task.yaml \
    --artifacts examples/development-passkeys/artifacts.yaml \
    --evidence examples/development-passkeys/evidence/01-red-test.yaml \
    --evidence examples/development-passkeys/evidence/02-implementation.yaml \
    --evidence examples/development-passkeys/evidence/03-verification.yaml \
    --evidence examples/development-passkeys/evidence/04-review.yaml \
    --advance
state       adversarial_verify (Adversarial verify)
owed here
  ✓ evidence test_result from test-runner (independent) [state adversarial_verify]
transitions
  adversarial_verify -> review [blocked]
      guard: evidence.missing == 0
Task incomplete in `adversarial_verify`:
  ✗ (tests.unit.failed == 0 and static_analysis.errors == 0 and evidence.missing == 0)  [completion]
      evidence.missing = 1
  ✓ (specification.satisfied and contracts.failed == 0)           [completion]
  ✓ specification.satisfied                                       [principle spec-driven]
  ✓ tests.unit.failed == 0                                        [principle test-driven]
  ...
  ? evidence verification (independent)                           [principle provenance-tracking]
      0 of 1 required record(s) submitted
```

Twenty-two of twenty-four conditions hold; the `✗` is the aggregate reporting `evidence.missing = 1`
and the `?` says which record is missing. Nobody has observed it yet — that is different from
something being broken, so the next move is to go and get an independent statement, not to change any
code. That is a sentence a user can act on; "blocked" is not.

For a machine consumer, every one of these commands takes `--format json` or `--format yaml`, and
`CompletionExplanation`, `Decision` and `Evaluation` all serialise. Show the text to people and the
JSON to programs — do not invent a third rendering.
