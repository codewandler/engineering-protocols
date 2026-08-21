//! From specification names to Go names, deterministically.
//!
//! The same rule the Rust emitter's [`name`](crate::rust) module states, for a different language:
//! two artifacts derived from one declaration have to agree on what it is called, so every rule
//! here is a pure function of its input and nothing decides a name at a call site.
//!
//! Go's own rules do the work Rust's `r#` escape does there, and they cut the other way:
//!
//! * **Export is spelling.** An identifier is visible outside its package exactly when it starts
//!   with an upper-case letter, so "public" and `PascalCase` are the same decision here. Every
//!   name a consumer touches is [`exported`]; the marker methods that seal an interface are the
//!   deliberate opposite ([`marker`]).
//! * **Keywords cannot collide with an exported name**, because every Go keyword and every
//!   predeclared identifier is lower-case. Only a *package* name can collide, and
//!   [`package_ident`] is where that is repaired.

/// Every Go keyword, plus the predeclared identifiers a package name must not shadow.
///
/// Keywords are a hard error; the predeclared identifiers are legal to shadow and catastrophic to
/// shadow — a package named `string` makes the type `string` unspellable in every file that
/// imports it. Both are repaired the same way, because "legal but unusable" is not a distinction
/// worth carrying.
const RESERVED: &[&str] = &[
    // Keywords.
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
    // Predeclared types and constants.
    "any",
    "bool",
    "byte",
    "comparable",
    "complex64",
    "complex128",
    "error",
    "false",
    "float32",
    "float64",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "iota",
    "nil",
    "rune",
    "string",
    "true",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    // Predeclared functions.
    "append",
    "cap",
    "clear",
    "close",
    "complex",
    "copy",
    "delete",
    "imag",
    "len",
    "make",
    "max",
    "min",
    "new",
    "panic",
    "print",
    "println",
    "real",
    "recover",
];

/// A Go type name for a declaration, relative to the bounded context that owns it.
///
/// The same rule the Rust emitter uses, because it is a rule about the *specification*: the
/// segments after the domain namespace, each pascal-cased, joined. `billing.invoice.Money` in
/// `billing.invoice` is `Money`, and the synthesised `billing.invoice.Invoice.State` is
/// `InvoiceState`.
pub fn type_name(name: &ess_domain::name::QualifiedName, domain_segments: usize) -> String {
    let mut out = String::new();
    for segment in name.segments().iter().skip(domain_segments) {
        out.push_str(&pascal(segment));
    }
    out
}

/// An exported Go identifier for a specification word: `wrong-state` is `WrongState`,
/// `customer_email` is `CustomerEmail`, `Draft` stays `Draft`.
///
/// The same function as [`pascal`], named for what it is *for*: in Go, pascal-casing a name is
/// what makes it visible outside its package, so a field or method that is spelled this way is
/// spelled this way deliberately.
pub fn exported(word: &str) -> String {
    pascal(word)
}

/// Pascal case: upper-case at every hyphen, underscore and word start, everything else kept.
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

/// A Go type-name fragment for an arbitrary specification spelling — `billing.invoice.Email` is
/// `BillingInvoiceEmail`.
///
/// The full spelling, not the last segment, for the reason the Rust emitter's fragment rule gives:
/// fragments are joined into identifiers, and two spellings that differ anywhere must not produce
/// one identifier.
pub fn type_fragment(text: &str) -> String {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(pascal)
        .collect()
}

/// The unexported method that seals an interface over its variants: `Payee` is sealed by
/// `isPayee`.
///
/// Unexported on purpose, and the whole encoding rests on it: a method whose name starts with a
/// lower-case letter cannot be implemented from another package, so the set of types satisfying
/// the interface is closed at the package boundary — which is Go's only way to say what Rust says
/// with `enum`.
pub fn marker(type_name: &str) -> String {
    format!("is{type_name}")
}

/// A Go package identifier for a specification word: lower case, letters and digits only.
///
/// Hyphens and underscores are dropped rather than kept, because a package name is also the
/// qualifier every reference to it is spelled with (`emailservice.EmailService`), and the
/// convention every Go reader expects there is one lower-case word. A word that would shadow a
/// keyword or a predeclared identifier gets a trailing underscore — legal, visibly repaired, and
/// rare enough that the alternative (a silently unusable `string` package) is not worth trading
/// for.
pub fn package_ident(word: &str) -> String {
    let mut out = String::new();
    for character in word.chars() {
        if character.is_alphanumeric() {
            out.extend(character.to_lowercase());
        }
    }
    if RESERVED.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

#[cfg(test)]
mod tests {
    use ess_domain::name::QualifiedName;

    use super::{marker, package_ident, type_fragment, type_name};

    #[test]
    fn a_nested_declaration_becomes_one_identifier() {
        // The synthesised state enum, as in Rust: `billing.invoice.Invoice.State` has no Go
        // spelling with the dot in it.
        let name = QualifiedName::new("billing.invoice.Invoice.State").expect("a legal name");
        assert_eq!(type_name(&name, 2), "InvoiceState");
    }

    #[test]
    fn a_package_name_that_would_shadow_a_predeclared_identifier_is_repaired() {
        // A domain whose last segment is `string` would otherwise make the type `string`
        // unspellable in every file importing it — legal Go, and unusable.
        assert_eq!(package_ident("string"), "string_");
        assert_eq!(package_ident("map"), "map_");
        assert_eq!(package_ident("email-service"), "emailservice");
        assert_eq!(package_ident("Invoice"), "invoice");
    }

    #[test]
    fn a_marker_method_is_unexported_which_is_what_seals_the_interface() {
        let sealed = marker("Payee");
        assert_eq!(sealed, "isPayee");
        assert!(
            sealed.chars().next().is_some_and(char::is_lowercase),
            "an exported marker method could be implemented from any package, which is not a \
             closed set at all"
        );
    }

    #[test]
    fn a_fragment_keeps_every_segment_because_identifiers_are_joined_from_them() {
        assert_eq!(
            type_fragment("billing.invoice.Email"),
            "BillingInvoiceEmail"
        );
    }
}
