# Feasibility review — the six driver decisions, the enforcement mapping and the adapter surface, against the code that exists

> **Subject:** [`docs/design/harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md) §§ 4.1–4.9 — the wave-2 update: D1–D6 in § 4.7, the enforcement mapping in § 4.8, the adapter surface in § 4.9 — and [`docs/plan/harness-wave-2-driver-decision.md`](../plan/harness-wave-2-driver-decision.md), its 22-row decision table, its W3.1–W3.6 build sketch and its three open cells.
> **Reviewed against:** the working tree on `main` at `ab50b91`+ (a parallel agent was committing under `website/` throughout; nothing under `website/` is read or relied on here).
> **Question asked:** not *should we build the driver* — the plan page reserves that outcome. The question W2.2 sets is *is every decision buildable as written*, and *which one is the one that has to move*.
> **Method:** every claim that names a type, a function or a line was opened. Each verdict carries a `file:line`, a command's output, or the hooks reference. Where it does not, it says "unverified" or "I'm guessing".
> **Hooks reference:** the facts about Claude Code hooks used to fill § 4.8's marked cells were supplied to this review as verified against the official documentation on 2026-08-21 (`code.claude.com/docs/en/hooks-guide.md`, `.../permissions.md`). They are cited as **hooks reference** and are not re-derived here.
> **Not reviewed:** §§ 4.1–4.6 as prose, `website/`, and the wave-1 plugin's shipped content beyond what § 4.8 rows depend on.

---

## Verdict

**Five of six decisions hold. One is circular and cannot be built as written, three enforcement cells are wrong or unimplementable, and W3.1 as sequenced does not compile — because the crate it describes cannot exist.**

The decision content is unusually good. Every one of the six holes has a mechanism, and almost every mechanism is a function that is already there and already does the thing claimed. `refresh_facts` is real (`crates/aep-engine/src/execution.rs:297`, called at `:291` from `restore`) — it was worth checking, and it is not invented. `Snapshot` genuinely carries neither workflow nor version (`execution.rs:56-74`), so D1's cursor is genuinely load-bearing. `approval_recorded` genuinely does not check who granted the approval (`crates/aep-engine/src/policy.rs:135-151`), so D3's refusal genuinely has to be the driver's. All three self-declared corrections stand.

Four things are wrong, in descending order of what they cost.

**First, `aep-driver` as W3.1 describes it cannot be a workspace member.** Design open-decision D4 puts step maps in `drivers/`, loaded by `load_tree`'s fixed table (`crates/aep-engine/src/load.rs:22-28`), and W3.1 puts `StepMap`/`RawStepMap` in `aep-driver` alongside a router that consumes `Evaluation` and `TransitionResult` — which are `aep-engine` types (`crates/aep-engine/src/evaluate.rs:124`, `engine.rs:104`). `load_tree` is in `aep-engine`; its table's values are `aep_schema::parse::DocumentKind`; `aep-engine` depends on `aep-schema` (`crates/aep-engine/Cargo.toml`). For `load_tree` to load a step map, `aep-schema` must see the type; for the router to route, `aep-driver` must see `aep-engine`. That is a cycle. It is fixable — split the crate — but it is a split W3.1 does not have and W3.2 rests on.

**Second, D6's lock is inside the directory the lock allocates.** `lock.json` lives at `.engineering/runs/<run-id>/` and the run id is "allocated **after taking the lock**, by counting the directories that already exist" (§ 4.7 D6). Two drivers starting together take two different `<run-id>` paths and both `create_new` succeed. The lock has to be at one fixed path per store.

**Third, "the allowlist derives from `CapabilityPolicy::allow` only" is not the same statement as "invariant 6's ordering".** `allow`, `approval_required` and `deny` are three independent `BTreeSet`s (`crates/aep-domain/src/capability.rs:485`, `:493`, `:497`) and membership is by `covers`, not equality (`capability.rs:612-614`, `:240-246`). The precedence lives in `decide` (`capability.rs:588-599`), not in the sets. An `allow` entry of `deployment.create` — unscoped, therefore `Deploy(Environment::Any)` — covers `deployment.create:production` while `decide` returns `RequiresApproval` for it. Iterating `.allow` hands out the tool. `profiles/release-progressive.yaml:29-31` avoids this today *by comment*, which is exactly the kind of rule that drifts.

**Fourth, two of § 4.8's audit columns name something that does not exist or cannot see what it needs to.** There is no expectation kind that reads the offered tool list — the IR has it (`crates/trace-domain/src/ir.rs:223`) and the vocabulary does not (`crates/trace-domain/src/spec.rs:777-830`, 49 names, no `env.tool_available`). And a `PreToolUse` hook is a separate process, so it cannot call `Engine::authorize`, which takes `&mut Execution` (`crates/aep-engine/src/engine.rs:285`) — so the hook rows' "audit trail — `ActionRequested`/`ActionDenied` events" is a column the hook cannot fill.

**The wave-3 sequence is buildable, after W3.1 is split in two and W3.4 gains a prerequisite.** No decision is refuted on its merits. The one that has to move is W3.1.

---

## Summary — every decision, every cell, one verdict each

| # | item | verdict | evidence |
|---|---|---|---|
| **D1.1** | spelling is `adp/default/1`, not `@1` | **CONFIRMED** (citation wrong) | `version.rs:214-221`, `:290-297`; the design cites `:131-138`, which is `ProtocolRef` |
| **D1.2** | a major bump orphans the map at load, free | **CONFIRMED** | `registry.rs:118-122` + `version.rs:206-208` |
| **D1.3** | the pin is *mandatory* | **NEEDS-CHANGE** | `WorkflowRef.major` is `Option`; `PATTERN` makes `/1` optional (`version.rs:296`). Schema will accept what the loader refuses |
| **D1.4** | verifiers checked via `kinds_for_verifier` | **NEEDS-CHANGE** | `ExternalTool` is in no `default_verifiers()` list (`evidence.rs:1317-1334`) → every external verifier refused at load |
| **D1.5** | two-phase cross-validation | **CONFIRMED** | `Task::protocol` (`task.rs:338`) is unseen by any loader; `declares_evidence` (`protocol.rs:103-105`) |
| **D1.6** | the cursor closes runs-in-flight | **CONFIRMED** | `Snapshot` (`execution.rs:56-74`) has no workflow, no version; `Execution::restore` checks task + state name only (`:257-292`) |
| **D1.7** | step maps in `drivers/`, via `load_tree` | **INFEASIBLE as sequenced** | `load.rs:22-28` is in `aep-engine`; `aep-engine` → `aep-schema`; `aep-driver` → `aep-engine`. Cycle |
| **D2.1** | rebuild the graph each iteration; all existing code | **CONFIRMED** | `store.rs:85`, `:329-330`; `engine.rs:250-258`; `refresh_facts` `execution.rs:291`, `:297-319` |
| **D2.2** | a broken store stops the run, via `graph()` errors | **NEEDS-CHANGE** | `graph()` returns Ok for a store with parse failures — `store.rs:324-328` says so in its own doc |
| **D2.3** | "one directory walk and one validating build" | **NEEDS-CHANGE** | `load()` reads and parses **every file** (`store.rs:85-120`); `restore` also **re-resolves the whole plan** (`engine.rs:255`) |
| **D2.4** | fact-family boundary; the driver writes none | **CONFIRMED** | `artifact.rs:1830-1872`, `execution.rs:322-398`; graph is input-only (`engine.rs:198-200`) |
| **D3.1** | allowlist from `allow` **only** | **NEEDS-CHANGE** | must be `decide()` — `capability.rs:588-599`; `covers` is not equality (`:612-614`, `:240-246`) |
| **D3.2** | the naive headless refusal refuses every run | **CONFIRMED** | `least-privilege.yaml:19-22`, no `applies_when` (`:1-2`) |
| **D3.3** | the reachability scan is static and decidable | **CONFIRMED with a correction** | plan exposes everything (`plan.rs:90-110`), but the enumeration misses `Transition::requires` (`workflow.rs:136`) and nested conditionals (`requirement.rs:899`) |
| **D3.4** | a `development.standard` headless run still starts | **CONFIRMED** | `approval-gates.yaml:22` is guarded by `defined(...)`, which is two-valued (`predicate.rs:402`) |
| **D3.5** | no auto-approve, and it has to be the driver's refusal | **CONFIRMED** | `approval_recorded` ignores the approver (`policy.rs:135-151`); `ApprovalRequirement::evaluate` does not (`requirement.rs:851`) |
| **D3.6** | an owed approval is an `operator` step: persist and exit | **CONFIRMED** | `explain_completion` (`engine.rs:267-269`); no code contradicts it |
| **D4.1** | one session per `llm` step | **CONFIRMED** | and it is what makes open cell (b) solvable — see below |
| **D4.2** | a session outliving a `Moved` outlives its allowlist | **CONFIRMED** | `effective_policy` (`policy.rs:84-92`) |
| **D4.3** | the driver's own `claude -p` invocation | **NEEDS-CHANGE** | `--bare` skips hooks (hooks reference). Unaddressed: the driver could disable its own enforcement arm |
| **D5.1** | crash ⇒ submit nothing ⇒ both guards Unknown ⇒ Blocked | **CONFIRMED** | `workflows/development/default.yaml:106-116`, `:117-127`; Kleene `and`/`or` (`predicate.rs:66-75`); `engine.rs:411-414` |
| **D5.2** | `trace check` exit 3 is Unknown **and** recorded | **CONFIRMED** | `crates/trace-spec/src/evidence.rs:43-54` |
| **D5.3** | retry budgets per step kind, spent not reset | **CONFIRMED** | no code claim to break; consistent with D6 and § 4.4 |
| **D6.1** | `ExecutionId` collides across invocations | **CONFIRMED with a correction** | it is per **`Engine` instance**, not per process (`engine.rs:173`, `:189`, `:210-213`) — a stronger form of the same hazard |
| **D6.2** | the lock is the allocator, at `runs/<run-id>/lock.json` | **INFEASIBLE** | circular: the lock lives inside the directory whose name the lock is supposed to allocate |
| **D6.3** | liveness, never age | **CONFIRMED** (placement unstated) | the argument holds; pid liveness must sit in `protocol-cli`, not the pure core (§ 4.1) |
| **D6.4** | a removable lockfile vs "nothing physically deleted" | **CONFIRMED — not a conflict** | invariant 16 is the entity command vocabulary (`AGENTS.md:239-242`), not the filesystem |
| **4.8.a** | the boundary: actions + transitions enforced, text free | **CONFIRMED** | stated at § 4.8 *The honest boundary*, and true — see below |
| **4.8.b** | row 1 audit: `permission.denied` | **NEEDS-CHANGE** | the kind is real (`spec.rs:377`, `check.rs:156`) but it is a **whole-run count** (`adapter.rs:477`, `:571-574`); `0` cannot distinguish held from never-tried |
| **4.8.c** | row 3 audit: "the tools actually offered, per step" | **INFEASIBLE today** | `SessionStart.tools` exists (`ir.rs:223`); no expectation kind reads it (`spec.rs:777-830`) |
| **4.8.d** | rows 1/2 audit: `ActionRequested` / `ActionDenied` | **NEEDS-CHANGE** | a hook is a separate process; `authorize` takes `&mut Execution` (`engine.rs:285`) |
| **4.8.e** | hook trust model | **NEEDS-CHANGE** | no public docs (hooks reference). An unnamed assumption in a section whose subject is naming mechanisms |
| **4.8.f** | TOCTOU between hook and engine advance | **CONFIRMED — closed** | closed by D4: the process exits before `transition()` is called |
| **4.8.g** | row 6: the `.engineering/planning/**` write guard | **NEEDS-CHANGE** | must matcher `Bash` too, or state that `command.execute` is ungranted so no shell exists |
| **4.9.1** | no adapter trait in `trace-spec`; the seam is the IR | **CONFIRMED** | `grep '^pub trait'` over `crates/trace-spec/src` and `crates/trace-domain/src` returns nothing; `adapter.rs:102`, `:90-93`; `ir.rs:506`, `:516`, `:528` |
| **4.9.2** | `LlmStepExecutor` is new, and named as new | **CONFIRMED** | nothing in the workspace has that shape |
| **4.9.3** | `tool_config` is a pure function, not a trait | **CONFIRMED, input corrected** | the argument holds; its input must be the *decision*, not `.allow` (D3.1) |
| **4.9.4** | the shell-echo harness as wave-3 acceptance | **CONFIRMED** | buildable, no network, no credential; it is the only test the neutrality claim has ever had |
| **cell (a)** | the `PreToolUse` deny mechanism | **RESOLVED** | filled below |
| **cell (b)** | per-state tool gating under `claude -p` | **RESOLVED** | filled below; D4 is what makes it easy |
| **cell (c)** | the harness tool-name table | **RESOLVED, with one non-function** | filled below; `Bash` is the exception and it matters |
| **W3.1** | the pure routing core | **NEEDS-CHANGE** | must be split in two; determinism scan and lints line missing |
| **W3.2–W3.4** | the map, `protocol drive`, the hooks | **NEEDS-CHANGE** | rest on W3.1's split, D6.2's lock path, and a 50th expectation kind |
| **W3.5–W3.6** | shell-echo harness, driven-eval acceptance | **CONFIRMED** | buildable as sequenced |

**Counts: 23 CONFIRMED · 14 NEEDS-CHANGE · 3 INFEASIBLE · 0 UNRESOLVED** (three open cells resolved; two sub-questions carry named unverifiables, listed at the end).

---

## F1 — `aep-driver` as W3.1 describes it cannot be a workspace member (critical, buildability)

This is the finding that moves the wave, so it is first.

W3.1 puts four things in one crate: `StepMap`/`RawStepMap` with the two-phase cross-validation, the cursor, the `LlmStepExecutor` trait, `tool_config`, and the three-valued router — "resting on … nothing outside `aep-domain` and `aep-engine`".

Two of those pull in opposite directions.

**The router needs `aep-engine`.** § 4.1: the core "consumes `Evaluation` and `TransitionResult` *verbatim*". `Evaluation` is `crates/aep-engine/src/evaluate.rs:124`. `TransitionResult` is `crates/aep-engine/src/engine.rs:104`. So `aep-driver` → `aep-engine`.

**The step map needs to be visible from `aep-schema`.** Design D4 (taken, § 6) puts step maps in `drivers/`, and the whole argument for `drivers/` is that `load_tree` already walks a fixed table:

```rust
// crates/aep-engine/src/load.rs:22-28
const TREE: &[(&str, DocumentKind)] = &[
    ("protocols", DocumentKind::Protocol),
    ("principles", DocumentKind::Principle),
    ("workflows", DocumentKind::Workflow),
    ("profiles", DocumentKind::Profile),
    ("artifacts/lifecycles", DocumentKind::Lifecycle),
];
```

`DocumentKind` is `aep_schema::parse::DocumentKind` (`crates/aep-schema/src/parse.rs:32-51`), and `load_tree` lives in `aep-engine`, which depends on `aep-schema` (`crates/aep-engine/Cargo.toml`). Adding a sixth row means a `DocumentKind::StepMap`, which means `aep-schema` must be able to parse a step map, which means `aep-schema` → wherever `RawStepMap` lives. If that is `aep-driver`, the graph is `aep-schema → aep-driver → aep-engine → aep-schema`. **Cycle. `cargo` refuses it.**

The precedent that looks like a counter-example is not one. `aep-schema` already depends sideways on `aep-backend-markdown`, `ess-domain` and `trace-domain` (`crates/aep-schema/Cargo.toml`, with the reasons in comments) — but all three are **leaves**: `aep-backend-markdown` depends on `aep-domain` and nothing else (`crates/aep-backend-markdown/Cargo.toml`). That is exactly the shape a step-map crate has to have, and exactly the shape `aep-driver` cannot have.

Nor does dropping the `aep-engine` dependency work. The load-time half would survive it — the verifier check reduces to `EvidenceKind::default_verifiers()` in `aep-domain` (`crates/aep-domain/src/evidence.rs:1317`), and `kinds_for_verifier` is a four-line filter over it (`engine.rs:499-505`). The router would not: it exists to consume `Evaluation`.

**The change, exactly.** Split W3.1 into two members:

| crate | holds | depends on |
|---|---|---|
| **`aep-driver-spec`** (leaf) | `RawStepMap` → `StepMap`, the cursor types, both cross-validation phases, `ToolConfig` | `aep-domain` only |
| **`aep-driver`** | the three-valued router, `LlmStepExecutor`, `tool_config` | `aep-domain`, `aep-engine`, `aep-driver-spec` |

`aep-schema` then depends on `aep-driver-spec` — the same sentence its own comment already writes for `aep-backend-markdown` — `load_tree` gains its sixth row, `cargo xtask schema` publishes `schemas/generated/driver-steps.schema.json`, and invariant 1 holds without a second loading mechanism. `drivers/` must be the **last** row of `TREE`, because phase 1 reads `workflow.states` out of a registry the earlier rows fill.

**The cheaper alternative, named so its absence reads as a decision:** put `StepMap` in `aep-domain`. It is one fewer member and it is wrong — a step map is explicitly *"a harness's business"* (§ 4.2), and `aep-domain` is the protocol vocabulary. The two-crate split keeps that boundary and costs one `Cargo.toml`.

**Cost now:** one line in W3.1, one manifest. **Cost after W3.1 lands:** discovering it at `cargo build`, then moving types between crates with the schema index and the drift check pointing at the old path.

---

## F2 — D6's lock is inside the directory the lock allocates (critical, D6 does not hold)

D6 states two things that cannot both be true.

> `.engineering/runs/<run-id>/` holds `snapshot.json`, `cursor.json`, `lock.json` and the step transcripts

> The driver allocates `<task-id>/<n>` **after taking the lock**, by counting the directories that already exist

The lock is *in* `<run-id>/`. `<run-id>` is chosen *after* the lock is taken. There is no order in which those two sentences execute.

The failure is not theoretical, and it is the exact failure D6 exists to prevent. Two `protocol drive` invocations against one store at the same moment each count the existing directories, each get `<task>/3`, each `create_new` at `.engineering/runs/<task>/3/lock.json` — and one wins. Or, worse, they count at slightly different moments, get `3` and `4`, and **both `create_new` succeed**, because they are different paths. Two live runs, one store, D2 rebuilding the graph under both. That is D6's own rejected option — *"no lock, last writer wins"* — reached by accident.

**The change, exactly.** One fixed path per store, created before anything is allocated:

```text
.engineering/runs/lock.json     ← create_new; the mutex. Carries pid, host, driver version,
                                   and the run id it granted.
.engineering/runs/current       ← the store-level pointer (unchanged)
.engineering/runs/<run-id>/     ← snapshot.json, cursor.json, transcripts. No lock file.
```

Everything else in D6 survives verbatim: `create_new` is still one atomic syscall; the holder is still named in the refusal (the run id is now *in* the lock rather than *around* it, which is strictly better — a refusal can print it without reading a second file); `--take-lock` still removes one path; the release-on-every-exit-path rule is unchanged; and the no-age-threshold argument is untouched.

**One consequence worth stating.** With the lock outside the run directories, `--resume` of a paused run must **re-take** the store lock before writing, and must refuse if another run holds it. D6 says a paused run does not hold a lock; it does not say resume re-acquires. Add the sentence.

---

## F3 — "derive the allowlist from `allow` only" is not invariant 6 (critical, D3(a) and § 4.9 point 2)

D3(a) and the plan's D3 row both say the `llm` step's tool set derives from `CapabilityPolicy::allow` **only**, and both call that "invariant 6's ordering … expressed at the one layer that can enforce it".

It is not. Invariant 6's ordering lives in one function:

```rust
// crates/aep-domain/src/capability.rs:588-599
pub fn decide(&self, capability: &Capability) -> CapabilityDecision {
    if Self::covered_by(&self.deny, capability) { return CapabilityDecision::Denied; }
    if Self::covered_by(&self.approval_required, capability) { return CapabilityDecision::RequiresApproval; }
    if Self::covered_by(&self.allow, capability) { return CapabilityDecision::Allowed; }
    CapabilityDecision::NotGranted
}
```

`allow`, `approval_required` and `deny` are three independent sets (`capability.rs:485`, `:493`, `:497`) and nothing removes a capability from `allow` when a principle adds it to `deny` — `CapabilityPolicy::grant` extends all three (`capability.rs:619-624`) and `restrict` extends two (`:630-634`). `AGENTS.md:176-179` says as much: the invariant's own enforcing test *"asserts its fixture holds one capability in all three sets before asserting the outcome"*. A capability in all three sets is a state the model is built to represent.

And membership is not equality:

```rust
// crates/aep-domain/src/capability.rs:612-614
fn find<'a>(set: &'a BTreeSet<Capability>, capability: &Capability) -> Option<&'a Capability> {
    set.iter().find(|entry| entry.covers(capability))
}
```

with `Capability::covers` widening across environments (`capability.rs:240-246`) and `Environment::covers` making `Any` cover everything (`:83-85`). So an `allow` entry of unscoped `deployment.create` is `Deploy(Environment::Any)`, and it **covers** `deployment.create:production` — which `approval-gates.yaml:38` puts behind approval and the protocol floor gates independently (`policy.rs:98-106`).

Today no shipped profile has that pairing, and `profiles/release-progressive.yaml:29-31` avoids it *in a comment*: *"An unscoped `deployment.create` would mean every environment, production included, which is exactly the grant the approval floor exists to prevent."* A rule that holds because a comment warned the author is the class of rule this repository writes registers about.

**The change, exactly.** Point 2 becomes a decision, not a projection:

```rust
fn tool_config(policy: &CapabilityPolicy) -> ToolConfig
// admits a capability iff policy.decide(&capability) == CapabilityDecision::Allowed
```

It is still a pure, total, clock-free function, so § 4.9's argument for a function over a trait is untouched — it just calls the one function that owns the ordering instead of reading one of its three inputs. The wave-3 test D3 already names ("an `llm` step's derived allowlist contains no tool for any capability in `approval_required` or `deny`") should be strengthened to the mutation that actually catches this: **a fixture policy whose `allow` holds an unscoped `deployment.create` and whose `approval_required` holds `deployment.create:production`, asserting no deploy tool is offered.**

---

## F4 — the three self-declared corrections: all three stand

W2.1's acceptance turns on these being real. They are.

### (1) `adp/default@1` is wrong; the spelling is `adp/default/1` — **CONFIRMED**

`WorkflowRef` is generated by `versioned_ref!` with the pattern the design quotes, and `Display` writes `{id}/{major}`:

```rust
// crates/aep-domain/src/version.rs:290-297
versioned_ref!(
    /// Reference to a workflow, such as `adp/default` or `workflow:incident-standard/2`.
    WorkflowRef, WorkflowId, "workflow", "workflow:",
    "^(workflow:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*(/[1-9][0-9]*)?$"
);
```

```rust
// crates/aep-domain/src/version.rs:214-221 — inside versioned_ref!
match self.major {
    Some(major) => write!(f, "{}/{}", self.id, major),
    None => write!(f, "{}", self.id),
}
```

and `split_version` (`version.rs:93-102`) takes the trailing all-digit segment, so `adp/default/1` is id `adp/default` at major 1 exactly as claimed. There is no `@` anywhere in the parser.

**One citation correction.** The design says `WorkflowRef` "is declared by the `versioned_ref!` macro at `crates/aep-domain/src/version.rs:131-138`". `:131-138` is `ProtocolRef::PATTERN` and its `Display` impl. The macro is `:174`, the declaration `:290-297`. Also `Workflow` is cited as `workflow.rs:153-160` for `workflow.states`; the struct opens at `:153` and `states` is at `:166`.

### (2) the naive headless refusal would refuse every run — **CONFIRMED, and the corrected rule works**

```yaml
# principles/governance/least-privilege.yaml:1-2, :19-22
# No `applies_when`: a privilege rule with exceptions is not a privilege rule. It holds for every
# task under every profile.
  require_approval:
    - production.write
    - deployment.create
    - network.write
```

So `approval_required` is non-empty for every plan under every profile. The naive rule refuses everything. Confirmed.

The **corrected** rule — reachability — also works, and it works for a subtler reason than the design gives itself credit for. `development.standard` includes `approval-gates` (`profiles/development-standard.yaml:18-22`), whose `before_completion` obligation carries a `human: true` approval (`principles/governance/approval-gates.yaml:22-26`). Read naively *that* also refuses every standard run. It does not, because the obligation is conditional and its guard is deliberately two-valued:

```yaml
# principles/governance/approval-gates.yaml:16-22
      # Guarded with `defined(...)` on purpose: the guard has to be two-valued.
      - when: defined(deployment.production.status)
```

```rust
// crates/aep-domain/src/predicate.rs:402
Self::Defined(path) => Truth::from_bool(facts.fact(path).is_some()),
```

With no deployment fact at pre-flight, `defined(...)` is `False`, the conditional is skipped, no `human: true` approval is reachable, and the run starts. **D3's headline wave-3 test passes for a reason the design does not state, and it should state it** — the test is only green because that one guard was written `defined(...)` rather than `deployment.production.status == succeeded`. A future principle author who writes the bare comparison silently turns every headless development run into a refusal.

**The honest consequence D3 omits:** `profiles/development-critical.yaml:46-52` carries an **unconditional** `reviews: [{subject_kind: design, result: approved, human: true, fresh: true}]`. Under the corrected rule, **a headless run under `development.critical` refuses to start** unless the design review already exists in the store. That is the right behaviour and it is a surprise if nobody wrote it down. Add it as the second wave-3 test.

### (3) there is no adapter trait in `trace-spec` — **CONFIRMED**

```console
$ grep -rn '^pub trait\|^trait ' crates/trace-spec/src/ crates/trace-domain/src/
$                                   # no output
```

The seam is what § 4.9 says it is: `read_transcript(&[u8]) -> Result<TraceIr, ValidationErrors>` (`crates/trace-spec/src/adapter.rs:102`) stamping `CLAUDE_CODE_STREAM_JSON: AdapterRef` (`adapter.rs:90-93`) into `TraceIr::adapter` (`crates/trace-domain/src/ir.rs:506`, `:516`, `:528-529`), with `check` (`check.rs:58`), `CheckReport` (`report.rs:435`) and `to_evidence` (`evidence.rs:169`) all taking the neutral IR. The decision not to add a trait speculatively is right and needs no change.

---

## F5 — D1's verifier check refuses every external tool (D1, needs-change)

D1's load phase checks "every verifier a step names can actually produce the kind it claims, via `aep_engine::engine::kinds_for_verifier`". That function is:

```rust
// crates/aep-engine/src/engine.rs:499-505
pub fn kinds_for_verifier(verifier: &Verifier) -> Vec<EvidenceKind> {
    EvidenceKind::ALL.iter().copied()
        .filter(|kind| kind.default_verifiers().contains(verifier))
        .collect()
}
```

and `default_verifiers` enumerates only the thirteen **named** verifiers (`crates/aep-domain/src/evidence.rs:1317-1334`; the list is `Verifier::NAMED`, `crates/aep-domain/src/verification.rs:72-86`). `Verifier::ExternalTool(ToolRef)` — the variant `Verifier::parse` falls through to for anything unrecognised (`verification.rs:110-117`) — appears in **no** row. So `kinds_for_verifier(&ExternalTool("ruff"))` is `[]`, and a step map reading

```yaml
      - kind: command
        run: [ruff, check, .]
        evidence: { kind: static_analysis, verifier: ruff }
```

is refused at load, naming a defect that is not one.

There is a second, quieter problem: `default_verifiers` is a table of **defaults**, not of constraints. `Diff` defaults to `[Compiler, StaticAnalyzer]` (`evidence.rs:1326`), so a `diff` produced by `git` is refused too. Using it as a hard load-time gate promotes a default into a rule, in a repository whose invariant register asks for the opposite.

**The change, exactly.** Phase 1 refuses only when the verifier is **named** and the kind is not in its list:

> A step naming a `Verifier::NAMED` verifier is refused when `kinds_for_verifier` does not contain the kind it declares. A step naming a `Verifier::ExternalTool` is not checked here — the protocol has nothing to check it against — and its kind is still checked at run start against `Protocol::declares_evidence` (`crates/aep-domain/src/protocol.rs:103-105`).

D1's wave-3 test survives verbatim: `contract_result` from `verifier: test-runner` is still refused, because `TestRunner` is named and `ContractResult` maps to `ContractRunner` alone. Add one: a step naming `verifier: some-external-tool` **loads**.

---

## F6 — D1's mandatory pin is stricter than the schema it will publish (D1, needs-change)

D1 makes the pin mandatory, and gives the right argument for it: *"a step map is an instruction sheet for a specific state graph"*. But `WorkflowRef` cannot express that:

* `major` is `Option<MajorVersion>` (`version.rs:200-202`), and `accepts` returns `true` for an unpinned ref (`:206-208`);
* the published pattern makes the version group optional (`version.rs:296`), and the `JsonSchema` impl writes exactly that pattern into the generated schema (`version.rs:325`).

So `schemas/generated/driver-steps.schema.json` will accept `workflow: adp/default` while the loader refuses it. Invariant 1 says schemas are generated from the types; an editor validating against the published schema will tell an author their map is fine and the loader will then refuse it.

**The change, exactly** — cheapest first:

1. **Preferred.** A `PinnedWorkflowRef` newtype in the step-map crate: `TryFrom<WorkflowRef>` refusing `major() == None` with an accumulating `ValidationCode`, its own `JsonSchema` writing `"^(workflow:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*/[1-9][0-9]*$"` — the same pattern with the group made required. `ProtocolRef` is the model to copy: it holds a non-optional `MajorVersion` (`version.rs:108-112`) and publishes a pattern with no optional group (`version.rs:132`). D1 already cites `ProtocolRef` as the precedent; take the type as well as the argument.
2. **Acceptable.** Keep `WorkflowRef`, refuse an unpinned one in `TryFrom`, and write one line in § 4.7 D1 saying the schema is deliberately looser than the validator and why.

Silently doing (2) without the sentence is the option to avoid.

---

## F7 — D2's "a broken store stops the run" is not what the code does (D2, needs-change, silent-corruption class)

D2 says:

> **A store that breaks mid-run stops the run.** `graph()` returns `ValidationErrors`.

`graph()` returns `ValidationErrors` for **graph** problems. It says so itself:

```rust
// crates/aep-backend-markdown/src/store.rs:319-328
/// Through [`ArtifactGraph::build`], which is where duplicate ids, edges pointing at nothing,
/// self-supersession and cycles are refused. …
///
/// A store with [failures](Self::failures) still produces a graph of what did load. That is
/// deliberate for reading — a listing of nine artifacts is more useful than a refusal because
/// the tenth file has a typo — and it is why every verb that *writes* checks
/// [`Self::is_clean`] first.
```

A document that fails to parse, or whose declared id does not match its path, lands in `report.failures` (`store.rs:100-120`) and never reaches `graph()`. So a driver that only checks `graph()` will, mid-run, get `Ok(graph)` from a store that has silently lost a document — and D2's own fact table says `artifact.**` comes from that graph (`crates/aep-domain/src/artifact.rs:1830-1872`). `artifact.story.count` drops by one. `artifact.design.approved` flips from `true` to `false`. The engine then evaluates a **completion gate** against a store that lost a file to a typo, and reports a requirement as unmet — or, in the `NoneOf`/`at_most` direction, as met.

This is the same defect class as an evidence record that fails open: a fact base that shrinks silently produces a verdict nobody can attribute.

**The change, exactly.** One sentence in D2 and one line in the loop:

> The driver stops when `report.is_clean()` is false **or** `graph()` returns errors. The first is the store's own integrity check (`StoreReport::is_clean`, `crates/aep-backend-markdown/src/store.rs:314`) and it must be consulted first, because a file that did not parse is not in the graph to be wrong about.

The wave-3 test D2 already names — *"a store with a broken relation target stops the run with the store's own errors"* — exercises only the `graph()` half. Add its twin: **a store with one unparseable file stops the run**, asserted by the fact store being unchanged rather than shrunk.

---

## F8 — D2's cost is understated in two ways, and both are still acceptable (D2, needs-change to the wording only)

D2 says the per-iteration cost is *"One directory walk and one validating build per iteration."* It is more than that, and the design's honesty standard asks for the number.

**One: `load()` reads and parses every file.** `collect_documents` walks (`store.rs:371`), then for every path: `fs::read_to_string` and `PlanningDocument::parse` (`store.rs:105-120`). It is O(bytes of the store), not O(directory entries).

**Two: `restore` re-resolves the whole plan.**

```rust
// crates/aep-engine/src/engine.rs:250-258
pub fn restore(&self, task, artifacts, snapshot) -> Result<Execution, ProtocolError> {
    let plan = resolve(&task, &self.registry)?;
    …
}
```

so every iteration also re-runs profile extension, principle selection, capability composition and obligation resolution, then `refresh_facts` rebuilds the whole fact store from plan facts, graph facts and the full evidence log (`execution.rs:297-319`).

**Neither is a problem and the conclusion does not move.** Both are pure CPU over local files with no clock and no network, both are linear, and D2's argument — *"the rebuild **is** the store's integrity check"* — applies to the re-resolve too: a document edited mid-run is re-validated. What changes is the sentence. Write it as *"a full read and parse of every planning document, plus a full plan re-resolution, per iteration"*, and keep D2's answer to the cost question, which is P4's SQLite backend rather than a cache.

**One thing the design should decide while it is there:** the registry. `Engine` holds it (`engine.rs:172`) and `restore` re-resolves against **whatever registry the engine was built with**. If the driver builds the `Engine` once at run start, a mid-run edit to `workflows/` is *not* picked up, while a mid-run edit to `.engineering/planning/` *is*. That asymmetry is defensible — D1's cursor pins the workflow anyway — but it is currently accidental. State it: **the registry is loaded once per `protocol drive` invocation; the store is rebuilt per iteration.**

---

## F9 — D3's reachability walk misses two shapes (D3, needs-change)

The walk is feasible: `ExecutionPlan` is fully public and exposes everything the scan needs — `completion`, `obligations[].requires`, `workflow`, `capability_policy`, `facts` (`crates/aep-domain/src/plan.rs:90-110`). Two shapes are missing from the enumeration.

**Transitions carry their own requirements.** The design walks `plan.completion`, `plan.obligations[].requires` and `workflow.states[].requires` (`workflow.rs:98`). It does not walk:

```rust
// crates/aep-domain/src/workflow.rs:127-138
pub struct Transition {
    pub from: StateId,
    pub to: StateId,
    pub when: Predicate,
    /// Structured requirements that must also hold.
    pub requires: RequirementSet,
    …
}
```

the evaluator reads it as a first-class requirement set (`crates/aep-engine/src/evaluate.rs:215`, beside `current.requires` at `:203` and `target.requires` at `:226`), so a `human: true` approval on a transition is genuinely owed and genuinely invisible to the scan as written. No shipped workflow uses it today (`workflows/development/default.yaml` has one `requires:`, at `:60`, on a *state*), which is exactly why it would be found by a user rather than by the gate.

**Conditionals nest.** `ConditionalRequirement.require` is `Box<RequirementSet>` (`requirement.rs:895-900`), so a conditional can contain conditionals. The precedent D3 cites — `count_missing_evidence` (`execution.rs:413-431`) — descends exactly one level, by design, because it is counting rather than proving absence. A *reachability* scan that stops at one level under-reports, and under-reporting here means starting a headless run that will wedge.

**The change, exactly.** D3's bullet list gains `every workflow.transitions[].requires`, and the walk over a `RequirementSet` is recursive through `conditional[].require`, with the `when == False` skip applied at every level. One added sentence, one recursive function.

**Wave-3 test to add:** a workflow whose `verify → complete` transition carries a `human: true` approval refuses a headless start, naming the transition.

---

## F10 — the `ExecutionId` hazard is real and worse than stated (D6, confirmed with a correction)

D6 says the ordinal counter is *"an `AtomicU64` initialised to zero **in each process**"*. It is initialised to zero **in each `Engine` value**:

```rust
// crates/aep-engine/src/engine.rs:173, :186-190
    executions: AtomicU64,
…
        Self { registry, clock, executions: AtomicU64::new(0) }
```

```rust
// crates/aep-engine/src/engine.rs:210-213
let ordinal = self.executions.fetch_add(1, Ordering::Relaxed) + 1;
let id = ExecutionId::new(format!("{}.{ordinal}", plan.task.id))…
```

So two `Engine`s in **one** process also collide — which is precisely the shape a test harness builds. D6's mitigation (the run id is the driver's, allocated under the lock; the `ExecutionId` goes inside the cursor) is correct and unaffected. Only the sentence needs one word. The wave-3 test D6 names ("two executions initialised in one process do not collide on a run directory") is already the right test and now has the right reason.

