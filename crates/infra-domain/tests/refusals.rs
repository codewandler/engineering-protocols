//! The refusal catalogue against committed fixture files.
//!
//! The unit tests beside each type prove every rule on constructed input; these two run whole
//! files through the same door a user's bundle takes, because the fixtures are also what the CLI
//! tests refuse — one artifact, two layers checked against it.

use infra_domain::{InfraCode, Observation, RawBundle, ValidationErrors};

fn validate(fixture: &str) -> Result<Observation, ValidationErrors> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("the committed fixture exists");
    let raw: RawBundle = serde_json::from_str(&text).expect("the fixture is JSON");
    Observation::try_from(raw)
}

#[test]
fn a_bundle_with_plain_string_secret_values_is_refused_and_no_value_is_echoed() {
    // The hard rule, end to end: this fixture is what an unsanitized or hostile scanner would
    // write, and it must never become an observation — defense in depth beside the scanner's
    // own sanitization.
    let errors = validate("unsanitized-secret.observation.json")
        .expect_err("an unsanitized bundle must be refused");
    let unsanitized = errors
        .as_slice()
        .iter()
        .filter(|error| error.code == InfraCode::UnsanitizedSecret)
        .count();
    assert_eq!(
        unsanitized, 2,
        "both plain values are refused, the digested one is not: {errors}"
    );
    let rendered = errors.to_string();
    for leaked in ["cGxhaW50ZXh0", "YWRtaW4"] {
        assert!(
            !rendered.contains(leaked),
            "a refusal echoed a secret value: {rendered}"
        );
    }
}

#[test]
fn every_defect_class_in_a_broken_bundle_is_reported_in_one_run() {
    let errors = validate("many-defects.observation.json")
        .expect_err("a bundle with five defects must be refused");
    for expected in [
        InfraCode::UnsupportedFormat,
        InfraCode::MissingKind,
        InfraCode::DuplicateIdentity,
        InfraCode::NonStringSelector,
        InfraCode::EmptyWorkload,
    ] {
        assert!(
            errors.contains(expected),
            "expected {expected} among the refusals of one single run, got: {errors}"
        );
    }
    assert_eq!(
        errors.len(),
        5,
        "exactly the five planted defects, nothing spurious: {errors}"
    );
}
