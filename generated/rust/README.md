# Synthesised Rust workspaces

**Do not edit these files.** They are synthesised from the specifications under
[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what
the specifications determine — or if a workspace stops compiling.

A workspace here is the part of an implementation that was never anyone's to write: the
types, the states whose illegal transitions do not compile, the contracts, one crate per
component holding its port, and one system crate holding the bindings and the one transport
the specification's own delivery words require. What remains deliberately unwritten is each
workspace's `PLAN.md` — every capability of the specification with exactly one disposition:
generated, an obligation carrying its contract, or a refusal carrying its reason — and every
obligation is also a typed stub in the workspace, refusing with a value that names it.
`Cargo.lock` and `target/` inside a workspace are written by `cargo check` and are not part
of the committed tree.

The other half of the bargain is hand-written, and lives outside this tree because the
ownership boundary is absolute: [`examples/billing-realization`](../../examples/billing-realization)
implements each obligation against its contract, and its linker assembles components and
implementations into a runnable system without ever choosing — zero implementations for an
obligation is an unsatisfied obligation, two is an ambiguity error naming both (gap register
D-2). `cargo xtask synth` then executes the committed conformance suite, unchanged, against
that linked system: 27 of 27 scenarios must pass, and the deliberately corrupted variant
beside the honest one must fail exactly the scenario that exists to catch it.

| workspace | generated from | generated | obligations | refused | plan |
| --- | --- | --- | --- | --- | --- |
| [`billing/`](billing) | billing v3 (model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861) | 33 | 8 | 4 | [`billing/PLAN.md`](billing/PLAN.md) |
