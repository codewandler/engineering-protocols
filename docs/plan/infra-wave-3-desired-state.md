# Infra wave 3 — desired state, simulate, drift

Status: **delivered**. This page accepts the scope it describes: the analyzed cluster of IW2 gets
something to be measured *against* — an authored desired state, a three-valued report of how the
snapshot answers it, and a typed comparison of two snapshots.

## Goal

**A sentence somebody writes about how the cluster ought to be, evaluated against what was
observed — and a third answer beside yes and no.** Every expectation gets `true`, `false` or
`unknown`; every `false` carries the typed gap between have and want; every `unknown` carries the
reason the snapshot cannot decide. Beside it, `protocol infra diff` answers the other question a
running cluster raises: what moved since the last scan.

## Delivered

* `crates/infra-spec` — the fourth infrastructure crate: `spec` (twelve expectation kinds, three
  scopes), `raw` (raw→validated, eight `INFRA-SPEC-*` refusals, accumulating), `facts` (the
  workload fact sheet the predicate escape hatch reads, and why each withheld fact is withheld),
  `simulate` (the evaluator, `infra-simulation/1`), `drift` (sixteen change kinds,
  `infra-drift/1`, one refusal), `render` (the text renderings).
* `INFRA-SPEC-001`…`008` joined `infra-domain`'s registry — the same registry `INFRA-IR-*` already
  lives in, and `ValidationErrors::into_result` beside it.
* `protocol infra simulate --spec <file> --path <bundle|ir> [--format text|json]` and
  `protocol infra diff --from <bundle|ir> --to <bundle|ir> [--format text|json]`.
* `examples/k3d-dev-cluster/` grew three files: `expected.yaml` (28 expectations, all twelve
  kinds, all three verdicts, five undecidable — one per reason), `observation.drifted.json` (the
  same cluster twenty documented mutations later) and the two committed reports.
* `cargo xtask infra` now owns three outputs instead of one; the gate step and the CI job are
  unchanged in shape.

## Decisions taken

1. **Two verbs, not one, and the desired↔observed comparison lives inside `simulate`.**
   `simulate` compares *unlike* things — a specification against a snapshot — and `diff` compares
   *like* things: two snapshots. That is the line, and it is why there is no `diff --spec`: every
   `false` verdict already carries its `Gap`, which is exactly the have-versus-want a third verb
   would have printed. Two producers of one answer is how a report and a gate come to disagree.
   The `ess diff`/`ess impact` split is the precedent in the other direction — those are two
   verbs because they are two *computations* over one delta, where this would have been two
   renderings of one.
2. **A simulation is a report, not a gate.** Exit 0 whatever the verdicts say; exit 1 only for an
   input that could not be simulated — a refused specification, a refused bundle, a tampered IR.
   IW2 decision 6, unchanged, and for the sharper version of its reason: an aspiration written
   down for the first time is least true on the day it is written, and a verb that goes red then
   is a verb nobody writes an aspiration for.
3. **The verdict is the variant, not a field beside it.** `Outcome` has three arms:
   `Holds` carries nothing, `Gap(Gap)` carries the typed have-versus-want, `Undecidable(reason)`
   carries the reason. There is no `Option<Gap>` next to a separate `verdict`, no `from_bool`, and
   `Outcome::verdict()` is derived. So a future rule *cannot* report a failure nobody can act on,
   and cannot report `unknown` without saying why — invariant 5 made structural rather than
   remembered, the way `Truth` itself does it.
4. **An expectation's verdict is the Kleene conjunction over its subjects**, not the worst of the
   three. `Unknown` beside `False` is `False`, because something was observed to be wrong. The
   committed fixture never reaches that state, so a purpose-built bundle does, and the test
   asserts the fixture reached it before asserting the rule.
5. **An empty scope is `unknown`, not vacuously true.** "Every container in `payments` declares
   limits" over a namespace holding no workloads is a sentence with no subject. Answering `true`
   would make selecting nothing the one way an expectation can be green and mean nothing.
