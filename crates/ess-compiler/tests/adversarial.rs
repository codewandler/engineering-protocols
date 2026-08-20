//! Phase 1 of the property-testing decision: adversarial specifications against the pipeline.
//!
//! The recorded property (wave 3.5 reconciliation page, § "The property-based work, and where it
//! sits"): **any generated document either yields at least one `ValidationCode`, or compiles and
//! re-serialises identically. No panic, no hang, no third outcome.** The billing fixture proves
//! the pipeline accepts one good specification; nothing before this file probed it with inputs
//! nobody thought to write — dangling references, mutually recursive structs, duplicate names,
//! upper-case and digit-led identifiers, empty systems — and asserted that every one of them is
//! *refused as data* rather than survived as a crash.
//!
//! The generator lives here in `tests/`, never in `src`: the same page keeps `ess-compiler/src`
//! under the banned-token scan (`tests/billing.rs`), and this crate's shipped code stays
//! RNG-free while its tests seed one deliberately.
//!
//! # Determinism
//!
//! The gate must not be flaky (invariant 9's spirit), so the runner is seeded with a fixed value
//! and every run draws the same specifications in the same order. Each drawn specification is
//! additionally run through the pipeline **twice from the raw text**, and the two outcomes must
//! match byte for byte — which turns every generated case into an instance of the byte-identical
//! test `tests/billing.rs` runs once. To explore beyond the committed sequence locally, raise
//! `PROPTEST_CASES` or edit the seed to `RngSeed::Random` for one session; commit neither.

use std::fmt::Write as _;

use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// The fixed-seed configuration every property in this file runs under.
fn seeded() -> ProptestConfig {
    ProptestConfig {
        rng_seed: RngSeed::Fixed(0x5eed_0002),
        ..ProptestConfig::default()
    }
}

/// Name fragments, valid and deliberately not: upper case, digit-led, spaced, hyphenated.
fn fragment() -> impl Strategy<Value = &'static str> {
    prop::sample::select(
        &[
            "left",
            "right",
            "ok",
            "a",
            "Left",
            "9bad",
            "UPPER",
            "with space",
            "x-y",
            "money",
        ][..],
    )
}

/// A field's declared type: primitives, misspellings, references that may dangle or cycle.
fn field_type() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(&["String", "Int", "Bool", "Decimal", "Nothing", "string"][..])
            .prop_map(str::to_owned),
        (0usize..4).prop_map(|index| format!("shop.orders.t{index}")),
        fragment().prop_map(|part| format!("shop.orders.{part}")),
    ]
}

/// A whole specification document, assembled as YAML text.
fn specification() -> impl Strategy<Value = String> {
    let field = (fragment(), field_type());
    let declared_type = (fragment(), prop::collection::vec(field, 0..3));
    let header = (
        prop::sample::select(&["shop", "Shop", "9shop", "shop_x", ""][..]),
        prop::sample::select(&["v1", "v2", "V1", "1", "vv"][..]),
    );
    (header, prop::collection::vec(declared_type, 0..4)).prop_map(|((system, version), types)| {
        let mut yaml = format!(
            "format: ess/1\nsystem: \"{system}\"\nversion: \"{version}\"\ndomain: shop.orders\n"
        );
        if types.is_empty() {
            yaml.push_str("types: []\n");
            return yaml;
        }
        yaml.push_str("types:\n");
        for (index, (part, fields)) in types.iter().enumerate() {
            let _ = writeln!(yaml, "  - name: shop.orders.t{index}{part}");
            yaml.push_str("    kind: struct\n");
            if fields.is_empty() {
                yaml.push_str("    fields: []\n");
                continue;
            }
            yaml.push_str("    fields:\n");
            for (name, declared) in fields {
                let _ = writeln!(yaml, "      - name: {name}");
                let _ = writeln!(yaml, "        type: {declared}");
            }
        }
        yaml
    })
}

