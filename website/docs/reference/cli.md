---
title: CLI reference
sidebar_position: 1
description: Every subcommand of the reference CLI, grouped by surface — protocol, entity, ESS and infrastructure — with exit codes.
---

# CLI reference

The reference CLI is `protocol`, built with `cargo build --release -p protocol-cli` and left at
`target/release/protocol`. `--help` on any subcommand carries the full flag list — this page is the
map.

Most verbs take `--format text|yaml|json`, with `text` the default: refusals, decisions and
evaluations all serialise. The exceptions are named in their own sections, and they are exceptions
because the thing being rendered is not a report — a graph has `dot` and `mermaid`, a drawing has
`svg` and `png`, and the two verbs that mint evidence default to `yaml` because that is what
`protocol evaluate --evidence` reads back.

General conventions: exit `0` is success, exit `1` is a refusal or invalid input, and errors
accumulate — a run reports every problem it found, not the first. Two exceptions are deliberate and
stated where they apply. A verb that *reports* rather than gates exits `0` whatever it found: that
covers `protocol evaluate` on a blocked execution and the four `infra` report verbs. And the two
verbs that judge an implementation or a run use a third code for *unverified* — `ess conform run`
and `trace check`.

## Protocol surface

| Command | Does |
|---|---|
| `protocol validate [--root .] [--artifacts m.yaml]` | checks a document tree structurally and semantically — including that every rule could actually fire |
| `protocol resolve --task task.yaml [--root .]` | resolves a task into an execution plan: workflow, principles in force, capabilities, obligations |
| `protocol inspect [reference]` | shows what a protocol, principle, workflow or profile declares — `aep/1`, `test-driven`, `development.standard` |
| `protocol evaluate --task … [--artifacts …] [--evidence e.yaml]… [--advance]` | evaluates an execution: what is owed, what is permitted, what is missing; `--advance` also attempts transitions |
| `protocol explain --task … --action production.write` | explains one decision — allowed or denied, by which rule, and what would unlock it |
| `protocol schema [name]` | prints the generated JSON Schemas, or one by file stem; no `--format`, because the output is already JSON |
| `protocol conformance [--level core\|audited\|full] [--suite name] [--inject fault]` | checks a storage backend against the AEP contract suites (16 suites at `full`, 14 at `audited`, 7 at `core`); `--inject` breaks one property on purpose to show the responsible suite fails |

Inside a project — a directory holding `.engineering/` — `resolve`, `evaluate` and `explain` take
their `--root`, `--task` and `--artifacts` from `.engineering/project.yaml`, so the three long paths
collapse to the verb. `explain --action` exits `1` when the answer is *denied*, which is what lets a
harness ask before it acts.

## Planning surface

`protocol artifact` reads and writes the markdown planning store: one artifact per file, YAML
frontmatter, free markdown body, under `<project>/.engineering/planning/` unless `--store` says
otherwise. The consequence for a person is the reason it is markdown and not a database: the diff of
a status move is one line, and `git log` already knows who made it.

Every verb here takes `--store` and `--root` (the document tree the lifecycles and templates come
from, default `.`).

| Command | Does |
|---|---|
| `protocol artifact new <kind> <name> --title … [--summary …] [--owner …] [--tag …] [--relate rel:id]` | writes one file, at the path the id determines; refuses to overwrite an existing one |
| `protocol artifact move <id> --to <status>` | moves it if the kind's lifecycle permits, and on a refusal names every status it could have moved to instead |
| `protocol artifact relate <id> <relation> <target>` | adds one edge |
| `protocol artifact list [--kind …] [--status …]` | the plan, one line per artifact |
| `protocol artifact board [--kind …]` | the same plan as status columns |
| `protocol artifact graph [--format dot\|json]` | the plan's graph — `dot` for `dot -Tsvg`, `json` for a consumer that would otherwise parse a diagram |
| `protocol artifact validate` | every file, every edge, every status, accumulated into one list: a file where its id does not put it, an edge pointing at nothing, a cycle, a duplicate id, a status the lifecycle does not have |
| `protocol artifact kinds` | the 26 artifact kinds, marking which are planning rather than output |
| `protocol artifact relations` | the 13 relations, with what each edge means |
| `protocol artifact lifecycle <kind>` | where a kind starts, and what may follow what |

