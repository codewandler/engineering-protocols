# Gap register — every open question, and what closes it

Started 2026-08-20, after wave 5 shipped and wave 6 was scheduled. One row per gap that some
document records honestly but nothing yet closes. The rule for this page: a gap leaves it either
**by decision** (recorded here, implemented where stated) or **by code** (the row names the commit
or wave that closed it). A gap that quietly disappears from this page without either is the failure
mode this page exists to prevent.

Sources are the feasibility review (`docs/reviews/2026-08-20-next-waves-feasibility-review.md`),
the per-invariant *Enforced by* lines in `AGENTS.md`, and the honest-limits sections of the wave 4
and 5 records.

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

## Open, owned by the post-wave-6 hardening batch

One batch, one wave-sized commit series, sequenced after wave 6 so it does not race the synthesis
work. Five of its seven rows closed as chunk A (below); what remains, each with its source:

| gap | evidence | close |
|---|---|---|
| nothing relates a command's input to an emitted event payload — the one fault caught by nothing | `crates/ess-conformance/src/faulty.rs:254` | a model construct (payload-from-input mapping), then the fault matrix row stops saying "nothing" |
| value-object invariant scenarios not synthesised (design §20) | `ess conform synthesize` prints the refusal today | a later `ess-conformance` slice, as the refusal text already promises |

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
