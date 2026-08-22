# Changelog

Notable changes to `engineering-protocols`. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), where a **major**
version is a breaking change to a protocol's semantics, not merely to a Rust API.

Entries record what changed for someone using the protocol. Rationale that does not fit in a line
belongs in the commit message or in `docs/design/`.

## [Unreleased]

### Added

- **`harness: metaharness` — a second executor on the seam that was waiting for one.** An `llm`
  step naming it is spawned through `metaharness run claude` instead of a bare `claude` argv: the
  step's per-state surface travels as a sealed `metaharness.frame/1` document (digest-verified by
  the binary, cross-checked byte-for-byte against it with no crate link between the
  repositories), the governed tree travels as the `--cwd` declaration, and per-call denials are
  `tool.decided` events in the event stream the executor writes as the transcript — not a
  side-channel log a forgotten `--plugin-dir` can silence, which is how all eight post-fix
  sessions of run `W4-2` ran unenforced while looking clean. Operation rendering mirrors
  `allowed_tools` decision-for-decision; `subagent.spawn` is never offered. The default
  `claude-code` executor is unchanged. What frame mode does not carry — the hooks' per-argument
  narrowing — is stated in the executor's doc comment and waits for `--decisions ask`.
  Acceptance: `story:metaharness-executor`.

- **`env.mcp_servers` — the fifty-first expectation kind, and the first thing that can say a
  session was hermetic.** A scratch `CLAUDE_CONFIG_DIR` isolates a directory: it keeps the
  operator's plugins, skills and output style out, and it does not keep out account-level MCP
  servers, which are attached to the login and arrive over the network. Two of the four model
  sessions of governed run `W4-1/1` listed three of them in their init event, in a config home
  with no `mcpServers` key and a tree with no `.mcp.json`. All three were `status: needs-auth`
  and exposed no tool, so the inventory was 28 with servers and without — which is why
  `env.tool_available` cannot see this and why the new kind is a bound on a count. `{count:
  {at_most: 0}}` is the hermetic claim; a missing field is `unk` and never `ok`, because absence
  of evidence is not hermeticity. The init event's `mcp_servers` is lifted into `trace-ir/1` as
  `SessionStart.mcp_servers`, with an absent field and an empty list kept apart all the way down.

- **The eval sessions now launch with zero MCP servers, and the assertion gates everywhere.** The
  register row that produced `env.mcp_servers` said no directory the runner controls can exclude
  account-level servers — a flag can: `eval/run.sh` passes `--strict-mcp-config`, which ignores
  every MCP configuration not on its own command line. With the exclusion in place the expectation
  gates at `{at_most: 0}` in all three specifications, including the interactive one where it had
  been advisory: a server in the init event no longer reports somebody's account, it reports a
  broken exclusion. The driver's own session launcher gains the same flag with the driver wave.

- **A second step map, `development/checks`, for work whose acceptance is written in checks.**
  `drivers/development/default.yaml` names `cargo` in every state that names a verifier, which is
  what wedged governed run `W4-1/1`: the model wrote nine red shell checks and the step after it
  ran `cargo test --workspace` green, recording `test.first_result = passed`, which never changes.
  The decision the register row offered is taken: that file is a Rust map and says so in its
  header, and `drivers/development/checks.yaml` is the map beside it — one verifier command,
  `bash .engineering/checks/run.sh`, run in `establish_verifiers` before any implementation exists
  so the first recorded suite is the red one; `protocol validate` and `protocol artifact validate`
  as contract suite and static analysis, so no compiler is named anywhere. It carries the two
  steps its sibling lacks: an `operator` step that asks a person to approve the specification,
  because `spec-driven.before_implementation` wants `approved` and a run that approved its own
  specification would satisfy the principle by writing to the document the principle is about; and
  a `command` step running `protocol trace evidence`, so a driven run mints its own
  `trace_conformance` instead of a person typing the verb afterwards. Two maps now fit
  `adp/default/1`, so `protocol drive run` refuses to choose and names both — `--map` says which.

- **A step map can name the document a verifier wrote.** `evidence.record: <path>` on a `command`
  step: the driver reads that document and submits what it says instead of minting a record from
  the exit status. This is what makes `trace_conformance` reachable from a map at all — its record
  carries a specification digest, a transcript digest and three counts, and an exit status carries
  none of them, so minting one would state numbers nobody read. Two placeholders are expanded in a
  step's `run` words and in `record:` — `{run_directory}` and `{transcript}`, the transcript of
  the `llm` step this one follows — because a run directory is allocated when the run starts and a
  document in the repository cannot name one. An unknown placeholder, and a `{transcript}` in a
  state with no `llm` step before it, are refused at load.

- **A Codex variant of the planning instructions**, at `integrations/codex/`: the same four
  guardrails and the same read-the-vocabulary-from-the-CLI rule, as a Codex skill and an
  `AGENTS.md` fragment, verified against codex-cli 0.145.0. Instruction surface only — no
  enforcement hooks, no transcript adapter and no live eval, each refused with its reason in the
  README. `task codex-eval` checks the surface itself: free, no API call, no model — nine checks
  against the files, so drift in the instructions fails a command instead of a reader.

## [0.11.0-ground-truth-and-docs] — 2026-08-22

### Fixed

- **`protocol evidence inspect` no longer refuses a record the day it is written.** The reference
  is a civil date, and the future check compared wall-clock milliseconds against the day's first
  millisecond — so a record stamped 14:07 today read as "has not happened yet at" today, and the
  verb's primary use was its failing case. The check now runs at the reference's own granularity:
  an observation is future only when its civil date is after the reference date. A planned check
  dated tomorrow is still refused. Found by the docs overhaul re-running every quoted command.

### Changed

- **An `llm` step is now told what guards the way *out* of its state.** The step context carried
  `Evaluation.requirements` — what must hold *while in* the state — and never the outgoing
  transition's, so in `W4-1/1` the model was never told that `implement` needed a red suite and an
  approved specification, and $8.36 of one state went on work the guard then refused.
  `StepContext` carries `reaching` beside `requirements`: one line per requirement that does not
  hold yet on a way out, prefixed with where that transition goes, under its own heading in the
  prompt and as a `reaching` array in `step-context.json` (additive to
  `aep.drive-step-context/1`).

- **An `operator` step is a question asked once.** The pause is the step's completion, so the
  cursor moves past it and `protocol drive resume` carries on from the step after — whether the
  person did what was asked is decided by the guard on the way out, which refuses with one line
  per unmet requirement. A cursor left pointing at the step that paused re-presented the same
  question on every resume, so no map with an `operator` step before its last state could ever
  move past one.

- **The driver's model sessions launch with `--strict-mcp-config`**, so a session's MCP surface is
  what the launch line gave it, which is nothing. An account's MCP servers arrive with the login
  and a scratch `CLAUDE_CONFIG_DIR` cannot exclude them; `env.mcp_servers` gates at zero in the
  driven specifications, so a driven run without the flag failed its own transcript check on an
  account property no document here controls.

- **The public docs, the guides and the control documents caught up with the tree.** Four
  parallel review passes over README, AGENTS.md, `docs/guide/`, the plugin READMEs and all
  26 website pages, with one rule: every number from a command run this session, every reference
  resolving, every quoted output reproduced. What that surfaced and fixed, beyond prose: two Rust
  snippets that no longer compiled (`EvidenceSubmission::new` takes `observed_at` now), four
  evidence documents shown in shapes that no longer validate, sixteen CLI leaf verbs absent from
  the reference page (and zero phantom ones), a limitations page rebuilt from the gap register's
  20 open rows, the driver documented as shipped instead of planned — including the first
  governed run and where it stopped — and the evidence concepts page teaching the two-times
  model and decay to Unknown. Blog posts keep their published text and carry dated
  "since publication" notes where a claim aged. Three literals that drifted inside generated or
  checked surfaces are register rows rather than hand edits, beside two defects the overhaul
  found (`horizon` published as an integer where the parser wants `7d`; `ess impact` counting
  56 of 38).

- **The guard-efficacy review's last two loose ends, closed.** Every substantive finding of
  `docs/reviews/2026-08-20-guard-efficacy-review.md` was fixed by later waves — the refusal that
  authorised, the unenforced `Deserialize` ban, the one-directional approval floor, the untested
  Kleene negation, the audit disjunct, the Decimal rejection test, proptest phase 1 — and two small
  ones were not: the two guards the review caught reporting a bare `left == right` now say what
  broke and why it matters (`kleene_conjunction_keeps_false_ahead_of_unknown`, the Decimal
  structural assertions), and the `identity` conformance suite's module doc records the mutation-11
  efficacy evidence the review's D5 accepted in place of an in-CI fault, so the one suite whose
  efficacy CI cannot verify carries its measured proof where a reader of the suite looks.

- **The horizons corpus is ground truth now, and the scanner reads two more positions.** The
  adopter fixed their reference implementation against the vendored corpus and re-issued
  `expected.json` as ground truth: 43 raw annotations, 43 parsed, `missed_by_reference: 0`, with
  `reference_is_not_ground_truth` kept as a field and the reason recorded beside it. The revision
  adds position 7 — a backticked annotation mid-line, after prose, whose live instance had a
  one-day horizon and was already stale — and the rule in the other direction: an annotation
  inside a fenced code block is an illustration, excluded from parsing and from the coverage
  denominator both, because otherwise every document that explains the convention reports a
  permanent, unfixable coverage gap. Inline backticks cannot carry that meaning — positions 6 and
  7 are real claims written in them — so the rule is one-directional: fence it if you are
  illustrating, anything else parses. `aep-backend-markdown` finds 43/43 with divergence 0; the
  fence-stripping in the raw counter is implemented separately from the parser's on purpose, so
  the denominator stays independent evidence. The durable-fact lesson from the same evening — an
  answered question must not leave a permanent re-check obligation behind it — is
  `story:claim-retirement`, not a code change here.

## [0.10.0-horizons-dogfood-lab] — 2026-08-21

### Added

- **Evidence horizons — a green result from three weeks ago is not a fact.** An evidence record now
  carries two times. `observed_at` is when somebody looked, is required, is supplied by the caller
  and is the identity of the fact; `produced_at` remains the engine's, and says when the record
  entered the log. A value in the future is refused outright (`observation_in_future`): a
  scheduled-but-never-performed check stored as an observation reads as the freshest record in the
  log, and the store can no longer answer whether anybody has ever looked.

  An evidence requirement may declare a `horizon: 3d`. Past it the requirement reads `Unknown` —
  never `False`, because a lapsed check has not failed, nobody has run it — with a reason naming the
  horizon, the observation date and the day it lapsed. The transition it used to permit is refused,
  including when the guard reads a fact rather than the requirement: a lapsed record's facts are
  withheld from the store under the strictest horizon the plan declares for its kind, and an absent
  fact is `Unknown`. `evidence.lapsed` joins `evidence.missing` so a stale gate is distinguishable
  from an empty one.

  The horizon is on the requirement and nowhere else. A record has no horizon field, there is no
  operation anywhere that mutates one, and a source scan over five crates refuses both `.horizon =`
  and any `fn` taking `&mut self` with `horizon` in its name — because if `extend` is as easy to
  call as `re-check`, it is the one that gets called. Re-submitting an identical record restores
  nothing; only a new observation time does, and a test says so.

  `aep-backend-markdown` gained a scanner for the one-line dated-claim annotation convention, which
  finds all 42 annotations in the vendored corpus at `examples/evidence-horizons-corpus/` — the
  reference implementation the fixture's expectations came from finds 37 and names the five it
  misses. It reports its own coverage: raw occurrences seen versus records produced, per file, a
  divergence being a finding rather than a silent drop. New verbs: `protocol evidence scan` and
  `protocol evidence inspect`; `--observed-at` on `ess conform evidence` and `trace evidence`.

  Design: `docs/design/evidence-horizons-design-v0.1.md`, corrected by adversarial review
  (19 CONFIRMED / 15 NEEDS-CHANGE / 3 INFEASIBLE, all applied).

- **The lab runs the specification instead of replaying it.** `/lab` on the website used to step a
  hardcoded array of eleven steps with real names in it. It now fetches
  `billing_web_realized.wasm` — the browser realization this repository synthesises from
  `examples/billing/`, linked with the hand-written behaviour in `examples/billing-realization/` —
  and sends five commands over its boundary: create, issue, pay, cancel-a-paid-invoice, and one
  refused amount. The middle panel is what `{"request":"catalog"}` answers, the right panel is the
  outcomes, published events, binding invocations and view rows that came back, and the lines the
  left panel highlights are found in the file itself rather than written down. Same module, same
  glue and same engine as the page, asserted outside a browser by `npm run test:lab`; the run is
  deterministic, so two loads of the page produce the same stream byte for byte. `task lab` builds
  the module — it is a build artifact and stays uncommitted, and a page opened without one says so
  rather than showing a run it did not do.

### Changed

- **The first governed run of a real story, and the record of where it stopped.** `protocol drive`
  walked `story:agent-eval-cases` out of this repository's own `.engineering/` store under
  `development.driven` and the shipped step map, with four headless model sessions, the plugin's
  hooks as the enforcement arm and `cargo` as the verifier. It **blocked in `establish_verifiers`**
  and never reached the person it was meant to stop at, for two reasons the engine printed: the
  specification it had created was `draft` where `spec-driven` wants `approved`, and
  `test.first_result` was `passed` where `test-driven` wants `failed` — because the model wrote its
  failing tests as shell checks, which is the idiom the story's acceptance is written in, and
  `drivers/development/default.yaml` can only run `cargo`. Nothing was changed to make the run go
  through; the run is the finding. The enforcement half held and is recorded with numbers: 80 hook
  decisions, 69 allow and 11 deny, and 11 `permission_denials` — one for one, a second independent
  confirmation of F13 on a map the eval never touched. `protocol artifact validate` is exit 0 over
  the 58 artifacts the run left. Record: `docs/plan/harness-wave-4-governed-dogfood.md` § W4.1,
  *The first run*.

