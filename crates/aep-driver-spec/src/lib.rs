//! The driver's documents and records — and nothing that runs.
//!
//! A workflow says what states exist and what evidence each transition needs. It deliberately does
//! not say *how* to obtain that evidence: that is a harness's business, and keeping it out of the
//! workflow is what lets one workflow govern a Rust repository and a Terraform one. This crate is
//! the missing half, as a **document** rather than as code — a step map — plus the two records a
//! run leaves behind and the harness-neutral description of what a model may hold while it runs.
//!
//! ```yaml
//! format: aep.driver-steps/1
//! id: development/default
//! workflow: adp/default/1
//! states:
//!   implement:
//!     steps:
//!       - kind: llm
//!         skills: [planning]
//!         prompt: implement the story's acceptance statement
//!       - kind: command
//!         run: [cargo, test, --workspace]
//!         evidence:
//!           kind: test_result
//!           verifier: test-runner
//!           suite: unit
//! ```
//!
//! # Why this is a leaf on `aep-domain` and nothing else
//!
//! `aep-schema` publishes [`RawStepMap`](map::RawStepMap)'s schema and `aep-engine` loads
//! `drivers/` as the last row of its document tree, so both must see these types — and
//! `aep-engine` already depends on `aep-schema`. A single `aep-driver` crate holding both these
//! documents *and* the router that consumes `Evaluation` would therefore close the cycle
//! `aep-schema → aep-driver → aep-engine → aep-schema`, which `cargo` refuses. The split is review
//! finding **F1**, and the boundary is exactly this: everything here is a value, and nothing here
//! decides anything about a run.
//!
//! # Where this crate deviates from the design sketch, and why
//!
//! * `harness-planning-and-driver-design-v0.1.md` § 4.2 writes `workflow: adp/default@1`. That
//!   spelling is wrong and § 4.7 D1 says so: this repository's versioned reference is
//!   `adp/default/1`, and a second spelling of a version pin is a second parser.
//! * The same sketch writes `skill: planning` on an `llm` step. This crate reads
//!   [`skills`](map::LlmStep::skills), a list, for the same reason: one spelling, and a step that
//!   names two skills needs no second key to say so.
//!
//! # The pin is a type, not a validator rule
//!
//! [`PinnedWorkflowRef`](pin::PinnedWorkflowRef) exists because the mandatory pin has to be
//! mandatory *in the published schema too*. `WorkflowRef::major` is an `Option`, its pattern makes
//! the version group optional, and its `JsonSchema` writes that pattern verbatim — so a generated
//! schema would have told an author `workflow: adp/default` was fine while the loader refused it.
//! That is invariant 1 inverted, and review finding **F6** is what caught it.
//!
//! # What is a document and what is a record
//!
//! Invariant 2 — *parse, then validate* — governs the **document**: [`RawStepMap`](map::RawStepMap)
//! deserialises, [`StepMap`](map::StepMap) is obtained only by validating, and no validated type
//! here implements `Deserialize`. The **records** are the other thing: a
//! [`DriverCursor`](cursor::DriverCursor) is written by the driver and read back by the driver, so
//! it round-trips through serde in both directions. It is not authored, has no schema and is not
//! a protocol document — which is why it can do that without the invariant having an opinion.

pub mod cursor;
pub mod digest;
pub mod map;
pub mod pin;
pub mod tool;

/// The step-map format version this build reads.
///
/// A document declaring anything else is refused rather than guessed at: a step map that a newer
/// driver wrote may name step kinds this one cannot execute, and executing the ones it recognises
/// would be a run that silently skipped the rest.
pub const STEP_MAP_FORMAT: &str = "aep.driver-steps/1";
