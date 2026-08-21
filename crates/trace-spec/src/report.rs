//! What a check produces: three verdicts, each carrying what that verdict owes.
//!
//! # The shape that makes invariant 5 structural
//!
//! [`Outcome`] has three variants, and the type refuses a verdict that says nothing:
//!
//! | outcome | carries | so it is impossible to |
//! |---|---|---|
//! | [`Ok`](Outcome::Ok) | the events that satisfied it | claim a pass nobody can check |
//! | [`Gap`](Outcome::Gap) | the events that contradicted it | report a failure nobody can act on |
//! | [`Undecidable`](Outcome::Undecidable) | the reason the transcript cannot decide | write `unk` and mean "false" |
//!
//! There is no `Outcome::from_bool`, no `Option<Citation>` beside a separate verdict field, and
//! no way to build a `Gap` without citing something. That is the enforcement invariant 5 asks
//! for, expressed the way `infra-spec`'s `Outcome` expresses it — by having no other shape
//! available.
//!
//! # The report is what a later evidence record is minted from
//!
//! [`CheckReport`] is the value `protocol trace evidence` consumes: the counts, the id of every
//! expectation that gapped, and — first-class, never derived at the call site — the **transcript
//! digest** and the **specification digest**. That pair is what makes an evidence record mean
//! something later: *"some agent passed some behavioural spec"* is worthless, and *"the run with
//! this digest satisfied the spec with that digest"* is not.
//!
//! # Redaction
//!
//! A transcript contains the prompt, the model's reasoning, file contents it read and commands it
//! ran. That is more sensitive than any other input this repository consumes, and a report is a
//! thing people paste into pull requests. [`CheckReport::redact`] replaces every citation's note
//! with a digest of it, leaving the event indices and both content digests intact — so every
//! verdict remains checkable by anyone holding the transcript, and nothing about the run leaks to
//! anyone who does not. The redacted report is still deterministic and still content-addressed,
//! so it can be committed.
//!
//! Redaction touches **notes only**, and that is a boundary worth stating: an
//! [`UnknownReason`] names specification vocabulary, field names and event indices, never a value
//! the run produced. Everything the transcript *said* goes in a note.

use std::fmt;

use serde::Serialize;
use trace_domain::digest::digest_of_bytes;
use trace_domain::ir::AdapterRef;
use trace_domain::spec::{OnUnknown, Severity};

/// The format string a persisted check report carries.
pub const REPORT_FORMAT: &str = "trace-report/1";

/// How many hex characters of a note's digest a redacted report shows.
///
/// Twelve, not sixty-four: the redacted note exists so two reports about one run are comparable
/// and a reader can see that two verdicts cite different content, not so anyone can brute-force
/// a command line out of it. The full digests that carry weight — the transcript's and the
/// specification's — stay at their full width.
const REDACTED_PREFIX: usize = 12;

/// One of the three answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The transcript satisfies the expectation.
    Ok,
    /// The transcript contradicts it.
    Gap,
    /// The transcript cannot decide it.
    ///
    /// Deliberately not a softer gap. *"The agent did the wrong thing"* and *"the transcript
    /// format moved under us"* want different people to be woken up.
    Unknown,
}

impl Verdict {
    /// The three-letter word a text report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Gap => "gap",
            Self::Unknown => "unk",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a verdict points at: the events that produced it, and what they said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Citation {
    /// The IR event indices, in order.
    ///
    /// May be empty for a fact that is a property of the transcript as a whole rather than of an
    /// event — how many API requests it made, for instance. It is never empty for a fact read off
    /// an event.
    pub events: Vec<usize>,
    /// What those events said, in one line.
    ///
    /// Transcript-derived, so this is the field [`CheckReport::redact`] replaces.
    pub note: String,
}

impl Citation {
    /// Builds one.
    pub fn new(events: Vec<usize>, note: impl Into<String>) -> Self {
        Self {
            events,
            note: note.into(),
        }
    }

    /// A citation for a whole-run fact, with no single event behind it.
    pub fn run(note: impl Into<String>) -> Self {
        Self::new(Vec::new(), note)
    }

    /// The same citation with its note replaced by a digest of it.
    fn redacted(self) -> Self {
        let digest = digest_of_bytes(self.note.as_bytes());
        Self {
            events: self.events,
            note: format!("sha256:{}", &digest[..REDACTED_PREFIX]),
        }
    }
}

impl fmt::Display for Citation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.events.is_empty() {
            return f.write_str(&self.note);
        }
        let rendered: Vec<String> = self.events.iter().map(ToString::to_string).collect();
        let plural = if self.events.len() == 1 {
            "event"
        } else {
            "events"
        };
        write!(f, "{} at {plural} {}", self.note, rendered.join(", "))
    }
}

