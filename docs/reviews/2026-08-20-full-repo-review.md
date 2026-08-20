# Full repository review — 2026-08-20

**Verdict: healthy at HEAD except for one gate breach (schema drift, CI red on `main` twice);
one real defect in committed wave-1 code (over-eager cycle refusal), surfaced by the in-flight
wave-2 tests; the in-flight wave-2 code itself is high quality.**

Reviewed: commit `95e210f` plus the uncommitted working tree, snapshotted 2026-08-20 ~04:00–04:40
CEST. **Caveat:** another interactive session (`engineering-protocols-9d`) was actively editing the
wave-2 files during this review — `resolve.rs` changed mid-review (mtime 04:37:51 vs. review read
~04:00). Working-tree findings below are a snapshot of work in flight, not a settled state. Nothing
in the working tree was modified by this review.

## 1. State at a glance

| Check | HEAD `95e210f` | Working tree (snapshot ~04:26) |
|---|---|---|
| `cargo fmt --check` | exit 0 | **exit 1** — 3 files (spec.rs, types.rs, ess-compiler test) |
| `cargo clippy -D warnings` | exit 0 | **fail** — `needless_borrow` in `ess-compiler` billing test; at 04:37 the crate did not compile mid-edit |
| `cargo test --workspace` | 642 passed, 0 failed | 447+ passed, **1 failed** (see F2) |
| `cargo xtask schema --check` | **exit 1** — `ess.schema.json` drift | exit 0 (tree contains the regenerated schema) |
| CI on `main` | run 32322480515: Gate ✓, **Schemas ✗**; run 32317533922 (`4450450`): **Format ✗, Schemas ✗** | n/a |

HEAD evidence from a throwaway worktree at `95e210f`; working-tree evidence from `task check` and
individual gate steps in this tree. The 642 at HEAD matches the README status table's claim.

## 2. Findings

### F1 — HEAD does not pass its own gate: generated schema drift (severity: high, committed)

`cargo xtask schema --check` fails at `95e210f`: *"1 file(s) differ from the Rust types:
ess.schema.json"*. CI's dedicated "Schemas up to date" job is red for the last **two** pushes to
`main` (runs 32317533922 and 32322480515); `4450450` additionally failed Format. This breaches
AGENTS.md §Gate ("Land nothing that does not pass it") and invariant 1 (schemas are generated,
kept in step).

The working tree already contains a regenerated `schemas/generated/ess.schema.json` that passes the
check — so the fix will land with wave 2. What is missing is the discipline, not the file:
**`main` was pushed twice without a green gate.** Repair: run `task check` before every push (CI
mirrors it step for step, per the comment at the top of `.github/workflows/ci.yml`); if wave-2 lands
soon, land it green and `main` recovers; if not, a one-commit `cargo xtask schema` fix on `main`
restores CI independently of wave 2.

### F2 — Two implementations of one rule disagree: cycle refusal vs. inhabitability (severity: high, one side committed)

The transitive required-reference cycle check exists **twice**:

* Committed wave 1: `check_cycles` / `ValidationCode::SelfReference` —
  `crates/ess-domain/src/system.rs:522`. Cycle detection over the "required dependency" graph
  (`required_dependencies`, system.rs:502): every `Named` leaf of a newtype, every struct field,
  and **every union variant** counts as required.
* In-flight wave 2: the compiler's inhabitability pass — `Resolver::uninhabitable` /
  `ESS-TYPE-002 UNINHABITABLE_TYPE`, `crates/ess-compiler/src/resolve.rs` (`inhabitable` /
  `reachable` fixpoint). Its doc states the correct semantics: *"a cycle is not the defect"* —
  a union with one terminating variant is fine; only a type no value can inhabit is refused.

