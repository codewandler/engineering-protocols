//! The text renderings: a report for a person, and a census for a person.
//!
//! Text for people, JSON for programs, and no third rendering — the repository's standing rule.
//! The JSON is [`CheckReport`]'s own `Serialize`, so the two cannot drift: there is one value and
//! two ways of printing it, not two producers of one answer.
//!
//! # The footer is part of the design
//!
//! Design decision **D3**: redaction is opt-in, and the un-redacted report *says what it
//! contains*. A report is most useful with its evidence visible, and a checker that hides evidence
//! by default is one people stop trusting — but pasting a transcript's command strings and file
//! paths into a pull request should be a decision rather than an accident. So the plain rendering
//! ends with one line naming what is in it and how to turn it off, and the redacted rendering
//! says it is redacted.

use std::fmt::Write as _;

use trace_domain::ir::Census;
use trace_domain::spec::Severity;

use crate::report::{CheckReport, Verdict};

/// How many characters of a digest a heading shows.
///
/// Twelve is enough to recognise a run across two reports and short enough to leave the line
/// readable; the full digest is in the JSON, and in the footer.
const DIGEST_PREFIX: usize = 12;

/// A check report, for a person.
///
/// The shape is the design's § 4 console output: a heading naming the specification, the run and
/// the counts, then one line per expectation with its verdict, its id and what the transcript
/// said.
#[must_use]
pub fn report_to_text(report: &CheckReport) -> String {
    let mut out = String::new();
    let summary = &report.summary;
    let _ = writeln!(
        out,
        "{} against transcript sha256:{}… — {} ok, {} gap, {} unk",
        report.spec_id,
        &report.transcript_digest[..DIGEST_PREFIX.min(report.transcript_digest.len())],
        summary.ok,
        summary.gap,
        summary.unknown
    );
    if let Some(title) = &report.spec_title {
        let _ = writeln!(out, "  {title}");
    }

    let width = report
        .expectations
        .iter()
        .map(|expectation| expectation.id.len())
        .max()
        .unwrap_or(0);
    for expectation in &report.expectations {
        let status = match (expectation.verdict, expectation.severity) {
            (verdict, Severity::Gate) => verdict.as_str().to_owned(),
            (verdict, Severity::Advisory) => format!("{} (adv)", verdict.as_str()),
        };
        let _ = writeln!(
            out,
            "  {status:<9} {:<width$}  {}",
            expectation.id,
            expectation.outcome.detail()
        );
    }

    let _ = writeln!(
        out,
        "spec sha256:{}…  adapter {}",
        &report.spec_digest[..DIGEST_PREFIX.min(report.spec_digest.len())],
        report.adapter.name
    );
    if !report.advisory_overrides.is_empty() {
        let _ = writeln!(
            out,
            "note: downgraded to advisory on the command line: {} — the specification's digest is \
             the document as authored",
            report.advisory_overrides.join(", ")
        );
    }
    let _ = writeln!(out, "{}", footer(report));
    out
}

/// The line that says what this report contains.
fn footer(report: &CheckReport) -> String {
    if report.redacted {
        format!(
            "note: redacted — every citation is an event index and a digest, no transcript \
             content. Transcript sha256:{}",
            report.transcript_digest
        )
    } else {
        format!(
            "note: this report quotes command strings and file paths read out of the transcript; \
             `--redact` replaces them with digests. Transcript sha256:{}",
            report.transcript_digest
        )
    }
}

/// The sentence that says what the verdict means and which exit code it produced.
///
/// Mirrors `ess conform`'s own verdict sentence, and for its reason: a reader should not have to
/// look up what exit 3 means, and a harness should not have to guess whether it is a softer 1.
#[must_use]
pub fn verdict_sentence(report: &CheckReport) -> String {
    match report.verdict {
        Verdict::Ok if report.summary.unknown > 0 => format!(
            "conformant: every gating expectation holds; {} advisory or undecidable row(s) are \
             reported and gate nothing (exit 0)",
            report.summary.unknown + report.summary.advisory_gap
        ),
        Verdict::Ok => {
            "conformant: the run satisfies every expectation the specification states (exit 0)"
                .to_owned()
        }
        Verdict::Gap => format!(
            "not conformant: the run contradicted {} expectation(s) — {} (exit 1)",
            report
                .expectations
                .iter()
                .filter(|row| row.verdict == Verdict::Gap && row.gates())
                .count(),
            report
                .expectations
                .iter()
                .filter(|row| row.verdict == Verdict::Gap && row.gates())
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Verdict::Unknown => format!(
            "undecided: nothing was contradicted and {} expectation(s) could not be judged from \
             this transcript — somebody should look at the format, not at the agent (exit 3)",
            report
                .expectations
                .iter()
                .filter(|row| row.verdict == Verdict::Unknown && row.gates())
                .count()
        ),
    }
}

/// A run's census, for a person.
///
/// The eval's informational metrics block, printed by a verb instead of by sixty-five lines of
/// `jq`. It states quantities and no opinions.
#[must_use]
pub fn census_to_text(census: &Census) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "transcript   sha256:{}", census.transcript_digest);
    let families: Vec<String> = census
        .events_by_family
        .iter()
        .map(|(family, count)| format!("{count} {family}"))
        .collect();
    let _ = writeln!(
        out,
        "events       {} total — {}",
        census.events,
        families.join(", ")
    );
    let _ = writeln!(
        out,
        "unread       {} event(s) the adapter could not read",
        census.opaque_events
    );
    let _ = writeln!(
        out,
        "requests     {} assistant events, {} api requests",
        census.assistant_events, census.api_requests
    );
    for (tool, traffic) in &census.tool_traffic {
        let _ = writeln!(
            out,
            "tool         {tool}: {} call(s), {} error(s), in {}B, results {}B",
            traffic.calls, traffic.errors, traffic.input_bytes, traffic.result_bytes
        );
    }
    let total_results: usize = census
        .tool_traffic
        .values()
        .map(|traffic| traffic.result_bytes)
        .sum();
    let total_calls: usize = census
        .tool_traffic
        .values()
        .map(|traffic| traffic.calls)
        .sum();
    let _ = writeln!(
        out,
        "tools-total  {total_calls} call(s), results {total_results}B into context"
    );
    let _ = writeln!(
        out,
        "repeated     {} identical call group(s)",
        census.repeated_call_groups
    );
    for (position, step) in census.steps.iter().enumerate() {
        let _ = writeln!(
            out,
            "step         {}. {} (event {}): gen {}, exec {}",
            position + 1,
            step.tool,
            step.call_event,
            step.gen_ms
                .map_or_else(|| "?".to_owned(), |ms| format!("{ms}ms")),
            step.exec_ms
                .map_or_else(|| "?".to_owned(), |ms| format!("{ms}ms")),
        );
    }
    let _ = writeln!(
        out,
        "time-split   inference {}, tool-exec {} across {} step(s)",
        census
            .inference_total_ms
            .map_or_else(|| "?".to_owned(), |ms| format!("{ms}ms")),
        census
            .tool_exec_total_ms
            .map_or_else(|| "?".to_owned(), |ms| format!("{ms}ms")),
        census.steps.len()
    );
    out
}

