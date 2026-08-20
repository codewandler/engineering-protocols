# Examples

Worked examples of complete inputs. These double as integration-test fixtures, so they are kept
runnable rather than illustrative — a change that breaks one fails the build.

| Example | What it is |
|---|---|
| [`development-passkeys/`](development-passkeys/) | A task, its artifact manifest, the evidence a harness would submit, and the decisions the engine returns |
| [`billing/`](billing/) | The normative executable system specification: two bounded contexts, parsed by a test in `ess-domain` and by `protocol ess validate` |
| [`oracle-fixture/`](oracle-fixture/) | Not a system anybody would run. The constructs billing deliberately does not carry, so the conformance oracle has an input for every check it must be able to fail |