### Fixed

- **Four things the documents invited an adopter to declare, which the engine then refused, ignored
  or could not reach.** All four came from the first adopter's report — the first document tree
  written against this specification that is not this repository's own — and all four were found by
  writing a tree rather than by reading the guide, which is the only way this class of defect gets
  found at all. The report is a repo-local review, held and unpublished
  (`docs/reviews/2026-08-21-first-adopter-report.md`); what follows is what changed in the code, and
  stands on its own.

  **A lifecycle document may leave out `kind:`, and it becomes the tree's fallback.** The field had
  been documented as "absent for the fallback lifecycle" since it was written, and a document that
  left it out was refused — so the sentence described a mechanism that did not exist, and a team's
  own artifact kinds were governed by nothing: every status legal, a misspelt one a shrug rather
  than a refusal. A kind-less document now registers the lifecycle every kind with no nearer one is
  held to. A tree may declare **at most one**, and a second is refused by name rather than
  overwriting the first, because which of two files won would otherwise depend on the order the
  directory was walked in.

  **A kind's parent is the kind its last hyphen segment names — for every kind, not just the ones
  this repository ships.** `architecture-design` is a `design` because the suffix is the noun and
  what precedes it narrows it; that rule was written out as a list of five variants, so
  `observation-log` was *not* a `log` and an organisation's own family of kinds could not share one
  ladder. Each of them needed its own lifecycle document saying the same thing. The rule is now the
  rule: a custom kind's parent is the kind its last segment names, aliases excluded (`openapi-spec`
  is a custom `spec`, not a `specification` — an alias is a spelling somebody typed, not a claim
  about lineage), and a single-segment kind is the top of its own family. One lifecycle registered
  on `log` now governs every `*-log` a team invents.

  **`on_failure` refuses a parameter its action does not take.** `{action: retry, retry: {to:
  write}}` used to validate: the `retry` key named nothing, was dropped, and what remained was a
  bare retry falling back to `block`. The document said one thing, the engine did another, and a
  reviewer had no way to tell — a policy that validates and does nothing is a gate that cannot fire.
  Each action now has a closed parameter set, every invented key is named at once rather than one
  per round, and the published JSON Schema says the same: one closed form per action, in place of
  "a string or a mapping of anything". Every committed document still parses unchanged.

  **The project directory's name is a default, not a constant.** `.engineering` was fixed at compile
  time, so a repository that spends that name on something else, or whose team calls this
  `.workflow`, could not be discovered at all. `AEP_PROJECT_DIR` renames it, honoured by walk-up
  discovery and by everything the CLI resolves against a project. It is read **once per process** —
  a value that could change between two reads would give one run two different projects — and it is
  read in the engine, at the edge that touches the filesystem, never in `aep-domain`, which stays
  free of the environment, the clock and the disk.

### Changed

- **`docs/guide/adopting.md` now says which documents a project may add without owning a tree, and
  which oblige it to own one.** The page showed a repository with `workflows/` and `protocols/` in
  it and left the reader to assume that a project pointing at somebody else's tree could do the
  same. It cannot: the project-local merge covers **principles and profiles**, and nothing else — a
  workflow under `.engineering/` is not read at all, and the failure surfaces where the workflow is
  named rather than where the file sits. The guide now states that plainly, with the refusal it
  produces, a table of what needs a tree of your own, and the `protocols: .` layout written out —
  including the two lines of `project.yaml` that keep the tree from being loaded twice, which is the
  part that is easy to miss and refuses with a duplicate id. The merge is not being extended here;
  that is tracked as a gap.

## [0.9.0-harness-waves-2-3] — 2026-08-21

### Added

- **`protocol drive` — a workflow you specified is now a workflow that runs, and a step the protocol
  does not permit does not happen (harness wave 3).** The engine has always decided; nothing in this
  repository had ever *done* what it decided. `protocol drive run` holds the loop: rebuild the
  artifact graph from the store, ask the engine what is owed, run the next step of the state the run
  is in, submit what a verifier produced, and ask to move. It evaluates no gate of its own — a driver
  that could would be a second protocol implementation with none of the conformance suites behind it,
  and the first time the two disagreed the one nobody tested would win.

  **A step map is the missing half of a workflow, and it is a document.** `drivers/<family>/<name>.yaml`
  is the fifth document kind in the tree, loaded, validated and schema-generated like the four before
  it: per state, an ordered list of steps, and the engine is asked to move when the list is done. It
  **pins the workflow it was written against** — `workflow: adp/default/1`, mandatory — so a workflow
  that reaches version 2 orphans the map at load, refused and naming both versions, rather than
  quietly applying instructions to a state graph nobody wrote them for.

  **Three step kinds, and the important one is what an `llm` step cannot do.** A `command` step runs
  a program and records `producer: verifier`, which is how `independent: true` is honestly satisfied.
  An `operator` step prints the engine's explanation verbatim, persists, releases the lock and exits
  0 with a line somebody else can resume from — because a driver holding a terminal open for a person
  loses the run when the terminal closes. An `llm` step **has no `evidence` field**: not a rule that
  could be relaxed, a variant with nothing to put a claim in. Anything a model was supposed to
  achieve that is checkable is observed by the `command` step after it.

  **What a run leaves behind, and what a refusal reads like.** `.engineering/runs/<task>/<n>/` holds
  the engine's snapshot and the driver's cursor side by side — two documents because they have two
  owners — plus each step's log and each model session's transcript. A blocked run prints the
  engine's own sentences and does not reword them: on the shipped fixture, six moves from `receive`
  to `adversarial_verify` and then `adversarial_verify -> review: guard: evidence.missing == 0`,
  character for character in both the report and the cursor. **A crash submits nothing**, because
  absence is the fact not being in the store, and collapsing a crash into a failure sends an agent to
  fix code nobody ran. **One run per store**: a lock at one fixed path, created before a run id
  exists, refused by name to a second invocation with the holder's run id, pid and host, and gone on
  every exit path the driver controls. `--resume` re-takes it and refuses if the workflow, the step
  map or the engine moved underneath.

  **The neutrality claim is now a test rather than a sentence.** A second harness — a shell script
  with no model, no network and no credential in it — implements the same three adapter points and
  runs inside `task check`: the same step map, the same `tool_config`, the same checker, and a
  transcript in a dialect of its own that mints a `trace_conformance` record. The Claude Code adapter
  refuses that dialect, which is asserted, because two readers that accepted each other's formats
  would be one reader tested twice.

- **A driven agent's shell holds only the protocol verbs, and a hand-edit of machine-owned
  frontmatter comes back refused with a reason.** The plugin now ships two `PreToolUse` hooks, and
  they are the driver's enforcement arm rather than a second, weaker driver.

  `store-integrity` is **always on**, with or without a run: under `.engineering/planning/**` a
  whole-file `Write` or `NotebookEdit` is denied by path, and an `Edit` is denied when it crosses the
  `---` fence or writes `id`, `kind`, `status`, `revision`, `relations` or `format`. A targeted body
  edit below the fence stays legal, because prose is not the CLI's business and there is no verb for
  it. What comes back is not "denied": it names the field, says that `status` moves only through
  `protocol artifact move`, and says why — a hand-retyped frontmatter is indistinguishable from a
  silently-altered one until something downstream breaks.

  `driven-surface` is **inert outside a driven run** and, inside one, holds a shell to one simple
  invocation of `protocol artifact …` or `protocol trace …` — no pipes, no redirection, no `&&`, no
  command substitution, because a composed command line is a second command wearing the first one's
  name. Both hooks **fail closed**: with neither `jq` nor `python3` on `PATH` they refuse rather than
  pass an unread call through, and every decision is appended to the run's own
  `hook-decisions.jsonl`, which is the only record that can tell *denied* from *never attempted*.

  **A new profile, `development.driven`, and it is not a relaxation.** It is `development.standard`
  plus `command.execute`, and it exists because the planning store has no tool surface other than the
  `protocol` CLI: under the two older development profiles a driven step can be told to write a
  specification as an artifact and has no way to create one. The narrower grant cannot be written —
  `command.execute:protocol` is a parse error, since capability scoping exists for deployment
  environments and nothing else — so the grant's outer bound is the profile and its inner bound is
  the hook. The approval floor is untouched, and the store's write guard no longer rests on there
  being no shell: it rests on the shell not being able to say `sed -i`. Choose it only for a run
  under `protocol drive`; interactive work and any harness without an equivalent constraint want
  `development.standard`.

  Both are exercised by a second eval, `integrations/claude-code/eval/run-driven.sh`, which drives a
  real model through an honest step and a deliberately refused one and then judges the result by the
  store, the decision log and two trace specifications. Like its neighbour it reaches the API and
  costs money, so it is **not** part of `task check`.

- **`protocol workflow render` — a workflow, and a run over it, as a picture.** Until now a workflow
  could only be printed as YAML. Four formats behind one scene: a standalone `svg`, a self-contained
  `html` page that fetches nothing, a `png` by way of `rsvg-convert`, and a `tui` frame for the
  terminal. Hand it `--run` and it draws where the run has been, how often it entered each state,
  what evidence it produced and why it stopped — with the engine's reasons **verbatim**, never
  summarised, because a picture that paraphrased a refusal would be answering a question it did not
  evaluate. `--watch` redraws the terminal frame as a run advances.

  It **decides nothing**: the overlay is handed in as a plain value and the crate depends on the
  domain types alone, not on the engine and not on the driver. Rendering is **byte-stable** — the
  same workflow and the same run produce the same bytes — so a committed figure does not turn up in a
  diff for a reason nobody chose.

## [0.8.0-harness-wave-1-trace-wave-1] — 2026-08-21

### Added

- **`protocol trace check` — what an agent run did, judged by a typed document instead of a shell
  pipeline (trace wave 1).** A harness transcript (first adapter: Claude Code `stream-json`)
  normalizes into a content-addressed, harness-neutral event IR, and a `trace-spec/1` document
  states expectations over it — forty-nine kinds, from *the skill completed* and *this tool was
  called with these arguments* through ordering, token, cost, cache, rate-limit, tool-traffic and
  per-step timing bounds. Verdicts are three-valued: `ok`, `gap`, and `unk` for the event the
  adapter could not read or the field this transcript does not carry — exit 0/1/3, the same
  contract as `ess conform`, because "the format moved under us" wakes a different person than
  "the agent did the wrong thing". Every verdict cites the transcript event indices behind it,
  and `protocol trace inspect` prints the census — event families, per-tool traffic in both
  directions, each step's `gen`/`exec` split — from the same IR the checker judges.

  The plugin eval now runs on it: five assertions in three shell idioms became
  `integrations/claude-code/eval/expectations.trace.yaml`, forty-one expectations with the
  observed value beside every bound, checked against two committed real transcripts by the
  ordinary gate — so a bound that stops holding is caught without a paid run.

- **`protocol trace evidence` — a passing check becomes an evidence record the engine admits.**
  `Evidence::TraceConformance` carries the verdict, the three counts, every gapped expectation's
  id, any command-line downgrades, and the digest pair binding it to exactly one transcript and
  one specification; the producer is the `trace-checker` verifier class, so an agent's own claim
  of conformance never satisfies the kind. The loop is asserted end to end: the emitted document
  feeds back into `protocol evaluate --evidence` and is accepted. This is the mechanism the
  future reference driver's model-calling steps rely on — an `llm` step cannot carry evidence by
  type, and the command step that observes it now can.

### Changed

- **The vision's refusal of "a workflow engine" is narrowed: a reference driver is now in scope —
  decided and designed, not yet built.** Nothing about the engine changes; it still only decides.
  What changes is that the harness contract in [`docs/guide/harness.md`](docs/guide/harness.md) —
  seven calls, three rules — is going to get a first implementation inside this repository, the way
  the storage contract has `aep-backend-memory`. A published contract that nothing implements is the
  same defect as an invariant that nothing enforces, and it had been that since the guide was
  written.

  What did **not** move is worth as much as what did. Gates are still evaluated by the engine and
  never by a driver. Invariant 7 is untouched: an agent's own statement never satisfies an
  independence requirement, and in the driver's design that is a type rather than a rule — a step
  that calls a model has no field to put evidence in. "External systems do the work; this project
  decides what the results permit" still holds, with the driver as the first of those external
  systems, kept in-tree the way the website is.

  [`docs/VISION.md`](docs/VISION.md) carries the argument;
  [`docs/plan/control-document-updates.md`](docs/plan/control-document-updates.md) carries the
  record of who decided it and when;
  [`docs/design/harness-planning-and-driver-design-v0.1.md`](docs/design/harness-planning-and-driver-design-v0.1.md)
  § 4 is the design, which is architecture with six named open problems and is explicitly **not**
  accepted for build.

### Added

