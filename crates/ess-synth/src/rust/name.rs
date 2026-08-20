//! From specification names to Rust names, deterministically.
//!
//! Every rule here is a pure function of its input, because two artifacts derived from one
//! declaration have to agree on what it is called — the plan names an item, the generated source
//! declares it, and a test greps for it. A naming decision made at a call site instead of here is a
//! decision three places make three ways.
//!
//! The specification's own patterns do most of the work: a type segment is already `PascalCase`
//! ([`StateName::PATTERN`](ess_domain::entity::StateName::PATTERN) and the type-name convention),
//! a field matches `^[A-Za-z][A-Za-z0-9_]*$`, and an outcome is kebab-case. What is left for this
//! module is joining, case conversion, and the one hazard the specification cannot see: a name that
//! is legal there and a keyword here.

/// A Rust type name for a declaration, relative to the bounded context that owns it.
///
/// The segments after the domain namespace, each pascal-cased, joined: `billing.invoice.Money` in
/// `billing.invoice` is `Money`, and the synthesised `billing.invoice.Invoice.State` is
/// `InvoiceState` — one identifier, because Rust has no dotted type names and a nested module per
/// entity would put the state enum somewhere no other projection files it.
pub fn type_name(name: &ess_domain::name::QualifiedName, domain_segments: usize) -> String {
    let mut out = String::new();
    for segment in name.segments().iter().skip(domain_segments) {
        out.push_str(&pascal(segment));
    }
    out
}

/// A Rust variant name for a specification word: `wrong-state` is `WrongState`, `person` is
/// `Person`, `Draft` stays `Draft`.
pub fn pascal(word: &str) -> String {
    let mut out = String::new();
    let mut boundary = true;
    for character in word.chars() {
        if character == '-' || character == '_' {
            boundary = true;
        } else if boundary {
            out.extend(character.to_uppercase());
            boundary = false;
        } else {
            out.push(character);
        }
    }
    out
}

/// A Rust type-name fragment for an arbitrary specification spelling — `ledger.core.AccountId`
/// is `LedgerCoreAccountId`, `Optional<String>` is `OptionalString`.
///
/// The full spelling, not the last segment, because the fragments are joined into identifiers
/// (a conversion obligation's trait is named from both of its ends) and two spellings that differ
/// anywhere must not produce one identifier.
pub fn type_fragment(text: &str) -> String {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(pascal)
        .collect()
}

/// A Rust value identifier for a specification word: lower snake case, keyword-escaped.
///
/// Fields, methods and modules all take this path, so `invoice_id` stays itself, a transition named
/// `IssueInvoice` becomes `issue_invoice`, and a field a specification may legally call `type`
/// becomes `r#type` rather than a compile error in an artifact nobody hand-edits.
pub fn value_ident(word: &str) -> String {
    escape(snake(word))
}

/// Lower snake case: an underscore before each interior upper-case letter, hyphens as underscores.
fn snake(word: &str) -> String {
    let mut out = String::new();
    for character in word.chars() {
        if character == '-' {
            out.push('_');
        } else if character.is_uppercase() {
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

/// Escapes a Rust keyword so it can be used as an identifier.
///
/// Raw identifiers (`r#type`) cover every keyword except the four path keywords, which cannot be
/// raw; those get a trailing underscore instead, which changes the spelling and is therefore the
/// last resort rather than the rule.
fn escape(ident: String) -> String {
    if PATH_KEYWORDS.contains(&ident.as_str()) {
        return format!("{ident}_");
    }
    if KEYWORDS.contains(&ident.as_str()) {
        return format!("r#{ident}");
    }
    ident
}

/// Keywords a raw identifier cannot spell.
const PATH_KEYWORDS: &[&str] = &["crate", "self", "super"];

/// Every other reserved word of the editions this crate can emit for.
///
/// The 2015/2018/2021 strict and reserved sets. Being over-inclusive is safe — escaping a word that
/// did not need it costs an `r#` — while missing one is generated code that does not compile.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];

#[cfg(test)]
mod tests {
    use ess_domain::name::QualifiedName;

    use super::{pascal, type_name, value_ident};

    #[test]
    fn a_nested_declaration_becomes_one_identifier() {
        // The synthesised state enum is the case that matters: `billing.invoice.Invoice.State` has
        // no Rust spelling with the dot in it, and `State` alone would collide the moment a second
        // entity exists in the module.
        let name = QualifiedName::new("billing.invoice.Invoice.State").expect("a legal name");
        assert_eq!(type_name(&name, 2), "InvoiceState");
    }

    #[test]
    fn a_kebab_case_outcome_becomes_a_variant() {
        assert_eq!(pascal("wrong-state"), "WrongState");
        assert_eq!(pascal("person"), "Person");
        assert_eq!(pascal("Draft"), "Draft");
    }

    #[test]
    fn a_field_the_specification_may_call_type_is_escaped_rather_than_broken() {
        // `type` matches the specification's field pattern, so refusing it is not this crate's
        // call; emitting it bare would be a generated file that does not compile.
        assert_eq!(value_ident("type"), "r#type");
        assert_eq!(value_ident("self"), "self_");
        assert_eq!(value_ident("invoice_id"), "invoice_id");
    }

    #[test]
    fn a_pascal_case_transition_name_becomes_a_method() {
        // `entity.rs` documents transition names like `IssueInvoice`; a method spelled that way is
        // a warning in the generated crate and a style no reader expects.
        assert_eq!(value_ident("IssueInvoice"), "issue_invoice");
        assert_eq!(value_ident("issue"), "issue");
    }
}
