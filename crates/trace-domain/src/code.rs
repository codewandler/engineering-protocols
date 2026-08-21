//! Refusal codes and the accumulator they arrive in.
//!
//! The same discipline as the protocol half (`aep-domain`), the specification half (`ess-domain`)
//! and the infrastructure half (`infra-domain`): every refusal carries a stable machine-readable
//! code, tests and harnesses match on the code and never on message text, and validation
//! accumulates — a specification with four broken expectations reports four refusals in one run
//! (invariant 3).
//!
//! The codes live in their own `TRACE-` namespace because an agent-run transcript is neither a
//! protocol document, nor a system specification, nor a cluster: a harness that sees
//! `TRACE-SPEC-004` knows which document to open without parsing a sentence.
//!
//! # One registry for the whole trace family
//!
//! `TRACE-SPEC-*` refuses an authored `trace-spec/1` document, which this crate validates.
//! `TRACE-ADAPT-*` refuses a *transcript* the adapter cannot read at all, which `trace-spec`
//! produces and this crate never sees. One enum, one [`TraceCode::ALL`], one accumulator across
//! the family — the `infra-domain` arrangement, adopted for the reason it is stated there: the
//! alternative is two parallel registries and a consumer that has to know which one a refusal
//! came from before it can print it.
//!
//! Note what is deliberately **not** here. An event shape the adapter does not recognise is not a
//! refusal: it becomes an opaque record in the IR and turns the expectations that depend on it
//! into `unk` (design § 2.9, D1). `TRACE-ADAPT-*` is reserved for a file that is not a transcript
//! at all — bytes that are not UTF-8, or a line that is not JSON.

use std::fmt;

/// Declares every trace refusal code once.
///
/// The wire string and the [`TraceCode::ALL`] list are generated from the same line as the
/// variant — the `validation_codes!` idiom from `aep-domain`, adopted here for the reason it
/// exists there: a hand-maintained list beside a hand-maintained enum is two lists, and two lists
/// drift.
macro_rules! trace_codes {
    ($( $(#[$attribute:meta])* $variant:ident => $wire:literal, )*) => {
        /// Stable machine-readable classification of a trace-document refusal.
        ///
        /// Codes are part of the public interface: harnesses and tests match on them rather than
        /// on message text.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(into = "String")]
        #[non_exhaustive]
        pub enum TraceCode {
            $( $(#[$attribute])* $variant, )*
        }

        impl TraceCode {
            /// Every code this build can produce, in declaration order.
            ///
            /// Generated, so it cannot fall behind the enum.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// The code as it appears in output, such as `TRACE-SPEC-001`.
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )*
                }
            }
        }
    };
}

trace_codes! {
    /// The specification's `format` is not `trace-spec/1`.
    SpecUnsupportedFormat => "TRACE-SPEC-001",

    /// The specification does not read as the `trace-spec/1` shape at all.
    ///
    /// The one refusal in this family that cannot accumulate with others: a document that does
    /// not deserialize has no expectations to go on and check.
    SpecMalformed => "TRACE-SPEC-002",

    /// Two expectations share an id, and an id is how a report names a verdict.
    SpecDuplicateExpectation => "TRACE-SPEC-003",

    /// The specification declares no expectations.
    ///
    /// A check of nothing is not a passing check; it is a report with no content, and reading one
    /// as green is the failure mode this refuses.
    SpecEmptyExpectations => "TRACE-SPEC-004",

    /// An expectation's own parameters cannot decide anything: an empty plugin set, a bound with
    /// no side, a `min` above its `max`, a blank name, a result matcher over no field.
    SpecInvalidExpectation => "TRACE-SPEC-005",

    /// An expectation id, or the specification's own id, is not a stable identifier.
    ///
    /// Lowercase letters, digits, dashes, and — for the document id only — one `/` separating a
    /// namespace from a name.
    SpecMalformedId => "TRACE-SPEC-006",

    /// A bound states a floor above its ceiling, or combines `exactly` with either.
    ///
    /// Its own code rather than [`SpecInvalidExpectation`](TraceCode::SpecInvalidExpectation):
    /// a bound is written by hand in every third expectation, and a reader who mistyped one wants
    /// to be told that it is the bound, not the kind.
    SpecUnsatisfiableBound => "TRACE-SPEC-007",

    /// A matcher this build does not implement was asked for by name.
    ///
    /// Reserved for `regex`, which the design's § 3.4 lists and this build refuses rather than
    /// silently reinterprets: the workspace carries no regular-expression engine and
    /// `AGENTS.md` § *Dependencies* says to record the refusal instead of adding one. The message
    /// names `glob` as what to write instead. Refusing by name is the point — a `regex:` key
    /// quietly read as `contains:` would be a specification that means something other than what
    /// it says.
    SpecUnsupportedMatcher => "TRACE-SPEC-008",

    /// A transcript's bytes are not UTF-8, or one of its lines is not JSON.
    ///
    /// Not an unrecognised *event*: that is an opaque record and an `unk` verdict, never a
    /// refusal (design § 2.9). This is a file that is not a transcript.
    AdapterMalformedTranscript => "TRACE-ADAPT-001",

    /// A transcript holds no events at all.
    ///
    /// An empty file judged against a specification would report every expectation as `unk`,
    /// which is true and useless; the honest answer is that there is nothing to judge.
    AdapterEmptyTranscript => "TRACE-ADAPT-002",
}

impl fmt::Display for TraceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<TraceCode> for String {
    fn from(code: TraceCode) -> Self {
        code.as_str().to_owned()
    }
}

/// One refusal: what rule broke, where in the document, and what a reader should know.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValidationError {
    /// The stable code a harness matches on.
    pub code: TraceCode,
    /// Where in the document, in dotted form: `expectations[2].count`.
    pub location: String,
    /// What is wrong, for a human.
    pub message: String,
}

