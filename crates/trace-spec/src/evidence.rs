//! The handoff: a checked run becomes the evidence record the protocol already understands.
//!
//! Everything else in this crate produces a [`CheckReport`], which is a document about one
//! transcript and one specification. The protocol cannot read it — it decides on
//! [`Evidence`], and specifically on [`TraceConformanceResult`], whose facts a principle can
//! predicate over. This module is the whole of the join, and it is deliberately small: the two
//! types already line up, because the report was designed with the record in mind.
//!
//! # Why the conversion lives here and not in the engine
//!
//! Invariant 7: **the engine never manufactures evidence.** It evaluates what verifiers and humans
//! produced. So the conversion sits in the crate that *ran the check*, on the producing side of
//! the boundary, and it takes no argument naming who produced it. [`TraceEvidence::PRODUCER`] is a
//! constant: there is no call site at which a caller can name itself the verifier, which is what
//! stops the record's independence from being an input to the record.
//!
//! # An agent's own claim never mints this kind
//!
//! This is the point of the whole family, and it is worth stating where the record is built rather
//! than only in a design document. A model reporting *"I consulted the CLI before editing"* is a
//! claim by the subject about the subject. It is not this evidence kind, it cannot become this
//! evidence kind, and the type system says so twice over:
//!
//! * the only constructor is [`CheckReport::to_evidence`], and the only way to obtain a
//!   [`CheckReport`] is to run [`check`](crate::check::check) over a transcript the harness wrote;
//! * the producer is a constant naming
//!   [`Verifier::TraceChecker`], which is the only class `EvidenceKind::TraceConformance` names —
//!   so a record carrying `producer: agent` is refused by the requirement rather than counted by
//!   it.
//!
//! The consequence is the one the design is for: **a behavioural claim about an LLM step becomes
//! admissible evidence without the LLM minting anything.** The model does not report how it
//! worked; a deterministic checker reads the transcript the model produced and establishes it.
//!
//! # What it does not buy
//!
//! Attestation. The record is YAML by the time the engine reads it, and a person can type one.
//! There is no signature over it and nothing binding the bytes to the process that produced them —
//! the same limit `ess-conformance` states about its own record, and the same gap `docs/VISION.md`
//! names. What `independent: true` buys is a structural statement about *which component* produced
//! the record; it is not a claim that the component proved who it was.
//!
//! # Three verdicts, three statuses
//!
//! | check | evidence | what it says |
//! |---|---|---|
//! | [`Verdict::Ok`] | `passed` | every gating expectation held |
//! | [`Verdict::Gap`] | `failed` | the run contradicted the specification |
//! | [`Verdict::Unknown`] | `inconclusive` | **nobody found out** |
//!
//! `Unknown` becoming `Inconclusive` rather than `Failed` is the honest half, and it is the same
//! mapping `ess-conformance` makes for the same reason: a run nobody could read is not a run that
//! found a contradiction. Neither is a pass, so `trace_conformance.passed` is false for both and a
//! requirement stays owed either way.

use aep_domain::error::ParseError;
use aep_domain::evidence::{
    Evidence, Producer, Provenance, SpecDigest, TraceConformanceResult, TranscriptDigest,
};
use aep_domain::verification::{VerificationStatus, Verifier};

use crate::report::{CheckReport, Verdict};

/// An evidence record, with the producer that produced it.
///
/// Serialises as one entry of the evidence document `protocol evaluate --evidence` reads: the
/// evidence's own fields under `kind: trace_conformance`, beside `producer` and `provenance`. That
/// is the whole interface between the checker and the engine, and it is a file rather than a
/// function call on purpose — the two halves run in different processes, and often on different
/// machines.
///
/// The producer is not a field. It is [`Self::PRODUCER`], the same value for every record this
/// crate makes, so there is no parameter through which a caller could describe itself as the
/// verifier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TraceEvidence {
    /// The observation, tagged `kind: trace_conformance`.
    #[serde(flatten)]
    evidence: Evidence,
    /// Who produced it: always [`Self::PRODUCER`].
    producer: Producer,
    /// How it was obtained. The caller may say which command it ran; it may not say who it is.
    #[serde(skip_serializing_if = "is_empty_provenance")]
    provenance: Provenance,
}

/// Whether provenance carries nothing, so an empty block is not written out.
fn is_empty_provenance(provenance: &Provenance) -> bool {
    provenance == &Provenance::default()
}

