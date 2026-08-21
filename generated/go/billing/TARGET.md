<!--
  generated from billing v3
  model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
  compiler 0.1.0 · generator 0.1.0
  do not edit: regenerate with `protocol ess synthesize --target go`
-->
# Target notes — go

For billing v3. The `PLAN.md` beside this file is language-neutral and **byte-identical in every target's tree**; this document is what *this* target could not carry across it. Regenerate with `protocol ess synthesize --target go`.

4 weakening(s), 0 target refusal(s). A weakening is emitted code that holds less than the first target's; a target refusal is a capability the plan marks generated and this language cannot represent — a fact about the language, never about the specification.

## Weakened — emitted, with less than the first target holds

| the guarantee | what this target provides | capabilities affected |
| --- | --- | --- |
| handling a closed set of variants is exhaustive: a `match` that forgets one does not compile | the set stays closed — an undeclared variant cannot implement the sealed interface's unexported marker from another package — but a `switch` over it is not checked, so a consumer that forgets a variant compiles and falls through. Go has no exhaustiveness check and none can be emitted; every generated sealed interface says so in its own doc comment | domain type, entity lifecycle, command contract, component port, binding delivery |
| a value of a generated type exists only where a generated constructor or transition produced one | Go gives every type a zero value that no constructor has to produce, so `Email{}`, an invoice resting in a state nothing moved it to, and a nil variant of a sealed interface are all spellable from any package. The unexported field stops a *populated* value being forged; nothing in the language stops the empty one existing | domain type, entity lifecycle, command contract, event type, error type, view type |
| refining a runtime state into the typed lifecycle is total: every declared state has an arm and no other state can reach it | the snapshot's state field is a sealed interface, whose zero value is nil and names no declared state — the previous row's weakening, reaching this one. Refinement therefore answers `(value, ok)`, and a caller that ignores the second result gets the interface's own zero value | entity lifecycle |
| every generated type compares by value | Go defines `==` only for comparable types, so a generated type carrying a list, a map or bytes cannot be compared at all — and no deep comparison is emitted in its place, because a hand-written equality is behaviour, and behaviour is not synthesised | domain type, entity lifecycle, command contract, event type, error type, view type |

## Refused by this target — planned, not emitted

| capability | source | why |
| --- | --- | --- |
