//! The handoff: a run becomes the evidence record the protocol already understands (§31).
//!
//! Everything else in this crate produces a [`ConformanceReport`], which is a document about one
//! implementation and one specification. The protocol cannot read it — it decides on
//! [`Evidence`], and specifically on
//! [`EssConformanceResult`], whose facts a principle can predicate over. This module is the whole
//! of the join, and it is deliberately small: the two types already line up, because
//! `EssConformanceResult` was designed from the report in §30.
//!
//! # Why the conversion lives here and not in the engine
//!
//! Invariant 7: **the engine never manufactures evidence.** It evaluates what verifiers and humans
//! produced. Design §32 draws the same line one level up — an agent may trigger a run, read the
//! report and repair what it says, and may not construct the record by assertion.
//!
//! So the conversion sits in the crate that *ran the suite*, on the producing side of the boundary,
//! and it takes no argument naming who produced it. [`ConformanceEvidence::PRODUCER`] is a constant:
//! there is no call site at which a caller can name itself the verifier, which is what stops the
//! record's independence from being an input to the record. That is the enforcement, and it is worth
//! being precise about its limits — see [*What `independent` does and does not
//! mean*](#what-independent-does-and-does-not-mean).
//!
//! # Failed, error and unsupported are three findings, not one
//!
//! [`ConformanceStatus`] has three values and
//! [`VerificationStatus`] has four, and the mapping is
//! the point of the module rather than a detail of it:
//!
//! | run | evidence | what it says |
//! |---|---|---|
//! | [`Passed`](ConformanceStatus::Passed) | `passed` | every scenario the specification obliges held |
//! | [`Failed`](ConformanceStatus::Failed) | `failed` | the implementation contradicted the specification, or could not expose a required observation |
//! | [`Error`](ConformanceStatus::Error) | `inconclusive` | **nobody found out** |
//!
//! `Error` becoming `Inconclusive` rather than `Failed` is the honest half. A run that could not be
//! carried out is not a run that found a contradiction: the first is a target to go and reach, the
//! second is a defect to fix, and a protocol that cannot tell them apart opens a bug report against
//! a system nobody managed to ask a question of. Neither is a pass —
//! [`VerificationStatus::is_pass`] is true
//! only for `Passed`, so `ess_conformance.passed` is false for both and the requirement stays owed
//! either way.
//!
//! `Unsupported` has no status of its own because §28 already folded it into the run's verdict: a
//! required scenario the target cannot expose makes conformance *fail*, and this crate has no
//! optional scenarios. It survives into the record through
//! [`EssConformanceResult::failed_scenarios`], where each entry is written `<status> <scenario>` —
//! so a reader of the evidence alone can tell `failed billing.invoice.CreateInvoice/outcome/rejected`
//! from `unsupported notify-on-invoice-created/binding/mapping` without the report in front of them.
//!
//! # What `independent` does and does not mean
//!
//! `principles/verification/ess-conformance.yaml` requires `independent: true` and
//! `verifier: conformance-runner`. Mechanically that is one check —
//! [`Producer::is_agent`](aep_domain::evidence::Producer::is_agent) is false and the verifier
//! matches — and it is worth writing down what that buys, because the loop asks a reader to trust it:
//!
//! * **It does buy** that the record was produced by code that executed the specification's own
//!   suite. Nothing in this workspace can produce an `ess_conformance` record any other way, because
//!   the only constructor is [`ConformanceReport::to_evidence`] and the only way to obtain a report
//!   is to run a suite against a target.
//! * **It does not buy** attestation. The record is YAML by the time the engine reads it, and a
//!   person can type one. There is no signature over it, no key, and nothing that binds the bytes to
//!   the process that produced them — `Provenance::digest` is left empty here rather than filled
//!   with a value that would read as tamper evidence and is not.
//!
//! The gap is real and is named in `docs/VISION.md`. What closes it is the same thing that closes it
//! for a test runner's `tests.unit.failed == 0`: the harness, not the protocol, decides which
//! producers it lets write records. `independent: true` is a statement about *which component*
//! produced the record, checked structurally; it is not a claim that the component proved who it was.

use aep_domain::evidence::{EssConformanceResult, Evidence, Producer, Provenance};
use aep_domain::time::ObservedAt;
use aep_domain::verification::{VerificationStatus, Verifier};

use crate::report::{ConformanceReport, ConformanceStatus, Status};

