//! The eval's own specification, against the two runs it was written from.
//!
//! The per-kind vocabulary — every one of the fifty-one kinds with a negative case beside it —
//! lives inline in `src/check.rs`, where the evaluator is. This file checks the thing the wave is
//! actually for: that `integrations/claude-code/eval/expectations.trace.yaml` is a document, that
//! it holds against two real committed transcripts, and that the ways it can be relaxed on the
//! command line are visible rather than silent.
//!
//! Both fixtures are real eval runs of the planning plugin from 2026-08-21, committed verbatim,
//! and both were 9 / 9 by hand under the shell assertions this document replaces.

use std::collections::BTreeSet;

use trace_domain::ir::TraceIr;
use trace_spec::adapter::read_transcript;
use trace_spec::check::check;
use trace_spec::report::{CheckReport, Outcome, UnknownReason, Verdict};

/// The committed transcript of eval run `7hTYjT`: 36 events, `Edit` × 3.
const SEVEN_H: &[u8] = include_bytes!("fixtures/plugin-eval-7hTYjT.jsonl");

/// The committed transcript of eval run `1huAQG`: 37 events, the same task with `Write` × 3.
///
/// The pair is deliberate. One run wrote the story bodies with `Edit` and the other with `Write`,
/// and a specification that only held for one of them would be a specification about a tool mix
/// rather than about behaviour.
const ONE_HU: &[u8] = include_bytes!("fixtures/plugin-eval-1huAQG.jsonl");

fn ir(bytes: &[u8]) -> TraceIr {
    read_transcript(bytes).expect("the committed fixture is a transcript this build reads")
}

/// The specification the eval ships, as the eval ships it.
fn eval_spec() -> trace_domain::spec::TraceSpec {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../integrations/claude-code/eval/expectations.trace.yaml"
    );
    let text = std::fs::read_to_string(path).expect("the eval's specification is committed");
    trace_domain::raw::read_spec(&text)
        .unwrap_or_else(|errors| panic!("the eval's specification must validate:\n{errors}"))
}

#[test]
fn the_evals_own_specification_passes_against_both_committed_runs() {
    // The claim the wave is for: the five transcript assertions `run.sh` grew in three idioms are
    // now one document, and it holds against the two runs that were 9/9 by hand.
    for (label, bytes) in [("7hTYjT", SEVEN_H), ("1huAQG", ONE_HU)] {
        let report = check(&eval_spec(), &ir(bytes), &[]);
        assert_eq!(
            report.exit_code(),
            0,
            "{label}: {}\n{}",
            trace_spec::render::verdict_sentence(&report),
            trace_spec::render::report_to_text(&report)
        );
        assert_eq!(report.summary.gap, 0, "{label} gapped");
        assert_eq!(
            report.summary.unknown, 0,
            "{label} left something undecidable"
        );
    }
}

#[test]
fn the_evals_specification_downgraded_on_the_command_line_still_reports_the_row() {
    // The `EVAL_USE_API_KEY=1` escape. The expectation is not skipped — it is evaluated, printed
    // and named in the report; it simply stops gating. A skipped check reads exactly like a
    // passing one, which is the failure mode `AGENTS.md` § Gate names.
    let mut spec = eval_spec();
    let unknown = spec.mark_advisory(&BTreeSet::from(["billed-to-the-session".to_owned()]));
    assert!(unknown.is_empty(), "the id the eval passes must exist");
    let report = check(&spec, &ir(SEVEN_H), &["billed-to-the-session".to_owned()]);
    let row = report
        .expectations
        .iter()
        .find(|row| row.id == "billed-to-the-session")
        .expect("the row is still in the report");
    assert!(!row.gates(), "it no longer gates");
    assert_eq!(row.verdict, Verdict::Ok, "and it was still evaluated");
    assert_eq!(report.advisory_overrides, vec!["billed-to-the-session"]);
    assert!(
        trace_spec::render::report_to_text(&report).contains("downgraded to advisory"),
        "the text report says so out loud"
    );
}

#[test]
fn a_transcript_the_adapter_could_not_fully_read_is_undecided_rather_than_green() {
    // Design D1's accepted cost, demonstrated: a harness upgrade that renames an event turns a
    // green run into an exit 3. Somebody should look — and the report says which events it could
    // not read.
    let mut lines: Vec<String> = String::from_utf8(SEVEN_H.to_vec())
        .expect("the fixture is UTF-8")
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    lines.push(r#"{"type":"tool_stream_v2","payload":{"name":"Bash"}}"#.to_owned());
    let doctored = lines.join("\n");
    let report = check(&eval_spec(), &ir(doctored.as_bytes()), &[]);
    assert_eq!(
        report.exit_code(),
        3,
        "an unread event must not read as a pass:\n{}",
        trace_spec::render::report_to_text(&report)
    );
    let reason = report
        .expectations
        .iter()
        .find_map(|row| match &row.outcome {
            Outcome::Undecidable(reason @ UnknownReason::OpaqueEvents { .. }) => Some(reason),
            _ => None,
        })
        .expect("at least one row names the events it could not read");
    assert!(
        reason.to_string().contains("tool_stream_v2"),
        "the reason names the type: {reason}"
    );
}

#[test]
fn the_same_transcript_and_specification_produce_a_byte_identical_report() {
    // Invariant 9, over the pair. No clock is read: every duration and every cost comes out of
    // the transcript.
    let render = |report: &CheckReport| {
        (
            serde_json::to_string(report).expect("a report serializes"),
            trace_spec::render::report_to_text(report),
        )
    };
    let first = render(&check(&eval_spec(), &ir(SEVEN_H), &[]));
    let second = render(&check(&eval_spec(), &ir(SEVEN_H), &[]));
    assert_eq!(first, second);
    let other = render(&check(&eval_spec(), &ir(ONE_HU), &[]));
    assert_ne!(
        first.0, other.0,
        "two runs are two reports — the transcript digest alone makes them different"
    );
}
