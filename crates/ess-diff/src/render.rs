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

use std::fmt::Write as _;

use crate::change::SemanticRelation;
use crate::delta::EssDelta;

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