6. **Six reasons, each naming something a reader can change**: no subject in scope; the namespace
   was not observed; the bundle did not scan the kind; the subject declares no such field; a pod's
   controller is underivable; a predicate read a withheld fact — and that last one nests the
   reason the fact was withheld, so the report says "the bundle did not scan disruption budgets"
   rather than "unknown".
7. **Pod counts are withheld for a whole namespace when any pod in it has an underivable
   controller.** An underived pod could belong to any workload in its namespace, so counting the
   derivable ones and calling it the workload's pod count turns a lower bound into a measurement.
   On the fixture the bare `debug-shell` pod does exactly this to `shop`.
8. **No second predicate language.** The escape hatch is `aep_domain::predicate::Predicate`
   evaluated against a `FactStore` this crate projects, with `Truth` semantics unchanged. Nothing
   here re-implements three-valued logic, so there is no second place for `Unknown` to be
   collapsed — and the `Unknown` arm is *produced by the evaluator*, not written here. The one
   addition is `INFRA-SPEC-008`: a predicate reading a path the projection never states is refused
   at validation, because a typo that evaluates `unknown` forever is a lie about a typo. No new
   path or selector concept was needed; the projection is eighteen `workload.*` facts.
9. **No wall clock, and no vocabulary for one.** Review finding I7. No expectation compares a
   timestamp and there is no duration type to write one with, so every verdict is a pure function
   of the specification and the snapshot — which is what lets the report be committed and
   drift-checked at all. A banned-token test forbids `Duration`, `elapsed`, `scanned_at` and
   `Timestamp` in this crate's sources, so a future `observed_within` fails before it is written.
10. **Drift is over declared state; pods are out.** A deployment's pods are renamed on every
    rollout, and a report listing a thousand of them is one nobody reads twice — IW2 decision 5's
    argument, applied to a different rendering. A test renames every pod in the fixture and
    asserts the report is empty.
11. **Containers compare by name, never by position.** Reordering a template's containers changes
    nothing about the system; a positional comparison would report every one of them moved.
12. **No catch-all in the change vocabulary.** Where a construct has more comparable fields than
    are worth a variant each, the change carries a closed enum — `WorkloadField`, `ServiceField` —
    so a reader can enumerate what this build can report and a new field cannot arrive as prose.
13. **A reference event is reported only for a holder present in both snapshots.** An object that
    arrived brought its references with it; one that left took them with it. The membership change
    already says what happened, and reporting both would count one event twice.
14. **One refusal on each side.** A specification whose format this build does not read is
    refused; two snapshots scanned in different kubeconfig contexts are refused. The second
    mirrors `ess diff`'s single `DifferentSystem`, and for its reason: comparing two clusters
    produces a change list where everything was added and everything was removed.
15. **`INFRA-SPEC-*` joins `infra-domain`'s registry rather than starting a second one.** The
    registry was already wider than that crate's own documents — `INFRA-IR-001`…`004` refuse a
    persisted IR document `infra-domain` never sees. One enum, one `ALL`, one accumulator across
    the family; the alternative is three parallel registries and a consumer that has to know which
    one a refusal came from before it can print it.
16. **The specification is read through one function.** `infra_spec::read_spec` parses YAML into a
    `serde_json::Value` and deserializes from *that*, because `serde_yaml` 0.9 spells an
    externally tagged enum as a YAML tag (`!replicas_within`) where the JSON data model spells one
    as a single-key map — which is the shape a specification is written in. One conversion buys
    the readable wire form and `deny_unknown_fields` on every variant at once, and a JSON
    specification reads through the same path because JSON is YAML.
17. **The kind is nested under `expect:` rather than flattened beside `id:`.** serde's
    `deny_unknown_fields` does not survive `flatten`, and a specification where `mim: 2` silently
    becomes `min: 0` is worse than one that is a line longer.
