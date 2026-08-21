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
//!
//! # One registry for the whole infrastructure family
//!
//! The registry is wider than this crate's own documents, deliberately, and it already was:
//! `INFRA-IR-001`…`004` refuse a *persisted IR document*, which `infra-compiler` reads and this
//! crate never sees. `INFRA-SPEC-001`…`010` join them for the *desired-state specification*
//! `infra-spec` reads. One enum, one `ALL`, one accumulator across the family — because the
//! alternative is three parallel code registries and three parallel `ValidationErrors`, and a
//! consumer that has to know which one a refusal came from before it can print it.

use std::fmt;

/// Declares every infrastructure refusal code once.
///
/// The wire string and the [`InfraCode::ALL`] list are generated from the same line as the
/// variant — the `validation_codes!` idiom from `aep-domain`, adopted here for the reason it exists
/// there: a hand-maintained list beside a hand-maintained enum is two lists, and two lists drift.
macro_rules! infra_codes {
    ($( $(#[$attribute:meta])* $variant:ident => $wire:literal, )*) => {
        /// Stable machine-readable classification of an infrastructure-document refusal.
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

    /// A persisted IR document's `format` is not `infra-ir/1`.
    IrUnsupportedFormat => "INFRA-IR-001",

    /// A persisted IR document's digest does not match its model — the document was edited
    /// after it was compiled.
    IrDigestMismatch => "INFRA-IR-002",

    /// A persisted IR document does not read as the `infra-ir/1` shape.
    IrMalformed => "INFRA-IR-003",

    /// A persisted IR document claims a reference is resolved, but the key it names is absent
    /// from the map that should hold it.
    ///
    /// No compilation produces this: a handle is minted only against the model it points into.
    /// A document carrying one was assembled by hand, and reading it as-is would turn a total
    /// lookup into a panic.
    IrDanglingHandle => "INFRA-IR-004",

    /// A desired-state specification's `format` is not `infra-spec/1`.
    SpecUnsupportedFormat => "INFRA-SPEC-001",

    /// A desired-state specification does not read as the `infra-spec/1` shape at all.
    ///
    /// The one refusal in this family that cannot accumulate with others: a document that does
    /// not deserialize has no expectations to go on and check.
    SpecMalformed => "INFRA-SPEC-002",

    /// Two expectations share an id, and an id is how a report names a verdict.
    SpecDuplicateExpectation => "INFRA-SPEC-003",

    /// A specification declares no expectations.
    ///
    /// A simulation of nothing is not a passing simulation; it is a report with no content, and
    /// reading one as green is the failure mode this refuses.
    SpecEmptyExpectations => "INFRA-SPEC-004",

    /// An expectation's own parameters cannot decide anything: an empty registry allowlist, a
    /// `min` above its `max`, an empty selector, a blank name, an unknown workload kind.
    SpecInvalidExpectation => "INFRA-SPEC-005",

    /// A scope that cannot select what this expectation is about — a workload-label selector on
    /// an expectation about services, or anything but cluster scope on one that names its own
    /// subject.
    SpecScopeNotApplicable => "INFRA-SPEC-006",

    /// An expectation id is not a stable identifier: lowercase letters, digits and dashes.
    SpecMalformedId => "INFRA-SPEC-007",

    /// A predicate expectation reads a fact the workload projection does not produce.
    ///
    /// The escape hatch's dangling-reference rule. A predicate over `workload.replica` instead of
    /// `workload.replicas` would otherwise evaluate `Unknown` forever and read as "the snapshot
    /// cannot decide", which is a lie about a typo.
    SpecUnknownFact => "INFRA-SPEC-008",

    /// An expectation carries a remedy its kind can never write.
    ///
    /// A remedy is the value a projection puts into a field the expectation found empty, so it
    /// belongs only to the kinds that *name* such a field. A `resources:` remedy beside
    /// `image_tag_not_latest` is data nothing will ever read, and carrying it silently would let
    /// a specification claim a projection it does not get.
    SpecRemedyNotApplicable => "INFRA-SPEC-009",

    /// A remedy's own parameters cannot be written: it states nothing, it states a half the
    /// expectation never asks for, or a port that is a quoted number.
    SpecInvalidRemedy => "INFRA-SPEC-010",
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

/// One refusal: what rule broke, where in the document, and what a reader should know.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValidationError {
    /// The stable code a harness matches on.
    pub code: InfraCode,
    /// Where in the document, in dotted form: `kinds.secrets.items[3].data.password`, or
    /// `expectations[2].scope` in a desired-state specification.
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

/// Every refusal found in one pass, in the order the document presents the defects.
///
/// Validation pushes into this and keeps going; returning on the first failure is how a document
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

    /// `value` when nothing was refused, every refusal otherwise.
    ///
    /// The `aep-domain` idiom, and it exists for the same reason: a validator that builds its
    /// result and then decides is a validator whose "did anything break" test is written once,
    /// rather than once per `TryFrom`.
    pub fn into_result<T>(self, value: T) -> Result<T, Self> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self)
        }
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
        assert_eq!(
            InfraCode::ALL.len(),
            25,
            "the catalogue is twenty-five codes: eleven observation refusals, four IR-document \
             ones and ten desired-state-specification ones"
        );
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
