//! The documentation projection, over `examples/billing/` rather than over a fixture.
//!
//! Wave 3's claim is that documentation is the cheapest check on model completeness: a construct
//! with no rendering shows up as a hole in a page a person reads. That claim is worth nothing unless
//! something asserts the hole is not there — so every construct the normative example declares gets
//! an assertion here, by name, and a projection that silently drops unions fails rather than looking
//! finished.
//!
//! Nothing here asserts a gap any more. Entities, views and actors used to be absent from the IR
//! and present on the page only as a declared hole; they are now rendered, so the assertions are the
//! ordinary kind — the specific fact a reader would lose, per construct. What is left of the gap
//! mechanism is asserted the other way round: an empty allowlist must print no "cannot show"
//! section anywhere, so the day it has an entry again is the day a section appears.
//!
//! Two fixtures are compiled inline, for corners `examples/billing/` does not have — a type nothing
//! reaches, and a grant that crosses two contexts. The example is normative and stays that way: a
//! corner added to it to satisfy a test is a corner every future reader has to explain away.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_gen::artifact::{run, Artifact};
use ess_gen::docs::Docs;

// ---- the example ------------------------------------------------------------------------------

/// Every `.yaml` file in the billing example, relative to it, in a stable order.
///
/// Discovered rather than listed, for the reason `ess-compiler`'s own test gives: a file added to
/// the example would otherwise be compiled by the CLI and ignored by the test meant to keep the
/// example honest.
fn files(base: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![base.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(path);
            }
        }
    }
    assert!(!found.is_empty(), "the billing example holds no files");
    found.sort();
    found
}

