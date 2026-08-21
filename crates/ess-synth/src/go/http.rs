//! The server-package emitter: the same transport the Rust target derives, in the second language.
//!
//! # Same derivation, same routes, same statuses
//!
//! Nothing here decides anything the Rust emitter did not already have decided for it. The paths
//! come from [`ess_gen::http::routes`] and the statuses from [`ess_gen::http::status`] — the same
//! two functions the `OpenAPI` projection builds its document from — so the question "do the two
//! synthesised applications serve the same surface" is not answered by comparing two emitters. It
//! is answered by there being one mapping.
//!
//! # What differs from the Rust target, and why
//!
//! | | Rust | Go |
//! |---|---|---|
//! | the HTTP layer | hand-written over `std::net::TcpListener`, about two hundred fixed lines | `net/http`, which is in the standard library and therefore free under the same no-dependency rule |
//! | JSON | the emitted reader and writer the browser bridge already needed | `encoding/json`, with `UseNumber` so an `Integer` past 2^53 survives |
//! | the codecs | generated encoders and decoders over the emitted `json::Value` | generated encoders and decoders over `any`, because the generated types carry unexported fields and `encoding/json` cannot see them |
//!
//! The third row is the one that matters: a Go type whose field is unexported is invisible to
//! `encoding/json`, and exporting them would undo the distinctness the newtype encoding exists for.
//! So the crossing is emitted beside the types, exactly as it is for Rust, and for the same reason
//! design §9 gives: a semantic type knows nothing about a transport.
//!
//! # What it does not decide
//!
//! It chooses no realization. `Serve*` takes the assembled system, and a system over unimplemented
//! obligations answers `501` naming what the plan owes.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{
    EssIr, ResolvedBody, ResolvedCommand, ResolvedComponent, ResolvedError, ResolvedField,
    ResolvedType, ResolvedTypeRef, ResolvedView,
};
use ess_domain::component::Reach;
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;
use ess_gen::http::{self, Method, Served};
use ess_gen::{Artifact, Provenance};

use crate::plan::{Capability, CapabilityKind, SynthesisPlan};

use super::layout::{Layout, Package};
use super::refusal::TargetRefusals;
use super::{name, Emit};

/// The label every startup line carries.
const LOG_FORMAT: &str = "ess/1";

/// The transport the emitted server speaks, as the startup record names it.
const TRANSPORT: &str = "http/1.1";

/// Every component whose surface the specification says is reached from outside, in name order.
pub(crate) fn served<'a>(ir: &'a EssIr, refusals: &TargetRefusals) -> Vec<&'a ResolvedComponent> {
    ir.components
        .values()
        .filter(|component| component.reached_by == Reach::Network)
        .filter(|component| {
            !refusals.refuses(&Capability {
                kind: CapabilityKind::ComponentPort,
                source: component.name.to_string(),
            })
        })
        .collect()
}

/// The server package, when any component's surface is served — and nothing at all when none is.
pub(super) fn server_package(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    refusals: &TargetRefusals,
    covered: &mut BTreeSet<Capability>,
) -> Vec<Artifact> {
    let components = served(ir, refusals);
    if components.is_empty() {
        return Vec::new();
    }
    let package = layout.server();
    let provenance = &plan.provenance;
    let mut artifacts = vec![
        helpers_file(ir, layout, package, provenance),
        wire_file(ir, plan, layout, refusals, package, provenance),
    ];

    for component in &components {
        covered.insert(Capability {
            kind: CapabilityKind::ComponentTransport,
            source: component.name.to_string(),
        });
        artifacts.push(surface_file(
            ir,
            plan,
            layout,
            package,
            component,
            &components,
            provenance,
        ));
        artifacts.push(Artifact::new(
            format!("{}/{}.openapi.json", package.dir, component.name),
            ess_gen::openapi::json(ir, component),
        ));
        artifacts.push(Artifact::new(
            format!("{}/{}.docs.md", package.dir, component.name),
            ess_gen::docs::served(ir, component),
        ));
    }
    artifacts
}

/// The package's own file: what an answer is, how a body is read, and the two media types.
///
/// One file for the package rather than one per served component, because Go would refuse the
/// second declaration of `response` — and because these lines are the same whatever the
/// specification, exactly as the Rust target's `http` module is.
fn helpers_file(
    ir: &EssIr,
    layout: &Layout,
    package: &Package,
    provenance: &Provenance,
) -> Artifact {
    let emit = Emit::new(ir, layout, package, None);
    emit.import("bytes");
    emit.import("encoding/json");
    emit.import("fmt");
    emit.import("io");
    emit.import("net/http");
    emit.import("strconv");
    emit.file(provenance, SERVER_DOC, SURFACE_HELPERS)
}

// ---- the codecs -------------------------------------------------------------------------------

/// Every generated declaration, as JSON, in both directions.
fn wire_file(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    refusals: &TargetRefusals,
    package: &Package,
    provenance: &Provenance,
) -> Artifact {
    let emit = Emit::new(ir, layout, package, None);
    emit.import("encoding/base64");
    emit.import("encoding/json");
    emit.import("fmt");
    emit.import("strconv");

    let presents = |kind: CapabilityKind, declared: &QualifiedName| {
        plan.is_generated(kind, &declared.to_string())
            && !refusals.refuses(&Capability {
                kind,
                source: declared.to_string(),
            })
    };

    let mut body = String::from(WIRE_HELPERS);
    for declared in ir.types.values() {
        if presents(CapabilityKind::DomainType, &declared.name) {
            type_encoder(&mut body, &emit, declared);
            type_decoder(&mut body, &emit, declared);
        }
    }
    for error in ir.errors.values() {
        if presents(CapabilityKind::ErrorType, &error.name) {
            error_encoder(&mut body, &emit, error);
        }
    }
    for view in ir.views.values() {
        if presents(CapabilityKind::ViewType, &view.name) {
            view_encoder(&mut body, &emit, view);
        }
    }
    for command in ir.commands.values() {
        if presents(CapabilityKind::CommandContract, &command.name) {
            command_decoder(&mut body, &emit, command);
        }
    }
    emit.file_at(format!("{}/wire.go", package.dir), provenance, "", &body)
}

/// The function-name fragment of a declaration: its whole qualified name, pascal-cased.
///
/// The whole name and never the local one, for the reason the Rust wire gives: two declarations in
/// two contexts can share a last segment, and a codec that named only that would silently be one
/// function. Not run through the layout's name table, deliberately — every identifier this package
/// declares is either `encode`/`decode` plus a unique qualified name, or one of the fixed
/// lower-case helpers below, and the two families cannot collide.
fn ident(declared: &QualifiedName) -> String {
    name::type_fragment(&declared.to_string())
}