/// Why a transcript cannot decide an expectation.
///
/// Closed, and every variant names something a reader can go and look at: an event index, a field
/// the harness did not record, a scope that selected nothing. *"Unknown"* on its own would name
/// none of them, which is the whole complaint against a two-valued checker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
#[non_exhaustive]
pub enum UnknownReason {
    /// The transcript carries no opening record, so nothing about the environment can be read.
    NoSessionStart,
    /// The transcript carries no terminal record.
    ///
    /// A transcript truncated by a crash has none, and that is exactly the case that must not
    /// read as a failed assertion.
    NoRunOutcome,
    /// The transcript carries no rate-limit event.
    NoRateLimitEvent,
    /// The transcript carries no thinking estimate.
    NoThinkingEstimate,
    /// The run produced no final assistant text.
    NoFinalText,
    /// The record the expectation reads carries no such field.
    ///
    /// The commonest reason, and the one design D1 predicts: a format that is not a stable public
    /// schema renames and drops fields between versions, and an absent one means this transcript
    /// cannot answer the question.
    FieldAbsent {
        /// The field, in dotted form: `usage.cache_read_input_tokens`.
        field: String,
    },
    /// The selector picked no tool call at all.
    ///
    /// Deliberately not vacuous truth. An expectation must not be able to pass by selecting
    /// nothing — the same rule `infra-spec` applies to a scope that matches no workload.
    NothingInScope {
        /// The selector, as the document wrote it.
        selector: String,
    },
    /// One side of an ordering never happened.
    ///
    /// "A before B" is undecidable when there is no A, and reporting it as a failure blames the
    /// wrong thing.
    NeverOccurred {
        /// The selector that matched nothing.
        selector: String,
    },
    /// A call matched and no result was correlated to it.
    ///
    /// A truncated transcript, which is not the same as a bad result.
    NoResultCorrelated {
        /// The event that carried the call.
        call_event: usize,
    },
    /// A result came back and does not carry the field the matcher names.
    ResultFieldAbsent {
        /// The event that carried the call.
        call_event: usize,
        /// The event that carried the result.
        result_event: usize,
        /// The field the matcher names.
        field: String,
    },
    /// A duration could not be derived because a timestamp was not recorded.
    ///
    /// Never zero, and never a value obtained by timing something.
    TimestampAbsent {
        /// The event whose interval could not be derived.
        event: usize,
    },
    /// The adapter met events it could not read, and one of them could have been a tool call.
    ///
    /// The conservative reading, and the design's whole argument: a checker that reported *"the
    /// tool was never called"* when it had stopped being able to see tool calls would be lying.
    /// It is only raised where an unseen event could change the answer — a lower bound already
    /// met cannot be unmet by an event nobody read.
    OpaqueEvents {
        /// The events, by index.
        events: Vec<usize>,
        /// The types they declared, where they declared one.
        types: Vec<String>,
    },
    /// An expectation was scoped to a model the run never used.
    ///
    /// `unk`, not `ok`: an expectation must not be able to pass by selecting nothing.
    ModelNotUsed {
        /// The model named.
        model: String,
    },
    /// A ratio has no denominator in this transcript.
    ///
    /// A rate over zero is not zero.
    RatioUndefined {
        /// What the denominator was, in words.
        denominator: String,
    },
}

impl fmt::Display for UnknownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSessionStart => f.write_str("this transcript records no session start"),
            Self::NoRunOutcome => f.write_str("this transcript records no terminal result"),
            Self::NoRateLimitEvent => f.write_str("this transcript records no rate-limit state"),
            Self::NoThinkingEstimate => f.write_str("this transcript records no thinking estimate"),
            Self::NoFinalText => f.write_str("this run produced no final assistant text"),
            Self::FieldAbsent { field } => write!(f, "this transcript records no `{field}`"),
            Self::NothingInScope { selector } => {
                write!(f, "no tool call matches {selector}")
            }
            Self::NeverOccurred { selector } => {
                write!(
                    f,
                    "{selector} never occurred, so the ordering is undecidable"
                )
            }
            Self::NoResultCorrelated { call_event } => write!(
                f,
                "the call at event {call_event} has no correlated result — a truncated \
                 transcript, not a bad result"
            ),
            Self::ResultFieldAbsent {
                call_event,
                result_event,
                field,
            } => write!(
                f,
                "the result at event {result_event} for the call at event {call_event} records \
                 no `{field}`"
            ),
            Self::TimestampAbsent { event } => write!(
                f,
                "no duration can be derived around event {event}: a timestamp was not recorded"
            ),
            Self::OpaqueEvents { events, types } => {
                let rendered: Vec<String> = events.iter().map(ToString::to_string).collect();
                let kinds = if types.is_empty() {
                    "untyped".to_owned()
                } else {
                    types.join(", ")
                };
                write!(
                    f,
                    "the adapter could not read events {} ({kinds}), and one of them could have \
                     changed this answer",
                    rendered.join(", ")
                )
            }
            Self::ModelNotUsed { model } => {
                write!(f, "this run never used `{model}`")
            }
            Self::RatioUndefined { denominator } => {
                write!(
                    f,
                    "the ratio has no denominator here: {denominator} is zero"
                )
            }
        }
    }
}