They genuinely disagree, not merely overlap. `Expr = union { leaf: Integer, pair: Pair }` with
`Pair = struct { left: Expr, right: Expr }` **is inhabitable** (`leaf`), the compiler accepts it
(asserted in `crates/ess-compiler/tests/rejections.rs:303`,
`a_union_with_one_terminating_variant_is_accepted_even_though_another_recurses` — passes because the
fixture bypasses `assemble`), and the committed domain check **refuses it** as `SelfReference`
(`Expr → Pair → Expr` is a cycle of names it treats as required). So committed wave-1 code refuses a
legitimate specification.

This is also the cause of the one failing test:
`a_diagnostic_from_the_whole_pipeline_carries_the_line_the_declaration_is_written_on`
(`crates/ess-compiler/tests/billing.rs:298`) expects `Specification::assemble` to accept mutual
struct recursion so the compiler can refuse it as `UNINHABITABLE_TYPE`; `assemble` refuses it first
with `self_reference`. The compiler's own ownership register (`resolve.rs`, the design-§20 table)
claims the type-graph cycle rule for the compiler — the domain crate has not ceded it.

Recommendation (a design decision; the in-flight session may already be on it): **one
implementation, and inhabitability is the right semantics.** Either

1. delete `check_cycles` from `ess-domain` and let the compiler own the rule (cost:
   `protocol ess validate` alone no longer refuses uninhabitable types — acceptable if `compile`
   becomes the verb harnesses run), or
2. move the inhabitability fixpoint into `ess-domain` (so `validate` still refuses, with the
   *correct* rule) and have the compiler defer to it like it defers every other domain-owned rule.

Either way, wave-1's `SelfReference` tests (system.rs:1257 among them) need updating, and the union
false positive is a wave-1 defect worth a CHANGELOG `### Fixed` line regardless of which side wins.

### F3 — Stale comment tied to F2 (severity: low, in-flight file)

`crates/ess-domain/src/binding.rs:859`: `WRAPPER_LIMIT`'s justification says §20's cycle refusal
"is not implemented yet". It is implemented (system.rs:522, committed). The comment becomes true
again only if F2 is resolved by deleting `check_cycles` — settle it together with F2. The bounded
walk itself is fine as defence in depth.

### F4 — Working-tree gate failures are all wave-2 in flight (severity: informational)

The fmt (3 files), clippy (`needless_borrow` in the ess-compiler billing test) and test (F2)
failures all live in the uncommitted wave-2 work, which was being edited during the review. Nothing
committed regressed except F1. Expected to churn; not itemised further.

### F5 — Redundant duplicate-mapping check (severity: note, no action)

`RawSpecFile::parse` (spec.rs) now refuses any duplicated YAML mapping key format-wide via the
two-stage parse, which makes `MappingTable`'s duplicate-target detection
(binding.rs:200) unreachable through the file pipeline. It still fires for a directly parsed
`RawBindingSpec` and both paths are tested — recording it so the next reader doesn't rediscover it
as a bug. Same pattern-class as F2, but here both implementations agree.

### F6 — Documentation tree (severity: low)

`codegate --language markdown` over the repo: rating **B, 71/100**, 42 documents, 51 findings, 0
violations. Substance: nearly all findings are benign (`CHANGELOG.md` duplicate anchors are
inherent to keep-a-changelog; 44 "large section" hits are the design docs being design docs).
`docs/design/consolidated-design-v0.2.md` uses 108 H1s (`#` per §) — deliberate style, but it is
what drags the maintainability score; not worth changing. One caveat on the tool run: codegate
scanned `target/` (a vendored font licence produced the `missing_h1` hit) — exclude build output if
this becomes a tracked score.

## 3. What holds up well

* **The 16 AGENTS.md invariants verify mechanically.** No `unsafe` (grep, 0 hits;
  `unsafe_code = "forbid"` in Cargo.toml), no `HashMap`/`HashSet` in any crate's src (grep, 0
  files), no `SystemTime::now`/RNG in domain crates (grep, 0 real hits), `missing_docs` +
  clippy-pedantic enforced workspace-wide.
