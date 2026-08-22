//! **F-W4.2-4**: the plan a run will be held to, against the step map that has to satisfy it —
//! decided before the first step executes.
//!
//! # The finding this closes, and what it cost
//!
//! `StepMap::check_run` validates the map *against the protocol*: every evidence kind a step
//! declares is a kind the protocol declares. It never validated the converse — that every kind the
//! **plan** will demand is a kind some step can produce. So a map that could never satisfy its plan
//! loaded, ran, and stopped only at the guard that needed the evidence.
//!
//! Run `W4-2/1` is the measurement. It walked six states, spent ten model sessions, 76 minutes and
//! $31.46, and arrived at `adversarial_verify -> review` with `evidence.missing = 2`: the plan
//! wanted a `specification` record and an independent `verification` record, and no step of
//! `development/checks` declares either kind. Every fact needed to say so was in two documents on
//! disk before the run started. This module is the comparison, and it is free — no clock, no
//! process, no network.
//!
//! # Which side of the line each case falls on
//!
//! A refusal that fires on a rule which would never have bitten is worse than no check at all,
//! because the fix is to weaken the check. So the three narrowings are named:
//!
//! * **Applicability is the plan's, not this module's.** A principle scoped away by a fact the task
//!   declared is already absent from the plan (`Principle::applies`), so nothing it wanted is
//!   demanded here. An **undeclared** fact leaves the principle in force and its demands count in
//!   full — invariant 5, and the reason a task saying nothing is refused where a task saying
//!   `change.code: false` is not.
//! * **Reachability is `aep_engine::demanded_evidence`'s**, and it is applied to states and
//!   transitions only. A requirement on a state nothing can walk into blocks nothing.
//! * **Person-shaped demands are never a refusal.** A `human-approval` or `human-review` verifier,
//!   and the `approval` and `review` kinds, are things the driver may not mint at all
//!   (invariant 7 — `tests/evidence_scan.rs` enforces it), so *the map cannot produce this* is true
//!   of every map ever written and says nothing about this one. A person supplies them, at an
//!   `operator` step or between runs, and the pre-flight that owns that question is
//!   [`crate::approval::reachable_approvals`]. Where the map has no `operator` step to ask at, this
//!   is reported as a **warning** rather than a refusal.
//!
//! One further case is a warning for a different reason — it is undecidable rather than benign. A
//! demand pinning a `verifier:` that no declaring step names may still be met, because a record's
//! producer is fixed when the step runs and not when the map is written; and it may not be. Unknown
//! is not False, so it prints and does not block.
//!
//! # What this cannot see, stated rather than implied
//!
//! A **predicate** obligation. `profiles/development-standard.yaml:38` asks for
//! `contracts.failed == 0`, a fact only a `ContractResult` projects, and no completion condition
//! anywhere is expressed as an evidence requirement this walk could read. Mapping a fact path back
//! to the kinds that project it would be a second copy of `Evidence::facts`, which is how a fact and
//! a requirement come to disagree — so it is not done, and a run can still block on one of these
//! after passing this check.
//!
//! # Why this lives in the driver and not in the engine
//!
//! Because it is about a **step map**, which the engine has never heard of. The half that reads the
//! plan is the engine's (`aep_engine::demanded_evidence`); what is left here is set arithmetic over
//! evidence kinds and a report. Like [`crate::approval`], this is a pre-flight scan and not a gate:
//! it decides no transition, and it runs before there is an execution to decide one in.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::evidence::EvidenceKind;
use aep_domain::plan::ExecutionPlan;
use aep_domain::verification::Verifier;
use aep_driver_spec::map::{Step, StepMap};
use aep_engine::evaluate::{demanded_evidence, DemandedEvidence};

/// One evidence kind the plan can demand and no step of the map can produce.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MissingProducer {
    /// The kind nothing produces.
    pub kind: EvidenceKind,
    /// Every document that asks for it, in a stable order, so the refusal can be navigated to.
    pub demanded_by: Vec<String>,
    /// What stays shut while it is unmet, in a stable order.
    pub blocks: Vec<String>,
}

/// What a map can produce, measured against what its plan will ask for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CoverageReport {
    /// Every kind some step of the map can put into the run, in a stable order.
    pub mintable: BTreeSet<EvidenceKind>,
    /// Kinds demanded with no producer. Non-empty means the run cannot finish and is refused.
    pub missing: Vec<MissingProducer>,
    /// Things worth saying that are not grounds for refusing, one line each.
    pub warnings: Vec<String>,
}

impl CoverageReport {
    /// `true` when every demanded kind has a producer.
    ///
    /// Warnings do not affect it: a warning is something nobody could decide at launch, and
    /// refusing on an undecided question is the failure mode this check was written against.
    pub fn is_covered(&self) -> bool {
        self.missing.is_empty()
    }
}