`Execution::restore` preserves the snapshot's id (`execution.rs:277`), so the hazard is confined to `initialize` — worth saying, because it bounds the fix.

---

## F11 — the enforcement mapping: what is honest, what is not

### The boundary claim is stated and it is true

> **Enforcement is complete over ACTIONS and TRANSITIONS, and over nothing else. … Text is free.**

Verified in both halves.

* **Transitions.** The driver never evaluates a gate (§ 4.1), and there is no API by which it could move an execution other than `transition()` — which computes `permitted_transitions()` from the evaluation and emits `TransitionBlocked` for every candidate otherwise (`engine.rs:391-415`). The graph is input-only (`engine.rs:198-200`).
* **Text.** All three named mechanisms check out: the `Llm` variant carries no evidence field (a type-level claim, unfalsifiable until the type exists, but nothing prevents it); `Producer::Agent` does not satisfy `independent: true` (`requirement.rs:191-195`); and `TraceEvidence::PRODUCER` is a constant with no settable call site (`crates/trace-spec/src/evidence.rs:98-100`), documented at `:17-33` in exactly those terms.

Both rows marked "**engine — exists today**" are accurate.

### Cell (a) — the `PreToolUse` deny mechanism — **FILLED**

Per the hooks reference (`code.claude.com/docs/en/hooks-guide.md`, `.../permissions.md`):

