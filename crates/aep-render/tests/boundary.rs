//! Decision 1 of the renderer plan, as a check rather than as a paragraph.
//!
//! **`aep-render` depends on `aep-domain` and on nothing else in this workspace.** That is not a
//! tidiness preference: `aep-schema → aep-driver → aep-engine → aep-schema` is a cycle `cargo`
//! refuses, and review finding **F1** is what found it. A renderer that could see `aep-engine` (for
//! `Snapshot`) or `aep-driver-spec` (for `DriverCursor`) would sit on the far side of it, and the
//! first person to need a picture inside the engine would discover that at link time. The seam that
//! avoids it is [`aep_render::RunView`] — a plain struct the *caller* fills — and this test is what
//! stops somebody quietly replacing it with the real thing.
//!
//! The manifest is read rather than the sources, because a `use aep_engine::…` cannot compile until
//! the dependency is in the manifest: the manifest is where the decision is actually taken.

use std::path::Path;

/// The one workspace crate this one may name.
const ALLOWED: &str = "aep-domain";

/// Every workspace dependency the manifest declares, and which section it is in.
///
/// A hand-rolled reader over the two `[dependencies]` sections rather than a TOML parser, on the
/// workspace rule *prefer no dependency, and record the refusal*: `toml` as a dev-dependency would
/// be a third-party crate added to make one assertion about eleven lines of text.
fn declared(manifest: &str) -> Vec<(String, String)> {
    let mut section = String::new();
    let mut found = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') {
            trimmed.trim_matches(['[', ']']).clone_into(&mut section);
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        // `aep-domain.workspace = true` and `serde_yaml.workspace = true` both put the crate name
        // before the first dot.
        let crate_name = name.split('.').next().unwrap_or(name).trim();
        if section.ends_with("dependencies") {
            found.push((crate_name.to_owned(), section.clone()));
        }
    }
    found
}

#[test]
fn the_renderer_names_no_workspace_crate_but_the_domain() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("the crate has a manifest");
    let declared = declared(&manifest);
    // The fixture has to have found the dependency sections at all, or the assertion below is
    // vacuously true over an empty list.
    assert!(
        declared.iter().any(|(name, _)| name == ALLOWED),
        "the reader found no `{ALLOWED}`, so it is not reading the manifest: {declared:?}"
    );
    let workspace_crates: Vec<&(String, String)> = declared
        .iter()
        .filter(|(name, _)| {
            (name.starts_with("aep-")
                || name.starts_with("ess-")
                || name.starts_with("infra-")
                || name.starts_with("trace-")
                || name.starts_with("adp-")
                || name.starts_with("aop-"))
                && name != ALLOWED
        })
        .collect();
    assert!(
        workspace_crates.is_empty(),
        "a renderer that can see these is a renderer on the far side of the cycle review finding \
         F1 found; the run overlay arrives as `RunView` instead: {workspace_crates:?}"
    );
}

#[test]
fn the_reader_sees_a_dependency_and_ignores_a_comment_about_one() {
    let sample = "[package]\nname = \"x\"\n\n[dependencies]\n# aep-engine would close the cycle\n\
                  aep-domain.workspace = true\n\n[dev-dependencies]\nserde_yaml.workspace = true\n";
    let found = declared(sample);
    assert_eq!(
        found,
        vec![
            ("aep-domain".to_owned(), "dependencies".to_owned()),
            ("serde_yaml".to_owned(), "dev-dependencies".to_owned()),
        ],
        "both sections are read, and prose about a refused crate is not a dependency on it"
    );
    assert!(
        !found.iter().any(|(name, _)| name == "name"),
        "`name = \"x\"` is in `[package]`, not in a dependency section"
    );
}
