# Wave 1 — the execution core

Goal: **a runnable end-to-end path.** At the end of this wave you can hand the CLI a task, a document
tree and a stream of evidence, and be told — deterministically, with reasons — what the agent may do,
what it still owes, which transition is permitted, and whether the task is complete.

Nothing in this wave needs the entity layer, the command/query contract or conformance (waves 2–4).
It builds only on `aep-domain` and `aep-schema`, which are done.

Projected status after the wave: **≈30% → ≈62%** (`aep-engine` 15% + documents 10% + `protocol-cli`
7%, using the README's weights).

## Dependency order

```text
W1.1 registry ──┬─▶ W1.3 resolve ──▶ W1.4 execution ──▶ W1.5 evaluate/policy/explain ──▶ W1.6 engine
W1.2 loader  ───┘                                                                             │
                                                                                              ▼
W1.7 documents (independent; only needs the schemas) ───────────────────────────▶ W1.9 examples + e2e
W1.8 CLI (needs W1.6 for evaluate/explain; validate/inspect need only W1.2) ─────────┘
W1.10 CI (independent)
```

W1.7 and W1.10 can be written at any point; everything else is a chain.

---

## W1.1 `aep-engine::registry` — the documents in force

`Registry` holds validated protocols, principles, workflows, profiles and artifact lifecycles, indexed
by their **declared** id (never by filename), and resolves references including `extends` chains.

Cross-document validation, which is where design §66 lives — each check gets a test asserting its
`ValidationCode`:

| Check | Code |
|---|---|
| referenced principle / workflow / profile / protocol exists | `unknown_principle`, `unknown_workflow`, `unknown_profile`, `unknown_protocol` |
| pinned major version matches the registry entry | `version_mismatch` |
| protocol major version is implemented by this build | `unsupported_protocol_version` |
| every capability a profile mentions is declared by the protocol | `undeclared_capability` |
| every evidence kind a requirement mentions is declared | `undeclared_evidence_kind` |
| every declared evidence kind has a verifier that can establish it | `no_verifier_for_evidence` |
| `extends` cycles between protocols or profiles | `unknown_protocol` / `unknown_profile` |

Deliverable: `registry.rs`, ~15 tests.

## W1.2 `aep-engine::load` — reading a document tree

Walks `protocols/`, `principles/`, `workflows/`, `profiles/`, `artifacts/lifecycles/` and builds a
`Registry`, reporting **every** bad file with its path rather than stopping at the first.

Deliverable: `load.rs`, plus an integration test that loads this repository's own document tree and
asserts it is clean — which makes the document set self-testing from W1.7 onwards.

## W1.3 `aep-engine::resolve` — `Task` + `Registry` → `ExecutionPlan`

The heart of the wave. In order:

1. Resolve the protocol, merging its `extends` chain.
2. Resolve the profile, merging its `extends` chain.
3. Apply the task's `principle_overrides`: add, remove, and **record every drop** on the plan, so
   "we turned mutation testing off for this one" stays visible afterwards.
4. Filter principles by applicability against the task's facts. A principle whose condition cannot be
   evaluated stays in force.
5. Compose the capability policy: protocol vocabulary → profile grants → each principle *restricts* →
   task constraints restrict. Record a `CapabilityGrant` per entry naming the document responsible,
   so a denial can be explained without re-deriving it.
6. Collect obligations from every principle in force.
7. Assemble the completion requirement set.
8. Cross-check, each with a test:

| Check | Code |
|---|---|
| an obligation is timed against a phase no state declares | `unknown_phase` |
| an obligation is timed against a state the workflow does not have | `unknown_state` |
| a completion or transition predicate reads a fact the protocol does not declare observable | `unobservable_fact` |
| a capability the task needs is denied by policy | `capability_conflict` |
| production mutation is granted with no approval requirement while `approval-gates` is in force | `production_write_without_approval` |

Deliverable: `resolve.rs`, ~20 tests.

## W1.4 `aep-engine::execution` — live state

`Execution` carries the plan, the current state, the states already entered, the evidence log (each
record stamped with its sequence number and the state it arrived in), the artifact graph, and the
event stream. It implements `RequirementContext`, which is what lets the domain's requirement layer
evaluate against it unchanged.

Derived facts the engine computes (nothing else can):

```text
state.current                    the current state id
state.<id>.entered               true once entered
evidence.count.<kind>            how many records of that kind
evidence.first_seq.<kind>        submission order — this is what makes red-before-green checkable
evidence.last_seq.<kind>
test.first_result                the first test outcome ever observed
evidence.missing                 unmet evidence requirements, for `evidence.missing == 0`
obligations.unmet
approvals.granted
principle.<id>.active
workflow.terminal
```

`Snapshot` (serialisable) plus `Engine::restore(plan, snapshot)`, so an execution survives a process
boundary — which is what `.engineering/state.yaml` needs.

Deliverable: `execution.rs`, ~12 tests, including one that asserts a fact the engine derives cannot be
overwritten by submitted evidence.

## W1.5 `aep-engine::evaluate`, `policy`, `explain`

* `Evaluation`: what is owed in the current state, every outgoing transition with its guard outcome
  and unmet list, the completion report, and whether the execution is blocked.
* `policy`: `authorize(execution, action_request) -> Decision`, producing the design §59 shape —
  `allowed: false`, the operation, the principle and rule that refused it, what is missing, and the
  current state.
* `explain`: the §60 rendering — `✓ unit tests / ✗ property test session_isolation` — from the same
  data structures, so the human view and the machine view cannot disagree.

Deliverable: three modules, ~18 tests.

## W1.6 `aep-engine::engine` — the trait

`ProtocolEngine` per design §61, with three additions the harness needs: `authorize`,
`restore`, and evidence submission that returns the assigned id.

Rules to be enforced here:

* Evidence of a kind the protocol does not declare is **rejected**, not stored.
* `transition` takes the first permitted transition in document order and reports any others that
  were also permitted, so the choice is deterministic and the ambiguity is visible.
* Every decision — allowed, denied, blocked, transitioned, completed — emits an event, including
  denials.
* `Clock` trait with `SystemClock` and `FixedClock`; the engine reads time only through it, so a
  replay produces byte-identical events.

Deliverable: `engine.rs`, ~15 tests, one of which runs the same execution twice under a fixed clock
and asserts identical event streams.

## W1.7 Documents — making the protocol real

The first content that is not code. Independent of W1.1–W1.6; only needs the generated schemas.

| Tree | Contents |
|---|---|
| `protocols/` | `aep/1`, plus `adp/1` and `aop/1` extending it |
| `principles/` | ~19 documents: the 8 core (spec-driven, test-driven, contract-testing, static-analysis, least-privilege, reversible-changes, provenance-tracking, approval-gates) plus what the incident and release profiles need (preserve-evidence, hypothesis-driven-diagnosis, blast-radius-limitation, verify-after-action, progressive-delivery) and what `development.critical` needs (property-based-testing, differential-testing, mutation-testing, clean-room, invariant-checking, architecture-decision-records) |
| `workflows/` | `adp/default` (receive → specify → decompose → establish_verifiers → implement → verify → adversarial_verify → review → complete), `incident/standard` (detect → triage → diagnose → mitigate → recover → verify → learn), `release/progressive` (qualify → stage → canary → observe → promote → verify → complete), `migration/forward-only` |
| `profiles/` | `development.fast`, `.standard`, `.critical`, `incident.standard`, `release.progressive` |
| `artifacts/lifecycles/` | design, ADR, specification, review-result, story |
| `artifacts/kinds/` | required sections for design, architecture design, ADR (§95, §96) |
| `artifacts/relations/` | the relation vocabulary and which kinds may be related how |

Acceptance: the loader test from W1.2 passes over the whole tree, and `test-driven`'s red-before-green
obligation is expressed as a checkable ordering fact
(`evidence.first_seq.test_result < evidence.first_seq.diff`) rather than as a comment.

## W1.8 `protocol-cli`

| Command | Behaviour |
|---|---|
| `validate [path]` | load a tree or a single document; report every failure with file, location and code |
| `resolve --task t.yaml` | print the execution plan |
| `inspect <ref>` | show what a protocol, principle, workflow or profile declares |
| `evaluate --task t.yaml --state s.yaml --evidence e/` | what is owed, what is permitted, what is missing |
| `explain --task … --action production.write` | why an action is refused, or why the task is incomplete |
| `schema [name]` | print generated schemas |

`--format text|yaml|json`; exit codes `0` ok, `1` invalid, `2` usage. Tests drive the real binary
through `env!("CARGO_BIN_EXE_protocol")`, so no new dependency.

Deliverable: `protocol-cli`, ~10 integration tests.

## W1.9 Examples and the end-to-end test

`examples/development-passkeys/`: `task.yaml`, `.engineering/artifacts.yaml`, an evidence sequence,
and the expected decisions. A workspace integration test replays it and asserts, specifically:

1. `implement` cannot be entered until an approved specification exists.
2. Submitting a *passing* test before any diff **fails** the red-before-green obligation.
3. `review` cannot be entered while contract tests fail; the refusal names the contract test.
4. An approval of design version 3 does not satisfy the review requirement once the design is at
   version 7.
5. `production.write` is refused with the principle and rule that refused it.
6. Completion is refused with the exact missing property test named.

This is the wave's real acceptance test: it is the first thing that proves the design's central claim.

## W1.10 CI

GitHub Actions running `task check` on push and pull request, plus a job asserting
`cargo xtask schema --check` separately so a schema drift failure is legible on its own.

---

## Out of scope for this wave

Deliberately deferred, in order: the entity layer (§13–18), `aep-contract` (§34–47), an in-memory
reference backend, `aep-conformance` (§78, §104), `adp-domain` and `aop-domain` types.

## Risks

| Risk | Handling |
|---|---|
| The fact vocabulary turns out to be too thin for real principles once the documents are written | W1.7 is written *alongside* W1.3–W1.6 rather than after, so a missing fact is found while the engine is still soft |
| `unobservable_fact` makes the observables list a maintenance burden | protocols declare families (`tests.**`), not individual paths; the check exists to catch typos, and a typo in a completion condition is otherwise silent |
| Derived facts and evidence facts could collide | the engine binds derived facts last and a test asserts they win |