| question the cell asks | answer |
|---|---|
| event name | `PreToolUse` |
| how to deny deterministically | exit code **2**, or JSON `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"…"}}` |
| is the reason surfaced | yes — fed back to the model, which is what lets a refusal name the document that refused |
| can the hook see the tool's arguments | **yes** — the full `tool_input` (`command`, `file_path`, …) |
| per-tool selection | `matcher` (e.g. `"Bash|Edit|Write"`) and `if` with permission-rule syntax (e.g. `Bash(git *)`) |
| can a hook *grant* | **no** — deny wins over allow rules; hooks cannot loosen restrictions |
| may the hook spawn a process (e.g. `protocol authorize`) | yes; default timeout 10 minutes, overridable per hook |
| does it fire under `claude -p` | **yes**, identically. `PermissionRequest` hooks do **not** fire in plain `-p`; `PreToolUse` is the one to use |
| how a plugin ships them | `hooks/hooks.json` at the plugin root. A plugin's AGENTS file cannot declare hooks; the plugin can |
| multiple sources | run in parallel; most restrictive wins (deny > ask > allow) |
| **not documented** | the **trust model** for plugin-supplied hooks. No public docs |

Row 1's mechanism ("a `PreToolUse` hook denies it if it is reached by another route") and row 6's ("a `PreToolUse` hook denying `Edit`/`Write` under `.engineering/planning/**`") are both **implementable exactly as written**. `tool_input.file_path` is what makes row 6 a path check rather than a tool check.