/// What the transcript said about one expectation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Satisfied, and here is what satisfied it.
    Ok(Citation),
    /// Contradicted, and here is what contradicted it.
    Gap(Citation),
    /// Undecidable, and here is why.
    Undecidable(UnknownReason),
}

impl Outcome {
    /// The verdict this outcome is, before any policy is applied.
    ///
    /// Derived from the variant rather than stored beside it, so a future rule cannot produce a
    /// gap without citing something or an unknown without saying why.
    pub fn verdict(&self) -> Verdict {
        match self {
            Self::Ok(_) => Verdict::Ok,
            Self::Gap(_) => Verdict::Gap,
            Self::Undecidable(_) => Verdict::Unknown,
        }
    }

    /// The one-line explanation a report prints beside the verdict.
    pub fn detail(&self) -> String {
        match self {
            Self::Ok(citation) | Self::Gap(citation) => citation.to_string(),
            Self::Undecidable(reason) => reason.to_string(),
        }
    }

    /// The events it cites, if any.
    pub fn events(&self) -> &[usize] {
        match self {
            Self::Ok(citation) | Self::Gap(citation) => &citation.events,
            Self::Undecidable(_) => &[],
        }
    }

    /// The same outcome with any transcript-derived note replaced by a digest of it.
    fn redacted(self) -> Self {
        match self {
            Self::Ok(citation) => Self::Ok(citation.redacted()),
            Self::Gap(citation) => Self::Gap(citation.redacted()),
            undecidable @ Self::Undecidable(_) => undecidable,
        }
    }
}

/// One expectation's row in the report.
///
/// `#[non_exhaustive]` because [`Self::verdict`] is *derived* from the outcome and the
/// expectation's `on_unknown` policy, and a struct literal built outside this crate could set the
/// two so they disagree. [`Self::new`] is the only constructor, and it derives it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct ExpectationReport {
    /// The id the document gave it.
    pub id: String,
    /// The author's own sentence, where they wrote one.
    pub statement: Option<String>,
    /// Which kind it is, by the name the document writes.
    pub kind: &'static str,
    /// Whether its verdict moves the exit code.
    pub severity: Severity,
    /// What the transcript said.
    pub outcome: Outcome,
    /// The verdict after the expectation's `on_unknown` policy is applied.
    ///
    /// Equal to `outcome.verdict()` except where the document declared `on_unknown: gap` and the
    /// transcript could not decide — which is how a specification says *"if this transcript
    /// cannot tell me, that is itself the failure"*.
    pub verdict: Verdict,
}

impl ExpectationReport {
    /// Builds one, deriving the verdict from the outcome and the policy.
    pub fn new(
        id: String,
        statement: Option<String>,
        kind: &'static str,
        severity: Severity,
        on_unknown: OnUnknown,
        outcome: Outcome,
    ) -> Self {
        let verdict = match (outcome.verdict(), on_unknown) {
            (Verdict::Unknown, OnUnknown::Gap) => Verdict::Gap,
            (verdict, _) => verdict,
        };
        Self {
            id,
            statement,
            kind,
            severity,
            outcome,
            verdict,
        }
    }

    /// `true` when this row moves the exit code.
    pub fn gates(&self) -> bool {
        self.severity == Severity::Gate
    }
}

/// How many of each verdict, and how many of those were advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Summary {
    /// How many expectations were evaluated.
    pub total: usize,
    /// How many hold.
    pub ok: usize,
    /// How many are contradicted.
    pub gap: usize,
    /// How many are undecidable.
    pub unknown: usize,
    /// Of the gaps, how many are advisory and therefore did not move the exit code.
    pub advisory_gap: usize,
    /// Of the unknowns, how many are advisory.
    pub advisory_unknown: usize,
}

