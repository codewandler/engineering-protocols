//! The handoff, against the two committed runs: a check report becomes a record the protocol reads.
//!
//! The unit of value here is not the conversion — it is that the record which comes out is one an
//! agent could not have written. Every test below is about some part of that: the producer nobody
//! can set, the digest pair that says *which* run against *which* document, and the three verdicts
//! surviving into the protocol's vocabulary instead of being flattened into a boolean.

use aep_domain::evidence::{Evidence, Producer, TranscriptDigest};
use aep_domain::facts::{FactPath, FactValue};
use aep_domain::time::{ObservedAt, Timestamp};
use aep_domain::verification::{VerificationStatus, Verifier};
use trace_domain::ir::TraceIr;
use trace_domain::spec::TraceSpec;
use trace_spec::adapter::read_transcript;
use trace_spec::check::check;
use trace_spec::evidence::TraceEvidence;
use trace_spec::report::CheckReport;

/// When every fixture in this file says the check was made.
///
/// The epoch, because nothing here is about time: these tests are about what the checker says,
/// and a fixed instant keeps the records byte-comparable.
const OBSERVED: ObservedAt = ObservedAt::new(Timestamp::EPOCH);

/// The committed transcript of eval run `7hTYjT`.
const SEVEN_H: &[u8] = include_bytes!("fixtures/plugin-eval-7hTYjT.jsonl");

fn ir() -> TraceIr {
    read_transcript(SEVEN_H).expect("the committed fixture is a transcript this build reads")
}

/// A specification written inline, through the same reader a file goes through.
fn spec(text: &str) -> TraceSpec {
    trace_domain::raw::read_spec(text)
        .unwrap_or_else(|errors| panic!("the fixture specification must validate:\n{errors}"))
}

/// The specification the eval ships, as the eval ships it.
fn eval_spec() -> TraceSpec {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/trace/expectations.trace.yaml"
    );
    let text = std::fs::read_to_string(path).expect("the eval's specification is committed");
    spec(&text)
}

fn report(specification: &TraceSpec, advisory: &[String]) -> CheckReport {
    check(specification, &ir(), advisory)
}

/// One fact of the record, by path.
fn fact(evidence: &Evidence, path: &str) -> Option<FactValue> {
    let wanted = FactPath::new(path).expect("a fact path");
    evidence
        .facts()
        .into_iter()
        .find(|(candidate, _)| candidate == &wanted)
        .map(|(_, value)| value)
}

#[test]
fn the_record_carries_the_digest_pair_the_report_computed() {
    // The field the whole record is worth anything for, twice over. Without both, the record says
    // that *some* run satisfied *some* specification, which establishes nothing a reader can go
    // and check.
    let checked = report(&eval_spec(), &[]);
    assert_eq!(
        checked.summary.gap, 0,
        "the fixture reaches a passing state"
    );

    let record = checked
        .to_evidence(OBSERVED)
        .expect("a real report converts");
    let result = record.result();

    assert_eq!(
        result.spec_digest.as_str(),
        checked.spec_digest,
        "the record's specification digest is the report's, not a value typed in beside it"
    );
    assert_eq!(
        result.transcript_digest.as_str(),
        checked.transcript_digest,
        "the record's transcript digest is the report's"
    );
    assert_ne!(
        result.spec_digest.as_str(),
        result.transcript_digest.as_str(),
        "the two digests are digests of different things, and a record where they agreed would \
         mean the builder transposed them"
    );
    assert!(
        result.attests(
            &TranscriptDigest::new(checked.transcript_digest.clone()).expect("64 hex characters")
        ),
        "the record must recognise the run it was built from"
    );
    assert_eq!(result.specification, "planning-plugin/eval");
    assert_eq!(result.status, VerificationStatus::Passed);
    assert_eq!(result.expectations_total, checked.summary.total);
    assert_eq!(result.expectations_gapped, 0);
    assert!(result.gapped_expectations.is_empty());
    assert_eq!(
        result.adapter.as_deref(),
        Some("claude-code/stream-json (written against 2.1.238)"),
        "which reader produced the verdict is part of the record — design D1"
    );
}

