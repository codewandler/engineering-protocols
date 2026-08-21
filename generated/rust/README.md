# Synthesised Rust workspaces

**Do not edit these files.** They are synthesised from the specifications under
[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what
the specifications determine — or if a workspace stops compiling.

A workspace here is the part of an implementation that was never anyone's to write: the
types, the states whose illegal transitions do not compile, the contracts, one crate per
component holding its port, and one system crate holding the bindings and the one transport
the specification's own delivery words require. What remains deliberately unwritten is each
workspace's `PLAN.md` — every capability of the specification with exactly one disposition:
generated, an obligation carrying its contract, or a refusal carrying its reason — and every
obligation is also a typed stub in the workspace, refusing with a value that names it.
`Cargo.lock` and `target/` inside a workspace are written by `cargo check` and are not part
of the committed tree.

The other half of the bargain is hand-written, and lives outside this tree because the
ownership boundary is absolute: [`examples/billing-realization`](../../examples/billing-realization)
implements each obligation against its contract, and its linker assembles components and
implementations into a runnable system without ever choosing — zero implementations for an
obligation is an unsatisfied obligation, two is an ambiguity error naming both (gap register
D-2). `cargo xtask synth` then executes the committed conformance suite, unchanged, against
that linked system: 27 of 27 scenarios must pass, and the deliberately corrupted variant
beside the honest one must fail exactly the scenario that exists to catch it.

## The second transport, and the record two applications write

A component whose specification says `reached_by: network` has callers that are not
deployed with it, so its surface exists on a wire. *Which* wire is derived rather than
chosen: this repository projects exactly one contract for a command surface — the OpenAPI
document under [`generated/openapi/`](../openapi) — and an OpenAPI document is an HTTP
contract, so a server speaking anything else would contradict the document committed beside
it. The emitted surface answers exactly the paths that document declares, plus
`GET /openapi.json` and `GET /docs`, which serve the committed contract and the committed
prose byte for byte. A path the contract does not declare is a 404; a declared path under
another method is a 405; neither is a status the contract declares, because both are facts
about a transport rather than about a command.

**The startup record.** Every served application writes three lines of JSON to standard
output before it answers anything, and every member of them is derived from the
specification — except `runtime`, which is the process's own: the language it was
synthesised into, the address it bound and the port it took.

| line | carries |
| --- | --- |
| `system.starting` | the system, its version, the model digest, the contract digest, every component, and the plan's disposition counts |
| `surface.serving` | the served component, its declared reach, the transport, the number of routes, and every route as method, path, what it serves and the construct it serves |
| `system.ready` | the system, and how many surfaces this process serves |

The split is the whole comparison. Two applications synthesised from one specification must
agree on every byte **outside** `runtime`, and `cargo xtask synth` starts both, reads their
records, strips `runtime` and compares — so a member that moved into `runtime` to make a
comparison pass would be a member that stopped being compared, and a member the record
gains tomorrow is compared without anyone editing the comparison.

| workspace | generated from | generated | obligations | refused | plan |
| --- | --- | --- | --- | --- | --- |
| [`billing/`](billing) | billing v3 (model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861, contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367) | 33 | 8 | 4 | [`billing/PLAN.md`](billing/PLAN.md) |
| [`gatepass/`](gatepass) | gatepass v1 (model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61, contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e) | 22 | 5 | 2 | [`gatepass/PLAN.md`](gatepass/PLAN.md) |