### Cell (b) — per-state tool gating under `claude -p` — **FILLED, and D4 is what makes it easy**

`--allowedTools` is **fixed at session launch**; there is no mid-session list swap (hooks reference). Read against § 4.8 row 3, which wants the set "re-rendered at every `Moved`", that looks like a problem. It is not, because of D4: **one `claude -p` invocation per `llm` step**, and a step never spans a transition. Every session is launched with the tool set for the state it runs in, at launch, by flag.

So row 3's rendering is:

| layer | mechanism |
|---|---|
| primary | `--allowedTools` at launch, from `tool_config(effective_policy(execution))` for the current state |
| backstop | a `PreToolUse` hook denying anything outside the same derived set, re-rendered per step |
| **not viable** | mid-session re-registration — the list is fixed at launch |

The backstop is not belt-and-braces: `--allowedTools` governs the tools *offered*, and a subagent, a plugin, or a `--continue` would each be a route around it. § 4.8's own "enforce and verify" argument applies one level down — the flag and the hook are two enforcement layers with different failure modes, and the hook is the one that sees arguments.

**This is a positive finding for D4 and it should be written into it.** D4 argues per-step sessions on replayability, allowlist lifetime and retry isolation. There is a fourth reason and it is mechanical: *per-step sessions are the only granularity at which a launch-time flag can express a per-state tool set.* Per-state sessions would already be wrong; one session per run would be unimplementable.

