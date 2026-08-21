//! JSON Schema for every message that crosses this system's boundary.
//!
//! A command's input, an event's payload, an error's payload, and every named type the model
//! declares: one draft 2020-12 document each. Roadmap W3.1 asks this projection to prove that the
//! type system is projectable without loss, so the interesting part is not the emitting — it is the
//! handful of places where JSON Schema is weaker than the model and something has to be *decided*
//! rather than translated.
//!
//! Those decisions are **not here**. They live in `types`, the one type mapping all three contract
//! projections call, because this file and `asyncapi.rs` each used to carry a copy and the two copies
//! disagreed about the same event. What is left here is the document layout: which files exist, what
//! each one describes, and where a reference resolves.
//!
//! # One self-contained document per message
//!
//! Every file declares the dialect, describes one thing, and carries under `$defs` exactly the named
//! types it reaches transitively. Nothing refers to another file.
//!
//! The alternative — one shared `types.json`, `$ref`d from every message — would give one canonical
//! definition per named type instead of `billing.invoice.Money` appearing in five files, and that is
//! what a code generator walking the whole tree wants: five copies invite five `Money` classes. It
//! lost anyway, because cross-file `$ref` resolution is where JSON Schema tooling actually fails. It
//! needs a base URI and a retriever, and an artifact tree that validates only when loaded through a
//! correctly configured registry is a tree that does not validate in the field. A file here can be
//! copied on its own into a service repository and handed to any validator, which is how these
//! travel.
//!
//! # Draft 2020-12, where `aep-schema` publishes draft-07
//!
//! `OpenAPI` 3.1's schema dialect *is* JSON Schema 2020-12, and the `OpenAPI` projection embeds these
//! schemas: one dialect means it embeds them rather than translating them, and a translation step
//! would have been a second place for `Map<String, Money>` to come out slightly different. 2020-12
//! also keeps keywords that sit beside a `$ref`, which draft-07 discards — this projection puts a
//! `title` and a field's summary beside almost every reference, and a type document, whose root *is*
//! a `$ref`, would lose its `$defs` and stop resolving at all. `aep-schema`'s draft is not a
//! convention worth matching: it is whatever `schemars` emits, for documents an editor validates,
//! which is a different consumer from a service validating a request body.
//!
//! # Provenance goes in a keyword, not a comment
//!
//! `x-ess-provenance`, at the root of every document, holding design §10's four facts.
//! `Attribution` carries why that is a keyword rather than a `$comment`. The
//! cost is that a linter demanding a closed keyword set will flag it.

// The shared mapping lives at `src/types.rs`, which is the layout this extraction was asked for, and
// it is declared from here rather than from `lib.rs` because `lib.rs` belongs to another change. The
// one-line fix, when that file is free, is to move this declaration into it and drop the attribute.
#[path = "types.rs"]
pub(crate) mod types;

use ess_compiler::ir::ResolvedType;
use ess_compiler::refs::{CommandRef, DeclaredTypeRef, ErrorRef, EssSemanticRef, EventRef};
use ess_compiler::EssIr;

use crate::artifact::{Artifact, Generator};
use crate::provenance::ProvenanceMint;
use crate::schema::types::{Attribution, Message, Node};

/// The dialect every emitted document declares.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// JSON Schema per command input, event payload and error payload.
pub struct JsonSchema;

impl Generator for JsonSchema {
    fn name(&self) -> &'static str {
        "schema"
    }

    fn describes(&self) -> &'static str {
        "JSON Schema per command input and event payload"
    }

    fn directory(&self) -> &'static str {
        "schema"
    }

    fn generate(&self, ir: &EssIr, mint: &ProvenanceMint) -> Vec<Artifact> {
        let mut out = Vec::new();

        // Every named type, not only the ones a message reaches: the union and the enums in the
        // billing example are declared and referenced by an entity, and a projection that dropped
        // them would look complete while the construct this crate most needs to prove — a tagged
        // union that round-trips — appeared in no file at all.
        //
        // Each document's slice is seeded at the one construct it is about. The closure brings in
        // everything the construct rests on — the types a payload reaches, however deep — so the
        // digest moves exactly when something this document could render moves.
        for declared in ir.types.values() {
            out.push(type_document(ir, declared, mint));
        }
        for command in ir.commands.values() {
            out.push(message_document(
                ir,
                &Message::of_command(command),
                CommandRef::new(command.name.clone()).into(),
                mint,
            ));
        }
        for event in ir.events.values() {
            out.push(message_document(
                ir,
                &Message::of_event(event),
                EventRef::new(event.name.clone()).into(),
                mint,
            ));
        }
        for error in ir.errors.values() {
            out.push(message_document(
                ir,
                &Message::of_error(error),
                ErrorRef::new(error.name.clone()).into(),
                mint,
            ));
        }

        out
    }
}

/// The document for one named type.
///
/// The root is a `$ref` into the document's own `$defs` rather than the type's schema inline, so that
/// the shape is the same whether or not the type reaches itself. A struct with a field of its own
/// type is representable in the model, and inlining would have needed a special case for exactly the
/// construct most likely to be got wrong.
fn type_document(ir: &EssIr, declared: &ResolvedType, mint: &ProvenanceMint) -> Artifact {
    // The type itself is inserted by hand rather than used as the root of the walk: a handle has no
    // public constructor, so a projection holding a `ResolvedType` cannot ask for its own handle —
    // and everything the type reaches is reachable from its body.
    let mut defs = types::definitions(ir, types::body_leaves(&declared.body));
    defs.insert(declared.name.to_string(), types::body(declared));

    let sliced = mint.of_seeds([DeclaredTypeRef::new(declared.name.clone()).into()]);
    let root = Node {
        dialect: Some(DIALECT),
        reference: Some(types::pointer(&declared.name)),
        provenance: Some(Attribution::new(&sliced.provenance)),
        defs,
        ..Node::default()
    };

    Artifact::sliced(
        format!("types/{}.schema.json", declared.name),
        root.to_canonical_json(),
        sliced.slice,
    )
}

/// The document for one message.
fn message_document(
    ir: &EssIr,
    carried: &Message<'_>,
    seed: EssSemanticRef,
    mint: &ProvenanceMint,
) -> Artifact {
    let sliced = mint.of_seeds([seed]);
    let root = Node {
        dialect: Some(DIALECT),
        provenance: Some(Attribution::new(&sliced.provenance)),
        defs: types::definitions(ir, types::field_leaves(carried.fields)),
        ..types::message(carried)
    };

    Artifact::sliced(
        format!("{}/{}.schema.json", carried.directory(), carried.name),
        root.to_canonical_json(),
        sliced.slice,
    )
}