/// An evidence record, with the producer that produced it.
///
/// Serialises as one entry of the evidence document `protocol evaluate --evidence` reads: the
/// evidence's own fields under `kind: ess_conformance`, beside `producer` and `provenance`. That is
/// the whole interface between the runner and the engine, and it is a file rather than a function
/// call on purpose — the two halves run in different processes, and often on different machines.
///
/// The producer is not a field. It is [`Self::PRODUCER`], the same value for every record this crate
/// makes, so there is no parameter through which a caller could describe itself as the verifier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConformanceEvidence {
    /// The observation, tagged `kind: ess_conformance`.
    #[serde(flatten)]
    evidence: Evidence,
    /// When the run happened.
    ///
    /// The caller's, because this crate holds no clock and a report is not a moment. It is written
    /// into the record because a conformance run is exactly the kind of observation that decays: an
    /// implementation that satisfied a specification three weeks ago satisfies nothing today unless
    /// somebody ran it again.
    observed_at: ObservedAt,
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

impl ConformanceEvidence {
    /// The only producer this crate ever stamps: the conformance runner, as a verifier.
    ///
    /// A constant rather than an argument. `independent: true` in a requirement means "the producer
    /// is not the agent under review" ([`Producer::is_agent`]), and
    /// `verifier: conformance-runner` means this exact value — so making it settable would make the
    /// record's independence something its caller asserts, which is design §32's incorrect diagram
    /// with an extra step.
    pub const PRODUCER: Producer = Producer::Verifier {
        verifier: Verifier::ConformanceRunner,
    };

