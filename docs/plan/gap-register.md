# Gap register — every open question, and what closes it

Started 2026-08-20, after wave 5 shipped and wave 6 was scheduled. One row per gap that some
document records honestly but nothing yet closes. The rule for this page: a gap leaves it either
**by decision** (recorded here, implemented where stated) or **by code** (the row names the commit
or wave that closed it). A gap that quietly disappears from this page without either is the failure
mode this page exists to prevent.

Sources are the feasibility review (`docs/reviews/2026-08-20-next-waves-feasibility-review.md`),
the per-invariant *Enforced by* lines in `AGENTS.md`, and the honest-limits sections of the wave 4
and 5 records.

## Open, from 2026-08-21 — the harness family

The first two rows on this page that are neither closed by decision nor closed by code. Both are
opened deliberately by [`harness-wave-1-planning-plugin.md`](harness-wave-1-planning-plugin.md) and
its design, [`harness-planning-and-driver-design-v0.1.md`](../design/harness-planning-and-driver-design-v0.1.md),
which is the correct way for a wave to leave something owed: name it here on the way in, rather than
have a reader find it by its absence.

| gap | what closes it |
|---|---|
| **The reference driver is decided and not built.** `docs/VISION.md` § *What this is deliberately not* now says the repository ships one, and no crate implements the harness contract — the same shape of claim as a published contract with no implementor, which is the defect the driver exists to fix. The design (§ 4) is architecture with six named holes: step-map versioning, store→facts, `require_approval` headless, session granularity, failure taxonomy, concurrency | **harness wave 3**, the driver build — which opens only behind harness wave 2's feasibility review of § 4 against the code. **Or** a recorded decision not to build it, which is a legitimate outcome of that review and closes this row just as building it would, provided the VISION narrowing is reverted in the same change |
| **The planning store is durable and is not a contract implementation.** `aep-backend-markdown` writes through its own `create`/`update` rather than through `CommandService` — deviation **D-P1** against invariant 14 — so the sixteen `aep-conformance` suites do not run against it, and it has no journal, no audit join and no history (**D-P3**). Until then, "there is a durable backend" is a claim the suites do not support | **P3**, the journal-backed `CommandService`/`QueryService` for the markdown store: the two write functions reroute through command envelopes, the journal becomes the history, and the store runs the sixteen suites. `AGENTS.md` § *Current state* states both halves in the meantime |

## Closed by code, 2026-08-21 — transcript conformance, phase 2

One row, opened on the way in by phase 1 and closed the same day by the phase that had to wait for
the checker's real types. It is on this page rather than absent from it because a vocabulary with
no producer is exactly the shape of defect the page exists to catch — and it was named here while
it was still true.

| gap | what closed it |
|---|---|
| **The vocabulary admitted transcript conformance and nothing could produce it.** `EvidenceKind::TraceConformance` existed, `Verifier::TraceChecker` could establish it and `protocols/adp/1.yaml` declared both — but `Evidence` had no `TraceConformance` payload, so `Evidence::kind()` could never return the kind and the engine's admission check could never be reached with it (`crates/aep-engine/src/engine.rs:320-321` reads the kind off the payload). A protocol could require the kind and nothing could satisfy it | `Evidence::TraceConformance(TraceConformanceResult)` — the verdict, the three counts, every gapped expectation's id, the ids downgraded on the command line and the digest pair, typed so the two digests cannot be transposed; `trace_conformance.**` declared observable; `CheckReport::to_evidence` in `crates/trace-spec/src/evidence.rs`, on the producing side, with a producer nobody can set; and `protocol trace evidence`. The loop is asserted end to end rather than by inspection: `crates/protocol-cli/tests/trace_cli.rs` writes the document and feeds the file back to `protocol evaluate --evidence`, in both renderings |

Three decisions inside it, each taken deliberately and each recoverable from here rather than from
a diff:

- **The record is a summary, not the report.** An expectation's citation quotes the transcript —
  the prompt, the model's reasoning, file contents it read — and an evidence record is a thing
  people paste into pull requests. Counts, ids and two digests cross the boundary; the rows do not,
  and `--redact` is therefore not an option on the evidence verb because there is nothing left in
  the record for it to remove.
