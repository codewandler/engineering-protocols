---
title: CLI reference
sidebar_position: 1
description: Every subcommand of the reference CLI, grouped by surface — protocol, entity, ESS and infrastructure — with exit codes.
---

# CLI reference

The reference CLI is `protocol`, built with `cargo build -p protocol-cli`. Every command takes
`--format text|yaml|json` (text is the default); refusals, decisions and evaluations all serialise.
`--help` on any subcommand carries the full flag list — this page is the map.

General conventions: exit `0` is success, exit `1` is a refusal or invalid input, and errors
accumulate — a run reports every problem it found, not the first.

## Protocol surface

| Command | Does |
|---|---|
| `protocol validate [--root .] [--artifacts m.yaml]` | checks a document tree structurally and semantically — including that every rule could actually fire |
| `protocol resolve --task task.yaml [--root .]` | resolves a task into an execution plan: workflow, principles in force, capabilities, obligations |
| `protocol inspect [reference]` | shows what a protocol, principle, workflow or profile declares — `aep/1`, `test-driven`, `development.standard` |
| `protocol evaluate --task … [--artifacts …] [--evidence e.yaml]… [--advance]` | evaluates an execution: what is owed, what is permitted, what is missing; `--advance` also attempts transitions |
| `protocol explain --task … --action production.write` | explains one decision — allowed or denied, by which rule, and what would unlock it |
| `protocol schema [name]` | prints the generated JSON Schemas, or one by file stem |
| `protocol conformance [--level core\|audited\|full] [--suite name] [--inject fault]` | checks a storage backend against the AEP contract suites (16 suites, 3 levels); `--inject` breaks one property on purpose to show the responsible suite fails |

## Entity surface

These seed an **in-memory** backend from `--artifacts` and then answer; nothing is durable, and what
`history` shows is this run's seeding.

| Command | Answers |
|---|---|
| `protocol entity list [--type aep.design/v1]` | every entity the manifest seeds, with type, locator, revision |
| `protocol entity get <locator-or-id>` | one entity; exit 1 when nothing matches |
| `protocol entity history <ref>` | revision records, oldest first |
| `protocol entity relations <ref> [--incoming]` | what an entity points at, or what points at it |
| `protocol audit [--correlation …] [--entity …] [--rejected]` | the audit trail, oldest first; `--rejected` shows only refused attempts |
| `protocol describe <entity-type>` | what a type *is*: mutable or not, which commands may target it, which relations it may have |

## ESS surface

All take `--path <file-or-dir>` (default `.`) unless noted.

| Command | Does |
|---|---|
| `protocol ess validate` | parses and checks a specification, naming every problem in one run |
| `protocol ess compile` | resolves every reference into the normalized IR |
| `protocol ess inspect <name>` | one declaration, resolved |
| `protocol ess graph [--format dot\|mermaid\|json\|yaml]` | the actor/command/event graph |
| `protocol ess generate --kind docs\|schema\|openapi\|asyncapi [--out dir]` | the projections; without `--out`, a listing only |
| `protocol ess synthesize [--target rust\|go\|web] [--out dir]` | the synthesis plan and one emitted tree |
| `protocol ess conform synthesize` | the conformance suite the specification obliges |
| `protocol ess conform run --target <name> [--inject fault] [--untraced]` | runs the suite against a compiled-in implementation |
| `protocol ess conform evidence --target <name>` | runs the suite and mints the AEP evidence record in the same process |
| `protocol ess diff --from <path> --to <path> [--format text\|json]` | the semantic delta between two revisions of one specification |
| `protocol ess impact --from <path> --to <path> [--suite suite.json] [--generated dir]` | what the delta invalidates: scenarios owed again, artifacts owed regeneration, each with its dependency path |

`ess conform run` exit codes differ from the general convention, because "wrong" and "unverified"
are different findings:

| Exit | Meaning |
|---|---|
| `0` | every scenario passed |
| `1` | the implementation contradicted the specification |
| `3` | nothing contradicted it, and at least one scenario could not be executed |

## Infrastructure surface

Inputs are files written by an external scanner; no verb reaches a cluster.

| Command | Does |
|---|---|
| `protocol infra validate --path <bundle>` | checks an `infra-observation/1` bundle |
| `protocol infra compile --path <bundle>` | compiles it to the content-addressed `infra-ir/1` document |
| `protocol infra inspect --path <ir> [--properties]` | per-object and per-workload facts |
| `protocol infra graph --path <ir> [--format json\|mermaid\|html]` | the typed dependency graph, with the evidence on every edge |
| `protocol infra diagnose --path <ir> [--candidates] [--directions]` | twenty coded findings, invariant candidates, ranked directions — a report, never a gate (exit 0) |
| `protocol infra view --path <ir>` | a self-contained HTML component page |
| `protocol infra simulate --spec expected.yaml --path <bundle\|ir>` | evaluates a desired state against a snapshot: `ok` / `gap` / `unk` per expectation (exit 0) |
| `protocol infra diff --from <ir> --to <ir>` | what moved between two scans of one cluster, over declared state |
| `protocol infra project --spec expected.yaml --path <bundle\|ir> --out <dir>` | writes the patch tree that would close the gaps, plus `OBLIGATIONS.md` and `SUMMARY.md`; applies nothing |

## Trace surface

Inputs are transcripts a harness already wrote; no verb runs an agent.

| Command | Does |
|---|---|
| `protocol trace inspect --transcript <file>` | the transcript's census from the typed event IR: event families, per-tool traffic in both directions, per-step `gen`/`exec` timing |
| `protocol trace check --spec <file> --transcript <file> [--redact] [--advisory <id>]` | judges the run against a `trace-spec/1` document: `ok` / `gap` / `unk` per expectation, every verdict citing event indices — exit 0 conformant, 1 contradicted, 3 unknown |
| `protocol trace evidence --spec <file> --transcript <file> [--out <file>]` | mints the verdict as a `trace_conformance` evidence record (producer `trace-checker`, digest pair binding it to one transcript and one spec) that `protocol evaluate --evidence` accepts |

## Repository automation (`cargo xtask`)

For contributors to the repository itself; each `--check` variant fails on any byte of drift.

| Command | Regenerates |
|---|---|
| `cargo xtask schema [--check]` | `schemas/generated/` from the Rust types |
| `cargo xtask generate [--check]` | `generated/` — the projections of the example specifications |
| `cargo xtask suite [--check]` | `suites/generated/` — the conformance suites |
| `cargo xtask synth [--check]` | `generated/rust\|go\|web/` — the synthesized trees, then builds them and runs the dual-target demonstration |
| `cargo xtask infra [--check]` | the example cluster's committed IR |
| `cargo xtask fmt [--check]` | formatting, scoped to workspace members |