#[test]
fn the_record_names_the_trace_checker_and_never_the_caller() {
    let record = report(&eval_spec(), &[])
        .to_evidence(OBSERVED)
        .expect("a real report converts");

    assert_eq!(
        record.producer(),
        &Producer::Verifier {
            verifier: Verifier::TraceChecker
        },
        "the producer is a constant, so there is no argument through which a caller could name \
         itself the verifier"
    );
    assert!(
        !record.producer().is_agent(),
        "an agent's own account of how it worked is not this evidence kind, and the requirement's \
         `independent: true` is checked on exactly this"
    );

    // JSON rather than YAML: this crate deliberately carries no YAML, and the document a person
    // reads is the CLI's rendering — `crates/protocol-cli/tests/trace_cli.rs` asserts that shape.
    let document = serde_json::to_string(&[&record]).expect("the record serialises");
    assert!(
        document.contains(r#""verifier":"trace-checker""#),
        "the document names the class: {document}"
    );
    assert!(
        !document.contains(r#""producer":"agent""#),
        "nothing in this crate can stamp an agent as the producer: {document}"
    );
}

#[test]
fn a_contradicted_run_is_written_down_rather_than_refused() {
    // The run called `Bash` many times; a document forbidding it gaps. The verb's job is to record
    // the verdict, not to decline to produce a record it does not like.
    let checked = report(
        &spec(
            r"
format: trace-spec/1
id: trace-evidence/forbids-bash
expectations:
  - id: nothing-shelled-out
    expect:
      tool.absent:
        tool: Bash
",
        ),
        &[],
    );
    assert_eq!(
        checked.summary.gap, 1,
        "the fixture reaches the gapping state"
    );

    let record = checked.to_evidence(OBSERVED).expect("converts");
    let result = record.result();
    assert_eq!(result.status, VerificationStatus::Failed);
    assert_eq!(result.expectations_gapped, 1);
    assert_eq!(
        result.gapped_expectations,
        vec!["nothing-shelled-out".to_owned()],
        "a failure names something actionable, not just a count"
    );
    assert_eq!(
        fact(record.evidence(), "trace_conformance.passed"),
        Some(FactValue::bool(false))
    );
}

#[test]
fn a_run_nobody_could_judge_is_inconclusive_and_not_failed() {
    // Exit 3, not a softer exit 1. A record that flattened "the transcript could not be read" into
    // "the agent did the wrong thing" would open a defect report against a run nobody managed to
    // ask a question of.
    let checked = report(
        &spec(
            r"
format: trace-spec/1
id: trace-evidence/undecidable
expectations:
  - id: a-tool-this-run-never-called
    expect:
      tool.result:
        tool: NoSuchToolExistsHere
        result:
          userModified: {equals: false}
",
        ),
        &[],
    );
    assert_eq!(
        checked.summary.unknown,
        1,
        "the fixture reaches the undecidable state: {}",
        trace_spec::render::report_to_text(&checked)
    );
    assert_eq!(checked.summary.gap, 0, "nothing was contradicted");

    let record = checked.to_evidence(OBSERVED).expect("converts");
    let result = record.result();
    assert_eq!(
        result.status,
        VerificationStatus::Inconclusive,
        "undecidable is its own answer"
    );
    assert_eq!(result.expectations_unknown, 1);
    assert_eq!(
        fact(record.evidence(), "trace_conformance.passed"),
        Some(FactValue::bool(false)),
        "and it is still not a pass: unproven is not proven"
    );
}

#[test]
fn a_command_line_downgrade_is_recorded_and_does_not_make_the_record_pass() {
    // `--advisory` exists so a bound that drifted with model routing cannot turn a CI job red. It
    // is a property of the invocation, not of the protocol's requirement — so it moves the exit
    // code and deliberately does not move `trace_conformance.passed`. A requirement a caller's own
    // flag could satisfy would not be a requirement.
    let document = spec(
        r"
format: trace-spec/1
id: trace-evidence/downgraded
expectations:
  - id: nothing-shelled-out
    expect:
      tool.absent:
        tool: Bash
",
    );
    let mut downgraded = document.clone();
    let unknown =
        downgraded.mark_advisory(&["nothing-shelled-out".to_owned()].into_iter().collect());
    assert!(unknown.is_empty(), "the downgrade names a declared id");

    let checked = check(&downgraded, &ir(), &["nothing-shelled-out".to_owned()]);
    assert_eq!(
        checked.exit_code(),
        0,
        "the fixture reaches the state the test is about: the gap no longer gates"
    );

    let record = checked.to_evidence(OBSERVED).expect("converts");
    let result = record.result();
    assert_eq!(
        result.status,
        VerificationStatus::Passed,
        "the record's status is the checker's verdict, so it matches the exit code"
    );
    assert_eq!(
        result.advisory_overrides,
        vec!["nothing-shelled-out".to_owned()],
        "the narrowing is visible in the record rather than silent"
    );
    assert_eq!(
        result.expectations_gapped, 1,
        "the contradiction is still counted; the downgrade did not unobserve it"
    );
    assert_eq!(
        fact(record.evidence(), "trace_conformance.passed"),
        Some(FactValue::bool(false)),
        "a caller's own flag must not satisfy a protocol requirement"
    );
}

#[test]
fn the_document_is_one_record_of_the_kind_the_protocol_declares() {
    let record = report(&eval_spec(), &[])
        .to_evidence(OBSERVED)
        .expect("a real report converts");
    // A list of one, because that is the shape `protocol evaluate --evidence` reads.
    let document = serde_json::to_string(&[&record]).expect("serialises");

    assert!(
        document.starts_with(r#"[{"kind":"trace_conformance","#),
        "the wire name is what `protocols/adp/1.yaml` declares, and the tag comes first: {document}"
    );
    assert!(
        document.contains(r#""transcript_digest":"#),
        "the digest pair is in the body: {document}"
    );
    assert!(
        !document.contains(r#""expectations":"#),
        "the record is a summary, not the report: a citation quotes the transcript, and an \
         evidence record is a thing people paste into pull requests: {document}"
    );
    assert_eq!(
        record.evidence().kind(),
        aep_domain::evidence::EvidenceKind::TraceConformance
    );
}

#[test]
fn a_report_whose_digests_are_not_digests_is_refused_rather_than_recorded() {
    // Unreachable for a report this crate produced, and reachable for one assembled by hand
    // through `CheckReport::new`. A silent `expect` there would turn a caller's mistake into a
    // panic in a verb whose whole job is to write a file.
    let checked = report(&eval_spec(), &[]);
    let hand_made = CheckReport::new(
        checked.spec_id.clone(),
        checked.spec_title.clone(),
        "not-a-digest".to_owned(),
        checked.transcript_digest.clone(),
        checked.adapter,
        Vec::new(),
        Vec::new(),
    );
    let error = hand_made
        .to_evidence(OBSERVED)
        .expect_err("a specification digest that is not hexadecimal is refused");
    assert!(
        error.to_string().contains("hexadecimal"),
        "the refusal says what was wrong: {error}"
    );
}

#[test]
fn the_producer_constant_is_the_class_the_evidence_kind_names() {
    // The join that makes the requirement mechanical. If these two drifted apart, a record this
    // crate produced would be refused by the very requirement it exists to satisfy, and nothing
    // else in either crate compares them.
    let Producer::Verifier { verifier } = &TraceEvidence::PRODUCER else {
        panic!("the checker produces evidence as a verifier");
    };
    assert!(
        aep_domain::evidence::EvidenceKind::TraceConformance
            .default_verifiers()
            .contains(verifier),
        "the constant must name a class that can establish `trace_conformance`"
    );
}
