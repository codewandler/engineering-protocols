//! What deciding a guard against a candidate can conclude.
//!
//! Three cases, because evaluation has three values and only one of them means "try another
//! candidate". See the crate documentation for why the third one must refuse rather than retry.

use std::fmt;

use aep_domain::facts::FactPath;
use aep_domain::predicate::{Predicate, PredicateOutcome};
use ess_compiler::ir::{ResolvedCondition, ResolvedOutcome};

/// The `when` predicate an outcome is taken by, when its condition is one.
///
/// `None` for the other three conditions, and the three are `None` for different reasons that a
/// caller must not merge. [`Otherwise`](ResolvedCondition::Otherwise) *is* decided by the input, but
/// only relative to every other branch of the same command, so deciding it needs the command rather
/// than one predicate. [`External`](ResolvedCondition::External) is decided outside the input, so no
/// candidate reaches it and a test has to inject the cause instead.
/// [`WrongState`](ResolvedCondition::WrongState) is decided by the subject the command arrives at,
/// so a test reaches it by arranging that subject and sends the input that would otherwise have
/// worked.
pub fn when(outcome: &ResolvedOutcome) -> Option<&Predicate> {
    match &outcome.condition {
        ResolvedCondition::When { predicate } => Some(predicate),
        ResolvedCondition::Otherwise
        | ResolvedCondition::External { .. }
        | ResolvedCondition::WrongState => None,
    }
}

/// What a candidate input does to a guard.
///
/// Deliberately not a `bool` and not an `Option<bool>`. Both spellings have exactly one place to put
/// `Unknown`, and wherever it is put it becomes indistinguishable from one of the other two — which
/// is the collapse invariant 5 forbids, arriving as an ordinary-looking type signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The guard holds. This candidate reaches the branch.
    Satisfied,
    /// The guard does not hold, and the values are why.
    ///
    /// This is the only case that means *try another candidate*: the predicate was decided, and it
    /// was decided against this input. Carries
    /// [`Predicate::outcome`](aep_domain::predicate::Predicate::outcome)'s explanation, so a runner
    /// reporting a shrunk counterexample already has the failing leaves.
    Refuted(PredicateOutcome),
    /// The guard could not be decided at all, and no retry will change that.
    ///
    /// Five of the six [reasons](Reason) are properties of the specification rather than of the
    /// candidate. Treating this as [`Refuted`](Decision::Refuted) is what turns a specification
    /// defect into a budget of wasted candidates and a test report nobody can act on.
    Unevaluable(Unevaluable),
}

impl Decision {
    /// `true` only for [`Decision::Satisfied`]; what reaching a branch requires.
    ///
    /// The mirror of [`Truth::is_satisfied`](aep_domain::predicate::Truth::is_satisfied), and named
    /// after it: there is no `as_bool` here either, because the two false-ish cases mean opposite
    /// things to the caller.
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Self::Satisfied)
    }

    /// The refutation, when the guard was decided and did not hold.
    pub fn refutation(&self) -> Option<&PredicateOutcome> {
        match self {
            Self::Refuted(outcome) => Some(outcome),
            Self::Satisfied | Self::Unevaluable(_) => None,
        }
    }

    /// The refusal, when the guard could not be decided.
    pub fn unevaluable(&self) -> Option<&Unevaluable> {
        match self {
            Self::Unevaluable(refusal) => Some(refusal),
            Self::Satisfied | Self::Refuted(_) => None,
        }
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Satisfied => f.write_str("satisfied"),
            Self::Refuted(outcome) => write!(f, "refuted: `{}` is false", outcome.expression),
            Self::Unevaluable(refusal) => write!(f, "{refusal}"),
        }
    }
}

/// A guard that could not be decided, and every leaf that is why.
///
/// Never empty: a predicate that evaluates to `Unknown` has at least one leaf that does, and the
/// walk that finds them ends at [`Reason::Unclassified`] rather than at an empty list, so a refusal
/// always says something. A diagnostic that names a predicate and no reason is a diagnostic that
/// sends its reader back to the evaluator's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unevaluable {
    /// The whole guard, as it reads.
    pub predicate: String,
    /// The command whose input the guard was read against.
    pub command: String,
    /// Every leaf that could not be decided, in the order the predicate declares them.
    pub causes: Vec<UnknownCause>,
}

