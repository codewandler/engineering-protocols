# Examples

Worked examples of complete inputs. These double as integration-test fixtures, so they are kept
runnable rather than illustrative — a change that breaks one fails the build.

| Example | What it is |
|---|---|
| [`development-passkeys/`](development-passkeys/) | A task, its artifact manifest, the evidence a harness would submit, and the decisions the engine returns |
| [`billing-conformance/`](billing-conformance/) | The closed loop: a task that completes because a conformance run against `billing/` passed, and refuses when the same run fails |
| [`billing/`](billing/) | The normative executable system specification: two bounded contexts, parsed by a test in `ess-domain` and by `protocol ess validate` |
| [`oracle-fixture/`](oracle-fixture/) | Not a system anybody would run. The constructs billing deliberately does not carry, so the conformance oracle has an input for every check it must be able to fail |
| [`revision-pair/`](revision-pair/) | Two revisions of one specification, differing by four semantic changes and a great deal of text that means nothing — what `protocol ess diff` is checked against |
| [`gatepass/`](gatepass/) | Visitor passes for a building, and the demonstration specification: its one component declares `reached_by: network`, so the surface both synthesised applications serve is an HTTP one — the same routes, the same startup record, the same published contract |
| [`gatepass-realization/`](gatepass-realization/) | Not a specification: the hand-written Rust half of the synthesised `gatepass` workspace — one implementation per obligation, a linker that never chooses, and the binary that hands the assembled system to the generated surface |
| [`gatepass-go-realization/`](gatepass-go-realization/) | The same five obligations again, written from the same specification against the synthesised Go module. Deliberately not a translation of the Rust one: the demonstration is that two independently written realizations answer the same requests the same way |
| [`billing-realization/`](billing-realization/) | Not a specification: the hand-written half of the synthesised `billing` workspace — one implementation per obligation in `generated/rust/billing/PLAN.md`, the linker that assembles them without choosing (D-2), and the bridge that runs the committed suite against the linked system |
