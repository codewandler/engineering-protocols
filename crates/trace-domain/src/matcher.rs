//! Bounds, matchers and selectors: the whole of the expectation language, and no more of it.
//!
//! # Structured matchers, not an expression language
//!
//! Design decision **D2**, held here by the types. A matcher applies to **one named field** of a
//! tool's input or of its result. There are no boolean combinators, no arithmetic, and no nesting
//! beyond one field — because the growth path when that becomes insufficient is *not* a second
//! predicate language, it is to project trace facts into the namespace the protocol's existing
//! three-valued predicate language already reads, exactly as `infra-spec`'s `workload_predicate`
//! does. This repository has met that fork once and chose projection; inventing an expression
//! language here would take the other branch by accident.
//!
//! # `regex` is refused by name, and `glob` is what to write instead
//!
//! Design § 3.4 lists a `regex` matcher. This build does not implement one and does not silently
//! reinterpret one: the workspace carries no regular-expression engine, `AGENTS.md`
//! § *Dependencies* says to prefer no dependency and record the refusal, and a `regex:` key
//! quietly read as `contains:` would be a specification that means something other than what it
//! says. [`crate::code::TraceCode::SpecUnsupportedMatcher`] refuses it and the message names
//! [`FieldMatcher::Glob`].
//!
//! What `glob` buys is what the design's own examples needed: `*.engineering/planning/*.md` is
//! the file-path assertion in § 3, and it is a glob wearing a regular expression's syntax. What it
//! does not buy is alternation, capture and quantifiers — which is a real loss, named here rather
//! than discovered later.
//!
//! # A bare number is not a bound
//!
//! `count: 1` cannot be read as "at least once" by one author and "exactly once" by the next,
//! because [`CountBound`] has no shorthand for it. And [`RangeBound`] — the bound over money,
//! ratios and derived durations — has **no `exactly` at all**, which is design decision **D6**
//! made structural: a cost expectation exists to catch a run that looped for forty minutes, not
//! to detect a 12% regression, and an equality over a float is a CI job people learn to ignore.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

use crate::ir::{Recorded, ToolCall, ToolResult};

/// A bound over a whole number: a count, a token total, a duration in milliseconds.
///
/// At least one side is always set — a bound that bounds nothing is refused at validation
/// ([`crate::code::TraceCode::SpecInvalidExpectation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct CountBound {
    /// The lowest acceptable value.
    pub at_least: Option<u64>,
    /// The highest acceptable value.
    pub at_most: Option<u64>,
    /// The only acceptable value. Never combined with the two above.
    pub exactly: Option<u64>,
}

impl CountBound {
    /// A bound that accepts exactly this value.
    pub fn exactly(value: u64) -> Self {
        Self {
            exactly: Some(value),
            ..Self::default()
        }
    }

    /// A bound that accepts this value or more.
    pub fn at_least(value: u64) -> Self {
        Self {
            at_least: Some(value),
            ..Self::default()
        }
    }

    /// A bound that accepts this value or less.
    pub fn at_most(value: u64) -> Self {
        Self {
            at_most: Some(value),
            ..Self::default()
        }
    }

    /// `true` when the observed value satisfies it.
    pub fn holds(self, value: u64) -> bool {
        if let Some(exactly) = self.exactly {
            return value == exactly;
        }
        self.at_least.is_none_or(|floor| value >= floor)
            && self.at_most.is_none_or(|ceiling| value <= ceiling)
    }

    /// `true` when it states no side at all.
    pub fn is_empty(self) -> bool {
        self.at_least.is_none() && self.at_most.is_none() && self.exactly.is_none()
    }

    /// `true` when it can never hold: a floor above its ceiling.
    pub fn is_unsatisfiable(self) -> bool {
        match (self.at_least, self.at_most) {
            (Some(floor), Some(ceiling)) => floor > ceiling,
            _ => false,
        }
    }
}

impl fmt::Display for CountBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.exactly, self.at_least, self.at_most) {
            (Some(exactly), _, _) => write!(f, "exactly {exactly}"),
            (_, Some(floor), Some(ceiling)) => write!(f, "between {floor} and {ceiling}"),
            (_, Some(floor), None) => write!(f, "at least {floor}"),
            (_, None, Some(ceiling)) => write!(f, "at most {ceiling}"),
            (None, None, None) => f.write_str("unbounded"),
        }
    }
}

/// A bound over a fractional quantity: money, a ratio, a utilization.
///
/// **No `exactly`, by construction** — design decision D6. Every quantity this bounds varies run
/// to run with model routing, cache state, service tier and load, and an equality over one is a
/// gate that goes red for reasons that have nothing to do with the change.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct RangeBound {
    /// The lowest acceptable value.
    pub at_least: Option<f64>,
    /// The highest acceptable value.
    pub at_most: Option<f64>,
}

