# Infra wave 4 — project the gaps back as a diff

Status: **delivered**. This page accepts the scope it describes: the three-valued report of IW3
gets an answer to the question it leaves open — *so what would I change?* — and the answer is a
directory of files a person reviews and commits, not a verb that touches a cluster.

## Goal

**Every gap a simulation reports, turned into the change that would close it — or into a named
decision, or into a refusal.** No value in a generated file is one this build chose: it comes out
of the gap itself or out of a `remedy:` a human wrote in the specification. Applying the whole tree
must leave every gap it claims to close closed, every gap it owed exactly where it was, and nothing
else moved.

## Delivered

* `crates/infra-project` — the fifth infrastructure crate: `patch` (merge and strategic patches,
  generated objects, deterministic filenames), `project` (the three dispositions, the fixed point,
  the self-check, `infra-projection/1`), `render` (`SUMMARY.md`, `OBLIGATIONS.md`, the terminal
  form).
* `protocol infra project --spec <file> --path <bundle|ir> [--out <dir>] [--format text|json]`.
* `infra-spec` grew a `remedy:` on an expectation — the value a projection writes where the
  expectation found a field empty — with `INFRA-SPEC-009` and `INFRA-SPEC-010` beside it. A remedy
  is **never evaluated**; `simulation.json` is byte-identical with and without one.
* `infra_spec::describe_gap` and `infra_analyze::pdb_covers` became public: one rendering of a gap
  and one definition of coverage across the family, rather than a second spelling here.
* `examples/k3d-dev-cluster/projection/` — seven committed files, drift-checked with an orphan
  scan by `cargo xtask infra --check`.
* `cargo xtask infra` owns four outputs instead of three; the gate step and the CI job are
  unchanged in shape.

## Decisions taken

1. **A patch against the observed object, never a rewritten manifest.** A whole manifest generated
   from a snapshot carries every field the observation model happens to keep and silently drops
   every field it does not — which is a rewrite of somebody's deployment wearing the costume of a
   fix. A patch names only what changes, and a reviewer can read all of it. The one exception is an
   object that does not exist: there is nothing to patch, so a disruption budget arrives as a whole
   manifest.
2. **The line between a patch and an obligation is whether anything but the gap decides the
   value.** A replica count outside `[2, 4]` has one nearest acceptable number and the range says
   which. An image tagged `latest` has no mechanically-nearest replacement — a generator that
   picked `1.4.2` would have taken an engineering decision on somebody's behalf and hidden it in a
   patch file. Resources and probes sit on both sides of that line, which is what the `remedy:`
   block is for.
3. **A remedy is a field of the expectation, not a parameter of the kind, and nothing evaluates
   it.** `resources_declared` means "every container declares requests and limits" and nothing
   else, whatever quantities somebody wrote beside it. Two consequences the wave depends on:
   adding a remedy to a committed specification cannot move a committed simulation, and the value a
   patch writes is provably one a human chose. `INFRA-SPEC-009` refuses a remedy beside a kind that
   can never write it, so a specification cannot claim a projection it does not get.
4. **The projection closes what it opens.** Raising `flaky-agent` from one replica to two satisfies
   `shop-replicas` and immediately breaks `shop-pdb`, which held only because the workload had a
   single replica. Generation is therefore a **fixed point**: each round simulates the specification
   against the working model — the observed one with every change so far applied — and disposes
   what that round reports. A gap the changes opened is marked `induced`, so a reader can tell "your
   cluster is wrong about this" from "this tree would make it wrong about this, and here is what
   also closes it". It terminates because a gap is disposed at most once, keyed by (expectation,
   subject), and that set is finite.
5. **It verifies itself, and re-reads a claim it cannot support.** Two expectations can disagree —
   `[2, 4]` and `[6, 8]` over one workload — and a projection that wrote both patches would emit a
   file that closes one gap while claiming to close two. After the fixed point, every generated
   entry is checked against the final simulation; one whose gap is still open becomes a refusal with
   `Contradicted`, because picking a winner between two declared expectations is not this crate's
   decision either.
6. **A gap closed on the way is accounted for, not dropped.** The replica patch also satisfies
   `workload.replicas >= 2`, and a gap that simply vanished from the report would be one nobody
   accounted for. Every observed gap that is neither disposed nor still open is recorded as closed,
   naming what closed it.
7. **The patch type is a property of the field, and it is on the filename.** A merge patch naming
   one container replaces the whole list and deletes every container it does not mention, so a
   change inside a container is a strategic merge patch. A file's type is the join over the changes
   in it, and there is never a merge file *and* a strategic file for one object — two files against
   one object is two things to apply in an order nobody wrote down.
8. **The file is the patch and nothing else.** No header, no provenance comment, no wrapper:
   `kubectl patch … --patch-file <file>` is meant to work on the file as committed. What the file
   *is* lives in `SUMMARY.md` and in the projection document, where a reviewer reads it once for
   the whole tree.