#[cfg(test)]
mod tests {
    use trace_domain::ir::AdapterRef;
    use trace_domain::spec::OnUnknown;

    use super::*;
    use crate::report::{Citation, ExpectationReport, Outcome, UnknownReason};

    fn report(redacted: bool) -> CheckReport {
        let rows = vec![
            ExpectationReport::new(
                "billed-to-the-session".to_owned(),
                None,
                "env.api_key_source",
                Severity::Gate,
                OnUnknown::Unknown,
                Outcome::Ok(Citation::new(vec![0], "api_key_source = none")),
            ),
            ExpectationReport::new(
                "within-budget".to_owned(),
                None,
                "cost.total",
                Severity::Advisory,
                OnUnknown::Unknown,
                Outcome::Gap(Citation::new(vec![35], "cost = $3.1000, at most 1")),
            ),
            ExpectationReport::new(
                "ttft-under-2s".to_owned(),
                None,
                "ttft",
                Severity::Gate,
                OnUnknown::Unknown,
                Outcome::Undecidable(UnknownReason::FieldAbsent {
                    field: "ttft_ms".to_owned(),
                }),
            ),
        ];
        let report = CheckReport::new(
            "planning-plugin/eval".to_owned(),
            None,
            "a".repeat(64),
            "b".repeat(64),
            AdapterRef {
                name: "claude-code/stream-json",
                written_against: &["2.1.238"],
            },
            Vec::new(),
            rows,
        );
        if redacted {
            report.redact()
        } else {
            report
        }
    }

    #[test]
    fn the_text_report_prints_one_line_per_expectation_and_marks_the_advisory_ones() {
        let text = report_to_text(&report(false));
        assert!(text.starts_with("planning-plugin/eval against transcript sha256:bbbbbbbbbbbb… — 1 ok, 1 gap, 1 unk\n"), "{text}");
        assert!(
            text.contains("\n  ok        billed-to-the-session"),
            "{text}"
        );
        assert!(
            text.contains("gap (adv) within-budget"),
            "an advisory row is visibly advisory, not hidden: {text}"
        );
        assert!(text.contains("unk       ttft-under-2s"), "{text}");
        assert_eq!(
            text.lines().count(),
            6,
            "heading, three rows, the spec line and the footer"
        );
    }

    #[test]
    fn the_plain_footer_says_what_the_report_contains_and_the_redacted_one_says_it_is_redacted() {
        // Design D3: the default is un-redacted and it warns, so pasting one somewhere public is
        // a decision rather than an accident.
        let plain = report_to_text(&report(false));
        assert!(plain.contains("--redact"), "{plain}");
        assert!(plain.contains("command strings and file paths"), "{plain}");
        let hidden = report_to_text(&report(true));
        assert!(hidden.contains("redacted"), "{hidden}");
        assert!(!hidden.contains("--redact"), "{hidden}");
        assert!(
            !hidden.contains("$3.1000"),
            "the note was a digest before it was printed: {hidden}"
        );
    }

    #[test]
    fn the_verdict_sentence_names_the_exit_code_and_which_expectations_gapped() {
        let gapping = report(false);
        assert_eq!(
            gapping.verdict,
            Verdict::Unknown,
            "the only gap is advisory, so the run is undecided rather than contradicted"
        );
        let sentence = verdict_sentence(&gapping);
        assert!(sentence.contains("exit 3"), "{sentence}");
        assert!(
            sentence.contains("not at the agent"),
            "exit 3 wakes a different person than exit 1: {sentence}"
        );
    }
}