impl RangeBound {
    /// A bound that accepts this value or less.
    pub fn at_most(value: f64) -> Self {
        Self {
            at_least: None,
            at_most: Some(value),
        }
    }

    /// A bound that accepts this value or more.
    pub fn at_least(value: f64) -> Self {
        Self {
            at_least: Some(value),
            at_most: None,
        }
    }

    /// `true` when the observed value satisfies it.
    pub fn holds(self, value: f64) -> bool {
        self.at_least.is_none_or(|floor| value >= floor)
            && self.at_most.is_none_or(|ceiling| value <= ceiling)
    }

    /// `true` when it states no side at all.
    pub fn is_empty(self) -> bool {
        self.at_least.is_none() && self.at_most.is_none()
    }

    /// `true` when it can never hold: a floor above its ceiling.
    pub fn is_unsatisfiable(self) -> bool {
        match (self.at_least, self.at_most) {
            (Some(floor), Some(ceiling)) => floor > ceiling,
            _ => false,
        }
    }
}

impl fmt::Display for RangeBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.at_least, self.at_most) {
            (Some(floor), Some(ceiling)) => write!(f, "between {floor} and {ceiling}"),
            (Some(floor), None) => write!(f, "at least {floor}"),
            (None, Some(ceiling)) => write!(f, "at most {ceiling}"),
            (None, None) => f.write_str("unbounded"),
        }
    }
}

/// A scalar an `equals` matcher compares against.
///
/// No float variant, deliberately: `equals: 0.62` over a recorded utilization is the equality D6
/// refuses, and offering the spelling would invite it. A fractional number is refused at
/// validation with the advice to write a bound instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ScalarValue {
    /// A boolean, such as `userModified: {equals: false}`.
    Bool(bool),
    /// A whole number.
    Integer(i64),
    /// A string.
    Text(String),
}

impl ScalarValue {
    /// `true` when a recorded value equals this, comparing like with like.
    ///
    /// Typed rather than textual: `equals: false` does not match the string `"false"`, because a
    /// harness that started recording a boolean as a string has changed the fact, and an
    /// expectation that kept passing across that change was never checking it.
    pub fn matches(&self, recorded: &Recorded) -> bool {
        match (self, recorded) {
            (Self::Bool(expected), Recorded::Bool(actual)) => expected == actual,
            (Self::Integer(expected), Recorded::Number(actual)) => {
                actual.as_i64() == Some(*expected)
            }
            (Self::Text(expected), Recorded::String(actual)) => expected == actual,
            _ => false,
        }
    }
}

impl fmt::Display for ScalarValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Text(value) => write!(f, "{value}"),
        }
    }
}

/// How one named field is compared.
///
/// Externally tagged — `{"contains": "protocol artifact new"}` — which is both the shape a
/// document writes and a shape serde can serialize a newtype variant into. An internally tagged
/// form would refuse a variant holding a bare string at run time rather than at compile time,
/// which is a failure a digest would only meet in production.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldMatcher {
    /// The whole field, character for character.
    Exact(String),
    /// A substring of the field.
    Contains(String),
    /// A glob over the field: `*` for any run of characters, `?` for one, everything else
    /// literal.
    Glob(String),
    /// A scalar field, compared like with like.
    Equals(ScalarValue),
}

impl FieldMatcher {
    /// `true` when a recorded value satisfies it.
    ///
    /// The three textual matchers read a non-string field through [`text_of`], so
    /// `contains: "planning"` finds it inside a nested object without the document needing a
    /// second syntax for reaching in. `equals` never does that: it compares types.
    pub fn matches(&self, recorded: &Recorded) -> bool {
        match self {
            Self::Exact(expected) => text_of(recorded) == *expected,
            Self::Contains(expected) => text_of(recorded).contains(expected.as_str()),
            Self::Glob(pattern) => glob_matches(pattern, &text_of(recorded)),
            Self::Equals(expected) => expected.matches(recorded),
        }
    }

    /// `true` when a plain string satisfies it.
    ///
    /// Used where the subject is already text — a final assistant message, a plugin name — rather
    /// than a recorded JSON value.
    pub fn matches_text(&self, text: &str) -> bool {
        match self {
            // `equals` on a string is spelled differently from `exact` and means the same thing
            // here, because a bare text subject has no type to compare. On a *recorded* value the
            // two diverge — see [`Self::matches`] — which is why both spellings exist.
            Self::Exact(expected) | Self::Equals(ScalarValue::Text(expected)) => text == expected,
            Self::Contains(expected) => text.contains(expected.as_str()),
            Self::Glob(pattern) => glob_matches(pattern, text),
            Self::Equals(_) => false,
        }
    }
}