9. **`OBLIGATIONS.md` is a separate file.** A projection that emitted nine changes and quietly left
   sixteen decisions unmade reads, in a pull request, exactly like one that closed everything.
   Split out, the second file is a diff of its own: it appears, it grows, it shrinks, and a
   reviewer sees which.
10. **Both digests, on every artifact.** A tree derived from "the specification" and "the cluster"
    is not reviewable, because a name is not an identity — two revisions of `expected.yaml` share a
    name. `InfraSpec::digest` was added for this, computed the way the IR's is and through the same
    function, so there is one implementation of the algorithm in the family.
11. **A disruption budget is `maxUnavailable: 1` over the workload's own selector.** The only bound
    the gap itself determines: it says "more than one replica and nothing covers it", and one pod
    at a time is the weakest budget that answers it. `minAvailable: <replicas>` would block every
    voluntary eviction, and any other number is a decision about how much of this workload may be
    down. The selector is the workload's own, checked with `infra_analyze::pdb_covers` before it is
    written — a budget that covers nothing would leave the gap exactly where it was.
12. **A generated manifest carries no `uid`.** It is what a person commits, and identity is the API
    server's to assign. That is the one honest seam between this tree and an observation bundle,
    and the round-trip test supplies the uid itself rather than letting the library do it.
13. **The command writes and never deletes.** A projection lands in a directory somebody owns, so a
    file the command did not write is named on stderr and left alone — a verb that pruned would eat
    a hand-edited patch on its way to being committed. The committed fixture *is* held to having no
    extra files, because there the owner is this repository and `cargo xtask infra --check` is the
    owner's scan.
14. **The merge-patch applier lives in the tests.** Applying is the acting half, and an applier
    that shipped would be the first half of the verb this repository refuses. It is thirty lines,
    it takes no dependency, and it implements RFC 7386 plus the one keyed-list rule the emitted
    strategic patches rely on — narrower than the real thing on purpose, because an applier that
    guessed would prove less.
15. **Exit 0 whatever it found.** IW2 decision 6 and IW3 decision 2, unchanged. A cluster with
    sixteen decisions owed has been successfully projected. Exit 1 is an input that could not be
    projected at all.

## Patch or obligation, per gap kind

Twelve gap kinds. The `patch` column is what the projection writes; the `negative` column is the
case beside it that gets a different answer, which is what keeps the rule from widening quietly.

| gap | disposition | patch type | negative case |
|---|---|---|---|
| `replicas_outside_range` | **patch** — set to the nearest bound the range names | merge | reached only when another expectation already wrote a different count (then: refused) |
| `resources_absent` | **patch** iff the expectation carries a `remedy: {resources: …}` stating every missing half | strategic | no remedy, or a remedy stating only one of two missing halves → obligation `value_unstated` |
| `probe_absent` | **patch** iff the expectation carries a `remedy: {probes: …}` stating every missing probe | strategic | no remedy → obligation `value_unstated` |
| `disruption_budget_absent` | **new object** — `maxUnavailable: 1` over the workload's selector | — (manifest) | the name is taken, the workload has no selector, or its selector does not match its own template labels → obligation |
| `workload_absent` | obligation `object_undefined` | — | — |
| `image_registry_not_allowed` | obligation `image_choice` | — | — |
| `image_tag_is_latest` | obligation `image_choice` | — | — |
| `image_not_pinned` | obligation `image_choice` | — | — |
| `selector_matches_no_pod` | obligation `target_unknown` | — | — |
| `reference_unresolved` | obligation `target_unknown` | — | — |
| `namespace_not_allowed` | obligation `field_immutable` | — | — |
| `predicate_false` | **refused** `not_a_field` | — | — |

Three of the four generating kinds are load-bearing on the committed fixture. The probe *patch* is
not, deliberately: `shop-probes` is namespace-scoped over four unrelated containers, and one
liveness probe for `storefront-server`, `switchboard`, `agent` and `redis` would be a guess written
as a specification. The fixture carries the obligation instead, and the patch is exercised where a
single container is in scope (`a_stated_probe_is_written_into_the_container_that_lacks_it`). The
same asymmetry is the reason `shop-resources` *does* carry a remedy: a namespace-wide default
envelope is a real practice — it is what a `LimitRange` states — and a probe is per-container.

The two refusal classes are both off the fixture for the same kind of reason: its one false
predicate is `workload.replicas >= 2`, which the replica patch closes on the way past, and a
contradiction needs two expectations that disagree. Both have purpose-built cases in
`tests/projection.rs`.

## The refusal catalogue this wave adds

| code | refuses |
|---|---|
| `INFRA-SPEC-009` | a remedy beside an expectation kind that can never write it |
| `INFRA-SPEC-010` | a remedy that states nothing, states a probe the expectation never asks for, holds two handlers or none, or names a port as a quoted number |

## The committed fixture

`examples/k3d-dev-cluster/projection/` — 2 patch files, 3 generated objects, `SUMMARY.md`,
`OBLIGATIONS.md`.

