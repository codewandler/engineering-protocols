# Trace wave 1 — the transcript-conformance checker

> **Accepted for implementation, 2026-08-21. Design:
> [`transcript-conformance-design-v0.1.md`](../design/transcript-conformance-design-v0.1.md);
> decided by the operator in session.** The design's § 9 milestones T1, T2 and T3 are taken up
> whole by this page, which sequences them and sets their acceptance criteria — the design does
> not. Its six open decisions D1–D6 are taken at their stated defaults, with one narrowing recorded
> below (D2's `regex` matcher is refused by name rather than implemented).

**Goal: the five transcript assertions `integrations/claude-code/eval/run.sh` grew in three
idioms become one typed document and one call to a checker — and the sixty-five-line metrics block
beside them gets somewhere to have an opinion.**

This is the first wave of a third observation domain. It is not ESS and not infra: it is
`infra-spec/1` pointed at an **agent run**, and it reuses that family's shape deliberately rather
than inventing a parallel one.

## What this wave is, in one sentence each way

For the person running an eval: the thing that decides whether your agent behaved is no longer a
`grep` for a string anywhere in 86KB of JSON, and it no longer has two different definitions of one
assertion selected by whether `jq` happens to be installed. It is a YAML file you can read, review
and diff, and a checker that tells you which events it read to reach each verdict.

For the machinery: an agent's *behaviour* becomes a checkable claim with a content-addressed
subject, which is the missing piece the Phase-2 driver's `llm` step needs — an LLM step cannot
carry an evidence block, and a subsequent deterministic `command` step reading the transcript can.

## Decisions, taken

| decision | taken as | why |
|---|---|---|
| where it lives | a crate **pair**, `trace-domain` and `trace-spec`, beside the infra family (design **D4**) | two crates because an adapter changes when a *harness* moves, a checker changes when the *evaluation* moves, and a model changes when the *vocabulary* moves. Three reasons |
| where the line between them runs | **models** in `trace-domain` (the event IR *and* the expectation document), **mechanisms** in `trace-spec` (the adapter *and* the checker) | a deviation from D4, which put the adapters in `trace-domain`. The models are the pair a schema is published from and a report is written against, and they change together; `aep-schema` then depends on exactly one trace crate instead of reaching into a crate that also holds a harness-specific reader |
| the wire form of a kind | externally tagged under `expect:`, keeping the design's dotted names verbatim — `expect: {tool.called: {…}}` | the design's own YAML sketch writes the kind flat beside `id:`. That cannot carry `deny_unknown_fields`, because serde's does not survive `flatten` — and a specification where `at_leats: 1` silently becomes "unbounded" is worse than one that is a line longer. The dotted names survive as serde renames, so the vocabulary is the design's word for word. `infra-spec/1` made the same trade for the same reason |
| `regex` matchers | **refused by name**, `TRACE-SPEC-008`, with the message naming `glob` | the workspace carries no regular-expression engine and `AGENTS.md` § *Dependencies* says to prefer no dependency and record the refusal. Reading `regex:` as `contains:` would be a specification that means something other than what it says; refusing an unknown *field* would tell the author the wrong thing. What `glob` buys is the design's own example (`*/.engineering/planning/*.md` is a glob wearing a regular expression's syntax); what it does not buy is alternation, capture and quantifiers, which is a real loss and is named here rather than discovered later |
| the third value | any event the adapter could not read poisons a count — **but only where an unread event could change the answer** | the design says an opaque event makes a tool expectation `unk`. Applied bluntly that is harsher than the truth: an unread event can only *add* calls, so `at_least: 1` with two calls already seen holds whatever it was, and `at_most: 2` with three seen fails whatever it was. The checker does the three-valued reasoning properly rather than reporting `unk` out of timidity |
| severity | a two-valued `severity: gate \| advisory` on every expectation, defaulting to `gate` | design § 3.6 documents the environment-dependent kinds "with `enabled: false` in every example", and D6 asks for generous bounds that do not fail CI. Both want the same thing and neither should be spelled as *off*: **a check that is switched off reads exactly like a check that passed**. An advisory expectation is evaluated, printed and in the report; it does not move the exit code |
| the `EVAL_USE_API_KEY` escape | a `--advisory <id>` flag on the check call, not a second spec file and not a skip | the eval opts into billing an exported key, which makes `env.api_key_source: none` false *by intention*. The row stays in the report and stays printed, the report names every id that was downgraded, and the specification's digest stays the digest of the document **as authored**. An id the document does not declare is a usage error, so a typo fails loudly instead of relaxing nothing |
| `on_unknown` | a per-expectation policy, `unknown` (default) or `gap` | the default is what exit code 3 is built around. `gap` exists for the expectation whose whole point is that the transcript must carry the field — *"this run must record its own cost"* — where silence is the defect rather than an obstacle to finding one |
| what a digest covers | the **transcript's raw bytes**, and separately the **validated specification's canonical JSON** | design § 2.9: an adapter upgrade that starts understanding a field must not silently rename the run. A comment or a reordered key, on the other side, is not a different specification. Both are `sha256`, 64 hex characters, the construction `ess-gen` and `infra-compiler` already use |
| timestamps | a forty-line reader for the one zulu-terminated form transcripts record; no date crate | what is needed is one fixed format with optional fractional seconds. A date library buys formats no harness writes and brings a transitive tree into a crate whose whole claim is that it reads no clock. Anything not in that exact shape parses to `None`, which becomes `unk` rather than a wrong duration |
| `--format` | `text` and `json`, no `yaml` | the design's § 4 lists three and then says "no third rendering". A report is read by a person as a table or parsed by a program as JSON; a third rendering is a third thing to keep in step. Same reasoning as `GraphFormat` and `DiffFormat` in the same binary |
| the metrics block | **kept as it is**, and superseded by `protocol trace inspect` | `inspect` prints the same census from the IR — event families, tool traffic, per-step `gen`/`exec`, the time split. Deleting the `jq` in the same wave that introduces its replacement would mean the eval's most-read output changed shape in a wave whose subject is the *verdict*. Named as a follow-up rather than left as duplication nobody noticed |