* **CI is honest and did its job.** The gate mirrors `task check` step for step; schema drift has
  its own job precisely so it reads as schema drift — and it caught F1 twice. The failure is that
  red `main` was pushed over, not that anything was missed.
* **The wave-2 compiler design is strong.** Unresolved references made unrepresentable (handles
  mintable only by `compile`, total lookups via the `handles!` macro); diagnostic-code collisions
  refused at compile time (`const _: () = assert!(distinct(ALL))`, resolve.rs:191 at snapshot);
  cascade suppression with an explicit three-valued `Found`; the line-location heuristic refuses to
  guess (a needle located only when unique across all files); determinism asserted by
  byte-comparing two independent compilations rather than claimed in prose.
* **Test discipline is real.** Tests assert `ValidationCode`s / diagnostic codes, never message
  text; accepted and refused cases asserted in pairs; the normative billing example is compiled
  from `examples/billing/` on disk rather than from an inlined copy, closing the drift the design
  review (F7) called out.
* **Process artifacts are maintained with the work**: CHANGELOG `[Unreleased]` already carries the
  wave-2 entries in the user's voice; tags exist per wave with descriptive messages
  (`git tag -n1`); README status table matches HEAD reality (642 tests verified in a clean
  worktree).

## 4. Recommended actions

| # | Action | Owner | Default if unanswered |
|---|---|---|---|
| 1 | Restore green `main`: land wave 2 gated, or push a lone `cargo xtask schema` commit now | in-flight session / Timo | rides along with wave 2 |
| 2 | Decide F2's rule owner (delete `check_cycles` vs. move inhabitability into `ess-domain`) | Timo | option 1 — compiler owns it, per its own §20 register |
| 3 | Fix the wave-1 union false positive and add a `### Fixed` CHANGELOG line | with F2 | done as part of F2 |
| 4 | Before pushing: `task check` locally — twice-red `main` is the only process failure this review found | everyone | — |

## 5. Resolution — 2026-08-20, after wave 2 landed

Recorded against the snapshot above, not a second review. Evidence is a command's output or a
`file:line`; nothing here is inferred from the fact that the work happened.

| # | Severity | State | Evidence |
|---|---|---|---|
| F1 | high | **resolved** | `cargo xtask schema --check` → "schemas are up to date". CI run on `ea68f18` (the wave-2 commit): `conclusion: success` — the first green `main` since `e30f8f9`. |
| F2 | high | **resolved, option 2** | `check_cycles` is gone; `check_inhabitation` (`crates/ess-domain/src/system.rs`) owns the rule, so `protocol ess validate` still refuses — with the correct semantics. `crates/ess-domain/src/system.rs:1719` asserts a union with one terminating variant is *not* a `SelfReference`. CHANGELOG `### Fixed` carries the user-facing line ("A legitimate expression tree was refused"). |
| F3 | low | **resolved** | `crates/ess-domain/src/binding.rs` — `WRAPPER_LIMIT`'s justification now cites `check_inhabitation` as implemented, and keeps the bound as defence in depth. |
| F4 | info | **moot** | The wave-2 working tree it described is committed and CI-green. |
| F5 | note | **open by design** | No action was asked for. The unreachable-through-the-file-pipeline duplicate-target check in `crates/ess-domain/src/binding.rs:200` stays; both paths remain tested. |
| F6 | low | **open, not worth doing** | Documentation score unchanged. The one actionable part — codegate scanning `target/` — only matters if the score becomes tracked, and it is not. |

Action 2 was decided the other way from the review's default: the inhabitability fixpoint moved into
`ess-domain` rather than the compiler keeping the rule alone. The reason is the one the review named
as option 2's advantage — `protocol ess validate` is a verb a harness runs, and a validate that
accepts a type no value can inhabit hands the refusal to a later stage that may never run.

Action 4 (`task check` before every push) is what caught the gate breach that blocked ESS wave 3
from landing: a `-D warnings` dead-code error in `ess-gen`, invisible to any check narrower than the
full gate.
