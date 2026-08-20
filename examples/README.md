# Examples

Worked examples of complete inputs. These double as integration-test fixtures, so they are kept
runnable rather than illustrative — a change that breaks one fails the build.

| Example | What it is |
|---|---|
| [`development-passkeys/`](development-passkeys/) | A task, its artifact manifest, the evidence a harness would submit, and the decisions the engine returns |
| [`billing/`](billing/) | The normative executable system specification: two bounded contexts, parsed by a test in `ess-domain` and by `protocol ess validate` |
