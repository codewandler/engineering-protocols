---
title: Check infrastructure against a desired state
sidebar_position: 8
description: Compile a cluster observation into a typed IR, diagnose it, evaluate it against a declared desired state, and project the gaps back as a reviewable patch tree.
---

# Check infrastructure against a desired state

The infrastructure half applies the ESS pattern to observed Kubernetes clusters. The boundary is
strict: **nothing in this project reaches a cluster or holds a credential.** An external scanner
(`infra-scout`, its own repository) writes an `infra-observation/1` bundle to a file; everything
here reads that file, refuses it or compiles it. Secrets appear only as digests — a bundle carrying
a plain-string secret value is refused without echoing it, one of eleven `INFRA-*` refusal codes
the observation model states.

A worked example ships in the repository, and every command on this page runs against it.
`examples/k3d-dev-cluster/` holds three hand-maintained inputs — `observation.json`, the same
cluster a working day later as `observation.drifted.json`, and a desired state in `expected.yaml` —
beside four committed outputs that `cargo xtask infra --check` re-derives and compares.

## The pipeline

```console
$ protocol infra validate  --path examples/k3d-dev-cluster/observation.json
$ protocol infra compile   --path examples/k3d-dev-cluster/observation.json --out cluster.ir.json
$ protocol infra inspect   --path examples/k3d-dev-cluster/cluster.ir.json --properties
$ protocol infra graph     --path examples/k3d-dev-cluster/cluster.ir.json --format mermaid
$ protocol infra diagnose  --path examples/k3d-dev-cluster/cluster.ir.json --candidates --directions
$ protocol infra view      --path examples/k3d-dev-cluster/cluster.ir.json
$ protocol infra simulate  --spec examples/k3d-dev-cluster/expected.yaml \
                           --path examples/k3d-dev-cluster/cluster.ir.json
$ protocol infra diff      --from examples/k3d-dev-cluster/observation.json \
                           --to   examples/k3d-dev-cluster/observation.drifted.json
$ protocol infra project   --spec examples/k3d-dev-cluster/expected.yaml \
                           --path examples/k3d-dev-cluster/cluster.ir.json --out patches/
```

Every verb after `validate` reads a bundle *or* a compiled IR, so the `compile` step is a
convenience rather than a precondition. Given a persisted IR, `inspect` recomputes the digest over
the document's model and refuses a mismatch: a content-addressed document whose digest does not
match its content is a document someone edited.

The observation model covers seventeen Kubernetes kinds — namespaces, nodes, deployments,
statefulsets, daemonsets, replicasets, pods, jobs, cronjobs, services, ingresses, configmaps,
secrets, service accounts, persistent volume claims, disruption budgets and horizontal pod
autoscalers. Compilation produces a content-addressed IR in which a dangling reference is a typed
unresolved fact, not an error — observed infrastructure is allowed to be wrong:

```console
$ protocol infra inspect --path examples/k3d-dev-cluster/cluster.ir.json
k3d-dev-cluster (infra-ir/1) — 2 namespace(s), 1 node(s), 6 workload(s), 6 service(s), 1 ingress(es), 3 configmap(s), 3 secret(s), 4 service account(s), 3 claim(s), 9 pod(s)
digest 9ed0e8608fd69c43c3b0405a7a5fd599fad61a6246b42e2cdd4cffd1e29c8e75
4 unresolved reference(s)
```

`--properties` adds each workload's replicas, images with tag and digest, and resource envelope.
`graph` renders the typed dependency graph as `mermaid` (the configuration topology, grouped by
namespace), `json` (the canonical document — every node, edge, site and ownership fact; 50 nodes
and 39 edges here) or `html` (one self-contained page, components badge-coloured by their worst
finding). `view` writes that page to a file and opens it in `$BROWSER` or `xdg-open` — the one verb
in this CLI that spawns another program, because opening the page is its whole purpose.

