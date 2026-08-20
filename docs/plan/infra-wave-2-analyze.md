# Infra wave 2 — analyze and visualize

Status: **delivered**. This page accepts the scope it describes: the compiled IR of IW1 becomes
*analyzed* — what depends on what (`protocol infra graph`), what is wrong
(`protocol infra diagnose`), and the per-workload facts IW3 will consume
(`protocol infra inspect --properties`).

## Goal

A real cluster's scan, mapped and diagnosed: a typed dependency graph with a reason on every
edge, and a diagnosis whose every finding carries a stable `INFRA-DIAG-*` code, a registered
severity, and the evidence it rests on. This closes the session goal "local dev cluster can be
mapped and analyzed".

## Delivered

* `crates/infra-analyze` — the third infrastructure crate: `graph` (typed edges, Mermaid and
  JSON renderings, derived pod ownership), `diagnose` (fourteen rules), `code` (the
  `INFRA-DIAG-*` registry with severities), `properties` (replicas, parsed images, resource
  envelopes).
* `infra-compiler::read_document` — a persisted `infra-ir/1` document back into a typed
  `InfraIr`, through validation (`INFRA-IR-001`…`004`), never through `Deserialize`.
* `protocol infra graph --path <bundle|ir> [--namespace <ns>] [--format mermaid|json]`,
  `protocol infra diagnose --path <bundle|ir> [--format text|json] [--min-severity …]`, and
  `--properties` on `protocol infra inspect`.
* The fixture extended so all fourteen rules, all ten edge relations and the ownership
  derivation are load-bearing on it, with a negative case per rule; committed IR regenerated.

## Decisions taken

1. **A separate `infra-analyze` crate**, not more modules in `infra-compiler` — the same split
   the ESS side settled on (`ess-compiler` versus `ess-diff`): compilation produces the IR,
   analysis interprets it, and a consumer that only compiles should not carry the interpreter.
2. **Reading a persisted IR back is a validation, not a `Deserialize`.** The digest proves the
   document was not edited (`INFRA-IR-002`) — it cannot prove a compiler wrote it, because any
   author can stamp a fresh digest over any model. What keeps the IR's total lookups total is
   the relational check: every `resolved` reference's key must exist in its map
   (`INFRA-IR-004`), accumulated with everything else in one run. Value-level rules
   `infra-domain` already enforced are deliberately not re-run; they cannot turn a lookup into
   a panic. Mirror types live in `infra-compiler/src/read.rs` because handle minting is
   crate-private — a read-back anywhere else would need a second minting door.
3. **The graph draws only what was observed.** A dangling reference is an IR fact and a
   diagnosis finding, not a phantom node; the edge vocabulary is closed (ten relations, each
   with a verb), every edge carries the sites in the dependent that state it, and one edge per
   `(from, relation, to)` holds all its sites rather than one arrow per site.
4. **Pod ownership is derived where the evidence closes, refused where it does not.** A
   ReplicaSet owner derives to its deployment only when the pod's `pod-template-hash` label is
   the owner-name suffix; StatefulSet and DaemonSet owners name their workload directly.
   Everything else — bare pod, Job pod, hash that does not confirm, derivation landing on
   nothing observed — is a typed `UnderivedOwner` fact with one of four reasons, carried on the
   graph document. Facts, not guesses; the mutation register below breaks the hash check to
   prove the confirmation is load-bearing.
5. **Mermaid draws the configuration topology; JSON is canonical.** The k3d cluster observes
   1186 pods; a flowchart with 1186 boxes is not a rendering anyone reads. So the diagram shows
   workloads, services, ingresses, configmaps, secrets, service accounts and claims, grouped in
   one subgraph per namespace, and the runtime layer (pods, cluster nodes, ownership,
   scheduling) lives in the JSON document — which carries every node, edge, site, the
   underived-owner facts, and the source IR's digest so the two documents chain.
6. **A diagnosis is a report, not a gate.** `protocol infra diagnose` exits 0 whatever the
   findings say — three errors are a *successful* diagnosis of an unhealthy cluster. Exit 1 is
   reserved for input that could not be diagnosed at all: a refused bundle, a tampered or
   hand-forged IR document. Anything else would make "run diagnose in CI" turn every cluster
   hiccup into a red build, which is a policy decision this wave does not take; a gate can be
   layered on `--format json` output when someone wants one.
7. **Severity is a function of the code.** The registry (`diag_codes!`, the third instance of
   the `validation_codes!` idiom) binds wire string, severity and meaning on one line each.
   Where one fact class genuinely splits by seriousness, the split is two codes — a required
   reference that is absent is `INFRA-DIAG-002` *error*, an optional one is `-003` *info* —
   so `--min-severity` means the same thing on every line.