impl ValidationError {
    /// Builds one.
    pub fn new(code: TraceCode, location: impl Into<String>, message: impl Into<String>) -> Self {
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
        code: TraceCode,
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
    pub fn contains(&self, code: TraceCode) -> bool {
        self.0.iter().any(|error| error.code == code)
    }

    /// How many refusals carry this code.
    ///
    /// Tests assert an exact count per code rather than "is an error", which is invariant 3's
    /// enforcement: a validator that returned on the first defect would report one here.
    pub fn count(&self, code: TraceCode) -> usize {
        self.0.iter().filter(|error| error.code == code).count()
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
    fn every_code_renders_in_the_trace_namespace_and_the_generated_list_holds_them_all() {
        assert_eq!(
            TraceCode::ALL.len(),
            10,
            "the catalogue is ten codes: eight specification refusals and two transcript ones"
        );
        for code in TraceCode::ALL {
            assert!(
                code.as_str().starts_with("TRACE-"),
                "{code:?} renders as {} — outside the TRACE- namespace",
                code.as_str()
            );
        }
    }

    #[test]
    fn wire_strings_are_unique_because_two_rules_sharing_one_code_are_indistinguishable_downstream()
    {
        let mut seen = std::collections::BTreeSet::new();
        for code in TraceCode::ALL {
            assert!(
                seen.insert(code.as_str()),
                "{} is declared for two variants",
                code.as_str()
            );
        }
    }

    #[test]
    fn errors_accumulate_rather_than_replace_and_are_counted_per_code() {
        let mut errors = ValidationErrors::new();
        errors.refuse(TraceCode::SpecMalformedId, "expectations[0].id", "blank");
        errors.refuse(
            TraceCode::SpecMalformedId,
            "expectations[1].id",
            "Uppercase",
        );
        errors.refuse(TraceCode::SpecEmptyExpectations, "expectations", "none");
        assert_eq!(errors.len(), 3, "all three refusals are kept");
        assert_eq!(errors.count(TraceCode::SpecMalformedId), 2);
        assert!(errors.contains(TraceCode::SpecEmptyExpectations));
    }
}