impl fmt::Display for FieldMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(value) => write!(f, "= {value:?}"),
            Self::Contains(value) => write!(f, "~ {value:?}"),
            Self::Glob(value) => write!(f, "glob {value:?}"),
            Self::Equals(value) => write!(f, "== {value}"),
        }
    }
}

/// A recorded value as text: a string as itself, anything else as its compact JSON.
///
/// One rule, written down, because the alternative is two readers disagreeing about whether
/// `contains: "true"` should find a boolean. It should: the textual matchers read the field as it
/// would be printed, and `equals` is the one that reads it as it was typed.
#[must_use]
pub fn text_of(recorded: &Recorded) -> String {
    match recorded {
        Recorded::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Matches a glob against a subject: `*` any run of characters, `?` exactly one.
///
/// Iterative with one backtrack point, so it is linear in practice and cannot blow up the way a
/// backtracking regular expression can — which matters for a checker that reads whatever a
/// transcript happens to contain.
#[must_use]
pub fn glob_matches(pattern: &str, subject: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let subject: Vec<char> = subject.chars().collect();
    let (mut p, mut s) = (0usize, 0usize);
    let (mut star, mut resume) = (None, 0usize);

    while s < subject.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some(p);
                resume = s;
                p += 1;
            }
            Some('?') => {
                p += 1;
                s += 1;
            }
            Some(literal) if *literal == subject[s] => {
                p += 1;
                s += 1;
            }
            _ => match star {
                Some(at) => {
                    p = at + 1;
                    resume += 1;
                    s = resume;
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|character| *character == '*')
}

/// Which tool calls an expectation is about.
///
/// The scope half of every tool-family kind. It is deliberately *scoped* rather than global,
/// because the design's error family has a nuance that must not be papered over: **a refusal this
/// project designed is correct behaviour, not a failure.** `protocol artifact move` exits 1 when
/// the move is illegal, so a run in which the model asked for an illegal move, received the
/// refusal and relayed it behaved exactly right — and contains a failed tool call. A blanket
/// `tool.error_rate: 0` would forbid the plugin's own intended behaviour; a selector lets a
/// specification say *no failed `Read`* and leave the deliberate refusal alone.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct CallSelector {
    /// The tool's name. Absent selects every tool.
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    pub args: BTreeMap<String, FieldMatcher>,
}

impl CallSelector {
    /// A selector for one tool and nothing else.
    pub fn tool(name: impl Into<String>) -> Self {
        Self {
            tool: Some(name.into()),
            args: BTreeMap::new(),
        }
    }

    /// `true` when a call is in scope.
    ///
    /// An argument the call does not carry does **not** match: a matcher over `command` on a call
    /// that has no `command` is a claim about a field that is not there, and reading absence as a
    /// match would let a selector widen silently when a harness renames a field.
    pub fn matches(&self, call: &ToolCall) -> bool {
        if let Some(name) = &self.tool {
            if call.name != *name {
                return false;
            }
        }
        self.args.iter().all(|(field, matcher)| {
            call.argument(field)
                .is_some_and(|value| matcher.matches(value))
        })
    }

    /// `true` when it selects everything — no tool name and no argument matcher.
    pub fn is_unscoped(&self) -> bool {
        self.tool.is_none() && self.args.is_empty()
    }
}

impl fmt::Display for CallSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tool.as_deref().unwrap_or("any tool"))
            .and_then(|()| {
                if self.args.is_empty() {
                    Ok(())
                } else {
                    let rendered: Vec<String> = self
                        .args
                        .iter()
                        .map(|(field, matcher)| format!("{field} {matcher}"))
                        .collect();
                    write!(f, "({})", rendered.join(", "))
                }
            })
    }
}

/// Matchers over a tool result's named fields, all of which must hold.
///
/// Separate from [`CallSelector`] because `tool.called` matches the **request** and `tool.result`
/// matches what came back, and the two are different claims: a `Bash` call whose command matched
/// and whose `interrupted` is `true` satisfies the first and should fail the second.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct ResultMatcher {
    /// Matchers over named result fields.
    pub fields: BTreeMap<String, FieldMatcher>,
}

impl ResultMatcher {
    /// `true` when every named field is present and satisfies its matcher.
    pub fn matches(&self, result: &ToolResult) -> bool {
        self.fields
            .iter()
            .all(|(field, matcher)| result.field(field).is_some_and(|it| matcher.matches(it)))
    }

