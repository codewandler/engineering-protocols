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

The Go half of a served surface is `net/http` and `encoding/json`, both standard library, and
generated codecs beside them: a generated type carries an unexported field, which
`encoding/json` cannot see, and exporting it would undo the distinctness the newtype encoding
exists for. The hand-written realization that links into it is a module of its own —
[`examples/gatepass-go-realization`](../../examples/gatepass-go-realization) — reaching this
tree through a filesystem `replace`, so nothing here resolves over a network either.

| module | generated from | generated | obligations | refused | weakened | target-refused | plan | target notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`billing/`](billing) | billing v3 (model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861, contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367) | 33 | 8 | 4 | 4 | 0 | [`billing/PLAN.md`](billing/PLAN.md) | [`billing/TARGET.md`](billing/TARGET.md) |
| [`gatepass/`](gatepass) | gatepass v1 (model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61, contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e) | 22 | 5 | 2 | 5 | 0 | [`gatepass/PLAN.md`](gatepass/PLAN.md) | [`gatepass/TARGET.md`](gatepass/TARGET.md) |
