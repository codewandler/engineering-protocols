# Conformance

Language-neutral fixtures, scenarios and expected results, so a backend in any language can prove it
implements the contract rather than merely compiling against it.

| Directory | Contents |
|---|---|
| `fixtures/` | input documents and entity graphs |
| `scenarios/` | ordered command/query steps, including the §104 end-to-end scenario |
| `expected/` | the observable results a conforming backend must produce |

Scenarios must not depend on sleeps: ordering is established with consistency tokens.
