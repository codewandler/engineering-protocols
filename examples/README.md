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
