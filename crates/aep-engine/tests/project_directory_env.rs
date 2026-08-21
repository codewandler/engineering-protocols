//! Renaming the project directory with `AEP_PROJECT_DIR`.
//!
//! `.engineering` is a good default and a bad requirement: a repository whose team already calls
//! this directory something else had no way to be discovered at all, because the name was a
//! compile-time constant. It is now a default with an override, read once, at the one edge that
//! touches the filesystem.
//!
//! # Why this is a test binary of its own, with one test in it
//!
//! [`project_directory`](aep_engine::project::project_directory) reads the environment exactly
//! once per process — which is the property that makes a run coherent, and the property that makes
//! this untestable beside anything else. Cargo gives each integration test file its own binary, so
//! this file is the process where the variable is set, and the crate's unit tests are the process
//! where it never is. Both facts get asserted, in the place each is true.

use std::path::{Path, PathBuf};

use aep_engine::project::{discover, project_directory, PROJECT_DIRECTORY_ENV};

/// Writes a project rooted at `root`, keeping its metadata in `directory`.
fn project(root: &Path, directory: &str) {
    let metadata = root.join(directory);
    std::fs::create_dir_all(&metadata).expect("the tree is writable");
    std::fs::write(
        metadata.join("project.yaml"),
        "protocol: adp/1\nprofile: development.standard\nprotocols: ../..\n",
    )
    .expect("the document is writable");
}

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("aep-project-dir-env-{name}"));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).expect("the tree is writable");
    root
}

#[test]
fn the_environment_names_the_project_directory_and_nothing_else_is_a_project() {
    // Set before the first read, which is what the whole file exists to guarantee.
    std::env::set_var(PROJECT_DIRECTORY_ENV, ".workflow");
    assert_eq!(project_directory(), ".workflow");

    let root = scratch("workflow");
    project(&root, ".workflow");
    let nested = root.join("services/billing/src");
    std::fs::create_dir_all(&nested).expect("the tree is writable");

    assert_eq!(
        discover(&nested),
        Some(root.clone()),
        "walk-up discovery finds the renamed directory from anywhere inside the project"
    );

    // The guard, broken the other way: with the override in force, the default name is not a
    // project. Without this, the test above would pass even if the override were ignored.
    let default_named = scratch("still-engineering");
    project(&default_named, ".engineering");
    assert_eq!(
        discover(&default_named),
        None,
        "`AEP_PROJECT_DIR` renames the directory, it does not add a second one"
    );

    // The value is read once: a later change to the variable cannot move a running process's
    // project out from under it.
    std::env::set_var(PROJECT_DIRECTORY_ENV, ".somewhere-else");
    assert_eq!(project_directory(), ".workflow");
    assert_eq!(discover(&nested), Some(root.clone()));

    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&default_named).ok();
}
