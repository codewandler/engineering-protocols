# k3d-dev-cluster — the example observation

A small, deterministic `infra-observation/1` bundle and the `infra-ir/1` document it compiles to.

| file | role |
|---|---|
| `observation.json` | input: a trimmed observation, derived from a real scan (see below) |
| `cluster.ir.json` | output: what `protocol infra compile` produces from it — committed, drift-checked by `cargo xtask infra --check` |

Never edit `cluster.ir.json` by hand; regenerate it with `cargo xtask infra`. `observation.json`
*is* hand-maintained — it is a fixture, not an output.

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

Secret values were `{sha256, length}` digests before the scanner ever wrote the bundle;
this repository refuses a bundle where they are anything else (`INFRA-SECRET-001`).

The compiled IR carries exactly four unresolved references — the optional `coredns-custom`
configmap, the `lost-lookup` selector, the `retired-api` backend, and `flaky-agent`'s required
`agent-credentials` secret — as typed facts, not errors: they are true statements about the
observed cluster, and `protocol infra diagnose` turns each into a coded finding.
