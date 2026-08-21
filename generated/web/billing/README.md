<!--
  generated from billing v3
  model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
  compiler 0.1.0 · generator 0.1.0
  do not edit: regenerate with `protocol ess synthesize --target web`
-->
# billing in a browser

**Do not edit these files.** They are synthesised from `examples/billing/` by `cargo xtask synth`, and CI fails if they differ from what the specification determines, if the module stops building for `wasm32-unknown-unknown`, or if `index.html` calls an export the module does not have. Regenerate with `protocol ess synthesize --target web`.

What is here:

| file | what it is |
| --- | --- |
| `index.html` | the page. Every command, field, event, view and state on it is built from `catalog.json`; none of it is typed into the HTML |
| `bridge.js` | the glue: reserve a buffer, write the request, call the module, read the response |
| `catalog.json` | the model, as the page reads it — and the same bytes the module carries |
| `crates/billing-web/` | the bridge crate: JSON in, JSON out, and the exports a browser calls |
| `PLAN.md`, `plan.json` | the plan, byte-identical to every other target's |
| `TARGET.md`, `target.json` | what a browser could not carry across the plan |

## Building it

```console
$ rustup target add wasm32-unknown-unknown
$ cargo build --release --target wasm32-unknown-unknown
```

That produces `target/wasm32-unknown-unknown/release/billing_web.wasm`. Every command will answer with the typed refusal naming an unmet obligation, because this tree implements none of them — that is the honest empty state, and the page shows the plan's own contract beside each one.

## Running it against a realization

A host crate that depends on this one as an `rlib`, links an implementation of every obligation, and exports `ess_realize` hands the assembled system to `install`. The exports below travel into that host's `cdylib`, so the same page drives it, and the page calls `ess_realize` if it is there. `examples/billing-web/` in this repository is that host.

The page looks for its module in three places, in order: the release build beside this file, the debug build, and `billing_web.wasm` in this directory. That last one is how a *realized* module is opened — copy it in under that name:

```console
$ (cd ../../../examples/billing-web && cargo build --release --target wasm32-unknown-unknown)
$ cp ../../../examples/billing-web/target/wasm32-unknown-unknown/release/*.wasm ./billing_web.wasm
```

Then serve this directory and open `index.html` — a browser will not instantiate WebAssembly from a `file://` URL:

```console
$ python3 -m http.server
$ open http://localhost:8000/index.html
```

## The exports

| export | what it does |
| --- | --- |
| `ess_input_reserve` | reserves a buffer of the given length for the next request and answers its address in linear memory |
| `ess_dispatch` | serves the request in that buffer and answers the address of the JSON response |
| `ess_output_len` | the length in bytes of the response just produced |
| `ess_realize` | optional, and never in this module: a host that links a realization exports it, and the page calls it if it is there |
