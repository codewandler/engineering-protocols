//! The fixture pair, compiled from the files it actually lives in.
//!
//! `examples/revision-pair/` is two revisions of one specification that differ by **exactly four
//! changes, one per relation** — a grant added, a grant removed, an enum variant added, an enum
//! variant removed. It is the deliverable of this slice, not the type: the failure mode of a diff is
//! producing a plausible answer nobody checks, which is the same failure mode as a schema that
//! accepts everything, and this repository has shipped that defect three times.
//!
//! So this file asserts the delta's *content*, change by change, and not that it is non-empty. A
//! test that only counted four would pass against an engine that reported four wrong things.
//!
//! The pair is also built so that a text diff of it lies in both directions: the `after` revision
//! renames its domain file, reorders every top-level block, rewrites every comment, and writes out
//! one naming default and then leaves it out. `git diff` reports most of the file; the delta reports
//! four changes.

use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::scenario::EssSemanticRef;
use ess_diff::change::{ActorChange, SemanticChange, TypeChange};
use ess_diff::{diff, DiffRefusal, RawEssDelta, SemanticRelation};
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

/// One half of the fixture pair, compiled.
///
/// Read from `examples/` rather than inlined, on the argument `crates/ess-compiler/tests/billing.rs`
/// makes about the same choice: a copy of a specification inside a test drifts from the one a person
/// reads, and this pair's whole value is that a person can audit it by hand.
fn compiled(revision: &str) -> EssIr {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/revision-pair")
        .join(revision)
        .canonicalize()
        .unwrap_or_else(|error| panic!("`examples/revision-pair/{revision}` exists: {error}"));

    let mut files: Vec<PathBuf> = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the fixture is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                files.push(path);
            }
        }
    }
    files.sort();
    assert!(
        files.len() >= 3,
        "`{revision}` should be three documents, and {} were found",
        files.len()
    );

    let parsed: Vec<(Source, RawSpecFile)> = files
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).expect("a readable document");
            let label = path
                .strip_prefix(&root)
                .expect("under the fixture root")
                .display()
                .to_string();
            let raw = RawSpecFile::parse(&text)
                .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
            (Source::new(label), raw)
        })
        .collect();

    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("`{revision}` validates:\n{errors}"));
    compile(&specification, &SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("`{revision}` resolves:\n{diagnostics}"))
}

/// The delta the fixture pair produces.
fn delta() -> ess_diff::EssDelta {
    diff(&compiled("before"), &compiled("after")).expect("the pair is two revisions of one system")
}

/// The change with this id, or a failure naming every id the delta does hold.
fn change(delta: &ess_diff::EssDelta, id: &str) -> SemanticChange {
    delta
        .changes()
        .iter()
        .find(|change| change.id().to_string() == id)
        .cloned()
        .unwrap_or_else(|| {
            let held: Vec<String> = delta
                .changes()
                .iter()
                .map(|change| change.id().to_string())
                .collect();
            panic!(
                "the delta holds no `{id}`; it holds:\n  {}",
                held.join("\n  ")
            )
        })
}

#[test]
fn the_fixture_pair_differs_by_exactly_four_changes() {
    let delta = delta();

    let held: Vec<String> = delta
        .changes()
        .iter()
        .map(|change| change.id().to_string())
        .collect();
    assert_eq!(
        delta.len(),
        4,
        "the pair is built to differ by four changes and nothing else; it reported:\n  {}",
        held.join("\n  ")
    );
    assert_eq!(delta.count(SemanticRelation::Expanded), 2);
    assert_eq!(delta.count(SemanticRelation::Narrowed), 2);
    assert_eq!(
        delta.count(SemanticRelation::Changed),
        0,
        "one change per relation, so nothing is left over"
    );
}

#[test]
fn adding_an_enum_variant_widens_the_type_that_accepts_it() {
    let delta = delta();
    let change = change(&delta, "type/catalog.pricing.Currency/variant-added/CHF");

    let SemanticChange::Type { subject, changed } = &change else {
        panic!("a currency change is a type change, and this is {change:?}");
    };
    assert_eq!(subject.to_string(), "catalog.pricing.Currency");
    assert_eq!(
        changed,
        &TypeChange::VariantAdded {
            variant: "CHF".to_owned()
        }
    );
    assert_eq!(change.relation(), SemanticRelation::Expanded);
    assert_eq!(
        change.subject(),
        Some(EssSemanticRef::Type {
            name: subject.clone()
        }),
        "the subject is the semantic name a conformance scenario records, not a string"
    );
}

#[test]
fn removing_an_enum_variant_narrows_the_type_that_accepted_it() {
    let delta = delta();
    let change = change(&delta, "type/catalog.pricing.Currency/variant-removed/GBP");

    let SemanticChange::Type { changed, .. } = &change else {
        panic!("a currency change is a type change, and this is {change:?}");
    };
    assert_eq!(
        changed,
        &TypeChange::VariantRemoved {
            variant: "GBP".to_owned()
        }
    );
    assert_eq!(
        change.relation(),
        SemanticRelation::Narrowed,
        "a value that used to parse and no longer does is a narrowing, and the direction is the \
         whole point of reporting it"
    );
}

