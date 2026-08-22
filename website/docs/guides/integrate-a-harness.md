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

## First: you may not have to build one

A reference driver ships in this repository. `protocol drive` makes the engine's calls in order,
executes the three kinds of step a step map declares — a program, a model, a person — and records
what it did:

```console
$ protocol drive run --project . --map development/default \
    --plugin-dir integrations/claude-code --pause-on-approval
$ protocol drive status
$ protocol drive resume AUTH-142/3      # the run id `drive run` allocated
```

`drive run` needs a model and costs money. `drive status` reads the run directory and needs
nothing. `--map` is not optional in this tree: two step maps are written against `adp/default/1`, so
a `drive run` given neither is refused, naming both ids rather than picking the first
(`crates/protocol-cli/src/drive.rs:401-411`).

It evaluates no gate itself. A driver that could evaluate a gate would be a second protocol
implementation with none of the conformance suites behind it, and the first time the two disagreed
the one nobody tested would win. Everything it decides, it decides by asking the engine — which is
also the argument for reading the rest of this page before writing your own.

Two things the driver does not do, both of which land on you if you build one:

* **It only knows the harness it was written against.** The `llm` step launches Claude Code. A
  second harness needs an adapter, and the harness-neutrality claim has never met one — see
  [Limitations](../status/limitations.md).
* **It reads a step map, and the two shipped maps verify the two shapes of work this repository
  has.** `drivers/development/default.yaml` (`development/default`) names `cargo` in every state
  that names a verifier, so a repository whose tests are not Rust tests cannot satisfy `test-driven`
  under it. `drivers/development/checks.yaml` (`development/checks`) names no compiler and runs one
  command, `bash .engineering/checks/run.sh`, so a story whose acceptance is checks somebody can run
  drives under that one. A repository that is neither writes its own map: the workflow is unchanged,
  and only the steps under it are yours.

## The seven calls

Add `aep-engine` and `aep-domain` to your crate; `aep-schema` if you read tasks and manifests from
YAML, and `serde_json` if you persist executions.

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