**Consequence for TOCTOU (asked, and closed).** A `PreToolUse` hook consulting `protocol authorize` races the engine only if the engine can advance while the session is live. It cannot: the driver executes the step to completion, *then* submits evidence, *then* calls `transition()` (§ 4.4). The window is zero by construction. **Row 3's TOCTOU concern is CONFIRMED closed, by D4** — and it becomes open again the moment anybody proposes a longer-lived session, which is worth recording where D4 is, not here.

### Cell (c) — the harness tool-name table — **FILLED, with one entry that is not a function**

Cross-checked against the guide's `Action → Capability` table (`docs/guide/harness.md:134-142`) and the real tool names in the committed transcript (`crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl`, `system/init` event: 32 tools offered; four called — `Bash` 4, `Read` 3, `Edit` 3, `Skill` 1).

| Claude Code tool | `Action` | `Capability` | offered when |
|---|---|---|---|
| `Read`, `Glob`, `Grep` | `RepositoryRead` | `repository.read` | `decide` = Allowed |
| `Edit`, `Write`, `NotebookEdit` | `RepositoryWrite` | `repository.write` | `decide` = Allowed |
| `WebFetch`, `WebSearch` | `NetworkRequest { intent: read }` | `network.read` | `decide` = Allowed |
| `Bash` | **not a function** — see below | `command.execute` at best | see below |
| `Skill` | *none* | *none* | **named exemption required** |
| `Task`, `SendMessage`, `ListAgents`, `Task*` (`TaskCreate`/`Get`/`List`/`Output`/`Stop`/`Update`) | *none* | *none* | **never** — and see the subagent note |
| `Cron*`, `Monitor`, `ScheduleWakeup`, `RemoteTrigger`, `PushNotification`, `Workflow`, `DesignSync`, `ReportFindings`, `EnterWorktree`, `ExitWorktree`, `ToolSearch` | *none* | *none* | **never** |