/// The answer: what this specification says about this run.
///
/// `#[non_exhaustive]` for [`ExpectationReport`]'s reason — the summary and the overall verdict
/// are derived from the rows, and a literal built elsewhere could make them disagree.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct CheckReport {
    /// The format claim, `trace-report/1`.
    pub format: &'static str,
    /// The specification's id.
    pub spec_id: String,
    /// The specification's title, where it has one.
    pub spec_title: Option<String>,
    /// The digest of the specification **as authored** — a first-class field, because an evidence
    /// record built from this report is worthless without it.
    pub spec_digest: String,
    /// The digest of the transcript's raw bytes — recomputable with `sha256sum` by anyone holding
    /// the file.
    pub transcript_digest: String,
    /// Which adapter read the transcript, and which harness versions it was written against.
    pub adapter: AdapterRef,
    /// Whether the notes have been replaced by digests.
    pub redacted: bool,
    /// Expectation ids a caller downgraded to advisory on the command line.
    ///
    /// Named rather than folded into the severities, because the specification's digest is the
    /// digest of the document *as authored*: this list is how a reader sees that the run gated on
    /// something narrower than the document says.
    pub advisory_overrides: Vec<String>,
    /// The counts.
    pub summary: Summary,
    /// The overall verdict, over the gating expectations alone.
    pub verdict: Verdict,
    /// Every expectation, in the order the document declares them.
    pub expectations: Vec<ExpectationReport>,
}

impl CheckReport {
    /// Builds a report from its rows, deriving the summary and the overall verdict.
    ///
    /// The overall verdict is over the **gating** rows only: an advisory gap is reported, printed
    /// and counted, and does not move it. A gap beats an unknown — something *was* observed to be
    /// wrong, which is the same fold `Truth::and` performs and the same rule `infra-spec` states.
    pub fn new(
        spec_id: String,
        spec_title: Option<String>,
        spec_digest: String,
        transcript_digest: String,
        adapter: AdapterRef,
        advisory_overrides: Vec<String>,
        expectations: Vec<ExpectationReport>,
    ) -> Self {
        let mut summary = Summary {
            total: expectations.len(),
            ..Summary::default()
        };
        let mut verdict = Verdict::Ok;
        for expectation in &expectations {
            match expectation.verdict {
                Verdict::Ok => summary.ok += 1,
                Verdict::Gap => {
                    summary.gap += 1;
                    if expectation.gates() {
                        verdict = Verdict::Gap;
                    } else {
                        summary.advisory_gap += 1;
                    }
                }
                Verdict::Unknown => {
                    summary.unknown += 1;
                    if expectation.gates() {
                        if verdict == Verdict::Ok {
                            verdict = Verdict::Unknown;
                        }
                    } else {
                        summary.advisory_unknown += 1;
                    }
                }
            }
        }
        Self {
            format: REPORT_FORMAT,
            spec_id,
            spec_title,
            spec_digest,
            transcript_digest,
            adapter,
            redacted: false,
            advisory_overrides,
            summary,
            verdict,
            expectations,
        }
    }

    /// The process exit code this report calls for.
    ///
    /// | code | meaning |
    /// |---|---|
    /// | `0` | every gating expectation holds |
    /// | `1` | at least one gating gap — the run contradicted the specification |
    /// | `3` | no gating gaps, and at least one gating unknown |
    ///
    /// Mirrors `ess conform`, which is the existing precedent: *`0` conformant, `1` contradicted,
    /// `3` nobody found out*. Exit 3 is not a softer exit 1 — a CI job may choose to treat it as
    /// a failure, and the checker refuses to make that choice on the job's behalf.
    pub fn exit_code(&self) -> u8 {
        match self.verdict {
            Verdict::Ok => 0,
            Verdict::Gap => 1,
            Verdict::Unknown => 3,
        }
    }

    /// Every gapped expectation's id — what an evidence record's body carries.
    pub fn gapped(&self) -> Vec<&str> {
        self.expectations
            .iter()
            .filter(|expectation| expectation.verdict == Verdict::Gap)
            .map(|expectation| expectation.id.as_str())
            .collect()
    }

    /// The same report with every transcript-derived note replaced by a digest of it.
    #[must_use]
    pub fn redact(mut self) -> Self {
        self.expectations = self
            .expectations
            .into_iter()
            .map(|mut expectation| {
                expectation.outcome = expectation.outcome.redacted();
                expectation
            })
            .collect();
        self.redacted = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, severity: Severity, outcome: Outcome) -> ExpectationReport {
        ExpectationReport::new(
            id.to_owned(),
            None,
            "tool.called",
            severity,
            OnUnknown::Unknown,
            outcome,
        )
    }