| | |
|---|---|
| gaps in the snapshot | 23 |
| gaps these changes open | 2 |
| closed by a generated change | 9 |
| owed as an obligation | 16 |
| refused | 0 |
| expectations before | 11 hold, 12 gaps, 5 undecidable |
| expectations after the whole tree is applied | 15 hold, 8 gaps, 5 undecidable |

Two of the three generated budgets are for gaps the projection's own replica patches opened. That
is the wave in one artifact: the tree does not trade one gap for another, and it says out loud
which of its entries are its own doing.

## The round trip, asserted

`crates/infra-project/tests/round_trip.rs` reaches the post-state a second, independent way: it
applies the emitted patch files to `observation.json`, appends the generated manifests, recompiles
the bundle from scratch and re-simulates. Then, for every entry:

* generated ⇒ the gap is **gone**;
* owed or refused ⇒ the gap is **exactly as it was**, same typed `Gap` value;
* every other subject outcome — including every `unknown` and its reason — is **identical**;
* and the summary the projection predicted is the summary the recompiled snapshot produces.

The third bullet is the blast radius: a patch that satisfies one expectation by breaking another
passes the first two and fails that one, with a sentence naming the expectation that moved.

## Mutations run

Each applied, watched fail with the failing test named, reverted (the `AGENTS.md` convention):

| mutation | caught by |
|---|---|
| the fixed point becomes a single pass, so an induced gap is never closed | `a_gap_this_projections_own_changes_open_is_marked_as_such_and_closed_in_the_same_tree` |
| the nearest bound is always the ceiling | `a_replica_count_below_the_range_is_raised_to_the_floor_and_nothing_more` |
| a half nobody stated is written as an empty map | `a_remedy_that_states_only_one_missing_half_leaves_the_whole_gap_owed` |
| the contradiction self-check is dropped | `two_expectations_that_disagree_leave_one_of_them_refused_rather_than_silently_lost` |
| a gap closed on the way is not accounted for | `every_gap_the_snapshot_reports_gets_exactly_one_entry_and_no_gap_is_lost`, `applying_the_emitted_tree_closes_every_gap_it_claims_and_moves_nothing_else` |
| a container patch is emitted as a plain merge patch | `a_file_holding_a_container_change_is_strategic_however_it_was_reached`, and the round trip |
| the container entry drops the `name` merge key | `the_container_list_carries_the_merge_key_it_is_matched_by`, and the round trip |
| the budget name-collision guard is dropped | `a_budget_whose_name_is_taken_is_owed_rather_than_written_over` |
| the secret scan is pointed at a string the tree does contain | `no_emitted_byte_carries_a_secrets_digest_or_key_name` (the scan itself fires) |
| the remedy strip stops stripping, so the equivalence test compares one document with itself | `the_committed_example_specification_simulates_identically_with_and_without_its_remedies` |
| `INFRA-SPEC-009` is not checked | `a_remedy_beside_a_kind_that_never_finds_an_empty_field_is_refused_rather_than_carried` |
| an emitted patch value is corrupted back to the observed one | `a_corrupted_patch_value_is_caught_and_the_regressed_expectation_is_named` |

## Acceptance (all held)

* Every gap kind gets the disposition the table above states, with a negative case for each of the
  four that generate.
* Applying the whole committed tree to the bundle, recompiling and re-simulating closes every gap
  the projection claims, leaves every owed gap byte-identical, and moves no other outcome.
* Two projections of one pair are byte-identical, a reversed bundle projects to the same bytes, and
  the committed tree is exactly what the library produces — in both directions, so a file nothing
  generates any more fails the check.
* No emitted byte carries a secret's digest, and the one place the projection is asked for a secret
  answers with an obligation that says why nothing here can write one.
* `simulation.json`, `cluster.ir.json` and `drift.json` are byte-identical to before this wave: the
  `remedy:` added to `expected.yaml` changed no verdict.

## What stays out

* **Apply.** Unchanged, and this wave is where it would have been easiest to slip: the tree is a
  proposal, the merge-patch applier lives in the tests, and the only mention of `kubectl` in this
  crate is a sentence in a document. The verb belongs to the scanner's repository, on the other
  side of the credential boundary, and is not scheduled here.
* **Choosing an image.** IW5's `propose` may consult a registry to answer "which tag replaces
  `latest`"; that needs a network, so it is adapter-side by construction.
* **Patching anything but the four fields above.** Environment variables, volumes, selectors,
  labels and ingress routing are all patchable in principle and none of them has a gap kind that
  determines a value. The vocabulary grows when a gap does.
* **A second patch format.** No JSON Patch (RFC 6902): it addresses by index into a list, and the
  one list this crate touches is the one whose indices mean nothing.
* **Reading a projection back.** It is an output; nothing here consumes one as an input, so it has
  no `Raw*` half — `ess-diff`'s argument, third instance.
* **Promoting an `INFRA-PROP-*` candidate into an expectation.** Still a generator, still not this
  one: this wave writes changes to a *cluster*, and that one would write changes to a
  *specification*.