Three things fall out, and all three belong in § 4.9 point 2 rather than in a reviewer's notes.

**1. `Bash` is the one tool that is not a function of a capability, and it is the hard case.** One `Bash` call can be `tests.execute` (`cargo test`), `command.execute` (`ls`), `repository.write` (`sed -i`, `>`), `network.write` (`curl -X POST`) or `secret.read` (`cat ~/.aws/credentials`). The guide's table is total in the `Action → Capability` direction (`harness.md:131`) and says nothing about the reverse, which is the direction a tool table needs. So:

> **`Bash` is offered only when `decide(command.execute) == Allowed`, and granting `command.execute` is understood to grant a superset of the shell's reach.** Any narrower gating of `Bash` — by `if: Bash(cargo test *)` or by a hook classifying `tool_input.command` — is **pattern-based and best-effort**, and § 4.8 must say so rather than list `Bash` as 100%-enforced.

**This is much less painful than it sounds, and the reason is a good one.** No development profile grants `command.execute`: `development.fast` allows `repository.read`, `repository.write`, `tests.execute`, `artifact.read`, `artifact.write` (`profiles/development-fast.yaml:30-35`) and `development.standard` adds `review.request`, `approval.request` (`profiles/development-standard.yaml:28-30`). Capabilities default to deny (invariant 6). **So under both development profiles, an `llm` step gets no `Bash` at all** — and `cargo test` still runs, because it is a `command` **step the driver executes**, not a tool the model holds. That is a genuinely strong property and § 4.8 should claim it explicitly: *the model never holds a shell in a development run; `tests.execute` is exercised by the driver.*

It also settles § 4.8 row 6 (F14 below): with no `Bash`, the `.engineering/planning/**` write guard only has to cover `Edit`/`Write`/`NotebookEdit`.

**2. `Skill` maps to no `Action` and must still be offered.** § 4.3 has `llm` steps naming skills; `harness.md:144-146` says *"a tool with no `Action` to describe it is a tool the protocol cannot govern"*. Both are right and they collide. The resolution is small and must be written: **`Skill` is a named exemption — it loads instructions and takes no action; everything it causes is a subsequent tool call, which is governed.** One sentence in § 4.9 point 2, so it reads as a decision rather than an oversight.

**3. `Task` is a hole and the audit for it already exists.** A subagent spawned through `Task` runs with its own tool set. Nothing in D1–D6 derives that set, so a subagent is a route around the per-state allowlist. Two things close it, and both are cheap: **`Task` is never offered**, and the driver's trace specification asserts `subagent.spawned: {count: {at_most: 0}}` — a kind that already ships (`crates/trace-domain/src/spec.rs:797`; dispatched at `crates/trace-spec/src/check.rs:161-163`, reading `subagent_stats.spawned` at `crates/trace-spec/src/adapter.rs:478-480`). Enforce and verify, on the same object, using vocabulary that exists.

### F12 — row 3's audit column is not implementable with today's vocabulary (**INFEASIBLE**, and cheap to fix)

Row 3 says what audits the per-state tool set is *"the transcript: the tools actually offered, per step"*.

The IR carries it:

```rust
// crates/trace-domain/src/ir.rs:222-223
    /// The tools offered, by name.
    pub tools: Option<Vec<String>>,
```

The expectation vocabulary does not. `ExpectationKind::NAMES` is 49 entries (`crates/trace-domain/src/spec.rs:777-830`) and includes `env.skill_available`, `env.agent_available`, `env.plugin_loaded`, `env.permission_mode` — and **no `env.tool_available`**. `tool.absent` is not a substitute: it asserts a tool was never *called*, and a tool can be offered and never called, which is the case an allowlist bug produces.

**The fix is a 50th kind, mirroring the 49th.** `env.skill_available` is four lines of dispatch:

```rust
// crates/trace-spec/src/check.rs:103-107
ExpectationKind::EnvSkillAvailable { skill } => {
    env_offers(ir, "skills", "skill", skill, |start| start.skills.as_deref())
}
```

`env.tool_available` is the same call against `start.tools`, plus a `RawExpectationKind` variant (`crates/trace-domain/src/raw.rs`), a `NAMES` entry and a name arm (`spec.rs`). Three files, and the drift test that asserts the raw and validated vocabularies agree (`spec.rs:772-776`) will catch a half-done job.

**This is a wave-3 prerequisite the plan does not have.** W3.4 ships the hooks; nothing ships the kind that audits them. Add it — to W3.4, or as a W3.0 — and the design's own standard ("an enforcement mechanism nobody audits is a claim") is met rather than asserted.

### F13 — row 1's audit is a whole-run count, and hook denials may not be in it (needs-change + one named unverifiable)

`permission.denied` is real: `ExpectationKind::PermissionDenied { count }` (`crates/trace-domain/src/spec.rs:377-378`), dispatched at `crates/trace-spec/src/check.rs:156-160`. What it reads is one number from the final `result` event:

```rust
// crates/trace-spec/src/adapter.rs:477 and :571-574
        permission_denials: count_at(value, "permission_denials"),
…
fn count_at(value: &Value, key: &str) -> Option<u64> {
    let entries = value.get(key)?.as_array()?;
    u64::try_from(entries.len()).ok()
}
```

and in the committed real transcript that field is `"permission_denials": []`, so the adapter yields `Some(0)`. Absence stays distinguishable from zero, correctly (`adapter.rs:474-478`).

Three limits, none fatal, all of which § 4.8 currently papers over:

1. **It is a count, not an attribution.** The array's entries — which tool, which reason — are discarded. The audit can say *"three refusals happened"*; it cannot say *"the refusal was for `secret.read`"*.
2. **`0` is ambiguous.** A run in which enforcement held perfectly and a run in which the model never attempted a denied action produce the same number. So `permission.denied` cannot, alone, evidence that a deny rule held. What it *can* evidence is the driven-eval case W3.6 should build: a task whose step deliberately attempts a denied capability, asserting `at_least: 1`.
3. **Unverified: whether a `PreToolUse` hook `permissionDecision: deny` increments `permission_denials`.** The hooks reference documents the deny mechanism and says the reason is fed back to the model; it says nothing about the `result` event's array. This is the single cheapest unknown in the review to close — one `claude -p` run with a denying hook, then read the last line — and it is named rather than guessed.

**The change:** row 1's audit cell reads *"the transcript's whole-run `permission.denied` count, which evidences that a refusal happened and not which; attribution requires a deliberate-attempt case in the driven eval (W3.6)"*, with limit 3 recorded as an open question in the gap register rather than in a design paragraph.

### F14 — the hook cannot fill the audit column rows 1 and 2 name (needs-change)

Rows 1 and 2 give the audit as *"the audit trail — `ActionRequested` / `ActionDenied` events (`engine.rs:288-311`)"*. Those events are emitted inside:

```rust
// crates/aep-engine/src/engine.rs:285
    fn authorize(&self, execution: &mut Execution, request: &ActionRequest) -> Decision {
```

`&mut Execution` — an in-memory value in the driver's process, whose mutation is the point (`harness.md:23-24`: *"`authorize` takes `&mut` because asking is itself an event"*). A `PreToolUse` hook is a **separate process** with a JSON payload on stdin. It cannot call this. It could shell out to a `protocol authorize` that re-resolves the plan from disk — the hooks reference permits spawning, with a ten-minute budget — but that process would build a *different* `Execution`, emit its events into *that* one, and drop them on exit. The driver's snapshot would never see them.

So the design has to pick one and say which:

