//! The normative example, compiled from the files it actually lives in.
//!
//! Design §38's success criterion for this wave is that the billing specification resolves. This is
//! that criterion, over `examples/billing/` rather than over a copy inlined here: the design
//! document's own snippets drifted three ways before anyone noticed (review F7), and a copy would
//! drift the same way.
//!
//! It is also where review F8 stops being an assertion. Determinism is claimed by three rules —
//! `BTreeMap` only, no clock, no RNG — and none of them is checkable by reading. What is checkable
//! is that two independent compilations of the same source produce the same bytes, so that is a test.

use std::path::{Path, PathBuf};

use ess_compiler::ir::{EssIr, ResolvedMappingValue};
use ess_compiler::resolve::{codes, compile, compile_locating, diagnose_locating};
use ess_compiler::source::SourceMap;
use ess_domain::binding::BindingName;
use ess_domain::name::QualifiedName;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

/// The example directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
///
/// Discovered rather than listed: a file added to the example would otherwise be compiled by the CLI
/// and silently ignored by the test meant to keep the example honest.
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
    assert!(!found.is_empty(), "the billing example holds no files");
    found.sort();
    found
}

/// The example, assembled, with the text every diagnostic could point into.
fn read_example() -> (Specification, SourceMap, Vec<String>) {
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
    (specification, sources, labels)
}

/// The example, compiled.
fn compiled() -> EssIr {
    let (specification, sources, labels) = read_example();
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

fn name(value: &str) -> QualifiedName {
    QualifiedName::new(value).expect("a valid qualified name")
}

#[test]
fn the_billing_specification_resolves() {
    let ir = compiled();

    assert_eq!(ir.system.to_string(), "billing");
    assert_eq!(ir.version.to_string(), "v3");
    assert_eq!(ir.domains.len(), 2, "two bounded contexts");
    assert!(
        ir.summary.is_some(),
        "a docs projection needs the paragraph"
    );
    assert_eq!(ir.components.len(), 2);
    assert_eq!(ir.workloads.len(), 2);
    assert_eq!(ir.bindings.len(), 1);
    assert_eq!(ir.conversions.len(), 1);
}

#[test]
fn every_handle_in_the_ir_names_something_the_ir_holds() {
    // The wave's whole claim, exercised: every lookup below is total. If any handle named something
    // absent, this test would panic inside the accessor rather than return `None` for someone to
    // ignore.
    let ir = compiled();

    for declared in ir.types.values() {
        assert_eq!(
            ir.named_type(handle_of(&ir, &declared.name)).name,
            declared.name
        );
    }
    for command in ir.commands.values() {
        assert!(!ir.domain(&command.domain).name.to_string().is_empty());
        for field in &command.input {
            for leaf in field.type_ref.named_leaves() {
                let _ = ir.named_type(leaf);
            }
        }
        for event in command.emits() {
            let _ = ir.event(event);
        }
        for error in command.errors() {
            let _ = ir.error(error);
        }
    }
    for event in ir.events.values() {
        let _ = ir.domain(&event.domain);
        for field in &event.fields {
            for leaf in field.type_ref.named_leaves() {
                let _ = ir.named_type(leaf);
            }
        }
    }
    for error in ir.errors.values() {
        let _ = ir.domain(&error.domain);
    }
    for binding in ir.bindings.values() {
        let _ = ir.event(&binding.event);
        let _ = ir.command(&binding.command);
    }
    for component in ir.components.values() {
        for domain in &component.owns {
            let _ = ir.domain(domain);
        }
        for command in &component.accepts {
            let _ = ir.command(command);
        }
        for event in &component.publishes {
            let _ = ir.event(event);
        }
    }
    for workload in ir.workloads.values() {
        let _ = ir.component(&workload.component);
    }
    for domain in ir.domains.values() {
        for declared in &domain.types {
            let _ = ir.named_type(declared);
        }
        for command in &domain.commands {
            let _ = ir.command(command);
        }
    }
}

/// A type handle for a name the IR declares, taken from the IR rather than constructed — which is
/// the only way to obtain one.
fn handle_of<'a>(ir: &'a EssIr, wanted: &QualifiedName) -> &'a ess_compiler::ir::TypeHandle {
    ir.domains
        .values()
        .flat_map(|domain| domain.types.iter())
        .find(|handle| handle.name() == wanted)
        .unwrap_or_else(|| panic!("`{wanted}` is listed by the domain that declares it"))
}

