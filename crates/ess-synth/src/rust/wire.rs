//! The wire module: every generated type, as JSON, in both directions.
//!
//! # Why this is generated and not reflected
//!
//! The types the Rust target emits derive nothing — no `Serialize`, no `Deserialize` — on purpose:
//! a semantic type knows nothing about a transport (design §9), and the moment one carries a serde
//! attribute the wire format has moved into the domain. So the crossing is emitted *beside* them,
//! from the same model, and it renders what the published contracts already fix: `Bytes` as base64,
//! `Decimal`, `Timestamp`, `Duration` and `Uuid` as strings, a tagged union adjacently tagged with
//! its payload under the key [`ess_gen::schema::union_content_key`] decides, an absent optional
//! field omitted rather than sent as `null`. Two projections of one model must not disagree about
//! what a value looks like, so neither of them decides alone.
//!
//! # Both directions, and what each is for
//!
//! A decoder exists for what enters the system — a command's input, and every type it reaches. An
//! encoder exists for what leaves it: events on the log, the error a refusal carries, a view's
//! rows. Every generated named type gets both, because a type can be reached from either side and
//! a partial pair is a hole the next specification falls into.

use std::fmt::Write as _;

use ess_compiler::ir::{
    ResolvedBody, ResolvedCommand, ResolvedError, ResolvedEvent, ResolvedField, ResolvedTypeRef,
    ResolvedView,
};
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;

use crate::rust::items::outcome_event_fields;
use crate::rust::layout::Layout as RustLayout;
use crate::rust::{name, Emit};

use super::Bridge;

/// A place expression, parenthesised when it needs to be.
///
/// The generated encoders walk into a value by dereferencing a binding, so an expression can be
/// `*held`; `*held.iter()` parses as `*(held.iter())`, which is a different program. One helper
/// rather than parentheses everywhere, because the noise would be in every emitted line.
fn place(expr: &str) -> String {
    if expr.starts_with('*') {
        format!("({expr})")
    } else {
        expr.to_owned()
    }
}

/// The function-name fragment of a declaration: its whole qualified name, snake-cased.
///
/// The whole name and never the local one: `billing.invoice.Email` and `billing.email.Email` are
/// two declarations, and a wire function that named only the last segment would silently be one.
pub(super) fn ident(name: &QualifiedName) -> String {
    name::value_ident(&name::type_fragment(&name.to_string()))
}

/// The emitted `wire` module.
pub(super) fn module(bridge: &Bridge<'_>) -> String {
    let mut out = String::new();
    out.push_str(HEADER);

    for declared in bridge.ir.types.values() {
        if !bridge.presents_type(&declared.name) {
            continue;
        }
        named_type(&mut out, bridge, declared);
    }
    for event in bridge.ir.events.values() {
        if bridge.presents_event(&event.name) {
            event_encoder(&mut out, bridge, event);
        }
    }
    for error in bridge.ir.errors.values() {
        if bridge.presents_error(&error.name) {
            error_encoder(&mut out, bridge, error);
        }
    }
    for view in bridge.ir.views.values() {
        if bridge.presents_view(&view.name) {
            view_encoder(&mut out, bridge, view);
        }
    }
    for command in bridge.ir.commands.values() {
        if bridge.presents_command(&command.name) {
            command_encoder(&mut out, bridge, command);
            command_decoder(&mut out, bridge, command);
            outcome_encoder(&mut out, bridge, command);
        }
    }
    out
}

/// The `wire` module's own documentation.
const HEADER: &str = "//! Every generated declaration, as JSON, in the renderings the published \
                      wire contracts fix.\n//!\n//! Generated from the model beside the types it \
                      crosses, so a field renamed in the specification\n//! is renamed here in the \
                      same regeneration. An absent optional field is omitted rather\n//! than sent \
                      as `null`, which is what the `required` list of the published schema \
                      says.\n\nuse crate::json;\n";

// ---- named types ------------------------------------------------------------------------------

/// One declared type's pair of wire functions.
fn named_type(out: &mut String, bridge: &Bridge<'_>, declared: &ess_compiler::ir::ResolvedType) {
    type_encoder(out, bridge, declared);
    type_decoder(out, bridge, declared);
}

