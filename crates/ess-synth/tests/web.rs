//! The billing specification, emitted for the browser, from the files it actually lives in.
//!
//! What is checked here, and what is checked in the gate: these tests read bytes. Whether the
//! emitted crate *compiles for `wasm32-unknown-unknown`*, whether the page calls exactly the
//! exports the module has, and whether the whole crossing still works end to end are checked by
//! `cargo xtask synth`, which owns the committed tree, builds the module and drives it through the
//! page's own glue with Node — failing loudly when either toolchain is missing. That is the same
//! division the first two targets already use: `cargo check` inside the generated workspace and
//! `go build` inside the generated module are gate steps rather than unit tests, because a test
//! suite that shelled out to a compiler would make `cargo test` depend on a toolchain it has no
//! other reason to need.

use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile_locating;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_synth::{synthesize_for, CapabilityKind, Synthesis, Target};

/// The example directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
fn files() -> Vec<String> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(
                    path.strip_prefix(&base)
                        .expect("inside the example")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    found.sort();
    found
}

/// The billing example, compiled where it lives.
fn billing() -> EssIr {
    let labels = files();
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in &labels {
        let text = std::fs::read_to_string(example().join(label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label.clone()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

/// An inline fixture, assembled and compiled from YAML strings.
fn fixture(documents: &[(&str, &str)]) -> EssIr {
    let mut sources = SourceMap::new();
    let mut labels = Vec::new();
    let mut parsed = Vec::new();
    for (label, text) in documents {
        let raw = RawSpecFile::parse(text)
            .unwrap_or_else(|error| panic!("the fixture `{label}` is well formed: {error}"));
        sources.insert((*label).to_owned(), (*text).to_owned());
        labels.push((*label).to_owned());
        parsed.push((Source::new((*label).to_owned()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the fixture validates:\n{errors}"));
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("the fixture resolves:\n{diagnostics}"))
}

/// The billing specification, emitted for the browser.
fn web() -> Synthesis {
    synthesize_for(&billing(), Target::Web)
}

/// One artifact's contents, by path.
fn artifact(synthesis: &Synthesis, path: &str) -> String {
    synthesis
        .artifacts
        .get(path)
        .unwrap_or_else(|| {
            panic!(
                "`{path}` is not among the emitted artifacts: {}",
                synthesis
                    .artifacts
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .contents
        .clone()
}

/// The catalogue, parsed.
fn catalog(synthesis: &Synthesis) -> serde_json::Value {
    serde_json::from_str(&artifact(synthesis, "catalog.json")).expect("the catalogue is JSON")
}

// ---- the seam ---------------------------------------------------------------------------------

#[test]
fn the_plan_is_byte_identical_in_all_three_targets_trees() {
    // The claim the seam exists to make, now with a third consumer that is not even a language:
    // if a browser boundary could not be emitted without changing the plan, the plan was never a
    // fact about the model.
    let ir = billing();
    let rust = synthesize_for(&ir, Target::Rust);
    let go = synthesize_for(&ir, Target::Go);
    let page = synthesize_for(&ir, Target::Web);
    for path in ["PLAN.md", "plan.json"] {
        assert_eq!(
            artifact(&rust, path),
            artifact(&page, path),
            "`{path}` differs between the Rust tree and the browser tree; the plan is \
             language-neutral, and a target that needed it changed would have refuted that"
        );
        assert_eq!(
            artifact(&go, path),
            artifact(&page, path),
            "`{path}` differs between the Go tree and the browser tree"
        );
    }
    assert_eq!(
        rust.plan, page.plan,
        "the planner produced two different plans for one specification"
    );
}

#[test]
fn the_web_target_reports_six_weakenings_and_refuses_nothing_of_billing() {
    let synthesis = web();
    let report = synthesis
        .target
        .as_ref()
        .expect("the web target reports what a browser could not carry");
    assert_eq!(report.target, "web");
    assert_eq!(
        report.weakenings.len(),
        6,
        "six rules, none of them about a language: {:?}",
        report
            .weakenings
            .iter()
            .map(|weakening| &weakening.guarantee)
            .collect::<Vec<_>>()
    );
    assert!(
        report.refusals.is_empty(),
        "every command of billing lands on exactly one component, so this target refuses nothing \
         of it: {:?}",
        report.refusals
    );
    let notes = artifact(&synthesis, "TARGET.md");
    assert!(
        notes.contains("`#![forbid(unsafe_code)]`") && notes.contains("`#[no_mangle]`"),
        "the weakening a reviewer will ask about first has to be in the document:\n{notes}"
    );
}

#[test]
fn every_weakening_is_visible_in_the_generated_source_and_not_only_in_the_report() {
    // A weakening recorded only beside the code is a weakening the next reader of the code does
    // not meet. Two of the six are properties of the crate itself, and both say so where they are.
    let synthesis = web();
    let library = artifact(&synthesis, "crates/billing-web/src/lib.rs");
    assert!(
        library.contains("# No `forbid(unsafe_code)`, and why"),
        "the crate that cannot forbid `unsafe` says so in its own documentation:\n{library}"
    );
    assert!(
        !library.contains("forbid(unsafe_code)]"),
        "and does not then declare the lint it just explained it cannot"
    );
    let code: String = library
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("unsafe"),
        "the property still holds even though the compiler no longer closes it:\n{code}"
    );
}

// ---- the page holds no model ---------------------------------------------------------------------

#[test]
fn the_page_names_no_construct_of_the_specification_it_was_generated_from() {
    // The whole argument for emitting a page rather than writing one: a command list typed into
    // HTML is wrong the first time the specification changes, silently, in the one artifact
    // nobody regenerates. So the page must contain none of the model — it renders the catalogue.
    let ir = billing();
    let page = artifact(&web(), "index.html");
    let mut named = Vec::new();
    for declared in ir
        .commands
        .keys()
        .chain(ir.events.keys())
        .chain(ir.errors.keys())
        .chain(ir.views.keys())
        .chain(ir.types.keys())
        .chain(ir.entities.keys())
    {
        if page.contains(&declared.to_string()) {
            named.push(declared.to_string());
        }
    }
    assert!(
        named.is_empty(),
        "the page names {} construct(s) of the specification instead of reading them from the \
         catalogue: {}",
        named.len(),
        named.join(", ")
    );
    assert!(
        page.contains("catalog.commands"),
        "and it builds the command list from the catalogue it asked the module for:\n{page}"
    );
}

#[test]
fn the_catalogue_carries_every_command_with_its_typed_input_and_every_declared_outcome() {
    let synthesis = web();
    let catalog = catalog(&synthesis);
    let commands = catalog["commands"].as_array().expect("commands");
    assert_eq!(commands.len(), 5, "billing declares five commands");
    let create = commands
        .iter()
        .find(|command| command["name"] == "billing.invoice.CreateInvoice")
        .expect("CreateInvoice is in the catalogue");
    assert_eq!(create["component"], "invoice-service");
    assert_eq!(create["dispatchable"], true);
    let input = create["input"].as_array().expect("a typed input");
    assert_eq!(input.len(), 2);
    assert_eq!(input[1]["name"], "amount");
    assert_eq!(input[1]["type"]["kind"], "declared");
    assert_eq!(input[1]["type"]["name"], "billing.invoice.Money");
    let outcomes = create["outcomes"].as_array().expect("declared outcomes");
    assert_eq!(
        outcomes.len(),
        2,
        "the refusal is beside the success, because a consumer that cannot see the refusal branch \
         handles only the happy path"
    );
    assert_eq!(outcomes[1]["error"], "billing.invoice.InvalidAmount");
    assert_eq!(
        create["behavior"]["disposition"], "obligation",
        "and the page can say whether a refusal is the system saying no or nobody having written \
         it yet"
    );
}

#[test]
fn the_catalogue_carries_the_lifecycle_and_says_where_instances_can_be_observed() {
    let catalog = catalog(&web());
    let entities = catalog["entities"].as_array().expect("entities");
    assert_eq!(entities.len(), 1);
    let invoice = &entities[0];
    assert_eq!(invoice["initial"], "Draft");
    assert_eq!(
        invoice["transitions"]
            .as_array()
            .expect("transitions")
            .len(),
        3,
        "every declared move is on the page, because the page cannot derive one"
    );
    assert_eq!(
        invoice["views"].as_array().expect("views").len(),
        2,
        "and the only way to see an instance is a declared view"
    );
}

// ---- the crossing ------------------------------------------------------------------------------

#[test]
fn every_generated_type_crosses_the_boundary_in_both_directions() {
    // A type can be reached from either side — an event carries it out, a command input brings it
    // in — and a partial pair is a hole the next specification falls into.
    let ir = billing();
    let synthesis = web();
    let wire = artifact(&synthesis, "crates/billing-web/src/wire.rs");
    let mut missing = Vec::new();
    for declared in ir.types.keys() {
        if !synthesis
            .plan
            .is_generated(CapabilityKind::DomainType, &declared.to_string())
        {
            continue;
        }
        let snake = snake_of(&declared.to_string());
        if !wire.contains(&format!("pub fn encode_{snake}(")) {
            missing.push(format!("encode_{snake}"));
        }
        if !wire.contains(&format!("pub fn decode_{snake}(")) {
            missing.push(format!("decode_{snake}"));
        }
    }
    assert!(
        missing.is_empty(),
        "{} generated type(s) cross the boundary in only one direction: {}",
        missing.len(),
        missing.join(", ")
    );
}

/// The wire function's name fragment for a qualified name, spelled as the emitter spells it.
///
/// Recomputed here rather than imported, deliberately: a test that asked the emitter what it
/// called something would pass whatever the emitter called it.
fn snake_of(name: &str) -> String {
    let pascal: String = name
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect();
    let mut out = String::new();
    for character in pascal.chars() {
        if character.is_uppercase() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

/// A specification whose event carries an optional field, a list and a map.
///
/// Billing exercises none of the three at a boundary: its optional fields belong to the entity's
/// data, which no view projects and no event carries, so without this fixture three emitted
/// crossings would ship untested.
fn shapes() -> EssIr {
    fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: shapes\nversion: v1\ndomains:\n  - shapes.core\n",
        ),
        (
            "core.yaml",
            "domain: shapes.core\ntypes:\n  - name: shapes.core.Tag\n    kind: newtype\n    of: \
             String\nevents:\n  - name: shapes.core.Recorded\n    fields:\n      - name: \
             note\n        type: Optional<String>\n      - name: tags\n        type: \
             List<shapes.core.Tag>\n      - name: labels\n        type: Map<Integer, \
             String>\ncommands:\n  - name: shapes.core.Record\n    input:\n      - name: \
             note\n        type: Optional<String>\n      - name: tags\n        type: \
             List<shapes.core.Tag>\n      - name: labels\n        type: Map<Integer, \
             String>\n    outcomes:\n      - name: done\n        \
             emits:\n          - shapes.core.Recorded\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: recorder\n    owns:\n      domains:\n        - \
             shapes.core\n    accepts:\n      commands:\n        - shapes.core.Record\n    \
             publishes:\n      events:\n        - shapes.core.Recorded\n",
        ),
    ])
}

#[test]
fn an_absent_optional_field_is_omitted_rather_than_sent_as_null() {
    // The published contract spells an absent optional field by leaving the name out of
    // `required`, not by a `null` branch — so two projections of one model would otherwise
    // disagree about what a value looks like.
    let wire = artifact(
        &synthesize_for(&shapes(), Target::Web),
        "crates/shapes-web/src/wire.rs",
    );
    assert!(
        wire.contains("if let Some(held0) = &value.note {"),
        "the optional member is written only when it is there:\n{wire}"
    );
    assert!(
        wire.contains("None | Some(json::Value::Null) => None,"),
        "and absent and `null` both mean absent on the way in:\n{wire}"
    );
}

#[test]
fn a_list_and_a_map_cross_as_the_shapes_json_already_has() {
    let wire = artifact(
        &synthesize_for(&shapes(), Target::Web),
        "crates/shapes-web/src/wire.rs",
    );
    assert!(
        wire.contains("out.push('[');")
            && wire.contains("for (index0, item0) in value.tags.iter().enumerate() {"),
        "a list is an array, element by element:\n{wire}"
    );
    assert!(
        wire.contains("json::push_text(out, &key0.to_string());"),
        "a map with a non-string key is still an object, because JSON has no other kind of key — \
         which is the same rendering the published schema's `propertyNames` constrains:\n{wire}"
    );
    assert!(
        wire.contains("json::key_integer(") && wire.contains("json::members_at("),
        "and the key is read back out of that text rather than assumed:\n{wire}"
    );
}

#[test]
fn a_tagged_union_crosses_where_the_published_schema_says_its_payload_sits() {
    let wire = artifact(&web(), "crates/billing-web/src/wire.rs");
    assert!(
        wire.contains("json::member(out, \"kind\");")
            && wire.contains("json::push_text(out, \"company\");")
            && wire.contains("json::member(out, \"value\");"),
        "adjacent tagging, with the payload under `value` — the layout `ess-gen` fixes and this \
         target reads from it rather than deciding again:\n{wire}"
    );
}

#[test]
fn the_bridge_names_no_realization_and_installs_none() {
    // Gap register D-2, reaching a page: the machinery does not choose. With nothing installed the
    // module runs the generated stubs and every command answers with the obligation it is owed,
    // which is the honest empty state rather than an empty screen.
    let library = artifact(&web(), "crates/billing-web/src/lib.rs");
    assert!(
        !library.contains("billing_realization") && !library.contains("install(Box::new"),
        "the emitted bridge must neither name an implementation of an obligation nor install \
         one:\n{library}"
    );
    assert!(
        library.contains("pub fn install(system: Box<dyn Bound>)"),
        "it offers a seam instead"
    );
    assert!(
        library
            .contains("billing_types::invoice::obligations::Unimplemented.create_invoice(input)"),
        "and its own default delegates to the generated stub, so the refusal a page shows is the \
         plan entry that stub names:\n{library}"
    );
}

#[test]
fn the_bridge_takes_no_dependency_because_the_gate_reaches_no_network() {
    let manifest = artifact(&web(), "crates/billing-web/Cargo.toml");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("the manifest declares dependencies")
        .1;
    for line in dependencies.lines().filter(|line| line.contains('=')) {
        assert!(
            line.contains("path = \""),
            "`cargo build` inside the emitted tree is a gate step, and a step that resolves a \
             crate is a step that reaches the network: {line}"
        );
    }
    assert!(
        artifact(&web(), "crates/billing-web/src/json.rs").contains("pub fn parse(text: &str)"),
        "which is why the JSON reader is emitted rather than depended on"
    );
}

// ---- what this target refuses ---------------------------------------------------------------------

/// A specification that declares a command no component accepts.
///
/// Billing exercises neither half of the rule: every one of its commands lands on exactly one
/// component. Without this fixture the target-stage refusal would ship untested, and the marking
/// exists precisely so a browser's limitation cannot masquerade as a fact about the model. Two
/// components accepting one command is the other half, and `ess-domain` refuses it outright —
/// §9 gives a command one handler — so the only reachable half is this one.
fn unclaimed() -> EssIr {
    fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: duo\nversion: v1\ndomains:\n  - duo.core\n",
        ),
        (
            "core.yaml",
            "domain: duo.core\nevents:\n  - name: duo.core.Done\n    fields: []\ncommands:\n  - \
             name: duo.core.Act\n    input: []\n    outcomes:\n      - name: done\n        \
             emits:\n          - duo.core.Done\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: north\n    owns:\n      domains:\n        - \
             duo.core\n    publishes:\n      events:\n        - duo.core.Done\n",
        ),
    ])
}

#[test]
fn a_command_no_component_accepts_is_refused_at_the_target_stage_and_gets_no_form() {
    let synthesis = synthesize_for(&unclaimed(), Target::Web);
    let report = synthesis.target.as_ref().expect("a target report");
    assert_eq!(
        report.refusals.len(),
        1,
        "one command, no port, no way for a page to reach it: {:?}",
        report.refusals
    );
    let refusal = &report.refusals[0];
    assert_eq!(refusal.capability.kind, CapabilityKind::CommandContract);
    assert_eq!(refusal.capability.source, "duo.core.Act");
    assert!(
        refusal
            .detail
            .contains("no component of this specification accepts this one"),
        "the refusal names the construct's own facts: {}",
        refusal.detail
    );
    assert_eq!(
        refusal.capability.kind,
        CapabilityKind::CommandContract,
        "and it is the contract that could not be reached, not the behaviour behind it"
    );

    let catalog: serde_json::Value =
        serde_json::from_str(&artifact(&synthesis, "catalog.json")).expect("the catalogue is JSON");
    let commands = catalog["commands"].as_array().expect("commands");
    assert_eq!(
        commands.len(),
        1,
        "the page still lists it: a surface that silently omits a declared command reads as \
         complete and is not"
    );
    assert_eq!(commands[0]["dispatchable"], false);
    assert!(
        commands[0]["refusal"]
            .as_str()
            .expect("the refusal travels with the entry")
            .contains("no port"),
        "and says why there is no form"
    );

    let wire = artifact(&synthesis, "crates/duo-web/src/wire.rs");
    assert!(
        !wire.contains("decode_command_duo_core_act"),
        "nothing is emitted to decode an input that can never be dispatched:\n{wire}"
    );
    let library = artifact(&synthesis, "crates/duo-web/src/lib.rs");
    assert!(
        !library.contains("\"duo.core.Act\" =>"),
        "and the dispatcher has no arm for it:\n{library}"
    );
}

// ---- determinism and provenance -------------------------------------------------------------------

#[test]
fn emitting_twice_is_byte_identical() {
    let ir = billing();
    let first = synthesize_for(&ir, Target::Web);
    let second = synthesize_for(&ir, Target::Web);
    assert_eq!(
        first.artifacts.keys().collect::<Vec<_>>(),
        second.artifacts.keys().collect::<Vec<_>>(),
        "two emissions of one specification produced different files"
    );
    for (path, artifact) in &first.artifacts {
        assert_eq!(
            artifact.contents, second.artifacts[path].contents,
            "`{path}` differs between two emissions of one specification"
        );
    }
}

#[test]
fn every_artifact_names_its_specification_and_the_verb_that_rewrites_it() {
    // An emitted file with no provenance is a file nobody can audit, and one naming the wrong verb
    // sends its reader to a command that rewrites a different tree.
    let synthesis = web();
    for (path, artifact) in &synthesis.artifacts {
        if path == "catalog.json" {
            // JSON has no comments: the catalogue carries its provenance as data, which is the
            // same choice `plan.json` and `target.json` make.
            let catalog: serde_json::Value =
                serde_json::from_str(&artifact.contents).expect("the catalogue is JSON");
            assert_eq!(catalog["provenance"]["system"], "billing", "{path}");
            continue;
        }
        if std::path::Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            || path == "PLAN.md"
        {
            // The plan's own renderings are byte-identical in every target's tree, so they name
            // the verb that rewrites all three. A per-target verb in them would be the one thing
            // the seam exists to refute.
            continue;
        }
        assert!(
            artifact.contents.contains("model digest"),
            "`{path}` carries no provenance"
        );
        assert!(
            artifact
                .contents
                .contains("protocol ess synthesize --target web"),
            "`{path}` names the wrong regeneration verb"
        );
    }
}

#[test]
fn the_committed_tree_holds_no_compiled_module() {
    // The `.wasm` is a build artifact. Committing one would put a binary nobody can diff under a
    // drift check that compares bytes, and the gate builds it instead.
    for path in web().artifacts.keys() {
        assert!(
            !std::path::Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm")),
            "`{path}` is a compiled artifact and the emitter must not produce one"
        );
    }
}