/// One declared type, written.
fn type_encoder(out: &mut String, emit: &Emit<'_>, declared: &ResolvedType) {
    let go = emit.reference(&declared.name);
    let function = format!("encode{}", ident(&declared.name));
    let _ = write!(
        out,
        "\n// {function} writes `{}` as JSON.\nfunc {function}(value {go}) any {{\n",
        declared.name
    );
    match &declared.body {
        ResolvedBody::Newtype { of, .. } => {
            let mut slot = 0;
            let expression = encode_into(out, "\t", "value.Value()", of, &mut slot);
            let _ = writeln!(out, "\treturn {expression}");
        }
        ResolvedBody::Struct { fields, .. } => {
            out.push_str("\tout := map[string]any{}\n");
            let mut slot = 0;
            for field in fields {
                encode_member(
                    out,
                    "\t",
                    ess_gen::schema::wire_field_name(field),
                    &format!("value.{}", name::exported(&field.name)),
                    &field.type_ref,
                    &mut slot,
                );
            }
            out.push_str("\treturn out\n");
        }
        ResolvedBody::Enum { variants } => {
            out.push_str("\tswitch value.(type) {\n");
            for variant in variants {
                let _ = writeln!(
                    out,
                    "\tcase {}:\n\t\treturn {variant:?}",
                    emit.reference_variant(&declared.name, variant)
                );
            }
            out.push_str(UNREACHABLE_VARIANT);
        }
        ResolvedBody::Union { tag, variants } => {
            let content = ess_gen::schema::union_content_key(tag);
            out.push_str("\tswitch shape := value.(type) {\n");
            for (label, payload) in variants {
                let _ = writeln!(
                    out,
                    "\tcase {}:",
                    emit.reference_variant(&declared.name, label)
                );
                let _ = writeln!(out, "\t\tout := map[string]any{{}}");
                let _ = writeln!(out, "\t\tout[{tag:?}] = {label:?}");
                let mut slot = 0;
                encode_member(out, "\t\t", content, "shape.Value", payload, &mut slot);
                out.push_str("\t\treturn out\n");
            }
            out.push_str(UNREACHABLE_SHAPE);
        }
    }
    out.push_str("}\n");
}

/// One declared type, read.
fn type_decoder(out: &mut String, emit: &Emit<'_>, declared: &ResolvedType) {
    let go = emit.reference(&declared.name);
    let function = format!("decode{}", ident(&declared.name));
    let _ = write!(
        out,
        "\n// {function} reads `{}` from JSON, or refuses at the path it was reached \
         at.\nfunc {function}(value any, at string) ({go}, error) {{\n\tvar out {go}\n",
        declared.name
    );
    match &declared.body {
        ResolvedBody::Newtype { of, .. } => {
            let mut slot = 0;
            let held = decode_into(out, emit, "\t", "value", "at", of, &mut slot);
            let _ = writeln!(
                out,
                "\treturn {}({held}), nil",
                emit.reference_ctor(&declared.name)
            );
        }
        ResolvedBody::Struct { fields, .. } => {
            let _ = writeln!(
                out,
                "\tif _, err := objectAt(value, at, \"an object\"); err != nil {{\n\t\treturn out, \
                 err\n\t}}"
            );
            let mut slot = 0;
            for field in fields {
                decode_member(
                    out,
                    emit,
                    "\t",
                    &format!("out.{}", name::exported(&field.name)),
                    field,
                    &mut slot,
                );
            }
            out.push_str("\treturn out, nil\n");
        }
        ResolvedBody::Enum { variants } => {
            let expected = variant_list(variants);
            let _ = writeln!(
                out,
                "\ttext, err := textAt(value, at, {expected:?})\n\tif err != nil {{\n\t\treturn \
                 out, err\n\t}}\n\tswitch text {{"
            );
            for variant in variants {
                let _ = writeln!(
                    out,
                    "\tcase {variant:?}:\n\t\treturn {}{{}}, nil",
                    emit.reference_variant(&declared.name, variant)
                );
            }
            let _ = writeln!(
                out,
                "\t}}\n\treturn out, DecodeError{{At: at, Expected: {expected:?}, Found: \
                 fmt.Sprintf(\"`%s`\", text)}}"
            );
        }
        ResolvedBody::Union { tag, variants } => {
            let content = ess_gen::schema::union_content_key(tag);
            let labels: Vec<String> = variants.keys().cloned().collect();
            let expected = variant_list(&labels);
            let _ = writeln!(
                out,
                "\ttagged, tagAt, err := required(value, at, {tag:?})\n\tif err != nil {{\n\t\t\
                 return out, err\n\t}}\n\tlabel, err := textAt(tagged, tagAt, \
                 {expected:?})\n\tif err != nil {{\n\t\treturn out, err\n\t}}\n\tswitch label {{"
            );
            for (label, payload) in variants {
                let _ = writeln!(out, "\tcase {label:?}:");
                let carried = ResolvedField {
                    name: content.to_owned(),
                    type_ref: payload.clone(),
                    naming: ess_domain::name::Naming::default(),
                };
                let mut slot = 0;
                decode_member(out, emit, "\t\t", "", &carried, &mut slot);
                let _ = writeln!(
                    out,
                    "\t\treturn {}{{Value: shape}}, nil",
                    emit.reference_variant(&declared.name, label)
                );
            }
            let _ = writeln!(
                out,
                "\t}}\n\treturn out, DecodeError{{At: tagAt, Expected: {expected:?}, Found: \
                 fmt.Sprintf(\"`%s`\", label)}}"
            );
        }
    }
    out.push_str("}\n");
}