#[test]
fn a_field_keeps_the_shape_of_its_type_rather_than_a_rendering_of_it() {
    let ir = compiled();
    let invoice_lines = ir
        .named_type(handle_of(&ir, &name("billing.invoice.LineItem")))
        .name
        .clone();
    assert_eq!(invoice_lines, name("billing.invoice.LineItem"));

    // `List<billing.invoice.LineItem>` on the entity is not in this wave's IR, but the same question
    // is asked of the union: is this variant a named type, and which one?
    let payee = ir.named_type(handle_of(&ir, &name("billing.invoice.Payee")));
    let ess_compiler::ir::ResolvedBody::Union { variants, tag } = &payee.body else {
        panic!("the example declares Payee as a union");
    };
    assert_eq!(tag, "kind");
    let person = variants.get("person").expect("a person variant");
    assert_eq!(
        person.declared().expect("a named type").name(),
        &name("billing.invoice.Email"),
        "a projection asks this without re-parsing a string"
    );
}

#[test]
fn the_crossing_between_two_contexts_is_recorded_with_the_reason_someone_gave_for_it() {
    let ir = compiled();
    let binding = &ir.bindings[&BindingName::new("notify-on-invoice-created").expect("a name")];

    assert_eq!(
        binding.mapping.len(),
        2,
        "one entry per input of `SendEmail`, in its order"
    );
    let recipient = &binding.mapping[0];
    assert_eq!(recipient.target, "recipient");
    let ResolvedMappingValue::EventField { field, .. } = &recipient.value else {
        panic!("`recipient` is filled from an event field");
    };
    assert_eq!(field, "customer_email");
    assert!(
        recipient
            .conversion
            .as_deref()
            .is_some_and(|because| because.contains("deliverable address")),
        "the declared crossing's reason travels into the IR: {:?}",
        recipient.conversion
    );

    let template = &binding.mapping[1];
    assert!(
        matches!(template.value, ResolvedMappingValue::Literal { .. }),
        "a literal is a distinct variant, so a reader can see what was not typechecked"
    );
}

#[test]
fn the_reaction_graph_names_the_binding_that_causes_each_command() {
    let ir = compiled();
    let reactions = ir.reactions();

    let created = reactions
        .keys()
        .find(|event| event.name() == &name("billing.invoice.InvoiceCreated"))
        .expect("the example binds InvoiceCreated");
    let bindings = &reactions[created];
    assert_eq!(bindings.len(), 1);
    assert_eq!(
        ir.command(&bindings[0].command).name,
        name("billing.email.SendEmail")
    );
}

// ---- review F8: determinism, mechanised ----------------------------------------------------

#[test]
fn compiling_the_billing_example_twice_produces_byte_identical_json() {
    // Two independent reads, assemblies and compilations. Nothing is shared between them, so an
    // unordered map, a clock or an address-dependent iteration order anywhere in the pass would show
    // up here as a diff rather than as a rumour.
    let first = compiled().to_canonical_json();
    let second = compiled().to_canonical_json();

    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "the same source compiled twice must be the same bytes"
    );
    assert!(
        first.len() > 1000,
        "the IR is not empty: {} bytes",
        first.len()
    );
}

