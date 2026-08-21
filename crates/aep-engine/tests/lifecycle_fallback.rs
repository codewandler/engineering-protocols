//! The lifecycle a tree falls back to when nothing nearer governs a kind.
//!
//! [`ArtifactKind`](aep_domain::artifact::ArtifactKind) is an open vocabulary: a team may name a
//! kind this crate has never heard of, and the engine accepts it. What used to happen next is that
//! the kind was governed by *nothing* — no lifecycle document could reach it, so every status was
//! legal and a misspelt one was not a refusal but a shrug. A lifecycle document with no `kind:` is
//! how a tree says what holds for the kinds nobody enumerated, and these tests are its reachability:
//! a tree on disk, loaded the way the CLI loads one, answering for a kind it never names.

use std::path::{Path, PathBuf};

use aep_domain::artifact::{
    Artifact, ArtifactGraph, ArtifactId, ArtifactKind, ArtifactLocation, ArtifactStatus,
};
use aep_engine::load_tree_report;

/// Builds a throwaway document tree, holding only the lifecycles each test writes into it.
fn tree(name: &str, lifecycles: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("aep-lifecycle-fallback-{name}"));
    std::fs::remove_dir_all(&root).ok();
    let directory = root.join("artifacts/lifecycles");
    std::fs::create_dir_all(&directory).expect("the tree is writable");
    for (file, contents) in lifecycles {
        std::fs::write(directory.join(file), contents).expect("the document is writable");
    }
    root
}

/// A lifecycle for kinds a tree does name: `log`, and by lineage every `*-log`.
const LOG: &str = "\
kind: log
initial: draft
transitions:
  draft: [active]
  active: [archived]
";

/// The same document with its `kind:` taken off — the fallback.
const FALLBACK: &str = "\
initial: draft
transitions:
  draft: [proposed]
  proposed: [accepted]
";

/// A second one, which a tree may not have.
const SECOND_FALLBACK: &str = "\
initial: proposed
transitions:
  proposed: [rejected]
";

fn artifact(id: &str, kind: &str, status: ArtifactStatus) -> Artifact {
    Artifact::new(
        ArtifactId::new(id).expect("artifact id"),
        ArtifactKind::parse(kind).expect("artifact kind"),
        status,
        ArtifactLocation::Inline,
    )
}

fn clean(root: &Path) {
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_kind_less_document_governs_every_kind_nothing_nearer_names() {
    let root = tree(
        "reachable",
        &[("log.yaml", LOG), ("fallback.yaml", FALLBACK)],
    );
    let outcome = load_tree_report(&root);
    assert!(
        outcome.failures.is_empty(),
        "a lifecycle document without `kind:` is the fallback, not a broken document: {}",
        outcome
            .failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    );

    let lifecycles = outcome.registry.lifecycles();
    assert_eq!(lifecycles.len(), 2, "one kinded lifecycle and one fallback");

    // The kind nobody registered, and the point of the whole mechanism.
    let digest = ArtifactKind::parse("weekly-digest").expect("kind");
    let governing = lifecycles
        .for_kind(&digest)
        .expect("an unregistered kind is governed by the fallback");
    assert_eq!(governing.initial, ArtifactStatus::Draft);
    assert!(governing.permits_transition(ArtifactStatus::Draft, ArtifactStatus::Proposed));
    assert!(
        !governing.permits(ArtifactStatus::Active),
        "the fallback is a lifecycle somebody wrote, not a permissive one"
    );

    // Nearer wins: the lineage rule finds `log` before the fallback is consulted.
    let observation_log = ArtifactKind::parse("observation-log").expect("kind");
    assert_eq!(
        lifecycles.for_kind(&observation_log),
        lifecycles.for_kind_exact(&ArtifactKind::parse("log").expect("kind")),
        "a lifecycle registered on the parent kind beats the fallback"
    );

    clean(&root);
}

#[test]
fn without_the_fallback_document_the_same_kind_is_governed_by_nothing() {
    // The guard, broken: the same tree minus one file. If this still resolved, the test above
    // would be passing on the lineage rule rather than on the fallback.
    let root = tree("absent", &[("log.yaml", LOG)]);
    let outcome = load_tree_report(&root);
    assert!(outcome.failures.is_empty());

    let digest = ArtifactKind::parse("weekly-digest").expect("kind");
    assert!(outcome.registry.lifecycles().for_kind(&digest).is_none());
    assert!(outcome.registry.lifecycles().fallback().is_none());

    clean(&root);
}

#[test]
fn a_second_kind_less_document_is_one_refusal_and_the_first_still_stands() {
    let root = tree(
        "duplicate",
        &[
            ("a-fallback.yaml", FALLBACK),
            ("b-fallback.yaml", SECOND_FALLBACK),
        ],
    );
    let outcome = load_tree_report(&root);

    assert_eq!(
        outcome.failures.len(),
        1,
        "exactly one refusal, naming the second document: {}",
        outcome
            .failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    );
    let failure = outcome.failures[0].to_string();
    assert!(failure.contains("duplicate_declaration"), "{failure}");
    assert!(failure.contains("b-fallback.yaml"), "{failure}");

    // The refusal is not a silent overwrite: the file read first is still the fallback.
    let fallback = outcome
        .registry
        .lifecycles()
        .fallback()
        .expect("the first document registered");
    assert_eq!(fallback.initial, ArtifactStatus::Draft);

    clean(&root);
}

#[test]
fn the_fallback_makes_a_status_on_an_unregistered_kind_refusable() {
    // What the mechanism is for: a manifest entry in a status the tree does not declare stops
    // being legal by default.
    let root = tree("manifest", &[("fallback.yaml", FALLBACK)]);
    let registry = load_tree_report(&root).registry;

    let graph = ArtifactGraph::build([artifact(
        "digest:2026-w34",
        "weekly-digest",
        ArtifactStatus::Active,
    )])
    .expect("the graph is well formed");

    let errors = graph.validate_lifecycles(registry.lifecycles());
    assert_eq!(
        errors.len(),
        1,
        "the fallback declares no `active`, so this one status is refused: {errors}"
    );
    assert!(errors.contains(aep_domain::error::ValidationCode::UnknownState));

    clean(&root);
}