    fn report(rows: Vec<ExpectationReport>) -> CheckReport {
        CheckReport::new(
            "planning-plugin/eval".to_owned(),
            None,
            "spec-digest".to_owned(),
            "transcript-digest".to_owned(),
            AdapterRef {
                name: "test",
                written_against: &["0"],
            },
            Vec::new(),
            rows,
        )
    }

    #[test]
    fn a_gap_beats_an_unknown_because_something_was_observed_to_be_wrong() {
        let report = report(vec![
            row(
                "a",
                Severity::Gate,
                Outcome::Undecidable(UnknownReason::NoRunOutcome),
            ),
            row("b", Severity::Gate, Outcome::Gap(Citation::run("two"))),
        ]);
        assert_eq!(report.verdict, Verdict::Gap);
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.gapped(), vec!["b"]);
    }

    #[test]
    fn an_unknown_with_no_gap_beside_it_is_exit_three_and_not_a_softer_exit_one() {
        let report = report(vec![
            row("a", Severity::Gate, Outcome::Ok(Citation::run("one"))),
            row(
                "b",
                Severity::Gate,
                Outcome::Undecidable(UnknownReason::NoRateLimitEvent),
            ),
        ]);
        assert_eq!(report.verdict, Verdict::Unknown);
        assert_eq!(
            report.exit_code(),
            3,
            "\"the agent did the wrong thing\" and \"the format moved under us\" wake different \
             people"
        );
    }

    #[test]
    fn an_advisory_gap_is_reported_and_counted_and_does_not_move_the_exit_code() {
        // The distinction the whole severity idea rests on: an advisory expectation is *not* a
        // disabled one. It is evaluated, it is in the report, and a reader sees it.
        let report = report(vec![
            row("a", Severity::Gate, Outcome::Ok(Citation::run("one"))),
            row(
                "cost",
                Severity::Advisory,
                Outcome::Gap(Citation::run("$3.10 over a $1.00 bound")),
            ),
        ]);
        assert_eq!(report.verdict, Verdict::Ok);
        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.summary.gap, 1, "the gap is counted");
        assert_eq!(report.summary.advisory_gap, 1, "and named as advisory");
        assert_eq!(
            report.gapped(),
            vec!["cost"],
            "and it is still in the gapped list a reader reads"
        );
    }

    #[test]
    fn on_unknown_gap_turns_an_undecidable_verdict_into_a_failure_for_that_expectation_alone() {
        let strict = ExpectationReport::new(
            "must-record-its-cost".to_owned(),
            None,
            "cost.total",
            Severity::Gate,
            OnUnknown::Gap,
            Outcome::Undecidable(UnknownReason::FieldAbsent {
                field: "total_cost_usd".to_owned(),
            }),
        );
        assert_eq!(strict.outcome.verdict(), Verdict::Unknown, "what was seen");
        assert_eq!(strict.verdict, Verdict::Gap, "what the document asked for");
        assert_eq!(report(vec![strict]).exit_code(), 1);
    }

    #[test]
    fn redaction_keeps_the_indices_and_both_digests_and_replaces_only_the_notes() {
        let report = report(vec![row(
            "created-through-the-cli",
            Severity::Gate,
            Outcome::Ok(Citation::new(
                vec![11, 13],
                "Bash(command ~ \"protocol artifact new\") in /home/someone/secret-project",
            )),
        )])
        .redact();
        assert!(report.redacted);
        let note = match &report.expectations[0].outcome {
            Outcome::Ok(citation) => citation,
            other => panic!("the outcome survives redaction: {other:?}"),
        };
        assert_eq!(note.events, vec![11, 13], "the indices stay");
        assert!(
            note.note.starts_with("sha256:") && note.note.len() == 7 + REDACTED_PREFIX,
            "the note is a digest, not a truncation: {}",
            note.note
        );
        assert!(
            !note.note.contains("secret-project"),
            "nothing about the run leaks to somebody who does not hold the transcript"
        );
        assert_eq!(report.transcript_digest, "transcript-digest");
        assert_eq!(report.spec_digest, "spec-digest");
    }

    #[test]
    fn an_undecidable_verdict_survives_redaction_because_a_reason_names_no_payload() {
        let report = report(vec![row(
            "a",
            Severity::Gate,
            Outcome::Undecidable(UnknownReason::FieldAbsent {
                field: "apiKeySource".to_owned(),
            }),
        )])
        .redact();
        assert!(
            report.expectations[0]
                .outcome
                .detail()
                .contains("apiKeySource"),
            "a field name is specification vocabulary, not something the run said"
        );
    }
}