impl TraceEvidence {
    /// The only producer this crate ever stamps: the trace checker, as a verifier.
    ///
    /// A constant rather than an argument, for the reason the module documentation gives: making
    /// it settable would make the record's independence something its caller asserts, and the one
    /// thing this record is for is being a statement about an agent that the agent did not make.
    pub const PRODUCER: Producer = Producer::Verifier {
        verifier: Verifier::TraceChecker,
    };

    /// The record itself.
    pub fn result(&self) -> &TraceConformanceResult {
        match &self.evidence {
            Evidence::TraceConformance(result) => result,
            // Unreachable by construction: the only constructor writes the `TraceConformance`
            // variant. Named rather than `unwrap`ped so that a future variant added here fails
            // loudly instead of returning a plausible other record.
            other => unreachable!("a trace check produces trace evidence, not {other:?}"),
        }
    }

    /// The observation, ready to submit.
    pub fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    /// What produced it. Always [`Self::PRODUCER`].
    pub fn producer(&self) -> &Producer {
        &self.producer
    }

    /// How it was obtained.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Records the command that produced the record, builder-style.
    ///
    /// Provenance, not producership: a caller saying *how* it ran the check adds to the record, and
    /// a caller saying *who ran it* would replace the only thing the requirement checks. Only the
    /// first is offered.
    #[must_use]
    pub fn obtained_by(mut self, command: impl Into<String>) -> Self {
        self.provenance.command = Some(command.into());
        self
    }

    /// Records a file the check read, builder-style.
    ///
    /// The specification and the transcript, by the paths the caller passed. Paths, not digests —
    /// the digests are in the body, where they identify the run and the document, and a path is
    /// only ever a hint about where to go and look.
    #[must_use]
    pub fn from_input(mut self, input: impl Into<String>) -> Self {
        self.provenance.inputs.push(input.into());
        self
    }
}

impl CheckReport {
    /// The evidence record this check produced.
    ///
    /// Every field comes from the report. Nothing is defaulted into place and nothing is asked of
    /// the caller — a conversion with a `transcript_digest: &str` parameter would let the record
    /// name a run other than the one that was read.
    ///
    /// The record carries the report's verdict, its three counts, the id of every gapped
    /// expectation and the digest pair, and it deliberately does **not** carry the expectations
    /// themselves: their citations quote the transcript, which is the most sensitive input this
    /// repository consumes, and an evidence record is a thing people paste into pull requests.
    ///
    /// # Errors
    ///
    /// [`ParseError`] when either digest in the report is not a digest. Unreachable for a report
    /// this crate produced — both come from `trace_domain::digest`, which writes 64 lowercase hex
    /// characters and nothing else — and reachable for a `CheckReport` assembled by hand through
    /// its public fields, which is exactly where a silent `expect` would be wrong.
    pub fn to_evidence(&self) -> Result<TraceEvidence, ParseError> {
        let result = TraceConformanceResult {
            specification: self.spec_id.clone(),
            spec_digest: SpecDigest::new(self.spec_digest.clone())?,
            transcript_digest: TranscriptDigest::new(self.transcript_digest.clone())?,
            status: evidence_status(self.verdict),
            expectations_total: self.summary.total,
            expectations_gapped: self.summary.gap,
            expectations_unknown: self.summary.unknown,
            advisory_overrides: self.advisory_overrides.clone(),
            adapter: Some(adapter_label(self)),
            gapped_expectations: self.gapped().into_iter().map(ToOwned::to_owned).collect(),
        };
        Ok(TraceEvidence {
            evidence: Evidence::TraceConformance(result),
            producer: TraceEvidence::PRODUCER,
            provenance: Provenance::default(),
        })
    }
}

/// The adapter, with the harness versions it was written against.
///
/// Both halves, because design D1 is that a harness output format is not a stable public schema: a
/// verdict that changed because the *reader* changed should be visible as such in the record, and
/// the name alone does not say which reader it was.
fn adapter_label(report: &CheckReport) -> String {
    if report.adapter.written_against.is_empty() {
        return report.adapter.name.to_owned();
    }
    format!(
        "{} (written against {})",
        report.adapter.name,
        report.adapter.written_against.join(", ")
    )
}

/// What a check's verdict is called in the protocol's vocabulary.
///
/// The one line where "nobody found out" is kept apart from "the agent did the wrong thing". See
/// the [module documentation](self) for why that distinction survives the handoff rather than
/// being flattened into a boolean.
fn evidence_status(verdict: Verdict) -> VerificationStatus {
    match verdict {
        Verdict::Ok => VerificationStatus::Passed,
        Verdict::Gap => VerificationStatus::Failed,
        Verdict::Unknown => VerificationStatus::Inconclusive,
    }
}