18. **Three committed reports, drift-checked.** `cargo xtask infra` grew from one output to three.
    A simulation is committed for a sharper version of the reason the IR is: a rule that quietly
    starts answering `false` where it answered `unknown` moves those bytes and nothing else.

## The expectation vocabulary

Twelve kinds. Each row's `unknown` column is the part a reader will otherwise assume away.

| kind | holds when | `unknown` when |
|---|---|---|
| `workload_exists` | a workload of that namespace, kind and name was observed | never — the three workload kinds are required in every bundle |
| `replicas_within` | every workload in scope declares a count in `[min, max]` | a daemonset, which declares no count at all |
| `resources_declared` | every container declares requests **and** limits | never |
| `probes_declared` | every container declares the probes asked for | never |
| `image_registry` | every image names a registry in the allowlist | an image names no registry — the default one resolves it, and the snapshot does not carry which |
| `image_tag_not_latest` | no image resolves to `latest` (untagged counts; a digest pin excuses the tag) | never |
| `image_pinned_by_digest` | every image carries a `sha256:` digest | never |
| `pdb_covers_multi_replica` | every workload declaring >1 replica has a covering budget | the bundle did not scan budgets, or the workload is a daemonset |
| `service_selector_resolves` | every service's selector matches ≥1 observed pod | the service has no selector — its endpoints are managed elsewhere, which is legal |
| `config_references_resolve` | every **required** configmap/secret reference resolves | never — an optional dangling reference *holds*, the `INFRA-DIAG-002`/`-003` split |
| `namespace_allowlist` | every workload in scope sits in an allowed namespace | never |
| `workload_predicate` | the predicate is `True` for every workload in scope | the predicate reads a fact the projection withheld — carrying *why* it was withheld |

Scopes: `cluster`, `{namespace: <name>}`, `{workload_selector: {<labels>}}`. A scope that cannot
select its expectation's subject class is refused (`INFRA-SPEC-006`) rather than silently selecting
nothing.

## The refusal catalogue

| code | refuses |
|---|---|
| `INFRA-SPEC-001` | a `format` this build does not read |
| `INFRA-SPEC-002` | a document that does not read as the shape at all — including an unknown kind and a misspelt parameter |
| `INFRA-SPEC-003` | two expectations sharing an id |
| `INFRA-SPEC-004` | a specification with no expectations |
| `INFRA-SPEC-005` | parameters that can decide nothing: `min` above `max`, an empty allowlist, neither probe asked for, a blank name, an unknown workload kind, an empty selector, an always-true predicate |
| `INFRA-SPEC-006` | a scope that cannot select this expectation's subjects |
| `INFRA-SPEC-007` | an id that is not a stable identifier |
| `INFRA-SPEC-008` | a predicate reading a fact the workload projection never states |

## The drift change vocabulary

Sixteen kinds, all sixteen load-bearing on the committed pair: `added` and `removed` (over eight
member kinds), `replicas_changed`, `container_added`, `container_removed`, `image_changed`,
`resources_changed`, `probes_changed`, `environment_changed`, `workload_field_changed`,
`service_field_changed`, `ingress_routing_changed`, `config_content_changed`,
`claim_phase_changed`, `reference_broke`, `reference_healed`.

`config_content_changed` names which keys were added, removed or changed and **never** what they
hold — not the value, and not even the digest of one, which the IR does carry and this deliberately
does not repeat. A test asserts the report leaks neither.

## Mutations run

Each applied, watched fail with the failing test named, reverted (the `AGENTS.md` convention):