A minimal loop, compiled as written against the workspace — the one warning is the `policy` binding,
which is there to be read rather than used:

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
    // `observed_at` is when the verifier looked — not when you got round to submitting.
    engine.submit_evidence(
        &mut execution,
        EvidenceSubmission::new(
            Evidence::TestResult(TestResult::failing(TestSuite::Unit, 0, 1)),
            Producer::Verifier { verifier: Verifier::TestRunner },
            ObservedAt::new(Timestamp::from_epoch_millis(1_699_785_600_000)),
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

The engine will record a `TestResult` you invented; nothing downstream can tell. That is exactly why
this is the harness's responsibility. Three mechanisms help, and none works if you route around
them:

* **Report the real producer.** `Producer::Agent { id }` and `Producer::Verifier { verifier }` are
  different variants, and `independent: true` requirements are not satisfied by the first.
* **Say when the verifier looked.** `EvidenceSubmission::new` takes `observed_at` as its third
  argument and has no default for it, which is a deliberate refusal: a caller who has to write down
  when they looked is a caller who cannot accidentally claim they looked just now. The same rule
  holds at the document boundary — a record with no observation time is not parsed, and one dated
  ahead of the clock is refused rather than recorded:

  ```console
  $ protocol evaluate --task task.yaml --artifacts artifacts.yaml --evidence no-observed-at.yaml
  error: evidence document (no-observed-at.yaml): .[0]: missing field `observed_at` at line 1 column 3   # exit 1

  $ protocol evaluate --task task.yaml --artifacts artifacts.yaml --evidence dated-2099.yaml
  error: submitting evidence from dated-2099.yaml: the observation time 4070908800000ms is in the
  future; it is 1787352723812ms                                                                          # exit 1
  ```
* **Fill in provenance** — `command`, `tool`, `revision`, `workspace`, `environment`, `digest`,
  `inputs` — through `EvidenceSubmission::with_provenance`, so the record can be re-derived by
  someone who does not trust you. `with_subject` says what it is about; `stored_as` points at the
  entity a backend holds it as, so the audit trail points at the record rather than at a copy.

Submit evidence **as you observe it**. Ordering is recorded (`evidence.first_seq.<kind>`) and rules
read it; batching a task's evidence at the end destroys the ordering facts, and the failure looks
like a broken rule.

### 2. Map capabilities onto the tools you actually have

`capabilities()` returns the policy in force in the current state. Every `Action` maps to exactly
one `Capability` (`Action::required_capability`, `crates/aep-domain/src/action.rs:252`), so
authorisation is a lookup:

| Action | Capability |
|---|---|
| `RepositoryRead` / `RepositoryWrite` | `repository.read` / `repository.write` |
| `TestExecute` | `tests.execute` |
| `CommandExecute`, `ToolInvoke` | `command.execute` |
| `NetworkRequest { intent }` | `network.read` or `network.write`, by intent |
| `TelemetryQuery` | `telemetry.read` |
| `Deploy { environment }` | `deployment.create[:environment]` |
| `Rollback { environment }` | `deployment.rollback[:environment]` |
| `ProductionMutate` | `production.write` |
| `SecretRead` | `secret.read` |
| `ArtifactWrite` | `artifact.write` |
| `ReviewRequest` | `review.request` |
| `ApprovalRequest` | `approval.request` |

The mapping is total in one direction only. Four capabilities have no `Action` that reaches them —
`artifact.read`, `production.read`, `planning.read` and `planning.write` — so a policy can grant or
deny them and `authorize` will never be asked about one. If your harness gates reads, gate them
yourself against `capabilities()`; do not expect a `Decision`.

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

## Two layers of enforcement, and what each one cannot see

`capabilities()` says what may be done. Turning that into a session has two layers, because they
fail differently:

| Layer | Sees | Blind to |
|---|---|---|
| the tool set the session is launched with | which tools exist at all | every argument — a `Bash` tool is a `Bash` tool whatever the command is |
| a pre-tool hook | the call's **arguments** | nothing about workflow state, unless you hand it some |

The reference driver runs both. It derives the tool set per state, not per run, from
`tool_config(&effective_policy(execution))` (`crates/aep-driver/src/run.rs:683`), and passes the
plugin directory into every model session with `--plugin-dir` (or `AEP_DRIVE_PLUGIN_DIR`), because a
session that never loaded the plugin never loaded the hooks. Every session also carries
`--strict-mcp-config`, so a session's MCP surface is what that line gave it, which is nothing: an
account's MCP servers arrive with the login rather than out of a file, and a scratch config
directory cannot exclude them. It also writes `step-context.json` into the run directory, which is
how a hook — a separate process, holding no execution — learns which state it is in:

```json
{
  "format": "aep.drive-step-context/1",
  "state": "establish_verifiers",
  "step_index": 0,
  "attempt": 1,
  "shell_offered": true,
  "capabilities": ["repository.read", "repository.write", "tests.execute", "command.execute",
                   "artifact.read", "artifact.write", "review.request", "approval.request"],
  "tools": ["Bash", "Edit", "Glob", "Grep", "NotebookEdit", "Read", "Skill", "Write"],
  "reaching": [
    "-> implement: guard: test.exists",
    "-> implement: ? test.exists — unobserved: test.exists [principle test-driven]",
    "-> implement: ? test.first_result == failed — unobserved: test.first_result [principle test-driven]"
  ]
}
```

(Two absolute paths — the run directory and the store — are dropped, and the arrays are broken
across lines to fit the page; the rest is the file's, except the `reaching` lines, which are what
`protocol evaluate --advance` prints under `transitions` for that transition — the driver and the
CLI both read `TransitionEvaluation::unmet()`.) `reaching` is one line per requirement that does not
hold yet on a way *out* of the state, each prefixed with where that transition goes. What must hold
*while in* it is a different list and is passed separately, because a step given only the second can
satisfy every line it was handed and still be refused on the way out.

Every adjudicated call, allow and deny alike, is appended to `hook-decisions.jsonl` in the same
directory. On this repository's first governed run: 80 decisions, 69 allow and 11 deny. A guard that
denies everything audits as little as one that denies nothing, so both halves are the point.

Three limits worth knowing before you copy the shape:

* **Hooks deny; they never grant.** The narrow fix for "let the model reach one CLI and no other
  program" would be a scoped capability, and the grammar cannot express it — scoping exists for one
  thing, an environment on `deployment.create` and `deployment.rollback`. So the pattern is a
  capability grant plus a hook constraint, and the constraint is pattern-based and best-effort
  rather than a function of the capability.
* **A hook's decision is not in the audit trail.** It is in the log and nowhere else. A `PreToolUse`
  hook is a separate process and cannot call `authorize`, which mutates an in-memory execution.
  Folding the log in would add provenance and not enforcement — every decision the log has ever held
  is a refusal, and a refusal changes no engine state. What closes it is an `authorize` ingestion
  that keeps the hook as the deciding party, together with the first case where a hook's decision
  would change what the engine does.
* **The launched tool set is not audited from the transcript.** A Claude Code `SessionStart` event
  lists the harness's tool *inventory*, not the session's allow rules. The committed fixture
  `crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl` lists **thirty-two** tools in its init
  event; the gap register records that it was launched with nine. A transcript check can rule out
  *"the tool did not exist"* as an explanation for a refusal; it cannot confirm the allowlist you
  passed.

## Persisting and replaying an execution

`Execution::snapshot()` returns a serialisable `Snapshot`: current state, states entered, evidence in
submission order, the event stream, the actor. It deliberately does **not** hold the plan — the plan
is re-resolved from the documents on restore, so an execution cannot keep enforcing last month's
rules after the documents changed. `Engine::restore(task, artifacts, snapshot)` refuses a snapshot
whose task does not match, and re-observes at the *restoring* engine's clock, so an evidence horizon
is decided against the present rather than against the moment the snapshot was taken.

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

## Checking the run afterwards

A harness that reports its own conformance is a harness reporting on itself. `protocol trace check`
reads the transcript the harness wrote and decides it against a typed specification, so *"the agent
consulted the CLI before touching the store"* becomes a verdict a program produced. `protocol trace
evidence` mints that verdict as a `trace_conformance` record the engine accepts, with
`producer: verifier / trace-checker` — see [Check what an agent run did](./check-a-transcript.md).
