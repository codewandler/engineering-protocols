# ESS wave 1 — the join, and the model

> **Delivered.** An executable system specification is a document this repository parses, validates
> and refuses, and the protocol can already require conformance to one. 642 tests, 0 failures.

Goal, from [`ess-roadmap.md`](ess-roadmap.md): make an ESS a thing that exists before anything is
generated from it. Nothing here compiles a specification. That is deliberate — a generator built on a
model nothing has stress-tested generates confident nonsense, and the ordering rule for this roadmap
is that **each wave must be falsifiable by the one before it**.

Projected status after: **≈20% of the ESS design**, with AEP unchanged at 100% of its v0.2 scope.

## What was built

| Deliverable | Where |
|---|---|
| The join: artifact kind, evidence kind, `ess-conformance` principle | `aep-domain`, `principles/verification/` |
| The typed model | [`crates/ess-domain/`](../../crates/ess-domain/) — 10 modules |
| The normative example, parsed by a test | [`examples/billing/`](../../examples/billing/) |
| `protocol ess validate` | [`crates/protocol-cli/`](../../crates/protocol-cli/) |
| Published JSON Schema, drift-checked | [`schemas/generated/ess.schema.json`](../../schemas/generated/ess.schema.json) |
| The adopter's guide | [`docs/guide/specification.md`](../guide/specification.md) |

`ess-domain` is 10 modules and 169 tests; the wave also added 67 tests elsewhere, most of them
consequences of the review below.

## Why the join came first

W1.1 landed before a line of `ess-domain` existed, and could have landed a year earlier: an artifact
kind, an evidence kind, a principle. With those, a task can be blocked until something proves an
implementation conforms to its specification, with a person producing that evidence by hand.

That is the whole vision working, before anything is compiled. Everything after it is about removing
the hand.

## What the model insists on, and what it cost to learn

Three of these came from writing the example *before* the model was finished. That ordering is the
reason they are in wave 1 rather than in a v2 migration.

**A command that can be refused has more than one outcome.** Not an `emits` list. A command with a
precondition has at least two results, and a specification recording only the happy one generates a
suite that never checks the branch where the money does not move.

**An outcome the input cannot decide says so.** Whether a mail provider accepts an address is not a
function of the request. `external: <the cause>` is a third thing, distinct from a satisfied
condition and from an unsatisfiable one — writing `when: false` would claim the branch is
unreachable, which is a different and false statement. This was discovered by trying to write
`SendEmail` in the example and finding the model could not express it.

**A projection declares its consistency.** `eventual` is what makes a generated assertion
`eventually` rather than immediate. The alternative failure is a suite that passes on a laptop and
flakes in CI, and the usual fix for that — a sleep — makes the suite test the machine it runs on.

**An entity's identity has a name, not just a type.** Found the same way: the example's view projects
`invoice_id`, and with only a type there, every generator would have invented its own name for it.

**An illegal move is illegal because nobody wrote it.** No rule forbids `Paid → Cancelled`; there is
simply no transition. A rule would be a second place for the same truth to live.

## What the review changed

Three independent reviews ran against the finished wave. Two blockers, both the same shape — **a
guard that could not guard**:

* The published schema declared the normative example invalid. `Version` is a `u32` inside and is
  written `v3` everywhere, so its derived JSON Schema described the representation rather than what
  an author writes. The fix generalised: `crates/aep-schema/tests/published.rs` now validates every
  document this repository ships against every schema it publishes, which immediately found the same
  bug in `principle.schema.json`.
* `ValidationCode::ALL` — the list the tests iterate — was maintained by hand, and had silently
  fallen five codes behind the enum. Its doc comment claimed "adding a variant without adding it here
  fails the test below". It did not. The enum, its wire strings and the list are now generated from
  one declaration, so the class of mistake is gone rather than fixed.

The rest were rules that existed and were never reached, codes that named the wrong thing, and
places where a specification could say something meaningless and be accepted.

## What was deliberately not done

| Not built | Where it goes |
|---|---|
| A compiler: source to a normalized IR | ESS wave 2 |
| Projections: OpenAPI, AsyncAPI, documentation | ESS wave 3 |
| Test synthesis | ESS wave 3 |
| Rust structural synthesis | wave 4 at the earliest, and only once the model has survived three projections |
| Components, bindings, deployment topology | not modelled at all yet — the example says so explicitly |