/// The set of legal spellings, as one phrase a refusal can carry.
fn variant_list(variants: &[String]) -> String {
    format!(
        "one of {}",
        variants
            .iter()
            .map(|variant| format!("`{variant}`"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// One declared error's encoder.
fn error_encoder(out: &mut String, emit: &Emit<'_>, error: &ResolvedError) {
    record_encoder(
        out,
        &format!("encodeError{}", ident(&error.name)),
        &emit.reference(&error.name),
        &format!("the declared error `{}`", error.name),
        &error.fields,
    );
}

/// One view row's encoder.
fn view_encoder(out: &mut String, emit: &Emit<'_>, view: &ResolvedView) {
    record_encoder(
        out,
        &format!("encodeView{}", ident(&view.name)),
        &emit.reference(&view.name),
        &format!("one row of the view `{}`", view.name),
        &view.fields,
    );
}

/// An encoder over a fixed list of fields.
fn record_encoder(
    out: &mut String,
    function: &str,
    go: &str,
    describes: &str,
    fields: &[ResolvedField],
) {
    let _ = write!(
        out,
        "\n// {function} writes {describes} as JSON.\nfunc {function}(value {go}) any \
         {{\n\tout := map[string]any{{}}\n"
    );
    let mut slot = 0;
    for field in fields {
        encode_member(
            out,
            "\t",
            ess_gen::schema::wire_field_name(field),
            &format!("value.{}", name::exported(&field.name)),
            &field.type_ref,
            &mut slot,
        );
    }
    out.push_str("\treturn out\n}\n");
    if fields.is_empty() {
        // A declaration with no fields is an empty struct here, so the parameter is never read.
        // Named `_` rather than silenced, because Go has no attribute for it and an unread
        // parameter is not an error — leaving it named would just be misleading.
        *out = out.replace(
            &format!("func {function}(value {go}) any {{"),
            &format!("func {function}(_ {go}) any {{"),
        );
    }
}

/// One command input's decoder.
fn command_decoder(out: &mut String, emit: &Emit<'_>, command: &ResolvedCommand) {
    let go = emit.reference(&command.name);
    let function = format!("decodeCommand{}", ident(&command.name));
    let _ = write!(
        out,
        "\n// {function} reads the input of `{}` from JSON.\nfunc {function}(value any, at string) \
         ({go}, error) {{\n\tvar out {go}\n\tif _, err := objectAt(value, at, \"an object\"); err \
         != nil {{\n\t\treturn out, err\n\t}}\n",
        command.name
    );
    let mut slot = 0;
    for field in &command.input {
        decode_member(
            out,
            emit,
            "\t",
            &format!("out.{}", name::exported(&field.name)),
            field,
            &mut slot,
        );
    }
    out.push_str("\treturn out, nil\n}\n");
}

// ---- the two walkers ---------------------------------------------------------------------------

/// One member of an object being written — or nothing at all where an optional value is absent.
fn encode_member(
    out: &mut String,
    indent: &str,
    wire: &str,
    source: &str,
    type_ref: &ResolvedTypeRef,
    slot: &mut usize,
) {
    if let ResolvedTypeRef::Optional { of } = type_ref {
        let held = format!("held{}", next(slot));
        let _ = writeln!(out, "{indent}if {source} != nil {{");
        let _ = writeln!(out, "{indent}\t{held} := *{source}");
        let inner = format!("{indent}\t");
        let expression = encode_into(out, &inner, &held, of, slot);
        let _ = writeln!(out, "{indent}\tout[{wire:?}] = {expression}");
        let _ = writeln!(out, "{indent}}}");
        return;
    }
    let expression = encode_into(out, indent, source, type_ref, slot);
    let _ = writeln!(out, "{indent}out[{wire:?}] = {expression}");
}

/// Emits whatever statements one value needs and returns the expression that is its JSON.
fn encode_into(
    out: &mut String,
    indent: &str,
    source: &str,
    type_ref: &ResolvedTypeRef,
    slot: &mut usize,
) -> String {
    match type_ref {
        ResolvedTypeRef::Primitive { name } => encode_primitive(*name, source),
        ResolvedTypeRef::Declared { name } => {
            format!("encode{}({source})", ident(name.name()))
        }
        // An absent optional inside a list or a map is `null`, which is the one place this
        // rendering differs from an absent *member*: a member is omitted, because that is what the
        // published contract's `required` list says, and a hole in an array has no such spelling.
        ResolvedTypeRef::Optional { of } => {
            let target = format!("held{}", next(slot));
            let _ = writeln!(out, "{indent}var {target} any");
            let _ = writeln!(out, "{indent}if {source} != nil {{");
            let inner = format!("{indent}\t");
            let held = format!("{target}Some");
            let _ = writeln!(out, "{indent}\t{held} := *{source}");
            let expression = encode_into(out, &inner, &held, of, slot);
            let _ = writeln!(out, "{indent}\t{target} = {expression}");
            let _ = writeln!(out, "{indent}}}");
            target
        }
        ResolvedTypeRef::List { of } => {
            let target = format!("items{}", next(slot));
            let _ = writeln!(
                out,
                "{indent}{target} := make([]any, 0, len({source}))\n{indent}for _, element := \
                 range {source} {{"
            );
            let inner = format!("{indent}\t");
            let expression = encode_into(out, &inner, "element", of, slot);
            let _ = writeln!(out, "{indent}\t{target} = append({target}, {expression})");
            let _ = writeln!(out, "{indent}}}");
            target
        }
        ResolvedTypeRef::Map { key, value } => {
            let target = format!("entries{}", next(slot));
            let _ = writeln!(
                out,
                "{indent}{target} := map[string]any{{}}\n{indent}for key, element := range \
                 {source} {{"
            );
            let inner = format!("{indent}\t");
            let expression = encode_into(out, &inner, "element", value, slot);
            let _ = writeln!(
                out,
                "{indent}\t{target}[{}] = {expression}",
                encode_key(*key, "key")
            );
            let _ = writeln!(out, "{indent}}}");
            target
        }
    }
}

/// One primitive, in the rendering the published contracts fix.
fn encode_primitive(primitive: Primitive, source: &str) -> String {
    match primitive {
        Primitive::String | Primitive::Boolean | Primitive::Integer => source.to_owned(),
        Primitive::Bytes => format!("base64.StdEncoding.EncodeToString({source})"),
        Primitive::Decimal | Primitive::Timestamp | Primitive::Duration | Primitive::Uuid => {
            format!("{source}.Value()")
        }
    }
}

/// One map key, as the text a JSON object key has to be.
fn encode_key(primitive: Primitive, source: &str) -> String {
    match primitive {
        Primitive::String => source.to_owned(),
        Primitive::Boolean => format!("strconv.FormatBool({source})"),
        Primitive::Integer => format!("strconv.FormatInt({source}, 10)"),
        Primitive::Bytes => format!("base64.StdEncoding.EncodeToString({source})"),
        Primitive::Decimal | Primitive::Timestamp | Primitive::Duration | Primitive::Uuid => {
            format!("{source}.Value()")
        }
    }
}

/// One member of an object being read, assigned to `target` — or left alone when it is optional
/// and absent. An empty `target` binds `shape` instead, which is what a union variant needs.
fn decode_member(
    out: &mut String,
    emit: &Emit<'_>,
    indent: &str,
    target: &str,
    field: &ResolvedField,
    slot: &mut usize,
) {
    let wire = ess_gen::schema::wire_field_name(field);
    let position = next(slot);
    let member = format!("member{position}");
    let at = format!("at{position}");
    let assign = |out: &mut String, indent: &str, expression: &str| {
        if target.is_empty() {
            let _ = writeln!(out, "{indent}shape := {expression}");
        } else {
            let _ = writeln!(out, "{indent}{target} = {expression}");
        }
    };

    if let ResolvedTypeRef::Optional { of } = &field.type_ref {
        let found = format!("found{position}");
        let _ = writeln!(
            out,
            "{indent}{member}, {at}, {found}, err := optional(value, at, {wire:?})\n{indent}if err \
             != nil {{\n{indent}\treturn out, err\n{indent}}}\n{indent}if {found} {{"
        );
        let inner = format!("{indent}\t");
        let held = decode_into(out, emit, &inner, &member, &at, of, slot);
        let holder = format!("some{position}");
        let _ = writeln!(out, "{indent}\t{holder} := {held}");
        assign(out, &inner, &format!("&{holder}"));
        let _ = writeln!(out, "{indent}}}");
        return;
    }

    let _ = writeln!(
        out,
        "{indent}{member}, {at}, err := required(value, at, {wire:?})\n{indent}if err != nil \
         {{\n{indent}\treturn out, err\n{indent}}}"
    );
    let held = decode_into(out, emit, indent, &member, &at, &field.type_ref, slot);
    assign(out, indent, &held);
}

/// Emits whatever statements one value needs and returns the variable holding the decoded value.
fn decode_into(
    out: &mut String,
    emit: &Emit<'_>,
    indent: &str,
    source: &str,
    at: &str,
    type_ref: &ResolvedTypeRef,
    slot: &mut usize,
) -> String {
    let position = next(slot);
    let held = format!("held{position}");
    match type_ref {
        ResolvedTypeRef::Primitive { name } => {
            let (helper, expected) = decode_primitive(*name);
            let _ = writeln!(
                out,
                "{indent}{held}, err := {helper}({source}, {at}, {expected:?})\n{indent}if err != \
                 nil {{\n{indent}\treturn out, err\n{indent}}}"
            );
            match name {
                Primitive::Decimal
                | Primitive::Timestamp
                | Primitive::Duration
                | Primitive::Uuid => {
                    format!("{}({held})", emit.primitive_ctor(*name))
                }
                _ => held,
            }
        }
        ResolvedTypeRef::Declared { name } => {
            let _ = writeln!(
                out,
                "{indent}{held}, err := decode{}({source}, {at})\n{indent}if err != nil \
                 {{\n{indent}\treturn out, err\n{indent}}}",
                ident(name.name())
            );
            held
        }
        // Inside a list or a map, `null` is the absent value: a member that is absent is handled by
        // `decode_member`, and this arm is the one an array element reaches.
        ResolvedTypeRef::Optional { of } => {
            let go = emit.go_type(of);
            let _ = writeln!(out, "{indent}var {held} *{go}");
            let _ = writeln!(out, "{indent}if {source} != nil {{");
            let inner = format!("{indent}\t");
            let inner_held = decode_into(out, emit, &inner, source, at, of, slot);
            let holder = format!("{held}Some");
            let _ = writeln!(
                out,
                "{indent}\t{holder} := {inner_held}\n{indent}\t{held} = &{holder}"
            );
            let _ = writeln!(out, "{indent}}}");
            held
        }
        ResolvedTypeRef::List { of } => {
            let go = emit.go_type(of);
            let items = format!("items{position}");
            let _ = writeln!(
                out,
                "{indent}{items}, err := itemsAt({source}, {at}, \"an array\")\n{indent}if err != \
                 nil {{\n{indent}\treturn out, err\n{indent}}}\n{indent}{held} := \
                 make([]{go}, 0, len({items}))\n{indent}for index, element := range {items} \
                 {{\n{indent}\telementAt := indexed({at}, index)"
            );
            let inner = format!("{indent}\t");
            let inner_held = decode_into(out, emit, &inner, "element", "elementAt", of, slot);
            let _ = writeln!(out, "{indent}\t{held} = append({held}, {inner_held})");
            let _ = writeln!(out, "{indent}}}");
            held
        }
        ResolvedTypeRef::Map { key, value } => {
            let key_go = emit.primitive_type(*key);
            let value_go = emit.go_type(value);
            let entries = format!("entries{position}");
            let _ = writeln!(
                out,
                "{indent}{entries}, err := objectAt({source}, {at}, \"an object\")\n{indent}if err \
                 != nil {{\n{indent}\treturn out, err\n{indent}}}\n{indent}{held} := \
                 make(map[{key_go}]{value_go}, len({entries}))\n{indent}for key, element := range \
                 {entries} {{\n{indent}\tentryAt := nested({at}, key)"
            );
            let inner = format!("{indent}\t");
            let decoded_key = decode_key(out, emit, &inner, *key, "key", "entryAt", slot);
            let inner_held = decode_into(out, emit, &inner, "element", "entryAt", value, slot);
            let _ = writeln!(out, "{indent}\t{held}[{decoded_key}] = {inner_held}");
            let _ = writeln!(out, "{indent}}}");
            held
        }
    }
}

/// The helper that reads one primitive, and what a refusal says belongs there.
fn decode_primitive(primitive: Primitive) -> (&'static str, &'static str) {
    match primitive {
        Primitive::String => ("textAt", "a string"),
        Primitive::Boolean => ("boolAt", "true or false"),
        Primitive::Integer => ("integerAt", "a whole number"),
        Primitive::Bytes => ("bytesAt", "base64-encoded bytes"),
        Primitive::Decimal => ("textAt", "a decimal as a string, such as `10.50`"),
        Primitive::Timestamp => ("textAt", "an RFC 3339 timestamp as a string"),
        Primitive::Duration => ("textAt", "an ISO 8601 duration as a string, such as `P30D`"),
        Primitive::Uuid => ("textAt", "a UUID as a string"),
    }
}

/// One map key, read back out of the text a JSON object key is.
fn decode_key(
    out: &mut String,
    emit: &Emit<'_>,
    indent: &str,
    primitive: Primitive,
    source: &str,
    at: &str,
    slot: &mut usize,
) -> String {
    let held = format!("key{}", next(slot));
    match primitive {
        Primitive::String => return source.to_owned(),
        Primitive::Boolean => {
            let _ = writeln!(
                out,
                "{indent}{held}, err := keyBool({source}, {at})\n{indent}if err != nil \
                 {{\n{indent}\treturn out, err\n{indent}}}"
            );
        }
        Primitive::Integer => {
            let _ = writeln!(
                out,
                "{indent}{held}, err := keyInteger({source}, {at})\n{indent}if err != nil \
                 {{\n{indent}\treturn out, err\n{indent}}}"
            );
        }
        Primitive::Bytes => {
            let _ = writeln!(
                out,
                "{indent}{held}, err := keyBytes({source}, {at})\n{indent}if err != nil \
                 {{\n{indent}\treturn out, err\n{indent}}}"
            );
        }
        Primitive::Decimal | Primitive::Timestamp | Primitive::Duration | Primitive::Uuid => {
            let _ = writeln!(
                out,
                "{indent}{held} := {}({source})",
                emit.primitive_ctor(primitive)
            );
        }
    }
    held
}

/// The next slot number, so two generated variables never share a name.
fn next(slot: &mut usize) -> usize {
    let position = *slot;
    *slot += 1;
    position
}

// ---- the surface ------------------------------------------------------------------------------

/// One served component: its routes, its startup record, its handlers and its listener.
fn surface_file(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    package: &Package,
    component: &ResolvedComponent,
    served: &[&ResolvedComponent],
    provenance: &Provenance,
) -> Artifact {
    let emit = Emit::new(ir, layout, package, None);
    emit.import_blank("embed");
    emit.import("encoding/json");
    emit.import("fmt");
    emit.import("net");
    emit.import("net/http");
    let system = emit.qualify(layout.system(), "System");
    let routes = http::routes(ir, component);
    let rows = table(&routes, ir);
    let exported = name::exported(&component.name.to_string());

    let mut body = String::new();
    documents(&mut body, component, &exported);
    route_table(&mut body, &rows, &exported);
    startup(&mut body, ir, plan, component, &rows, served, &exported);

    let _ = write!(
        body,
        "\n// Serve{exported} serves `{}` at address, and does not return while it can \
         answer.\n//\n// address may name port 0, which binds an ephemeral port; the startup \
         record says which one\n// was taken, because a caller that cannot learn the port cannot \
         make a request.\n//\n// It chooses no realization. Every command reaches the port, and a \
         port over unimplemented\n// obligations answers the typed refusal this surface reports as \
         501.\nfunc Serve{exported}(system *{system}, address string) error {{\n\tlistener, err := \
         net.Listen(\"tcp\", address)\n\tif err != nil {{\n\t\treturn err\n\t}}\n\tbound, ok := \
         listener.Addr().(*net.TCPAddr)\n\tif !ok {{\n\t\treturn fmt.Errorf(\"the listener bound \
         something that is not a TCP address\")\n\t}}\n\tannounce{exported}(bound)\n\treturn \
         http.Serve(listener, http.HandlerFunc(func(writer http.ResponseWriter, request \
         *http.Request) {{\n\t\tanswer := dispatch{exported}(system, \
         request)\n\t\tanswer.write(writer)\n\t}}))\n}}\n",
        component.name
    );

    dispatch(&mut body, ir, &routes, &rows, &system, &exported);

    for route in &routes {
        match route.serves {
            Served::Command(handle) => {
                command_handler(
                    &mut body,
                    &emit,
                    layout,
                    component,
                    ir.command(handle),
                    &system,
                );
            }
            Served::View(handle) => {
                view_handler(
                    &mut body,
                    &emit,
                    layout,
                    component,
                    ir.view(handle),
                    &system,
                );
            }
        }
    }

    emit.file_at(
        format!("{}/{}.go", package.dir, module_stem(component)),
        provenance,
        &format!(
            "// The `{}` component of `{}` {}, on the wire.\n//\n// The specification says this \
             component's callers are not deployed with it, so its surface\n// exists on a wire. \
             Which wire is derived rather than chosen: the one contract this model\n// projects \
             for a command surface is the OpenAPI document, and an OpenAPI document is an\n// HTTP \
             contract. The document is beside this file, served verbatim at \
             `/openapi.json`.\n",
            component.name, ir.system, ir.version
        ),
        &body,
    )
}

/// The route match: one arm per path, and one arm for everything else.
fn dispatch(
    body: &mut String,
    ir: &EssIr,
    routes: &[http::Route<'_>],
    rows: &[Row],
    system: &str,
    exported: &str,
) {
    let _ = write!(
        body,
        "\n// dispatch{exported} answers one request.\n//\n// A path this table does not hold is a \
         404 naming where the whole table is published; a path\n// it holds under a different \
         method is a 405 naming the one it answers. Neither is a status\n// the contract declares, \
         and neither should be: both are facts about a transport rather than\n// about any \
         command.\nfunc dispatch{exported}(system *{system}, request *http.Request) response \
         {{\n\tbody, refused := readBody(request)\n\tif refused != nil {{\n\t\treturn \
         *refused\n\t}}\n\tswitch request.URL.Path {{\n"
    );
    for (method, path, _, _) in rows {
        let _ = writeln!(
            body,
            "\tcase {path:?}:\n\t\tif request.Method != {method:?} {{\n\t\t\treturn \
             methodNotAllowed({method:?})\n\t\t}}"
        );
        if path == http::OPENAPI {
            let _ = writeln!(
                body,
                "\t\treturn response{{status: 200, contentType: mediaJSON, body: \
                 openapi{exported}}}"
            );
        } else if path == http::DOCS {
            let _ = writeln!(
                body,
                "\t\treturn response{{status: 200, contentType: mediaMarkdown, body: \
                 docs{exported}}}"
            );
        } else {
            let route = routes
                .iter()
                .find(|route| &route.path == path)
                .expect("every non-document row of the table is a route");
            match route.serves {
                Served::Command(handle) => {
                    let _ = writeln!(
                        body,
                        "\t\treturn serve{}(system, body)",
                        ident(&ir.command(handle).name)
                    );
                }
                Served::View(handle) => {
                    let _ = writeln!(
                        body,
                        "\t\treturn serve{}(system)",
                        ident(&ir.view(handle).name)
                    );
                }
            }
        }
    }
    let _ = write!(
        body,
        "\t}}\n\treturn refusal(404, fmt.Sprintf(\"`%s` is not a path this surface declares; `GET \
         /openapi.json` publishes every one that is\", request.URL.Path))\n}}\n"
    );
}

/// The two documents this surface publishes about itself, embedded from the files beside it.
fn documents(body: &mut String, component: &ResolvedComponent, exported: &str) {
    let _ = write!(
        body,
        "\n// The contract this surface answers and the prose the same model produced, byte for \
         byte as\n// `generated/` commits them. Embedded rather than rebuilt at run time: a server \
         that\n// regenerated its own contract could publish one the repository never \
         reviewed.\n//\n//go:embed {0}.openapi.json\nvar openapi{exported} string\n\n//go:embed \
         {0}.docs.md\nvar docs{exported} string\n",
        component.name
    );
}

/// Every route, as a table the startup record and the reader both read.
fn route_table(body: &mut String, rows: &[Row], exported: &str) {
    let _ = write!(
        body,
        "\n// Routes{exported} is every route this surface answers, in path order.\n//\n// The \
         same set the OpenAPI document declares, plus the two documents about the surface \
         itself,\n// which no specification construct names and nothing can therefore derive. A \
         path absent from\n// this table is answered with 404, including one the document declares \
         and this table forgot.\nvar Routes{exported} = [][2]string{{\n"
    );
    for (method, path, _, _) in rows {
        let _ = writeln!(body, "\t{{{method:?}, {path:?}}},");
    }
    body.push_str("}\n");
}

/// The three startup lines, and the function that closes each with this process's own facts.
fn startup(
    body: &mut String,
    ir: &EssIr,
    plan: &SynthesisPlan,
    component: &ResolvedComponent,
    rows: &[Row],
    served: &[&ResolvedComponent],
    exported: &str,
) {
    let lines = startup_lines(ir, plan, component, rows, served);
    let _ = write!(
        body,
        "\n// Startup{exported} is what this process says about itself as it starts.\n//\n// Three \
         lines of JSON on standard output, in this order, every member of them derived from the\n\
         // specification — except `runtime`, which is appended below and holds what is true of \
         *this\n// process*: the language it was synthesised into, and the address it bound. \
         Everything outside\n// `runtime` is the same in every language this plan is emitted into, \
         and `cargo xtask synth\n// --check` starts both and compares \
         them.\nvar Startup{exported} = []string{{\n"
    );
    for line in &lines {
        let _ = writeln!(body, "\t{line:?},");
    }
    body.push_str("}\n");

    let _ = write!(
        body,
        "\n// announce{exported} writes the startup record, with this process's own facts closing \
         each line.\nfunc announce{exported}(address *net.TCPAddr) {{\n\tfor _, facts := range \
         Startup{exported} {{\n\t\truntime, err := json.Marshal(map[string]any{{\"address\": \
         address.String(), \"language\": \"go\", \"port\": address.Port}})\n\t\tif err != nil \
         {{\n\t\t\tcontinue\n\t\t}}\n\t\t\
         fmt.Printf(\"%s,\\\"runtime\\\":%s}}\\n\", facts, runtime)\n\t}}\n}}\n"
    );
}

/// One row of the surface table: `(method, path, what it serves, the construct's name)`.
type Row = (&'static str, String, &'static str, String);

/// The file one component's surface is emitted into, named after the component.
fn module_stem(component: &ResolvedComponent) -> String {
    name::package_ident(&component.name.to_string())
}

/// The whole surface as rows of `(method, path, what it serves, the construct's name)`.
fn table<'a>(routes: &'a [http::Route<'a>], ir: &'a EssIr) -> Vec<Row> {
    let mut rows: Vec<Row> = vec![
        (
            Method::Get.as_str(),
            http::DOCS.to_owned(),
            "documentation",
            "docs".to_owned(),
        ),
        (
            Method::Get.as_str(),
            http::OPENAPI.to_owned(),
            "contract",
            "openapi".to_owned(),
        ),
    ];
    for route in routes {
        rows.push(match route.serves {
            Served::Command(handle) => (
                route.method.as_str(),
                route.path.clone(),
                "command",
                ir.command(handle).name.to_string(),
            ),
            Served::View(handle) => (
                route.method.as_str(),
                route.path.clone(),
                "view",
                ir.view(handle).name.to_string(),
            ),
        });
    }
    rows.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(right.0)));
    rows
}

/// The three startup lines, as JSON text without the closing brace of each.
///
/// Built by the same code path the Rust emitter uses — [`crate::rust::http::startup_facts`] — so
/// the two languages cannot disagree about a member. What each appends is its own `runtime`.
fn startup_lines(
    ir: &EssIr,
    plan: &SynthesisPlan,
    component: &ResolvedComponent,
    rows: &[Row],
    served: &[&ResolvedComponent],
) -> Vec<String> {
    crate::rust::http::startup_facts(
        ir,
        plan,
        component,
        rows,
        served.len(),
        LOG_FORMAT,
        TRANSPORT,
    )
}

/// One accepted command: body in, declared outcome out, at the status the contract publishes.
fn command_handler(
    out: &mut String,
    emit: &Emit<'_>,
    layout: &Layout,
    component: &ResolvedComponent,
    command: &ResolvedCommand,
    system: &str,
) {
    let function = ident(&command.name);
    let field = name::exported(&component.name.to_string());
    let method = layout.declared(&command.name);
    let _ = write!(
        out,
        "\n// serve{function} answers `POST` `{}`: reads the declared input, runs the port, \
         answers the\n// declared outcome.\nfunc serve{function}(system *{system}, body []byte) \
         response {{\n\tvalue, refused := readJSON(body)\n\tif refused != nil {{\n\t\treturn \
         *refused\n\t}}\n\tinput, err := decodeCommand{function}(value, \"body\")\n\tif err != nil \
         {{\n\t\t// 400 and not 422: this is a body the schema decides, which is the \
         difference\n\t\t// between fixing a value and fixing a serialiser.\n\t\treturn \
         refusal(400, err.Error())\n\t}}\n\toutcome, unmet := \
         system.{field}.{method}(input)\n\tif unmet != nil {{\n\t\treturn refusal(501, \
         unmet.Error())\n\t}}\n\treturn answer{function}(outcome)\n}}\n",
        command.name
    );

    let outcome_type = emit.reference_outcome(&command.name);
    let _ = write!(
        out,
        "\n// answer{function} renders one declared outcome of `{}` as the contract publishes it: \
         the\n// branch that was taken, the declared error where there is one, and that error's \
         own\n// payload.\nfunc answer{function}(outcome {outcome_type}) response {{\n\tbody := \
         map[string]any{{}}\n\tswitch taken := outcome.(type) {{\n",
        command.name
    );
    for outcome in &command.outcomes {
        let _ = writeln!(
            out,
            "\tcase {}:",
            emit.reference_outcome_variant(&command.name, outcome.name.as_str())
        );
        let _ = writeln!(out, "\t\tbody[\"outcome\"] = {:?}", outcome.name.as_str());
        if let Some(handle) = &outcome.error {
            let declared = emit.ir.error(handle);
            let _ = writeln!(out, "\t\tbody[\"error\"] = {:?}", declared.name.to_string());
            if declared.fields.is_empty() {
                out.push_str("\t\t_ = taken\n");
            } else {
                let _ = writeln!(
                    out,
                    "\t\tbody[\"payload\"] = encodeError{}(taken.Error)",
                    ident(&declared.name)
                );
            }
        } else {
            out.push_str("\t\t_ = taken\n");
        }
        let _ = writeln!(out, "\t\treturn rendered({}, body)", http::status(outcome));
    }
    out.push_str(
        "\t}\n\t// Go cannot check that a switch over a sealed interface is total, which is this \
         target's\n\t// standing weakening (see TARGET.md). An outcome no branch above named is a \
         value no\n\t// generated code can construct, and it is reported rather than \
         dropped.\n\treturn refusal(500, \"the port answered an outcome this surface has no \
         branch for\")\n}\n",
    );
}

/// One declared view: the rows the projection holds, under the key the contract declares.
fn view_handler(
    out: &mut String,
    emit: &Emit<'_>,
    layout: &Layout,
    component: &ResolvedComponent,
    view: &ResolvedView,
    system: &str,
) {
    let _ = emit;
    let function = ident(&view.name);
    let field = name::exported(&component.name.to_string());
    let method = layout.declared(&view.name);
    let _ = write!(
        out,
        "\n// serve{function} answers `GET` `{}` at `{}` consistency: every row the owed \
         projection\n// holds.\nfunc serve{function}(system *{system}) response {{\n\trows, unmet \
         := system.{field}.{method}()\n\tif unmet != nil {{\n\t\treturn refusal(501, \
         unmet.Error())\n\t}}\n\tencoded := make([]any, 0, len(rows))\n\tfor _, row := range rows \
         {{\n\t\tencoded = append(encoded, encodeView{function}(row))\n\t}}\n\treturn \
         rendered(200, map[string]any{{\"rows\": encoded}})\n}}\n",
        view.name,
        view.consistency.as_str()
    );
}

/// What the emitted enum and union encoders answer for a value no branch names.
///
/// A `default` arm rather than a statement after the switch, for two reasons that happen to agree:
/// a switch whose every arm returns and which has a default is a *terminating* statement, so Go
/// does not ask for a second return it would then call unreachable; and the binding a union's
/// switch makes is in scope inside the switch and nowhere else.
const UNREACHABLE_VARIANT: &str = "\tdefault:\n\t\t// Go cannot check that a switch over a \
                                   sealed interface is total (see TARGET.md).\n\t\t// A value no \
                                   branch above names is one no generated code can \
                                   construct.\n\t\treturn nil\n\t}\n";

/// The same, for a union, whose switch binds the shape it matched.
const UNREACHABLE_SHAPE: &str = "\tdefault:\n\t\t_ = shape\n\t\t// Go cannot check that a \
                                 switch over a sealed interface is total (see \
                                 TARGET.md).\n\t\t// A shape no branch above names is one no \
                                 generated code can construct.\n\t\treturn nil\n\t}\n";

/// The package's documentation, on the one file that carries the package clause a reader meets
/// first.
const SERVER_DOC: &str = "// Package server is the HTTP surface of every component the \
                          specification says is reached\n// over a network.\n//\n// The codecs \
                          beside this file are generated rather than derived: a generated type \
                          carries\n// an unexported field, which `encoding/json` cannot see, and \
                          exporting it would undo the\n// distinctness the newtype encoding \
                          exists for. What they render is what the published\n// contracts \
                          already fix — bytes as base64, a decimal, timestamp, duration and UUID \
                          as\n// strings, an absent optional member omitted rather than sent as \
                          null.\n";

/// The fixed helpers the generated codecs call.
const WIRE_HELPERS: &str = r#"
// DecodeError is a refusal at one path, with what the declaration says belongs there and what
// arrived instead.
//
// The path is what makes it usable: a caller that sent a nested command input gets the field, not
// "invalid request".
type DecodeError struct {
	// At is where in the document, as a dotted path from its root.
	At string
	// Expected is what the declaration says belongs there.
	Expected string
	// Found is what was there instead.
	Found string
}

// Error renders the refusal.
func (e DecodeError) Error() string {
	return fmt.Sprintf("%s: expected %s, found %s", e.At, e.Expected, e.Found)
}

// describes names what a decoded JSON value is, for a refusal.
func describes(value any) string {
	switch shaped := value.(type) {
	case nil:
		return "null"
	case bool:
		return "a boolean"
	case json.Number:
		return "a number"
	case string:
		return "a string"
	case []any:
		return "an array"
	case map[string]any:
		return "an object"
	default:
		_ = shaped
		return "a value of an unknown shape"
	}
}

// nested is one step further into a document, for a message a reader can follow back.
func nested(at string, step string) string {
	if at == "" {
		return step
	}
	return at + "." + step
}

// indexed is one step into an array.
func indexed(at string, index int) string {
	return fmt.Sprintf("%s[%d]", at, index)
}

// objectAt is the object at this path.
func objectAt(value any, at string, expected string) (map[string]any, error) {
	object, ok := value.(map[string]any)
	if !ok {
		return nil, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return object, nil
}

// itemsAt is the array at this path.
func itemsAt(value any, at string, expected string) ([]any, error) {
	items, ok := value.([]any)
	if !ok {
		return nil, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return items, nil
}

// required is the member a declaration says must be there, and the path it sits at.
func required(value any, at string, name string) (any, string, error) {
	memberAt := nested(at, name)
	object, err := objectAt(value, at, "an object")
	if err != nil {
		return nil, memberAt, err
	}
	member, ok := object[name]
	if !ok {
		// The same sentence the Rust target's reader writes, word for word. Two applications
		// synthesised from one specification and refusing one request differently would be two
		// diagnostics a caller has to learn, and `cargo xtask synth --check` compares the bodies.
		return nil, memberAt, DecodeError{At: memberAt, Expected: "a value", Found: "nothing"}
	}
	return member, memberAt, nil
}

// optional is the member a declaration says may be there. An absent member and a null one are the
// same answer, because the published contract omits an absent optional rather than sending null.
func optional(value any, at string, name string) (any, string, bool, error) {
	memberAt := nested(at, name)
	object, err := objectAt(value, at, "an object")
	if err != nil {
		return nil, memberAt, false, err
	}
	member, ok := object[name]
	if !ok || member == nil {
		return nil, memberAt, false, nil
	}
	return member, memberAt, true, nil
}

// textAt is the string at this path.
func textAt(value any, at string, expected string) (string, error) {
	text, ok := value.(string)
	if !ok {
		return "", DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return text, nil
}

// boolAt is the boolean at this path.
func boolAt(value any, at string, expected string) (bool, error) {
	held, ok := value.(bool)
	if !ok {
		return false, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	return held, nil
}

// integerAt is the whole number at this path.
//
// Read through json.Number, which is why the decoder is configured with UseNumber: the default
// float64 loses whole numbers past 2^53, and an Integer in this model is 64 bits.
func integerAt(value any, at string, expected string) (int64, error) {
	number, ok := value.(json.Number)
	if !ok {
		return 0, DecodeError{At: at, Expected: expected, Found: describes(value)}
	}
	held, err := number.Int64()
	if err != nil {
		return 0, DecodeError{At: at, Expected: expected, Found: fmt.Sprintf("`%s`", number.String())}
	}
	return held, nil
}

// bytesAt is the base64-encoded bytes at this path.
func bytesAt(value any, at string, expected string) ([]byte, error) {
	text, err := textAt(value, at, expected)
	if err != nil {
		return nil, err
	}
	held, decodeErr := base64.StdEncoding.DecodeString(text)
	if decodeErr != nil {
		return nil, DecodeError{At: at, Expected: expected, Found: fmt.Sprintf("`%s`", text)}
	}
	return held, nil
}

// keyBool reads a boolean written as an object key.
func keyBool(key string, at string) (bool, error) {
	held, err := strconv.ParseBool(key)
	if err != nil {
		return false, DecodeError{At: at, Expected: "a key spelling true or false", Found: fmt.Sprintf("`%s`", key)}
	}
	return held, nil
}

// keyInteger reads a whole number written as an object key.
func keyInteger(key string, at string) (int64, error) {
	held, err := strconv.ParseInt(key, 10, 64)
	if err != nil {
		return 0, DecodeError{At: at, Expected: "a key spelling a whole number", Found: fmt.Sprintf("`%s`", key)}
	}
	return held, nil
}

// keyBytes reads base64-encoded bytes written as an object key.
func keyBytes(key string, at string) ([]byte, error) {
	held, err := base64.StdEncoding.DecodeString(key)
	if err != nil {
		return nil, DecodeError{At: at, Expected: "a key spelling base64-encoded bytes", Found: fmt.Sprintf("`%s`", key)}
	}
	return held, nil
}
"#;

/// The fixed helpers one served component's file needs, emitted once per file.
///
/// Once per *file* rather than once per package, because a package holds one file per served
/// component and Go would refuse the second declaration. They are therefore named after nothing —
/// see [`surface_file`], which emits exactly one served component per file, and the emitter refuses
/// more than one served component per package below.
const SURFACE_HELPERS: &str = r#"
// The media type every answer derived from the model carries.
const mediaJSON = "application/json"

// The media type the prose answer carries.
//
// The bytes served are the committed Markdown, unrendered: rendering it to HTML here would be a
// second rendering of the documentation, and the two would differ the first time either moved.
const mediaMarkdown = "text/markdown; charset=utf-8"

// The largest body this surface reads, in bytes.
//
// A caller can claim any length, and a server that allocated whatever it was told to is a server
// anyone can stop by saying a large number. A megabyte is far past any command input this model
// can describe.
const maxBody = 1048576

// response is one answer: a status, a media type and a body.
type response struct {
	status      int
	contentType string
	body        string
}

// write sends the answer and lets the connection close behind it.
//
// Content-Length is set rather than left to the server: without it a body past the write buffer is
// sent with chunked transfer encoding, and the two applications synthesised from one specification
// would then differ on the wire for a reason no reader of the specification could predict. A caller
// that reads to the end of the connection gets the same bytes from both.
func (r response) write(writer http.ResponseWriter) {
	writer.Header().Set("Content-Type", r.contentType)
	writer.Header().Set("Content-Length", strconv.Itoa(len(r.body)))
	writer.Header().Set("Connection", "close")
	writer.WriteHeader(r.status)
	_, _ = writer.Write([]byte(r.body))
}

// refusal is an answer this surface makes rather than the specification.
//
// A malformed request, a path nothing declares, a method a path does not answer, an obligation
// nothing has satisfied. None of these is a declared outcome and none is published in the
// contract, because each is a fact about a transport rather than about a command. The body is
// JSON with one member: a caller that has just failed to satisfy a contract should not have to
// parse a second format to read why.
func refusal(status int, detail string) response {
	return rendered(status, map[string]any{"refused": detail})
}

// methodNotAllowed is the answer for a path this surface holds under a different method.
func methodNotAllowed(allowed string) response {
	return refusal(405, fmt.Sprintf("this path answers `%s`, and the contract declares no other method for it", allowed))
}

// rendered is one answer whose body is a value this package built.
func rendered(status int, body any) response {
	encoded, err := json.Marshal(body)
	if err != nil {
		return response{status: 500, contentType: mediaJSON, body: `{"refused":"the answer could not be encoded"}`}
	}
	return response{status: status, contentType: mediaJSON, body: string(encoded)}
}

// readBody reads at most maxBody bytes of a request, or the refusal that says why it could not.
func readBody(request *http.Request) ([]byte, *response) {
	if request.Body == nil {
		return nil, nil
	}
	defer func() { _ = request.Body.Close() }()
	body, err := io.ReadAll(io.LimitReader(request.Body, maxBody+1))
	if err != nil {
		answer := refusal(400, fmt.Sprintf("the body could not be read: %s", err))
		return nil, &answer
	}
	if len(body) > maxBody {
		answer := refusal(413, fmt.Sprintf("the body is longer than %d bytes, which is all this surface reads", maxBody))
		return nil, &answer
	}
	return body, nil
}

// readJSON parses a request body, or the refusal that says why it is not JSON.
//
// UseNumber, so an Integer past 2^53 survives the crossing: the default reads every number as a
// float64, and a visit id or a count would come back changed.
func readJSON(body []byte) (any, *response) {
	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.UseNumber()
	var value any
	if err := decoder.Decode(&value); err != nil {
		answer := refusal(400, fmt.Sprintf("the body is not JSON: %s", err))
		return nil, &answer
	}
	return value, nil
}
"#;
