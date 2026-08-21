# ESS wave 7 — the loop closed over generated code

> **In progress. Scheduled 2026-08-20 by operator instruction, and re-scoped 2026-08-21 by the
> same authority: W7.1 and W7.2 run; W7.3 (the Go emitter) and W7.4 (obligations as artifacts) are
> deferred by operator decision and stay on the roadmap unscheduled.** Design:
> [`ess-semantic-diff-impact-evolution-design-v0.1.md`](../design/ess-semantic-diff-impact-evolution-design-v0.1.md)
> §33, which was blocked on `contract_digest` existing in no code. W7.1 makes it exist. Where that
> section's verdict vocabulary contradicts gate G19's polarity — its `still valid` and
> `unchanged` answers — G19 wins, exactly as it did in wave 5.

**Goal: a generated artifact knows which part of the model it derives from, and `ess impact`
narrows "the specification moved, everything is owed" to the artifacts whose slice moved — without
ever saying an artifact still stands.**

## W7.1 — the diff learns about generated artifacts. Delivered.

`contract_digest` became code. Every generated artifact — the 36 projections under `generated/`,
each committed conformance suite, each synthesised workspace — now carries, beside the whole-model
`source_digest`, the digest of the **model slice** it derives from, stamped through the one
provenance mechanism every generator already used (`ess_gen::Provenance`).

### The slice rule, and why it leans big

An artifact's slice is its seed constructs closed over everything they rest on, by the same
dependency graph `ess impact` walks — one graph, one walk, moved from `ess-diff` down into
`ess-compiler` (`graph`, `refs`) so the crate that stamps a digest and the crate that asks "did
this slice move" cannot drift apart. The membership rule resolves every doubt by including more:

| rule | why |
|---|---|
| sub-constructs travel with their parents — a command brings its outcomes, an entity its moves | an artifact derived from a command was derived from its branches; omitting them claims "still current" past a change to an error an outcome refuses with |
| the system header, naming, conversions and workloads are in **every** slice | none of them has an `EssSemanticRef` a slice could name, so a change there cannot be attributed — and the only digest that does not lie about an unattributable change is one that moves for every artifact |
| documents that read by inversion seed wide — `OpenAPI` takes every actor and binding, `AsyncAPI` every binding, command and component, a domain page its whole context plus the bindings and components | "which actors are relevant to this document" is itself an answer that changes; an actor granted its first accepted command tomorrow must move the digest today |
| the suite, the synthesised workspaces, the indexes and the system-wide docs pages are whole-model | legitimately: each renders or obliges something for every construct |

The asymmetry that justifies the lean: a too-big slice costs a regeneration nobody needed; a
too-small one costs a false "still current". Those are not comparable errors.

### The impact answer

`protocol ess impact --from A --to B [--suite S] [--generated DIR]` now writes `ess-impact/2`:
`--suite` became optional, the document gained an `artifacts` section, and the churn carries
`generated_artifacts_total` / `generated_artifacts_owed`. The artifact answer has wave 5's exact
polarity, enforced by the same shapes: `ArtifactAnswer` is `Whole` or `Narrowed` and nothing else,
an artifact absent from the answer was **not reached** — never "still current" — and everything the
analysis cannot follow is owed, stated as such:

* a change to the specification itself (no construct to seed a closure at) owes every artifact;
* a move in a family the delta does not compare owes every artifact;
* a committed artifact whose provenance cannot be read — every pre-wave-7 artifact included — is
  owed as `provenance-unreadable`;
* one whose contract digest is not what its slice computes against `--from` is owed as
  `contract-mismatch`, a false claim about derivation;
* a committed file the model derives nothing at is owed as `unfollowed`;
* a suite whose own contract digest its model does not compute is **refused**, not narrowed — the
  short list it would produce looks exactly like a correct short list.

Every narrowed obligation carries the path from the artifact's seeds to the changed construct, one
hop per line, same as scenario impact.

### The check that bites

`generate-check`, `suite-check` and `synth-check` all flow through one `sync` comparison in
`xtask`, and that one place now reads the contract digest out of both the committed and the freshly
generated artifact: a mismatch fails the gate with its own sentence — *a stale contract digest is a
false claim about the model slice it derives from* — beside the plain byte-drift message. No tenth
step; the same three steps got sharper teeth.

### Proven on the fixture pair

`crates/ess-diff/tests/artifacts.rs`, by name: the revision pair's four-change delta narrows the
owed artifacts to a strict subset — the `Currency` changes reach the `Money`, `Headline` and
command schemas, the domain page and both API documents, while the `PriceListId` and event schemas
are absent; a grant change reaches the `OpenAPI` document (which renders grants) and not the
`AsyncAPI` one (which reads no actor); and a system-header change owes every artifact. The paths
are asserted hop by hop where they are load-bearing.

## W7.2 — entities and commands join the delta

*Placeholder; not started.* Gap register D-1 executed: predicate comparison as conservative
canonical equality — equal says nothing, different says *changed* with no direction, implication
stays refused. Entities, commands, views and bindings enter the comparison, and W7.1's
`UncomparedFamilyChanged` arm shrinks by construction as each family starts arriving as change
entries instead of landing in the fail-closed catch-all. A predicate-bearing construct in a slice
already falls into the fail-closed path today; W7.2 is what lets a change to one be *named*.

## W7.3 and W7.4 — deferred by operator decision

The second emitter (Go) and obligations-as-artifacts stay described on
[`ess-roadmap.md`](ess-roadmap.md) and are not scheduled. Nothing in W7.1 or W7.2 depends on
either; W7.4's precondition — a contract digest that exists in code — is now met, so scheduling it
later is a decision, not a build.
