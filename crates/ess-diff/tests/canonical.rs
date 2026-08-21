//! Determinism, the format contract, and the checks a delta document is read back through.
//!
//! Design §59 and §60 claim four things about the output: `BTreeMap`/`BTreeSet` only, no clock, no
//! RNG, and an ordering that is a contract rather than an accident. Two of those are invisible from
//! inside a running test — a `HashMap` iterates the same way twice in a row and a timestamp only
//! differs across runs — so they are read for in the source rather than trusted, exactly as
//! `crates/ess-compiler/tests/billing.rs` reads for them.
//!
//! The third scan is this crate's own: no source file here may call an `EssIr` handle accessor. A
//! handle minted by the `before` IR is structurally identical to one minted by `after` and using it
//! against the wrong one panics, so for a diff engine the hazard is the normal case rather than an
//! edge case.

use std::path::{Path, PathBuf};

use aep_domain::error::ValidationCode;
use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::scenario::{
    ActorRef, BindingRef, CommandRef, ComponentRef, DeclaredTypeRef, DomainRef, EntityRef,
    ErrorRef, EventRef, ViewRef,
};
use ess_diff::change::{
    ActorChange, BindingChange, ChangeCategory, CommandChange, ComponentChange, EntityChange,
    ErrorChange, EventChange, SemanticChange, SystemChange, TypeChange, ViewChange,
};
use ess_diff::{diff, EssDelta, RawEssDelta};
use ess_domain::name::{QualifiedName, Version};
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

/// One half of the fixture pair, compiled from scratch.
fn compiled(revision: &str) -> EssIr {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/revision-pair")
        .join(revision)
        .canonicalize()
        .expect("the fixture exists");

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
    let specification = Specification::assemble(parsed).expect("the fixture validates");
    compile(&specification, &SourceMap::new()).expect("the fixture resolves")
}

/// The fixture pair's delta, from two fresh compilations.
fn delta() -> EssDelta {
    diff(&compiled("before"), &compiled("after")).expect("one system")
}

#[test]
fn diffing_the_same_pair_twice_produces_byte_identical_json() {
    // Four independent reads, assemblies and compilations, and two independent comparisons. Nothing
    // is shared between them, so an unordered map, a clock or an address-dependent iteration order
    // anywhere in the walk shows up here as a diff rather than as a rumour. Generalised from
    // `compiling_the_billing_example_twice_produces_byte_identical_json`.
    let first = delta().to_canonical_json();
    let second = delta().to_canonical_json();

    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "the same pair compared twice must be the same bytes"
    );
    assert!(
        first.len() > 500,
        "the delta is not empty: {} bytes",
        first.len()
    );
}

#[test]
fn canonical_json_ends_in_a_newline() {
    let json = delta().to_canonical_json();

    assert!(
        json.ends_with('\n'),
        "a generated file without a trailing newline is a file that shows up modified in the next \
         diff"
    );
    assert!(!json.ends_with("\n\n"), "one newline, not two");
}

#[test]
fn the_changes_are_written_in_the_category_order_and_not_the_alphabet() {
    // The fixture is chosen because it discriminates: it holds `type` changes and `actor` changes,
    // and design §60 puts `type` first while the alphabet puts `actor` first. A delta sorted by its
    // rendered id would come out the other way round, so this test fails under exactly the mistake
    // it exists to catch.
    let delta = delta();
    let categories: Vec<ChangeCategory> = delta
        .changes()
        .iter()
        .map(SemanticChange::category)
        .collect();

    assert_eq!(
        categories,
        vec![
            ChangeCategory::Type,
            ChangeCategory::Type,
            ChangeCategory::Entity,
            ChangeCategory::Command,
            ChangeCategory::Actor,
            ChangeCategory::Actor
        ]
    );

    let ids: Vec<String> = delta
        .changes()
        .iter()
        .map(|change| change.id().to_string())
        .collect();
    let mut alphabetical = ids.clone();
    alphabetical.sort();
    assert_ne!(
        ids, alphabetical,
        "this fixture must be one the alphabet orders differently, or the test passes whether the \
         contract holds or not"
    );
}

