# Infra wave 1 — observe and persist

Status: **delivered**. This page accepts the scope it describes; the ESS pattern
(raw→validated, compiler-minted handles, canonical bytes, drift-checked committed output) is
instantiated a second time, on infrastructure.

## Goal

A CLI scans a Kubernetes cluster into an observation bundle; this repository turns the bundle
into a validated, deterministic, content-addressed infrastructure IR — the foundation for the
graph and diagnosis (IW2), desired-state diff and simulation (IW3), and manifest projection
(IW4).

## The boundary, first

**The scanner is external.** `infra-scout` (its own repository) holds the kubeconfig, shells out
to `kubectl`, and writes `infra-observation/1` bundles; *it* replaces every secret value with
`{sha256, length}` before anything touches disk. Nothing in this workspace reaches a network,
holds a credential, or links a Kubernetes client — the bundle arrives as a file, and the two
new crates (`infra-domain`, `infra-compiler`) plus the `protocol infra` verbs are pure
functions over it. The gate stays offline.

## Decisions taken

1. **External actor.** Scanner and toolchain are separate repositories with a file-format
   contract between them, so credentials and API skew stay on the side that already has them.
2. **Kubernetes subset v1.** Twelve kinds, modelled only as far as IW2–IW4 ask: identity,
   labels, selectors, workload pod-template essentials (containers with env/envFrom/volume
   mounts/probes/resources, volumes, service account, replicas), service type/selector/ports,
   ingress rules to backend service references, configmap and secret *keys with value digests*,
   service accounts, claims (class/size/access), pod runtime essentials (phase, readiness,
   restarts, node, owner), node capacity and info. Extending the subset is a format-shaped
   change, not a patch.
3. **Noise exclusions, by class.** Write-tracking bookkeeping, timestamps, assigned runtime
   addresses, status beyond the essentials, rollout mechanics, and configuration *values* are
   absent from the validated model — the IR is a function of semantic cluster state, not of API
   bookkeeping. `infra-domain`'s crate documentation carries the register.
4. **Dangling references are facts, not refusals.** A selector matching nothing, an env var
   reading a configmap that is not there: real clusters legitimately contain these, and an IR
   that refused a degraded cluster would be useless exactly when IW2's diagnosis needs it.
   Where a reference *does* resolve, the ESS property holds unchanged — a handle is mintable
   only by the compiler and its lookup is total. What does not resolve is a typed
   `UnresolvedReference`, carried openly and sorted.
5. **No wall-clock predicates.** `scanned_at`, `context` and `scout_version` live in provenance,
   outside the digested bytes: two scans of an unchanged cluster produce the same digest, which
   is the entire point of hashing. The digest is the full SHA-256 (64 hex — the width gap
   register D-4 settled) over the model's canonical JSON: compact, keys sorted, recomputable by
   any reader of the persisted document — `protocol infra inspect` recomputes it and refuses a
   tampered file.
6. **The secret hard rule, twice.** The scanner sanitizes before writing, and `infra-domain`
   refuses a bundle whose secret values are anything but `{sha256, length}` digests
   (`INFRA-SECRET-001`, tested against a committed deliberately-unsanitized fixture) — so a
   secret value cannot enter the IR even through a bundle the scanner never touched, and the
   refusal itself never echoes the value. Configmap values get the same treatment one step
   later: hashed at validation, keys and digests survive, content does not.

## The refusal catalogue

| code | refuses |
|---|---|
| `INFRA-BUNDLE-001` | a format string other than `infra-observation/1` |
| `INFRA-BUNDLE-002` | an absent kind — "not scanned" is not "none exist" |
| `INFRA-OBJECT-001` | an item that does not read as its kind's shape |
| `INFRA-OBJECT-002` | an object without name, namespace or uid |
| `INFRA-OBJECT-003` | two same-kind objects sharing namespace and name |
| `INFRA-SECRET-001` | a secret value that is a plain string — an unsanitized bundle |
| `INFRA-SECRET-002` | a secret value that is an object but not a well-formed digest |
| `INFRA-SELECTOR-001` | a non-string value in a labels or selector map |
| `INFRA-WORKLOAD-001` | a workload whose template runs no containers |
| `INFRA-WORKLOAD-002` | a container without name or image |
| `INFRA-INGRESS-001` | an ingress path whose backend names no service |

Validation accumulates: one run reports every refusal (invariant 3), each with a stable code
(invariant 4) and a dotted location.

## Delivered

* `crates/infra-domain` — raw types (permissive, tolerate unknown fields) → validated
  observation via `TryFrom`, accumulating `ValidationErrors`; validated types never
  `Deserialize` (invariant 2, enforced by a source scan with an inverse assertion).
* `crates/infra-compiler` — `InfraIr`: `BTreeMap`s keyed by identity
  (`namespace/name`; workloads `namespace/kind/name`), compiler-minted handles with total
  lookups, unresolved facts, provenance, digest. Compilation is total.
* `protocol infra validate | compile | inspect` — clap derive, `--format text|json|yaml`,
  exit 1 on refusal, every problem in one run; `compile --out` persists the IR document;
  `inspect` reads either format and verifies a persisted document's digest against its content.
* `examples/k3d-dev-cluster/` — the trimmed observation (derived from a real k3d scan; its
  README documents the derivation and the three synthesized objects) and the committed IR,
  drift-checked by `cargo xtask infra --check` in the Taskfile (`infra-check`, gate step 8 of 9)
  and its own CI job.
* Refusal fixtures under `crates/infra-domain/tests/fixtures/`, exercised at both the domain
  layer and through the real binary.

## Acceptance (all held)

* The full 1186-pod k3d bundle validates and compiles in under a second; the compiled document
  round-trips through `inspect` with the digest verified.
* Compiling twice is byte-identical; reordering the bundle's `kinds` and every item list
  compiles to the identical IR; editing `scanned_at`/`context`/`scout_version` moves provenance
  and not the digest; a semantic edit moves the digest.
* The unsanitized fixture is refused with `INFRA-SECRET-001` per plain value, and no refusal
  echoes a value.
* The load-bearing guards were mutation-verified (mutation applied, watched fail, reverted):
  the unsanitized-secret refusal (plain-string branch removed → fixture accepted → both tests
  fail naming the code), digest stability (provenance hashed into the digest → stability test
  fails), order-insensitivity (`facts.sort()` removed → reorder test fails on differing bytes).

## What the next waves are

* **IW2 — graph and diagnosis:** a typed dependency graph over the IR (ownership chains,
  selector edges, config edges) and diagnoses over it — starting from the unresolved facts and
  pod runtime state the IR already carries. **Delivered** as `protocol infra graph|diagnose`;
  see [`infra-wave-2-analyze.md`](infra-wave-2-analyze.md).
* **IW3 — desired state:** a declared target model, semantic diff observed↔desired, and
  simulation of a change before anything applies it.
* **IW4 — projection:** manifests generated *from* the model, closing the loop the ESS side
  already closed for code: the cluster's description becomes the source, not the residue.