8. **Two typed exemptions, both mutation-verified.** The stuck-waiting rule ignores
   `ContainerCreating`/`PodInitializing` (normal startup); the orphan rule skips secrets of
   type `kubernetes.io/service-account-token` (the token controller's, consumed by machinery
   outside the modelled reference surface). Each exemption has a fixture object that exists to
   fail the test if the exemption is removed.
9. **The orphan rules say what they checked.** A statefulset's `volumeClaimTemplates` claims
   and kubelet-projected configmaps reach pods outside the modelled sites, so `-011`/`-013`
   findings read "no env, envFrom or volume site … references", never "unused". The fixture's
   `config-asterisk-0` (a real volumeClaimTemplate claim) fires `-013` under exactly that
   wording — a true statement about the modelled surface.
10. **Model additions, IW2-shaped.** `ContainerStatus.waiting_reason` (verbatim kubelet
    vocabulary, not an enum that would erase the word an operator needs) and
    `PersistentVolumeClaim.phase` (`ClaimPhase`, closed like `PodPhase`) joined `infra-domain`
    — the subset extends exactly as far as the wave that asks (IW1 decision 2). An *unknown*
    claim phase does not fire `-012`: unobserved is not unbound (invariant 5's spirit).
11. **`HIGH_RESTART_THRESHOLD` is 5, a registered constant.** A flag would let two runs of one
    build disagree about what "high" means.

## The finding catalogue

| code | severity | fires when |
|---|---|---|
| `INFRA-DIAG-001` | warning | a service selector matches no observed pod |
| `INFRA-DIAG-002` | error | a required reference names something not observed |
| `INFRA-DIAG-003` | info | an optional reference names something not observed |
| `INFRA-DIAG-004` | warning | a container has no resource requests and/or limits |
| `INFRA-DIAG-005` | warning | a container has no liveness and/or readiness probe |
| `INFRA-DIAG-006` | warning | an image is `:latest` or untagged, and not digest-pinned |
| `INFRA-DIAG-007` | info | a workload wants exactly one replica |
| `INFRA-DIAG-008` | error | a container waits with a non-transient reason (`CrashLoopBackOff`, …) |
| `INFRA-DIAG-009` | warning | a container restarted ≥ 5 times |
| `INFRA-DIAG-010` | warning | a controller-managed pod is not ready and not `Succeeded` |
| `INFRA-DIAG-011` | info | a configmap/secret referenced by no modelled site |
| `INFRA-DIAG-012` | warning | a claim observed `Pending` or `Lost` |
| `INFRA-DIAG-013` | info | a claim referenced by no workload volume |
| `INFRA-DIAG-014` | info | two or more services select the identical workload set |

`INFRA-DIAG-010` is scoped to pods whose controller *derived*: a bare pod states no readiness
expectation and a Job's pod finishing is its job — both already surface as underived-owner
facts on the graph.

## Mutations run

Each applied, watched fail with the failing test named, reverted (the `AGENTS.md` convention):

| mutation | caught by |
|---|---|
| each of the 13 rule functions early-returned (14 codes) | its per-rule test **and** `every_registered_code_fires_at_least_once…`, naming the code |
| benign-waiting exemption removed | `a_crashlooping_container_is_an_error_and_a_creating_one_is_not` |
| service-account-token exemption removed | `unreferenced_config_fires_and_referenced_or_token_managed_config_does_not` |
| `findings.sort()` removed | `findings_arrive_sorted_and_each_carries_its_codes_registered_severity` |
| severity hardcoded at the constructor | the severity test and the floor test |
| template-hash confirmation dropped from ownership derivation | `a_replicaset_name_derives_its_deployment_only_when_the_hash_confirms_it` |
| read-back relational check removed | `a_hand_written_resolved_claim_is_refused_even_when_its_digest_is_freshly_stamped` |

## Acceptance (all held)

* All fourteen codes fire on the committed fixture, each with a negative case beside it; the
  committed IR is regenerated and drift-checked by the unchanged ninth gate step.
* Graphing the bundle and graphing the committed IR document produce byte-identical JSON
  through the real binary — the read-back is indistinguishable from a fresh compile.
* Two constructions render byte-identical graph JSON, Mermaid and diagnosis (invariant 9),
  and the crate is under the same banned-token scan as `infra-compiler`.
* The real k3d bundle (1186 pods) and a dev-cluster bundle (301 pods, 29 namespaces) validate,
  compile, graph and diagnose end to end; numbers on the wave report.

## What IW3 is

Desired state: a declared target model, semantic diff observed↔desired over the properties this
wave extracts, and simulation of a change before anything applies it.
