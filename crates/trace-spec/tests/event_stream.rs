//! The event-stream reader against two whole driven steps, and the three shipped documents.
//!
//! # These two fixtures are **synthesized**, and that is the difference from `adapter.rs`
//!
//! The `stream-json` fixtures beside them are real eval runs, committed verbatim, and the tests
//! over them assert numbers a paid run actually produced. These two are not: they are written by
//! hand against metaharness's own emitted stream — its
//! `crates/metaharness-claude/fixtures/c2/session.expected.jsonl`, read on 2026-08-22 — and they
//! are structurally faithful rather than observed. Every payload field is present, an absent one
//! is an explicit `null`, `at` is omitted where a vendor recorded no timestamp, and the seam's
//! `tool.decided` events carry the reasons this repository's own `decide_tool` policy writes.
//!
//! What that means for a reader of a failing assertion here: a number in this file is a number
//! **this file chose**, so a mismatch is a change in the reader and never a change in a harness.
//! The moment a real driven run is committed, it belongs beside these and these become what they
//! are — a shape test.
//!
//! # What each fixture is for
//!
//! | fixture | the step it stands for |
//! |---|---|
//! | `metaharness-driven-honest-step.jsonl` | the session asked to do the ordinary thing: loads the skill, creates through the CLI, validates, and is refused once for chaining a command line |
//! | `metaharness-driven-denial-step.jsonl` | the session induced to hand-edit frontmatter and to reach outside the driven surface: three calls, three denials, no result the guardrails did not intend |

use trace_domain::ir::TraceIr;
use trace_domain::spec::TraceSpec;
use trace_spec::check::check;
use trace_spec::event_stream::read_event_stream;
use trace_spec::reader::{detect, read_any, TranscriptFormat};
use trace_spec::report::{Outcome, UnknownReason, Verdict};

/// The honest driven step.
const HONEST: &[u8] = include_bytes!("fixtures/metaharness-driven-honest-step.jsonl");

/// The deliberate-denial driven step.
const DENIAL: &[u8] = include_bytes!("fixtures/metaharness-driven-denial-step.jsonl");

/// A recorded Claude Code run, which must keep reading through the other adapter untouched.
const RECORDED: &[u8] = include_bytes!("fixtures/plugin-eval-7hTYjT.jsonl");

fn ir(bytes: &[u8]) -> TraceIr {
    read_event_stream(bytes).expect("the committed fixture is an event stream this build reads")
}

/// One of the three shipped expectation documents, as the repository ships it.
fn document(name: &str) -> TraceSpec {
    let path = format!(
        "{}/../../conformance/trace/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{path} is committed"));
    trace_domain::raw::read_spec(&text)
        .unwrap_or_else(|errors| panic!("{name} must validate:\n{errors}"))
}

/// The ids of every row with this verdict, in report order.
fn rows_with(report: &trace_spec::report::CheckReport, verdict: Verdict) -> Vec<&str> {
    report
        .expectations
        .iter()
        .filter(|row| row.verdict == verdict)
        .map(|row| row.id.as_str())
        .collect()
}

#[test]
fn the_driven_step_document_holds_against_a_driven_event_stream() {
    // The story's first acceptance, end to end: the specification the migration left behind is
    // checkable again, without a word of it changing.
    let report = check(
        &document("expectations.driven-step.trace.yaml"),
        &ir(HONEST),
        &[],
    );
    assert_eq!(
        report.exit_code(),
        0,
        "{}",
        trace_spec::render::report_to_text(&report)
    );
    assert_eq!(report.summary.gap, 0, "nothing contradicted");
    assert_eq!(
        rows_with(&report, Verdict::Unknown),
        vec!["the-skill-ran-to-completion"],
        "exactly one row is undecidable, and it is the advisory one that reads a per-tool result \
         field this wire does not carry"
    );
    assert_eq!(
        report.adapter.name, "metaharness/event-stream",
        "the report says which reader judged the run"
    );
}

#[test]
fn the_denial_step_document_holds_against_a_denied_driven_event_stream() {
    // The other half, and the one the seam changed: three refusals taken by this repository's own
    // policy, none of them in the vendor's array, all three counted.
    let report = check(
        &document("expectations.denial-step.trace.yaml"),
        &ir(DENIAL),
        &[],
    );
    assert_eq!(
        report.exit_code(),
        0,
        "{}",
        trace_spec::render::report_to_text(&report)
    );
    assert_eq!(report.summary.gap, 0);
    assert_eq!(report.summary.unknown, 0, "nothing was left undecidable");
}

