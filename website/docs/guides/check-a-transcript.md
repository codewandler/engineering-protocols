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

`protocol trace` has three verbs. `inspect` reports what is in a transcript, `check` judges it
against a specification, and `evidence` mints the verdict as a record the engine reads. None of
them starts an agent, calls a model or reaches a network — they read a file and evaluate typed
predicates over it, which is what makes a verdict reproducible on any machine on any day.

The commands below run against a real committed transcript — a Claude Code `stream-json` session
from the planning-plugin eval — so every output shown here is reproducible from a checkout.

## See what the transcript contains

```bash
B=target/debug/protocol
$B trace inspect --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl
```

```text
transcript   sha256:6522e1ebe318da1e0a604e595ecc9afed1d1041c6e418a1382e4f1600a17640b
events       36 total — 6 assistant_text, 2 assistant_thinking, 1 rate_limit, 1 run_outcome, 1 session_start, 1 synthetic_injection, 2 thinking_estimate, 11 tool_call, 11 tool_result
unread       0 event(s) the adapter could not read
requests     19 assistant events, 8 api requests
tool         Bash: 4 call(s), 0 error(s), in 1116B, results 2739B
tool         Edit: 3 call(s), 0 error(s), in 6863B, results 637B
tool         Read: 3 call(s), 0 error(s), in 361B, results 4656B
tool         Skill: 1 call(s), 0 error(s), in 423B, results 47B
tools-total  11 call(s), results 8079B into context
repeated     0 identical call group(s)
step         1. Skill (event 5): gen 1486ms, exec 35ms
step         2. Bash (event 10): gen 1290ms, exec 187ms
step         3. Bash (event 13): gen 1088ms, exec 21ms
step         4. Bash (event 15): gen 3205ms, exec 38ms
step         5. Read (event 18): gen 555ms, exec 36ms
step         6. Read (event 20): gen 560ms, exec 6ms
step         7. Read (event 22): gen 305ms, exec 16ms
step         8. Edit (event 25): gen 8742ms, exec 26ms
step         9. Edit (event 27): gen 5968ms, exec 28ms
step         10. Edit (event 29): gen 4482ms, exec 9ms
step         11. Bash (event 32): gen 80ms, exec 13ms
time-split   inference 27761ms, tool-exec 415ms across 11 step(s)
```