#[test]
fn every_change_in_a_delta_has_its_own_id() {
    let delta = delta();
    let mut ids: Vec<String> = delta
        .changes()
        .iter()
        .map(|change| change.id().to_string())
        .collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();

    assert_eq!(
        ids.len(),
        total,
        "two changes share an id, so anything stored against one names both"
    );
}

/// Every `.rs` file of this crate's `src/`, as `(path, contents)`, in a stable order.
fn sources() -> Vec<(String, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&directory)
        .expect("the crate has sources")
        .map(|entry| entry.expect("a readable entry").path())
        .filter(|path| path.extension().is_some_and(|it| it == "rs"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("a readable source file");
            (path.display().to_string(), text)
        })
        .collect();
    sources.sort();
    sources
}

#[test]
fn no_source_file_in_the_diff_engine_reads_a_clock_or_an_unordered_map() {
    let sources = sources();
    for (path, text) in &sources {
        for banned in ["HashMap", "HashSet", "SystemTime", "Instant", "rand::"] {
            assert!(
                !text.contains(banned),
                "{path} uses {banned}, which makes a delta depend on when and where it was built"
            );
        }
    }
    assert!(
        sources.len() >= 5,
        "only {} source files were read",
        sources.len()
    );
}

#[test]
fn no_source_file_in_the_diff_engine_calls_an_ir_handle_accessor() {
    // The discipline this crate is the first consumer to need. `EssIr::event(&handle)` is total and
    // panics rather than returning `None` when the handle came from the other compilation — "a
    // handle belongs to the IR that minted it" — and a diff engine holds two IRs by definition, so
    // `after.event(&before_binding.event)` is the natural line to write and the one that turns a
    // review tool into a crash report.
    //
    // Every accessor `crates/ess-compiler/src/ir.rs` generates, spelt as the call would be. The list
    // is read from that macro's `handles!` block; an accessor added there and not here is the one
    // hole in this scan, which is why the message says where to look.
    const ACCESSORS: [&str; 9] = [
        ".named_type(",
        ".entity(",
        ".command(",
        ".event(",
        ".error(",
        ".view(",
        ".actor(",
        ".domain(",
        ".component(",
    ];

    for (path, text) in &sources() {
        for accessor in ACCESSORS {
            assert!(
                !text.contains(accessor),
                "{path} calls `{accessor}` — an `EssIr` handle accessor. Resolve through \
                 `handle.name()` and the target IR's own map instead: a handle from one compilation \
                 used against another panics. (The list of accessors is the `handles!` block in \
                 `crates/ess-compiler/src/ir.rs`.)"
            );
        }
    }
}

#[test]
fn a_system_still_has_no_naming_a_document_can_set() {
    // Why `SystemChange` has no wire-name or display-name variant. `SystemSpec` carries a `Naming`
    // and the only reader that populates one is `#[cfg(test)]` in `ess-domain`, so every
    // specification an author can write resolves to `Naming::default()`. This asserts the gap is
    // still there: when the model gains a way to set it, this fails and points at the three change
    // kinds that then have to exist.
    for revision in ["before", "after"] {
        let ir = compiled(revision);
        assert!(
            ir.naming.is_empty(),
            "`{revision}` resolved a system naming ({:?}), so `SystemChange` now needs \
             wire-name, display-name and naming-summary variants and `system_changes` has to \
             compare them",
            ir.naming
        );
    }
}

