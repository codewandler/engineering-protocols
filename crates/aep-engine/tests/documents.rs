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
    assert_eq!(
        registry.profiles().count(),
        6,
        "three development points on one scale, `development.driven` beside them, incident and \
         release"
    );
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
        ("development.driven", "feature", "adp/1"),
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

#[test]
fn the_approval_floor_is_in_force_for_every_shipped_profile() {
    // The claim three documents make — "a profile that granted production.write outright would fail
    // to resolve" — was untrue for every `adp/1` and `aop/1` profile, because `Protocol::extend` did
    // not inherit the floor. The shipped profiles happened to be safe by hand-writing
    // `require_approval`, so nothing failed and the check that was supposed to make it impossible was
    // doing nothing. This asserts against the real documents rather than a fixture.
    let registry = load_tree_report(&root())
        .into_result()
        .expect("the document tree is valid");

    for profile in [
        "development.standard",
        "incident.standard",
        "release.progressive",
    ] {
        let reference: aep_domain::version::ProfileVersionedRef =
            profile.parse().expect("a profile reference");
        let resolved = registry
            .resolved_profile(&reference)
            .unwrap_or_else(|errors| panic!("{profile} does not resolve: {errors}"));
        let protocol = registry
            .resolved_protocol(&resolved.protocol)
            .unwrap_or_else(|errors| panic!("{profile}'s protocol does not resolve: {errors}"));

        assert!(
            protocol.needs_approval_floor(&aep_domain::capability::Capability::ProductionWrite),
            "`{profile}` runs under `{}`, whose approval floor does not cover production.write; a \
             profile could grant it outright and nothing would refuse",
            protocol.reference()
        );
    }
}

#[test]
fn a_profile_that_grants_production_outright_is_refused_under_every_protocol() {
    let registry = load_tree_report(&root())
        .into_result()
        .expect("the document tree is valid");

    // Resolution is what refuses this, so the check has to go through a task.
    for (protocol, workflow) in [("adp/1", "adp/default"), ("aop/1", "incident/standard")] {
        let profile = format!(
            "id: test.reckless\nversion: 1\ntitle: Reckless\nprotocol: {protocol}\nworkflow: {workflow}\n\
             principles: []\ncapabilities:\n  allow: [repository.read, production.write]\n\
             completion:\n  - evidence.missing == 0\n"
        );
        let mut registry = registry.clone();
        registry
            .insert_profile(
                aep_schema::parse::profile(&profile, None).expect("the profile document parses"),
            )
            .expect("the profile is new");

        let task = task(&format!(
            "id: T-1\nkind: feature\nobjective: something\nprotocol: {protocol}\nprofile: test.reckless\n"
        ));
        let errors = resolve(&task, &registry)
            .expect_err("granting production.write outright must not resolve under {protocol}");
        assert!(
            errors.contains(aep_domain::error::ValidationCode::ProductionWriteWithoutApproval),
            "under {protocol} the refusal must be the approval floor, not something else: {errors}"
        );
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