The census is computed from the same IR the checker judges: per-tool traffic in both directions
(tool *inputs* spend output tokens; tool *results* land in the next request's input), and per
step a `gen`/`exec` split — the inference interval that produced each call, and the call's own
execution time — derived from the transcript's recorded timestamps, never measured.

`inspect` states quantities and has no opinion about any of them, which is why it exits 0 whatever
the census says. An opinion about a quantity belongs in a specification.

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

The report is one row per expectation and this specification declares forty-two, so the command
below takes the first twelve lines rather than abridging by hand:

```bash
$B trace check --spec conformance/trace/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl | head -12
```

```text
planning-plugin/eval against transcript sha256:6522e1ebe318… — 41 ok, 0 gap, 0 unk
  The planning plugin behaves as its skill says it does
  ok        our-plugin-loaded                              engineering-protocols 0.1.0 from engineering-protocols@inline is loaded at event 0
  ok        nothing-else-loaded                            exactly engineering-protocols loaded at event 0
  ok        billed-to-the-session                          api_key_source = none at event 0
  ok        the-run-did-not-ask                            permission_mode = dontAsk at event 0
  ok        the-operators-output-style-did-not-leak        output_style = default at event 0
  ok        the-skill-was-offered                          skill engineering-protocols:planning is among 17 offered at event 0
  ok        the-decomposer-loaded                          agent engineering-protocols:decomposer is among 7 offered at event 0
  ok        skill-completed                                engineering-protocols:planning completed 1 time(s) with success=true, at least 1 at events 5, 6
  ok        consulted-the-skill-before-touching-the-store  first Skill at 5, first Bash(command ~ "protocol artifact") at 10 at events 5, 10
  ok        created-through-the-cli                        Bash(command ~ "protocol artifact new") called 2 time(s), at least 1 at events 13, 15
```

The rows in between are the resource-shaped ones, each marked `ok (adv)`: an advisory expectation
is evaluated and printed and gates nothing, so a cost bound that drifted with model routing cannot
turn a job red on its own. The last four lines close the report:

```bash
$B trace check --spec conformance/trace/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl | tail -4
```

```text
  ok (adv)  served-at-standard-speed                       usage.speed = standard at event 35
spec sha256:8eca7c40a57e…  adapter claude-code/stream-json
note: this report quotes command strings and file paths read out of the transcript; `--redact` replaces them with digests. Transcript sha256:6522e1ebe318da1e0a604e595ecc9afed1d1041c6e418a1382e4f1600a17640b
conformant: the run satisfies every expectation the specification states (exit 0)
```

Every verdict cites the transcript event indices behind it. The exit codes carry the same
contract as `ess conform`: **0** conformant, **1** contradicted, **3** nobody found out — an
event the adapter could not read, or a field this transcript does not carry. Unknown is not
false: *"the format moved under us"* wakes a different person than *"the agent did the wrong
thing"*, and collapsing the two is how checks rot.

## Two flags to know before the report leaves your machine

A transcript holds the prompt, the model's reasoning, the file contents it read and the commands
it ran. A check report quotes all of that, and a report is a thing people paste into pull
requests. **`--redact` replaces every citation with an event index and a digest:**

```bash
$B trace check --spec conformance/trace/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl --redact | head -6
```

```text
planning-plugin/eval against transcript sha256:6522e1ebe318… — 41 ok, 0 gap, 0 unk
  The planning plugin behaves as its skill says it does
  ok        our-plugin-loaded                              sha256:254fa2a0a580 at event 0
  ok        nothing-else-loaded                            sha256:90325e36d98d at event 0
  ok        billed-to-the-session                          sha256:bf73c5c38808 at event 0
  ok        the-run-did-not-ask                            sha256:a8ea2249bf47 at event 0
```

It is opt-in rather than the default, and the un-redacted rendering carries the footer above
naming what it contains — so pasting one somewhere public is a decision rather than an accident.

**`--advisory <EXPECTATION_ID>` downgrades one named expectation for one run.** The row is still
evaluated, still printed, and the report names every id that was downgraded:

```bash
$B trace check --spec conformance/trace/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl \
    --advisory billed-to-the-session | grep -E 'billed|downgraded'
```

```text
  ok (adv)  billed-to-the-session                          api_key_source = none at event 0
note: downgraded to advisory on the command line: billed-to-the-session — the specification's digest is the document as authored
```

It is not a way to skip a check. An id the specification does not declare is a usage error, not a
silent no-op — `--advisory not-a-real-id` exits 1 with *"a downgrade that matched nothing would
relax nothing while looking as though it had"* — and the downgrade deliberately does not move
`trace_conformance.passed` in the record below, because a flag the caller passed must not satisfy
a requirement the protocol asked for.

## Mint the verdict as evidence

```bash
$B trace evidence --spec conformance/trace/expectations.trace.yaml \
    --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl \
    --observed-at 2026-08-21 --out evidence.yaml
```

```yaml
- kind: trace_conformance
  specification: planning-plugin/eval
  spec_digest: 8eca7c40a57e3f45d311c9499102980cb8846c3045ca407e6a3148abc3b8f74f
  transcript_digest: 6522e1ebe318da1e0a604e595ecc9afed1d1041c6e418a1382e4f1600a17640b
  status: passed
  expectations_total: 41
  expectations_gapped: 0
  expectations_unknown: 0
  adapter: claude-code/stream-json (written against 2.1.238)
  observed_at: 1787270400000
  producer:
    producer: verifier
    verifier: trace-checker
  provenance:
    command: protocol trace evidence --spec conformance/trace/expectations.trace.yaml --transcript crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl
    inputs:
    - conformance/trace/expectations.trace.yaml
    - crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl
```

`observed_at` is required, and it defaults to now — the truth, since the transcript is checked by
this process in this second. `--observed-at` takes a date or epoch milliseconds and exists for the
one case that needs it: a record committed to a repository has to regenerate byte for byte, and a
record whose only moving field is a clock reading fails every drift check. Pin it and the output
above is reproducible. Pin it into the future and the engine refuses the submission rather than
accepting a claim about a check that has not happened.

The record is a summary, not the report: counts, ids and the digest pair cross the boundary; the
cited transcript rows — prompts, file contents — do not. Its producer is the `trace-checker`
verifier class, and the record is minted in the same process that ran the check, so an agent's own
claim of conformance never satisfies the kind. `protocol evaluate --evidence evidence.yaml` reads
the emitted document directly: `trace_conformance` is one of the evidence kinds the development
protocol declares, and `trace_conformance.**` one of its observable fact families
(`protocols/adp/1.yaml`). A behavioural claim about *how an agent worked* is now a fact the
protocol can require, with the same standing as a test result or a conformance run.

`trace evidence` exits 0 even for a run it judged badly. Its exit code answers *"was a record
produced?"*; the verdict is in the record, and the engine is what decides on it.

## The whole loop, in one picture

![Diagram of the mechanism: transcript events on the left; the checker ticking expectations off
against them, each verdict citing event indices; the passing check minting a trace_conformance
evidence record with its digest pair; and a workflow transition that stays Blocked until exactly
that record is submitted, then moves.](/img/trace-evidence-gate.svg)

The drawing is of the mechanism, not a screenshot of any one run — and its last panel is still
labelled *soon*, because it was drawn before the driver shipped. **The driver ships today.**
`protocol drive run`, `drive status` and `drive resume` walk a workflow: they make the engine's
calls in order, execute the three kinds of step that touch the world — a program, a model, a
person — and record what they did. The driver evaluates no gate itself. A driver that could
evaluate a gate would be a second protocol implementation with none of the conformance suites,
and the first time the two disagreed the one nobody tested would win.

Running one needs a harness and a model, so there is no reproducible command for it on this page.
There is a record instead, and it is not a success story. The first governed run of this
repository's own backlog — `W4-1/1`, 2026-08-21 — **blocked**, in `establish_verifiers`, four
states short of the person it was meant to stop at, on two requirements the engine printed: a
specification artifact still in `draft`, and `test.first_result == failed` reading `passed`. Four
model sessions, 80 hook decisions of which 11 were denials, and 11 `permission_denials` entries in
the transcripts — one for one, each naming its tool. `protocol trace check` decided those four
transcripts, and `protocol trace evidence` minted a `trace_conformance` record from one of them.
What the run found was about the step map, not about the enforcement. The full record, including
what it cost and what broke, is `docs/plan/harness-wave-4-governed-dogfood.md` § *The first run*.

## Sources

The checker and IR live in `crates/trace-domain` and `crates/trace-spec`; the specification
format is published as `schemas/generated/trace-spec.schema.json`; the worked specification is
`conformance/trace/expectations.trace.yaml`, whose forty-two expectations are checked
against two committed transcripts by the ordinary test suite. Design and acceptance:
`docs/design/transcript-conformance-design-v0.1.md`, `docs/plan/trace-wave-1-transcript-checker.md`.
The driver is `crates/aep-driver` behind `protocol drive`, its enforcement arm is the plugin's
hooks (`integrations/claude-code/README.md` § *The hooks, and what changed about "no hooks"*), and
the governed-run record is `docs/plan/harness-wave-4-governed-dogfood.md`. For building a driver of
your own against the same engine calls, see [Integrate an agent
harness](./integrate-a-harness.md).