#[test]
fn no_source_file_in_the_compiler_reads_a_clock_or_an_unordered_map() {
    // Two of F8's three failure modes are visible in the source, and a test that compiles the same
    // input twice inside one process cannot see either of them: a `HashMap` iterates the same way
    // twice in a row, and a timestamp only differs across runs. So they are read for, not trusted.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in std::fs::read_dir(&source).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|it| it == "rs") {
            let text = std::fs::read_to_string(&path).expect("readable");
            for banned in ["HashMap", "HashSet", "SystemTime", "Instant", "rand::"] {
                assert!(
                    !text.contains(banned),
                    "{} uses {banned}, which makes the IR depend on when and where it was built",
                    path.display()
                );
            }
            checked += 1;
        }
    }
    assert!(checked >= 4, "only {checked} source files were read");
}

#[test]
fn canonical_json_ends_in_a_newline() {
    let json = compiled().to_canonical_json();

    assert!(
        json.ends_with('\n'),
        "a generated file without a trailing newline is a file that shows up modified in the next \
         diff"
    );
    assert!(!json.ends_with("\n\n"), "one newline, not two");
}

#[test]
fn the_json_orders_its_keys_the_way_a_btreemap_does() {
    let json = compiled().to_canonical_json();
    let email = json
        .find("billing.email.EmailSent")
        .expect("the email context is in the output");
    let invoice = json
        .find("billing.invoice.InvoiceCreated")
        .expect("the invoice context is in the output");

    assert!(
        email < invoice,
        "sorted by name, so the order does not depend on which file was read first"
    );
}

// ---- the whole pipeline, end to end --------------------------------------------------------

#[test]
fn a_refusal_from_the_whole_pipeline_carries_a_code_and_the_line_it_belongs_on() {
    // What `protocol ess compile` does with a broken document. Two structs that require each other
    // describe a value nobody can build; `ess-domain` owns that rule and refuses it during assembly,
    // where the verdict is a `ValidationError` with a document path and prose. The bridge is what
    // turns it into `ESS-TYPE-008` at `types.yaml:6` — the two things a `ValidationError` lacks.
    let text = "\
format: ess/1
system: shop
version: v1
domain: shop.orders
types:
  - name: shop.orders.Left
    kind: struct
    fields:
      - name: right
        type: shop.orders.Right
  - name: shop.orders.Right
    kind: struct
    fields:
      - name: left
        type: shop.orders.Left
";
    let raw = RawSpecFile::parse(text).expect("well formed");
    let errors = Specification::assemble([(Source::new("types.yaml"), raw)])
        .expect_err("no value of either type can exist");
    let mut sources = SourceMap::new();
    sources.insert("types.yaml", text);

    let diagnostics = diagnose_locating(&errors, &sources, &["types.yaml"]);

    assert!(
        diagnostics.contains(codes::UNINHABITABLE_TYPE),
        "{diagnostics}"
    );
    let refused = diagnostics
        .as_slice()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::UNINHABITABLE_TYPE)
        .expect("the refusal");
    let span = refused.span.as_ref().expect("a span");
    assert_eq!(span.source, "types.yaml");
    assert_eq!(
        span.located.expect("the declaration was found").line,
        6,
        "the line `shop.orders.Left` is declared on: {span}"
    );
    assert!(refused.to_string().contains("ESS-TYPE-008"), "{refused}");
    assert!(
        refused.hint.is_some(),
        "the domain crate's repair advice survives the crossing"
    );
}

#[test]
fn compiling_without_the_file_list_still_reports_the_document_path() {
    let (specification, sources, _) = read_example();
    // `compile` has no labels to search, so no diagnostic can carry a line — and the example has no
    // diagnostics anyway. What is being asserted is that the entry point the CLI uses agrees with
    // the located one about the IR itself.
    let plain = compile(&specification, &sources).expect("the example resolves");
    let located = compiled();

    assert_eq!(
        plain.to_canonical_json(),
        located.to_canonical_json(),
        "line numbers are a property of diagnostics, not of the IR"
    );
}