/// The billing example, compiled.
fn billing() -> EssIr {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists");
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for path in files(&base) {
        let label = path
            .strip_prefix(&base)
            .expect("inside the example")
            .display()
            .to_string();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

/// The documentation for the billing example, keyed by the path it lands on.
fn docs() -> BTreeMap<String, Artifact> {
    pages(&billing())
}

/// The documentation for any IR, keyed by the path it lands on.
fn pages(ir: &EssIr) -> BTreeMap<String, Artifact> {
    run(&Docs, ir).expect("no two pages claim one path")
}

// ---- fixtures for the corners the example does not have --------------------------------------

/// A specification written inline, compiled — one entry per file, because a file carries one domain.
fn compiled(files: &[(&str, &str)]) -> EssIr {
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for (label, text) in files {
        let raw = RawSpecFile::parse(text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert((*label).to_owned(), (*text).to_owned());
        parsed.push((Source::new((*label).to_owned()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the fixture validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("the fixture resolves:\n{diagnostics}"))
}

/// A context with a type nothing in the system reaches — not even through an entity or a view.
///
/// `Forgotten` is the case the billing example no longer has: every type it declares is reached
/// from an entity's field, a view's, a message's or a crossing, now that those are in the IR.
const AN_UNREACHED_TYPE: &str = r"
format: ess/1
system: warehouse
version: v1
domain: warehouse.core

types:
  - name: warehouse.core.CrateId
    kind: newtype
    of: Uuid
  - name: warehouse.core.Forgotten
    kind: newtype
    of: String

entities:
  - name: warehouse.core.Crate
    identity:
      name: crate_id
      type: warehouse.core.CrateId
    lifecycle:
      initial: Stored
      states: [Stored]
      terminal: [Stored]
";

/// Two contexts, where the actor in one may invoke a command in the other.
///
/// The billing example grants only within a context, so nothing there exercises the link that has to
/// leave the page it is written on.
const A_GRANT_ACROSS_CONTEXTS: &[(&str, &str)] = &[
    (
        "shipping.yaml",
        r"
format: ess/1
system: depot
version: v1
domain: depot.shipping

commands:
  - name: depot.shipping.Dispatch
    outcomes:
      - name: dispatched
        emits:
          - depot.shipping.Dispatched

events:
  - name: depot.shipping.Dispatched
",
    ),
    (
        "orders.yaml",
        r"
domain: depot.orders

actors:
  - name: depot.orders.Dispatcher
    may:
      - depot.shipping.Dispatch
",
    ),
];

/// One page, or a failure naming the pages there are.
fn page(docs: &BTreeMap<String, Artifact>, path: &str) -> String {
    docs.get(path).map_or_else(
        || {
            panic!(
                "no page at `{path}`; there are {:?}",
                docs.keys().collect::<Vec<_>>()
            )
        },
        |artifact| artifact.contents.clone(),
    )
}

/// Every page's text, concatenated, for "does this construct appear anywhere" questions.
fn everything(docs: &BTreeMap<String, Artifact>) -> String {
    docs.values()
        .map(|artifact| artifact.contents.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Asserts a phrase is somewhere in a page, and says which page when it is not.
fn assert_says(page: &str, wanted: &str, why: &str) {
    assert!(
        page.contains(wanted),
        "the documentation does not say {wanted:?}, and {why}"
    );
}

// ---- the properties every artifact has -------------------------------------------------------

#[test]
fn generating_the_documentation_twice_produces_byte_identical_output() {
    // The whole determinism claim, in the only form worth having it: `BTreeMap` only, no clock and
    // no RNG are three rules nobody can check by reading.
    let ir = billing();

    let first = run(&Docs, &ir).expect("paths are unique");
    let second = run(&Docs, &ir).expect("paths are unique");

    assert_eq!(
        first, second,
        "the same IR produced two different doc trees"
    );
}

#[test]
fn every_page_says_which_specification_produced_it() {
    let ir = billing();
    let docs = docs();
    assert!(!docs.is_empty(), "the projection produced nothing");

    for (path, artifact) in &docs {
        let text = &artifact.contents;
        assert!(
            text.starts_with("<!--\n"),
            "`{path}` does not open with a provenance comment"
        );
        assert_says(
            text,
            &format!("generated from {} {}", ir.system, ir.version),
            "an artifact that cannot say which specification produced it is one nobody can audit",
        );
        assert_says(text, "model digest", "two checkouts differ by their digest");
        assert_says(
            text,
            "protocol ess generate",
            "a reader about to hand-edit a generated file has to be told not to",
        );
    }
}

#[test]
fn the_provenance_header_is_a_markdown_comment_a_renderer_can_close() {
    // `Provenance::commented("<!--")` offers exactly that prefix for Markdown, and it is wrong: a
    // per-line prefix cannot close an HTML comment, so four lines each opening one and none closing
    // it leaves a renderer swallowing the page. This asserts the block form this crate uses instead.
    let docs = docs();

    for (path, artifact) in &docs {
        let text = &artifact.contents;
        let end = text.find("-->").unwrap_or_else(|| {
            panic!("the provenance comment in `{path}` is never closed, so the page never renders")
        });
        let header = &text[..end];
        assert_eq!(
            header.matches("<!--").count(),
            1,
            "`{path}` opens a second comment inside the first"
        );
        assert!(
            !header["<!--".len()..].contains("--"),
            "`{path}` closes its provenance comment early: {header:?}"
        );
    }
}

#[test]
fn every_link_between_pages_lands_on_a_page_that_exists_at_the_heading_it_names() {
    // A link that opens the right document at the wrong place fails silently, so both halves are
    // checked: the file, and the anchor.
    let docs = docs();
    let mut checked = 0_usize;

    for (path, artifact) in &docs {
        for link in links(&artifact.contents) {
            let (target, fragment) = split_fragment(&link);
            let resolved = resolve(path, target);
            let destination = page(&docs, &resolved);
            if let Some(fragment) = fragment {
                let anchors = anchors(&destination);
                assert!(
                    anchors.contains(fragment),
                    "`{path}` links to `{resolved}#{fragment}`, which has no such heading; it has \
                     {anchors:?}"
                );
            }
            checked += 1;
        }
    }

    assert!(
        checked > 5,
        "only {checked} links: the pages are not linked"
    );
}

// ---- every construct the IR carries ----------------------------------------------------------

#[test]
fn every_type_kind_reaches_a_page_including_the_tagged_union() {
    let docs = docs();
    let invoice = page(&docs, "docs/domains/billing.invoice.md");

    // A newtype is not its representation, and the page has to say so or the reader learns the
    // opposite from the fact that both are strings.
    assert_says(&invoice, "### `Email`", "a newtype is a declared type");
    assert_says(
        &invoice,
        "wraps `String` and is not interchangeable with one",
        "a newtype rendered as its representation is the projection losing the model's whole point",
    );

    assert_says(&invoice, "### `Money`", "a struct is a declared type");
    assert_says(
        &invoice,
        "- `amount` — `Decimal`",
        "a struct's fields are its shape",
    );
    assert_says(
        &invoice,
        "Every value satisfies `amount >= 0`",
        "an invariant dropped from the docs is an invariant nobody knows to implement",
    );

    assert_says(&invoice, "### `Channel`", "an enum is a declared type");
    assert_says(
        &invoice,
        "is one of `Email`, `Post` and `Portal`",
        "an enum's variants are the whole type",
    );

    // The one this test exists for: a projection that silently drops unions is the bug wave 3 names.
    assert_says(&invoice, "### `Payee`", "a union is a declared type");
    assert_says(
        &invoice,
        "told apart by a `kind` field",
        "an untagged rendering of a tagged union is a decoder picking the wrong branch at runtime",
    );
    assert_says(
        &invoice,
        "- `person` — `billing.invoice.Email`",
        "a variant",
    );
    assert_says(
        &invoice,
        "- `company` — `billing.invoice.CompanyRef`",
        "a variant",
    );

    // Every declared type appears somewhere, not only the four hand-checked above.
    let all = everything(&docs);
    for name in billing().types.keys() {
        assert_says(
            &all,
            &name.to_string(),
            "no type may be missing from the docs",
        );
    }
}

#[test]
fn a_type_nothing_references_is_flagged_rather_than_left_looking_used() {
    let warehouse = pages(&compiled(&[("warehouse.yaml", AN_UNREACHED_TYPE)]));
    let core = page(&warehouse, "docs/domains/warehouse.core.md");

    let note = core
        .lines()
        .find(|line| line.contains("reached by nothing else in this system"))
        .unwrap_or_else(|| {
            panic!(
                "a type nothing reaches should say so, or a reader assumes the model is whole: \
                 {core}"
            )
        });

    assert!(
        note.contains("`warehouse.core.Forgotten`"),
        "an unreached type is named, not counted: {note}"
    );
    assert!(
        !note.contains("CrateId"),
        "`CrateId` is the entity's identity type, so it is reached: {note}"
    );
}

#[test]
fn a_type_reached_only_through_an_entitys_field_is_not_called_unreached() {
    // The regression this protects against is subtle and was real: while entities were absent from
    // the IR, every reference to `Channel`, `LineItem` and `Payee` was invisible, so the page called
    // three types orphans on a page that now draws all three inside `Invoice`. An orphan count that
    // means "reached only through a construct the projection ignores" is worse than no count.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert!(
        !invoice.contains("reached by nothing else in this system"),
        "every type this context declares is reached, now that entities and views are counted: \
         {invoice}"
    );
}

#[test]
fn a_commands_refusal_branch_is_documented_and_not_only_its_name() {
    let docs = docs();
    let invoice = page(&docs, "docs/domains/billing.invoice.md");

    assert_says(&invoice, "### `CreateInvoice`", "a command has a section");
    assert_says(
        &invoice,
        "called `create-invoice` on the wire",
        "a wire name is what a deployed consumer depends on",
    );
    assert_says(
        &invoice,
        "- `amount` — `billing.invoice.Money`",
        "its input",
    );
    assert_says(
        &invoice,
        "It has two outcomes.",
        "outcomes, not an emits list",
    );

    assert_says(&invoice, "**`accepted`**", "the happy branch");
    assert_says(
        &invoice,
        "Taken when `amount.amount > 0` holds of the input",
        "an outcome without its condition is an outcome nobody can test",
    );
    assert_says(
        &invoice,
        "It emits `billing.invoice.InvoiceCreated`",
        "which branch emits which event is the distinction wave 1 restructured the model to keep",
    );

    // The interesting branch. A specification that records only the happy one generates a suite
    // that says nothing about the case where the money does not move.
    assert_says(&invoice, "**`rejected`**", "the refusal branch");
    assert_says(
        &invoice,
        "The default branch, taken when no other outcome's condition matched",
        "a refusal branch's condition",
    );
    assert_says(
        &invoice,
        "It reports `billing.invoice.InvalidAmount`, carrying `submitted`",
        "a refusal that does not say what it carries is a refusal a caller cannot react to",
    );
    assert_says(&invoice, "It emits nothing.", "emitting nothing is a fact");
}

#[test]
fn a_wrong_state_branch_is_documented_with_the_states_the_document_never_lists() {
    // The page has to do the subtraction, because the author is forbidden from doing it: `issue`
    // declares `from: [Draft]` and nothing anywhere writes down that `Issued`, `Paid` and
    // `Cancelled` are therefore states `IssueInvoice` refuses in. A reader who cannot see that set
    // has to hold a lifecycle and a command in their head at once.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(&invoice, "**`wrong-state`**", "the branch has a heading");
    assert_says(
        &invoice,
        "a `billing.invoice.Invoice` in `Cancelled`, `Issued` and `Paid`",
        "the states are derived and printed; the document lists none of them",
    );
    assert_says(
        &invoice,
        "It reports `billing.invoice.InvoiceStateConflict`, carrying `state`",
        "the one thing the author does write, and the reason the branch exists",
    );
    assert_says(
        &invoice,
        "A test reaches it by driving an instance into one of those states",
        "a branch whose page does not say how to reach it is a branch nobody tests",
    );
    assert!(
        !invoice.contains("in `Draft`, `Issued`, `Paid` and `Cancelled`"),
        "the state the move does run from must not be in the set: {invoice}"
    );
}

#[test]
fn an_outcome_that_changes_an_entity_says_which_instance_and_where_the_identity_is_read() {
    // A page that says an invoice moved and not *which* invoice describes a system nobody can call.
    // The two sentences differ because the two surfaces do, and both are on the page: an existing
    // instance is named by the caller, and a new one is announced by the event the branch emits,
    // because it did not exist when the request was made.
    let docs = docs();
    let invoice = page(&docs, "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "The instance is the one named by the input field `invoice_id`.",
        "a `moves:` acts on the instance the caller names",
    );
    assert_says(
        &invoice,
        "The new instance's identity is published as `invoice_id` on \
         `billing.invoice.InvoiceCreated`.",
        "a `creates:` cannot be told which instance, so the page says where its identity appears",
    );
}

#[test]
fn an_outcome_the_input_cannot_decide_says_so_rather_than_claiming_it_is_unreachable() {
    let docs = docs();
    let email = page(&docs, "docs/domains/billing.email.md");

    assert_says(&email, "**`failed`**", "the external branch");
    assert_says(
        &email,
        "Decided outside the input: the provider rejects the recipient address",
        "the declared cause is the only thing that distinguishes this from an unreachable branch",
    );
    assert_says(
        &email,
        "A test reaches it by injecting the declared fault, because no input can.",
        "the test strategy is computed on the model so two projections cannot disagree",
    );
}

#[test]
fn an_events_payload_and_an_errors_payload_are_both_documented_field_by_field() {
    let docs = docs();
    let invoice = page(&docs, "docs/domains/billing.invoice.md");
    let email = page(&docs, "docs/domains/billing.email.md");

    assert_says(&invoice, "### `InvoiceCreated`", "an event has a section");
    assert_says(
        &invoice,
        "- `customer_email` — `billing.invoice.Email`",
        "an event's payload is what a consumer codes against",
    );
    assert_says(
        &invoice,
        "Emitted by `billing.invoice.CreateInvoice` on its `accepted` outcome.",
        "an event whose cause is unstated is an event nobody can trace",
    );

    assert_says(&invoice, "### `InvalidAmount`", "an error has a section");
    assert_says(
        &invoice,
        "- `submitted` — `billing.invoice.Money`",
        "an error carries what a caller needs in order to react, not just a name",
    );

    // And an error with no payload says that, rather than leaving a reader to guess.
    assert_says(&email, "### `Undeliverable`", "an error with no fields");
    assert_says(
        &email,
        "It carries nothing beyond its name",
        "an empty payload is a fact about the error, not an omission in the page",
    );

    let all = everything(&docs);
    let ir = billing();
    for name in ir.events.keys() {
        assert_says(&all, &name.to_string(), "no event may be missing");
    }
    for name in ir.errors.keys() {
        assert_says(&all, &name.to_string(), "no error may be missing");
    }
}

#[test]
fn a_bindings_delivery_and_failure_semantics_are_stated_in_words() {
    let docs = docs();
    let interactions = page(&docs, "docs/interactions.md");

    assert_says(&interactions, "## `notify-on-invoice-created`", "a binding");
    assert_says(
        &interactions,
        "Delivered **at least once**",
        "a delivery guarantee left implicit is the guarantee everyone assumes wrongly",
    );
    assert_says(
        &interactions,
        "must be idempotent",
        "at-least-once without the obligation it puts on the command is half the statement",
    );
    assert_says(
        &interactions,
        "it is **escalated** — surfaced to a person",
        "on_failure is a required word because a binding that fails silently is a demo",
    );
    assert_says(
        &interactions,
        "publishes `billing.email.DeliveryEscalated` to say so",
        "an escalation is a hand-off out of the system, so the event is the only mark it leaves \
         inside it — a page that named none would describe a requirement nobody can prove",
    );

    // The mapping, and the one place two independently written contexts have to agree about a type.
    assert_says(
        &interactions,
        "- `recipient` (`billing.email.EmailAddress`) ← the event's `customer_email` \
         (`billing.invoice.Email`)",
        "a mapping is where a rename breaks silently",
    );
    assert_says(
        &interactions,
        "the compiler took it on trust rather than checking it",
        "a literal is not typechecked, and a reader must be able to see which mappings were",
    );
}

#[test]
fn a_declared_conversion_carries_its_reason_everywhere_a_reader_might_start() {
    let docs = docs();
    let because = "An invoice's customer email is a deliverable address";

    // The audit page: the whole set, whether or not anything uses it.
    let crossings = page(&docs, "docs/crossings.md");
    assert_says(
        &crossings,
        "## `billing.invoice.Email` may be used as `billing.email.EmailAddress`",
        "a crossing is a question about the system, so it has a page",
    );
    assert_says(
        &crossings,
        because,
        "the reason is the point of declaring it",
    );
    assert_says(
        &crossings,
        "Relied on by",
        "an audit needs to know whether the permission is used",
    );

    // Beside the type, on both contexts' pages: this is how someone finds it without knowing to
    // look for it.
    for path in [
        "docs/domains/billing.invoice.md",
        "docs/domains/billing.email.md",
    ] {
        assert_says(
            &page(&docs, path),
            because,
            "a reader reading about `Email` is where the sentence about `Email` has to be",
        );
    }

    // And at the point of use.
    assert_says(
        &page(&docs, "docs/interactions.md"),
        because,
        "the mapping that relies on the crossing has to carry its justification",
    );
}

#[test]
fn a_components_ownership_and_a_workloads_replica_floor_are_both_documented() {
    let docs = docs();
    let readme = page(&docs, "docs/README.md");
    let topology = page(&docs, "docs/topology.md");

    assert_says(&readme, "**`invoice-service`**", "a component");
    assert_says(
        &readme,
        "It owns [`billing.invoice`](domains/billing.invoice.md).",
        "ownership is the only claim a component makes",
    );
    assert_says(
        &readme,
        "It accepts `billing.invoice.CancelInvoice`, `billing.invoice.CreateInvoice`, \
         `billing.invoice.IssueInvoice` and `billing.invoice.PayInvoice`.",
        "a component's outer surface, in full: a list that stopped at the first command would say \
         the component answers for one thing when it answers for four",
    );
    assert_says(
        &readme,
        "It publishes `billing.invoice.InvoiceCancelled`, `billing.invoice.InvoiceCreated`, \
         `billing.invoice.InvoiceIssued` and `billing.invoice.InvoicePaid`.",
        "a component's outer surface",
    );
    assert_says(
        &readme,
        "not a deployment",
        "a component confused with a deployment is the mistake the three-layer split exists to stop",
    );

    assert_says(&topology, "## `invoice-service`", "a workload");
    assert_says(
        &topology,
        "At least 2 instances.",
        "a replica floor is a claim about correctness and has to be in the docs as one",
    );
    assert_says(
        &topology,
        "not correct with fewer",
        "min: 2 means the system is wrong with one instance, not that it is busy",
    );
    assert_says(
        &topology,
        "No ceiling is declared.",
        "an absent max is a fact",
    );
    assert_says(&topology, "Stateless", "whether state outlives a request");
    assert_says(&topology, "- `postgres` — `invoice-store`", "a requirement");
    assert_says(&topology, "- `publish` — `invoice-events`", "a requirement");

    let all = everything(&docs);
    for name in billing().components.keys() {
        assert_says(&all, name.as_str(), "no component may be missing");
    }
}

#[test]
fn a_binding_renders_as_a_flow_and_a_lifecycle_as_a_state_diagram() {
    // The two things a reader cannot get from a table: what happens after a failure, and what shape
    // a lifecycle is.
    let docs = docs();

    let interactions = page(&docs, "docs/interactions.md");
    assert_says(
        &interactions,
        "```mermaid\nflowchart LR",
        "a binding is a flow",
    );
    assert_says(
        &interactions,
        "escalated to a person",
        "the failure edge leaves the system, and that edge is why the word is required",
    );
    assert_says(
        &interactions,
        "escalation[\"billing.email.DeliveryEscalated\"]",
        "the escalation's event is a node on the flow, because it is the one thing a reader can \
         look for to tell an escalation from nothing happening",
    );

    let readme = page(&docs, "docs/README.md");
    assert_says(&readme, "```mermaid\nflowchart TB", "the system is a graph");
    assert_says(
        &readme,
        "-.->|\"notify-on-invoice-created\"|",
        "a binding is the dashed edge between two components",
    );

    let invoice = page(&docs, "docs/domains/billing.invoice.md");
    assert_says(
        &invoice,
        "```mermaid\nstateDiagram-v2",
        "a lifecycle is a diagram",
    );
    for state in ["Draft", "Issued", "Paid", "Cancelled"] {
        assert_says(&invoice, state, "a state the entity can be in");
    }
}

// ---- entities, views and actors ---------------------------------------------------------------

#[test]
fn an_entitys_lifecycle_transitions_reach_the_page_as_arrows() {
    // The state *set* was all the IR used to carry, and a diagram of four unconnected states is not
    // a lifecycle: which moves exist is the whole content of `examples/billing/`'s invoice.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "    [*] --> Draft\n",
        "where an instance starts is a fact a reader cannot infer from the state names",
    );
    for arrow in [
        "    Draft --> Issued: issue (IssueInvoice)\n",
        "    Issued --> Paid: settle (PayInvoice)\n",
        "    Draft --> Cancelled: cancel (CancelInvoice)\n",
        "    Issued --> Cancelled: cancel (CancelInvoice)\n",
    ] {
        assert_says(
            &invoice,
            arrow,
            "a transition missing from the diagram is a move nobody knows is legal, and one with no \
             command on it is a move nobody can make",
        );
    }
    // `cancel` leaves two states, and one arrow per source is the only rendering that keeps that.
    assert_eq!(
        invoice.matches(": cancel (CancelInvoice)\n").count(),
        2,
        "`cancel` leaves both Draft and Issued: {invoice}"
    );
    for terminal in ["    Paid --> [*]\n", "    Cancelled --> [*]\n"] {
        assert_says(
            &invoice,
            terminal,
            "a terminal state is where an instance may stop, which is declared and not inferred",
        );
    }
    assert_says(
        &invoice,
        "An instance is created in `Draft`.",
        "the initial state in words, for a reader who does not read Mermaid",
    );
}

#[test]
fn the_command_that_takes_each_move_reaches_the_page_beside_the_move_itself() {
    // Gate G14, as a reader meets it. A lifecycle whose arrows have no verbs is a diagram of what
    // may happen with nothing that makes any of it happen, and `Issued -> Paid` is design §19's
    // worked example: the scenario it wants to generate is unwritable until the page can say which
    // command takes it.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "- `settle` — taken by `billing.invoice.PayInvoice` on its `settled` outcome",
        "`Issued -> Paid` names the command and the branch that takes it",
    );
    assert_says(
        &invoice,
        "- `issue` — taken by `billing.invoice.IssueInvoice` on its `issued` outcome",
        "every declared move names its cause, not only the interesting one",
    );
    assert_says(
        &invoice,
        "- `cancel` — taken by `billing.invoice.CancelInvoice` on its `cancelled` outcome",
        "a move leaving two states still has one cause",
    );
    assert_says(
        &invoice,
        "An instance is brought into existence by `billing.invoice.CreateInvoice` on its \
         `accepted` outcome.",
        "creation is not a transition, and a page that only listed moves would never say where an \
         instance comes from",
    );
    assert!(
        !invoice.contains("taken by nothing in this specification"),
        "an uncaused move is refused rather than documented: {invoice}"
    );
}

#[test]
fn an_outcome_says_what_it_does_to_an_entity_and_a_refusal_says_it_changes_none() {
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "It creates a `billing.invoice.Invoice`, which starts in `Draft`.",
        "the branch that brings an invoice into existence says so, and says where it starts",
    );
    assert_says(
        &invoice,
        "It moves a `billing.invoice.Invoice` from `Issued` to `Paid`, along the declared move \
         `settle`.",
        "an outcome that moves an entity carries both states, so a scenario knows what to set up \
         and what to assert",
    );
    assert_says(
        &invoice,
        "No entity in this specification changes.",
        "silence would read the same as a projection that dropped the field; a refusal says it \
         changed nothing",
    );

    // `SendEmail` acts on no entity at all, which is the reason the link is optional on this side.
    let email = page(&docs(), "docs/domains/billing.email.md");
    assert_says(
        &email,
        "No entity in this specification changes.",
        "a command that changes no entity is not made to invent one",
    );
    assert!(
        !email.contains("It creates a "),
        "nothing in the email context creates an entity: {email}"
    );
}

#[test]
fn an_entitys_absent_transition_is_named_as_a_move_the_specification_does_not_permit() {
    // The example's headline case: a paid invoice may not be cancelled, and the model says so by not
    // saying anything. A diagram cannot draw an absence, so the complement is written out — and it
    // must be the complement of the real transitions, not of an empty set.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "- `Paid` may not become `Cancelled`",
        "the one prohibition the billing example exists to express",
    );
    assert_says(
        &invoice,
        "- `Draft` may not become `Paid`",
        "an invoice is not paid without being issued",
    );
    assert!(
        !invoice.contains("`Draft` may not become `Issued`"),
        "`issue` moves Draft to Issued, so calling it forbidden would be a false statement: \
         {invoice}"
    );
    assert!(
        !invoice.contains("*missing*, not *absent*"),
        "the transitions are carried now, so the page must not warn that they are missing: \
         {invoice}"
    );
}

#[test]
fn an_entitys_invariant_reaches_the_page_as_a_condition_on_every_instance() {
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "Every instance satisfies `total.amount >= 0`",
        "an invariant dropped from the docs is an invariant nobody knows to implement",
    );
    // The author's own spelling, not a re-rendering of the parsed predicate: a diagnostic quotes the
    // specification, and so does this page.
    assert!(
        !invoice.contains("total.amount >= 0.0"),
        "the invariant is quoted as written: {invoice}"
    );
}

#[test]
fn an_entitys_identity_reaches_the_page_by_name_and_not_only_by_type() {
    // The wave-1 decision the IR carries: without the identity's name, every projection invents one
    // and the view projecting `invoice_id` agrees with none of them.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "An instance is identified by `invoice_id`, a `billing.invoice.InvoiceId`",
        "the identity's name is part of the model, so it is on the page",
    );
    assert_says(
        &invoice,
        "- `settlement_window` — `Duration`",
        "an entity's own fields are its shape, and were absent from the IR entirely",
    );
    assert_says(
        &invoice,
        "Its state is a `billing.invoice.Invoice.State`",
        "the synthesised state enum belongs to the entity that forms it, not to the type list",
    );
}

#[test]
fn a_views_eventual_consistency_reads_differently_from_an_immediate_one() {
    // Two views over one entity, and the difference between them decides whether a generated
    // assertion may run once. A page that renders both the same way is a page that cannot be used to
    // write either test.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "**Eventual**: it catches up some time after the command returns",
        "`InvoiceById` is a projection, and a reader has to know a read may not see its own write",
    );
    assert_says(
        &invoice,
        "retries the assertion until the projection catches up",
        "what eventual consistency costs a generated test is the reason the field exists",
    );

    assert_says(
        &invoice,
        "**Read-your-writes**: it is current the moment the command that changed it returns",
        "`OutstandingInvoices` promises more, and the promise is what a caller relies on",
    );
    assert_says(
        &invoice,
        "asserts it once, immediately after the command",
        "an immediate view asserted with a retry is a promise nobody checks",
    );

    // Both views read the same entity, and both say so rather than leaving a reader to match names.
    assert_eq!(
        invoice.matches("It reads [`Invoice`](#invoice).").count(),
        2,
        "each view names its source entity: {invoice}"
    );
}