/// One declared type, written.
fn type_encoder(out: &mut String, bridge: &Bridge<'_>, declared: &ess_compiler::ir::ResolvedType) {
    let path = bridge.path(&declared.name);
    let ident = ident(&declared.name);
    let name = &declared.name;

    let _ = write!(
        out,
        "\n/// Writes `{name}` as JSON.\npub fn encode_{ident}(value: &{path}, out: &mut String) \
         {{\n"
    );
    match &declared.body {
        ResolvedBody::Newtype { of, .. } => encode_value(out, "    ", "value.0", of, 0),
        ResolvedBody::Struct { fields, .. } => {
            out.push_str("    out.push('{');\n");
            for field in fields {
                encode_member(
                    out,
                    "    ",
                    ess_gen::schema::wire_field_name(field),
                    &format!("value.{}", name::value_ident(&field.name)),
                    &field.type_ref,
                    0,
                );
            }
            out.push_str("    out.push('}');\n");
        }
        ResolvedBody::Enum { variants } => {
            out.push_str("    match value {\n");
            for variant in variants {
                let _ = writeln!(
                    out,
                    "        {path}::{} => json::push_text(out, {variant:?}),",
                    name::pascal(variant)
                );
            }
            out.push_str("    }\n");
        }
        ResolvedBody::Union { tag, variants } => {
            let content = ess_gen::schema::union_content_key(tag);
            out.push_str("    match value {\n");
            for (label, payload) in variants {
                let _ = writeln!(
                    out,
                    "        {path}::{}(held) => {{\n            out.push('{{');\n            \
                     json::member(out, {tag:?});\n            json::push_text(out, {label:?});",
                    name::pascal(label)
                );
                encode_member(out, "            ", content, "*held", payload, 0);
                out.push_str("            out.push('}');\n        }\n");
            }
            out.push_str("    }\n");
        }
    }
    out.push_str("}\n");
}