`new`, `move` and `relate` write without an `--out`, unlike `ess generate` and `ess synthesize`.
The difference is that they write exactly one file, at a path the id determines, inside a directory
somebody opted into — and an item you did not want is removed with `rm`.

## Driver surface

`protocol drive` walks a workflow: it makes the engine's calls in order, runs the three kinds of
step that touch the world — a program, a model, a person — and records what it did. It evaluates no
gate itself, because a driver that could evaluate a gate would be a second protocol implementation
with none of the conformance suites behind it.

| Command | Does |
|---|---|
| `protocol drive run [--map <file-or-id>] [--pause-on-approval] [--max-iterations 25] [--take-lock]` | starts a new run of a task, allocating a run id such as `AUTH-142/3` |
| `protocol drive status [--run <id>]` | what the store's last run is doing, and who holds the lock |
| `protocol drive resume <run> [--pause-on-approval] [--max-iterations 25] [--take-lock]` | continues a run that stopped, re-taking the store lock |

All three discover `--project`, `--root`, `--task` and `--store` from the project when omitted, and
take `--plugin-dir` (repeatable; `AEP_DRIVE_PLUGIN_DIR` supplies it when the flag is absent) to load
a harness plugin into every `llm` step's session. `--pause-on-approval` runs until the first thing a
person owes, then persists and exits `0`. `run` and `resume` exit `0` when the run completes or
stops awaiting an operator, and `1` otherwise.

`protocol workflow render` draws the same thing for a reader: the states down the page, the guards
beside the arrows, and — with `--run` or `--state` — where a run is, where it has been, what it
produced and why it stopped. It evaluates nothing; every overlay was decided by the engine and read
out of a run directory.

| Command | Does |
|---|---|
| `protocol workflow render --id adp/default [--root .] [--format svg\|html\|png\|tui] [--out f]` | the workflow, as a standalone SVG, a self-contained HTML page, a raster image by way of `rsvg-convert`, or one terminal frame |
| `protocol workflow render --id … --run AUTH-142/3 [--project …] [--watch]` | the same figure with a driver run drawn over it; `--watch` redraws as the run advances, and is `--format tui` with `--run` only |
| `protocol workflow render --id … --state snapshot.yaml` | an engine snapshot drawn over it instead |

Without `--out`, everything but `png` goes to standard output.

## Evidence surface

The observation half of evidence horizons. Neither verb writes anything, neither resolves a plan and
neither decides a gate: they report what a document says about when somebody last looked.

| Command | Does |
|---|---|
| `protocol evidence scan <paths>… [--at 2026-09-01] [--warn-days n] [--strict] [--fail-on-expired]` | reads human-written markdown for dated claims and reports coverage beside the classification; a directory is read one level deep for `*.md` |
| `protocol evidence inspect <files>… [--at …] [--horizon 7d]` | reads the evidence document `protocol evaluate --evidence` submits and reports, per record, when somebody last looked |

`scan` classifies each record `ok`, `expiring`, `expired` or `malformed`, and closes with a coverage
line — occurrences found, records parsed, and how many it could not read:

```text
43 occurrence(s), 43 record(s), 0 unparsed — 27 ok, 6 expiring, 10 expired, 8 malformed (at 2026-09-01)
```

That line is the point. A scanner over human-written documents needs a coverage claim of its own,
because an annotation that is present, correct, legible to a human and invisible to the gate is the
one failure a clean report cannot show.

The two exit flags on `scan` answer different questions and are separate for that reason. `--strict`
fails when the parser found fewer records than there are annotation-shaped occurrences — *is the
gate blind?* `--fail-on-expired` fails when a record is past its horizon — *is the claim stale?* An
expired record is a normal finding; a corpus with none is a corpus nobody has kept.