#[test]
fn the_denials_the_seam_took_are_what_permission_denied_counts() {
    // The load-bearing mapping. `session.ended.permission_denials` is empty in this run — the
    // seam refused all three calls before the vendor's own permission pipeline saw them — so a
    // reader that only read the vendor's array would report the run where enforcement worked as
    // the run where nothing was refused.
    let denial = ir(DENIAL);
    let outcome = denial.run_outcome().expect("a terminal record");
    assert_eq!(outcome.permission_denials, Some(3));

    // And the honest step, where the vendor listed the same refusal the seam took: one denial,
    // not two.
    let honest = ir(HONEST);
    assert_eq!(
        honest
            .run_outcome()
            .expect("a terminal record")
            .permission_denials,
        Some(1),
        "one refused call is one denial however many layers wrote it down"
    );
}

#[test]
fn the_census_of_a_driven_step_counts_the_control_plane_out_and_nothing_as_unread() {
    let census = ir(HONEST).census();
    assert_eq!(
        census.events, 18,
        "twenty-nine lines, eleven of them control plane: step and turn boundaries, four \
         decisions, and the usage records that fold into the request series"
    );
    assert_eq!(
        census.opaque_events, 0,
        "an event with no IR family is not an event nobody could read — routing the control plane \
         through the opaque path would make every count in every driven run `unk`"
    );
    assert_eq!(
        census.api_requests, 3,
        "three `usage` events, three requests"
    );
    assert_eq!(census.tool_traffic["Bash"].calls, 3);
    assert_eq!(
        census.tool_traffic["Bash"].errors, 1,
        "the refused command came back to the model as an error result"
    );
    assert_eq!(census.repeated_call_groups, 0);
}

#[test]
fn the_plugin_eval_document_reports_a_driven_step_as_a_different_run_rather_than_as_a_defect() {
    // The third shipped document is the *interactive* plugin eval's, and a driven step is not its
    // subject. It is checked here anyway, because the useful thing is the list: which of its rows
    // a driven event stream cannot satisfy, and why. Pinned so that a change in either the
    // document or the reader has to face the list rather than discover it in a paid run.
    let report = check(&document("expectations.trace.yaml"), &ir(HONEST), &[]);

    assert_eq!(
        rows_with(&report, Verdict::Gap),
        vec![
            // Three facts about the *run*, not about the reader:
            // a metaharness session stays in the vendor's default posture, because decisions
            // arrive over the seam rather than from a permission mode;
            "the-run-did-not-ask",
            // and the driven surface refused one chained command line, which the driven-step
            // document bounds at two rather than forbidding.
            "the-cli-never-refused-a-shell-call",
            "no-permission-denials",
        ],
        "{}",
        trace_spec::render::report_to_text(&report)
    );

    assert_eq!(
        rows_with(&report, Verdict::Unknown),
        vec![
            // One fact about the reader: metaharness does not carry the vendor's per-tool result
            // sibling, so the `commandName`/`success` pair `skill.completed` reads is not there.
            "skill-completed",
            // Three quantities the wire does not carry at all.
            "one-pass-per-request",
            "thinking-tokens-within-reason",
            "served-at-standard-speed",
        ],
        "{}",
        trace_spec::render::report_to_text(&report)
    );

    let reason = report
        .expectations
        .iter()
        .find(|row| row.id == "skill-completed")
        .map(|row| &row.outcome)
        .expect("the row is in the report");
    assert!(
        matches!(
            reason,
            Outcome::Undecidable(UnknownReason::ResultFieldAbsent { field, .. }) if field == "commandName"
        ),
        "the reason names the field rather than the conclusion: {reason:?}"
    );
}

#[test]
fn a_recorded_transcript_and_a_driven_stream_take_the_same_arguments() {
    // Acceptance 2, at the seam a caller actually meets: one entry point, and the file says which
    // reader it needs. The recorded fixtures still read through the `stream-json` adapter, which
    // is what keeps two years of committed runs checkable.
    assert_eq!(detect(HONEST), TranscriptFormat::MetaharnessEventStream);
    assert_eq!(detect(RECORDED), TranscriptFormat::ClaudeStreamJson);

    let driven = read_any(HONEST).expect("a driven stream reads");
    let recorded = read_any(RECORDED).expect("a recorded transcript still reads");
    assert_eq!(driven.adapter.name, "metaharness/event-stream");
    assert_eq!(recorded.adapter.name, "claude-code/stream-json");
    assert_ne!(
        driven.transcript_digest, recorded.transcript_digest,
        "the digest names the bytes, so two runs are two runs"
    );
}

#[test]
fn the_same_stream_and_specification_produce_a_byte_identical_report() {
    // Invariant 9, on the new reader. Reading a file twice must produce the same IR, and checking
    // it twice must produce the same bytes — a report that moved between runs could not be
    // committed, diffed or used as evidence.
    let specification = document("expectations.driven-step.trace.yaml");
    let first = serde_json::to_string(&check(&specification, &ir(HONEST), &[]))
        .expect("a report renders as JSON");
    let second = serde_json::to_string(&check(&specification, &ir(HONEST), &[]))
        .expect("a report renders as JSON");
    assert_eq!(first, second);
    assert_eq!(ir(HONEST), ir(HONEST), "and the IR itself is stable");
}
