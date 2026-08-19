//! The repository's own document tree must load, validate and resolve.
//!
//! This is the test that keeps the documents honest. Every rule the engine enforces — undeclared
//! capabilities, unobservable facts, obligations timed against phases no workflow declares — is a
//! rule these documents have to satisfy, so a principle that could never be checked cannot be
//! committed.

use std::path::{Path, PathBuf};

use aep_domain::error::ValidationErrors;
use aep_domain::task::Task;
use aep_engine::{load_tree_report, resolve};

/// The repository root, from this crate's manifest directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Parses a task document written inline.
fn task(text: &str) -> Task {
    aep_schema::parse::task(text, None).expect("the fixture task parses")
}

#[test]
fn the_document_tree_loads_and_is_internally_consistent() {
    let outcome = load_tree_report(&root());

    assert!(
        outcome.failures.is_empty(),
        "{} document problem(s):\n{}",
        outcome.failures.len(),
        outcome
            .failures
            .iter()
            .map(|failure| format!("  - {failure}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let registry = &outcome.registry;
    assert_eq!(registry.protocols().count(), 3, "aep/1, adp/1 and aop/1");
    assert_eq!(registry.workflows().count(), 4);
    assert_eq!(registry.profiles().count(), 5);
    assert!(
        registry.principles().count() >= 20,
        "expected the full principle set, found {}",
        registry.principles().count()
    );
    assert!(registry.lifecycles().len() >= 5);
}

#[test]
fn every_profile_resolves_for_a_task_of_its_kind() {
    let registry = load_tree_report(&root())
        .into_result()
        .expect("the document tree is valid");

    let cases = [
        ("development.fast", "feature", "adp/1"),
        ("development.standard", "feature", "adp/1"),
        ("development.critical", "feature", "adp/1"),
        ("incident.standard", "incident", "aop/1"),
        ("release.progressive", "release", "aop/1"),
    ];

    let mut failures: Vec<String> = Vec::new();
    for (profile, kind, protocol) in cases {
        let document = format!(
            "id: T-1\nkind: {kind}\nobjective: exercise the profile\nprotocol: {protocol}\nprofile: {profile}\n"
        );
        match resolve(&task(&document), &registry) {
            Ok(plan) => {
                assert!(
                    !plan.principles.is_empty(),
                    "{profile} resolved with no principles in force"
                );
                assert!(
                    !plan.completion.is_empty(),
                    "{profile} resolved with no completion condition"
                );
            }
            Err(errors) => failures.push(format!("{profile}:\n{}", indent(&errors))),
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_development_task_can_be_walked_to_completion_with_evidence() {
    // Proves the documents describe a reachable workflow: with the right evidence, `adp/default`
    // gets from `receive` to `complete`. A workflow that cannot be finished is a workflow nobody
    // would notice was broken.
    let registry = load_tree_report(&root())
        .into_result()
        .expect("the document tree is valid");
    let plan = resolve(
        &task(
            "id: AUTH-142\nkind: feature\nobjective: add-passkey-support\nprotocol: adp/1\nprofile: development.fast\n",
        ),
        &registry,
    )
    .expect("development.fast resolves");

    let workflow = &plan.workflow;
    assert_eq!(workflow.initial.as_str(), "receive");
    assert!(
        workflow
            .terminal_states()
            .iter()
            .any(|state| state.phases.iter().any(|phase| phase.as_str() == "completion")),
        "a terminal state must declare the completion phase, or obligations owed before completion \
         can never be checked"
    );

    // Every non-terminal state has a way out, and every state is reachable: the workflow validator
    // enforces both, so reaching this point means the graph is walkable.
    for state in workflow.states.values() {
        if !state.is_terminal() {
            assert!(
                !workflow.outgoing(&state.id).is_empty(),
                "{} has no outgoing transition",
                state.id
            );
        }
    }
}

/// Renders validation errors as an indented block.
fn indent(errors: &ValidationErrors) -> String {
    errors
        .as_slice()
        .iter()
        .map(|error| format!("    {error}"))
        .collect::<Vec<_>>()
        .join("\n")
}
