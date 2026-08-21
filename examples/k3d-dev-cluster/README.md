# k3d-dev-cluster — the example observation

A small, deterministic `infra-observation/1` bundle and the `infra-ir/1` document it compiles to.

| file | role |
|---|---|
| `observation.json` | input: a trimmed observation, derived from a real scan (see below) |
| `observation.drifted.json` | input: the same cluster one working day later — twenty documented mutations of the file above |
| `expected.yaml` | input: the desired state, as somebody would write it (`infra-spec/1`) |
| `cluster.ir.json` | output: what `protocol infra compile` produces from `observation.json` |
| `simulation.json` | output: what `protocol infra simulate --spec expected.yaml --path observation.json` produces |
| `drift.json` | output: what `protocol infra diff --from observation.json --to observation.drifted.json` produces |

The three outputs are committed and drift-checked by `cargo xtask infra --check`. Never edit one by
hand; regenerate all three with `cargo xtask infra`. The three inputs *are* hand-maintained — they
are fixtures, not outputs.

## Derivation

The bundle was derived from a live scan of a k3d development cluster
(`infra-scout scan`, context `k3d-example`, 2026-08-20 — 12 kinds, 1186 pods), reduced to a
subset small enough to review and rich enough that every construct the model reads appears:

* **Kept:** 2 namespaces (`kube-system`, `shop`), the single node, 5 workloads — the `coredns`
  deployment (probes, configmap volumes, and a genuinely dangling *optional* reference to
  `coredns-custom`, exactly as k3s ships it), the `storefront-server` deployment (env literals
  and an optional `secretKeyRef`), the `queue-redis` deployment (a claim-backed volume), the
  `switchboard` statefulset (governing service), the `svclb` daemonset — plus the services that
  select them, the configmap/secrets/service accounts/claims they reference, and 6 pods across
  the phases `Running`, `Pending` and `Failed`.
