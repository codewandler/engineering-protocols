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
a plain-string secret value is refused without echoing it.

## The pipeline

```console
$ protocol infra validate  --path observation.json      # is the bundle well-formed?
$ protocol infra compile   --path observation.json      # → content-addressed infra-ir/1
$ protocol infra inspect   --path cluster.ir.json --properties
$ protocol infra graph     --path cluster.ir.json --format mermaid
$ protocol infra diagnose  --path cluster.ir.json --candidates --directions
$ protocol infra view      --path cluster.ir.json       # self-contained HTML page
$ protocol infra simulate  --spec expected.yaml --path cluster.ir.json
$ protocol infra diff      --from before.ir.json --to after.ir.json
$ protocol infra project   --spec expected.yaml --path cluster.ir.json --out patches/
```

The observation model covers seventeen Kubernetes kinds. Compilation produces a content-addressed
IR in which a dangling reference is a typed unresolved fact, not an error — observed infrastructure
is allowed to be wrong. A worked example ships in the repository: `examples/k3d-dev-cluster/` is a
trimmed, reviewed observation from a real k3d scan, drift-checked in the gate.

## Diagnose: what is wrong, under stable codes

`diagnose` runs twenty rules (`INFRA-DIAG-001`…`020`), each finding coded, severity-registered and
carrying named evidence. Findings never fail the run — a report about a cluster is not a gate.
`--candidates` adds uniformity candidates (`INFRA-PROP-001`…`003`, with exceptions carried as
evidence), and `--directions` groups findings into a severity-ranked summary by shared root cause.

## Simulate: expected against observed, three-valued

You declare a desired state as an `infra-spec/1` document. Every expectation comes back one of
three verdicts:

| Verdict | Meaning | Example detail line |
|---|---|---|
| `ok` | observed, and it holds | — |
| `gap` | observed, and contradicted — the line says what would have to change | *`storefront-server` declares 2 replicas and no disruption budget covers it* |
| `unk` | the snapshot cannot decide, with the reason | *namespace `payments` was not observed*; *the bundle did not scan `poddisruptionbudgets`* |

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

`simulate` exits 0 whatever the verdicts say; exit 1 means the *input* could not be simulated. The
example's `expected.yaml` holds 28 expectations reaching all three verdicts on the example cluster:
11 hold, 12 gaps, 5 undecidable.

## Diff: what moved between two scans

`infra diff` compares two IRs of one cluster over **declared** state — sixteen typed change kinds:
objects added and removed, replicas, images, containers, resource bounds, probes, environment,
workload and service fields, ingress routing, configuration content (naming which keys moved, never
what they hold), claim phases, references broken or healed. Pods are deliberately absent — they are
renamed on every rollout. It refuses one thing: two snapshots scanned in different kubeconfig
contexts.

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

Two files make the tree honest in review: `OBLIGATIONS.md`, because a tree that closed nine gaps
and quietly left sixteen decisions unmade reads exactly like a tree that closed everything, and
`SUMMARY.md`, carrying the counts and the digests of both inputs. On the example cluster:
twenty-three gaps in, nine come back as changes, sixteen as decisions nobody can take for you —
seven committed files, drift-checked in the gate.

---

**Sources.** `CHANGELOG.md` § *0.7.1-infra-waves-1-4*; `AGENTS.md` § *Current state* (infra);
`examples/k3d-dev-cluster/`; `crates/infra-domain/` through `crates/infra-project/`.
