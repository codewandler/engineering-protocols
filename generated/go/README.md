# Synthesised Go modules

**Do not edit these files.** They are synthesised from the specifications under
[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what
the specifications determine — or if a module stops being `gofmt`-clean, stops compiling, or
stops passing `go vet`.

This tree is the **second emitter** behind the synthesis seam, and the reason it exists is
that a claim about language-neutrality is worth exactly one test. Go was chosen because it
has no sum type: every tagged union, every enum and every command outcome has to be encoded
by hand — as a sealed interface, one unexported marker method and one struct per variant — or
refused out loud. The plan did not change to admit it: each module's `PLAN.md` and `plan.json`
are **byte-identical** to the ones in [`../rust`](../rust).

What Go holds more weakly than Rust, and what it cannot represent at all, is in each module's
`TARGET.md` — never in the plan, because a weakening is a fact about a language and the plan
is a fact about the model. Standard library only, and a module path under the reserved
`example.invalid` domain, so nothing here can be mistaken for something publishable.

| module | generated from | generated | obligations | refused | weakened | target-refused | plan | target notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`billing/`](billing) | billing v3 (model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861, contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367) | 33 | 8 | 4 | 4 | 0 | [`billing/PLAN.md`](billing/PLAN.md) | [`billing/TARGET.md`](billing/TARGET.md) |