impl Unevaluable {
    /// `true` when *every* cause would go away given a different candidate input.
    ///
    /// Still a refusal either way — a runner is not licensed to retry on an `Unknown`, because the
    /// budget it would spend is unbounded and the answer it would report is wrong. What this says is
    /// which of the two repairs to suggest: supply the absent field, or fix the specification.
    pub fn fixable_by_another_candidate(&self) -> bool {
        self.causes
            .iter()
            .all(|cause| cause.reason.fixable_by_another_candidate())
    }
}

impl fmt::Display for Unevaluable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` cannot be decided against an input of `{}`",
            self.predicate, self.command
        )?;
        for cause in &self.causes {
            write!(f, "\n  - {cause}")?;
        }
        Ok(())
    }
}

/// One leaf of a guard that could not be decided, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCause {
    /// The leaf, as it reads — `amount.amount > 0`, not the whole `all`.
    pub expression: String,
    /// Why that leaf could not be decided.
    pub reason: Reason,
}

impl fmt::Display for UnknownCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.expression, self.reason)
    }
}

/// Why a leaf could not be decided.
///
/// Six sources and a drift alarm, and the split that matters is not between them but across them:
/// only
/// [`ValueAbsent`](Reason::ValueAbsent) describes the *candidate*. The other five describe the
/// specification, so re-rolling values against them is a loop with no exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A segment of the path names nothing the command's input declares.
    ///
    /// `ess-domain` checks only the path's **first** segment against the input field names, and says
    /// so: resolving a deeper one "belongs with the IR, which knows every type in the system". So
    /// `amount.vat > 0` on an input whose `amount` is a two-field struct is a specification that
    /// parses, validates and compiles, and is decidable by nothing.
    PathNotDeclared {
        /// The whole path, as the predicate wrote it.
        path: FactPath,
        /// The segment that names nothing.
        segment: String,
    },
    /// The path lands on a construct no [`FactValue`](aep_domain::facts::FactValue) can hold.
    ///
    /// A fact value is `Bool | Number | Text`. A list, a map, a union and a struct have no scalar
    /// spelling, and a fact path has no index or variant selector to reach inside one with, so this
    /// is unevaluable by construction rather than by omission.
    PathNotScalar {
        /// The whole path, as the predicate wrote it.
        path: FactPath,
        /// What it landed on: `a list`, `a map`, `a union`, `a struct`.
        holds: &'static str,
    },
    /// The path resolves to a scalar the candidate did not supply.
    ///
    /// The one reason a different candidate can repair — an `Optional` left out, or a field the
    /// candidate omitted. It is still a refusal: see
    /// [`Unevaluable::fixable_by_another_candidate`].
    ValueAbsent {
        /// The path nothing was bound to.
        path: FactPath,
    },
    /// Two text values were ordered, and no declared scale contains both.
    ///
    /// `aep-domain` refuses to guess a lexicographic answer for `risk >= medium`, and a protocol
    /// declares [`Scales`](aep_domain::facts::Scales) to give the comparison a meaning. **An ESS
    /// specification declares none**, so every `<`, `<=`, `>` and `>=` between two text values is
    /// unevaluable until [`InputFacts::with_scales`](crate::InputFacts::with_scales) supplies one.
    TextNotOrdered {
        /// The left value.
        left: String,
        /// The right value.
        right: String,
    },
    /// Two values were ordered whose types have no ordering between them.
    ///
    /// Cross-type — a text against a number — and also the same type where no ordering exists, such
    /// as a boolean against a boolean. `==` and `!=` are still decided; only the four ordering
    /// operators land here.
    TypesNotOrdered {
        /// The left value's type name.
        left: &'static str,
        /// The right value's type name.
        right: &'static str,
    },
    /// Resolving the path walked deeper than
    /// [`MAX_TYPE_DEPTH`](ess_domain::types::MAX_TYPE_DEPTH).
    ///
    /// Nothing in the workspace refuses a named type that refers to itself, so `A = struct { b: B }`
    /// beside `B = struct { a: A }` compiles. Bounding the walk is what turns that from a stack
    /// overflow with no diagnostic into a refusal with one.
    TypeTooDeep {
        /// The path being resolved.
        path: FactPath,
        /// The limit it exceeded.
        limit: usize,
    },
    /// The evaluator returned `Unknown` for a leaf this crate does not know a reason for.
    ///
    /// Unreachable against today's `aep-domain`, and present anyway: the five reasons above mirror
    /// the branches of `Predicate::evaluate`, and a branch added there without one added here would
    /// otherwise surface as a refusal with an empty explanation. This is the drift alarm, and
    /// `unclassified_is_a_drift_alarm_and_no_enumerated_source_trips_it` is what watches it.
    Unclassified,
}

impl Reason {
    /// `true` when a different candidate input could make this reason go away.
    ///
    /// Exactly one variant answers `true`. The rest are statements about the specification, and a
    /// runner that retried on one of them would retry forever.
    pub fn fixable_by_another_candidate(&self) -> bool {
        matches!(self, Self::ValueAbsent { .. })
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathNotDeclared { path, segment } => write!(
                f,
                "`{path}` reads `{segment}`, which the input's types do not declare",
            ),
            Self::PathNotScalar { path, holds } => {
                write!(f, "`{path}` is {holds}, and no fact value can hold one")
            }
            Self::ValueAbsent { path } => {
                write!(
                    f,
                    "`{path}` is declared, and this candidate supplied nothing for it"
                )
            }
            Self::TextNotOrdered { left, right } => write!(
                f,
                "cannot order {left:?} against {right:?}: no declared scale contains both values",
            ),
            Self::TypesNotOrdered { left, right } => {
                write!(f, "cannot order a {left} against a {right}")
            }
            Self::TypeTooDeep { path, limit } => write!(
                f,
                "resolving `{path}` walked deeper than {limit} types, which a type referring to \
                 itself is the only way to do",
            ),
            Self::Unclassified => f.write_str(
                "the evaluator could not decide it, for a reason this crate does not enumerate",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use aep_domain::facts::FactPath;

    fn path(value: &str) -> FactPath {
        FactPath::new(value).expect("a valid fact path")
    }

    /// Every reason, so that adding one without deciding this question fails here.
    fn every_reason() -> Vec<Reason> {
        vec![
            Reason::PathNotDeclared {
                path: path("amount.vat"),
                segment: "vat".to_owned(),
            },
            Reason::PathNotScalar {
                path: path("lines"),
                holds: "a list",
            },
            Reason::ValueAbsent { path: path("note") },
            Reason::TextNotOrdered {
                left: "USD".to_owned(),
                right: "EUR".to_owned(),
            },
            Reason::TypesNotOrdered {
                left: "text",
                right: "number",
            },
            Reason::TypeTooDeep {
                path: path("a.b"),
                limit: 32,
            },
            Reason::Unclassified,
        ]
    }

    #[test]
    fn exactly_one_reason_says_another_candidate_would_help() {
        let repairable: Vec<Reason> = every_reason()
            .into_iter()
            .filter(Reason::fixable_by_another_candidate)
            .collect();
        assert_eq!(
            repairable,
            vec![Reason::ValueAbsent { path: path("note") }],
            "every other reason is a property of the specification, and a runner that retried \
             values on one of them would retry forever"
        );
    }

    #[test]
    fn a_refusal_renders_the_predicate_the_command_and_every_reason() {
        let refusal = Unevaluable {
            predicate: "amount.vat > 0".to_owned(),
            command: "witness.orders.PlaceOrder".to_owned(),
            causes: every_reason()
                .into_iter()
                .map(|reason| UnknownCause {
                    expression: "amount.vat > 0".to_owned(),
                    reason,
                })
                .collect(),
        };
        let rendered = Decision::Unevaluable(refusal).to_string();

        assert!(rendered.contains("amount.vat > 0"));
        assert!(rendered.contains("witness.orders.PlaceOrder"));
        assert_eq!(
            rendered.lines().count(),
            every_reason().len() + 1,
            "one line per reason, so a guard with four defects reads as four: {rendered}"
        );
    }

    #[test]
    fn a_decision_reads_its_two_other_cases_as_neither_satisfied_nor_the_other() {
        let satisfied = Decision::Satisfied;
        assert!(satisfied.is_satisfied());
        assert!(satisfied.refutation().is_none());
        assert!(satisfied.unevaluable().is_none());
    }
}
