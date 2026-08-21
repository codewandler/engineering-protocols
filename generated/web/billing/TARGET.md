<!--
  generated from billing v3
  model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
  compiler 0.1.0 · generator 0.1.0
  do not edit: regenerate with `protocol ess synthesize --target web`
-->
# Target notes — web

For billing v3. The `PLAN.md` beside this file is language-neutral and **byte-identical in every target's tree**; this document is what *this* target could not carry across it. Regenerate with `protocol ess synthesize --target web`.

6 weakening(s), 0 target refusal(s). A weakening is emitted code that holds less than the first target's; a target refusal is a capability the plan marks generated and this language cannot represent — a fact about the language, never about the specification.

## Weakened — emitted, with less than the first target holds

| the guarantee | what this target provides | capabilities affected |
| --- | --- | --- |
| the generated crate forbids `unsafe`, so the compiler closes the question rather than a reader checking it | a WebAssembly export is a `#[no_mangle]` item, and rustc's own `unsafe_code` lint flags one — so the bridge crate cannot declare `#![forbid(unsafe_code)]` and declares `#![deny(missing_docs)]` alone. It contains no `unsafe` block, no `unsafe fn` and no raw-pointer dereference; what is lost is the compiler closing the question, not the property | component port, command contract |
| a move the lifecycle does not declare does not compile | the page speaks JSON, and JSON carries no type parameter: any declared command can be sent from any state, and an illegal move comes back as the declared refusal the behaviour answers with — at run time, from the system, rather than as a build that failed. The typed lifecycle still holds inside the system this bridge drives; it simply does not reach across the boundary | entity lifecycle, command contract |
| the current state of every instance is observable | the synthesised system holds no entity store — where instances live is an obligation — so the page shows each declared view's rows beside the entity's declared lifecycle, and shows a per-instance state only where a view projects one. Deriving a state from the event log would be behaviour, and behaviour is not synthesised | entity lifecycle, view type |
| an `Integer` is sixty-four bits wide, end to end | the bridge writes it as a JSON number, which is what the published wire contract fixes, and a browser reads every JSON number as a double — so a magnitude past 2^53 is rounded by the page. The bridge itself never truncates: a fraction, an exponent or an out-of-range magnitude arriving from the page is refused with the path it was found at | domain type, command contract, event type, error type, view type |
| a generated tree builds from exactly its committed bytes, outside this repository | this tree is a front end over the Rust target's crates, so its manifest names them by relative path — `../../rust/<system>/` from this tree's root, which is the layout `cargo xtask synth` commits. Copy both trees or neither; a browser realization on its own has no system to drive | domain type, entity lifecycle, command contract, event type, error type, view type, conversion, binding transformation, binding delivery, component port |
| a binding whose failure policy is `retry` is redelivered on the schedule the transport provides | the page is the transport's caller, and nothing here advances a clock: redelivery is a request a person makes, one occurrence at a time, and the duplicate `at_least_once` permits is something to watch rather than something to wait for. When to try again is a deployment decision the specification does not take, and this target does not take it either | binding delivery |

## Refused by this target — planned, not emitted

| capability | source | why |
| --- | --- | --- |
