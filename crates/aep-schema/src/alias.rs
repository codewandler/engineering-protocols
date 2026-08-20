//! Spellings the parsers accept beside the canonical ones, published in the schema as well.
//!
//! `AGENTS.md` records that wire-format aliases are deliberate: both spellings appear in the design
//! documents, and a document written to the other one still has to parse. `serde` implements that
//! with `#[serde(alias = "…")]` — and **`schemars` cannot see that attribute**, so the derived
//! schema publishes the canonical spelling and nothing else.
//!
//! That gap is not cosmetic. `docs/guide/specification.md` tells an author to point their editor at
//! `schemas/generated/ess.schema.json`; every one of these types also carries
//! `#[serde(deny_unknown_fields)]`, so the missing spelling is not a lenient omission but a
//! refusal. The editor marked this repository's own normative example invalid, in the exact place
//! the guide's examples put it, and offered no repair — because the spelling it objected to was the
//! spelling the guide writes.
//!
//! # An alias is not simply a second optional property
//!
//! Where the canonical field is **required**, exactly one of the two spellings must be present. A
//! schema that made both optional would accept a component that names itself neither way, which the
//! parser refuses. So the required entry is replaced by a `oneOf` over the two one-element
//! `required` lists: false for a document giving both, false for one giving neither, true for one
//! giving either.
//!
//! Where the canonical field is **optional**, `serde` reads the two spellings into one field, so a
//! document giving both is a duplicate key rather than a merge. That is a `not` over a `required`
//! naming both.
//!
//! # Why a list and a pass, rather than a hand-written `JsonSchema`
//!
//! Hand-writing the impl is what this repository reached for the first two times
//! ([`CapabilityPolicy`](aep_domain::capability::CapabilityPolicy),
//! [`EvidenceKind`](aep_domain::evidence::EvidenceKind)), and it is one more place to forget: the
//! attribute and the schema are written in different files by different people, and nothing relates
//! them. Neither end of this one is trusted instead. `crates/aep-schema/tests/published.rs` scans
//! every crate's sources for `#[serde(alias = …)]` and fails — naming the file, the type and the
//! spelling — when one sits on a type a published schema carries and that schema does not publish
//! it. It checks the list the other way too, so nothing here can publish a spelling no parser
//! accepts. Adding an alias without publishing it therefore breaks the gate rather than the next
//! author's editor.
//!
//! [`CapabilityPolicy`](aep_domain::capability::CapabilityPolicy) is deliberately **not** in
//! [`WIRE_ALIASES`]: it publishes `require_approval` from its own hand-written impl, which predates
//! this pass. The test checks the published schema rather than this list, so either mechanism
//! satisfies it, and the pass leaves a spelling that is already there alone.

use schemars::schema::{RootSchema, Schema, SchemaObject};
use serde_json::Value;

/// One spelling a parser accepts beside the canonical one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WireAlias {
    /// The Rust type carrying the `#[serde(alias = …)]`, as the schema names it.
    pub type_name: &'static str,
    /// The canonical spelling: a field name, or the tag value of an internally tagged variant.
    pub canonical: &'static str,
    /// The other spelling a document may write.
    pub alias: &'static str,
}

/// One entry, written as a call so the list below reads as a table.
const fn alias(type_name: &'static str, canonical: &'static str, alias: &'static str) -> WireAlias {
    WireAlias {
        type_name,
        canonical,
        alias,
    }
}

/// Every alias this pass publishes.
///
/// In type order, then canonical order, so a reader can find one and a diff stays small. Each is
/// checked against the `#[serde(alias = …)]` it describes by `crates/aep-schema/tests/published.rs`
/// — in both directions, so an entry here that no parser accepts fails the gate too.
pub const WIRE_ALIASES: &[WireAlias] = &[
    alias("Evidence", "diff", "source_diff"),
    alias("Evidence", "test_result", "test_execution"),
    alias("RawBindingSpec", "name", "id"),
    alias("RawComponentSpec", "name", "component"),
    alias("RawObligation", "requires", "require"),
    alias("RawPrinciple", "applies_when", "applicability"),
    alias("RawPrinciple", "on_failure", "failure_policy"),
    alias("RawProfile", "without_principles", "remove_principles"),
    alias("RawProtocol", "approval_floor", "approval_required"),
    alias("RawProtocol", "default_failure_policy", "on_failure"),
    alias("RawState", "on_failure", "failure_policy"),
    alias("RawState", "requires", "require"),
    alias("RawTask", "kind", "type"),
    alias("RawTask", "manifest", "artifact_manifest"),
    alias("RawTask", "principle_overrides", "principles"),
    alias("RawTransition", "on_failure", "failure_policy"),
    alias("RawTransition", "requires", "require"),
];

/// Publishes every alias whose type this schema carries.
///
/// Applied to each generated schema after `schema_for!`, because a type appears in several of them:
/// `CapabilityPolicy` is reachable from four documents, and an alias published in one and not the
/// next is the same defect one level down.
pub(crate) fn publish(root: &mut RootSchema) {
    for entry in WIRE_ALIASES {
        if title_of(&root.schema) == Some(entry.type_name) {
            publish_into(&mut root.schema, entry);
        }
        if let Some(Schema::Object(target)) = root.definitions.get_mut(entry.type_name) {
            publish_into(target, entry);
        }
    }
}