`inspect`'s `--horizon` is report-only: a what-if applied to a printed table. It reaches no
requirement and no evaluation, and nothing it prints can extend the life of a record. The horizon
that decides a gate is declared on a requirement, in a reviewed document. `inspect` exits `1` on a
record whose observation time is in the future — the same refusal the engine applies, available
before anything is submitted.

## Entity surface

These seed an **in-memory** backend from `--artifacts` (an artifact manifest) or `--planning` (a
markdown planning store) and then answer; one of the two is required. Nothing is durable, and what
`history` shows is this run's seeding — every entity is at revision 1.

| Command | Answers |
|---|---|
| `protocol entity list <--artifacts m.yaml\|--planning dir> [--type aep.design/v1]` | every entity the source seeds, with type, locator, revision |
| `protocol entity get <source> <locator-or-id>` | one entity; exit 1 when nothing matches |
| `protocol entity history <source> <ref>` | revision records, oldest first |
| `protocol entity relations <source> <ref> [--incoming]` | what an entity points at, or what points at it |
| `protocol audit <source> [--correlation …] [--entity …] [--rejected]` | the audit trail, oldest first; `--rejected` shows only refused attempts |
| `protocol describe <source> <entity-type>` | what a type *is*: mutable or not, which commands may target it, which relations it may have |

`--organisation` (default `local`) and `--space` (default `manifest`) set the namespace the seeded
locators live under.

## ESS surface

All take `--path <file-or-dir>` (default `.`) unless noted.

| Command | Does |
|---|---|
| `protocol ess validate` | parses and checks a specification, naming every problem in one run |
| `protocol ess compile` | resolves every reference into the normalized IR |
| `protocol ess inspect <name> [--kind domain\|type\|command\|event\|error\|binding\|component]` | one declaration, resolved |
| `protocol ess graph [--format dot\|mermaid\|json\|yaml]` | the actor/command/event graph |
| `protocol ess generate --kind docs\|schema\|openapi\|asyncapi [--out dir]` | the projections; without `--out`, a listing only |
| `protocol ess synthesize [--target rust\|go\|web] [--out dir]` | the synthesis plan and one emitted tree |
| `protocol ess conform synthesize [--out dir]` | the conformance suite the specification obliges; `--format json` carries the suite document itself |
| `protocol ess conform run --target <name> [--suite suite.json] [--inject fault] [--untraced]` | runs the suite against a compiled-in reference implementation |
| `protocol ess conform evidence --target <name> [--observed-at date] [--out f]` | runs the suite and mints the AEP evidence record in the same process |
| `protocol ess diff --from <path> --to <path> [--format text\|json]` | the semantic delta between two revisions of one specification |
| `protocol ess impact --from <path> --to <path> [--suite suite.json] [--generated dir]` | what the delta invalidates: scenarios owed again, artifacts owed regeneration, each with its dependency path |

`--target` names a reference implementation this binary was compiled with — `billing` or
`oracle-fixture`. It cannot reach yours: a conformance target is a Rust trait, and nothing here
speaks to an implementation over a socket. To hold your own system to a specification, depend on
`ess-conformance`, implement the trait, and run the committed `suites/generated/<system>/suite.json`
against it — the same document this verb writes.

`ess conform run` exit codes differ from the general convention, because "wrong" and "unverified"
are different findings:

| Exit | Meaning |
|---|---|
| `0` | every scenario passed |
| `1` | the implementation contradicted the specification, or a scenario the specification requires is one the target cannot expose |
| `3` | nothing contradicted it, and at least one scenario could not be executed |

`ess conform evidence` exits `0` whenever a record was produced, **including for a failing run** —
the verdict is in the record, and the engine is what decides on it. Its `--observed-at` exists so a
committed record can be regenerated byte for byte; it defaults to now, which is the truth.

## Infrastructure surface

Inputs are files written by an external scanner; no verb reaches a cluster.