## Diagnose: what is wrong, under stable codes

`diagnose` runs twenty rules (`INFRA-DIAG-001`…`020`), each finding coded, severity-registered and
carrying named evidence:

```console
$ protocol infra diagnose --path examples/k3d-dev-cluster/cluster.ir.json --min-severity error
INFRA-DIAG-002 error ingresses/shop/edge rules[0].paths[1]: the required service `retired-api` was not observed
INFRA-DIAG-002 error workloads/shop/deployment/flaky-agent containers[agent].env[AGENT_TOKEN]: the required secret `agent-credentials` was not observed
INFRA-DIAG-008 error pods/shop/flaky-agent-6d8f9c7b44-x1q2z containers[agent]: container `agent` is stuck waiting: CrashLoopBackOff
INFRA-DIAG-018 error horizontal_pod_autoscalers/shop/ghost-scaler scaleTargetRef: the target deployment `retired-api-server` was not observed; the autoscaler manages nothing
4 of 37 finding(s) at or above error
4 error(s), 22 warning(s), 11 info(s)
```

Findings never fail the run — a report about a cluster is not a gate, so exit 0 holds whatever the
counts say, and exit 1 means the *input* was refused. `--min-severity` narrows what is shown and
never what is counted: the summary line above still totals all thirty-seven.

`--candidates` adds uniformity candidates — `INFRA-PROP-001`…`003`, each with its exceptions
attached as evidence rather than hidden:

```console
$ protocol infra diagnose --path examples/k3d-dev-cluster/cluster.ir.json --candidates | grep INFRA-PROP-002 -A1
INFRA-PROP-002 every multi-replica workload has a disruption budget — holds for 2 of 3; except:
  workloads/shop/deployment/storefront-server — 2 replicas, no covering budget
```

`--directions` groups findings by shared root cause and ranks them by severity, so a wall of
warnings reads as a handful of causes. Both formats are `text` and `json`.

## Simulate: expected against observed, three-valued

You declare a desired state as an `infra-spec/1` document. Every expectation comes back one of
three verdicts:

| Verdict | Meaning | Example detail line, from the worked example |
|---|---|---|
| `ok` | observed, and it holds | — |
| `gap` | observed, and contradicted — the line says what would have to change | *`workloads/shop/deployment/flaky-agent` — declares 1 replicas, wanted [2, 4]* |
| `unk` | the snapshot cannot decide, with the reason | *namespace `payments` was not observed*; *`workloads/shop/deployment/queue-redis` declares no `containers[redis].image registry`* |

The three-valued discipline is the same as the protocol's: `unknown` is never quietly a failure,
and an expectation cannot pass by selecting nothing — a scope matching no workload is `unk`, not
`ok`. An expectation with one contradicted subject and one undecidable subject is a `gap`.

Twelve expectation kinds exist, kept small and decidable: a workload exists; replicas within a
range; resources declared; probes declared; images from a registry allowlist, not `latest`, pinned
by digest; a disruption budget covers every multi-replica workload; a service selector matches a
pod; configmap and secret references resolve; workloads only in listed namespaces; and a labelled
predicate over eighteen `workload.*` facts using the protocol's own predicate language. Scopes are
the whole cluster, one namespace, or a label set. No expectation reads a clock — there is no way to
write a duration — so the same specification and snapshot always produce the same report, which is
what makes a report committable and reviewable as a diff.

```console
$ protocol infra simulate --spec examples/k3d-dev-cluster/expected.yaml \
      --path examples/k3d-dev-cluster/cluster.ir.json | tail -4
unk retired-replicas — replicas within [1, 1] [workloads matching app=retired, 0 subjects]
      workloads matching app=retired — the scope selects no subject in this snapshot

28 expectations: 11 hold, 12 gaps, 5 undecidable
```

`simulate` exits 0 whatever the verdicts say; exit 1 means the *input* could not be simulated. The
example's `expected.yaml` is written to reach all three verdicts on the example cluster, which is
what makes it usable as a fixture: a specification everything passes tests nothing.