- **`protocol artifact` — planning artifacts live in your repository, and a status move is checked
  before it happens (harness wave 1).** Epics, stories, tasks and initiatives are markdown files
  under `.engineering/planning/<kind>/<slug>.md`: frontmatter the CLI owns, a body you own. Ten
  verbs — `new`, `move`, `relate`, `list`, `board`, `graph`, `validate`, `kinds`, `relations`,
  `lifecycle`.

  **A refused move tells you where you can actually go.** `move story:credential-store --to
  implemented` from `draft` does not print "illegal transition"; it prints that `implemented` is not
  reachable from `draft` and names the statuses that are. A refusal that sends you off to read a
  lifecycle file is a refusal that gets guessed around, which is the one outcome a validated
  lifecycle exists to prevent.

  **`validate` reports everything, not the first thing.** A store with four unresolvable relation
  targets reports four, each naming the artifact and the edge, and exits 1. Run it after a batch of
  edits; it is also what catches a status somebody hand-edited into a file, which a file store
  cannot prevent and this is honest about.

  **An id is declared and never allocated.** An artifact's id is `<kind>:<slug>` and must agree with
  its path. There is no counter, because two branches that both ask a counter for the next number
  both get it, both merge cleanly, and the store then holds two artifacts with one id — a corruption
  git cannot see, because nothing was in conflict. Slugs collide only when two people meant the same
  thing, and then git conflicts on the path. A consequence worth having: `story:dev-399` is a legal
  id, so a team whose tickets are named elsewhere can keep the name.

  **No timestamps in the file.** Git already knows when the file changed and who changed it, and it
  cannot be made to say otherwise by editing a line. The cost is real and stated: "how long has this
  been in draft" has no answer inside the store, and `git log` is the answer until the journal
  milestone.

  The on-disk format, `aep.planning-md/1`, belongs to `aep-backend-markdown` — `aep-domain` gained
  no types for it, and no other backend is obliged to store anything this way. It is described all
  the same: `schemas/generated/planning-document.schema.json` is generated from the parser's own
  type, so the published description of the format cannot drift from the code that refuses a bad
  one. The `format:` line is optional and defaults to `aep.planning-md/1`, so a file you write by
  hand does not need it. Unknown frontmatter keys are preserved rather than stripped, so another
  tool writing into the same file does not lose its fields.

  This is a store and **not** an implementation of the storage contract: it writes through its own
  functions rather than through `CommandService`, so the sixteen conformance suites do not run
  against it and it has no journal or audit trail yet. Both facts, and what closes them, are in
  [`docs/plan/gap-register.md`](docs/plan/gap-register.md).

- **Three new artifact lifecycles: `epic`, `task` and `initiative`.** Each mirrors the ladder
  `story` already had — `draft → proposed → active → implemented`, with `rejected` and `archived`
  where they belong — so all four planning kinds move by one set of rules that an operator learns
  once. Every status word already existed in the vocabulary, so nothing else changed to make room
  for them.

- **A Claude Code plugin, at [`integrations/claude-code/`](integrations/claude-code/).** One
  `planning` skill and two agents: `decomposer`, which breaks an epic into stories and produces
  drafts only, and `plan-reviewer`, which reads the store and changes nothing. Install it from the
  marketplace entry at the repository root.

  **The skill carries rules and no vocabulary.** It does not list the kinds, the statuses or the
  legal moves; it asks `protocol artifact kinds`, `relations` and `lifecycle <kind>` at the moment it
  needs them. A prose copy of a validated document inside a skill file is neither validated nor
  versioned, and it goes stale the first time a kind gains a status — after which the agent recites
  last month's ladder confidently and proposes a move that does not exist.

  What it does inline is four guardrails: a status changes only through `protocol artifact move`,
  the body is edited directly because the CLI does not own prose, `validate` runs after a batch of
  edits and its output is relayed verbatim, and a refusal is the answer rather than an obstacle — the
  legal moves get reported to you, not routed around by editing the file.

  **No hooks, on purpose.** Deterministic interception of what an agent may do is the reference
  driver's job, and a hook layer would be a second, weaker driver — one that sees tool calls instead
  of workflow states and cannot ask the engine anything. There is no `commands/` directory either:
  the CLI is the command surface, and a slash command wrapping a verb is a second spelling that
  drifts from the first.

  Its behaviour is checked by a repeatable eval — `integrations/claude-code/eval/run.sh` runs a
  headless session in a scratch directory and then inspects the store it left behind, asserting on
  the artifacts, statuses and edges rather than on the model's wording; it is deliberately not a step
  of `task check`, because the gate reaches no network and this calls a model.

  The eval runs **hermetically**: a scratch `CLAUDE_CONFIG_DIR` carrying only the login
  credentials, so the operator's own plugins, skills and output style cannot leak into the run —
  asserted, not assumed, from the session's init event, alongside eight other mechanical checks.
  Every report also carries run metrics (tool traffic in bytes and tokens, failing and repeated
  calls, per-step `gen`/`exec` timing derived from recorded timestamps, cache use, rate-limit
  state) and an **adversarial review**: a second, independent session reads the task, the verdict,
  the metrics, a timing-annotated timeline and the created artifacts verbatim, and reports what
  assertions cannot see. The review is advisory by design and never moves the exit code — and it
  has already earned its place once: it caught the agent re-typing machine-owned frontmatter via
  whole-file writes, the skill's guardrail was tightened in response, and the next runs switched
  to targeted edits and a clean advisory.

## [0.7.1-infra-waves-1-4] — 2026-08-21

### Added

- **`protocol infra project --spec <file> --path <bundle|ir> --out <dir>` — the gaps, handed back
  as a diff you can read (infra wave 4).** `simulate` tells you a container declares no limits.
  This writes the patch that would declare them, into a directory you review, edit and apply with
  your own hands. Nothing is applied and nothing reaches a cluster — the output is files.

  **Every value in a generated file came from the gap or from you.** A replica count outside
  `[2, 4]` has one nearest acceptable number and the range says which, so that one is written. An
  image tagged `latest` has no mechanically-nearest replacement, so it is not: you get an
  obligation naming the decision — *choose the version of `registry.local/flaky-agent` that
  container `agent` should run* — instead of a patch containing a version somebody's generator
  picked. Resources and probes sit on both sides of that line: state the values once as a
  `remedy:` on the expectation and they are written; state nothing and they are owed.

  **A patch is against the object that was observed**, not a rewritten manifest — a whole manifest
  regenerated from a snapshot silently drops every field the observation model does not keep.
  Container-level changes are emitted as *strategic* merge patches and say so in the filename,
  because an RFC 7386 patch naming one container deletes every container it does not mention.

  **The projection closes what it opens.** Raising a workload from one replica to two satisfies
  "replicas within [2, 4]" and immediately breaks "a disruption budget covers every multi-replica
  workload" — so it simulates its own changes, sees the gap it opened, marks it *induced* and
  writes the budget in the same tree. Applying the whole directory leaves more expectations holding
  and none newly broken; the test suite applies the emitted files to the bundle, recompiles and
  re-simulates to prove exactly that, including that no unrelated verdict moved.

  **`OBLIGATIONS.md` is a file of its own**, because a tree that closed nine gaps and quietly left
  sixteen decisions unmade reads, in a pull request, exactly like a tree that closed everything.
  `SUMMARY.md` carries the counts and the digests of both inputs it was computed from — a name is
  not an identity, and two revisions of your specification share a name.

  Two expectations that disagree are not silently reconciled: the one the emitted patch does not
  satisfy comes back **refused**, naming the expectation that contradicts it. Exit 0 whatever it
  finds, as `simulate` and `diagnose` already behave.

- **`infra-spec/1` expectations may carry a `remedy:`** — the value a projection writes where the
  expectation finds a field empty:

  ```yaml
  - id: shop-resources
    scope: {namespace: shop}
    expect: resources_declared
    remedy:
      resources:
        requests: {cpu: 25m, memory: 64Mi}
        limits: {cpu: 500m, memory: 256Mi}
  ```

  **A remedy never changes a verdict.** Nothing evaluates it; `resources_declared` still means
  "declares requests and limits" and nothing else, so adding one to a specification you already
  committed cannot move a simulation you already reviewed. Two new refusals guard it: a remedy
  beside a kind that can never write it is `INFRA-SPEC-009`, and one that states nothing, names a
  probe the expectation never asks for, or writes a port as `"8080"` in quotes is
  `INFRA-SPEC-010`.

- **`examples/k3d-dev-cluster/projection/` — a real patch tree in the repository.** Seven committed
  files for the committed specification and observation: two strategic patches, three generated
  disruption budgets, `SUMMARY.md` and `OBLIGATIONS.md`. Twenty-three gaps go in; nine come back as
  changes, sixteen as decisions nobody can take for you. Drift-checked by `cargo xtask infra
  --check` with an orphan scan, so a patch file nothing generates any more cannot sit there looking
  like a proposal somebody still stands behind.

- **`protocol infra simulate --spec <file> --path <bundle|ir>` — what you expected, against what
  was observed, with a third answer beside yes and no (infra wave 3).** You write an
  `infra-spec/1` document saying how the cluster ought to be, and every expectation in it comes
  back `ok`, `gap` or `unk`. A `gap` says what would have to change — *`storefront-server` declares
  2 replicas and no disruption budget covers it*, not "failed". An `unk` says why the snapshot
  cannot decide — *the `svclb` daemonset declares no replica count*, *`redis:7-alpine` names no
  registry so which one resolves it is not observed*, *namespace `payments` was not observed*, *the
  bundle did not scan `poddisruptionbudgets`*, *the bare `debug-shell` pod has an underivable
  controller, so pod counts in its namespace are a lower bound*, *the scope selects no subject*.

  **`unknown` is never quietly a failure**, and an expectation cannot pass by selecting nothing:
  a scope that matches no workload comes back `unk`, not `ok`. An expectation with one contradicted
  subject and one undecidable subject is a `gap` — something *was* observed to be wrong.

  Twelve expectation kinds, kept small and decidable: a workload exists; replicas within a range;
  requests and limits declared; probes declared; images from a registry allowlist, not `latest`,
  pinned by digest; a disruption budget covers every multi-replica workload; a service selector
  matches a pod; every required configmap and secret reference resolves; workloads only in listed
  namespaces; and a labelled predicate over eighteen `workload.*` facts, using the protocol's own
  three-valued predicate language rather than a second one. Scopes are the whole cluster, one
  namespace, or workloads carrying a set of labels.

  **No expectation asks what time it is.** Nothing compares a timestamp and there is no way to
  write a duration, so the same specification and the same snapshot always produce the same
  report — which is what lets a report be committed and reviewed as a diff at all.

  **Simulating is a report, not a gate**: exit 0 whatever the verdicts say, exactly as
  `protocol infra diagnose` behaves. Exit 1 means the input could not be simulated — a
  specification this build refuses, a bundle that is not valid, an IR document somebody edited.

- **`protocol infra diff --from <ir> --to <ir>` — what moved between two scans of one cluster.**
  Sixteen typed change kinds over the *declared* state: objects added and removed, replicas,
  images, containers, resource bounds, probes, environment, workload and service fields, ingress
  routing, configuration content, claim phases, and references that broke or healed. A configuration
  change names which keys moved and never what they hold. Pods are deliberately absent — they are
  renamed on every rollout, and a report listing a thousand of them is one nobody reads. Reordering
  a template's containers is not a change. It refuses one thing: two snapshots scanned in different
  kubeconfig contexts.

- **`examples/k3d-dev-cluster/` grew a desired state and a second scan.** `expected.yaml` is 28
  expectations reaching all three verdicts on the example cluster (11 hold, 12 gaps, 5 undecidable),
  `observation.drifted.json` is the same cluster twenty documented mutations later, and
  `simulation.json` and `drift.json` are the two reports — committed and drift-checked by
  `cargo xtask infra --check` beside the compiled IR, so a rule that starts answering `false` where
  it answered `unknown` shows up as a reviewable diff.

### Changed

- **`cargo xtask infra` writes three documents instead of one**, and `task infra-check` checks all
  three. The CI job is renamed to match.

## [0.7.0-ess-wave-7] — 2026-08-21

### Added

- **One specification, two running applications, one surface (ESS wave 7, W7.5).**
  [`examples/gatepass/`](examples/gatepass) is a new application specification — visitor passes for
  a building — and `protocol ess synthesize` now emits, for Rust *and* for Go, a binary that serves
  it over HTTP. Start either one and it writes the same three lines of JSON about itself, answers
  the same seven routes, and publishes the same contract and the same documentation, byte for byte.
  `cargo xtask synth` starts both on ephemeral ports and holds them to it.

  **A component can now say where its callers are, and that is what forces HTTP.** The model gained
  one word: `reached_by: network` on a component, against `in_process`, which is what silence has
  always meant. It names no protocol. What follows is a derivation and not a preference — a surface
  whose callers are not deployed with it has to exist on a wire, and this repository projects
  exactly one contract for a component's command surface, the `OpenAPI` document, which is an HTTP
  contract. A synthesised server speaking anything else would contradict the document committed
  beside it. **A specification that says nothing keeps everything it had**: the word is left out of
  the resolved model when unstated, so every committed artifact of every existing specification
  keeps the digest it had, and no server is emitted for a system that never asked for one.

  **A view is exposed only where the specification says something outside reads it.** The `OpenAPI`
  projection has always refused to give a view a path, because nothing in the model said how one is
  read; it still refuses, unless the component declares a network surface. Then each view gets
  `GET /{domain}/views/{view}`, its rows under one key, its declared filter in the description and
  its consistency as `x-ess-consistency` — and still no page size, no cursor, no ordering and no
  filter parameter, because the specification states none of them. A component that declares a
  network surface and has neither a command to accept nor a view to project is refused, naming what
  is missing.

  **A server and its contract cannot disagree about a path.** `ess_gen::http` holds one route
  mapping and one status mapping, and the published document, the Rust server and the Go server all
  read them. `GET /openapi.json` serves the committed contract and `GET /docs` the committed
  Markdown, both embedded at emission rather than rebuilt at run time — a server that regenerated
  its own contract could publish one nobody reviewed. A path the contract does not declare is a
  404, a declared path under another method is a 405, a body the schema refuses is a 400, and an
  obligation nothing has satisfied is a 501 naming it; none of the four is a status the contract
  declares, because each is a fact about a transport rather than about a command.

  **Neither tree takes a dependency**, and neither chooses a realization. Rust serves over
  `std::net::TcpListener`; Go over `net/http` and `encoding/json`, with generated codecs beside the
  types because a generated Go type carries an unexported field `encoding/json` cannot see. The
  hand-written halves live outside `generated/` as they always have —
  [`examples/gatepass-realization/`](examples/gatepass-realization) and
  [`examples/gatepass-go-realization/`](examples/gatepass-go-realization) — each with a linker that
  resolves exactly one implementation per obligation and names both rather than choosing when two
  are offered. The two were written from the specification rather than from each other, which is
  what makes "they answer the same way" a claim about the specification.

  **The startup record splits what the model determines from what the process does.** Everything
  outside a declared `runtime` member — the system, its version, both digests, the components, the
  plan's disposition counts, the served component, its reach, the transport, and every route — is
  derived from the specification and must be identical in every language. `runtime` carries the
  language, the address and the port. The gate *removes* `runtime` and refuses a line that has
  none, rather than comparing a list of members, so a fact the record gains tomorrow is compared
  without anyone editing the comparison.

  The browser target refuses this transport out loud rather than emitting one: a page holds the
  system in one tab and binds no socket, so a network surface is one a page would call rather than
  contain. `task check` gains no step — `synth-check` grew a fifth reason to fail.