- **`trace_conformance.passed` ignores `--advisory`.** A downgrade moves the checker's exit code so
  a cost bound that drifted with model routing cannot turn a CI job red (design D6). It is a
  property of the *invocation*, not of the protocol's requirement, and a requirement a caller's own
  flag could satisfy would not be a requirement. The record names every downgraded id so the
  narrowing is visible, and the fact stays strictly stronger than exit 0 — the same polarity as
  everything else here: unproven is not proven.
- **`Evidence::spec_digest` does not opt in.** That accessor is the *resolved-model* digest the ESS
  revision binding compares against an artifact. A trace specification's digest is the digest of an
  authored YAML document about behaviour, and no ESS artifact will ever pin one — returning it
  would make every trace record fail the revision comparison for a reason unrelated to the
  revision. The match arm says so where a reader will look for it.

## Closed by decision, 2026-08-21

### D-5 — transcript conformance is its own evidence kind, not a `Verification`

The transcript-conformance design (§ 5.1) flags that `EvidenceKind` is a closed enum and that a
`TraceConformance` variant is therefore a **domain change**, belonging in the acceptance decision
rather than being discovered during implementation. Accepted, and the alternative refused for the
reason the design gives: reusing `Verification` would make a claim about *how an agent worked*
indistinguishable from every other verifier statement, and being distinguishable is the entire value
of the record.

**Executed by code, 2026-08-21 (trace-evidence phases 1 and 2).** Phase 1 is the vocabulary below;
phase 2 added the payload, the builder and the verb, and closed the row it opened — see *Closed by
code, 2026-08-21 — transcript conformance, phase 2* above.

- `EvidenceKind::TraceConformance`, wire name `trace_conformance`. No alias: the list of aliases
  exists for documents written against earlier drafts, and this kind has none.
- `Verifier::TraceChecker`, wire name `trace-checker`, the only class that can establish it — named
  separately from `conformance-runner` for the same reason that class was named separately in the
  first place, that an agent reporting on its own run is not a check of it, and the type says so.
  Deliberately **not** `artifact-validator`, which the design's own § 5.2 step example writes: a
  transcript is a record of the worker rather than an artifact of the work, and letting any artifact
  validator mint the claim gives away exactly the distinguishability this decision is about. The
  design's § 5.2 step example was the one place left disagreeing with the code, and phase 2
  corrected it in place (`docs/design/transcript-conformance-design-v0.1.md:782`).
- Declaration in a protocol document is **required**, not optional: the engine refuses a submission
  whose kind the protocol does not declare (`crates/aep-engine/src/engine.rs:321`, stated for
  harness authors at `docs/guide/harness.md:18`). Both spellings therefore go into
  `protocols/adp/1.yaml` beside `ess_conformance`, and not into the base protocol — development is
  the reversible direction, because widening a declaration to every profile later is additive and
  narrowing one is not.
- No new observable family. The engine projects `evidence.count.trace_conformance` from the kind's
  own name, and `aep/1` already declares `evidence.**` (`protocols/aep/1.yaml:119`). The
  `trace_conformance.**` family belongs with the payload that projects facts into it, which is the
  open row above.

## Closed by decision, 2026-08-20

### D-1 — predicate comparison in the diff: conservative canonical equality

The wave 5 record excludes entities and commands from the delta because their invariants and
conditions are predicates, "and predicate comparison is where an undecidable answer lives". That
sentence conflates two questions. Predicate *implication* — does the new `when` accept everything
the old one did — is undecidable in general and stays refused. Predicate *equality after
canonicalisation* is decidable and cheap, and it is all the delta needs:

- canonically equal ⇒ the construct did not change, and the delta says nothing
- canonically different ⇒ **changed**, no direction derived, and the impact closure invalidates
  everything that depends on it — the same fail-closed polarity as everything else

No "still valid" claim is ever produced from predicate reasoning; a rewritten-but-equivalent
predicate that canonicalisation cannot recognise reports as *changed* and costs a re-run, which is
the cheap error. This unblocks entities, commands, views and bindings joining the delta as a later
slice of `ess-diff`, with the four directional relations staying exactly where they are.

**Executed by code, 2026-08-21 (ESS W7.2).** The four families are compared; the canonical form is
the parsed `Predicate` exactly as the compiler resolves it (the parser's own simplifications, no
reordering, no rewrite rules), plus the author's statement where the model keeps one. See
`docs/plan/ess-wave-7-closing-the-loop.md` § W7.2.

