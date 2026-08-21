//! One specification, two applications, one surface — the byte-level half of W7.5.
//!
//! # What is checked here, and what is checked in the gate
//!
//! These tests read bytes. That the two applications actually *start*, answer the same requests
//! with the same statuses and the same bodies, and publish the same two documents is checked by
//! `cargo xtask synth`, which builds both from the committed trees and their hand-written
//! realizations and drives them — the same division the other two targets already use, where
//! compiling and running belong to the task that owns the committed tree.
//!
//! What a byte test *can* settle is the thing a running test cannot: that the routes a server
//! answers are the routes the published contract declares, from one mapping rather than two, and
//! that the specification-derived half of the startup record is one string in both trees rather
//! than two strings that happen to match today.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile_locating;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_gen::http;
use ess_synth::{synthesize_for, CapabilityKind, Synthesis, Target};

/// One example directory, compiled where it lives.
fn example(name: &str) -> EssIr {
    let base: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
        .canonicalize()
        .unwrap_or_else(|error| panic!("the `{name}` example exists: {error}"));

    let mut labels = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                labels.push(
                    path.strip_prefix(&base)
                        .expect("inside the example")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    labels.sort();

    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in &labels {
        let text = std::fs::read_to_string(base.join(label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label.clone()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the `{name}` specification validates:\n{errors}"));
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("the `{name}` specification resolves:\n{diagnostics}"))
}

/// The demonstration specification: one component, reached over a network.
fn gatepass() -> EssIr {
    example("gatepass")
}

/// The normative example: every component reached in process.
fn billing() -> EssIr {
    example("billing")
}

/// One synthesis, by target.
fn synthesized(ir: &EssIr, target: Target) -> Synthesis {
    synthesize_for(ir, target)
}

/// One artifact's contents, by path.
fn file<'a>(synthesis: &'a Synthesis, path: &str) -> &'a str {
    &synthesis
        .artifacts
        .get(path)
        .unwrap_or_else(|| {
            panic!(
                "no artifact at `{path}`; the tree holds {}",
                synthesis
                    .artifacts
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .contents
}

#[test]
fn a_specification_that_says_nothing_about_reach_gets_no_server_at_all() {
    // The normative example, unchanged by this whole wave: every component of it is reached in
    // process, so there is no surface to serve and nothing is emitted. Asserted rather than
    // assumed, because the alternative — a server crate every specification acquires — would put
    // an HTTP listener in a tree whose specification never asked for one.
    for target in [Target::Rust, Target::Go] {
        let synthesis = synthesized(&billing(), target);
        let served: Vec<&String> = synthesis
            .artifacts
            .keys()
            .filter(|path| path.contains("server"))
            .collect();
        assert!(
            served.is_empty(),
            "{} emitted a server for a specification that declares no network surface: {served:?}",
            target.name()
        );
        assert!(
            !synthesis
                .plan
                .is_generated(CapabilityKind::ComponentTransport, "invoice-service"),
            "and the plan does not owe one either"
        );
    }
}

#[test]
fn the_routes_a_server_answers_are_the_routes_the_contract_declares() {
    // The agreement that makes `GET /openapi.json` worth serving. Not "the two look similar": the
    // emitted table and the document's `paths` are read out of the two artifacts and compared as
    // sets, so a path served and not published — or published and not served — fails here.
    let ir = gatepass();
    let component = ir
        .components
        .values()
        .next()
        .expect("the demonstration declares a component");

    let declared: BTreeSet<(String, String)> = http::routes(&ir, component)
        .iter()
        .map(|route| (route.method.as_str().to_owned(), route.path.clone()))
        .collect();
    assert!(
        !declared.is_empty(),
        "the fixture must reach the state where the rule decides anything"
    );

    let rust = synthesized(&ir, Target::Rust);
    let table = file(&rust, "crates/gatepass-server/src/pass_service.rs");
    let served: BTreeSet<(String, String)> = table
        .lines()
        .skip_while(|line| !line.starts_with("pub const ROUTES"))
        .skip(1)
        .take_while(|line| !line.starts_with("];"))
        .map(|line| {
            let mut parts = line
                .trim()
                .trim_end_matches("),")
                .trim_start_matches('(')
                .split(", ");
            (
                parts.next().expect("a method").trim_matches('"').to_owned(),
                parts.next().expect("a path").trim_matches('"').to_owned(),
            )
        })
        .collect();

    // The two documents about the surface itself are in the emitted table and in no specification,
    // so they are added rather than expected of `routes`.
    let mut expected = declared.clone();
    expected.insert(("GET".to_owned(), http::OPENAPI.to_owned()));
    expected.insert(("GET".to_owned(), http::DOCS.to_owned()));
    assert_eq!(
        served, expected,
        "the emitted route table and the published contract are one mapping, or they are two \
         answers that agree only today"
    );

    let contract = file(
        &rust,
        "crates/gatepass-server/src/pass-service.openapi.json",
    );
    let document: serde_json::Value =
        serde_json::from_str(contract).expect("the served contract is JSON");
    for (method, path) in &declared {
        assert!(
            document["paths"][path][method.to_lowercase()].is_object(),
            "`{method} {path}` is served and the contract does not declare it"
        );
    }
}

#[test]
fn the_served_contract_is_the_document_the_projection_publishes() {
    let ir = gatepass();
    let component = ir
        .components
        .values()
        .next()
        .expect("the demonstration declares a component");
    for target in [Target::Rust, Target::Go] {
        let synthesis = synthesized(&ir, target);
        let path = match target {
            Target::Rust => "crates/gatepass-server/src/pass-service.openapi.json",
            _ => "server/pass-service.openapi.json",
        };
        assert_eq!(
            file(&synthesis, path),
            ess_gen::openapi::json(&ir, component),
            "{} embeds something other than the document `ess-gen` publishes",
            target.name()
        );
    }
}

#[test]
fn both_applications_carry_the_same_startup_record_outside_the_runtime_they_append() {
    // The heart of the demonstration, as bytes: the specification-derived half of every startup
    // line is one string, embedded in both trees. The gate then starts both and compares what they
    // actually write, which is what catches a target appending its `runtime` wrongly — but if the
    // strings differed here, the two applications would be describing two systems.
    let ir = gatepass();
    let rust = synthesized(&ir, Target::Rust);
    let go = synthesized(&ir, Target::Go);

    let from_rust = startup_lines(file(&rust, "crates/gatepass-server/src/pass_service.rs"));
    let from_go = startup_lines(file(&go, "server/passservice.go"));
    assert_eq!(from_rust.len(), 3, "three lines: starting, serving, ready");
    assert_eq!(
        from_rust, from_go,
        "the two targets embed different startup records for one specification"
    );
    assert!(
        from_rust[0].contains("\"system\":\"gatepass\"")
            && from_rust[0].contains("\"model_digest\":"),
        "the first line names the system and the model it was synthesised from: {}",
        from_rust[0]
    );
    assert!(
        from_rust[1].contains("\"transport\":\"http/1.1\"")
            && from_rust[1].contains("\"reached_by\":\"network\""),
        "the second names the transport and the declaration that determined it: {}",
        from_rust[1]
    );
    for line in &from_rust {
        assert!(
            !line.contains("runtime"),
            "the embedded half carries no `runtime`: that member is the process's own, appended \
             at run time, and the comparison ignores exactly it — {line}"
        );
    }
}

/// The startup constants of one emitted surface file, unescaped, in order.
///
/// Read out of the emitted source rather than recomputed, because what this test is about is the
/// bytes each tree ships.
fn startup_lines(source: &str) -> Vec<String> {
    source
        .lines()
        .skip_while(|line| !line.contains("STARTUP") && !line.contains("StartupPassService = "))
        .skip(1)
        .take_while(|line| !line.starts_with(']') && !line.starts_with('}'))
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            serde_json::from_str::<String>(trimmed).ok()
        })
        .collect()
}

#[test]
fn a_browser_cannot_bind_a_socket_and_says_so_rather_than_emitting_one() {
    // The third target meets the same specification. It refuses the transport at the *target*
    // stage — a fact about a tab, not about the model — so a reader can tell "no target can do
    // this" from "this one cannot", and switching targets dissolves only the second.
    let synthesis = synthesized(&gatepass(), Target::Web);
    let report = synthesis
        .target
        .as_ref()
        .expect("the web target reports what it could not carry");
    let refusal = report
        .refusals
        .iter()
        .find(|refused| refused.capability.kind == CapabilityKind::ComponentTransport)
        .expect("a page holding the system in one tab cannot serve a network surface");
    assert_eq!(refusal.capability.source, "pass-service");
    assert!(
        refusal.detail.contains("binds no socket"),
        "the refusal says what a tab cannot do: {}",
        refusal.detail
    );
}

#[test]
fn emitting_a_served_surface_twice_is_byte_identical() {
    let ir = gatepass();
    for target in [Target::Rust, Target::Go, Target::Web] {
        let first = synthesized(&ir, target);
        let second = synthesized(&ir, target);
        assert_eq!(
            first.artifacts.keys().collect::<Vec<_>>(),
            second.artifacts.keys().collect::<Vec<_>>(),
            "{} emitted a different set of files the second time",
            target.name()
        );
        for (path, artifact) in &first.artifacts {
            assert_eq!(
                artifact.contents,
                second.artifacts[path].contents,
                "{} emitted `{path}` differently the second time",
                target.name()
            );
        }
    }
}

#[test]
fn the_plan_is_byte_identical_in_both_trees_of_the_demonstration() {
    // The seam, again, on the specification that added a construct to the model: `reached_by`
    // reached the *plan* as one capability with one disposition, and neither emitter's tree
    // renders that plan differently.
    let ir = gatepass();
    let rust = synthesized(&ir, Target::Rust);
    let go = synthesized(&ir, Target::Go);
    for path in ["PLAN.md", "plan.json"] {
        assert_eq!(
            file(&rust, path),
            file(&go, path),
            "`{path}` differs between the two trees"
        );
    }
    assert!(
        file(&rust, "plan.json").contains("component_transport"),
        "and the plan carries the transport capability the declaration determined"
    );
}
