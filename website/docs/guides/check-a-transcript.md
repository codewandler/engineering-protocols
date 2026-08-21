---
title: Check what an agent run did
sidebar_position: 9
description: Normalize a harness transcript into a typed event IR, judge it against a trace specification, and mint the verdict as evidence the protocol admits.
---

# Check what an agent run did

An agent harness records everything the agent did — every tool call, every result, the loaded
plugins, the tokens, the timings. Almost nobody checks that record with anything stronger than a
`grep`. This guide runs the checker that does: a transcript is normalized into a typed event IR,
a **trace specification** states expectations over it, and the verdict is three-valued and
evidence-cited.

The commands below run against a real committed transcript — a Claude Code `stream-json` session
from the planning-plugin eval — so every output shown here is reproducible from a checkout.

## See what the transcript contains

```bash
B=target/debug/protocol
$B trace inspect --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl
```

```text
transcript   sha256:6522e1ebe318da1e0a604e595ecc9afed1d1041c6e418a1382e4f1600a17640b
events       36 total — 6 assistant_text, 2 assistant_thinking, 1 rate_limit, 1 run_outcome,
             1 session_start, 1 synthetic_injection, 2 thinking_estimate, 11 tool_call, 11 tool_result
unread       0 event(s) the adapter could not read
tool         Bash: 4 call(s), 0 error(s), in 1116B, results 2739B
tool         Edit: 3 call(s), 0 error(s), in 6863B, results 637B
step         1. Skill (event 5): gen 1486ms, exec 35ms
step         2. Bash (event 10): gen 1290ms, exec 187ms
```

The census is computed from the same IR the checker judges: per-tool traffic in both directions
(tool *inputs* spend output tokens; tool *results* land in the next request's input), and per
step a `gen`/`exec` split — the inference interval that produced each call, and the call's own
execution time — derived from the transcript's recorded timestamps, never measured.

## Judge it against a specification

A `trace-spec/1` document states expectations by kind — behavioural (a skill completed, a tool
was called with matching arguments, one thing happened before another), environmental (exactly
these plugins loaded, auth came from the login), and resource-shaped (turns, tokens, cost,
cache use, per-step timing), each `gate` or `advisory`:

```yaml
- id: consulted-the-skill-before-touching-the-store
  statement: the skill was loaded before the CLI was reached for, not afterwards
  expect:
    order:
      first: {tool: Skill}
      before: {tool: Bash, args: {command: {contains: "protocol artifact"}}}

- id: created-through-the-cli
  statement: artifacts were created with the CLI, not with hand-written frontmatter
  expect:
    tool.called:
      tool: Bash
      args: {command: {contains: "protocol artifact new"}}
```

```bash
$B trace check --spec integrations/claude-code/eval/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl
```

```text
planning-plugin/eval against transcript sha256:6522e1ebe318… — 41 ok, 0 gap, 0 unk
  ok    skill-completed             engineering-protocols:planning completed 1 time(s) with success=true, at least 1 at events 5, 6
  ok    consulted-the-skill-before-touching-the-store  first Skill at 5, first Bash(command ~ "protocol artifact") at 10 at events 5, 10
  ok    created-through-the-cli     Bash(command ~ "protocol artifact new") called 2 time(s), at least 1 at events 13, 15
conformant: the run satisfies every expectation the specification states (exit 0)
```

Every verdict cites the transcript event indices behind it. The exit codes carry the same
contract as `ess conform`: **0** conformant, **1** contradicted, **3** nobody found out — an
event the adapter could not read, or a field this transcript does not carry. Unknown is not
false: *"the format moved under us"* wakes a different person than *"the agent did the wrong
thing"*, and collapsing the two is how checks rot.

## Mint the verdict as evidence

```bash
$B trace evidence --spec integrations/claude-code/eval/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl --out evidence.yaml
```

```yaml
- kind: trace_conformance
  specification: planning-plugin/eval
  spec_digest: 8eca7c40a57e3f45d311c9499102980cb8846c3045ca407e6a3148abc3b8f74f
  transcript_digest: 6522e1ebe318da1e0a604e595ecc9afed1d1041c6e418a1382e4f1600a17640b
  status: passed
  expectations_total: 41
  producer:
    producer: verifier
    verifier: trace-checker
```

The record is a summary, not the report: counts, ids and the digest pair cross the boundary; the
cited transcript rows — prompts, file contents — do not. Its producer is the `trace-checker`
verifier class, so an agent's own claim of conformance never satisfies the kind, and the emitted
document feeds straight back into `protocol evaluate --evidence`: a behavioural claim about *how
an agent worked* is now a fact the protocol can require, with the same standing as a test result
or a conformance run.

That closing move is what the rest of the system was waiting for. A protocol can already refuse
to call work done until independent conformance evidence exists; it can now also refuse until
the *run that produced the work* checked out — the skill was consulted before the store was
touched, nothing shelled out to `rm -rf`, the environment was the one the eval promised.

## The whole loop, in one picture

![Animated diagram: transcript events stream in on the left; the checker ticks expectations off
against them, each verdict citing event indices; the passing check mints a trace_conformance
evidence record with its digest pair; and in the reference driver — decided, not yet built — a
workflow transition stays Blocked until exactly that record is submitted, then
moves.](/img/trace-evidence-gate.svg)

The last panel is deliberately drawn dashed: the gate it shows is the [reference
driver](../status/roadmap.md), decided and designed but not built. The mechanism it will use
exists today — the evidence record above is already accepted by `protocol evaluate --evidence`,
and a state's evidence requirement is already how the engine answers `Blocked { reasons }`. What
the driver adds is only the loop that asks.

## Sources

The checker and IR live in `crates/trace-domain` and `crates/trace-spec`; the specification
format is published as `schemas/generated/trace-spec.schema.json`; the worked specification is
`integrations/claude-code/eval/expectations.trace.yaml`, whose forty-one expectations are checked
against two committed transcripts by the ordinary test suite. Design and acceptance:
`docs/design/transcript-conformance-design-v0.1.md`, `docs/plan/trace-wave-1-transcript-checker.md`.