| option | what it costs | what it buys |
|---|---|---|
| **(a) the hook writes an append-only decision log** into `.engineering/runs/<run-id>/hook-decisions.jsonl`; the driver folds each line into the execution via `Engine::authorize` after the step exits | one file format, one fold; decisions are recorded a moment late | the events land in the real trail, the snapshot carries them, `audit_trail` sees them |
| **(b) the driver runs a local socket** the hook queries | a server in a driver that is otherwise a batch program; a second failure mode | live authorisation |
| **(c) the hook enforces without asking** — it renders the same `tool_config` the launch flag did, from a file the driver wrote for that step | no engine call at all; simplest | no `ActionRequested` events from the hook path; audit is the transcript only |

**(a) is the recommendation**, and it is the one that keeps § 4.8's own claim true. (c) is defensible but then rows 1 and 2's audit cells must be rewritten to say *the transcript*, not *the audit trail*. What is not defensible is leaving the cell as written: it names a mechanism the layer it is attached to cannot reach.

### F15 — `--bare` and the hook trust model: two assumptions § 4.8 does not name

**`--bare` skips hooks** (hooks reference; re-enable via `--settings`). The driver launches `claude -p` itself (D4). Nothing in § 4.7 or § 4.8 constrains that command line. A future implementer reaching for `--bare` to get a clean, reproducible environment — a reasonable instinct in a repository this deterministic — would **silently delete the driver's own enforcement arm**, and every § 4.8 row whose layer is "plugin hook" would become a claim with nothing behind it. The tool set would still be constrained by `--allowedTools`, so the failure is partial, silent and exactly the kind this repository writes registers about.

**The change:** one line in D4 — *"the driver never passes `--bare`; the hook configuration is passed with `--settings`"* — and one wave-3 test, in the shape the repository already uses for guard efficacy: **assert the constructed command line contains no `--bare` and does contain the settings path.** A test that reads the argv the driver built is cheap and it is the only thing that keeps this from being prose.

**The hook trust model has no public documentation** (hooks reference). § 4.8's preamble says marked cells are "written from the shape of the mechanism rather than from its documentation" and asks the review to fill or correct them. This one cannot be filled — it can only be **named**:

> **Assumption, unverified and undocumented:** that hooks shipped by an installed plugin execute without a per-invocation consent step, and that a user who installed the plugin has thereby accepted them. If that is wrong, or becomes wrong, the hook layer of § 4.8 degrades to advisory and the `--allowedTools` layer carries enforcement alone.

Naming it costs a sentence. Not naming it is the failure mode the section's own preamble describes.

**Stop hooks are not relied on, and that is right.** Nothing in §§ 4.7–4.9 uses `Stop`/`SubagentStop`. Worth keeping that way: the hooks reference notes the model overrides after 8 consecutive blocks, which is a bound a driver's run-completion logic must not sit on top of. The driver decides completion from `TransitionResult::Completed` (`engine.rs:388`), which has no such bound.

### F16 — row 6's write guard, and one thing it does not cover

Row 6 is implementable exactly as written: a `PreToolUse` hook with `matcher: "Edit|Write"` reading `tool_input.file_path` (hooks reference). Two additions:

* **`NotebookEdit`** writes files too and is in the offered set (`plugin-eval-7hTYjT.jsonl`, `system/init`). Add it to the matcher.
* **`Bash`** is the obvious hole — `sed -i`, `>`, `cp`. Per cell (c) it is not offered under any development profile, so the hole is closed by the allowlist rather than by the matcher. **Say so in the row**, because the row's argument ("leaving `protocol artifact` the only writer") reads as though the matcher were exhaustive, and it is only exhaustive given a fact stated somewhere else.

The audit half of row 6 is the strongest in the table and needs no change: `protocol artifact validate` catches an illegal status *whether or not the hook fired*, which is the enforce-and-verify shape done properly.

---

## F17 — the lockfile against "nothing physically deleted": overturned, it is not a conflict

The review was asked whether a lockfile that gets removed conflicts with the invariant. **It does not**, and the reason is in the invariant's own enforcement clause:

```text
AGENTS.md:239-242
16. **Nothing is physically deleted.** `ArchiveEntity` and `SupersedeEntity` are the vocabulary.
    *Enforced by* the command vocabulary — there is no delete variant to call — and by a test that
    `CommandKind::parse("aep.entity.delete/v1")` fails, naming the kind it refused
```

Its subject is the **entity command vocabulary** in the storage contract, not the filesystem. A lock file is not an entity, `--take-lock` is not a command, and there is no `CommandKind` involved. Removing `lock.json` is no more a breach than removing a build artifact.

**Two adjacent rules the driver should still adopt, in the invariant's spirit, since they cost nothing:**

* a run directory is **never** deleted or reused. `--restart` allocates a new run id — D6 already says this, and it is the right shape;
* `--take-lock` **supersedes** rather than erases: the stolen lock's contents go into the new run's cursor, so *"this run took the lock from pid 4711 of run `<task>/2`"* is in the record. One field, and it is the difference between a run whose history explains itself and one that does not.

---

## F18 — W3.1–W3.6: buildable as sequenced, after two moves

W2.2's acceptance asks plainly whether the breakdown is buildable and which item moves. Answer: **buildable, with W3.1 split and one prerequisite added ahead of W3.4.**

| | as written | verdict | what has to change |
|---|---|---|---|
| **W3.1** | one crate: `StepMap`, cursor, `LlmStepExecutor`, `tool_config`, router | **NEEDS-CHANGE** | split into `aep-driver-spec` (leaf, `aep-domain` only) and `aep-driver` (F1). Add `[lints] workspace = true` to both manifests — `AGENTS.md:213-214`: *"a new crate that omits that line is outside every lint here"*. Add `crates/aep-driver/tests/determinism.rs` and a row to invariant 9's scan list (F19) |
| **W3.2** | `drivers/development/default.yaml` + generated schema | **rests on W3.1's split** | `aep-schema` gains the dependency; `drivers/` is the **last** row of `load.rs`'s `TREE`, after `workflows` |
| **W3.3** | `protocol drive`, executors, run directory, lock, flags | **NEEDS-CHANGE** | the store lock moves to a fixed path (F2); `--resume` re-takes it; pid liveness lives here, not in the pure core (F19) |
| **W3.4** | the plugin's hooks | **NEEDS-CHANGE, and needs a prerequisite** | `env.tool_available` must exist first (F12); the hook↔engine channel must be decided (F14); `--bare` must be forbidden (F15); the matcher gains `NotebookEdit` (F16) |
| **W3.5** | the shell-echo harness | **CONFIRMED** | none. It is the best-designed item in the sketch and the only test the neutrality claim will ever have had |
| **W3.6** | driven-eval acceptance | **CONFIRMED** | add the deliberate-denial case, so `permission.denied` audits something (F13) |

**W3.5 deserves the note the plan gives it.** *"Today one adapter exists, and 'harness-neutral' is a property nothing has ever tested"* is exactly right, and `trace-wave-1-transcript-checker.md:263-265` says the same in the crate's own plan page. A second `LlmStepExecutor` plus a second `read_transcript` returning `TraceIr` with its own `AdapterRef` proves all three points inside `task check`, with no network — which is what makes it a gate rather than a paragraph. Nothing about it is blocked and nothing about it needs a model.

---

## F19 — two AGENTS.md invariants the sketch touches and does not mention

**Invariant 9 (determinism) is scan-enforced, and `aep-driver` would be claiming it without a scan.** The register lists exactly which crates carry a banned-token scan and which are deliberately exempt — `aep-engine` because `src/clock.rs` owns `SystemTime::now`, plus *"`ess-conformance`… the backends, the CLI and `xtask`"* (`AGENTS.md:196-204`). § 4.1 makes a stronger purity claim for `aep-driver` than for any of those: *"clock-free and randomness-free, the same discipline `aep-domain` holds under invariant 8"*. `AGENTS.md:141-144` is explicit that a claim in that register must point at something: *"Do not write an enforcement here that you cannot point at."*

**So W3.1 must ship `crates/aep-driver/tests/determinism.rs`** — the same banned-token shape the ten listed crates use — **and add a row to invariant 9's list in the same change.** Costs an hour; without it the crate's headline property is prose.

**Pid liveness is where the pure/impure line actually bites.** D6 decides staleness by liveness rather than by age, which is right. A pid-liveness probe reads ambient OS state, and a banned-token scan will not catch it (it uses neither `SystemTime::now` nor `rand`). § 4.1 already puts "the three things that touch the world" in `protocol-cli`; D6 never says where the lock lives. **Say it in D6: the lock, the liveness probe and the run directory are `protocol-cli`'s (W3.3); `aep-driver` sees a `LockState` it was handed.** That also makes the lock testable without a second process.

---