#[test]
fn a_views_filter_reaches_the_page_rather_than_being_silently_dropped() {
    // A filter is what makes a view a subset. Dropping it turns "the invoices still owed" into "the
    // invoices", which is a different promise and one nobody would notice being made.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(
        &invoice,
        "It contains the instances where `state == Issued` holds",
        "the filter is rendered as written, so a reader can tell which instances are in the view",
    );
    assert_says(
        &invoice,
        "It contains every instance of that entity: no filter narrows it",
        "an unfiltered view says so; silence would read the same as a filter that was dropped",
    );
}

#[test]
fn an_actors_grant_renders_as_an_edge_from_the_actor_to_that_command_in_the_index_graph() {
    let docs = docs();
    let readme = page(&docs, "docs/README.md");

    assert_says(
        &readme,
        "    subgraph who[\"who may ask\"]",
        "design §9's graph begins at the actor, and so does this one",
    );
    assert_says(
        &readme,
        "who1[\"billing.invoice.Customer\"]",
        "an actor is a node in the system graph",
    );
    // The node id is read out of the graph rather than written here: Mermaid ids are indices into
    // the IR's own order, so hard-coding one asserts how many commands the example happens to
    // declare instead of asserting that the grant is an edge to *this* command.
    let node = readme
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_suffix("[\"billing.invoice.CreateInvoice\"]")
        })
        .unwrap_or_else(|| panic!("the graph has a node for `CreateInvoice`: {readme}"));
    assert_says(
        &readme,
        &format!("who1 -->|\"may invoke\"| {node}"),
        "the grant is the edge; without it the actor is decoration",
    );
    assert!(
        !readme.contains("who0 -->"),
        "`Auditor` may invoke nothing, so no edge leaves it: {readme}"
    );
    assert!(
        !readme.contains("that step is missing here"),
        "the index no longer apologises for the arrow it now draws: {readme}"
    );

    // And the same grant, beside the actor, as a link to where the command is written.
    let invoice = page(&docs, "docs/domains/billing.invoice.md");
    assert_says(
        &invoice,
        "It may invoke [`CreateInvoice`](#createinvoice).",
        "a grant is only useful if a reader can reach the command it names",
    );
    assert_says(
        &invoice,
        "### `CreateInvoice`",
        "that link has to land on a heading that exists",
    );
}