    /// The fields it names that a result does not carry.
    ///
    /// A missing field is not a failed match: the transcript did not say, and the verdict is
    /// `unk`. This is what lets the checker tell the two apart.
    pub fn absent_fields(&self, result: &ToolResult) -> Vec<String> {
        self.fields
            .keys()
            .filter(|field| result.field(field).is_none())
            .cloned()
            .collect()
    }

    /// `true` when it names no field.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl fmt::Display for ResultMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered: Vec<String> = self
            .fields
            .iter()
            .map(|(field, matcher)| format!("{field} {matcher}"))
            .collect();
        f.write_str(&rendered.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_bound_reads_exactly_before_either_side_so_one_number_has_one_meaning() {
        assert!(CountBound::exactly(0).holds(0));
        assert!(!CountBound::exactly(0).holds(1));
        assert!(CountBound::at_least(1).holds(9));
        assert!(!CountBound::at_least(1).holds(0));
        assert!(CountBound::at_most(3).holds(3));
        assert!(!CountBound::at_most(3).holds(4));
    }

    #[test]
    fn a_bound_with_a_floor_above_its_ceiling_is_unsatisfiable_and_says_so() {
        let bound = CountBound {
            at_least: Some(5),
            at_most: Some(2),
            exactly: None,
        };
        assert!(bound.is_unsatisfiable());
        assert!(!bound.holds(3), "nothing can satisfy it");
        assert!(RangeBound {
            at_least: Some(1.0),
            at_most: Some(0.5),
        }
        .is_unsatisfiable());
    }

    #[test]
    fn equals_compares_like_with_like_so_a_boolean_becoming_a_string_is_visible() {
        let matcher = FieldMatcher::Equals(ScalarValue::Bool(false));
        assert!(matcher.matches(&serde_json::json!(false)));
        assert!(
            !matcher.matches(&serde_json::json!("false")),
            "a harness that started recording a boolean as a string changed the fact"
        );
    }

    #[test]
    fn a_textual_matcher_reads_a_non_string_field_through_its_json_rendering() {
        let matcher = FieldMatcher::Contains("planning".to_owned());
        assert!(matcher.matches(&serde_json::json!({ "skill": "protocols:planning" })));
        assert!(!matcher.matches(&serde_json::json!({ "skill": "protocols:review" })));
    }

    #[test]
    fn a_glob_matches_the_paths_the_design_writes_and_refuses_the_ones_it_does_not() {
        assert!(glob_matches(
            "*/.engineering/planning/*.md",
            "/work/project/.engineering/planning/story/x.md"
        ));
        assert!(!glob_matches(
            "*/.engineering/planning/*.md",
            "/work/project/docs/plan/x.md"
        ));
        assert!(
            glob_matches("*", ""),
            "a lone star matches the empty string"
        );
        assert!(glob_matches("a?c", "abc"));
        assert!(!glob_matches("a?c", "ac"), "`?` is exactly one character");
        assert!(
            !glob_matches("abc", "abcd"),
            "a glob is anchored at both ends"
        );
    }

    #[test]
    fn a_selector_over_an_argument_the_call_does_not_carry_does_not_match() {
        // The widening this refuses: if a harness renamed `command` tomorrow, reading the absent
        // field as a match would turn "every Bash call running the CLI" into "every Bash call".
        let call = ToolCall {
            call_id: None,
            name: "Bash".to_owned(),
            input: BTreeMap::new(),
            input_bytes: 0,
            result_event: None,
        };
        let mut selector = CallSelector::tool("Bash");
        selector.args.insert(
            "command".to_owned(),
            FieldMatcher::Contains("protocol".to_owned()),
        );
        assert!(!selector.matches(&call));
        assert!(
            CallSelector::tool("Bash").matches(&call),
            "the tool name alone still selects it"
        );
    }

    #[test]
    fn a_result_matcher_separates_a_field_that_disagrees_from_a_field_that_is_absent() {
        let mut fields = BTreeMap::new();
        fields.insert("userModified".to_owned(), serde_json::json!(true));
        let result = ToolResult {
            call_id: None,
            is_error: None,
            content_bytes: 0,
            content: None,
            fields,
        };
        let mut matcher = ResultMatcher::default();
        matcher.fields.insert(
            "userModified".to_owned(),
            FieldMatcher::Equals(ScalarValue::Bool(false)),
        );
        assert!(!matcher.matches(&result), "the field disagrees");
        assert!(
            matcher.absent_fields(&result).is_empty(),
            "and it is present, which is what makes this a gap rather than an unknown"
        );

        let mut missing = ResultMatcher::default();
        missing.fields.insert(
            "interrupted".to_owned(),
            FieldMatcher::Equals(ScalarValue::Bool(false)),
        );
        assert_eq!(missing.absent_fields(&result), vec!["interrupted"]);
    }
}