    /// The record itself.
    pub fn result(&self) -> &EssConformanceResult {
        match &self.evidence {
            Evidence::EssConformance(result) => result,
            // Unreachable by construction: the only constructor writes the `EssConformance`
            // variant. Named rather than `unwrap`ped so that a future variant added here fails
            // loudly instead of returning a plausible other record.
            other => unreachable!("a conformance run produces conformance evidence, not {other:?}"),
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

    /// When the run happened.
    pub fn observed_at(&self) -> ObservedAt {
        self.observed_at
    }

    /// Records the command that produced the record, builder-style.
    ///
    /// Provenance, not producership: a caller saying *how* it ran the suite adds to the record, and a
    /// caller saying *who ran it* would replace the only thing the requirement checks. Only the first
    /// is offered.
    #[must_use]
    pub fn obtained_by(mut self, command: impl Into<String>) -> Self {
        self.provenance.command = Some(command.into());
        self
    }

    /// Records what the run was given — the specification or suite it read, builder-style.
    #[must_use]
    pub fn from_input(mut self, input: impl Into<String>) -> Self {
        self.provenance.inputs.push(input.into());
        self
    }
}

impl ConformanceReport {
    /// The evidence record this run produced (§31).
    ///
    /// Every field comes from the run. Nothing is defaulted into place, and nothing is asked of the
    /// caller — a conversion with an `implementation: &str` parameter would let the record name a
    /// system other than the one that answered.
    ///
    /// The spec digest is the field the whole record is worth anything for: gate G19 made evidence
    /// without it unable to demonstrate which revision produced it, and made the binding fail
    /// closed. It is carried straight from [`SuiteProvenance`](crate::scenario::SuiteProvenance),
    /// which took it from the compiled model, so there is no step at which it could be typed in.
    pub fn to_evidence(&self, observed_at: ObservedAt) -> ConformanceEvidence {
        let result = EssConformanceResult {
            // `billing/v3`: the label a person reads. `spec_digest` is what identifies it.
            specification: format!("{}/{}", self.suite.system, self.suite.specification_version),
            spec_digest: self.suite.spec_digest.clone(),
            implementation: self.implementation.to_string(),
            status: evidence_status(self.status),
            scenarios_total: self.scenarios.len(),
            scenarios_failed: self
                .scenarios
                .iter()
                .filter(|result| result.status != Status::Passed)
                .count(),
            suite_version: Some(self.suite.suite_version.to_string()),
            compiler_version: Some(self.suite.compiler_version.clone()),
            generator_version: Some(self.suite.generator_version.clone()),
            failed_scenarios: self
                .failures()
                .map(|result| format!("{} {}", result.status, result.scenario))
                .collect(),
        };
        ConformanceEvidence {
            evidence: Evidence::EssConformance(result),
            observed_at,
            producer: ConformanceEvidence::PRODUCER,
            provenance: Provenance::default(),
        }
    }
}

/// What a run's verdict is called in the protocol's vocabulary.
///
/// The one line where "nobody found out" is kept apart from "the implementation is wrong". See the
/// [module documentation](self) for why that distinction survives the handoff rather than being
/// flattened into a boolean.
fn evidence_status(status: ConformanceStatus) -> VerificationStatus {
    match status {
        ConformanceStatus::Passed => VerificationStatus::Passed,
        ConformanceStatus::Failed => VerificationStatus::Failed,
        ConformanceStatus::Error => VerificationStatus::Inconclusive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{CheckResult, Diagnostic, ScenarioResult};
    use crate::scenario::{CommandRef, ScenarioId, SuiteFormat, SuiteProvenance};
    use crate::target::ImplementationIdentity;
    use aep_domain::evidence::SpecDigest;
    use aep_domain::time::Timestamp;
    use ess_domain::command::OutcomeName;
    use ess_domain::name::QualifiedName;

    fn outcome(command: &str, name: &str) -> crate::scenario::OutcomeRef {
        crate::scenario::OutcomeRef::new(
            CommandRef::new(QualifiedName::new(command).expect("valid")),
            OutcomeName::new(name).expect("valid"),
        )
    }

    fn scenario(command: &str, name: &str, status: Status) -> ScenarioResult {
        let outcome = outcome(command, name);
        ScenarioResult {
            scenario: ScenarioId::Outcome {
                outcome: outcome.clone(),
            },
            purpose: "a positive amount is accepted".to_owned(),
            status,
            checks: vec![match status {
                Status::Passed => CheckResult::passed(crate::report::CheckCode::Outcome, "outcome"),
                Status::Failed => CheckResult::failed(
                    "outcome",
                    Diagnostic::new(
                        crate::report::CheckCode::Outcome,
                        ScenarioId::Outcome {
                            outcome: outcome.clone(),
                        },
                    ),
                ),
                Status::Error => CheckResult::errored(
                    "outcome",
                    Diagnostic::new(
                        crate::report::CheckCode::Target,
                        ScenarioId::Outcome {
                            outcome: outcome.clone(),
                        },
                    ),
                ),
                Status::Unsupported => CheckResult::unsupported(
                    "outcome",
                    Diagnostic::new(
                        crate::report::CheckCode::Target,
                        ScenarioId::Outcome { outcome },
                    ),
                ),
            }],
            duration_ms: 0,
        }
    }

    fn report(scenarios: Vec<ScenarioResult>) -> ConformanceReport {
        ConformanceReport {
            suite: SuiteProvenance {
                suite_version: SuiteFormat::CURRENT,
                system: "billing".to_owned(),
                specification_version: "v3".to_owned(),
                spec_digest: SpecDigest::new(
                    "13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861",
                )
                .expect("a digest"),
                contract_digest: SpecDigest::new(
                    "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
                )
                .expect("a digest"),
                compiler_version: "0.1.0".to_owned(),
                generator_version: "0.1.0".to_owned(),
                synthesizer_version: "0.1.0".to_owned(),
            },
            implementation: ImplementationIdentity::new("billing-reference", "0.1.0"),
            started_at: aep_domain::time::Timestamp::from_epoch_millis(1_700_000_000_000),
            completed_at: aep_domain::time::Timestamp::from_epoch_millis(1_700_000_001_000),
            status: ConformanceReport::verdict(&scenarios),
            scenarios,
        }
    }

    #[test]
    fn a_passing_run_carries_the_digest_the_specification_was_resolved_to() {
        // The field gate G19 exists for. Without it the record says that *some* implementation
        // passed *some* suite, and the revision binding has nothing to compare.
        let evidence = report(vec![scenario(
            "billing.invoice.CreateInvoice",
            "accepted",
            Status::Passed,
        )])
        .to_evidence(ObservedAt::new(Timestamp::EPOCH));

        let result = evidence.result();
        assert_eq!(
            result.spec_digest.as_str(),
            "13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861"
        );
        assert_eq!(result.specification, "billing/v3");
        assert_eq!(result.implementation, "billing-reference 0.1.0");
        assert_eq!(result.status, VerificationStatus::Passed);
        assert_eq!(result.scenarios_total, 1);
        assert_eq!(result.scenarios_failed, 0);
        assert!(result.failed_scenarios.is_empty());
    }

    #[test]
    fn the_record_names_the_conformance_runner_and_never_the_caller() {
        // `independent: true` is checked against the producer, so a record this crate makes must
        // carry the verifier and no way to overwrite it. `obtained_by` is offered and must not
        // touch it.
        let evidence = report(vec![scenario(
            "billing.invoice.CreateInvoice",
            "accepted",
            Status::Passed,
        )])
        .to_evidence(ObservedAt::new(Timestamp::EPOCH))
        .obtained_by("protocol ess conform evidence --path examples/billing --target billing");

        assert_eq!(evidence.producer(), &ConformanceEvidence::PRODUCER);
        assert!(
            !evidence.producer().is_agent(),
            "an agent-produced record does not satisfy `independent: true`"
        );
        assert_eq!(
            evidence.provenance().command.as_deref(),
            Some("protocol ess conform evidence --path examples/billing --target billing")
        );
    }

    #[test]
    fn a_run_nobody_could_carry_out_is_inconclusive_and_not_a_contradiction() {
        // The distinction the handoff exists to keep. The fixture reaches all three verdicts,
        // because a mapping checked against one value proves nothing about the other two.
        let passed = report(vec![scenario(
            "billing.invoice.CreateInvoice",
            "accepted",
            Status::Passed,
        )]);
        let failed = report(vec![scenario(
            "billing.invoice.CreateInvoice",
            "rejected",
            Status::Failed,
        )]);
        let errored = report(vec![scenario(
            "billing.invoice.CreateInvoice",
            "accepted",
            Status::Error,
        )]);
        assert_eq!(passed.status, ConformanceStatus::Passed);
        assert_eq!(failed.status, ConformanceStatus::Failed);
        assert_eq!(errored.status, ConformanceStatus::Error);

        assert_eq!(
            passed
                .to_evidence(ObservedAt::new(Timestamp::EPOCH))
                .result()
                .status,
            VerificationStatus::Passed
        );
        assert_eq!(
            failed
                .to_evidence(ObservedAt::new(Timestamp::EPOCH))
                .result()
                .status,
            VerificationStatus::Failed
        );
        assert_eq!(
            errored
                .to_evidence(ObservedAt::new(Timestamp::EPOCH))
                .result()
                .status,
            VerificationStatus::Inconclusive,
            "a run that could not be carried out is not a run that found a contradiction"
        );
        assert!(
            !errored
                .to_evidence(ObservedAt::new(Timestamp::EPOCH))
                .result()
                .status
                .is_pass(),
            "and it is still not a pass"
        );
    }

    #[test]
    fn an_unsupported_scenario_is_counted_and_named_as_unsupported_rather_than_as_a_failure() {
        // §28: an unsupported required scenario is not a skip. It counts against the run — so a
        // reader of the record alone cannot read `0 failed` off a suite that checked nothing — and
        // it keeps its own word, so nobody opens a defect against an implementation that never
        // contradicted anything.
        let evidence = report(vec![
            scenario("billing.invoice.CreateInvoice", "accepted", Status::Passed),
            scenario("billing.invoice.PayInvoice", "settled", Status::Unsupported),
        ])
        .to_evidence(ObservedAt::new(Timestamp::EPOCH));

        let result = evidence.result();
        assert_eq!(result.status, VerificationStatus::Failed);
        assert_eq!(result.scenarios_total, 2);
        assert_eq!(result.scenarios_failed, 1);
        assert_eq!(
            result.failed_scenarios,
            vec!["unsupported billing.invoice.PayInvoice/outcome/settled".to_owned()],
            "the record says which word applied, because `failed` and `unsupported` send a reader \
             to different places"
        );
    }

    #[test]
    fn the_facts_a_failing_run_projects_leave_the_shipped_requirement_owed() {
        // The end of the handoff, checked here rather than only in an integration test: the two
        // predicates `principles/verification/ess-conformance.yaml` names must both read false off
        // a failing run's own facts.
        let evidence = report(vec![
            scenario("billing.invoice.CreateInvoice", "accepted", Status::Passed),
            scenario("billing.invoice.CreateInvoice", "rejected", Status::Failed),
        ])
        .to_evidence(ObservedAt::new(Timestamp::EPOCH));

        let facts: std::collections::BTreeMap<String, String> = evidence
            .evidence()
            .facts()
            .into_iter()
            .map(|(path, value)| (path.to_string(), value.to_string()))
            .collect();

        assert_eq!(
            facts.get("ess_conformance.passed").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            facts
                .get("ess_conformance.scenarios.failed")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            facts.get("ess_conformance.spec_digest").map(String::as_str),
            Some("13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861"),
            "a failing run still names the revision it was run against"
        );
    }

    #[test]
    fn the_record_serialises_as_one_entry_of_an_evidence_document() {
        // Tagged `kind: ess_conformance`, with the producer beside the observation rather than
        // inside it — the shape `protocol evaluate --evidence` parses. Nothing type-checks a file,
        // so if this drifts the loop breaks at the one place a compiler cannot see.
        let record = report(vec![scenario(
            "billing.invoice.CreateInvoice",
            "accepted",
            Status::Passed,
        )])
        .to_evidence(ObservedAt::new(Timestamp::EPOCH))
        .obtained_by("protocol ess conform evidence");

        let json = serde_json::to_value(&record).expect("an evidence record serialises");
        assert_eq!(json["kind"], "ess_conformance");
        assert_eq!(json["specification"], "billing/v3");
        assert_eq!(
            json["spec_digest"],
            "13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861"
        );
        assert_eq!(json["suite_version"], "ess-conformance/1");
        assert_eq!(json["producer"]["producer"], "verifier");
        assert_eq!(json["producer"]["verifier"], "conformance-runner");
        assert_eq!(
            json["provenance"]["command"],
            "protocol ess conform evidence"
        );
    }
}