- **`protocol ess synthesize --target web` — the billing system in a browser, and the third
  emitter behind one plan (ESS wave 7, W7.3b).** The same specification now synthesises a
  `WebAssembly` bridge and the page that drives it, committed under `generated/web/` and
  drift-checked in the gate. Open it and you can send any declared command with a typed form,
  watch the outcome it took, read the event log the transport published, redeliver an occurrence
  to see the duplicate `at_least_once` explicitly permits, watch the binding invoke the next
  command with the input it filled, and read every declared view's rows — all of it from the
  model. **Nothing about any system is typed into the HTML.** The command list, the input controls,
  the event names, the views and the lifecycles are all built at load time from a `catalog.json`
  the module carries, so a specification that changes changes the page in the same regeneration
  rather than leaving one artifact nobody regenerated.

  The plan did not change to admit a target that is not a language: `PLAN.md` and `plan.json` are
  byte-identical in all three trees. **No `wasm-bindgen`, no `wasm-pack`, no build tool and no
  third-party crate** — the boundary is three exported functions passing JSON over linear memory
  with fifty lines of hand-written glue, because `cargo build` inside the emitted tree is a
  gate step and a step that resolves a crate is a step that reaches the network.

  **The bridge chooses no realization.** Built on its own, every command answers with the typed
  refusal naming the obligation the plan owes, and the page shows that obligation's contract beside
  it — which is the honest empty state rather than an empty screen. A host crate that links one
  implementation per obligation and exports `ess_realize` turns the same page into a running
  system; `examples/billing-web/` is that host, forty lines, and gap register D-2 is untouched.

  **What a browser cannot carry is written down.** `TARGET.md` beside the plan carries six
  weakenings, none of them about a language: a `#[no_mangle]` export is flagged by rustc's own
  `unsafe_code` lint, so this one generated crate cannot declare `#![forbid(unsafe_code)]` (it
  contains no `unsafe`); a JSON boundary carries no type parameter, so an illegal lifecycle move is
  a run-time refusal here rather than a build that failed; instances are observable only as far as
  a declared view projects them, because the synthesised system holds no entity store; an
  `Integer` past 2^53 is rounded by the browser, not by the bridge; the tree is a front end over
  the Rust target's crates rather than standalone; and redelivery is a request a person makes,
  because nothing here advances a clock. A command no component accepts is a **target-stage
  refusal**: the page lists it, says why there is no form, and emits nothing to dispatch it.

  The gate builds the module for `wasm32-unknown-unknown`, checks that the page calls exactly the
  exports the module has — a page naming an export that does not exist is HTML's version of a
  dangling reference, and nothing in a browser would refuse it — and then loads the realized module
  outside a browser with Node, through the page's own `bridge.js`, and holds seventeen claims about
  one round trip. `task check` now needs the `wasm32-unknown-unknown` target and Node beside the Go
  toolchain, and says which is missing rather than skipping.

- **`protocol ess synthesize --target go` — a second emitter, and the proof that the synthesis
  plan is language-neutral (ESS wave 7, W7.3).** The same specification now synthesises a
  standard-library-only Go module beside the Rust workspace, committed under `generated/go/` and
  drift-checked in the gate along with `gofmt -l`, `go build ./...` and `go vet ./...`. The plan
  did not change to admit it: `PLAN.md` and `plan.json` are byte-identical in both trees. Go was
  chosen because it has no sum type, so every tagged union, enum and command outcome had to be
  encoded honestly — a **sealed interface**, one unexported marker method per variant set, which no
  other package can join — or refused out loud. A lifecycle becomes one type per state with
  transitions as methods on exactly the states that declare them, so an illegal move is a method
  that does not exist, as it is in Rust; a newtype becomes a struct with an unexported field, a
  constructor and an accessor, because `type Email string` would let an untyped constant become an
  `Email` by assignment.

  **What Go holds more weakly is written down, never silently downgraded.** Each module carries a
  `TARGET.md` (and `target.json`) beside the plan with four weakenings — a `switch` over a sealed
  interface is not checked for exhaustiveness, Go's zero value needs no constructor, refinement
  from a runtime state therefore answers `(value, ok)` where Rust's is total, and `==` is undefined
  for a type carrying a list, a map or bytes — each also stated in the generated doc comment where
  a reader meets it. Two things Go cannot represent at all become **target-stage refusals**, marked
  as such so they can never read as facts about the model: a `Map<Bytes, _>` (a Go map key must be
  comparable) and two obligation seams of one component that derive the same method name (a Go type
  has one method set). A refusal travels the way dependence does — the command that holds the
  unrepresentable input is refused, and so is the port that accepts it — rather than emitting a
  surface with one handler quietly missing.

- **`protocol ess diff` compares entities, commands, views and bindings (ESS wave 7, W7.2).** Ten
  construct families now, in the canonical order `system, type, entity, command, event, error,
  view, actor, component, binding`; 74 new typed change kinds — lifecycle moves and routes, an
  entity's identity, an outcome's guard, subject, emitted events, payload table and error, a
  view's filter and consistency promise, a binding's trigger, mapping and failure policy — each
  reported by name, none with a direction. Where a construct carries a predicate, the comparison
  is conservative canonical equality (gap register D-1, executed): two spellings the parser
  normalises to one form are no change, anything canonically different is *changed*, and whether
  the new predicate implies the old stays refused. An edit that used to arrive as an empty delta
  and put **everything** back to owed — a strengthened entity invariant, a moved `when:`, an
  erased payload mapping — now arrives as a named change, and `ess impact` narrows through it: on
  the worked revision pair (now six changes, ten scenarios) the invariant edit owes nine scenarios
  and twelve artifacts, the guard edit ten and ten, and neither owes a type schema. The
  fail-closed uncompared-family arm shrank to what still has no family — conversions, workloads,
  and each domain's naming, the last closing a fail-open gap where a domain's wire name, display
  name or summary could move without either a change entry or the catch-all firing. `ess-diff/1`
  documents are unchanged in shape; the new change kinds are additive rows, and pre-W7.2 deltas
  still read back.

- **`protocol infra view --path <bundle|ir> [--namespace <ns>]`** — the component view as one
  self-contained HTML page, written and opened (`$BROWSER`, else `xdg-open`). `infra graph`
  gains `--format html` for the same page on stdout. The page badge-colours each component by
  its worst finding and scopes findings and directions to the namespace when one is given; its
  only external reference is a version-pinned Mermaid script tag the viewer's browser fetches.
- **The infrastructure observation reads five more kinds, and analysis grows invariants,
  directions and an HTML component view (IW2.5).** Replicasets, jobs, cronjobs, pod disruption
  budgets and autoscalers join the model — each *optional* in a bundle, so a scan taken before
  the scanner grew them still validates and their absence stays `None`, never "none exist". Pod
  ownership is exact where the chain was observed (pod→replicaset→deployment, pod→job→cronjob,
  each edge's site the `ownerReferences` that states it); the `pod-template-hash` derivation
  remains as the old-bundle fallback and names itself on the edge. Six new diagnosis rules
  (`INFRA-DIAG-015`…`020`; none can fire on a bundle that did not scan its kind), per-workload
  properties widened to observed/ready replicas, registry-split images and budget/autoscaler
  coverage, `INFRA-PROP-001`…`003` invariant candidates (uniformity with exceptions carried as
  evidence, never as violations), a severity-ranked directions summary grouped by shared root
  cause, and `infra_analyze::render_html` — one self-contained page, Mermaid from a
  version-pinned CDN tag, optional namespace filter. Library-level; CLI flags follow. The IR
  model grew, so every `infra-ir/1` digest moves.
- **Every generated artifact carries a `contract_digest` — the digest of the model slice it
  derives from — beside its whole-model `source_digest` (ESS wave 7, W7.1).** The 36 projections
  under `generated/`, each committed conformance suite and each synthesised workspace are stamped
  through the one existing provenance mechanism: comment headers gain a `contract digest` line and
  the serialized forms (`x-ess-provenance`, suite provenance, `plan.json`) gain the field. The
  slice is the artifact's seed constructs closed over everything they rest on, by the same
  dependency graph `ess impact` walks; membership resolves every doubt by including more — a
  too-big slice costs a regeneration, a too-small one a false "still current". A suite document
  now requires the field on read: a pre-wave-7 suite no longer parses, and regenerating it is the
  fix.
- **`protocol ess impact` answers for the generated artifacts, not only the suite.** `--suite`
  is now optional and `--generated <dir>` reads the committed projection tree. The report —
  `ess-impact/2`; the document gained an `artifacts` section, `suite`/`invalidation` appear only
  when a suite was given, and the churn counts `generated_artifacts_total`/`_owed` — narrows "the
  specification moved, everything is owed" to the artifacts whose slice the delta reached, one
  path hop per line, under wave 5's exact polarity: an artifact absent from the answer was not
  reached, never "still current". Everything the analysis cannot follow is owed, stated as such —
  unreadable provenance (every pre-wave-7 artifact included), a contract digest the slice does not
  compute, a committed file the model derives nothing at, a derived artifact the tree lacks. A
  *suite* whose contract digest its own model does not compute is refused outright, because the
  short list it would produce looks exactly like a correct one.

- **`protocol infra graph` — the observed cluster as a typed dependency graph.** Edges exist
  where a reference resolved — service→workload by selector match, ingress→service,
  workload→configmap/secret/claim/service-account per env, `envFrom` and volume site,
  statefulset→governing service, pod→node, pod→workload — each under one of ten closed
  relations and carrying the sites in the dependent that state it. Deployment pods are tied to
  their deployment through the `pod-template-hash` label without observing ReplicaSets; a pod
  whose controller cannot be derived on that evidence is a typed underived-owner fact with the
  reason, never a guess. `--format json` is the canonical document (nodes, edges, sites,
  ownership facts, and the source IR's digest); `--format mermaid` draws the configuration
  topology grouped by namespace and leaves the runtime layer to the JSON; `--namespace`
  restricts either.
- **`protocol infra diagnose` — what is wrong, typed and coded, and never a refusal.**
  Fourteen rules, each finding under a stable `INFRA-DIAG-001`…`014` code with a severity
  registered on the code (error / warning / info) and named evidence: dangling selectors,
  missing required vs. optional references (an absent optional ref is info, a required one is
  an error), containers without resource bounds or probes, `:latest`/untagged images, single
  replicas, containers stuck in `CrashLoopBackOff` and kin, high restart counts,
  controller-managed pods that are not ready, orphaned configmaps/secrets/claims, unbound
  claims, and duplicate service selectors. The command exits 0 whatever the findings say — a
  diagnosis is a report about a cluster that is allowed to be wrong, not a gate; exit 1 means
  the input itself was invalid. `--min-severity` filters the listing and keeps the totals.
- **A persisted `infra-ir/1` document can be read back — through validation, never
  `Deserialize`.** `graph`, `diagnose` and `inspect --properties` accept either a bundle or a
  compiled IR document; the read-back re-verifies the digest (`INFRA-IR-002`) and re-minted
  handle by handle refuses any hand-written `resolved` reference whose key its map does not
  hold (`INFRA-IR-004`), so a forged document is refused instead of panicking a total lookup.
  Graphing the bundle and graphing its committed IR are byte-identical.
- **`protocol infra inspect --properties`** — per workload, the observed invariant-like facts
  IW3 will diff a desired state against: replica count, images parsed into
  repository/tag/digest, and the request/limit envelope per container.
- **The observation model reads two more runtime facts**: a waiting container's reason
  (`CrashLoopBackOff`, kept verbatim) and a claim's phase (`Bound`/`Pending`/`Lost`). Both are
  digested semantic state, so IRs compiled before this change carry a different digest.

- **`protocol infra validate | compile | inspect` — a scanned cluster becomes a validated,
  content-addressed IR.** An external scanner (`infra-scout`, its own repository) writes an
  `infra-observation/1` bundle; `validate` refuses a broken one with every problem in one run,
  each under a stable `INFRA-` code; `compile` turns a valid one into an `infra-ir/1` document —
  `BTreeMap`-normalized, references resolved to compiler-minted handles, danglings carried as
  typed unresolved facts rather than refused, digest = full SHA-256 of the canonical model bytes,
  provenance (`scanned_at`, `context`, `scout_version`) outside the digest so two scans of an
  unchanged cluster address the same content; `inspect` summarizes either format and refuses a
  persisted document whose digest no longer matches its content. The boundary is deliberate:
  the scanner holds the credentials, and nothing in this workspace reaches a cluster or a
  network — an observation arrives as a file or not at all.
- **A bundle carrying a plain-string secret value is refused (`INFRA-SECRET-001`), and the
  refusal never echoes the value.** The scanner already writes secrets as `{sha256, length}`;
  this rule is the second, independent enforcement, so a secret value cannot enter the IR even
  through a bundle the scanner never touched. Configmap values are digested the same way at
  validation — keys and change-detection survive, content does not.
- **`examples/k3d-dev-cluster/`** — a trimmed, reviewed observation derived from a real k3d
  scan, and the committed IR it compiles to, drift-checked in the gate (`task infra-check`,
  `cargo xtask infra --check`) and in its own CI job.

### Changed

- **A stale contract digest fails the gate as its own finding.** `generate-check`, `suite-check`
  and `synth-check` now read the contract digest out of both the committed and the freshly
  generated artifact, and a mismatch is reported as *a false claim about the model slice it
  derives from* — beside, not instead of, the byte-drift message. Same three steps, no new step.

## [0.6.1-ess-wave-6.5] — 2026-08-21

### Changed

- **The model digest is the full SHA-256 — 64 hex characters, not 16 (gap register D-4).** Since
  gate G19 a task's completion can rest on a conformance record's `spec_digest`, and since wave 5
  `protocol ess impact` refuses a suite whose digest mismatches; a 64-bit truncation is fine
  against drift and weak against construction, so the width had to follow the responsibility.
  Every provenance header, committed projection, conformance suite and synthesised workspace now
  carries the full digest, regenerated in one pass. A record written before the widening still
  parses — `SpecDigest` accepts 16 to 64 characters — and is refused where it always would be: at
  the comparison, which names both digests so the holder knows what to re-run.

### Added

- **The three invariants that were enforced by nothing are now enforced** (wave 6.5 chunk A, gap
  register): an engine that constructs an evidence payload outside its test code fails the build
  (`aep-engine/tests/evidence_scan.rs`, invariant 7); a clock, RNG or unordered map in `aep-domain`
  or `ess-gen` fails the build (`tests/determinism.rs` in each, invariant 8 — `ess-diff` and
  `ess-synth` already scanned themselves); and a second write path on the contract — any new public
  trait method beside `CommandService::execute` and the seven queries — fails the build naming
  itself (`aep-contract/tests/write_surface.rs`, invariant 14). Every scan carries an inverse
  assertion, so a scan that silently stops seeing violations fails instead of passing.
- **Property-based testing, phase 1 (`proptest`, dev-only, fixed seed).** The Kleene laws of
  `Truth` hold over generated expressions (`aep-domain/tests/truth_laws.rs`), and any generated
  adversarial specification is either refused with at least one reason or compiles to byte-identical
  canonical JSON twice — no panic, no hang, no third outcome
  (`ess-compiler/tests/adversarial.rs`). Seeds are fixed so the gate cannot be flaky; raise
  `PROPTEST_CASES` to widen a local run.
- **An outcome can declare where an emitted event's payload comes from** (wave 6.5 chunk B, gap
  register). `payload:` on a command outcome maps an event's fields onto the command's input
  (`amount: input.amount`) or a literal, with the binding mapping's own discipline read in the
  other direction: target field checked against the event, types checked with the same
  declared-conversion escape hatch (`ESS-COMMAND-002`, and the new `ESS-COMMAND-003` for a field
  the event does not carry), duplicates refused while the document form can still show them. The
  block is optional per field, and the absence is a statement: an undetermined field — a minted
  identity — is asserted for presence and type and never for a value, and there is no
  `unmapped_payload_field` refusal. Synthesis asserts the declared values, which closes the one
  fault the matrix recorded as caught by nothing: `wrong-event-payload` is now caught by
  `billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled`, blast
  radius 2.
- **A value object's invariants are read at the field positions that hold one** (design §20's last
  unsynthesised slice, wave 6.5 chunk B). New scenario family
  `<type>/invariant/at/<view>/<field>`: the type's own predicate rebased onto each observable view
  position — `Money`'s `amount >= 0` becomes `total.amount >= 0` — required of every row with at
  least one row demanded. Billing's suite grows 27→29 and its refusal count drops to zero; what
  has no witness keeps a refusal under the honest new cause (`ESS-SYNTH-013`) instead of "not
  synthesised yet". A new deliberate fault, `negative-projected-total`, corrupts one projection's
  rows and is caught by the scenario at exactly that position while the sibling position stays
  green.