/// What the plan will demand, against what the map declares it can produce.
///
/// The module documentation says which cases refuse, which warn, and which are deliberately
/// invisible.
pub fn evidence_coverage(plan: &ExecutionPlan, map: &StepMap) -> CoverageReport {
    let declared = map.declared_evidence_kinds();
    let verifiers = declared_verifiers(map);
    let operator_steps = map
        .states
        .values()
        .flat_map(|entry| entry.steps.iter())
        .filter(|step| matches!(step, Step::Operator(_)))
        .count();

    let mut mintable = declared.clone();
    if operator_steps > 0 {
        // An `operator` step is the run stopping and asking. What a person hands back is exactly
        // what the driver may not write for itself, so these two kinds become reachable — through
        // somebody at a keyboard, which is the only route the protocol admits.
        mintable.insert(EvidenceKind::Approval);
        mintable.insert(EvidenceKind::Review);
    }

    let mut gaps: BTreeMap<EvidenceKind, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    let mut warnings: BTreeSet<String> = BTreeSet::new();

    for entry in demanded_evidence(plan) {
        let kind = entry.requirement.kind;
        if entry.requirement.at_least == 0 {
            // Satisfied before any record is read, so nothing has to produce it. `at_least: 0`
            // beside a horizon is refused at parse time for the same reason.
            continue;
        }

        // Asked before the kind is looked up, because a pinned human verifier is a statement about
        // *who*, and no step of any map is a person: a `test_result` owed from `human-approval` is
        // not produced by the `cargo test` step that declares the same kind.
        if is_person_shaped(&entry) {
            if operator_steps == 0 {
                let from = entry
                    .requirement
                    .verifier
                    .as_ref()
                    .map_or_else(|| " ".to_owned(), |verifier| format!(" from `{verifier}` "));
                warnings.insert(format!(
                    "`{}` is asked for by {}{from}and is a person's to give — the driver mints no \
                     approval and signs as no person — and this map has no `operator` step to ask \
                     at. The record has to arrive between runs",
                    kind.as_str(),
                    describe(&entry)
                ));
            }
            continue;
        }

        if declared.contains(&kind) {
            if let Some(pinned) = &entry.requirement.verifier {
                if !verifiers
                    .get(&kind)
                    .is_some_and(|named| named.contains(pinned))
                {
                    warnings.insert(format!(
                        "`{}` is declared by a step, but {} asks for it from `{pinned}` and no \
                         step names that verifier — whether the record will match is decided when \
                         the step runs",
                        kind.as_str(),
                        describe(&entry)
                    ));
                }
            }
            continue;
        }

        let gap = gaps.entry(kind).or_default();
        gap.0.insert(describe(&entry));
        for blocked in &entry.blocks {
            gap.1.insert(blocked.label());
        }
    }

    CoverageReport {
        mintable,
        missing: gaps
            .into_iter()
            .map(|(kind, (demanded_by, blocks))| MissingProducer {
                kind,
                demanded_by: demanded_by.into_iter().collect(),
                blocks: blocks.into_iter().collect(),
            })
            .collect(),
        warnings: warnings.into_iter().collect(),
    }
}

/// `true` when only a person could produce this record.
///
/// Two spellings of the same thing: a pinned human verifier, and the two kinds whose payload is a
/// human decision. Both are outside every map's reach by construction, so neither may refuse one.
fn is_person_shaped(entry: &DemandedEvidence) -> bool {
    entry
        .requirement
        .verifier
        .as_ref()
        .is_some_and(Verifier::is_human)
        || matches!(
            entry.requirement.kind,
            EvidenceKind::Approval | EvidenceKind::Review
        )
}

/// Which verifiers the map's steps name for each kind they declare.
fn declared_verifiers(map: &StepMap) -> BTreeMap<EvidenceKind, BTreeSet<Verifier>> {
    let mut found: BTreeMap<EvidenceKind, BTreeSet<Verifier>> = BTreeMap::new();
    for step in map.states.values().flat_map(|entry| entry.steps.iter()) {
        if let Step::Command(command) = step {
            if let Some(mapping) = &command.evidence {
                found
                    .entry(mapping.kind)
                    .or_default()
                    .insert(mapping.verifier.clone());
            }
        }
    }
    found
}

/// One demand, attributed to the document that wrote it and the branch that reaches it.
///
/// A principle's obligation is named beside the principle, because a principle can oblige more than
/// once and *which* clause asked is what a person has to open the file at. The synthetic id the
/// engine gives a principle's top-level `evidence:` list is dropped: it points at no clause anybody
/// wrote.
fn describe(entry: &DemandedEvidence) -> String {
    use std::fmt::Write;

    let mut line = entry.source.label();
    if let aep_engine::RequirementSource::Principle {
        principle,
        obligation,
    } = &entry.source
    {
        if obligation.as_str() != format!("{principle}/evidence") {
            let _ = write!(line, " obligation {obligation}");
        }
    }
    if entry.requirement.independent {
        line.push_str(" (independent)");
    }
    if !entry.guards.is_empty() {
        let _ = write!(line, " under {}", entry.guards.join(", then "));
    }
    line
}
