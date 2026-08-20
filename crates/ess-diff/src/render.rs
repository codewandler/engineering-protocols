//! The delta as a person reads it.
//!
//! Beside the JSON rather than instead of it, and in the library rather than in the CLI: the text
//! form is a *rendering of the same values*, so a harness that wants the sentence gets the sentence
//! the command line prints, and there is one place where a change is put into words. The CLI's
//! `--format json` prints [`EssDelta::to_canonical_json`] and its `--format text` prints this.
//!
//! # What the shape is for
//!
//! A reviewer's first question is "did this widen anything", not "how many lines moved", so the
//! count line answers it before the list starts, and every change leads with its relation rather
//! than with its subject. The id is on its own line under each change because it is the thing a
//! review comment or a later impact record quotes, and it is long.
//!
//! [`impact()`] follows the same shape one level up: the delta first, then how much of the suite is
//! owed again, then one block per scenario carrying the change and the edges that reached it. A
//! scenario id with nothing under it is the report design §24 was written against.

use std::fmt::Write as _;

use crate::change::SemanticRelation;
use crate::delta::EssDelta;
use crate::impact::{EssImpact, Invalidation};

/// The delta as lines a person reads, ending in a newline.
///
/// An empty delta says so in one line. That is the answer design §67 asks for and the one the
/// fixture pair under `examples/revision-pair/` exists to make checkable: two source trees can
/// differ in every comment and every declaration order and still mean the same system.
pub fn text(delta: &EssDelta) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "{} {} → {}",
        delta.after.system, delta.before.specification_version, delta.after.specification_version
    );
    let _ = writeln!(out, "  before  {}", delta.before.spec_digest);
    let _ = writeln!(out, "  after   {}", delta.after.spec_digest);
    out.push('\n');

    if delta.is_empty() {
        out.push_str("no semantic change: these two revisions mean the same system\n");
        return out;
    }

    let _ = writeln!(
        out,
        "{} change(s): {} widening, {} narrowing, {} other",
        delta.len(),
        delta.count(SemanticRelation::Expanded),
        delta.count(SemanticRelation::Narrowed),
        delta.count(SemanticRelation::Changed)
    );
    out.push('\n');

    for change in delta.changes() {
        let _ = writeln!(
            out,
            "  {:<8} {} {}: {}",
            change.relation().verb(),
            change.category(),
            change.subject_name(),
            change.describe()
        );
        let _ = writeln!(out, "           {}", change.id());
    }

    out
}

/// The impact report as lines a person reads, ending in a newline.
///
/// Shaped by what a reader does next. The delta comes first, because "what changed" is the question
/// under "what does it invalidate" and design §24's complaint is exactly about a report that answers
/// the second without the first. Then the count line, so the size of the answer arrives before the
/// list. Then one block per scenario, each with the change that reached it and the edges in between
/// — never a scenario id on its own, which is the `email-component risk = high` §24 refuses.
///
/// A whole-suite answer says so in one line and gives the reason, rather than printing every
/// scenario with the same sentence beside it.
pub fn impact(report: &EssImpact) -> String {
    let mut out = text(&report.delta);
    out.push('\n');

    let _ = writeln!(
        out,
        "suite {} {} ({}): {} of {} scenario(s) owed again",
        report.suite.system,
        report.suite.specification_version,
        report.suite.spec_digest,
        report.churn.conformance_scenarios_invalidated,
        report.churn.conformance_scenarios_total
    );
    let _ = writeln!(
        out,
        "{} construct(s) reached: {} changed, {} depend on one directly, {} through another",
        report.churn.semantic_elements_directly_changed
            + report.churn.semantic_elements_directly_dependent
            + report.churn.semantic_elements_transitively_impacted,
        report.churn.semantic_elements_directly_changed,
        report.churn.semantic_elements_directly_dependent,
        report.churn.semantic_elements_transitively_impacted
    );
    out.push('\n');

    match &report.invalidation {
        Invalidation::Whole { because } => {
            let _ = writeln!(out, "  every scenario in the suite is owed again, because");
            let _ = writeln!(out, "  {because}");
        }
        Invalidation::Narrowed { scenarios } => {
            if scenarios.is_empty() {
                // Not "nothing is affected". The distinction is the whole polarity of the wave, and
                // a line that blurred it here would undo in prose what the types make impossible.
                let _ = writeln!(
                    out,
                    "  no scenario in this suite rests on anything this delta reached. That is not \
                     a claim that its results still hold — gate G19 still binds evidence to the \
                     specification digest it was produced against, and the digest moved."
                );
            }
            for (id, impact) in scenarios {
                let _ = writeln!(out, "  {id}");
                for reason in impact.reasons() {
                    let _ = writeln!(
                        out,
                        "    {} {} — {}",
                        reason.class(),
                        reason.target,
                        reason.change
                    );
                    out.push_str(&reason.explain());
                }
            }
        }
    }

    out
}