| mutation | caught by |
|---|---|
| the `Unknown` arm of the predicate evaluator answers `False` | `each_undecidable_expectation_on_the_fixture_carries_the_reason_its_name_promises` |
| the Kleene fold becomes worst-of-three, so `Unknown` buries `False` | `one_scope_holding_both_a_contradicted_and_an_undecidable_subject_reads_false` |
| an empty scope is vacuously satisfied | `a_scope_naming_an_observed_but_empty_namespace_is_undecidable_not_vacuously_satisfied` |
| an unscanned budget kind is read as "no budgets" | `a_bundle_that_never_scanned_disruption_budgets_is_undecidable_and_not_uncovered` |
| a daemonset's absent replica count is read as zero | `the_committed_example_reaches_all_three_verdicts_and_the_counts_are_the_documented_ones` |
| an image naming no registry is blamed instead of being undecidable | `each_undecidable_expectation_on_the_fixture_carries_the_reason_its_name_promises` |
| the optional/required split is dropped and every dangling reference is a gap | `an_optional_dangling_reference_holds_and_a_required_one_does_not` |
| the pod-ownership blindness is ignored and counts become measurements | `each_undecidable_expectation_on_the_fixture_carries_the_reason_its_name_promises` |
| the `INFRA-SPEC-008` fact check is skipped | `a_predicate_reading_a_fact_the_projection_never_states_is_refused_as_a_typo` |
| containers compare by position rather than by name | `reordering_a_templates_containers_is_not_a_change_because_containers_compare_by_name` |
| a reference event is reported for a holder present on one side only | `every_change_kind_the_pair_was_built_to_exercise_appears_exactly_where_it_should` |
| the change ordering is dropped | `the_committed_documents_are_what_the_library_produces_right_now` |
| two different contexts are compared instead of refused | `two_snapshots_of_different_clusters_are_refused_rather_than_compared` |

## Acceptance (all held)

* All twelve expectation kinds fire on the committed fixture with a negative case beside each;
  `image_pinned_by_digest`'s positive is a named exception in a purpose-built bundle, and the test
  that allows it names it so the exception stays deliberate.
* All six `unknown` reasons are reachable and asserted; five on the committed fixture, the sixth
  (`kind unscanned`) on a bundle that never scanned budgets.
* All sixteen drift change kinds fire on the committed pair, and the pair produces exactly those
  sixteen.
* Simulating the bundle and simulating the committed IR produce byte-identical documents through
  the real binary — the read-back is indistinguishable from a fresh compile.
* Two runs render byte-identical simulation JSON, drift JSON and both text forms (invariant 9), a
  reversed bundle produces the same bytes, and the crate is under the same banned-token scan as the
  other three infrastructure crates, plus a second scan forbidding the duration vocabulary.
* The real k3d bundle (1186 pods, 11 MB) simulates end to end against the example specification;
  the numbers are on the wave report.

## What stays out

* **Apply.** Nothing in this workspace reaches a cluster, and a gap report is not a plan to change
  one. `docs/VISION.md` refuses the credential; this wave refuses the verb. A gap says *what would
  have to change*, and a human or another tool changes it.
* **Manifest projection.** Generating manifests *from* a desired-state model is IW4. It needs a
  model that describes a whole workload, where this one describes expectations *about* one — a
  different document, not a bigger version of this one.
* **Wall-clock freshness.** See decision 9 and review finding I7.
* **Reading a simulation or a drift report back.** Both are outputs; nothing in this repository
  consumes one as an input, so neither has a `Raw*` half. The day something does — a gate that
  compares yesterday's report with today's — is the day that half is owed, and it is the same
  argument `ess-diff`'s `RawEssDelta` already makes on the other side.
* **Promoting an `INFRA-PROP-*` invariant candidate into a declared expectation.** IW2.5 mines the
  uniformity a cluster almost keeps and IW3 can express every one of the three as an expectation
  by hand; the *automatic* promotion — read a candidate, write the `expected.yaml` line — is a
  generator, and it belongs beside IW4's projection work rather than in the evaluator.
* **A percentage or a majority rule.** `infra_analyze::invariants` answers "what does this cluster
  almost do"; this answers "does it do what I declared". Mixing the two would make a declared
  expectation satisfiable by 51 % of a namespace.