## F20 — three smaller things, each one line to fix

* **§ 4.4 and D6 disagree about the run directory.** § 4.4 (`:618`) says `.engineering/runs/<execution-id>/`; D6 (`:1014`) says `<run-id>/` and gives the correctness reason it must not be the execution id. W2.1's acceptance says §§ 4.1–4.6 are "extended in place with pointers"; § 4.4 got the pointer for the back-edge budget and not for this. Add one: *"corrected in § 4.7 D6 — the path is the driver's run id, not the execution id."*
* **`Snapshot` has a `deny_unknown_fields` and a serde default.** `#[serde(deny_unknown_fields)]` at `execution.rs:54` and `#[serde(default = "default_actor")]` at `:72`. A driver that writes `snapshot.json` and reads it back on a later build is exposed to both: a field added by a future engine makes an *old* driver refuse a *new* snapshot. D1's cursor already refuses a moved workflow; it should also record the engine version, so the refusal is *"this snapshot is from a newer engine"* rather than a serde error. One field.
* **`ProtocolRef` is the citation D1 wants for the mandatory pin, and it is stronger than the design uses it.** `ProtocolRef` holds a non-optional `MajorVersion` (`version.rs:108-112`) and publishes a pattern with no optional group (`version.rs:132`) — it is the *type-level* precedent for F6's option 1, not just a rhetorical one.

---

## What this decision set gets right

Said plainly, because thirteen findings bury it and most of this is right.

1. **Every mechanism is an existing function, and they were checked.** `refresh_facts` exists (`execution.rs:297`). `StoreReport::graph` exists (`store.rs:329`). `kinds_for_verifier` exists (`engine.rs:499`). `approval_recorded`'s blind spot exists (`policy.rs:135-151`). The `ExecutionId` counter hazard exists (`engine.rs:210-213`). A decision list where the reviewer's job is *"is the line number right"* rather than *"does this function exist"* is a rare thing to receive.
2. **D5 is the best of the six and it is exactly right.** *"`Unknown` is spelled 'submit nothing'"* is invariant 5 discovered from the driver's side, and the mechanism checks out end to end: with no test evidence, `all: [tests.unit.failed == 0, …]` is Unknown and `any: [… > 0]` is Unknown (Kleene, `predicate.rs:66-85`), both `verify` guards are Unknown, `transition()` returns `Blocked` (`engine.rs:411-414`), and that is the state a retry runs against. The `trace check` exit-3 exception — Unknown *and* recorded — is the right call for the right reason.
3. **D3's three-part structure is the correct decomposition of a genuinely hard problem.** "Never a tool / a pause when owed / a static refusal when headless" separates enforcement, interaction and pre-flight cleanly, and the *reachability* framing is the insight — the naive rule really does refuse every run, and the design found that itself.
4. **The refusal to construct an `Evidence::Approval` anywhere in `aep-driver`, enforced by a source scan on the model of `crates/aep-engine/tests/evidence_scan.rs`.** That is invariant 7's enforcement pattern applied one layer out, by a design that noticed `approval_recorded` does not check the approver. It is the single most valuable line in § 4.7.
5. **§ 4.9's refusal to add a trait to `trace-spec`.** Verified: there is no trait in either trace crate, the seam really is `read_transcript → TraceIr + AdapterRef`, and *"a second adapter is a second free function"* is the smaller design. Refusing symmetry until there is a second implementation to design against is the right instinct and the gap register is the right home for the note.
6. **W3.5, the shell-echo harness.** A neutrality claim tested by a fake second harness with no model, no network and no credential, inside `task check`. It converts a sentence into a gate that can go red, which is this repository's whole method.
7. **D6's rejection of an age threshold**, with the argument that any threshold must exceed the longest legitimate step and the longest legitimate step is a person. Correct, and it is the reasoning that makes the liveness rule non-arbitrary.
8. **§ 4.8's *enforce and verify* framing.** *"An enforcement mechanism nobody audits is a claim, and an audit with no enforcement is a report about a horse that has already left."* Three of this review's findings (F12, F13, F14) are that standard applied to the section itself, which is what a good standard should do to its author.

---

## What wave 3 must not start without

Ordered by what blocks what. Everything here is a document change or a manifest except the last, which is three files.

1. **The `aep-driver` split decided and written into W3.1** — `aep-driver-spec` (leaf, `aep-domain` only) for the step map, the cursor and `ToolConfig`; `aep-driver` for the router. Without it W3.1 does not compile and W3.2 has nowhere to load from. **(F1)**
2. **D6's lock moved to a fixed store-level path**, with `--resume` re-acquiring it. Without it D6's headline is circular and two drivers can run against one store. **(F2)**
3. **`tool_config` reading `CapabilityPolicy::decide`, not `.allow`** — in D3(a), in the plan's D3 row and in § 4.9 point 2, with the unscoped-`deployment.create` mutation as the test. Without it, the one thing the design says a model cannot do, it can. **(F3)**
4. **`env.tool_available` scheduled ahead of W3.4.** Without it the per-state allowlist ships with nothing that can audit it, and § 4.8 row 3 is a claim. **(F12)**
5. **The hook↔engine channel decided** — recommendation: an append-only decision log the driver folds in after each step. Without it, rows 1 and 2 name an audit the hook layer cannot produce. **(F14)**
6. **`--bare` forbidden in D4, with a test over the constructed argv**, and the **hook trust model named as an unverified assumption** in § 4.8. Without the first, the driver can silently disable its own enforcement; without the second, § 4.8 has exactly the confidently-fonted guess its preamble refuses. **(F15)**
7. **D2's stop condition widened to `report.is_clean()`.** Without it a parse error mid-run silently shrinks the fact base under a live gate. **(F7)**
8. **D3's walk extended to `Transition::requires` and made recursive through nested conditionals**, and `development.critical`'s unconditional human review named as a run the corrected rule refuses. **(F9)**
9. **D1's verifier check exempting `Verifier::ExternalTool`**, and the mandatory pin expressed in a type whose published pattern matches the validator. **(F5, F6)**
10. **`crates/aep-driver/tests/determinism.rs` and a row in `AGENTS.md` invariant 9**, plus `[lints] workspace = true` in both new manifests. Without these the crate's central claim is unenforced in a register that says not to do that. **(F19)**

Items 1–3 are the ones that change what gets built. Items 4–6 are the ones that change whether anybody can tell it worked.

---

## What I could not verify

Named rather than glossed, because an unfilled cell that is not reported is the one failure this review can have that nobody would notice.

**Verified against the working tree:** every `file:line` above, read from the tree during this review; the tool names and the `permission_denials` shape in `crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl`; the absence of any trait in `crates/trace-spec/src` and `crates/trace-domain/src`; the absence of `env.tool_available` from `ExpectationKind::NAMES`; the absence of `ExternalTool` from every `default_verifiers()` row; the crate dependency edges, read from the four `Cargo.toml` files named.

**Taken from the hooks reference and not independently verified**, because this review has no Claude Code process to run against: the `PreToolUse` deny payload shape and exit-code-2 behaviour; `matcher` and `if` syntax; `tool_input` visibility; deny-wins across parallel hook sources; `hooks/hooks.json` as the plugin's hook location; `--allowedTools` being fixed at launch; `--bare` skipping hooks; `PermissionRequest` not firing under plain `-p`; the ten-minute default hook timeout; the 8-block Stop-hook bound.

**Not documented anywhere and therefore not verifiable by anyone right now:** the trust model for plugin-supplied hooks. Named as an assumption in F15 rather than answered.

**Unverified and cheap to close — one command each:**

* whether a `PreToolUse` `permissionDecision: deny` appears in the final `result` event's `permission_denials` array. One `claude -p` run with a denying hook, then read the last line. This decides whether § 4.8 row 1's audit works at all. **(F13, limit 3)**
* whether a subagent spawned through `Task` inherits the parent session's `--allowedTools`, or gets its own set. The recommendation (never offer `Task`; assert `subagent.spawned: at_most 0`) is safe under either answer, which is why it is the recommendation.

**Inferred, and labelled in place:** that `load()`'s per-iteration cost is acceptable for a store of hundreds of documents — extrapolated from the code path (`store.rs:85-120`), not measured, because no `.engineering/planning/` store exists in this tree to measure. If a store ever gets large enough for D2's rebuild to hurt, that is the number to produce before reaching for a cache.

**Not assessed:** the writing; §§ 4.1–4.6 as prose; anything under `website/`, which a parallel agent held throughout; whether the six decisions are the *right* six, which W2.1 settled and this review takes as given.