## Diff: what moved between two scans

`infra diff` compares two snapshots of one cluster over **declared** state — sixteen typed change
kinds: objects added and removed, replicas, images, containers added and removed, resource bounds,
probes, environment, workload and service fields, ingress routing, configuration content (naming
which keys moved, never what they hold), claim phases, references broken and references healed.
Pods are deliberately absent — they are renamed on every rollout, and a report listing a thousand
of them is one nobody reads. It refuses one thing: two snapshots scanned in different kubeconfig
contexts.

```console
$ protocol infra diff --from examples/k3d-dev-cluster/observation.json \
      --to examples/k3d-dev-cluster/observation.drifted.json | head -6
k3d-dev-cluster: 9ed0e8608fd6 -> af6c34c7f640

  namespace payments added
  workload shop/deployment/checkout-api added
  service shop/checkout-api added
  configmap shop/checkout-config added
```

Twenty-two changes in that pair, and the committed `drift.json` beside the inputs is the same
report as a document, re-derived in the gate.

## Project: a gap becomes a diff a person can review

`infra project` writes the patch tree that would close the gaps — into a directory you review, edit
and apply with your own hands. Nothing is applied; the output is files.

The dividing line: **every value in a generated file came from the gap or from you.**

* A replica count outside `[2, 4]` has one nearest acceptable number — written as a patch.
* An image tagged `latest` has no mechanically-nearest replacement — you get an obligation naming
  the decision, not a patch containing a version somebody's generator picked.
* Resources and probes sit on both sides: state values once as a `remedy:` on the expectation and
  they are written; state nothing and they are owed. A remedy never changes a verdict — nothing
  evaluates it — and malformed or misplaced remedies are refused (`INFRA-SPEC-009`/`010`).

Patches are against the object that was observed, not regenerated manifests (a regenerated manifest
silently drops every field the observation model does not keep); container-level changes are
strategic merge patches and say so in the filename. The projection also **closes what it opens**:
raising replicas to two can break "a disruption budget covers every multi-replica workload", so it
simulates its own changes, marks the induced gap, and writes the budget in the same tree — the test
suite applies the emitted files, recompiles and re-simulates to prove that no unrelated verdict
moved.

```console
$ protocol infra project --spec examples/k3d-dev-cluster/expected.yaml \
      --path examples/k3d-dev-cluster/cluster.ir.json | tail -2
25 gap(s): 9 generated, 16 owed, 0 refused; 2 patch file(s), 3 new object(s)
expectations 11 hold -> 15 hold, 12 gaps -> 8 gaps
```

Twenty-three of those gaps are in the snapshot; the other two are the ones this tree's own changes
would open, and it closes them in the same pass. Nine come back as changes across five files.
Sixteen come back as decisions nobody can take for you: which workload should carry the label a
selector names, what a missing secret contains, where a workload in the wrong namespace should be
recreated.

Two more files make the tree honest in review. `OBLIGATIONS.md`, because a tree that closed nine
gaps and quietly left sixteen decisions unmade reads exactly like a tree that closed everything;
and `SUMMARY.md`, carrying the counts, the digests of both inputs, the `kubectl` line for each
patch, and the before/after verdict counts. That is seven committed files under
`examples/k3d-dev-cluster/projection/`, drift-checked in the gate along with an orphan scan — a
patch file for an object nothing patches any more would sit there looking like a proposal somebody
still stands behind.

---

**Sources.** `CHANGELOG.md` § *0.7.1-infra-waves-1-4*; `AGENTS.md` § *Current state* (infra);
`examples/k3d-dev-cluster/README.md`, which documents how the bundle was reduced from a live scan;
and the five crates behind the verbs — `infra-domain`, `infra-compiler`, `infra-analyze`,
`infra-spec`, `infra-project`.