#[test]
fn an_actor_that_may_invoke_nothing_is_still_on_the_page() {
    // "Who is in this picture" is part of the specification. An actor with no grants dropped from
    // the page reads as an actor nobody declared.
    let invoice = page(&docs(), "docs/domains/billing.invoice.md");

    assert_says(&invoice, "### `Auditor`", "an observer is declared");
    assert_says(
        &invoice,
        "It may invoke nothing: it observes.",
        "an empty grant list is a statement, not an unfinished line",
    );
}

#[test]
fn a_grant_that_crosses_two_contexts_links_to_the_other_contexts_page() {
    // Within one context a grant is a fragment; across two it has to leave the page, and a link that
    // opens the right document at the wrong place fails silently.
    let depot = pages(&compiled(A_GRANT_ACROSS_CONTEXTS));
    let orders = page(&depot, "docs/domains/depot.orders.md");

    assert_says(
        &orders,
        "It may invoke [`depot.shipping.Dispatch`](depot.shipping.md#dispatch).",
        "a grant across contexts names the other context's page, not a fragment of this one",
    );
    assert_says(
        &page(&depot, "docs/domains/depot.shipping.md"),
        "### `Dispatch`",
        "and that page has the heading the link names",
    );
}

#[test]
fn every_name_the_ir_holds_appears_on_some_page() {
    // The acceptance criterion for this projection, stated once over everything rather than per
    // construct: a name in the model and not in the documentation is the model outgrowing its
    // description, which is the failure documentation-first exists to catch.
    let ir = billing();
    let all = everything(&docs());

    let mut names: Vec<String> = Vec::new();
    names.extend(ir.domains.keys().map(ToString::to_string));
    names.extend(ir.types.keys().map(ToString::to_string));
    names.extend(ir.entities.keys().map(ToString::to_string));
    names.extend(ir.commands.keys().map(ToString::to_string));
    names.extend(ir.events.keys().map(ToString::to_string));
    names.extend(ir.errors.keys().map(ToString::to_string));
    names.extend(ir.views.keys().map(ToString::to_string));
    names.extend(ir.actors.keys().map(ToString::to_string));
    names.extend(ir.bindings.keys().map(ToString::to_string));
    names.extend(ir.components.keys().map(ToString::to_string));
    for entity in ir.entities.values() {
        names.push(entity.identity.name.clone());
        names.extend(entity.fields.iter().map(|field| field.name.clone()));
        names.extend(
            entity
                .lifecycle
                .states
                .iter()
                .map(|state| state.as_str().to_owned()),
        );
        names.extend(
            entity
                .lifecycle
                .transitions
                .iter()
                .map(|transition| transition.name.clone()),
        );
        names.extend(
            entity
                .invariants
                .iter()
                .map(|invariant| invariant.statement.clone()),
        );
    }
    for view in ir.views.values() {
        names.extend(view.fields.iter().map(|field| field.name.clone()));
        if let Some(filter) = &view.filter {
            names.push(filter.to_string());
        }
    }
    for command in ir.commands.values() {
        names.extend(command.input.iter().map(|field| field.name.clone()));
        names.extend(
            command
                .outcomes
                .iter()
                .map(|outcome| outcome.name.to_string()),
        );
    }
    for event in ir.events.values() {
        names.extend(event.fields.iter().map(|field| field.name.clone()));
    }
    for error in ir.errors.values() {
        names.extend(error.fields.iter().map(|field| field.name.clone()));
    }
    for conversion in &ir.conversions {
        names.push(
            conversion
                .because
                .split(';')
                .next()
                .unwrap_or_default()
                .to_owned(),
        );
    }

    let missing: Vec<_> = names
        .iter()
        .filter(|name| !all.contains(name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "the documentation never mentions {missing:?}"
    );
}

// ---- the gap mechanism, kept and empty --------------------------------------------------------

#[test]
fn an_empty_gap_allowlist_puts_no_cannot_show_section_on_any_page() {
    // The mechanism survives its own emptiness. Every construct the IR carries is rendered, so no
    // page may carry a "what this cannot show" heading — and the assertion is written from
    // `known_gaps` rather than from a hard-coded expectation, so the day an entry comes back the
    // section has to come back with it.
    let docs = docs();
    assert!(
        Docs::known_gaps().is_empty(),
        "a declared gap has to be printed, so this test has to become the other assertion: {:?}",
        Docs::known_gaps()
    );

    for (path, artifact) in &docs {
        // The table the mechanism prints, rather than the heading each page chooses for it: a page
        // may reword its heading, and this assertion is about there being nothing under one.
        assert!(
            !artifact
                .contents
                .contains("| construct | what is dropped |"),
            "`{path}` prints a gap table with nothing in it, which teaches a reader to skip the one \
             that matters"
        );
        assert!(
            !artifact.contents.contains("What this page cannot show"),
            "`{path}` heads a section it has nothing to put in"
        );
        assert!(
            !artifact.contents.contains("ess-compiler"),
            "`{path}` still asks a reader to go fix the compiler for a construct it now renders"
        );
    }
}

#[test]
fn every_member_of_a_resolved_domain_reaches_the_page_of_the_context_it_belongs_to() {
    // The guard against the IR growing a member this projection then quietly ignores. It is not a
    // list of names kept in step by hand: each member is paired with the thing a reader would lose
    // if it stopped being rendered, so a new member fails here until somebody renders it and says
    // what it is for.
    let evidence: BTreeMap<&str, &str> = [
        (
            "name",
            "`billing.invoice` is one of billing's bounded contexts",
        ),
        ("naming", "# Invoicing"),
        ("types", "### `Money`"),
        ("entities", "### `Invoice`"),
        ("views", "### `OutstandingInvoices`"),
        ("commands", "### `CreateInvoice`"),
        ("events", "### `InvoiceCreated`"),
        ("errors", "### `InvalidAmount`"),
        ("actors", "### `Customer`"),
    ]
    .into_iter()
    .collect();

    let ir = billing();
    let domain = ir
        .domains
        .get(&ess_domain::name::QualifiedName::new("billing.invoice").expect("a name"))
        .expect("the invoice context");
    let json = serde_json::to_value(domain).expect("a domain serialises");
    let members: BTreeSet<&str> = json
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(
        members,
        evidence.keys().copied().collect::<BTreeSet<_>>(),
        "`ResolvedDomain` has grown a member: docs.rs must render it, and this test must say what a \
         reader would lose if it did not"
    );

    let invoice = page(&docs(), "docs/domains/billing.invoice.md");
    for (member, wanted) in &evidence {
        assert_says(
            &invoice,
            wanted,
            &format!("`ResolvedDomain::{member}` has to reach the context's own page"),
        );
    }
}

// ---- link plumbing ---------------------------------------------------------------------------

/// Every relative Markdown link target in a page.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(offset) = text[index..].find("](") {
        let start = index + offset + 2;
        let end = bytes[start..]
            .iter()
            .position(|byte| *byte == b')')
            .map_or(text.len(), |it| start + it);
        let target = &text[start..end];
        if !target.starts_with("http") && !target.starts_with('#') {
            out.push(target.to_owned());
        }
        index = end;
    }
    out
}

/// A link split into its file and its heading.
fn split_fragment(link: &str) -> (&str, Option<&str>) {
    match link.split_once('#') {
        Some((target, fragment)) => (target, Some(fragment)),
        None => (link, None),
    }
}

/// A link resolved against the page holding it, with `..` applied.
fn resolve(from: &str, target: &str) -> String {
    let mut segments: Vec<&str> = from.split('/').collect();
    segments.pop();
    for segment in target.split('/') {
        if segment == ".." {
            segments.pop();
        } else if segment != "." {
            segments.push(segment);
        }
    }
    segments.join("/")
}

/// Every anchor a Markdown renderer derives from a page's headings.
///
/// Implemented here rather than shared with `docs.rs`, so that agreeing with the renderer's rule is
/// what makes the test pass, not agreeing with this crate's idea of the rule.
fn anchors(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.trim_start().strip_prefix('#'))
        .map(|line| {
            let heading = line.trim_start_matches('#').trim();
            heading
                .chars()
                .filter_map(|character| {
                    if character.is_ascii_alphanumeric() {
                        Some(character.to_ascii_lowercase())
                    } else if character == '-' || character == '_' {
                        Some(character)
                    } else if character == ' ' {
                        Some('-')
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect()
}