#[test]
fn a_binding_still_has_one_delivery_a_document_can_write() {
    // Why `BindingChange` has no `delivery-changed` kind. `Delivery` has one inhabitant —
    // `at_least_once` is the only guarantee this build implements — so a change kind for it would
    // be a refusal that cannot fire, the defect class
    // `docs/reviews/2026-08-20-guard-efficacy-review.md` was written about. The match below is
    // exhaustive without a wildcard: when the model gains a second delivery word, this stops
    // compiling and points at the change kind that then has to exist and the comparison
    // `compare_bindings` then has to make.
    match ess_domain::binding::Delivery::AtLeastOnce {
        ess_domain::binding::Delivery::AtLeastOnce => {}
    }
}

// ---- the word every change is written with ----------------------------------------------------

/// Builds a qualified name for a fixture.
fn name(value: &str) -> QualifiedName {
    QualifiedName::new(value).expect("a valid qualified name")
}

/// A pair of strings, for the many `before`/`after` variants.
fn pair() -> (String, String) {
    ("was".to_owned(), "is".to_owned())
}

/// One of every `SystemChange`.
fn system_changes() -> Vec<SystemChange> {
    vec![
        SystemChange::VersionChanged {
            before: Version::V1,
            after: Version::new(2).expect("v2"),
        },
        SystemChange::SummaryChanged {
            before: None,
            after: Some("is".to_owned()),
        },
    ]
}