### D-2 — linking two implementations that claim one obligation is an error

The synthesis design (§20–§21) proposes `link` over obligation implementations and separately
proposes multiple implementations per obligation, and the review (finding 8 under "What is
missing") notes nothing says how `link` chooses. Decided: it does not choose. In wave 6's linker,
zero implementations for an obligation is an unsatisfied obligation and two is an **ambiguity
error naming both**; selection among alternatives is `Realization` material (§30–§34) and stays
proposed with it. Recorded on the wave 6 plan page as a constraint on W6.3.

### D-3 — attested evidence: the proposal now exists, and is not accepted

`docs/VISION.md` names the gap: what the loop asks you to trust is that a producer declaring
itself independent is. Review finding M6 says neither design closes it, and nothing has proposed
closing it — which made it a gap with no owner. It now has a proposed shape, so accepting or
rejecting it is a decision rather than an omission:

- the conformance runner holds a keypair; the report carries a signature over the canonical report
  bytes plus the suite and specification digests
- `independent: true` stops being a self-declaration and becomes *derived*: present and valid
  signature from a registered runner key ⇒ independent; anything else ⇒ not
- key registration is deliberately out of scope of the proposal's first slice (a file of trusted
  keys beside the protocol documents is enough to make the property mechanical; rotation and
  revocation are real and later)

Status: **proposed, not accepted.** It adds a dependency class (signatures) to a workspace with
nine third-party crates and a written policy about that, which is exactly the kind of cost the
acceptance decision is for. Until accepted, the VISION's trust sentence stays as written — narrow
and named.

### D-4 — the model digest widens before anything else rests on it

`SuiteProvenance`/`Provenance` carry a 16-hex (64-bit) truncation of the model digest
(`crates/ess-gen/src/provenance.rs`), and review M5 said to widen it "if a completion decision is
going to rest on it". Gate G19 then made completion decisions rest on it, and wave 5 made suite
acceptance rest on it (`ess impact` refuses a suite whose digest mismatches). A 64-bit digest is
fine against drift and weak against construction. Decided: widen to the full SHA-256 hex in the
next model-touching batch (below), regenerating committed artifacts once. Not done live because
every committed suite and projection embeds the digest and the regeneration belongs in one commit.

## Closed by code, 2026-08-21 — wave 7, W7.5

One gap, closed by one word, and it had never been on this list because nothing had needed it: the
model could say what a component accepts and publishes and could not say **where its callers are**.
Every synthesised system therefore had exactly one derivable transport — the in-process log a
binding's `at_least_once` determines — and a second one could only have been chosen by preference,
which the wave-6 rule forbids.

Each new guard was verified by mutation before being trusted: the one-line violation it exists to
catch was applied, the failure was watched naming the defect, and the mutation reverted.

| gap | what closes it now |
|---|---|
| the model cannot state that a component's surface is reached from outside the process, so no specification can *derive* a network transport | `reached_by:` on a component (`ess-domain`), a closed two-word set — `in_process` (the default, and what silence has always meant) and `network` — validated by the raw→validated pair and refused as `EmptyDeclaration` when a network surface has neither an accepted command nor an owned domain that projects a view. Skipped from the resolved model's serialisation when unstated, so **no existing specification's digest moved**. Mutation: the rule made to return no errors, caught by `a_component_reached_over_a_network_that_serves_nothing_is_refused` |
| `openapi.rs`'s "what this refuses to guess" carried *pagination, filtering, sorting* — a view is in the IR and nothing said how one is read | the row is closed by a declaration rather than by a generator's opinion: where a component says `network`, each view its domains declare gets `GET /{domain}/views/{view}`, its rows under one key, its declared filter in the response description and its consistency as `x-ess-consistency`. Still no page size, no cursor, no ordering and no filter parameter, because the specification states none. Mutation: view paths published unconditionally, caught by `a_view_is_served_only_where_the_specification_says_something_outside_reads_it` **and** by `generate-check` on `openapi/invoice-service.yaml` |
| a synthesised server and its published contract could disagree about a path or a status | `ess_gen::http` holds one route mapping and one status mapping, read by the `OpenAPI` projection, the Rust emitter and the Go emitter. Mutation: two rows dropped from the emitted route table, caught by `the_routes_a_server_answers_are_the_routes_the_contract_declares` with both sets printed |

