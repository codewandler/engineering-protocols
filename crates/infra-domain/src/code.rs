//! Refusal codes and the accumulator they arrive in.
//!
//! The same discipline as the protocol half (`aep-domain`) and the specification half
//! (`ess-domain`): every refusal carries a stable machine-readable code, tests and harnesses match
//! on the code and never on message text, and validation accumulates — a bundle with four defects
//! reports four refusals in one run.
//!
//! The codes live in their own `INFRA-` namespace because an observation bundle is neither a
//! protocol document nor a specification: a harness that sees `INFRA-SECRET-001` knows which tool
//! to re-run without parsing a sentence.

use std::fmt;

/// Declares every infrastructure refusal code once.
///
/// The wire string and the [`InfraCode::ALL`] list are generated from the same line as the
/// variant — the `validation_codes!` idiom from `aep-domain`, adopted here for the reason it exists
/// there: a hand-maintained list beside a hand-maintained enum is two lists, and two lists drift.
macro_rules! infra_codes {
    ($( $(#[$attribute:meta])* $variant:ident => $wire:literal, )*) => {
        /// Stable machine-readable classification of an observation-bundle refusal.
        ///
        /// Codes are part of the public interface: harnesses and tests match on them rather than
        /// on message text.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(into = "String")]
        #[non_exhaustive]
        pub enum InfraCode {
            $( $(#[$attribute])* $variant, )*
        }

        impl InfraCode {
            /// Every code this build can produce, in declaration order.
            ///
            /// Generated, so it cannot fall behind the enum.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// The code as it appears in output, such as `INFRA-SECRET-001`.
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )*
                }
            }
        }
    };
}

infra_codes! {
    /// The bundle's `format` is not `infra-observation/1`.
    UnsupportedFormat => "INFRA-BUNDLE-001",

    /// One of the twelve observed kinds is absent from `kinds`.
    ///
    /// Absent, not empty: an empty list is an observation ("there are no ingresses"), a missing
    /// key is a scan that did not look, and an IR built from one would silently claim the cluster
    /// has none of something nobody checked.
    MissingKind => "INFRA-BUNDLE-002",

    /// An item in a kind's list cannot be read as an object of that kind.
    MalformedObject => "INFRA-OBJECT-001",

    /// An object is missing its name, namespace or uid — the identity everything downstream
    /// keys on.
    MissingIdentity => "INFRA-OBJECT-002",

    /// Two objects of the same kind share a namespace and name.
    ///
    /// A live API server cannot serve this, so a bundle that carries it was assembled or edited
    /// by hand — and an IR keyed by identity would silently drop one of the two.
    DuplicateIdentity => "INFRA-OBJECT-003",

    /// A secret's data value is a plain string rather than a `{sha256, length}` digest.
    ///
    /// The hard rule, enforced twice by design: the scanner sanitizes before writing, and this
    /// model refuses what an unsanitized bundle would carry — so a secret value cannot enter the
    /// IR even through a bundle the scanner never touched. The refusal names the key and never
    /// echoes the value.
    UnsanitizedSecret => "INFRA-SECRET-001",

    /// A secret's data value is an object but not a well-formed digest: `sha256` is not 64
    /// lowercase hex characters, or `length` is not a non-negative integer.
    MalformedSecretDigest => "INFRA-SECRET-002",

    /// A labels or selector map carries a value that is not a string.
    NonStringSelector => "INFRA-SELECTOR-001",

    /// A workload's pod template declares no containers.
    EmptyWorkload => "INFRA-WORKLOAD-001",

    /// A container is missing its name or image.
    MissingContainerField => "INFRA-WORKLOAD-002",

    /// An ingress path's backend names no service.
    IncompleteBackend => "INFRA-INGRESS-001",
}

impl fmt::Display for InfraCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<InfraCode> for String {
    fn from(code: InfraCode) -> Self {
        code.as_str().to_owned()
    }
}

/// One refusal: what rule broke, where in the bundle, and what a reader should know.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValidationError {
    /// The stable code a harness matches on.
    pub code: InfraCode,
    /// Where in the bundle, in dotted form: `kinds.secrets.items[3].data.password`.
    pub location: String,
    /// What is wrong, for a human. Never carries a secret value.
    pub message: String,
}

impl ValidationError {
    /// Builds one.
    pub fn new(code: InfraCode, location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            location: location.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.code, self.location, self.message)
    }
}

/// Every refusal found in one pass, in the order the bundle presents the defects.
///
/// Validation pushes into this and keeps going; returning on the first failure is how a bundle
/// with forty defects costs forty runs to fix.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ValidationErrors(Vec<ValidationError>);

impl ValidationErrors {
    /// None yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one.
    pub fn push(&mut self, error: ValidationError) {
        self.0.push(error);
    }

    /// Records one from its parts.
    pub fn refuse(
        &mut self,
        code: InfraCode,
        location: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push(ValidationError::new(code, location, message));
    }

    /// Every refusal.
    pub fn as_slice(&self) -> &[ValidationError] {
        &self.0
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` when there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `true` when at least one refusal carries this code.
    pub fn contains(&self, code: InfraCode) -> bool {
        self.0.iter().any(|error| error.code == code)
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_renders_in_the_infra_namespace_and_the_generated_list_holds_them_all() {
        assert_eq!(InfraCode::ALL.len(), 11, "the catalogue is eleven codes");
        for code in InfraCode::ALL {
            assert!(
                code.as_str().starts_with("INFRA-"),
                "{code:?} renders as {} — outside the INFRA- namespace",
                code.as_str()
            );
        }
    }

    #[test]
    fn wire_strings_are_unique_because_two_rules_sharing_one_code_are_indistinguishable_downstream()
    {
        let mut seen = std::collections::BTreeSet::new();
        for code in InfraCode::ALL {
            assert!(
                seen.insert(code.as_str()),
                "{} is declared for two variants",
                code.as_str()
            );
        }
    }

    #[test]
    fn errors_accumulate_rather_than_replace() {
        let mut errors = ValidationErrors::new();
        errors.refuse(InfraCode::MissingIdentity, "kinds.pods.items[0]", "no name");
        errors.refuse(InfraCode::MissingIdentity, "kinds.pods.items[1]", "no uid");
        assert_eq!(errors.len(), 2, "both refusals are kept");
        assert!(errors.contains(InfraCode::MissingIdentity));
    }
}