/// Where one pipeline run ended, with everything a second run must reproduce.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The document is not a document; the refusal text is the outcome.
    ParseRefused(String),
    /// Validation refused, with how many errors accumulated and what they say.
    ValidationRefused(usize, String),
    /// Resolution refused, with how many diagnostics and what they say.
    ResolutionRefused(usize, String),
    /// The specification compiled; the canonical JSON is the outcome.
    Compiled(String),
}

/// One run of the whole pipeline over `text`.
fn run(text: &str) -> Outcome {
    let raw = match RawSpecFile::parse(text) {
        Ok(raw) => raw,
        Err(error) => return Outcome::ParseRefused(error.to_string()),
    };
    let mut sources = SourceMap::new();
    sources.insert("adversarial.yaml", text);
    let specification = match Specification::assemble([(Source::new("adversarial.yaml"), raw)]) {
        Ok(specification) => specification,
        Err(errors) => return Outcome::ValidationRefused(errors.len(), errors.to_string()),
    };
    match compile(&specification, &sources) {
        Ok(ir) => Outcome::Compiled(ir.to_canonical_json()),
        Err(diagnostics) => Outcome::ResolutionRefused(diagnostics.len(), diagnostics.to_string()),
    }
}

/// The property's own inverse assertion, in the spirit of every scan in this workspace: both
/// arms of the trichotomy are reachable from the generator's value space, witnessed by concrete
/// documents the strategy can draw. Without this, a generator drifting into producing only
/// garbage would turn the determinism half of the property into a vacuous truth — 256 refusals
/// prove nothing about canonical output.
#[test]
fn the_generator_reaches_both_compilation_and_refusal() {
    let good = "format: ess/1\nsystem: \"shop\"\nversion: \"v1\"\ndomain: shop.orders\ntypes:\n  - name: shop.orders.t0money\n    kind: struct\n    fields:\n      - name: left\n        type: String\n";
    match run(good) {
        Outcome::Compiled(json) => assert!(json.contains("shop.orders.t0money")),
        other => panic!(
            "a well-formed document the strategy can draw must compile; the pipeline said \
             {other:?}, so the property above is exercising refusals only"
        ),
    }

    let dangling = "format: ess/1\nsystem: \"shop\"\nversion: \"v1\"\ndomain: shop.orders\ntypes:\n  - name: shop.orders.t0left\n    kind: struct\n    fields:\n      - name: left\n        type: shop.orders.t3\n";
    match run(dangling) {
        Outcome::ValidationRefused(count, _) | Outcome::ResolutionRefused(count, _) => {
            assert!(count >= 1);
        }
        other => panic!(
            "a dangling reference the strategy can draw must be refused; the pipeline said \
             {other:?}, so the property above is exercising acceptances only"
        ),
    }
}

proptest! {
    #![proptest_config(seeded())]

    /// The recorded phase-1 property, verbatim: refusal with at least one reason, or a compile
    /// that re-serialises identically. Reaching the end of the closure is itself the no-panic,
    /// no-hang half of the claim.
    #[test]
    fn every_document_is_refused_with_reasons_or_compiled_identically_twice(
        text in specification()
    ) {
        let first = run(&text);
        match &first {
            Outcome::ParseRefused(reason) => {
                prop_assert!(!reason.is_empty(), "a refusal with no reason is a shrug");
            }
            Outcome::ValidationRefused(count, _) => {
                prop_assert!(*count >= 1, "a validation refusal carries its errors");
            }
            Outcome::ResolutionRefused(count, _) => {
                prop_assert!(*count >= 1, "a resolution refusal carries its diagnostics");
            }
            Outcome::Compiled(json) => {
                prop_assert!(
                    json.ends_with('\n'),
                    "canonical output ends in exactly one newline"
                );
            }
        }

        let second = run(&text);
        prop_assert_eq!(
            &first, &second,
            "two runs over the same bytes diverged, so something in the pipeline reads a clock, \
             an unordered map, or other ambient state; the input was:\n{}", text
        );
    }
}