* **Stripped per object:** `managedFields`, `resourceVersion`, `generation`, `creationTimestamp`,
  annotations, status beyond the modelled essentials, and spec fields outside the k8s subset v1
  (`infra-domain`'s module documentation lists the exclusion classes and why).
* **Synthesized (not present in the live cluster), each to make a modelled construct appear:**
  1. an `envFrom` import on `storefront-server` and the `storefront-env` configmap it reads —
     the live cluster uses no `envFrom`;
  2. the `lost-lookup` service, whose selector `app: retired` matches no pod — the
     dangling-selector fact;
  3. the `edge` ingress — the live cluster has none — with one healthy backend
     (`storefront-server`) and one path to the nonexistent `retired-api` service.
  Synthetic objects carry `synthetic-…` uids.
* **Synthesized for IW2**, so that every diagnosis rule (`INFRA-DIAG-001`…`014`) and every
  graph construct is load-bearing on this one fixture — each entry names what it exists to
  exercise:
  1. the `flaky-agent` deployment: untagged image, no probes, no resource bounds, one replica,
     and a *required* env reference to the absent `agent-credentials` secret
     (DIAG-002/004/005/006/007);
  2. its pod `flaky-agent-6d8f9c7b44-x1q2z`: `CrashLoopBackOff`, 17 restarts, not ready, owned
     through a ReplicaSet name the template hash derives (DIAG-008/009/010, the ownership
     derivation);
  3. the `cache-warm-jc7dd` pod (a `Job`'s — an owner kind outside the model) and the bare
     `debug-shell` pod: the two underived-owner facts;
  4. the `abandoned-config` configmap and the `orphan-cache` claim (`Pending`, referenced by
     nothing): DIAG-011/012/013 — beside the *real* orphaned `devspace-cache-acd` secret;
  5. the `sa-token-legacy` secret of type `kubernetes.io/service-account-token`: the orphan
     rule's typed exemption, present so removing the exemption fails a test;
  6. the `switchboard-client` service, selecting exactly what `switchboard-headless` selects
     (DIAG-014);
  7. the `switchboard` statefulset's `replicas` raised from the live 1 to 2 — the negative case
     for DIAG-007 — and `status.phase: Bound` added to the two live claims (their true state;
     the trim had dropped `status`).

* **Extended for IW2.5**, when the scanner grew replicasets, jobs, cronjobs, pod disruption
  budgets and autoscalers (all five *optional* in a bundle — an older scan without them still
  validates). Each entry names what it exists to exercise:
  1. four replicasets (`coredns-ccb96694c`, `storefront-server-94f65b68f`,
     `queue-redis-66f544d5b` real, `flaky-agent-6d8f9c7b44` synthetic), so deployment pods
     derive **exactly** through their observed replicaset's `ownerReferences` — the
     `pod-template-hash` heuristic is now the old-bundle fallback only;
  2. the `cache-warm` job (completed — DIAG-019's negative, and its pod now derives instead of
     being an underived fact) and the failed `reindex-29301120` job owned by the `reindex`
     cronjob (DIAG-019 positive, the job→cronjob edge);
  3. the `reindex` (running) and `nightly-report` (suspended) cronjobs (DIAG-020);
  4. three budgets: `coredns` and `switchboard` guarding real pods (DIAG-015/016 negatives), and
     `retired-workers`, whose **two-label** selector shares one label with a live pod — only
     the AND-semantics of `matchLabels` keeps it guarding nothing (DIAG-015 positive, and the
     mutation register's case against weakening the match);
  5. three autoscalers: `switchboard` pinned min=max=2 (DIAG-017), `storefront-server` with a
     real range (the negatives), `ghost-scaler` aimed at the absent `retired-api-server`
     (DIAG-018);
  6. `storefront-server` raised to 2 replicas: the multi-replica workload *without* a budget
     (DIAG-016 positive, the `INFRA-PROP-002` exception); `coredns` raised to 2 replicas and
     the `svclb` containers given requests and limits, so the coverage and resource-bounds
     invariant candidates hold their strict majorities on this fixture.

* **Extended for IW3**, the desired-state wave. Two fixtures joined the directory and the
  observation itself gained nothing — IW3 reads what IW2.5 already collected.

Secret values were `{sha256, length}` digests before the scanner ever wrote the bundle;
this repository refuses a bundle where they are anything else (`INFRA-SECRET-001`).

The compiled IR carries exactly four unresolved references — the optional `coredns-custom`
configmap, the `lost-lookup` selector, the `retired-api` backend, and `flaky-agent`'s required
`agent-credentials` secret — as typed facts, not errors: they are true statements about the
observed cluster, and `protocol infra diagnose` turns each into a coded finding.

## `expected.yaml` — the desired state

Twenty-eight expectations covering all twelve kinds of the `infra-spec/1` vocabulary. Every kind
appears at least twice — once where this cluster satisfies it and once where it does not — so a
kind that stops working fails a test rather than quietly reporting `ok` everywhere. One kind,
`image_pinned_by_digest`, has no satisfying case here because nothing in this cluster is
digest-pinned; that positive lives in a purpose-built bundle in `crates/infra-spec/tests/`, and
`every_expectation_kind_holds_somewhere_on_the_fixture_and_fails_somewhere_on_it` names the
exception so it stays deliberate.

Five expectations are deliberately **undecidable**, one per reason the snapshot can give:

| id | verdict | why the snapshot cannot decide |
|---|---|---|
| `kube-system-replicas` | `unknown` | the `svclb` daemonset declares no replica count |
| `shop-registry` | `unknown` | `redis:7-alpine` names no registry, so which one resolves it is not observed |
| `payments-resources` | `unknown` | the namespace `payments` was never observed |
| `shop-ready-pods` | `unknown` | the bare `debug-shell` pod has an underivable controller, so no pod count in `shop` is a count |
| `retired-replicas` | `unknown` | the workload selector `app: retired` matches nothing |

The sixth reason — a bundle that never scanned a kind — cannot appear here, because this bundle
scans all seventeen. It is covered by
`a_bundle_that_never_scanned_disruption_budgets_is_undecidable_and_not_uncovered`.

The report is `11 hold, 12 gaps, 5 undecidable`, and `simulation.json` holds those bytes.

## `observation.drifted.json` — the derivation

A copy of `observation.json` with twenty mutations, each one existing to make a drift change kind
load-bearing. Nothing about the runtime layer moved: no pod was renamed, no phase changed, no
restart counter advanced — because drift is over *declared* state, and a fixture that also churned
its pods would prove nothing about that.

| # | mutation | change kind it exercises |
|---|---|---|
| 1 | `scanned_at` moved forward; the context is unchanged | none — provenance is outside the digest, and a different context is the one refusal |
| 2 | the `payments` namespace appears | `added` (namespace) |
| 3 | the `checkout-api` deployment appears, digest-pinned, with probes and bounds | `added` (workload) |
| 4 | the `flaky-agent` deployment is gone, with its required dangling secret | `removed` (workload), and **no** `reference_healed` |
| 5 | `switchboard` goes from 2 replicas to 3 | `replicas_changed` |
| 6 | `storefront-server`'s image tag moves | `image_changed` |
| 7 | its `BC_LOG_LEVEL` moves from `debug` to `info` | `environment_changed` |
| 8 | a `metrics-sidecar` container joins it | `container_added` |
| 9 | `svclb` loses its `lb-tcp-443` container | `container_removed` |
| 10 | `queue-redis` gains requests, limits and a readiness probe | `resources_changed`, `probes_changed` |
| 11 | it also gains a required `configMapKeyRef` to the unobserved `redis-tuning` | `reference_broke`, `environment_changed` |
| 12 | `coredns` gains a `managed-by` label | `workload_field_changed` (labels) |
| 13 | the `lost-lookup` service is gone, with its dangling selector | `removed` (service), and **no** `reference_healed` |
| 14 | a `checkout-api` service appears | `added` (service) |
| 15 | `queue-redis-master`'s port moves from 6379 to 6380 | `service_field_changed` (ports) |
| 16 | the `edge` ingress routes `/legacy` to `checkout-api` instead of the unobserved `retired-api` | `ingress_routing_changed`, `reference_healed` |
| 17 | `storefront-env`'s `REGION` value changes | `config_content_changed` (configmap) |
| 18 | a `checkout-config` configmap appears | `added` (configmap) |
| 19 | the `storefront-server` secret gains a `session-key` | `config_content_changed` (secret) |
| 20 | `orphan-cache` moves from `Pending` to `Bound` | `claim_phase_changed` |

Twenty-two changes in total, covering every one of the sixteen change kinds this build can report.
Mutations 4 and 13 are there for the rule that a reference event is only reported for a holder
present in *both* snapshots: the removal already says what happened, and reporting both would count
one event twice.
