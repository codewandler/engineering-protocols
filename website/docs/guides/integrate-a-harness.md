---
title: Integrate an agent harness
sidebar_position: 3
description: The engine's seven calls, the three rules a harness must get right, and how to persist and replay an execution.
---

# Integrate an agent harness

For the person building the thing that runs the agent. The engine does no work of its own: it holds
no tools, calls no model, touches no repository. It answers seven questions, and every answer is a
function of the validated documents plus the evidence you submitted — never of anything it observed
itself.

Add `aep-engine` and `aep-domain` to your crate; `aep-schema` if you read tasks and manifests from
YAML, and `serde_json` if you persist executions.

## The seven calls

| Call | Answers | Returns |
|---|---|---|
| `initialize(task)` | what is this task held to? | `Execution`, positioned at the workflow's initial state |
| `requirements(&execution)` | what is owed *here*, and by which document? | `Vec<Requirement>` |
| `capabilities(&execution)` | what may be done at all? | `CapabilityPolicy` — allow / approval required / deny |
| `authorize(&mut execution, &request)` | may this specific action proceed? | `Decision`, with the rule that produced it |
| `submit_evidence(&mut execution, submission)` | record what a verifier found | `EvidenceId`, or a rejection if the protocol does not declare that kind |
| `evaluate(&execution)` | what is permitted now, what is missing, is it finished? | `Evaluation` |
| `transition(&mut execution)` | move | `Moved` / `Completed` / `Blocked { reasons }` |

`initialize` takes ownership of the task, so nothing can mutate it under an execution already being
evaluated. `authorize` takes `&mut` because asking is itself an event: the request and its answer
both land in the audit trail, including denials.

A minimal loop (compiles as written against the workspace):

```rust
use std::fs;
use std::path::Path;

use aep_domain::action::{Action, ActionRequest, RepositoryWrite};
use aep_domain::evidence::{Evidence, Producer, TestResult, TestSuite};
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

    // Submit what a verifier produced. The producer is the verifier's, not yours.
    engine.submit_evidence(
        &mut execution,
        EvidenceSubmission::new(
            Evidence::TestResult(TestResult::failing(TestSuite::Unit, 0, 1)),
            Producer::Verifier { verifier: Verifier::TestRunner },
        ),
    )?;

    match engine.transition(&mut execution)? {
        TransitionResult::Moved { from, to, .. } => println!("{from} -> {to}"),
        TransitionResult::Completed { state } => println!("complete in {state}"),
        TransitionResult::Blocked { state, .. } => {
            println!("blocked in {state}");
            print!("{}", engine.explain_completion(&execution)); // verbatim, to the user
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

## The three rules a harness must get right

### 1. Never manufacture evidence

The engine will happily record a `TestResult` you invented; nothing downstream can tell. That is
exactly why this is the harness's responsibility. Two mechanisms help, and neither works if you
route around them:

* **Report the real producer.** `Producer::Agent { id }` and `Producer::Verifier { verifier }` are
  different variants, and `independent: true` requirements are not satisfied by the first.
* **Fill in provenance** — command, tool, revision, workspace, environment, digest, inputs — so the
  record can be re-derived by someone who does not trust you.

Submit evidence **as you observe it**. Ordering is recorded
(`evidence.first_seq.<kind>`) and rules read it; batching a task's evidence at the end destroys the
ordering facts, and the failure looks like a broken rule.

### 2. Map capabilities onto the tools you actually have

`capabilities()` returns the policy in force in the current state. Every `Action` maps to exactly
one `Capability`, so authorisation is a lookup:

| Action | Capability |
|---|---|
| `RepositoryRead` / `RepositoryWrite` | `repository.read` / `repository.write` |
| `TestExecute` | `tests.execute` |
| `CommandExecute`, `ToolInvoke` | `command.execute` |
| `NetworkRequest { intent }` | `network.read` or `network.write`, by intent |
| `Deploy { environment }` | `deployment.create[:environment]` |
| `ProductionMutate` | `production.write` |
| `SecretRead` | `secret.read` |

Do the mapping once at tool-registration time and check before each call. A tool with no `Action` to
describe it is a tool the protocol cannot govern. To find gaps at plan time rather than mid-task,
`aep_engine::engine::kinds_for_verifier(&verifier)` says which evidence kinds each verifier class
can produce — which is how you learn you have no contract runner before the transition that needs
one.

### 3. Route the three truth values to different behaviour

Only `True` permits a transition; `False` means observed-and-wrong, `Unknown` means
nobody-observed-it. They want opposite responses:

```rust
use aep_domain::predicate::Truth;
use aep_engine::{Engine, Execution, ProtocolEngine};

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

Collapsing `Unknown` into `False` produces an agent that tries to fix code nobody has tested;
collapsing it into `True` produces one that finishes tasks nothing verified.

## Persisting and replaying an execution

`Execution::snapshot()` returns a serialisable `Snapshot`: current state, states entered, evidence
in submission order, the event stream, the actor. It deliberately does **not** hold the plan — the
plan is re-resolved from the documents on restore, so an execution cannot keep enforcing last
month's rules after the documents changed. `Engine::restore(task, artifacts, snapshot)` refuses a
snapshot whose task does not match.

For replay, construct the engine with `Engine::with_clock(registry, FixedClock::new(millis))` — with
an injected clock the event stream reproduces exactly, which is what makes an audit trail diffable.
`SystemClock` is the default and is right in production.

## The audit trail

`aep_engine::trail` joins what the protocol decided with what the storage contract records, so a
refusal by the protocol and a refusal by a backend land in the same queryable trail:

| Function | Use |
|---|---|
| `audit_trail(&execution)` | every `AuditRecord` the event stream implies, in order |
| `decision_record(&execution, &decision, at)` | one authorisation decision as a record |
| `command_context(&execution, request_id, idempotency_key, at)` | the `CommandContext` a command issued during this execution should carry |
| `correlation_id(&execution)` | the execution id, doubling as the correlation id |

`command_context` takes the request id and idempotency key from you, because only you know whether
this is a first attempt or a retry.

## When a transition blocks, show the explanation verbatim

`CompletionExplanation` is the protocol's own account — one line per requirement, each naming the
document that asked for it, already written for a person. Summarising it into "some checks failed"
throws away the only part that tells anyone what to do. For machine consumers, every explanation
serialises; show text to people and JSON to programs, and do not invent a third rendering.
