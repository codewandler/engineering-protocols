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

| workspace | generated from | generated | obligations | refused | plan |
| --- | --- | --- | --- | --- | --- |
| [`billing/`](billing) | billing v3 (model digest e19d384dac86219a) | 33 | 8 | 4 | [`billing/PLAN.md`](billing/PLAN.md) |