/// One of every `TypeChange`.
fn type_changes() -> Vec<TypeChange> {
    let (was, is) = pair();
    vec![
        TypeChange::Added,
        TypeChange::Removed,
        TypeChange::KindChanged {
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::RepresentationChanged {
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::FieldAdded {
            field: "f".to_owned(),
            type_ref: is.clone(),
        },
        TypeChange::FieldRemoved {
            field: "f".to_owned(),
        },
        TypeChange::FieldTypeChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::FieldWireNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::FieldDisplayNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::FieldSummaryChanged {
            field: "f".to_owned(),
            before: None,
            after: None,
        },
        TypeChange::FieldOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        TypeChange::VariantAdded {
            variant: "V".to_owned(),
        },
        TypeChange::VariantRemoved {
            variant: "V".to_owned(),
        },
        TypeChange::VariantOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        TypeChange::VariantTypeChanged {
            variant: "V".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::UnionTagChanged {
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::InvariantsChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        TypeChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        TypeChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        TypeChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `EventChange`.
fn event_changes() -> Vec<EventChange> {
    let (was, is) = pair();
    vec![
        EventChange::Added,
        EventChange::Removed,
        EventChange::DomainChanged {
            before: DomainRef::new(name("a.b")),
            after: DomainRef::new(name("a.c")),
        },
        EventChange::FieldAdded {
            field: "f".to_owned(),
            type_ref: is.clone(),
        },
        EventChange::FieldRemoved {
            field: "f".to_owned(),
        },
        EventChange::FieldTypeChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EventChange::FieldWireNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EventChange::FieldDisplayNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EventChange::FieldSummaryChanged {
            field: "f".to_owned(),
            before: None,
            after: None,
        },
        EventChange::FieldOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        EventChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        EventChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        EventChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `ErrorChange`.
fn error_changes() -> Vec<ErrorChange> {
    let (was, is) = pair();
    vec![
        ErrorChange::Added,
        ErrorChange::Removed,
        ErrorChange::DomainChanged {
            before: DomainRef::new(name("a.b")),
            after: DomainRef::new(name("a.c")),
        },
        ErrorChange::FieldAdded {
            field: "f".to_owned(),
            type_ref: is.clone(),
        },
        ErrorChange::FieldRemoved {
            field: "f".to_owned(),
        },
        ErrorChange::FieldTypeChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        ErrorChange::FieldWireNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        ErrorChange::FieldDisplayNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        ErrorChange::FieldSummaryChanged {
            field: "f".to_owned(),
            before: None,
            after: None,
        },
        ErrorChange::FieldOrderChanged {
            before: vec![was],
            after: vec![is],
        },
        ErrorChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `ActorChange`.
fn actor_changes() -> Vec<ActorChange> {
    let (was, is) = pair();
    vec![
        ActorChange::Added,
        ActorChange::Removed,
        ActorChange::DomainChanged {
            before: DomainRef::new(name("a.b")),
            after: DomainRef::new(name("a.c")),
        },
        ActorChange::GrantAdded {
            command: CommandRef::new(name("a.b.C")),
        },
        ActorChange::GrantRemoved {
            command: CommandRef::new(name("a.b.C")),
        },
        ActorChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        ActorChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        ActorChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `ComponentChange`.
fn component_changes() -> Vec<ComponentChange> {
    let (was, is) = pair();
    vec![
        ComponentChange::Added,
        ComponentChange::Removed,
        ComponentChange::OwnsAdded {
            domain: DomainRef::new(name("a.b")),
        },
        ComponentChange::OwnsRemoved {
            domain: DomainRef::new(name("a.b")),
        },
        ComponentChange::AcceptsAdded {
            command: CommandRef::new(name("a.b.C")),
        },
        ComponentChange::AcceptsRemoved {
            command: CommandRef::new(name("a.b.C")),
        },
        ComponentChange::PublishesAdded {
            event: EventRef::new(name("a.b.E")),
        },
        ComponentChange::PublishesRemoved {
            event: EventRef::new(name("a.b.E")),
        },
        ComponentChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        ComponentChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        ComponentChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `EntityChange`.
fn entity_changes() -> Vec<EntityChange> {
    let (was, is) = pair();
    vec![
        EntityChange::Added,
        EntityChange::Removed,
        EntityChange::DomainChanged {
            before: DomainRef::new(name("a.b")),
            after: DomainRef::new(name("a.c")),
        },
        EntityChange::IdentityRenamed {
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::IdentityTypeChanged {
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::IdentityWireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::IdentityDisplayNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::IdentitySummaryChanged {
            before: None,
            after: None,
        },
        EntityChange::FieldAdded {
            field: "f".to_owned(),
            type_ref: is.clone(),
        },
        EntityChange::FieldRemoved {
            field: "f".to_owned(),
        },
        EntityChange::FieldTypeChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::FieldWireNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::FieldDisplayNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::FieldSummaryChanged {
            field: "f".to_owned(),
            before: None,
            after: None,
        },
        EntityChange::FieldOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        EntityChange::InitialStateChanged {
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::TerminalAdded {
            state: "S".to_owned(),
        },
        EntityChange::TerminalRemoved {
            state: "S".to_owned(),
        },
        EntityChange::TransitionAdded {
            transition: "t".to_owned(),
        },
        EntityChange::TransitionRemoved {
            transition: "t".to_owned(),
        },
        EntityChange::TransitionRouteChanged {
            transition: "t".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::InvariantsChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        EntityChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        EntityChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        EntityChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `CommandChange`.
fn command_changes() -> Vec<CommandChange> {
    let (was, is) = pair();
    vec![
        CommandChange::Added,
        CommandChange::Removed,
        CommandChange::DomainChanged {
            before: DomainRef::new(name("a.b")),
            after: DomainRef::new(name("a.c")),
        },
        CommandChange::InputAdded {
            field: "f".to_owned(),
            type_ref: is.clone(),
        },
        CommandChange::InputRemoved {
            field: "f".to_owned(),
        },
        CommandChange::InputTypeChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        CommandChange::InputWireNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        CommandChange::InputDisplayNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        CommandChange::InputSummaryChanged {
            field: "f".to_owned(),
            before: None,
            after: None,
        },
        CommandChange::InputOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        CommandChange::OutcomeAdded {
            outcome: "o".to_owned(),
        },
        CommandChange::OutcomeRemoved {
            outcome: "o".to_owned(),
        },
        CommandChange::OutcomeConditionChanged {
            outcome: "o".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        CommandChange::OutcomeSubjectChanged {
            outcome: "o".to_owned(),
            before: None,
            after: Some(is.clone()),
        },
        CommandChange::OutcomeEmitsChanged {
            outcome: "o".to_owned(),
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        CommandChange::OutcomePayloadChanged {
            outcome: "o".to_owned(),
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        CommandChange::OutcomeErrorChanged {
            outcome: "o".to_owned(),
            before: None,
            after: Some(is.clone()),
        },
        CommandChange::OutcomeSummaryChanged {
            outcome: "o".to_owned(),
            before: None,
            after: None,
        },
        CommandChange::OutcomeOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        CommandChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        CommandChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        CommandChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `ViewChange`.
fn view_changes() -> Vec<ViewChange> {
    let (was, is) = pair();
    vec![
        ViewChange::Added,
        ViewChange::Removed,
        ViewChange::DomainChanged {
            before: DomainRef::new(name("a.b")),
            after: DomainRef::new(name("a.c")),
        },
        ViewChange::SourceChanged {
            before: EntityRef::new(name("a.b.E")),
            after: EntityRef::new(name("a.b.F")),
        },
        ViewChange::FieldAdded {
            field: "f".to_owned(),
            type_ref: is.clone(),
        },
        ViewChange::FieldRemoved {
            field: "f".to_owned(),
        },
        ViewChange::FieldTypeChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        ViewChange::FieldWireNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        ViewChange::FieldDisplayNameChanged {
            field: "f".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        ViewChange::FieldSummaryChanged {
            field: "f".to_owned(),
            before: None,
            after: None,
        },
        ViewChange::FieldOrderChanged {
            before: vec![was.clone()],
            after: vec![is.clone()],
        },
        ViewChange::FilterChanged {
            before: None,
            after: Some(is.clone()),
        },
        ViewChange::ConsistencyChanged {
            before: was.clone(),
            after: is.clone(),
        },
        ViewChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        ViewChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        ViewChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One of every `BindingChange`.
fn binding_changes() -> Vec<BindingChange> {
    let (was, is) = pair();
    vec![
        BindingChange::Added,
        BindingChange::Removed,
        BindingChange::EventChanged {
            before: EventRef::new(name("a.b.E")),
            after: EventRef::new(name("a.b.F")),
        },
        BindingChange::CommandChanged {
            before: CommandRef::new(name("a.b.C")),
            after: CommandRef::new(name("a.b.D")),
        },
        BindingChange::MappingAdded {
            target: "t".to_owned(),
            value: is.clone(),
        },
        BindingChange::MappingRemoved {
            target: "t".to_owned(),
        },
        BindingChange::MappingValueChanged {
            target: "t".to_owned(),
            before: was.clone(),
            after: is.clone(),
        },
        BindingChange::FailureChanged {
            before: was.clone(),
            after: is.clone(),
        },
        BindingChange::WireNameChanged {
            before: was.clone(),
            after: is.clone(),
        },
        BindingChange::DisplayNameChanged {
            before: was,
            after: is,
        },
        BindingChange::SummaryChanged {
            before: None,
            after: None,
        },
    ]
}

/// One `SemanticChange` per variant of every family, with a subject each.
fn one_of_every_change() -> Vec<SemanticChange> {
    let mut all = Vec::new();
    all.extend(
        system_changes()
            .into_iter()
            .map(|changed| SemanticChange::System {
                subject: name("witness"),
                changed,
            }),
    );
    all.extend(
        type_changes()
            .into_iter()
            .map(|changed| SemanticChange::Type {
                subject: DeclaredTypeRef::new(name("witness.a.T")),
                changed,
            }),
    );
    all.extend(
        entity_changes()
            .into_iter()
            .map(|changed| SemanticChange::Entity {
                subject: EntityRef::new(name("witness.a.N")),
                changed,
            }),
    );
    all.extend(
        command_changes()
            .into_iter()
            .map(|changed| SemanticChange::Command {
                subject: CommandRef::new(name("witness.a.C")),
                changed,
            }),
    );
    all.extend(
        event_changes()
            .into_iter()
            .map(|changed| SemanticChange::Event {
                subject: EventRef::new(name("witness.a.E")),
                changed,
            }),
    );
    all.extend(
        error_changes()
            .into_iter()
            .map(|changed| SemanticChange::Error {
                subject: ErrorRef::new(name("witness.a.X")),
                changed,
            }),
    );
    all.extend(
        actor_changes()
            .into_iter()
            .map(|changed| SemanticChange::Actor {
                subject: ActorRef::new(name("witness.a.A")),
                changed,
            }),
    );
    all.extend(
        view_changes()
            .into_iter()
            .map(|changed| SemanticChange::View {
                subject: ViewRef::new(name("witness.a.V")),
                changed,
            }),
    );
    all.extend(
        component_changes()
            .into_iter()
            .map(|changed| SemanticChange::Component {
                subject: ComponentRef::new(
                    ess_domain::component::ComponentName::new("a-service")
                        .expect("a component name"),
                ),
                changed,
            }),
    );
    all.extend(
        binding_changes()
            .into_iter()
            .map(|changed| SemanticChange::Binding {
                subject: BindingRef::new(
                    ess_domain::binding::BindingName::new("a-binding").expect("a binding name"),
                ),
                changed,
            }),
    );
    all
}

#[test]
fn a_change_is_spelt_the_same_way_in_its_id_and_in_the_document() {
    // Two spellings of one word is the failure this repository keeps arriving at, and here it would
    // be silent: an id says `variant-added` and the document says `variant_added`, so grepping the
    // artifact for an id quoted in a review finds nothing. The subtype word is hand-written in
    // `kind()` and generated by serde's `rename_all`, and this is the only thing keeping the two in
    // step.
    let all = one_of_every_change();
    assert!(all.len() >= 139, "only {} variants were built", all.len());

    for change in &all {
        let value = serde_json::to_value(change).expect("a change serialises");
        let written = value["changed"]["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("{change:?} writes a `kind`"));
        assert_eq!(
            written,
            change.kind(),
            "`{written}` in the document and `{}` in the id",
            change.kind()
        );
        assert!(
            change.id().to_string().contains(change.kind()),
            "the id `{}` does not carry the subtype `{}`",
            change.id(),
            change.kind()
        );

        let category = value["category"]
            .as_str()
            .expect("a change writes a category");
        assert_eq!(
            category,
            change.category().written(),
            "the category word in the document and in the id disagree"
        );
    }
}

#[test]
fn every_change_variant_has_something_to_say_for_itself() {
    // A `describe` arm that returned an empty string would render as a blank line in the report,
    // which is a change a reader cannot see. Cheap to assert while every variant is in hand.
    for change in one_of_every_change() {
        assert!(
            !change.describe().trim().is_empty(),
            "{change:?} renders as nothing"
        );
    }
}

// ---- reading a delta back --------------------------------------------------------------------

/// The fixture pair's delta as a mutable JSON value.
fn document() -> serde_json::Value {
    serde_json::from_str(&delta().to_canonical_json()).expect("the document parses")
}

/// Reads a document back, expecting it to be refused.
fn refused(document: &serde_json::Value) -> aep_domain::error::ValidationErrors {
    let raw: RawEssDelta =
        serde_json::from_value(document.clone()).expect("the document still parses");
    EssDelta::try_from(raw).expect_err("the document is refused")
}

#[test]
fn a_delta_whose_id_was_edited_is_refused() {
    let mut document = document();
    document["changes"][0]["id"] = serde_json::json!("type/catalog.pricing.Currency/added");

    let errors = refused(&document);

    assert!(errors.contains(ValidationCode::ConflictingDeclaration));
    assert_eq!(errors.as_slice().len(), 1, "one edit, one error: {errors}");
    assert!(
        errors.as_slice()[0].location == "delta.changes[0].id",
        "the error names the change that was edited: {errors}"
    );
}

#[test]
fn a_delta_whose_relation_was_edited_is_refused() {
    // The check that makes it safe to write a derived fact into the document at all. A grant removed
    // is a narrowing; a build that wrote `expanded` beside it — or a person who edited it — would
    // otherwise hand the next reader a classification nothing produced.
    let mut document = document();
    let position = document["changes"]
        .as_array()
        .expect("a list of changes")
        .iter()
        .position(|change| change["relation"] == "narrowed")
        .expect("the fixture holds a narrowing");
    document["changes"][position]["relation"] = serde_json::json!("expanded");

    let errors = refused(&document);

    assert!(errors.contains(ValidationCode::ConflictingDeclaration));
    assert!(
        errors.to_string().contains("narrowed"),
        "the refusal says what the content derives: {errors}"
    );
}

#[test]
fn a_delta_written_in_a_format_this_build_does_not_read_is_refused() {
    let mut document = document();
    document["format"] = serde_json::json!("ess-diff/2");

    let errors = refused(&document);

    assert!(errors.contains(ValidationCode::UnsupportedFormatVersion));
}

#[test]
fn a_delta_whose_changes_are_out_of_order_is_refused() {
    // Canonical order is a format contract, so a document that is not in it is not a document this
    // format defines — and repairing it silently on the way in would make the contract untestable.
    // The fixture is reversed rather than shuffled, so the first pair compared is already wrong.
    let mut document = document();
    let changes = document["changes"].as_array_mut().expect("a list");
    changes.reverse();

    let errors = refused(&document);

    assert!(errors.contains(ValidationCode::ConflictingDeclaration));
    assert!(
        errors.to_string().contains("order"),
        "the refusal says it is about ordering: {errors}"
    );
}

#[test]
fn a_delta_naming_two_systems_is_refused_on_the_way_in_as_well() {
    // The same rule `diff` refuses on, at the other door. A document is read by a later process that
    // never saw the comparison, so the rule has to hold there too — and one rule with two doors is
    // better than a rule that only guards the door it was written at.
    let mut document = document();
    document["after"]["system"] = serde_json::json!("ordering");

    let errors = refused(&document);

    assert!(errors.contains(ValidationCode::ConflictingDeclaration));
    assert!(
        errors.to_string().contains("ordering"),
        "the refusal names the system that does not belong: {errors}"
    );
}

#[test]
fn a_document_with_six_defects_reports_six() {
    // Invariant 3, at this document's own conversion: a delta with six doctored ids reports six
    // errors, not the first one. The fixture reaches the state the rule is load-bearing in — every
    // one of the six changes is edited — before it asserts the count.
    let mut document = document();
    let total = document["changes"].as_array().expect("a list").len();
    assert_eq!(total, 6, "the fixture pair holds six changes");
    for index in 0..total {
        document["changes"][index]["id"] = serde_json::json!("system/catalog/version-changed");
    }

    let errors = refused(&document);

    assert_eq!(
        errors
            .as_slice()
            .iter()
            // The last dotted segment, not `ends_with(".id")`: the location is a document path
            // like `delta.changes[0].id`, and asking for its final segment says that, where a
            // suffix match reads as though `.id` were a file extension.
            .filter(|error| error.location.rsplit('.').next() == Some("id"))
            .count(),
        6,
        "six edits, six errors: {errors}"
    );
}

#[test]
fn a_delta_this_build_wrote_is_read_back_without_complaint() {
    // The inverse the scans above need to stay honest. Every test in this section doctors a document
    // and expects a refusal; if the conversion started refusing everything, they would all still
    // pass. This is the one that fails when it does.
    let raw: RawEssDelta = serde_json::from_value(document()).expect("parses");

    EssDelta::try_from(raw).expect("an unedited document validates");
}