/// One declared type, read.
fn type_decoder(out: &mut String, bridge: &Bridge<'_>, declared: &ess_compiler::ir::ResolvedType) {
    let path = bridge.path(&declared.name);
    let ident = ident(&declared.name);
    let name = &declared.name;

    let _ = write!(
        out,
        "\n/// Reads `{name}` from JSON.\n///\n/// # Errors\n///\n/// [`json::DecodeError`] naming \
         the path and what the declaration says belongs \
         there.\npub fn decode_{ident}(value: &json::Value, at: &str) -> Result<{path}, \
         json::DecodeError> {{\n"
    );
    match &declared.body {
        ResolvedBody::Newtype { of, .. } => {
            let _ = writeln!(
                out,
                "    Ok({path}({}))",
                decode_value(bridge, "value", "at", of, 0)
            );
        }
        ResolvedBody::Struct { fields, .. } => {
            let _ = writeln!(out, "    Ok({path} {{");
            for (position, field) in fields.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "        {}: {},",
                    name::value_ident(&field.name),
                    decode_member(bridge, field, position)
                );
            }
            out.push_str("    })\n");
        }
        ResolvedBody::Enum { variants } => {
            let expected = variant_list(variants);
            let _ = writeln!(
                out,
                "    Ok(match json::text_at(value, at, {expected:?})? {{"
            );
            for variant in variants {
                let _ = writeln!(
                    out,
                    "        {variant:?} => {path}::{},",
                    name::pascal(variant)
                );
            }
            let _ = writeln!(
                out,
                "        other => return Err(json::DecodeError {{ at: at.to_owned(), expected: \
                 {expected:?}.to_owned(), found: format!(\"`{{other}}`\") }}),\n    }})"
            );
        }
        ResolvedBody::Union { tag, variants } => {
            let content = ess_gen::schema::union_content_key(tag);
            let expected = variant_list(&variants.keys().cloned().collect::<Vec<_>>());
            let _ = writeln!(
                out,
                "    let tag = json::member_at(value, at, {tag:?})?;\n    let at_tag = \
                 json::nested(at, {tag:?});\n    Ok(match json::text_at(tag, &at_tag, \
                 {expected:?})? {{"
            );
            for (position, (label, payload)) in variants.iter().enumerate() {
                let carried = ResolvedField {
                    name: content.to_owned(),
                    type_ref: payload.clone(),
                    naming: ess_domain::name::Naming::default(),
                };
                let _ = writeln!(
                    out,
                    "        {label:?} => {path}::{}({}),",
                    name::pascal(label),
                    decode_member(bridge, &carried, position)
                );
            }
            let _ = writeln!(
                out,
                "        other => return Err(json::DecodeError {{ at: at_tag.clone(), expected: \
                 {expected:?}.to_owned(), found: format!(\"`{{other}}`\") }}),\n    }})"
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

// ---- declarations that are records of fields ----------------------------------------------------

/// One event's encoder.
fn event_encoder(out: &mut String, bridge: &Bridge<'_>, event: &ResolvedEvent) {
    record_encoder(
        out,
        &format!("encode_event_{}", ident(&event.name)),
        &bridge.path(&event.name),
        &format!("the event `{}`", event.name),
        &event.fields,
    );
}

/// One declared error's encoder.
fn error_encoder(out: &mut String, bridge: &Bridge<'_>, error: &ResolvedError) {
    record_encoder(
        out,
        &format!("encode_error_{}", ident(&error.name)),
        &bridge.path(&error.name),
        &format!("the declared error `{}`", error.name),
        &error.fields,
    );
}

/// One view row's encoder.
fn view_encoder(out: &mut String, bridge: &Bridge<'_>, view: &ResolvedView) {
    record_encoder(
        out,
        &format!("encode_view_{}", ident(&view.name)),
        &bridge.path(&view.name),
        &format!("one row of the view `{}`", view.name),
        &view.fields,
    );
}

/// An encoder over a fixed list of fields.
fn record_encoder(
    out: &mut String,
    function: &str,
    path: &str,
    describes: &str,
    fields: &[ResolvedField],
) {
    let _ = write!(
        out,
        "\n/// Writes {describes} as JSON.\npub fn {function}(value: &{path}, out: &mut String) \
         {{\n    out.push('{{');\n"
    );
    for field in fields {
        encode_member(
            out,
            "    ",
            ess_gen::schema::wire_field_name(field),
            &format!("value.{}", name::value_ident(&field.name)),
            &field.type_ref,
            0,
        );
    }
    out.push_str("    out.push('}');\n}\n");
    if fields.is_empty() {
        // A declaration with no fields is a unit struct in the Rust target, so the binding is
        // never read. Named `_value` there rather than silenced with an attribute, because a
        // generated file that opens with `allow` teaches the next reader the wrong habit.
        let unit = format!("pub fn {function}(value: &{path}, out: &mut String) {{");
        *out = out.replace(
            &unit,
            &format!("pub fn {function}(_value: &{path}, out: &mut String) {{"),
        );
    }
}

// ---- commands ------------------------------------------------------------------------------------

/// One command input's encoder, for the record the transport keeps of what a binding passed.
fn command_encoder(out: &mut String, bridge: &Bridge<'_>, command: &ResolvedCommand) {
    record_encoder(
        out,
        &format!("encode_command_{}", ident(&command.name)),
        &bridge.path(&command.name),
        &format!("the input of `{}`", command.name),
        &command.input,
    );
}

/// One command input's decoder.
fn command_decoder(out: &mut String, bridge: &Bridge<'_>, command: &ResolvedCommand) {
    let path = bridge.path(&command.name);
    let _ = write!(
        out,
        "\n/// Reads the input of `{}` from JSON.\n///\n/// # Errors\n///\n/// \
         [`json::DecodeError`] naming the path and what the declaration says belongs \
         there.\npub fn decode_command_{}(value: &json::Value, at: &str) -> Result<{path}, \
         json::DecodeError> {{\n    Ok({path} {{\n",
        command.name,
        ident(&command.name)
    );
    for (position, field) in command.input.iter().enumerate() {
        let _ = writeln!(
            out,
            "        {}: {},",
            name::value_ident(&field.name),
            decode_member(bridge, field, position)
        );
    }
    out.push_str("    })\n}\n");
    if command.input.is_empty() {
        let unit = format!(
            "pub fn decode_command_{}(value: &json::Value, at: &str)",
            ident(&command.name)
        );
        *out = out.replace(
            &unit,
            &format!(
                "pub fn decode_command_{}(_value: &json::Value, _at: &str)",
                ident(&command.name)
            ),
        );
    }
}

/// One command outcome's encoder: which branch was taken, what it published, what it refused with.
///
/// The refusal is rendered beside the success and never below it, which is design §8 reaching the
/// page: a browser that can only show the happy path is a consumer that handles only the happy
/// path.
fn outcome_encoder(out: &mut String, bridge: &Bridge<'_>, command: &ResolvedCommand) {
    let path = bridge.path(&command.name);
    let domain = bridge.ir.domain(&command.domain).name.clone();
    let emit = Emit {
        ir: bridge.ir,
        layout: bridge.layout.rust(),
        domain: &domain,
    };
    let _ = write!(
        out,
        "\n/// Writes the outcome of `{}` as JSON: the branch taken, what it published, and the \
         declared\n/// refusal it carries where it carries one.\npub fn \
         encode_outcome_{}(value: &{path}Outcome, out: &mut String) {{\n    out.push('{{');\n    \
         match value {{\n",
        command.name,
        ident(&command.name)
    );
    for outcome in &command.outcomes {
        let variant = name::pascal(outcome.name.as_str());
        let carried = outcome_event_fields(&emit, outcome);
        let mut bindings: Vec<String> = carried.iter().map(|field| field.field.clone()).collect();
        if outcome.error.is_some() {
            bindings.push("error".to_owned());
        }
        let pattern = if bindings.is_empty() {
            format!("{path}Outcome::{variant}")
        } else {
            format!("{path}Outcome::{variant} {{ {} }}", bindings.join(", "))
        };
        let _ = writeln!(out, "        {pattern} => {{");
        let _ = writeln!(
            out,
            "            json::member(out, \"outcome\");\n            json::push_text(out, \
             {:?});",
            outcome.name.as_str()
        );
        out.push_str("            json::member(out, \"published\");\n            out.push('[');\n");
        for (position, field) in carried.iter().enumerate() {
            if position > 0 {
                out.push_str("            out.push(',');\n");
            }
            let _ = writeln!(
                out,
                "            out.push('{{');\n            json::member(out, \"event\");\n            \
                 json::push_text(out, {:?});\n            json::member(out, \
                 \"payload\");\n            encode_event_{}({}, out);\n            \
                 out.push('}}');",
                field.event.name().to_string(),
                ident(field.event.name()),
                field.field
            );
        }
        out.push_str("            out.push(']');\n");
        if let Some(error) = &outcome.error {
            let _ = writeln!(
                out,
                "            json::member(out, \"refusal\");\n            out.push('{{');\n            \
                 json::member(out, \"error\");\n            json::push_text(out, \
                 {:?});\n            json::member(out, \"payload\");\n            \
                 encode_error_{}(error, out);\n            out.push('}}');",
                error.name().to_string(),
                ident(error.name())
            );
        }
        out.push_str("        }\n");
    }
    out.push_str("    }\n    out.push('}');\n}\n");
}

// ---- the two walkers ------------------------------------------------------------------------------

/// One member of an object being written: the separator, the key, and the value — or nothing at
/// all when an optional value is absent.
fn encode_member(
    out: &mut String,
    indent: &str,
    wire: &str,
    expr: &str,
    type_ref: &ResolvedTypeRef,
    depth: usize,
) {
    if let ResolvedTypeRef::Optional { of } = type_ref {
        let held = format!("held{depth}");
        let _ = writeln!(
            out,
            "{indent}if let Some({held}) = &{expr} {{\n{indent}    json::member(out, {wire:?});"
        );
        encode_value(
            out,
            &format!("{indent}    "),
            &format!("*{held}"),
            of,
            depth + 1,
        );
        let _ = writeln!(out, "{indent}}}");
        return;
    }
    let _ = writeln!(out, "{indent}json::member(out, {wire:?});");
    encode_value(out, indent, expr, type_ref, depth);
}

/// The statements that write one value of a declared type as JSON.
fn encode_value(
    out: &mut String,
    indent: &str,
    expr: &str,
    type_ref: &ResolvedTypeRef,
    depth: usize,
) {
    match type_ref {
        ResolvedTypeRef::Primitive { name } => {
            let _ = writeln!(out, "{indent}{}", encode_primitive(*name, expr));
        }
        ResolvedTypeRef::Declared { name } => {
            let _ = writeln!(out, "{indent}encode_{}(&{expr}, out);", ident(name.name()));
        }
        ResolvedTypeRef::Optional { of } => {
            let held = format!("held{depth}");
            let _ = writeln!(
                out,
                "{indent}match &{expr} {{\n{indent}    Some({held}) => {{"
            );
            encode_value(
                out,
                &format!("{indent}        "),
                &format!("*{held}"),
                of,
                depth + 1,
            );
            let _ = writeln!(
                out,
                "{indent}    }}\n{indent}    None => out.push_str(\"null\"),\n{indent}}}"
            );
        }
        ResolvedTypeRef::List { of } => {
            let index = format!("index{depth}");
            let item = format!("item{depth}");
            let _ = writeln!(
                out,
                "{indent}out.push('[');\n{indent}for ({index}, {item}) in {}.iter().enumerate() \
                 {{\n{indent}    if {index} > 0 {{\n{indent}        \
                 out.push(',');\n{indent}    }}",
                place(expr)
            );
            encode_value(
                out,
                &format!("{indent}    "),
                &format!("*{item}"),
                of,
                depth + 1,
            );
            let _ = writeln!(out, "{indent}}}\n{indent}out.push(']');");
        }
        ResolvedTypeRef::Map { key, value } => {
            let index = format!("index{depth}");
            let entry = format!("key{depth}");
            let item = format!("item{depth}");
            let _ = writeln!(
                out,
                "{indent}out.push('{{');\n{indent}for ({index}, ({entry}, {item})) in \
                 {}.iter().enumerate() {{\n{indent}    if {index} > 0 {{\n{indent}        \
                 out.push(',');\n{indent}    }}\n{indent}    {}\n{indent}    out.push(':');",
                place(expr),
                encode_key(*key, &entry)
            );
            encode_value(
                out,
                &format!("{indent}    "),
                &format!("*{item}"),
                value,
                depth + 1,
            );
            let _ = writeln!(out, "{indent}}}\n{indent}out.push('}}');");
        }
    }
}

/// One primitive, written.
fn encode_primitive(primitive: Primitive, expr: &str) -> String {
    match primitive {
        Primitive::String => format!("json::push_text(out, &{expr});"),
        Primitive::Boolean => format!("json::push_bool(out, {expr});"),
        Primitive::Integer => format!("json::push_integer(out, {expr});"),
        Primitive::Bytes => format!("json::push_base64(out, &{expr});"),
        Primitive::Decimal | Primitive::Timestamp | Primitive::Duration | Primitive::Uuid => {
            format!("json::push_text(out, &{}.0);", place(expr))
        }
    }
}

/// One map key, written — always as a string, because JSON has no other kind of key.
fn encode_key(primitive: Primitive, expr: &str) -> String {
    match primitive {
        Primitive::String => format!("json::push_text(out, {expr});"),
        Primitive::Boolean => {
            format!("json::push_text(out, if *{expr} {{ \"true\" }} else {{ \"false\" }});")
        }
        Primitive::Integer => format!("json::push_text(out, &{expr}.to_string());"),
        Primitive::Bytes => format!("json::push_base64(out, {expr});"),
        Primitive::Decimal | Primitive::Timestamp | Primitive::Duration | Primitive::Uuid => {
            format!("json::push_text(out, &{expr}.0);")
        }
    }
}

/// One field of an object being read, absence included where the declaration permits it.
fn decode_member(bridge: &Bridge<'_>, field: &ResolvedField, position: usize) -> String {
    let wire = ess_gen::schema::wire_field_name(field);
    let at = format!("at{position}");
    let member = format!("member{position}");
    if let ResolvedTypeRef::Optional { of } = &field.type_ref {
        // Absent and `null` both mean absent: the published contract omits an absent optional
        // field, and a page that sends the key with `null` is saying the same thing.
        return format!(
            "match value.member({wire:?}) {{\n            None | Some(json::Value::Null) => \
             None,\n            Some({member}) => {{\n                let {at} = json::nested(at, \
             {wire:?});\n                Some({})\n            }}\n        }}",
            decode_value(bridge, &member, &format!("&{at}"), of, position)
        );
    }
    format!(
        "{{\n            let {at} = json::nested(at, {wire:?});\n            let {member} = \
         json::member_at(value, at, {wire:?})?;\n            {}\n        }}",
        decode_value(
            bridge,
            &member,
            &format!("&{at}"),
            &field.type_ref,
            position
        )
    )
}

/// The expression that reads one value of a declared type from JSON.
fn decode_value(
    bridge: &Bridge<'_>,
    value: &str,
    at: &str,
    type_ref: &ResolvedTypeRef,
    depth: usize,
) -> String {
    match type_ref {
        ResolvedTypeRef::Primitive { name } => decode_primitive(bridge, *name, value, at),
        ResolvedTypeRef::Declared { name } => {
            format!("decode_{}({value}, {at})?", ident(name.name()))
        }
        ResolvedTypeRef::Optional { of } => format!(
            "if matches!({value}, json::Value::Null) {{ None }} else {{ Some({}) }}",
            decode_value(bridge, value, at, of, depth + 1)
        ),
        ResolvedTypeRef::List { of } => {
            let items = format!("items{depth}");
            let index = format!("index{depth}");
            let element = format!("element{depth}");
            let nested = format!("nested{depth}");
            format!(
                "{{\n                let mut {items} = Vec::new();\n                for \
                 ({index}, {element}) in json::items_at({value}, {at}, \
                 \"an array\")?.iter().enumerate() {{\n                    let {nested} = \
                 json::nested({at}, &{index}.to_string());\n                    \
                 {items}.push({});\n                }}\n                {items}\n            }}",
                decode_value(bridge, &element, &format!("&{nested}"), of, depth + 1)
            )
        }
        ResolvedTypeRef::Map { key, value: held } => {
            let entries = format!("entries{depth}");
            let entry = format!("key{depth}");
            let element = format!("element{depth}");
            let nested = format!("nested{depth}");
            format!(
                "{{\n                let mut {entries} = std::collections::BTreeMap::new();\n                \
                 for ({entry}, {element}) in json::members_at({value}, {at}, \"an \
                 object\")? {{\n                    let {nested} = json::nested({at}, \
                 {entry});\n                    {entries}.insert({}, {});\n                \
                 }}\n                {entries}\n            }}",
                decode_key(bridge, *key, &entry, &nested),
                decode_value(bridge, &element, &format!("&{nested}"), held, depth + 1)
            )
        }
    }
}

/// One primitive, read.
fn decode_primitive(bridge: &Bridge<'_>, primitive: Primitive, value: &str, at: &str) -> String {
    let primitives = format!("{}::primitives", bridge.types);
    match primitive {
        Primitive::String => format!("json::text_at({value}, {at}, \"a string\")?.to_owned()"),
        Primitive::Boolean => format!("json::bool_at({value}, {at}, \"a boolean\")?"),
        Primitive::Integer => format!("json::integer_at({value}, {at}, \"an integer\")?"),
        Primitive::Bytes => {
            format!("json::bytes_at({value}, {at}, \"base64-encoded bytes\")?")
        }
        Primitive::Decimal => format!(
            "{primitives}::Decimal(json::text_at({value}, {at}, \"a decimal string\")?.to_owned())"
        ),
        Primitive::Timestamp => format!(
            "{primitives}::Timestamp(json::text_at({value}, {at}, \"an RFC 3339 \
             instant\")?.to_owned())"
        ),
        Primitive::Duration => format!(
            "{primitives}::Duration(json::text_at({value}, {at}, \"an ISO 8601 \
             duration\")?.to_owned())"
        ),
        Primitive::Uuid => {
            format!("{primitives}::Uuid(json::text_at({value}, {at}, \"a UUID\")?.to_owned())")
        }
    }
}

/// One map key, read back out of the string JSON always spells it as.
fn decode_key(bridge: &Bridge<'_>, primitive: Primitive, key: &str, at: &str) -> String {
    let primitives = format!("{}::primitives", bridge.types);
    match primitive {
        Primitive::String => format!("{key}.clone()"),
        Primitive::Boolean => format!("json::key_bool({key}, {at})?"),
        Primitive::Integer => format!("json::key_integer({key}, {at})?"),
        Primitive::Bytes => format!("json::key_bytes({key}, {at})?"),
        Primitive::Decimal => format!("{primitives}::Decimal({key}.clone())"),
        Primitive::Timestamp => format!("{primitives}::Timestamp({key}.clone())"),
        Primitive::Duration => format!("{primitives}::Duration({key}.clone())"),
        Primitive::Uuid => format!("{primitives}::Uuid({key}.clone())"),
    }
}

/// A declaration's path from inside the bridge crate.
pub(super) fn path(layout: &RustLayout, types: &str, declared: &QualifiedName) -> String {
    crate::rust::port::types_path(layout, types, declared)
}