- **`ess impact` fails closed on a change the delta cannot see** (mechanism 6). The construct
  families wave 5 deliberately does not compare — entities, commands, views, bindings, conversions,
  topology — are checked for canonical equality, and any difference owes the whole suite: a
  payload-only change arrives as an empty delta and `Invalidation::Whole
  { because: uncompared-family-changed }`, never as a narrowing to nothing. The arm shrinks by
  construction as W7.2 teaches the delta each family.

## [0.6.0-ess-wave-6] — 2026-08-20

### Added

- **The generated code passes the generated tests — wave 6's criterion, executed.** The committed
  billing suite (`suites/generated/billing/suite.json`, exactly as wave 4 wrote it, 27 scenarios,
  digest-checked against the workspace's plan) now runs against the synthesised workspace linked
  with hand-written obligation implementations, and passes 27 of 27. The falsifiability half runs
  beside it: one obligation implementation deliberately corrupted — `accepts-any-amount`, the
  `CreateInvoice` guard dropped — and the same unchanged suite fails exactly
  `billing.invoice.CreateInvoice/outcome/rejected`, with a blast radius of one. Both halves are
  part of `synth-check`, so CI executes the criterion rather than trusting it.
- **`examples/billing-realization` — the hand-written half of the synthesised workspace.** One
  implementation per obligation in the generated `PLAN.md`, written by reading
  `examples/billing/`: the amount guard on the wire rendering (never a float), lifecycle moves
  through the generated typestate, both view projections, the provider stand-in for `SendEmail`,
  and the escalation that records the delivery that was given up on. Hand-written code satisfies
  generated interfaces by import and never enters `generated/`.
- **The linker never chooses (gap register D-2).** Assembling the system takes exactly one offered
  implementation per obligation: zero offers is an unsatisfied obligation, two is an ambiguity
  error naming both claimants, and refusals accumulate — a linker with three empty slots reports
  three. The linker's obligation list is held equal to the committed plan by a test.
- **The generated transport is now observable where a conformance run needs it.** The system crate
  records every command a binding invoked with the input it passed (`BindingInvocation`, read by
  the runner's mapping check), and grows `redeliver` — one already-published occurrence delivered
  to its bindings again, the duplicate `at_least_once` permits, without publishing a second
  occurrence.

### Fixed

- **A binding's failure policy now answers the declared refusal, not an unfinished workspace.**
  The generated delivery arm matched `is_err()`, which conflated the provider refusing an address
  (the declared `failed` outcome) with the behaviour behind the port not being implemented yet
  (an `UnmetObligation`). It now matches the outcome enum: an error-carrying outcome takes the
  declared policy — escalate, retry, or drop — and an unmet obligation propagates out of `pump`,
  because escalating it would publish a domain event no domain fact caused. Found by W6.3's suite
  run: under the old shape a forced `SendEmail` failure produced no `DeliveryEscalated`, and
  `notify-on-invoice-created/binding/on-failure` failed.

- **`protocol ess synthesize` now emits component skeletons and one transport.** The plan's scope
  grows from `semantic-types` to `component-skeletons`: each component becomes its own generated
  crate whose port is the specification's declared surface — accepted commands as typed handlers,
  declared views as typed queries, published events as a typed outbox — and a system crate wires
  the bindings over the one transport the specification's own words determine (`at_least_once`,
  in-process, standard library only; the log of published events is the observable record). On
  billing, three of the interaction-layer refusals become generated — the binding's
  transformation and delivery, and both component ports — so the plan moves from 43 capabilities
  (29 generated / 7 obligations / 7 refused) to 45 (33 / 8 / 4). A binding is now three
  capabilities with three honest dispositions; the new obligation is the escalation, because the
  declared `DeliveryEscalated` event says nothing about how its fields are filled. A binding
  whose command zero or several components accept is refused rather than routed by guesswork.
- **Every obligation is now a typed stub in the generated workspace.** Each owed behaviour, query,
  conversion, transformation and escalation gets a trait beside its contract and an
  `Unimplemented` implementation whose body returns `UnmetObligation { capability, source }` —
  a value naming the plan entry, never `todo!()` and never a panic — so a workspace built
  entirely on stubs compiles and reports exactly what it cannot yet do. The plan's obligation
  list and the workspace's stub set are held to a bijection by the emitter and by a test.

- **`protocol ess synthesize --path <spec> [--out <dir>] [--target rust]`** — the part of an
  implementation that was never yours to write, plus the typed list of exactly what remains. Every
  semantic capability of the specification gets exactly one disposition in a language-neutral
  `SynthesisPlan`: **generated**, **obligation** (the contract is declared, the behaviour is yours —
  with the reason, in the specification author's own words where the spec declares one), or
  **refused** (with the reason, and the stage that refused). Zero guessed business logic: on the
  billing example the plan holds 43 capabilities — 29 generated, 7 obligations, 7 refusals — and
  `calculate_tax`-shaped inventions are unrepresentable, because no disposition means "generated,
  roughly".

  The Rust emitter writes a standalone zero-dependency workspace: newtypes distinct from their
  representations, tagged unions as enums, events and declared errors as types, one outcome enum
  per command with the refusal branches beside the successes, views as row types — and lifecycles
  as typestate, where the transition the specification refuses is a method that does not exist:
  `Paid → Cancelled` on the billing invoice does not compile. `PLAN.md` and `plan.json` travel
  inside the workspace. `--target` takes `rust` today; the plan itself never names a language.
- **`generated/rust/billing/` is committed and gated.** `cargo xtask synth` regenerates it,
  `cargo xtask synth --check` — a new step in `task check` and its own CI job — fails on a
  byte-level drift from the specification *and* runs `cargo check` inside each committed workspace,
  so "it compiles" is executed rather than claimed. `Cargo.lock` and `target/` inside a generated
  workspace are the toolchain's, ignored and never committed.

## [0.5.0-ess-wave-5] — 2026-08-20

### Added

- **`protocol ess impact --from <dir> --to <dir> --suite <file>`** — which scenarios a change
  invalidates, and *why*. Every impact carries the path that produced it, one hop per line — `type
  Money has a field of type type Currency` → `type Headline wraps type Money` → `entity PriceList has
  a field of type type Headline` — because an impact nobody can explain is an impact nobody acts on.

  It narrows what has to be re-established and can never widen what survives. Marking something still
  valid is not a thing the code can express: there is no such verdict in the vocabulary, the only
  combinator is a join whose top element is "invalidate the whole thing", a change to the system
  header invalidates everything, and a dependency the graph does not recognise invalidates everything.
  A suite whose digest does not match the earlier revision is refused rather than narrowed.

  On the normative example, moving an actor's grant narrows 27 owed scenarios to 7. Changing an enum
  variant narrows 27 to 23, and that is worth knowing rather than hiding: nearly every scenario acts
  on an entity, so a type most entities reach is genuinely reached by most scenarios. Authority
  changes are where the narrowing pays; type changes are where it barely does.
- **`protocol ess diff --from <dir> --to <dir>`** — what actually moved between two revisions of a
  specification, as typed changes rather than as text. On the worked fixture pair, 208 changed lines
  across three files, one of them renamed, reduce to **four** semantic changes: renaming a file,
  reordering blocks, rewriting a comment and writing out a default that was already implied all reach
  nothing, and each of those is asserted by name rather than left to chance.

  Six construct families are compared field by field — the system header, types, events, errors,
  actors and components — with 65 typed changes and no untyped catch-all. Entities, commands, views,
  bindings and topology are deliberately left out of this first slice: comparing their invariants and
  conditions means comparing predicates, which is where an undecidable answer lives.

  A change carries a direction where one can be derived mechanically and only there: a grant added or
  an enum variant added *widens*, either removed *narrows*, and everything else is simply changed.
  Three relations rather than the seven the design proposed, because four of them could not fire in
  this slice, and a variant nothing can produce is the same defect as a test that cannot fail.

## [0.4.0-ess-wave-4] — 2026-08-20

### Changed

- **A command outcome that changes an entity must say which instance.** `creates:`, `moves:` and
  `updates:` named the entity; `instance:` now names the field carrying its identity, and an outcome
  with a subject and no instance is refused. **This will refuse a specification that used to be
  accepted** — every state-changing outcome needs one word added.

  The reason is a measurement rather than a preference. A generated conformance suite could not test
  a single lifecycle transition without it: `PayInvoice` settles *an* invoice, and nothing connected
  its input to that invoice's identity, so twenty-eight scenarios across the two example
  specifications refused to generate rather than fabricate an id — and a fabricated id fails a
  *correct* implementation, which is worse than generating nothing. With the link declared, those
  twenty-eight became scenarios.

  It is declared rather than inferred, because inference has no answer when a command carries two
  fields of the identity's type and no answer when it carries none — and because an inferred link
  would silently change which scenarios exist when someone adds an unrelated field, while stored
  conformance results are keyed on exactly those names. It hangs on the outcome, not the command,
  for the reason the subject does: a command's branches disagree about what they touch, and a
  command-level key would attach an instance to a refusal.

  `creates:` is the exception and points at an event rather than the input: a created instance does
  not exist when the caller calls, so its identity is published rather than supplied.

### Added

- **A specification generates its own conformance scenarios.** All five families: one per reachable
  command outcome with the refusal branch asserting the success event did *not* occur, an externally
  decided branch reached by configuring the fault rather than by an input, a lifecycle transition
  proved and an illegal one refused, an entity's invariants checked after each state-changing command,
  and a binding checked for its mapping, its delivery guarantee and its failure policy. The normative
  example yields twenty-seven scenarios and the oracle fixture thirty-one. Nothing executes them yet.
- **The generated suite is checked against implementations that are deliberately wrong.** Ten faults,
  each injected one at a time: a wrong event, an accepted invalid amount, an illegal transition
  allowed, a dropped binding, a swapped mapping, a stale read-your-writes view, an ignored external
  outcome. Seven are caught by the scenario that exists to catch them — named, not merely "the run
  went red" — and the matrix asserts each fault's blast radius against an allowance, so a suite that
  starts over-reaching fails rather than looking thorough.
- **A command can say what happens when it is attempted in the wrong state**, and an author writes
  only the error. `wrong_state: true` with an `error:` is a fourth kind of outcome beside a guarded
  branch, a default branch and an externally decided one. The *states* are not written down: the
  lifecycle already says which states each transition may be taken from, so everything else is wrong
  by construction — add a `from:` to a transition and the branch narrows without anyone editing a
  second list.

  Until now a generated suite could only check that something went wrong, not that the right thing
  went wrong. An implementation that refuses with the wrong error passed all twenty-seven scenarios of
  the normative example; it now fails the scenario that exists to catch it. Omitting `wrong_state:` is
  still valid — the scenario is still generated, and the suite says plainly that the specification
  declares no answer for it.

  For anyone generating contracts: the branch surfaces in OpenAPI as `409`, not `422` — the caller's
  request was well formed, and telling them to fix it would send them looking for a mistake they did
  not make.
- **Two of those three faults are now caught.** A command may no longer announce an event belonging to
  a branch it did not take: every event the specification declares and the branch does not emit is
  asserted absent, scoped to that invocation. And a read-your-writes view whose command returned no
  consistency token is no longer quietly read at whatever is current — the check fails, naming the
  command that owes the token, because a weaker read that passes is a skip wearing a pass's clothes.
- **An event's payload is checked for shape.** Every declared field must be present and of its
  declared type, down to the leaves. Its *value* still is not, and cannot be: nothing in the model
  relates a command's input to an emitted event's payload, so `InvoiceCreated.amount` matching
  `CreateInvoice.amount` is a coincidence of field names rather than something the specification says.
  Closing that needs a construct in the shape `mapping:` already has, and until then the fault stays
  recorded as uncaught with its reason narrowed.
- **A view assertion names the instance the scenario acted on**, rather than meaning "the view holds
  some row". The weaker form was correct only because scenarios are isolated, and would have passed
  against a shared target for reasons unrelated to the rule being tested.
- **Three faults are caught by nothing, and the matrix records that too.** An event may be published
  with any payload, and a command may announce an event belonging to a branch it did not take, because
  synthesis asserts an event by name and writes no payload; and a target that returns no consistency
  token gets a weaker read instead of a reported failure. Each is recorded as an uncaught fault with
  the reason, and the test asserts it is *still* uncaught — so closing one of these holes breaks the
  row rather than being quietly forgotten.
- **`protocol ess conform`** — `synthesize` writes a suite from a specification, `run` executes one
  against an implementation. It can run the two reference implementations this repository ships, and
  its help says outright that it cannot run yours, with the four-line adapter recipe rather than an
  implication that more is there. Exit codes distinguish the three answers that matter: `0` conformant,
  `1` the implementation contradicted the specification or could not expose something required, `3` the
  run could not be carried out at all — because telling a harness the system is wrong when nobody found
  out is its own kind of lie.
- **The generated suites are committed and drift-checked**, under `suites/generated/`, as a seventh
  step of the gate and a CI job of its own. They sit beside the projections rather than inside them,
  because that tree has one owner and an orphan scan that deletes what its owner did not produce — two
  writers there would each delete the other's committed contract.

  The committed index also lists every construct that got **no** scenario, with the reason. A suite
  quietly holding fewer checks than it used to is the one failure a passing run cannot show you, and
  now it is a line in a diff.
- **A generated suite runs against an implementation.** A `ConformanceTarget` offers nine methods,
  each traceable to something the specification declares — execute a command, query a view, observe
  events, configure an externally decided outcome, redeliver an event, isolate a scenario. There is no
  assertion method and no escape hatch: if a step cannot be executed through concepts the model
  declares, that is a finding about the model rather than a method on the trait. All twenty-seven
  scenarios of the normative example pass against a hand-written reference implementation, and two
  runs produce byte-identical reports, because the runner owns the clock and the id source and nothing
  beneath it reaches for an ambient one.
- **A scenario the target cannot observe fails the run rather than passing quietly.** `unsupported` is
  its own status beside `passed`, `failed` and `error`, and a required scenario that ends in it makes
  conformance fail — a skip that reads as a pass is how a suite comes to certify what it never checked.
- **A binding's promises are each a test.** The mapping is asserted field by field, so a swap between
  two same-typed fields is caught rather than passing. `at_least_once` delivers the event twice and
  requires the consequence to survive it — not to happen exactly once, which is the assertion that
  looks right and fails a correct at-least-once handler. An escalation asserts the event the model
  now requires it to name.
- **`on_failure: drop` generates a refusal rather than a scenario**, saying so in the suite: a policy
  that gives up silently publishes nothing, so there is nothing to assert, and the hint says to write
  `escalate:` if it has to be provable. The refusal is the honest output — a scenario would have to
  invent an observation the specification declines to make.

- **`protocol ess graph --format mermaid`.** The system graph as a Mermaid flowchart, unfenced, so it
  can be piped into a Markdown file, a docs site or a pull request without going through the generated
  documentation tree. `dot`, `json` and `yaml` are the other spellings; `--format text` still means DOT
  and is kept as an alias of it.

### Fixed

- **An artifact evidence record could not be written in a document at all.** The evidence envelope is
  tagged by `kind`, and this one kind of record also had a field called `kind` — so the parser
  consumed the key as the tag and then reported the field it had just consumed as missing. Every
  attempt failed with `missing field 'kind'`, however it was written. The field is `artifact_kind` on
  the wire now.

  The consequence was wider than one record type: `design-by-contract` and `preserve-evidence` both
  require artifact evidence, so no `development.critical` task could satisfy either through a
  document, and none could reach `implement`. The variant existed, was documented, appeared in the
  published schema, and was unreachable from the one place a person writes evidence.
- **The CLI and the documentation page were drawing two different system graphs.** The command line
  showed no actors and no grants at all, and it grouped a command by which component *owns* its domain
  while the page grouped by what a component *accepts* and *publishes* — and the model allows those to
  differ, since a component may accept a command from a domain it does not own. Two pictures of one
  system, from two code paths, with nothing comparing them. There is now one renderer and a test that
  runs the real binary and the real generator and requires their output to match.

## [0.3.3-ess-wave-3.5] — 2026-08-20

### Added

- **A command outcome says which entity it acts on, and a transition nobody takes is refused.** An
  outcome declares `creates:`, `moves:` or `updates:`, so `CreateInvoice.accepted` creates an invoice
  and `CreateInvoice.rejected` creates nothing — the distinction lives on the outcome because a
  subject on the command would attach a state change to a refusal. A lifecycle transition no outcome
  takes is now `missing_causation`: it is a state change nothing can trigger, which is the lifecycle's
  version of a type no value can inhabit, and the refusal names the outcome that could take it.
- **The published schemas accept every spelling the parsers do.** `component:` beside `name:` in a
  specification, `id:` beside `name:` on a binding, `type:` beside `kind:` in a task, `require:` beside
  `requires:` in a workflow, and fourteen more. An editor loaded with
  `schemas/generated/ess.schema.json` marked this repository's own normative example invalid, and
  offered no fix, because the spelling it objected to was the spelling the guide's examples use. The
  aliases were always deliberate; the schema simply did not know about them, since a `#[serde(alias)]`
  is invisible to schema generation. Fifteen of the seventeen were in documents nobody had checked.
- **Conformance evidence is bound to the revision it was produced against.** A run against yesterday's
  specification no longer satisfies a requirement about today's, and a specification artifact that
  records no model digest is conformed to by nothing. The second half is deliberate and is the
  uncomfortable one: unproven is not proven, so a specification whose artifact carries no digest leaves
  its conformance requirement permanently owed until someone records one. The alternative — treating an
  unrecorded digest as "probably fine" — is how evidence outlives the thing it was evidence for.
- **`ess-conformance`** — the one piece the verification oracle cannot start without: a candidate
  command input projected into facts, and a guard decided against it. It answers with four outcomes
  rather than a boolean, because "this value does not satisfy the guard" and "this guard cannot be
  decided at all" are different answers and only the first means *try another value*. A guard ordering
  two pieces of text with no declared scale, or reading a path no type declares, is unevaluable — and
  saying so is the point, since treating it as a failure would report a specification's defect as a
  flaky test.
- **A binding that escalates must say what that emits.** `on_failure: escalate` on its own is now
  refused: write `escalate:` with `emits:` naming a declared event. "Surface it to a person" is not
  something a conformance target can be asked to prove, so a failure policy that said only that was a
  promise nobody could be held to. `retry` and `drop` are unchanged and stay single words — a retry is
  observable as another invocation, and a drop is unobservable on purpose, which is the whole reason it
  has to be typed out.
- **A property-test result carries the seed that reproduces it.** A counterexample you cannot re-run
  is a bug report without a repro, so `seed` is now part of the record — an opaque string rather than a
  number, because proptest, Hypothesis, fast-check and a fuzz corpus each spell a seed differently and
  a numeric field would force three of them to encode a lie.
- **Conformance evidence names the specification it attests**, by digest and not by a free-text string.
  A record that cannot say which specification produced it proves that some implementation passed some
  suite; it cannot prove that the implementation in front of you conforms to the specification in front
  of you.

### Changed

- **`version: 4294967296` is refused rather than silently becoming `4294967295`.** The two spellings of
  a version now agree: `v4294967296` was already refused, while the numeric form saturated, so two
  documents that disagreed about a version compared equal.
- **A YAML mapping key written twice is refused in every document this repository reads.** It was
  already refused in a specification; a protocol, principle, workflow, profile or lifecycle silently
  kept the last of the two. A profile that granted a capability twice lost one of them with no
  diagnostic.
- **A number a document cannot round-trip is refused.** `1e400` parses as an infinity, and JSON has no
  spelling for one — so it was published as `null`, turning a guard the author wrote into a guard
  nobody wrote. `.nan` likewise slipped past the constructor into a type whose documentation promises
  it cannot exist, which made ordering unreliable for every comparison against it.
- **A type or predicate nested deeper than 32 levels is refused instead of overflowing the stack.** A
  refusal names the construct and the limit; the abort it replaces named nothing.

### Fixed

- **A refused approval no longer authorises the action it refused.** A reviewer who read a change,
  refused it, and recorded that refusal was granting the production write — at three separate places in
  the engine. Also: a capability a principle denied could be downgraded to merely requiring an
  approval, an approval floor on `deployment.create:production` did not catch a profile granting the
  broader `deployment.create`, and the audit trail accepted a record that claimed a refusal and listed
  the rows it changed.
- **A validated type can no longer be conjured from a document.** Adding `Deserialize` to a type that
  is supposed to be reachable only through validation compiled and passed every check; the invariant
  every other guarantee rests on was enforced by nothing. It is now enforced mechanically.

## [0.3.2-ess-wave-3] — 2026-08-20

### Added

- **The specification now produces the documentation and the contracts, and they are in the
  repository.** [`generated/`](generated/) holds 27 files projected from
  [`examples/billing/`](examples/billing/): Markdown with Mermaid diagrams, one JSON Schema per
  command input, event payload, error payload and named type, an OpenAPI 3.1 document per component
  and an AsyncAPI 3.0 document per component. Committed rather than built on demand, because a
  contract a consumer cannot read without first installing a toolchain is a contract they copy by
  hand — and once it is committed it can be checked, so a specification change nobody regenerated
  fails the build instead of shipping a document that describes last week's system.
- **Every generated artifact says which specification produced it.** The system and its version, a
  digest of the resolved model, the compiler version and the generator version — at the top of every
  file, as a comment a person reads and as data (`x-ess-provenance`) a tool reads. When two checkouts
  disagree about an OpenAPI document the only question anyone asks is which of the two is stale, and
  the answer is now in the file rather than in whoever remembers running the generator. The digest is
  over the resolved model, not the source text, so it does not move when a comment does.
- **A named type stays a named type in every projection.** `Email` and `EmailAddress` are both a
  `String` underneath, and a projection rendering both as `{"type": "string"}` throws away the one
  distinction the model exists to make. Each keeps its own definition, its own reference and its own
  name in the schemas and in both contracts, so a code generator reading them emits two types. The
  limit is stated rather than papered over: on the wire both are a bare JSON string, so **an instance
  with the two values swapped still validates** — JSON Schema constrains structure and cannot carry
  nominal identity.
- **Where an OpenAPI path or an AsyncAPI channel comes from is a stated convention, and the generated
  document states it.** The model has no `exposures:` or `transport:` construct, so nothing in a
  specification names a method, a path, a status or a topic. Rather than invent one silently, each
  generator writes its rule into the document it produces. A command is always `POST`, at
  `/{domain wire name}/commands/{command wire name}` — `/invoices/commands/create-invoice`, with the
  `commands` segment there to stop the path pretending to be a resource, and the command's qualified
  name as the `operationId`. An event's channel address is its declared `naming.wire` or else its
  full qualified name, and every channel carries `x-ess-address-source` so a reader can tell an
  address somebody chose from one that was derived. Each of those is a rule a reviewer can disagree
  with, which is why it is written down; when `exposures:` exists it should override the convention
  rather than replace it.
- **A status code comes from the outcome, and `external` is not the caller's fault.** An outcome that
  was taken is `202`, a refusal the input decides is `422`, and a refusal decided outside the request
  is `502`. Reporting an `external` branch as a `4xx` would tell the caller to go fix the one thing
  it cannot fix and tell every retry layer in between that retrying is pointless. Outcomes sharing a
  status stay distinguishable — one response, `oneOf` the outcome schemas, each pinning its own
  `outcome` — because a status that collapsed two branches would lose the branch. `servers`,
  `security`, pagination, `201`, `ETag` and the other things an OpenAPI document usually has are
  absent: no specification backs them, and a plausible default in a contract is a claim nobody made.
- **A binding's `delivery` and `on_failure` survive the trip into the contracts.** A command some
  binding invokes with `delivery: at_least_once` gets a **required** `Idempotency-Key` header, because
  the consequence of at-least-once lands on the receiver and a surface with no way to say "this is the
  same invocation as the last one" leaves it deduplicating with no key. A command no binding invokes
  gets no header. On the messaging side both facts reach the subscriber's document, the publisher's
  document and the prose description — including `on_failure: drop`, where the work being abandoned is
  the publisher's event, so the publisher's document has to be able to say so.
- **Regenerating is byte-identical, and CI fails on a diff.** `task generate` writes the tree,
  `task generate-check` fails when the committed output is not what the specification produces, and it
  runs both inside `task check` and as a CI job of its own — "Projections up to date" — so a drifted
  contract is reported as drift rather than surfacing as an unrelated test failure. No clock, no RNG,
  `BTreeMap`/`BTreeSet` only, and a test per projection that generates twice and compares bytes.
- **A committed artifact no generator produces any more is reported as an orphan, not quietly kept.**
  A check that only compares the files a generator emits cannot see the other direction: a schema that
  was renamed or withdrawn leaves its file behind, and a consumer goes on validating against a
  contract this repository no longer stands behind. `cargo xtask generate --check` names those files
  and fails; `cargo xtask generate` removes them.
- **`protocol ess generate --kind docs|schema|openapi|asyncapi`** — and every projection at once when
  `--kind` is not given. Read-only unless `--out` is given: without it the artifacts are listed rather
  than written, because a verb that scatters files over whatever directory you happened to be in is a
  verb nobody tries twice. `--format json|yaml` carries their contents for a consumer that does not
  want a directory.
- **An entity, a view and an actor are on the generated pages.** An entity arrives with its identity
  by name and not only by type, its fields in declaration order, its invariants as the author wrote
  them, and its lifecycle as a state diagram that also lists the moves the specification does *not*
  permit — a page showing only the legal arrows reads as though the others were never considered. A
  view arrives with the entity it projects, its filter, and what its consistency level obliges a
  generated test to do: an `eventual` view asserted once races the projection, and the repair everyone
  reaches for is a sleep. An actor arrives with the commands it may invoke, drawn as edges in the
  system graph, so design §9's first arrow — somebody asking for something — is on the page instead of
  apologised for.
- **Two documents generated from one model cannot disagree about what is valid.** Every projection
  publishing a schema for a construct publishes the *same* schema for it, and a test compares them
  fragment by fragment rather than trusting that three copies of one mapping stayed equal. This
  started as a real divergence: the `AsyncAPI` document accepted an amount that was not a number and
  extra fields nobody declared, both of which the JSON Schema tree refused — so a service validating
  against one document and a service validating against the other disagreed about the same bytes. A
  difference in what a document *accepts* fails the test, and so does a difference in what it *says*
  about a construct, because a code generator reading two documents needs one answer to "which
  construct is this".
- **The published `AsyncAPI` payloads refuse what the model refuses.** They now carry
  `additionalProperties: false`, the `Decimal` pattern, the `Uuid` pattern, base64 `contentEncoding`
  for `Bytes`, `propertyNames` for a map with a non-string key, `anyOf [T, null]` for an optional
  outside a field, and a tagged `oneOf` for a union — so a branch is decidable rather than guessed. If
  you were validating events against the previous documents, messages that used to pass may now fail:
  that is the point, and each failure is something the specification never permitted.
- **An operation says which actors may invoke it** (`x-ess-may-invoke`), and no document invents a
  security scheme. `may:` states who may ask for something; an `OpenAPI` `securityScheme` states how a
  caller proves who it is, and the model says nothing about that — so a generated client would have
  implemented an authentication mechanism no specification backs.
- **A construct the documentation cannot render is named on the page where a reader went looking for
  it.** The list is an allowlist rather than a discovery, so a *new* gap fails a test and a closed one
  is a deleted entry that changes the pages with it. It is currently empty: every construct the
  specification language has reaches the IR and reaches a page. A page that quietly leaves an entity
  out reads exactly like a system that has none, which is why the empty list is a test and not a
  claim.
- **An entity, a view and an actor survive compilation.** The resolved IR carries an entity's
  identity field with its name, its fields in order, its invariants and its lifecycle; a view's source
  entity, filter, exposed fields and consistency; and an actor's grants as references that cannot name
  a command nobody declared. Before this, a specification could declare all three and everything
  downstream saw only the set of an entity's state names — so anything derived from the model was
  derived from a fraction of it.

### Not built

Test synthesis — a generated conformance suite, and an implementation deliberately wrong to prove the
suite bites — is ESS wave 4; Rust structural synthesis is wave 5. Entities, views and actors reach
the documentation but no contract projection derives from them yet: a view is a read model an
`OpenAPI` document could expose and does not, and an actor's grants are authorization rather than
authentication — the model states who may invoke a command and says nothing about how a caller proves
who it is, so no document here emits a security scheme. Every schema each document embeds is
validated against the 2020-12 meta-schema, but the `OpenAPI` and `AsyncAPI` envelopes themselves are
checked structurally rather than against the `OpenAPI` 3.1 and `AsyncAPI` 3.0 meta-schemas: neither is
vendored here.

## [0.3.1-ess-wave-2] — 2026-08-20

### Added

- **A system's decomposition, interaction and runtime shape are part of the specification.** Three
  layers above the domains, each answering something the domains cannot: which component owns which
  bounded context, what happens when an event occurs, and how many instances the design needs to be
  correct. A component is not a deployment — whether `invoice-service` ships as a process or a module
  is the topology's business, and changing that answer changes nothing in `domains/`.
- **A binding says what happens when it fails.** `delivery:` and `on_failure:` are required words, not
  defaults. A binding that can fail silently is the difference between specifying a system and
  specifying a demo, and the way that difference disappears is a default nobody read. `drop` is legal
  and has to be typed: a system that loses work is a decision, and the decision has to be findable in
  the document that made it.
- **A mapping between two bounded contexts is typechecked.** `InvoiceCreated.customer_email` into
  `SendEmail.recipient` is the one place two independently-written contexts must agree about a type,
  so it is the one place a rename in one breaks the other silently. Both sides are resolved, and the
  refusal names both paths, both types, and that no conversion is declared.
- **A type crossing must be declared, with a reason.** `Email` and `EmailAddress` are both a `String`
  underneath, and the whole value of naming them apart is that the model refuses to treat one as the
  other. `conversions:` records the crossings that are intended and requires `because:` — a conversion
  with no reason is exactly what this declaration prevents: a widening someone added to make a build
  pass, which the next reader finds and cannot evaluate. Crossings are directional.
- **`ess-compiler`** — resolution, a normalized IR whose type carries the guarantee that every
  reference resolves, and diagnostics with a stable code, a `file:line` and a machine-readable body.
  A `Specification` holds names that *probably* resolve; anything downstream either re-checked them or
  trusted that someone else had, and both are how a generator emits code for a type that does not
  exist.
- **`protocol ess compile`, `ess inspect`, `ess graph`.** `inspect` resolves a name in any of seven
  namespaces and refuses an ambiguity rather than guessing; `graph` emits DOT with components as
  clusters, and its output is byte-identical across runs.
- Generation is reproducible, and there is a test that says so rather than a comment: the same source
  compiled twice is byte-identical. `BTreeMap`/`BTreeSet` only, no clock and no RNG anywhere in the
  compiler.

### Fixed

- **A legitimate expression tree was refused.** A type reaching itself through a union was treated as
  a forbidden dependency cycle, but `Expr = union {leaf: Integer, pair: Pair}` with
  `Pair = struct {left: Expr, right: Expr}` is perfectly ordinary — every value of it bottoms out in
  a `leaf`. The rule now asks the question that matters, whether any value of the type can exist,
  rather than the shape that usually causes the answer to be no. A union needs one buildable variant,
  not all of them, and the refusal now names which requirement is unmet instead of only that
  something is.
- **A key written twice was silently discarded.** `serde_yaml` keeps the last of two identical mapping
  keys and says nothing, so a document declaring the same workload, type or even `system:` twice lost
  one of them. Reading now goes through a stage that refuses it, with the key and the line — one check
  covering every mapping in the format rather than one per section.
- A binding's mapping could not report an input mapped twice, because the raw form was a map and the
  duplicate was gone before anything could look.

### Changed

- Two new validation codes distinguish faults that were being reported as each other:
  `misspelled_reference` for text written where a reference was meant — `evnt.customer_email` parses
  clean and gets *sent* — and `unsupported_construct` for something this build will implement later, as
  against `unsupported_format_version` for a document it cannot read at all. "Upgrade the tool" and
  "write it another way" are different instructions.

## [0.3.0-ess-wave-1] — 2026-08-20

### Added

- **A system can be specified, and the specification can be refused.** `ess-domain` is the typed
  model for an Executable System Specification: domains, entities with lifecycles, commands with
  outcomes, events, errors, views with declared consistency, actors and a type system with tagged
  unions. `protocol ess validate --path <file-or-directory>` parses one and reports every problem in
  a single run, each with a code and a location.
- **[`examples/billing/`](examples/billing/)** — the single normative example, parsed by a test, and
  checked to exercise *every* construct the model has: each type kind, each primitive,
  `Optional`/`List`/`Map`, both consistency levels, an actor with grants and one without. A construct
  added to the model without reaching the example fails the build, because what the normative example
  leaves out is what nothing checks.
- **A command that can be refused says so.** Outcomes rather than a bare `emits` list: a command with
  a precondition has at least two results, and a specification recording only the happy one generates
  a suite that never checks the branch where the money does not move.
- **An outcome the input cannot decide says that too.** `external: <the cause>` marks a branch caused
  by the world — a mail provider rejecting an address — so a generator injects a fault instead of
  trying to construct an input for it. `when: false` would have claimed the branch was unreachable,
  which is a different and false statement.
- **A projection declares its consistency**, which is what decides whether a generated assertion is
  `eventually` or immediate — rather than a sleep, which makes a suite test the machine it runs on.
- **A declaration is addressable from outside** — `ep://acme/billing/ess-command/billing.invoice.CreateInvoice`,
  the protocol's own scheme rather than a new `ess://` one, so an approval against a command in a
  specification is recorded the same way as an approval against a design.
- **[`schemas/generated/ess.schema.json`](schemas/generated/ess.schema.json)** — an editor validates
  a specification as it is typed. Generated from the same Rust types the validator runs, drift-checked
  in CI, and the generated index now lists every published schema so one cannot land undocumented.
- **[`docs/guide/specification.md`](docs/guide/specification.md)** — how to write one, and what the
  model insists on.
- **[`docs/VISION.md`](docs/VISION.md)** — what this project is for, and how its two halves compose:
  AEP governs how engineering work is performed, ESS specifies what software must exist, and they
  meet at evidence.
- **[`docs/design/ess-implementor-design-v0.1.md`](docs/design/ess-implementor-design-v0.1.md)** —
  the Executable System Specification design: a system described once as a typed semantic model, from
  which contracts, documentation, tests, deployment artifacts and structural code are derived.
- **[`docs/design/ess-review-v0.1.md`](docs/design/ess-review-v0.1.md)** — a review of that design
  against what this repository learned building the same shape twice: eleven findings, three of which
  would make generated tests assert false things, and a narrower recommended v0.1 scope.
- **A task can require conformance to a specification.** `ArtifactKind::ExecutableSystemSpecification`,
  `EvidenceKind::EssConformance` and the `ess-conformance` principle — conditional on the project
  having a specification, and satisfied only by `independent: true` evidence from a
  `conformance-runner`. An agent's own report that its implementation matches the specification is
  not evidence that it does.

### Changed

- **A validation error names what actually went wrong.** A specification had been borrowing the
  protocol's document codes, so a duplicated command name reported `duplicate_principle` and a
  missing event reported `unknown_state`. Nine codes now say what they mean —
  `undeclared_reference`, `duplicate_declaration`, `missing_declaration`, `empty_declaration`,
  `conflicting_declaration`, `type_mismatch`, `unsupported_format_version`,
  `non_exhaustive_branches`, `unreachable_branch` — and sixteen places in the protocol half moved
  onto them too, so an undeclared reference is not one code in a specification and a different one
  in an artifact manifest.
- **The published schemas accept what the parser accepts.** Ten document types had a hand-written
  parser and a derived JSON Schema, so the schema described the *representation* rather than what an
  author writes: a bare `- verification` evidence requirement, a one-line objective, a
  `require_approval` capability, an `in-review` status. Twenty-eight rejections across eighteen of
  this repository's own documents. Every schema is now checked against every document the repository
  ships.
- `v01` and `ess/01` are refused. Both parsed, and both were rejected by the pattern the same build
  published — a document an editor called invalid and the tool accepted.

### Fixed

- **A schema that called the normative example invalid.** `version: v3` is what every document says;
  the published schema required an integer.
- **A guard that could not guard.** The list of validation codes the tests iterate was maintained by
  hand and had fallen five codes behind the enum, while its own comment claimed that adding a variant
  without listing it would fail the test. The enum, its wire strings and the list are now generated
  from one declaration.
- Rules that existed and were never reached: an error's payload types and an event's duplicate fields
  were checked by methods nothing called.
- A specification could name a domain in the header that nothing declares, declare an actor no domain
  owns, define two types that cannot be built without each other, filter a view on a lifecycle state
  the entity does not have, declare a type no value can be, or declare a union with no tag field. All
  six are refused.
- **A misspelt key in a type declaration was silently dropped.** `invarants:` on a value object
  parsed clean and lost the invariant, because a flattened body rules out `deny_unknown_fields` at the
  outer level. It is now a parse error with a line number.
- **A type's invariants are predicates, checked against the type's own fields**, as an entity's
  already were. `nonexistent_field >= 0` on a value object was accepted, and so was text that is not
  a predicate at all.
- A field name must survive into generated code as an identifier. `""` and `not a field name!` were
  accepted.
- An entity invariant may read the identity field. It could not, although a view projecting the same
  entity could — so a valid specification was refused with a message that was not true.
- A field may not shadow the identity's name, which produced two fields with one name and different
  types.
- A state whose only transition returns to itself is a dead end. A self-loop was counted as an exit,
  so an entity could reach a state it can never leave.
- A domain can be given a wire and display name. `naming:` on a domain file was refused, although the
  model has always carried it — so a bounded context's wire name was unreachable from any document.
- A malformed header no longer hides the reference errors under it.
- `protocol ess validate` names the file a problem is in when the specification is one file, refuses
  a directory that is not a specification instead of reading every YAML file it can find, and reads
  each file once when a symlink points back up the tree.
- `cargo xtask schema --check` fails on a schema nothing generates any more, not only on one that
  drifted.

### Not built

No compiler, no OpenAPI, no test synthesis: those are ESS waves 2 and 3 in
[`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md). Conformance evidence is produced by hand.

## [0.2.1] — 2026-08-20

### Added

- **A project can be discovered.** `.engineering/project.yaml` names the protocol, the profile and
  where the protocol tree lives; `protocol resolve` and `protocol evaluate` run with no arguments
  anywhere inside a project, walking up to find it. An adopting team's first command no longer needs
  four paths.
- **Project-local principles and profiles.** `.engineering/principles/` and `.engineering/profiles/`
  are merged over the protocol tree's, because no organisation's rules are entirely somebody else's.
  They are documents in the same format, validated the same way — and a project-local profile still
  cannot grant a capability the protocol's approval floor keeps behind approval.
- `protocol resolve` and `protocol evaluate` report where their inputs came from, so it is never
  ambiguous whether a flag or the project supplied them.

### Fixed

- **The approval floor was inert for every `adp/1` and `aop/1` profile.** `Protocol::extend` merged
  capabilities, evidence kinds, verifiers, phases, observables and scales — but not the approval
  floor, and neither derived protocol declares one of its own. A profile written against `adp/1`
  could therefore grant `production.write` outright and resolution would accept it, while three
  documents claimed that was impossible. The shipped profiles were unaffected because each
  hand-writes `require_approval`; the check meant to make the mistake impossible was doing nothing.
  Now inherited, with a regression test over the real documents that fails without the fix.
- **The CLI crashed when its reader stopped reading.** `protocol inspect | head -3` ended in a panic
  and a stack trace, because Rust's `println!` panics on a closed pipe. Output now ends quietly.

## [0.2.0-wave-3] — 2026-08-20

### Added

- **`aep-conformance`** — sixteen black-box suites a backend runs against itself to prove it
  implements the contract: identity, command execution, idempotency, optimistic concurrency, query,
  consistency, relations, history, immutability, audit, rejected-action audit, correlation, causation,
  provenance, events and type discovery. Reports name the *property* that failed, not the assertion,
  so a failure says what to fix.
- **Conformance levels** — `core`, `audited`, `full`. A backend states what it claims and the suite
  proves or refutes it, instead of a README asserting it.
- **`FaultyBackend`** — a wrapper that breaks exactly one property at a time. The crate's own tests
  assert that the suite responsible for each fault fails and the others still pass, because a suite
  that passes everything tells you nothing about whether it would catch anything.
- **`protocol conformance --level core|audited|full [--suite <name>] [--inject <fault>]`** — runs the
  suites, and can deliberately break a property to show which suite catches it.
- **`adp-domain`** — development types (`adp.specification/v1`, `adp.test-plan/v1`,
  `adp.acceptance-criteria/v1`, `adp.change/v1`) and commands (`adp.story.start/v1`,
  `adp.story.complete/v1`, `adp.test-plan.record/v1`, `adp.specification.satisfy/v1`). A
  specification declared satisfied by no evidence is refused — the exact claim the protocol exists to
  stop.
- **`aop-domain`** — operations types (`aop.incident/v1`, `aop.runbook/v1`, `aop.release/v1`) with
  their status ladders, and commands (`aop.incident.acknowledge|mitigate|resolve/v1`,
  `aop.release.promote|rollback/v1`). Promoting to production without naming an approval is refused
  at the command, which is a second defence beside the protocol's approval floor.
- **`docs/guide/`** — how to adopt the protocol, wire a harness to the engine, and implement and
  prove a backend.
- `Fault::caught_by()` names the suite responsible for each fault, and the crate's own tests assert
  that suite fails when the fault is injected. `DropAffected` fails eight suites, which is a finding
  about how load-bearing `affected` is rather than a flaw in the suites, and is recorded as such.

### Changed

- The in-memory backend now **refuses an update to an immutable type**. A review result records what
  someone concluded at a moment; editing it afterwards changes what the record says a person decided.
  Archiving stays available — keeping a record and editing it are different acts.

## [0.2.0-wave-2] — 2026-08-20

### Added

- **Identity.** Every addressable thing now has an opaque `EntityId`, a logical `EntityLocator`
  (`ep://acme/payments/design/passkeys-auth`), a versioned `EntityType` (`aep.design/v1`) and a
  monotonic `EntityRevision`. `AUTH-142` is a key in a locator, not identity — so two repositories can
  refer to the same design, and an approval can name the exact revision it approved.
- **`ActorRef`** — `human:alice`, `agent:planning-agent`, `service:release-controller`, `system`.
  Distinct from an evidence `Producer`: an actor bears responsibility, a producer made an observation.
  Commands carry both an actor and an executor, so "alice authorised it, agent-17 ran it" is
  answerable, and a trail that collapses them can answer neither question.
- **`aep-contract`** — the storage-independent interaction contract: `CommandService` and
  `QueryService`, command envelopes with the six identifiers that make a trail reconstructable,
  consistency tokens giving read-your-writes without sleeps, a typed failure taxonomy, and
  `TypeDescriptor` so a harness can ask what a design is instead of hard-coding it.
- **Commands** (`aep-domain::command`) — six generic (`CreateEntity`, `UpdateEntity`,
  `CreateRelation`, `RemoveRelation`, `ArchiveEntity`, `SupersedeEntity`) and three domain
  (`SubmitDesignReview`, `ApproveDesign`, `AcceptAdr`). A domain command can be validated where a
  generic patch cannot: `ApproveDesign{design@7, review}` checks that the review is about *that*
  revision.
- **Domain events** (`aep-domain::domain_event`) — a versioned event vocabulary with an open
  `Custom` variant, separate from the protocol's execution events. An event caused by a command
  names that command as its cause.
- **Audit records** (`aep-domain::audit`) — actor and executor, correlation and causation, decision
  records and change records with before/after revisions, and **rejected attempts**: a denied command
  changes nothing and still leaves a record, which is the half most systems lose.
- **`aep-backend-memory`** — a complete in-memory implementation of both contract surfaces, so the
  contract is exercised by something before anyone builds a durable backend. It passes the
  specification's nineteen-step reference scenario, including idempotent replay, stale-revision
  conflicts and the audit record a refusal leaves behind.
- **`aep-engine::trail`** — protocol decisions become audit records, and a command issued during an
  execution inherits its correlation, execution and task. A refusal by the protocol and a refusal by
  a backend now land in the same trail, queryable the same way.
- Evidence may be submitted as an entity reference, so the trail points at the stored evidence rather
  than at the engine's copy of it.
- `RelationKind::Delivers`, and `ArtifactKind::entity_type()` mapping the human-facing artifact
  vocabulary onto entity types.
- **CLI**: `protocol entity list|get|history|relations`, `protocol audit [--correlation|--entity|
  --rejected]` and `protocol describe <type>`, backed by an in-memory backend seeded from an artifact
  manifest through real commands — so seeding produces history and audit records like anything else.

### Changed

- **Nine new `ValidationCode`s** — `self_reference`, `empty_change`, `refusal_mutated_state`,
  `unreconstructable_change`, `unexplained_decision`, `redaction_inconsistent`,
  `event_payload_mismatch`, `incomplete_event_subject`, `missing_causation`. Previously these
  failures all reported `unknown_state`, so a caller could not tell "this audit record claims a
  refusal changed something" from "this workflow references a state that does not exist".
- Minimum supported Rust version is 1.85 (`Waker::noop`, which lets the contract define `async fn`
  traits without an executor dependency or a line of `unsafe`).
- A protocol may declare an **approval floor** — capabilities no profile may grant outright.
  `aep/1` declares `production.write` and `deployment.create:production`, and a profile that grants
  one fails to resolve.

## [0.2.0-wave-1] — 2026-08-20

### Added

- **The execution core.** `aep-engine` resolves a task against a document tree and answers what is
  owed, what may be done, which transitions are permitted and whether the task is complete:
  - `registry` — the documents in force, with the cross-document checks (unknown references, pinned
    version mismatches, undeclared capabilities and evidence kinds, evidence no verifier can
    establish);
  - `load` — reads a document tree, reporting every bad file with its path rather than the first;
  - `resolve` — task + registry → execution plan: `extends` chains merged, principles filtered by
    applicability, capabilities composed with the document responsible recorded for each entry,
    obligations collected, and the whole configuration checked for rules that could never fire;
  - `execution` — live state with derived facts (`evidence.first_seq.*`, `test.first_result`,
    `evidence.missing`) and a serialisable snapshot;
  - `evaluate`, `policy`, `explain` — what is owed, capability decisions naming the rule that
    decided, and the `✓ / ✗ / ?` completion checklist;
  - `engine` — the `ProtocolEngine` trait, deterministic transitions, an injected `Clock`.
- **The documents.** 42 of them: `aep/1` plus `adp/1` and `aop/1`; 21 principles across intent,
  construction, verification and governance; 4 workflows (development, incident, progressive release,
  forward-only migration); 5 profiles; 5 artifact lifecycles; artifact kind and relation definitions;
  8 templates.
- **`protocol` CLI** — `validate`, `resolve`, `inspect`, `evaluate`, `explain`, `schema`, with
  `--format text|yaml|json`.
- **Worked example** (`examples/development-passkeys/`) — a task, its artifact graph and a five-step
  evidence sequence that walks to completion, replayed by the integration tests.
- **Protocol approval floor.** A protocol may declare capabilities no profile can grant outright;
  `aep/1` declares `production.write` and `deployment.create:production`. A profile that grants one
  fails to resolve.
- **`Action::ProductionMutate`** — production changes that are not deployments now have an action, so
  a policy naming only deployments cannot let them through.
- **CI** — GitHub Actions mirroring `task check`, with schema drift as its own job.

### Fixed

- `evidence.missing` counted evidence required by conditional rules that did not apply, so a task
  could show every requirement met and still be unable to finish.
- The approval floor is now violated by any *overlap*: granting `deployment.create` for every
  environment no longer slips past a floor on `deployment.create:production`.
- A task may name the base protocol its profile refines (`aep/1` with a profile written against
  `adp/1`), which is the form the design documents use.

### Changed

- Evidence files spell the envelope's subject `about`, not `subject`, so it cannot silently consume a
  payload's own `subject` — a review's subject is the artifact reviewed.
- `protocol evaluate` exits `0` whenever it produced a report. A blocked execution is an answer, not
  a failure; `explain --action` still exits `1` when an action is refused.

## [0.1.0] — 2026-08-19

### Added

- **`aep-domain`** — the source-of-truth model: identifiers and versioned references, a three-valued
  predicate language, facts and ordered scales, capabilities with default-deny, actions, evidence with
  provenance, verifiers and counterexamples, the artifact graph with lifecycles and typed relations,
  review semantics with revision-bound approval, requirements over evidence/artifacts/reviews/
  approvals/conditions, principles with phase-timed obligations, workflows, tasks, protocols,
  profiles, execution plans and the audit event vocabulary.
- **`aep-schema`** — document reading that separates syntax from semantic failure, and JSON Schema
  generation for six document types and four interchange types.
- **`xtask schema [--check]`** — schemas are generated from the Rust types, and CI proves they match.
- Repository scaffolding: workspace, `Taskfile.yml` gate, Apache-2.0 licence, `AGENTS.md`.

[Unreleased]: https://github.com/codewandler/engineering-protocols/compare/0.7.1-infra-waves-1-4...HEAD
[0.7.1-infra-waves-1-4]: https://github.com/codewandler/engineering-protocols/compare/0.7.0-ess-wave-7...0.7.1-infra-waves-1-4
[0.7.0-ess-wave-7]: https://github.com/codewandler/engineering-protocols/compare/0.6.1-ess-wave-6.5...0.7.0-ess-wave-7
[0.6.1-ess-wave-6.5]: https://github.com/codewandler/engineering-protocols/compare/0.6.0-ess-wave-6...0.6.1-ess-wave-6.5
[0.6.0-ess-wave-6]: https://github.com/codewandler/engineering-protocols/compare/0.5.0-ess-wave-5...0.6.0-ess-wave-6
[0.5.0-ess-wave-5]: https://github.com/codewandler/engineering-protocols/compare/0.4.0-ess-wave-4...0.5.0-ess-wave-5
[0.4.0-ess-wave-4]: https://github.com/codewandler/engineering-protocols/compare/0.3.3-ess-wave-3.5...0.4.0-ess-wave-4
[0.3.3-ess-wave-3.5]: https://github.com/codewandler/engineering-protocols/compare/0.3.2-ess-wave-3...0.3.3-ess-wave-3.5
[0.3.2-ess-wave-3]: https://github.com/codewandler/engineering-protocols/compare/0.3.1-ess-wave-2...0.3.2-ess-wave-3
[0.3.1-ess-wave-2]: https://github.com/codewandler/engineering-protocols/compare/0.3.0-ess-wave-1...0.3.1-ess-wave-2
[0.3.0-ess-wave-1]: https://github.com/codewandler/engineering-protocols/compare/0.2.1...0.3.0-ess-wave-1
[0.2.1]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-3...0.2.1
[0.2.0-wave-3]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-2...0.2.0-wave-3
[0.2.0-wave-2]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-1...0.2.0-wave-2
[0.2.0-wave-1]: https://github.com/codewandler/engineering-protocols/compare/0.1.0...0.2.0-wave-1
[0.1.0]: https://github.com/codewandler/engineering-protocols/releases/tag/0.1.0