/// The title a schema was generated under, which is the Rust type's name.
fn title_of(schema: &SchemaObject) -> Option<&str> {
    schema
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.title.as_deref())
}

/// Publishes one alias, as a field spelling or as a variant tag value.
///
/// Which of the two it is comes from the schema rather than from the list, because the schema is
/// what has to end up right: a field alias names a property, a variant alias names a tag value, and
/// a list entry that matched neither would be a silent no-op. `publishes_alias` in the test file is
/// what refuses that.
fn publish_into(target: &mut SchemaObject, entry: &WireAlias) {
    if !publish_field(target, entry) {
        publish_variant(target, entry);
    }
}

/// Publishes an alias for a field, and returns whether the type had such a field.
fn publish_field(target: &mut SchemaObject, entry: &WireAlias) -> bool {
    let Some(canonical) = target.object().properties.get(entry.canonical).cloned() else {
        return false;
    };
    // A type that publishes the spelling from its own `JsonSchema` impl is left alone, so the two
    // mechanisms cannot produce one property twice or one constraint twice.
    if target.object().properties.contains_key(entry.alias) {
        return true;
    }

    let mut described = canonical.into_object();
    described.metadata().description = Some(format!(
        "An accepted spelling of `{}`; give one or the other, not both.",
        entry.canonical
    ));
    target
        .object()
        .properties
        .insert(entry.alias.to_owned(), described.into());

    let was_required = target.object().required.remove(entry.canonical);
    let constraint = if was_required {
        exactly_one(entry)
    } else {
        at_most_one(entry)
    };
    target
        .subschemas()
        .all_of
        .get_or_insert_with(Vec::new)
        .push(constraint);
    true
}

/// Publishes an alias for a variant of an internally tagged enum, and returns whether it found one.
///
/// The variant is found by its tag value rather than by position: `Evidence` is a `oneOf` whose
/// members are indistinguishable except for the one required property holding a single-valued
/// `enum`, which is exactly what the tag is.
fn publish_variant(target: &mut SchemaObject, entry: &WireAlias) -> bool {
    let Some(variants) = target
        .subschemas
        .as_mut()
        .and_then(|subschemas| subschemas.one_of.as_mut())
    else {
        return false;
    };

    for variant in variants {
        let Schema::Object(variant) = variant else {
            continue;
        };
        let required = variant.object().required.clone();
        for (name, property) in &mut variant.object().properties {
            if !required.contains(name) {
                continue;
            }
            let Schema::Object(property) = property else {
                continue;
            };
            let Some(values) = property.enum_values.as_mut() else {
                continue;
            };
            if values.len() != 1 || values[0] != Value::String(entry.canonical.to_owned()) {
                continue;
            }
            values.push(Value::String(entry.alias.to_owned()));
            return true;
        }
    }
    false
}

/// `required` naming exactly `names`, and nothing else.
fn requiring(names: &[&str]) -> Schema {
    let mut schema = SchemaObject::default();
    for name in names {
        schema.object().required.insert((*name).to_owned());
    }
    schema.into()
}

/// Exactly one of the two spellings, for a field the parser requires.
///
/// Both is a duplicate key; neither leaves the value unset, which is what the missing `required`
/// entry would otherwise permit.
fn exactly_one(entry: &WireAlias) -> Schema {
    let mut schema = SchemaObject::default();
    schema.metadata().description = Some(format!(
        "Exactly one of `{}` and `{}` is required.",
        entry.canonical, entry.alias
    ));
    schema.subschemas().one_of = Some(vec![
        requiring(&[entry.canonical]),
        requiring(&[entry.alias]),
    ]);
    schema.into()
}

/// At most one of the two spellings, for a field the parser defaults.
fn at_most_one(entry: &WireAlias) -> Schema {
    let mut schema = SchemaObject::default();
    schema.metadata().description = Some(format!(
        "`{}` and `{}` are one field; give one or the other, not both.",
        entry.canonical, entry.alias
    ));
    schema.subschemas().not = Some(Box::new(requiring(&[entry.canonical, entry.alias])));
    schema.into()
}

#[cfg(test)]
mod tests {
    use super::{WireAlias, WIRE_ALIASES};

    #[test]
    fn no_alias_is_listed_twice_and_none_renames_a_spelling_to_itself() {
        // Two entries for one pair would apply the constraint twice, which turns `oneOf` into a
        // contradiction no document can satisfy; an entry aliasing a spelling to itself would
        // remove the `required` entry and put nothing back.
        let mut seen = std::collections::BTreeSet::new();
        for entry in WIRE_ALIASES {
            assert_ne!(
                entry.canonical, entry.alias,
                "{} aliases `{}` to itself",
                entry.type_name, entry.canonical
            );
            assert!(
                seen.insert((entry.type_name, entry.alias)),
                "{} publishes `{}` twice",
                entry.type_name,
                entry.alias
            );
        }
    }

    #[test]
    fn the_list_is_sorted_so_a_new_alias_lands_where_a_reader_looks_for_it() {
        let sorted: Vec<WireAlias> = {
            let mut copy = WIRE_ALIASES.to_vec();
            copy.sort_unstable();
            copy
        };
        assert_eq!(
            sorted,
            WIRE_ALIASES.to_vec(),
            "WIRE_ALIASES is out of order; sort by type, then canonical spelling"
        );
    }
}