| Command | Does |
|---|---|
| `protocol infra validate --path <bundle>` | checks an `infra-observation/1` bundle |
| `protocol infra compile --path <bundle> [--out f]` | compiles it to the content-addressed `infra-ir/1` document |
| `protocol infra inspect --path <ir> [--properties]` | per-object and per-workload facts |
| `protocol infra graph --path <ir> [--namespace n] [--format mermaid\|json\|html]` | the typed dependency graph, with the evidence on every edge |
| `protocol infra diagnose --path <ir> [--min-severity info\|warning\|error] [--candidates] [--directions]` | twenty coded findings (`INFRA-DIAG-001`…`020`), invariant candidates, ranked directions — a report, never a gate |
| `protocol infra view --path <ir> [--namespace n] [--out f]` | writes the self-contained HTML component page and opens it in a browser; the one verb here that spawns another program |
| `protocol infra simulate --spec expected.yaml --path <bundle\|ir>` | evaluates a desired state against a snapshot: `ok` / `gap` / `unk` per expectation |
| `protocol infra diff --from <ir> --to <ir>` | what moved between two scans of one cluster, over declared state |
| `protocol infra project --spec expected.yaml --path <bundle\|ir> --out <dir>` | writes the patch tree that would close the gaps, plus `OBLIGATIONS.md` and `SUMMARY.md`; applies nothing |

`diagnose`, `simulate`, `project` and `diff` exit `0` whatever they found, and take
`--format text|json` only. A cluster with sixteen decisions owed has been successfully diagnosed,
simulated and projected; drift is a report too. Exit `1` here means an input that could not be read
— or, for `diff`, the one refusal: two snapshots of different clusters. `view` takes no `--format`
at all: it has one output, and its purpose is to open it.

## Trace surface

Inputs are transcripts a harness already wrote; no verb runs an agent, calls a model or reaches a
network. All three take `--format text|json`, except `trace evidence`, which writes the record and
so takes the shared `text|yaml|json` with `yaml` the default.

| Command | Does |
|---|---|
| `protocol trace inspect --transcript <file>` | the transcript's census from the typed event IR: event families, per-tool traffic in both directions, per-step `gen`/`exec` timing |
| `protocol trace check --spec <file> --transcript <file> [--redact] [--advisory <id>]` | judges the run against a `trace-spec/1` document: `ok` / `gap` / `unk` per expectation, every verdict citing event indices — exit 0 conformant, 1 contradicted, 3 unknown |
| `protocol trace evidence --spec <file> --transcript <file> [--advisory <id>] [--observed-at date] [--out <file>]` | mints the verdict as a `trace_conformance` evidence record (producer `trace-checker`, digest pair binding it to one transcript and one spec) that `protocol evaluate --evidence` accepts |

`--redact` cites event indices and digests only — no command strings, no file paths, no text. It is
opt-in, and the un-redacted rendering carries a footer naming what it contains, so pasting a report
somewhere public is a decision rather than an accident.

`--advisory <id>` downgrades one named expectation for this run: still evaluated, still printed,
gating nothing, and every downgraded id named in the report. An id the specification does not
declare is a usage error, not a silent no-op. In an evidence record, `trace_conformance.passed`
ignores the downgrade, because a flag the caller passed must not satisfy a requirement the protocol
asked for.

## Repository automation (`cargo xtask`)

For contributors to the repository itself; each `--check` variant fails on any byte of drift.

| Command | Regenerates |
|---|---|
| `cargo xtask schema [--check]` | `schemas/generated/` from the Rust types |
| `cargo xtask generate [--check]` | `generated/` — the projections of the example specifications |
| `cargo xtask suite [--check]` | `suites/generated/` — the conformance suites |
| `cargo xtask synth [--check]` | `generated/rust\|go\|web/` — the synthesized trees, then builds them and runs the dual-target demonstration |
| `cargo xtask infra [--check]` | the example cluster's committed IR, simulation, drift and projection |
| `cargo xtask status [--check]` | `docs/status.md` — the delivered-waves record, from the repository's tags |
| `cargo xtask fmt [--check]` | formatting, scoped to workspace members |
