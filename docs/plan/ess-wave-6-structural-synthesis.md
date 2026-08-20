# ESS wave 6 — structural synthesis

> **Accepted for implementation, 2026-08-20.** Design:
> [`ess-structural-synthesis-obligations-realizations-design-v0.1.md`](../design/ess-structural-synthesis-obligations-realizations-design-v0.1.md),
> reviewed against the code in
> [`2026-08-20-next-waves-feasibility-review.md`](../reviews/2026-08-20-next-waves-feasibility-review.md).
> The precondition the roadmap set is met, both halves: the oracle has been seen to pass a correct
> implementation (27 of 27 at `0.4.0-ess-wave-4`) and to fail twelve deliberately wrong ones, and the
> suites it generated are committed and drift-checked. Generating code judged by an oracle nobody has
> seen fail was the mistake the ordering rule existed to prevent, and it can no longer happen here.

**Goal: a generated Rust workspace from the billing specification that compiles, and that passes the
suite wave 4 generated — with a deliberately faulty implementation still failing it.**

## What this wave is, in one sentence each way

For the person adopting this: the specification that already writes your documentation, your
contracts and your tests now writes the part of your code that was never yours to write — the types,
the states, the ports — and hands you a typed list of exactly what remains.

For the machinery: a fifth projection, except the artifact has to *run*, which is why it comes last
and why its acceptance criterion is executed by the oracle rather than asserted by a test the
generator wrote about itself.

## Decisions, taken

| decision | taken as | why |
|---|---|---|
| where it lives | a new crate, `ess-synth` | `ess-gen` is a pure projection engine and stays one; the review's H6 argument against mixing kinds in that crate applies here unchanged |
| the verb | `protocol ess synthesize --path <spec> --out <dir>` | the roadmap's name. No collision: `conform synthesize` writes a suite, this writes a workspace, and each says so |
| the hinge is in | `SynthesisPlan` first, code second — every capability gets exactly one disposition: **generated**, **obligation**, or **refused** | design §2 and §5 are this repository's own refusal culture applied to codegen. Guessed business logic is the `calculate_tax` the design names, and a plan makes "what was not generated" a document instead of an absence |
| obligations stay in the plan | an obligation is a typed entry in the plan document — not an AEP artifact, not a task, not an identity scheme | design §13–§16 and §22–§23 build a programme on symbols this wave does not need to close its loop. They stay proposed |
| §28 is refused, not deferred | obligation kinds never derive capability grants | a second grant path is what invariant 6 exists to forbid, and `CapabilityPolicy::restrict` mechanically refuses it — review H8 |
| the generated workspace is committed | under `generated/rust/billing/`, excluded from the root workspace, drift-checked byte-identical | same policy as every other projection: invariant 9 makes the check cheap, and a generator whose output is not committed is a generator whose regressions are invisible |
| hand-written code never enters the generated tree | obligation implementations live in a committed crate outside `generated/`, satisfying generated interfaces by import | design §17's ownership boundary, kept absolute so `generated/` stays fully disposable |
| one transport | exactly the one the billing example's components and bindings require | every further adapter is a later wave; pretending otherwise is how a generator acquires six half-adapters |
| the gate grows a step | `synth-check`: regenerate, diff, then `cargo test` in the generated workspace | the acceptance criterion that matters has to be the one CI executes, or it is a claim |

Rejected outright rather than deferred, with the roadmap: §36's behavioural synthesis, §41's agent
loop, §38's residual-synthesizer framing. Deferred with their designs: `Realization` (§30–§34),
topology synthesis (§35), formal verification.

## W6.1 — the plan, and Rust semantic types

`SynthesisPlan` from the billing `EssIr`: deterministic, inspectable, every disposition carrying its
reason. Then the types: newtypes that stay distinct from their representations, tagged unions as
enums, events and declared errors as types, one outcome enum per command, views as types — and
lifecycles as state types whose illegal transitions do not compile, because "the compiler refuses
the transition the specification refuses" is this wave's version of an unresolved reference being
unrepresentable.

Acceptance: the emitted crate passes `cargo check`; emitting twice is byte-identical; the plan for
billing lists every construct with a disposition and zero guesses.

## W6.2 — component skeletons and one transport

A component's inner domain generated in full; its outer surface generated as a port — command
handlers, view queries — with exactly one transport implemented. Obligations surface here: everything
a port needs that the specification does not determine becomes a named entry, not a `todo!()`.

Acceptance: the generated workspace builds end to end with obligation implementations stubbed as
refusals, and the plan's obligation list is exactly the set of stubs.

## W6.3 — the generated code passes the generated tests

The only acceptance criterion that matters, and it is executed rather than asserted: the committed
billing suite — the same `suites/generated/billing/suite.json` wave 4 produced, unchanged — runs
against the generated workspace linked with hand-written obligation implementations, and passes.
Then the falsifiability half: one obligation implementation deliberately corrupted, and the same
suite fails the scenario that exists to catch it. Both results land in CI via `synth-check`.

## What is deliberately not in this wave

Behavioural synthesis, any second transport, obligation-as-artifact and the ADP task integration,
`Realization` and everything downstream of it, topology generation, and touching the diff:
`ess-diff` does not learn about generated code this wave, so a change to the specification owes the
whole generated workspace, per the fail-closed polarity wave 5 made structural. Narrowing that is a
later wave's work, and it has a design to argue with.