#[test]
fn granting_a_command_to_an_actor_widens_what_the_system_permits() {
    let delta = delta();
    let change = change(
        &delta,
        "actor/catalog.pricing.PricingManager/grant-added/catalog.pricing.RetirePriceList",
    );

    let SemanticChange::Actor { subject, changed } = &change else {
        panic!("a grant is an actor change, and this is {change:?}");
    };
    assert_eq!(subject.to_string(), "catalog.pricing.PricingManager");
    let ActorChange::GrantAdded { command } = changed else {
        panic!("this is the grant that was added, and it is {changed:?}");
    };
    assert_eq!(command.to_string(), "catalog.pricing.RetirePriceList");
    assert_eq!(change.relation(), SemanticRelation::Expanded);
}

#[test]
fn taking_a_command_from_an_actor_narrows_what_the_system_permits() {
    let delta = delta();
    let change = change(
        &delta,
        "actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList",
    );

    let SemanticChange::Actor { subject, changed } = &change else {
        panic!("a grant is an actor change, and this is {change:?}");
    };
    assert_eq!(subject.to_string(), "catalog.pricing.Auditor");
    let ActorChange::GrantRemoved { command } = changed else {
        panic!("this is the grant that was removed, and it is {changed:?}");
    };
    assert_eq!(command.to_string(), "catalog.pricing.RetirePriceList");
    assert_eq!(
        change.relation(),
        SemanticRelation::Narrowed,
        "an authorization that now fails is the direction a reviewer looks for first"
    );
}

#[test]
fn nothing_the_after_revision_only_rewrote_reaches_the_delta() {
    // The claim design §7 makes, and the reason the fixture is shaped the way it is. Between the two
    // revisions: the domain file is renamed, every top-level block is in a different order, every
    // comment is rewritten, and `display: Auditor` is written out on one side and left to the
    // model's own fallback on the other. None of it is a change to the system.
    //
    // Asserted by naming what a delta *would* have said about each, so this fails loudly if one of
    // them ever starts being reported rather than merely making the count wrong.
    let delta = delta();
    let reported: Vec<String> = delta
        .changes()
        .iter()
        .map(|change| change.id().to_string())
        .collect();

    for silent in [
        "actor/catalog.pricing.Auditor/display-name-changed",
        "event/catalog.pricing.PriceListPublished/field-order-changed",
        "type/catalog.pricing.Currency/variant-order-changed",
        "component/pricing-service/accepts-added/catalog.pricing.RetirePriceList",
    ] {
        assert!(
            !reported.contains(&silent.to_owned()),
            "`{silent}` is source noise, not a semantic change"
        );
    }

    // And the file rename itself: identity comes from `domain:`, not from the filename, so the
    // domain is the same domain and every construct in it kept its owner.
    assert!(
        !reported
            .iter()
            .any(|id| id.contains("domain-changed") || id.contains("/removed")),
        "renaming the file that declares a domain removed something: {reported:?}"
    );
}

#[test]
fn a_revision_compared_with_itself_reports_nothing() {
    let delta = diff(&compiled("after"), &compiled("after")).expect("one system");

    assert!(
        delta.is_empty(),
        "identical semantic IR must produce an empty delta, and this one holds {}",
        delta.len()
    );
    assert_eq!(delta.before.spec_digest, delta.after.spec_digest);
}

#[test]
fn two_different_systems_are_refused_rather_than_reported_as_a_rewrite() {
    // The state the rule is load-bearing in: both sides are real specifications that compile, so
    // there is a delta available to produce — every construct of one added and every construct of
    // the other removed. That answer is plausible, enormous, and about a question nobody asked.
    let billing = {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/billing")
            .canonicalize()
            .expect("the billing example exists");
        let mut files: Vec<PathBuf> = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).expect("readable") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|it| it == "yaml") {
                    files.push(path);
                }
            }
        }
        files.sort();
        let parsed: Vec<(Source, RawSpecFile)> = files
            .iter()
            .map(|path| {
                let text = std::fs::read_to_string(path).expect("readable");
                let raw = RawSpecFile::parse(&text).expect("well formed");
                (Source::new(path.display().to_string()), raw)
            })
            .collect();
        let specification = Specification::assemble(parsed).expect("billing validates");
        compile(&specification, &SourceMap::new()).expect("billing resolves")
    };
    let catalog = compiled("before");
    assert_ne!(billing.system, catalog.system, "the fixture is two systems");
    assert!(
        !billing.commands.is_empty() && !catalog.commands.is_empty(),
        "both sides declare something, so a delta was available to produce"
    );

    let refusal = diff(&billing, &catalog).expect_err("two systems are refused");

    assert_eq!(
        refusal,
        DiffRefusal::DifferentSystem {
            before: billing.system.clone(),
            after: catalog.system.clone(),
        }
    );
    assert!(
        refusal.to_string().contains("billing") && refusal.to_string().contains("catalog"),
        "the refusal names both systems: {refusal}"
    );
}

#[test]
fn the_delta_survives_being_written_and_read_back() {
    // Invariant 2's other half. The document is what a later wave reads, so the round trip is what
    // makes the `RawEssDelta` pair worth having: every id and every relation in the file is checked
    // against what the change beside it derives, and the delta that comes back is the one that went
    // out.
    let original = delta();
    let json = original.to_canonical_json();

    let raw: RawEssDelta = serde_json::from_str(&json).expect("the document parses");
    let read_back = ess_diff::EssDelta::try_from(raw)
        .unwrap_or_else(|errors| panic!("the document this build wrote validates:\n{errors}"));

    assert_eq!(read_back, original);
    assert_eq!(read_back.to_canonical_json(), json);
}
