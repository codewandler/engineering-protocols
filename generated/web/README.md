# Synthesised browser realizations

**Do not edit these files.** They are synthesised from the specifications under
[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what
the specifications determine, if a tree stops building for `wasm32-unknown-unknown`, or if a
page calls an export its module does not have.

This tree is the **third emitter** behind the synthesis seam, and the first one a person can
click. It is not a fourth rendering of the model: it is the *boundary* around the Rust
target's system — JSON in over linear memory, JSON out — beside a `catalog.json` the page
builds itself from. Nothing about any system is typed into the HTML: the command list, the
input forms, the event names, the views and the lifecycles all come from the catalogue, so a
specification that changes changes the page in the same regeneration.

The plan did not change to admit it: each tree's `PLAN.md` and `plan.json` are
**byte-identical** to the ones in [`../rust`](../rust) and [`../go`](../go). What a browser
holds more weakly — a boundary that carries no types, instances observable only through
declared views, a number format narrower than the model's — is in each tree's `TARGET.md`.

The compiled `.wasm` is **not committed**: it is a build artifact, and `cargo xtask synth`
builds it rather than trusting a binary nobody can diff. The bridge chooses no realization
(gap register D-2), so the module it builds alone answers every command with the obligation
it is owed; [`examples/billing-web`](../../examples/billing-web) is the hand-written host that
links one in, and the gate drives *its* module through the page's own `bridge.js`.

| tree | generated from | generated | obligations | refused | weakened | target-refused | plan | target notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`billing/`](billing) | billing v3 (model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861, contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367) | 33 | 8 | 4 | 6 | 0 | [`billing/PLAN.md`](billing/PLAN.md) | [`billing/TARGET.md`](billing/TARGET.md) |