## Shipped kinds, and the ones deliberately not shipped

**Forty-nine of the design's kinds ship**, which is every kind named in §§ 3.1–3.6. Nothing in
those tables was cut. What follows is the honest list of what is *not* here, each with its reason —
cutting is allowed, silent cutting is not.

| deferred | where the design puts it | why not now |
|---|---|---|
| the `regex` matcher | § 3.4 | no regular-expression engine in the workspace; `glob` ships instead and `regex:` is refused **by name** with `TRACE-SPEC-008`. See the decisions table |
| assertions over the per-request *series* — the cache-read ramp is monotone, cache creation is front-loaded, no request above a share of the total | § 2.7, § 7 | deferred **by the design itself**. The data is retained (`TraceIr::requests` keeps every assistant event's usage); what is missing is a vocabulary for sequences that a single-field matcher does not have, and designing one under this wave's deadline would be designing it for a different feature |
| an expectation kind for "the skill's text entered the model's context" | § 2.8 | the design records the synthetic injection as *observable with no expectation kind in v0.1*: a matcher over "a synthetic event containing the skill's text" would be a wording assertion wearing a structural costume. The event is in the IR (`SyntheticInjection`), so adding the kind later costs nothing |
| a streaming checker | **D5** | batch only, deferred by name. Incremental evaluation, partial verdicts and a halt signal are not designable against a format that is not stable (**D1**) |
| the `review` attachment slot on the run record | § 6.5 | the adversarial reviewer stays exactly where it is — outside the verdict, in its own file. The slot is a shape the design sketches and does not settle |
| `--format yaml` | § 4 | see the decisions table |
| `protocol trace evidence` and `EvidenceKind::TraceConformance` | § 5 | **not deferred — split, and now delivered.** It was part of this wave and was implemented by a parallel workstream in two phases; W1.5 below records what shipped and how each acceptance claim is held |

Two kinds ship and are documented as weak, because the design documents them as weak and hiding
that would be worse than the kinds themselves:

* **`text.matches`** is the weakest kind on the list. It exists because *"the refusal was relayed to
  the operator"* has no other observable form today. It is used in **no** expectation of the
  shipped eval specification, deliberately.
* **`speed`** and **`service_tier`** are environment-dependent: a specification that gated on
  either would fail on somebody else's account rather than on the agent's behaviour. Both appear in
  the eval specification as `severity: advisory`.

## W1.1 — `trace-domain`: the two models

`trace-ir/1` — the harness-neutral event IR. Seven recognised event families (session start,
assistant text, assistant reasoning, tool call, tool result, synthetic injection, thinking estimate,
rate limit, run outcome) and one opaque one. Every field a harness might not record is an `Option`
down to the leaves, so absence stays distinguishable from zero all the way to the verdict. Derived,
never measured: `TraceIr::steps` subtracts two recorded timestamps and yields `None` where either is
absent, and `TraceIr::census` is the metrics block as a value.

`trace-spec/1` — the expectation model: forty-nine kinds, count bounds, range bounds (which have no
`exactly`, so **D6 is structural** rather than advice), field matchers, call selectors and result
matchers. `RawTraceSpec` deserializes; `TraceSpec` does not, and `TryFrom` is the only door
(invariant 2). Ten `TRACE-*` refusal codes in their own registry.

**Acceptance:**

* every kind's `unk` arm is on its own variant's documentation, because that is the part a reader
  will otherwise assume away;
* a document with four distinct defects reports **exactly four** refusals in one pass, asserted
  per code and not as "is an error" (invariant 3);
* every one of the forty-nine published names is reachable from a document, asserted by a test that
  builds one for each and compares the covered set against `ExpectationKind::NAMES` — the two lists
  cannot drift;
* an unsupported matcher is refused **by name** with a message naming what to write instead;
* the specification digest survives a command-line downgrade, and downgrading an id the document
  does not declare is reported rather than ignored;
* `task check` is green.

## W1.2 — `trace-spec`: the adapter and the checker

The Claude Code `stream-json` adapter, and the checker.

The adapter tolerates unknown *fields* on a recognised event and refuses to guess at an unknown
*event* — the deliberate opposite of the `deny_unknown_fields` rule the authored documents follow,
and the point of the seam. `TRACE-ADAPT-001` is for a file that is not a transcript; an event this
build does not recognise is an opaque record and an `unk` verdict, never a refusal.

The checker is accumulating, citing and deterministic. `Outcome` has three variants and each
carries what that verdict owes, so a gap without evidence and an `unk` without a reason are both
unrepresentable — the shape `infra-spec` uses, for its reason.

**Acceptance:**

* the adapter reproduces the census of a **committed real transcript** exactly — 36 events, **zero
  opaque**, 19 assistant events, 8 API requests, `Bash` 4 / `Edit` 3 / `Read` 3 / `Skill` 1 — and
  the design's § 2.6 step table to the millisecond: `gen` 1 486 / 1 290 / 1 088 / 3 205 / 555 / 560
  / 305 / 8 742 / 5 968 / 4 482 / 80, `exec` 35 / 187 / 21 / 38 / 36 / 6 / 16 / 26 / 28 / 9 / 13,
  totalling 27 761 ms of inference against 415 ms of tool execution;
* the first four events of that transcript carry **no timestamp**, and the test asserts that before
  it asserts the step table — the fixture reaches the state where the rule is load-bearing;
* **every one of the forty-nine kinds is exercised against that transcript with a negative case
  beside it**, and a coverage guard fails if a kind has only one of the two. This is the design's
  own T2 criterion and the standard `infra-spec` sets for itself: a positive case alone would pass
  for a checker whose every arm returned `ok`;
* a transcript with an event the adapter cannot read produces **exit 3, not exit 0**, and the
  reason names the event's type;
* the same transcript and specification produce a byte-identical report twice over, and two
  transcripts produce different ones.

## W1.3 — `protocol trace check|inspect`, and the published schema

`crates/protocol-cli/src/trace.rs`, the binary's second module split, on the criterion the first
one set.

```console
$ protocol trace check --spec …/expectations.trace.yaml --transcript "$WORK/result.jsonl"
planning-plugin/eval against transcript sha256:6522e1ebe318… — 41 ok, 0 gap, 0 unk
  ok        skill-completed          engineering-protocols:planning completed 1 time(s) with
                                     success=true, at least 1 at events 5, 6
  ok        created-through-the-cli  Bash(command ~ "protocol artifact new") called 2 time(s),
                                     at least 1 at events 13, 15
  ok (adv)  cost-under-a-dollar      cost = $0.2737, at most 1 at event 35
conformant: the run satisfies every expectation the specification states (exit 0)
```

Exit codes mirror `ess conform`, which is the precedent in the same binary: `0` conformant, `1`
contradicted, `3` nobody found out. **Exit 3 is not a softer exit 1** — a CI job may treat it as a
failure and the checker refuses to make that choice on the job's behalf.

`--redact` cites event indices and digests only. It is opt-in and the plain rendering carries a
footer naming what it contains (**D3**), so pasting a report somewhere public is a decision rather
than an accident.

`protocol trace inspect` prints the census: event families, tool traffic in both directions, the
per-step `gen`/`exec` split and the time split. It exits `0` whatever it says — a census is a
report, not a gate, the position `protocol infra simulate` already takes.

`RawTraceSpec` is published as `schemas/generated/trace-spec.schema.json` through
`cargo xtask schema`, the way `planning-document.schema.json` was wired: one generator, one drift
check, one index.

**Acceptance:**

* `protocol trace check` against the two kept transcripts exits 0 with the shipped expectations
  file, and the demo is captured in this wave's record;
* `--redact` leaves the event indices and both digests intact and replaces every note with a
  digest of it, verified by asserting that a path in the un-redacted note is absent from the
  redacted one;
* `--advisory` on an id the specification does not declare is a usage error;
* `schema-check` passes with the new schema committed.

## W1.4 — the eval stops greping

`integrations/claude-code/eval/expectations.trace.yaml`: 41 expectations over 40 kinds. Assertions
3.4–3.8 become `tool.called`, `skill.completed`, `result` + `permission.denied`, `env.exclusive`
and `env.api_key_source`; the metrics block's quantities become bounded advisory expectations with
the observed value in the comment beside each, per **D6**.

`run.sh` keeps its three workspace assertions — a run is checked by **composition**, and folding
workspace inspection into a transcript checker would produce a second, worse artifact validator
(§ 3.7). It loses the `jq`-or-`grep` fork, which is the clearest single symptom of the old
arrangement: two different definitions of one assertion in one file, selected by whether a tool
happens to be installed, and in 3.7's case a check that passed *unconditionally* when it was
absent.

Eighteen expectations gate; twenty-three are advisory and appear in the verdict table as `note`
rows, counted separately and printed in full.

**Acceptance:**

* the specification validates and holds against **both** committed transcripts, checked by
  `cargo test -p trace-spec` — so a bound that stops holding is caught by the gate rather than by a
  paid eval run;
* the two runs differ (`Edit` × 3 against `Write` × 3) and the specification holds for both, which
  is what makes it a specification about behaviour rather than about a tool mix;
* the verdict table still prints one line per expectation — 18 `PASS` rows and 23 `note` rows on
  the committed run — and `bash -n run.sh` passes;
* `EVAL_USE_API_KEY=1` downgrades exactly one expectation and the report says which;
* **the script fails when the checker produced no rows at all.** A missing specification, an
  unreadable transcript or a mistyped `--advisory` id all leave the checker at exit 1 with the
  reason on stderr and an empty table — and a verdict table with no transcript rows in it would go
  green while checking nothing, which is the failure mode `AGENTS.md` § *Gate* names. The row count
  is asserted, not assumed.

## W1.5 — the evidence join (`trace_conformance`) — **delivered**

Part of this wave, implemented by a parallel workstream in two phases, and split because the two
halves touch disjoint files: `EvidenceKind::TraceConformance` is a change to `aep-domain`'s closed
enum, which the checker half does not own.

What the checker half guaranteed it: `trace_spec::report::CheckReport` is a serializable value
carrying the counts, every gapped expectation's id, and — as first-class fields, never derived at
the call site — the **transcript digest** and the **specification digest**. That pair is what makes
the record mean something later: *"some agent passed some behavioural spec"* is worthless, and
*"the run with this digest satisfied the spec with that digest"* is not. The `trace` verb family
was left extensible for it, and `protocol trace evidence` is the third arm on `TraceCommand` it was
left room for.

**Phase 1 — the vocabulary.** `EvidenceKind::TraceConformance` (wire name `trace_conformance`, no
alias) and `Verifier::TraceChecker` (`trace-checker`), the only class that can establish it. Both
declared in `protocols/adp/1.yaml`, because declaration is *required*: the engine refuses a
submission whose kind the protocol does not declare
(`crates/aep-engine/src/engine.rs:320-321`). Recorded as gap-register **D-5**.

**Phase 2 — the record and the verb.** `Evidence::TraceConformance(TraceConformanceResult)`
carrying the verdict, the three counts, every gapped expectation's id, the ids downgraded on the
command line, and the digest pair; `CheckReport::to_evidence` in `crates/trace-spec/src/evidence.rs`
converting on the producing side; and `protocol trace evidence --spec … --transcript … [--out]
[--format] [--advisory]` in `crates/protocol-cli/src/trace.rs`.

The record is a **summary and not the report**: an expectation's citation quotes the transcript —
the most sensitive input this repository consumes — and an evidence record is a thing people paste
into pull requests. Counts, ids and two digests survive the handoff; the rows do not.

**Acceptance, met:**

| claim | how it is held |
|---|---|
| the record is minted in the **same process that ran the check**, so no caller can author its own verdict | `mint_evidence` runs `perform` and hands the report straight to `to_evidence`; there is no `--report` input, and `perform` is shared with `trace check` so the record cannot come from a different evaluation than the one a reader was shown |
| its producer is `Producer::Verifier`, because the checker observed a file and did not ask an agent how it went | `TraceEvidence::PRODUCER` is a constant, not a parameter — `the_record_names_the_trace_checker_and_never_the_caller` |
| the record the checker writes is one the engine reads | `crates/protocol-cli/tests/trace_cli.rs` writes the document with `--out` and feeds the file to `protocol evaluate --evidence`, in both renderings the verb offers |
| a run that gapped is written down rather than exited on | `a_run_that_gapped_is_written_down_rather_than_exited_on`: `trace check` exits 1 on the same pair of files, `trace evidence` exits 0 and the record says `status: failed` and names the expectation |
| a `--advisory` downgrade cannot satisfy a protocol requirement | the record names every downgraded id, and `trace_conformance.passed` counts all gaps — `a_command_line_downgrade_is_recorded_and_does_not_make_the_record_pass` |

**The loop closes**, and this is the sentence the whole family exists for: a behavioural claim about
an LLM step is now admissible evidence **without the LLM minting anything**. The model does not
report that it consulted the CLI before editing; a deterministic checker reads the transcript the
model produced and establishes it, and the independence boundary is not weakened but satisfiable
for the first time for a claim about *how* an agent worked.

## What is deliberately not in this wave

* **No LLM, anywhere in the checker.** This is the single most tempting place in the repository to
  break that rule — *"ask a model whether the agent behaved reasonably"* is one function call away
  and would make every verdict unreproducible and unfalsifiable at once. The eval's adversarial
  reviewer is not a counter-example: it is a separate artifact beside the report, it cannot move
  the exit code, and the protocol would classify anything it said as `Producer::Agent` and refuse
  it as independent evidence.
* **No score, no percentage, no leaderboard.** A specification is satisfied, contradicted or
  undecidable. Scores invite tuning against the score.
* **No workspace inspection.** § 3.7. The trace specification owns the transcript and nothing else.
* **No second adapter.** One harness format, versioned and declared. A second harness is a second
  adapter and not a second specification language, and until there is one the claim is untested —
  which is stated here rather than assumed.
* **No network, no clock, no randomness** in either new crate.