## Closed by code, 2026-08-21 — wave 6.5 chunk B

The last two rows of the post-wave-6 hardening batch. As with chunk A, each new guard was verified
by mutation before being trusted: the one-line violation it exists to catch was applied, the
failure was watched naming the defect, and the mutation reverted.

| gap | what closes it now |
|---|---|
| nothing relates a command's input to an emitted event payload — the one fault caught by nothing | a `payload:` declaration on a command outcome (`ess-domain`), resolved and type-checked with the binding mapping's own discipline (`ess-compiler`, `ESS-COMMAND-003` and the shared `ESS-COMMAND-002`); synthesis asserts the declared values in `ExpectEvent`, and `wrong-event-payload` moved to the caught side of the matrix — designated by `billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled`, blast radius 2. A field with no declared source stays *undetermined* by decision: the suite asserts its presence and type and never a value, and there is no `unmapped_payload_field` refusal |
| value-object invariant scenarios not synthesised (design §20) | `ScenarioId::ValueInvariant` — `<type>/invariant/at/<view>/<field>` — one scenario per observable field position that holds a value of the type, the type's own predicate rebased onto the position and required of every row with at least one row demanded. Billing gains two (`Money` at `InvoiceById.total` and `OutstandingInvoices.total`, 27→29 scenarios, refusals 1→0); what has no witness keeps a refusal under the new honest cause `ESS-SYNTH-013` rather than "not synthesised yet". The family's own fault, `negative-projected-total`, is caught by the position it corrupts with blast radius 2 |

A change to either construct lands in the command family, which the semantic delta deliberately
does not compare until W7.2 — so `ess impact` gained fail-closed mechanism 6: the uncompared
families are checked for canonical equality, and any difference owes the whole suite
(`WholeSuite::UncomparedFamilyChanged`, with the test that a payload-only change is `Whole` over an
empty delta).

## Closed by code, 2026-08-20 — wave 6.5 chunk A

Each guard was verified by mutation before being trusted: the one-line violation it exists to catch
was applied, the failure was watched naming the defect, and the mutation reverted.

| gap | what closes it now |
|---|---|
| invariant 7 — "engine never manufactures evidence" | `crates/aep-engine/tests/evidence_scan.rs`: payload types read off `Evidence` itself, every construction in shipped engine code refused, destructuring and the `submit_evidence` envelope allowed. Mutation: a fabricated `Evidence::TestResult` in `submit_evidence`, caught at file:line |
| invariant 8 — clock/RNG-free domain crate | `crates/aep-domain/tests/determinism.rs` and `crates/ess-gen/tests/determinism.rs` extend the banned-token scan to both crates that stated the property unscanned (`ess-diff` and `ess-synth` already scanned themselves; `ess-domain` states no claim). Mutations: a `SystemTime::now` in `time.rs`, a `HashMap` import in `types.rs`, both caught |
| invariant 14 — one write path | `crates/aep-contract/tests/write_surface.rs`: every method of every public trait enumerated and pinned; `CommandService::execute` is the one write path. Mutation: a default-bodied `fn purge` on `CommandService`, caught by name |
| digest widening (D-4) | `crates/ess-gen/src/provenance.rs` writes the full 64-hex SHA-256; every committed projection, suite and synthesised workspace regenerated once; `SpecDigest` still parses 16–64 so a pre-widening record fails at the comparison that names both digests, not at parse |
| property-based testing phase 1 (`proptest`) | `crates/aep-domain/tests/truth_laws.rs` (Kleene laws over generated expressions) and `crates/ess-compiler/tests/adversarial.rs` (the recorded property: any generated document is refused with reasons or compiles byte-identically twice). Fixed seeds, so the gate cannot be flaky. Mutations: `and` collapsing `Unknown` to `False`, a clock read in `to_canonical_json`, both caught with shrunk counterexamples |

## Not gaps, verified closed

Recorded so nobody re-opens them: command↔transition and command↔entity exist in the model
(wave 3.5, gates); witness synthesis refuses on `Truth::Unknown` and blames the specification
(`crates/ess-conformance/src/input.rs:165`, `:332`); scenario synthesis refuses as data rather
than failing the build (suites list every construct that got no scenario, with the reason); the
wave 3.5 page's gate count no longer disagrees with itself.
