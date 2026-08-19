//! Protocol execution: resolution, evaluation and transitions.
//!
//! **Status: not yet implemented.** Planned public surface, from
//! `docs/design/consolidated-design-v0.2.md` §61 and `docs/design/reconciliation-v0.2.md` §4:
//!
//! | module | responsibility |
//! |---|---|
//! | `registry` | the documents in force: protocols, principles, workflows, profiles, lifecycles |
//! | `resolve` | `Task` + registry → [`ExecutionPlan`](aep_domain::ExecutionPlan), with the §66 cross-document checks |
//! | `execution` | live execution state: current state, facts, evidence log, event stream |
//! | `evaluate` | which requirements are owed now, which transitions are permitted |
//! | `policy` | capability decisions, with the rule that produced each one |
//! | `explain` | machine-readable explanations for a blocked action or an incomplete task |
//! | `engine` | the `ProtocolEngine` trait and its deterministic implementation |
