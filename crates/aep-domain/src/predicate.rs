//! The predicate language.
//!
//! Predicates are the only way AEP asks a question about the world. A transition, a
//! completion condition, a principle's applicability and an obligation are all predicates
//! over [facts](crate::facts).
//!
//! # Three-valued logic
//!
//! Evaluation is Kleene three-valued: [`Truth::True`], [`Truth::False`] and
//! [`Truth::Unknown`]. The third value is the point of the design. `tests.unit.failed == 0`
//! is *false* when a suite failed and *unknown* when nothing ran, and those two situations
//! need different responses from a harness: fix the code, or go run the tests. A predicate
//! only permits a transition when it is `True`, so unknown never advances a workflow.
//!
//! # Syntax
//!
//! Both a compact string form and a structured form parse to the same value:
//!
//! ```yaml
//! # string form
//! - tests.unit.failed == 0
//! - error_rate < service.slo.error_threshold
//! - recovery_verified == true
//! - specification.satisfied              # bare path: the fact is present and truthy
//! - defined(deployment.previous_revision)
//! - not artifact.design.superseded
//!
//! # structured form
//! all:
//!   - any: [service.health == healthy, service.health == degraded]
//!   - not: change.architectural
//!   - task.kind: {any_of: [feature, bugfix]}
//!   - risk: {gte: medium}
//!   - change.architectural: true
//! ```
//!
//! A bare list is an implicit `all`. On the right-hand side of a comparison, a bare word
//! containing a dot is read as a fact path and anything else as a literal; quote a literal
//! that contains dots (`version == "1.2.3"`).

use std::cmp::Ordering;
use std::fmt;

use crate::error::ParseError;
use crate::facts::{FactPath, FactSource, FactValue};
use crate::node::Node;

/// The result of evaluating a predicate.
///
/// `Unknown` means no observation has been made yet; it is distinct from `False`, which means
/// an observation contradicts the predicate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Truth {
    /// Observed to hold.
    True,
    /// Observed not to hold.
    False,
    /// Not yet observable: a referenced fact has no value.
    Unknown,
}

impl Truth {
    /// Kleene conjunction: `False` dominates, then `Unknown`.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::True, Self::True) => Self::True,
        }
    }

    /// Kleene disjunction: `True` dominates, then `Unknown`.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::False, Self::False) => Self::False,
        }
    }

    /// Kleene negation: `Unknown` negates to itself.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }

    /// `true` only for [`Truth::True`]; what a transition requires.
    pub fn is_satisfied(self) -> bool {
        self == Self::True
    }

    /// The truth value as it appears in output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::True => "true",
            Self::False => "false",
            Self::Unknown => "unknown",
        }
    }

    /// Builds a truth value from a boolean observation.
    pub fn from_bool(value: bool) -> Self {
        if value {
            Self::True
        } else {
            Self::False
        }
    }
}

impl fmt::Display for Truth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A comparison operator.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

impl CompareOp {
    /// The operator as written in a predicate expression.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }

    /// `true` when this operator needs an ordering, not just equality.
    pub fn needs_ordering(self) -> bool {
        !matches!(self, Self::Eq | Self::Ne)
    }

    /// Parses the map-form spelling of an operator, such as `gte`.
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "eq" | "equals" | "==" => Some(Self::Eq),
            "ne" | "not_equals" | "!=" => Some(Self::Ne),
            "lt" | "<" => Some(Self::Lt),
            "le" | "lte" | "<=" => Some(Self::Le),
            "gt" | ">" => Some(Self::Gt),
            "ge" | "gte" | ">=" => Some(Self::Ge),
            _ => None,
        }
    }

    /// Applies this operator to an ordering.
    fn accepts(self, ordering: Ordering) -> bool {
        match self {
            Self::Eq => ordering == Ordering::Equal,
            Self::Ne => ordering != Ordering::Equal,
            Self::Lt => ordering == Ordering::Less,
            Self::Le => ordering != Ordering::Greater,
            Self::Gt => ordering == Ordering::Greater,
            Self::Ge => ordering != Ordering::Less,
        }
    }
}

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One side of a comparison: either a fact to look up, or a literal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum Operand {
    /// A fact path, resolved against the fact source at evaluation time.
    Fact(FactPath),
    /// A constant written in the document.
    Literal(FactValue),
}

impl Operand {
    /// Resolves this operand, returning `None` when a referenced fact is unobserved.
    fn resolve(&self, facts: &dyn FactSource) -> Option<FactValue> {
        match self {
            Self::Fact(path) => facts.fact(path),
            Self::Literal(value) => Some(value.clone()),
        }
    }

    /// The fact path this operand reads, if any.
    fn fact_path(&self) -> Option<&FactPath> {
        match self {
            Self::Fact(path) => Some(path),
            Self::Literal(_) => None,
        }
    }

    /// Parses an operand as written on the right-hand side of a comparison.
    ///
    /// A bare word containing a dot is a fact path; everything else is a literal. Quote a
    /// literal that contains dots.
    fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        let quoted = trimmed.starts_with('"') || trimmed.starts_with('\'');
        if !quoted && trimmed.contains('.') && trimmed.parse::<f64>().is_err() {
            if let Ok(path) = FactPath::new(trimmed) {
                return Self::Fact(path);
            }
        }
        Self::Literal(FactValue::parse_literal(trimmed))
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fact(path) => write!(f, "{path}"),
            Self::Literal(FactValue::Text(text)) if text.contains('.') || text.is_empty() => {
                write!(f, "{text:?}")
            }
            Self::Literal(value) => write!(f, "{value}"),
        }
    }
}

/// How deep `all`/`any`/`not` nesting may go before a predicate is refused.
///
/// Both spellings let a document choose the parser's recursion depth. The structured form nests
/// mappings, which `serde_yaml` caps at 128 on its own; the string form is one scalar, so
/// `not not not …` is bounded by nothing but the file size. Measured on this machine, a debug build
/// of [`Predicate::parse_expression`] overflows the 8 MiB main-thread stack between 2 500 and 3 000
/// prefixes and the 2 MiB a spawned worker gets between 600 and 800. A stack overflow aborts the
/// process with no diagnostic, which is the opposite of what this parser does with every other bad
/// input.
///
/// 32 because the deepest predicate anybody writes is an `all` of `any`s of comparisons — three or
/// four — and because `MAX_TYPE_DEPTH` and `ess-domain`'s `WRAPPER_LIMIT` are also 32: one number
/// across the workspace is easier to defend, and to remember, than three. It sits an order of
/// magnitude under the smallest measured floor and well under `serde_yaml`'s 128, so a document
/// nested past it is refused here, with a code and a limit, rather than by the deserializer.
pub const MAX_PREDICATE_DEPTH: usize = 32;

/// A one-level rendering of a node, for an error that must not walk what it is refusing.
///
/// [`Display`] on a [`Node`] recurses, and the node being refused here is by definition the deep
/// one — rendering it to describe it would take exactly the stack the refusal exists to save.
fn shallow(node: &Node) -> String {
    match node {
        Node::Text(text) => text.clone(),
        Node::Seq(items) => format!("a list of {}", items.len()),
        Node::Map(entries) => entries
            .keys()
            .next()
            .map_or_else(|| "an empty mapping".to_owned(), |key| format!("{key}: …")),
        scalar => scalar.type_name().to_owned(),
    }
}

/// A condition over facts.
///
/// # Depth
///
/// Every predicate a document can produce is at most [`MAX_PREDICATE_DEPTH`] deep: the two parsers
/// below are the only routes from a document to one, [`serde::Deserialize`] goes through
/// [`Predicate::from_node`], and [`Predicate::all`], [`Predicate::any`] and [`Predicate::not`]
/// simplify rather than deepen — `not not x` is `x`. That is what lets [`Predicate::evaluate`],
/// [`Predicate::outcome`], [`Predicate::fact_paths`], [`Predicate::to_node`],
/// [`Display`](fmt::Display) and the walkers in `ess-domain` recurse without counting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Predicate {
    /// Always holds. The default where a document omits a condition.
    #[default]
    Always,
    /// Never holds. Useful to disable a transition without deleting it.
    Never,
    /// Every child must hold.
    All(Vec<Predicate>),
    /// At least one child must hold.
    Any(Vec<Predicate>),
    /// The child must not hold.
    Not(Box<Predicate>),
    /// Compares two operands.
    Compare {
        /// Left-hand side.
        left: Operand,
        /// The operator.
        op: CompareOp,
        /// Right-hand side.
        right: Operand,
    },
    /// The fact is observed and truthy.
    Truthy(FactPath),
    /// The fact has been observed at all, whatever its value.
    Defined(FactPath),
    /// The fact equals one of the listed values.
    AnyOf {
        /// The fact to read.
        path: FactPath,
        /// Accepted values.
        values: Vec<FactValue>,
    },
    /// The fact equals none of the listed values.
    NoneOf {
        /// The fact to read.
        path: FactPath,
        /// Rejected values.
        values: Vec<FactValue>,
    },
}

impl Predicate {
    /// Conjunction, simplified: an empty list is [`Predicate::Always`], a single child is
    /// returned unwrapped.
    pub fn all(children: Vec<Self>) -> Self {
        Self::combine(children, true)
    }

    /// Disjunction, simplified: an empty list is [`Predicate::Never`], a single child is
    /// returned unwrapped.
    pub fn any(children: Vec<Self>) -> Self {
        Self::combine(children, false)
    }

    fn combine(mut children: Vec<Self>, conjunction: bool) -> Self {
        match children.len() {
            0 => {
                if conjunction {
                    Self::Always
                } else {
                    Self::Never
                }
            }
            1 => children.remove(0),
            _ => {
                if conjunction {
                    Self::All(children)
                } else {
                    Self::Any(children)
                }
            }
        }
    }

    /// Negation, simplified.
    #[allow(clippy::should_implement_trait)]
    pub fn not(inner: Self) -> Self {
        match inner {
            Self::Always => Self::Never,
            Self::Never => Self::Always,
            Self::Not(nested) => *nested,
            other => Self::Not(Box::new(other)),
        }
    }

    /// `true` when this predicate holds without observing anything.
    pub fn is_trivially_true(&self) -> bool {
        matches!(self, Self::Always)
    }

    /// Evaluates this predicate against `facts`.
    pub fn evaluate(&self, facts: &dyn FactSource) -> Truth {
        match self {
            Self::Always => Truth::True,
            Self::Never => Truth::False,
            Self::All(children) => children
                .iter()
                .fold(Truth::True, |acc, child| acc.and(child.evaluate(facts))),
            Self::Any(children) => children
                .iter()
                .fold(Truth::False, |acc, child| acc.or(child.evaluate(facts))),
            Self::Not(inner) => inner.evaluate(facts).not(),
            Self::Compare { left, op, right } => Self::evaluate_compare(left, *op, right, facts).0,
            Self::Truthy(path) => facts
                .fact(path)
                .map_or(Truth::Unknown, |value| Truth::from_bool(value.is_truthy())),
            Self::Defined(path) => Truth::from_bool(facts.fact(path).is_some()),
            Self::AnyOf { path, values } => facts.fact(path).map_or(Truth::Unknown, |observed| {
                Truth::from_bool(values.contains(&observed))
            }),
            Self::NoneOf { path, values } => facts.fact(path).map_or(Truth::Unknown, |observed| {
                Truth::from_bool(!values.contains(&observed))
            }),
        }
    }

    /// Evaluates a comparison, also returning a note when the comparison is not well defined.
    fn evaluate_compare(
        left: &Operand,
        op: CompareOp,
        right: &Operand,
        facts: &dyn FactSource,
    ) -> (Truth, Option<String>) {
        let (Some(left_value), Some(right_value)) = (left.resolve(facts), right.resolve(facts))
        else {
            return (Truth::Unknown, None);
        };

        match (&left_value, &right_value) {
            (FactValue::Number(left_number), FactValue::Number(right_number)) => (
                Truth::from_bool(op.accepts(left_number.cmp(right_number))),
                None,
            ),
            (FactValue::Text(left_text), FactValue::Text(right_text)) if op.needs_ordering() => {
                match facts.scales().compare(left_text, right_text) {
                    Some(ordering) => (Truth::from_bool(op.accepts(ordering)), None),
                    None => (
                        Truth::Unknown,
                        Some(format!(
                            "cannot order {left_text:?} against {right_text:?}: no protocol scale \
                             contains both values"
                        )),
                    ),
                }
            }
            _ if op.needs_ordering() => (
                Truth::Unknown,
                Some(format!(
                    "cannot order a {} against a {}",
                    left_value.type_name(),
                    right_value.type_name()
                )),
            ),
            _ => {
                let equal = left_value == right_value;
                (
                    Truth::from_bool(if op == CompareOp::Eq { equal } else { !equal }),
                    None,
                )
            }
        }
    }

    /// Evaluates this predicate and explains why it is not satisfied.
    ///
    /// The returned causes are minimal: for a conjunction only the children that did not hold,
    /// for a disjunction every child (since all of them failed), and for a negation the leaves
    /// of the inner predicate that did hold.
    pub fn outcome(&self, facts: &dyn FactSource) -> PredicateOutcome {
        let mut causes = Vec::new();
        let truth = self.collect_causes(facts, false, &mut causes);
        PredicateOutcome {
            expression: self.to_string(),
            truth,
            causes,
        }
    }

    /// Walks the predicate, recording the leaves responsible for a non-satisfied result.
    fn collect_causes(
        &self,
        facts: &dyn FactSource,
        negated: bool,
        causes: &mut Vec<LeafOutcome>,
    ) -> Truth {
        let satisfied = |truth: Truth| {
            if negated {
                truth == Truth::False
            } else {
                truth == Truth::True
            }
        };

        match self {
            Self::All(children) | Self::Any(children) => {
                let conjunction = matches!(self, Self::All(_));
                let truths: Vec<Truth> =
                    children.iter().map(|child| child.evaluate(facts)).collect();
                let truth = if conjunction {
                    truths.iter().fold(Truth::True, |acc, item| acc.and(*item))
                } else {
                    truths.iter().fold(Truth::False, |acc, item| acc.or(*item))
                };
                if !satisfied(truth) {
                    // For a conjunction, the failing children are the cause; for a
                    // disjunction, every child failed, so all of them are.
                    for (child, child_truth) in children.iter().zip(&truths) {
                        if conjunction != negated && satisfied(*child_truth) {
                            continue;
                        }
                        child.collect_causes(facts, negated, causes);
                    }
                }
                truth
            }
            Self::Not(inner) => inner.collect_causes(facts, !negated, causes).not(),
            Self::Always => Truth::True,
            Self::Never => {
                if !satisfied(Truth::False) {
                    causes.push(LeafOutcome {
                        expression: "never".to_owned(),
                        truth: Truth::False,
                        observed: Vec::new(),
                        missing: Vec::new(),
                        note: Some("this transition is explicitly disabled".to_owned()),
                        negated,
                    });
                }
                Truth::False
            }
            leaf => {
                let (truth, note) = leaf.evaluate_leaf(facts);
                if !satisfied(truth) {
                    let mut observed = Vec::new();
                    let mut missing = Vec::new();
                    for path in leaf.fact_paths() {
                        match facts.fact(path) {
                            Some(value) => observed.push((path.clone(), value)),
                            None => missing.push(path.clone()),
                        }
                    }
                    causes.push(LeafOutcome {
                        expression: leaf.to_string(),
                        truth,
                        observed,
                        missing,
                        note,
                        negated,
                    });
                }
                truth
            }
        }
    }

    /// Evaluates a leaf, returning any note about an ill-defined comparison.
    fn evaluate_leaf(&self, facts: &dyn FactSource) -> (Truth, Option<String>) {
        match self {
            Self::Compare { left, op, right } => Self::evaluate_compare(left, *op, right, facts),
            other => (other.evaluate(facts), None),
        }
    }

    /// Every fact path this predicate reads, in traversal order, without duplicates.
    pub fn fact_paths(&self) -> Vec<&FactPath> {
        let mut paths = Vec::new();
        self.visit_fact_paths(&mut |path| {
            if !paths.contains(&path) {
                paths.push(path);
            }
        });
        paths
    }

    fn visit_fact_paths<'a>(&'a self, visit: &mut impl FnMut(&'a FactPath)) {
        match self {
            Self::Always | Self::Never => {}
            Self::All(children) | Self::Any(children) => {
                for child in children {
                    child.visit_fact_paths(visit);
                }
            }
            Self::Not(inner) => inner.visit_fact_paths(visit),
            Self::Compare { left, right, .. } => {
                if let Some(path) = left.fact_path() {
                    visit(path);
                }
                if let Some(path) = right.fact_path() {
                    visit(path);
                }
            }
            Self::Truthy(path)
            | Self::Defined(path)
            | Self::AnyOf { path, .. }
            | Self::NoneOf { path, .. } => visit(path),
        }
    }

    /// Parses a predicate from a document fragment.
    ///
    /// Refuses nesting beyond [`MAX_PREDICATE_DEPTH`] with [`ParseError::TooDeep`]. The budget is
    /// shared with [`Self::parse_expression`], because the two forms interleave: `{not: {not: "not
    /// not a.b"}}` is four levels of one predicate written two ways, and two counters would let a
    /// document alternate between them to buy twice the depth.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        Self::from_node_nested(node, 0)
    }

    /// [`Self::from_node`], counting how deep it already is.
    fn from_node_nested(node: &Node, depth: usize) -> Result<Self, ParseError> {
        if depth > MAX_PREDICATE_DEPTH {
            return Err(ParseError::too_deep(
                "predicate",
                &shallow(node),
                MAX_PREDICATE_DEPTH,
            ));
        }
        match node {
            Node::Bool(true) => Ok(Self::Always),
            Node::Bool(false) => Ok(Self::Never),
            Node::Text(expression) => Self::parse_expression_nested(expression, depth),
            Node::Seq(items) => {
                let children = items
                    .iter()
                    .map(|item| Self::from_node_nested(item, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::all(children))
            }
            Node::Map(entries) => {
                let mut children = Vec::new();
                for (key, value) in entries {
                    children.push(Self::from_entry(key, value, depth)?);
                }
                Ok(Self::all(children))
            }
            Node::Null => Err(ParseError::shape(
                "predicate",
                "an expression, list or mapping",
                "null",
            )),
            Node::Number(number) => Err(ParseError::shape(
                "predicate",
                "an expression, list or mapping",
                format!("the number {number}"),
            )),
        }
    }

    /// Parses one `key: value` entry of a predicate mapping.
    fn from_entry(key: &str, value: &Node, depth: usize) -> Result<Self, ParseError> {
        let nested = |node: &Node| Self::from_node_nested(node, depth + 1);
        match key {
            "all" | "and" | "all_of" => {
                let children = value
                    .as_seq_or_single()
                    .into_iter()
                    .map(nested)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::all(children))
            }
            "any" | "or" => {
                let children = value
                    .as_seq_or_single()
                    .into_iter()
                    .map(nested)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::any(children))
            }
            "not" => Ok(Self::not(nested(value)?)),
            "none" | "none_of_these" => {
                let children = value
                    .as_seq_or_single()
                    .into_iter()
                    .map(nested)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::not(Self::any(children)))
            }
            path => {
                let path = FactPath::new(path).map_err(|error| {
                    ParseError::predicate(
                        path,
                        format!(
                            "{error}; expected a fact path or one of `all`, `any`, `not`, `none`"
                        ),
                    )
                })?;
                Self::from_constraint(path, value)
            }
        }
    }

    /// Parses the constraint attached to a fact path in mapping form.
    fn from_constraint(path: FactPath, value: &Node) -> Result<Self, ParseError> {
        match value {
            Node::Bool(expected) => Ok(Self::Compare {
                left: Operand::Fact(path),
                op: CompareOp::Eq,
                right: Operand::Literal(FactValue::Bool(*expected)),
            }),
            Node::Number(number) => Ok(Self::Compare {
                left: Operand::Fact(path),
                op: CompareOp::Eq,
                right: Operand::Literal(FactValue::Number(*number)),
            }),
            Node::Text(text) => Ok(Self::Compare {
                left: Operand::Fact(path),
                op: CompareOp::Eq,
                right: Operand::Literal(FactValue::parse_literal(text)),
            }),
            Node::Seq(items) => Ok(Self::AnyOf {
                path,
                values: literal_values(items)?,
            }),
            Node::Map(entries) => {
                let mut children = Vec::new();
                for (operator, operand) in entries {
                    children.push(Self::from_operator(path.clone(), operator, operand)?);
                }
                Ok(Self::all(children))
            }
            Node::Null => Err(ParseError::predicate(
                &path.to_string(),
                "a fact constraint must be a value, a list of values or a mapping of operators",
            )),
        }
    }

    /// Parses one operator constraint, such as `any_of` or `gte`.
    fn from_operator(path: FactPath, operator: &str, operand: &Node) -> Result<Self, ParseError> {
        if let Some(op) = CompareOp::from_keyword(operator) {
            let right = match operand {
                Node::Text(text) => Operand::parse(text),
                Node::Bool(value) => Operand::Literal(FactValue::Bool(*value)),
                Node::Number(number) => Operand::Literal(FactValue::Number(*number)),
                other => {
                    return Err(ParseError::predicate(
                        &format!("{path}: {{{operator}: {other}}}"),
                        "a comparison operand must be a scalar",
                    ))
                }
            };
            return Ok(Self::Compare {
                left: Operand::Fact(path),
                op,
                right,
            });
        }

        match operator {
            "any_of" | "in" | "one_of" => Ok(Self::AnyOf {
                path,
                values: literal_values(&operand.as_seq_or_single_owned())?,
            }),
            "none_of" | "not_in" => Ok(Self::NoneOf {
                path,
                values: literal_values(&operand.as_seq_or_single_owned())?,
            }),
            "exists" | "defined" => {
                let expected = match operand {
                    Node::Bool(value) => *value,
                    other => {
                        return Err(ParseError::predicate(
                            &format!("{path}: {{{operator}: {other}}}"),
                            "`exists` takes a boolean",
                        ))
                    }
                };
                let defined = Self::Defined(path);
                Ok(if expected {
                    defined
                } else {
                    Self::not(defined)
                })
            }
            "truthy" => Ok(Self::Truthy(path)),
            unknown => Err(ParseError::predicate(
                &format!("{path}: {{{unknown}: …}}"),
                format!(
                    "unknown operator {unknown:?}; expected one of eq, ne, lt, lte, gt, gte, \
                     any_of, none_of, exists, truthy"
                ),
            )),
        }
    }

    /// Parses the compact string form of a predicate.
    ///
    /// Refuses a `not` prefix stacked deeper than [`MAX_PREDICATE_DEPTH`] with
    /// [`ParseError::TooDeep`]. This is the string half of the same budget
    /// [`Self::from_node`] spends.
    pub fn parse_expression(expression: &str) -> Result<Self, ParseError> {
        Self::parse_expression_nested(expression, 0)
    }

    /// [`Self::parse_expression`], counting how deep it already is.
    fn parse_expression_nested(expression: &str, depth: usize) -> Result<Self, ParseError> {
        let trimmed = expression.trim();
        if depth > MAX_PREDICATE_DEPTH {
            return Err(ParseError::too_deep(
                "predicate",
                trimmed,
                MAX_PREDICATE_DEPTH,
            ));
        }
        if trimmed.is_empty() {
            return Err(ParseError::predicate(expression, "expression is empty"));
        }
        match trimmed {
            "always" | "true" => return Ok(Self::Always),
            "never" | "false" => return Ok(Self::Never),
            _ => {}
        }
        if let Some(rest) = trimmed.strip_prefix("not ") {
            return Ok(Self::not(Self::parse_expression_nested(rest, depth + 1)?));
        }
        for (function, negate) in [("defined", false), ("exists", false), ("missing", true)] {
            if let Some(inner) = call_argument(trimmed, function) {
                let path = FactPath::new(inner).map_err(|error| {
                    ParseError::predicate(expression, format!("{function}() argument: {error}"))
                })?;
                let predicate = Self::Defined(path);
                return Ok(if negate {
                    Self::not(predicate)
                } else {
                    predicate
                });
            }
        }

        if let Some((left, op, right)) = split_comparison(trimmed) {
            let left_path = FactPath::new(left.trim()).map_err(|error| {
                ParseError::predicate(
                    expression,
                    format!("left-hand side must be a fact path: {error}"),
                )
            })?;
            if right.trim().is_empty() {
                return Err(ParseError::predicate(
                    expression,
                    format!("nothing to compare against after `{op}`"),
                ));
            }
            return Ok(Self::Compare {
                left: Operand::Fact(left_path),
                op,
                right: Operand::parse(right),
            });
        }

        let path = FactPath::new(trimmed).map_err(|error| {
            ParseError::predicate(
                expression,
                format!(
                    "{error}; a predicate is either a comparison (`a.b == 0`) or a bare fact path"
                ),
            )
        })?;
        Ok(Self::Truthy(path))
    }

    /// Renders this predicate back into document form.
    pub fn to_node(&self) -> Node {
        match self {
            Self::Always => Node::Bool(true),
            Self::Never => Node::Bool(false),
            Self::All(children) => Node::Map(
                [(
                    "all".to_owned(),
                    Node::Seq(children.iter().map(Self::to_node).collect()),
                )]
                .into_iter()
                .collect(),
            ),
            Self::Any(children) => Node::Map(
                [(
                    "any".to_owned(),
                    Node::Seq(children.iter().map(Self::to_node).collect()),
                )]
                .into_iter()
                .collect(),
            ),
            Self::Not(inner) => {
                Node::Map([("not".to_owned(), inner.to_node())].into_iter().collect())
            }
            Self::AnyOf { path, values } => Node::Map(
                [(
                    path.to_string(),
                    Node::Map(
                        [(
                            "any_of".to_owned(),
                            Node::Seq(values.iter().map(value_node).collect()),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                )]
                .into_iter()
                .collect(),
            ),
            Self::NoneOf { path, values } => Node::Map(
                [(
                    path.to_string(),
                    Node::Map(
                        [(
                            "none_of".to_owned(),
                            Node::Seq(values.iter().map(value_node).collect()),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                )]
                .into_iter()
                .collect(),
            ),
            leaf => Node::Text(leaf.to_string()),
        }
    }
}

/// Converts a fact value into a document node.
fn value_node(value: &FactValue) -> Node {
    match value {
        FactValue::Bool(inner) => Node::Bool(*inner),
        FactValue::Number(inner) => Node::Number(*inner),
        FactValue::Text(inner) => Node::Text(inner.clone()),
    }
}

/// Parses a list of literal values.
fn literal_values(items: &[Node]) -> Result<Vec<FactValue>, ParseError> {
    items
        .iter()
        .map(|item| match item {
            Node::Bool(value) => Ok(FactValue::Bool(*value)),
            Node::Number(value) => Ok(FactValue::Number(*value)),
            Node::Text(value) => Ok(FactValue::parse_literal(value)),
            other => Err(ParseError::shape(
                "predicate value list",
                "a scalar",
                other.type_name(),
            )),
        })
        .collect()
}

impl Node {
    /// This node as a list of owned nodes, treating a scalar as a one-element list.
    fn as_seq_or_single_owned(&self) -> Vec<Node> {
        self.as_seq_or_single().into_iter().cloned().collect()
    }
}

/// Extracts `argument` from `function(argument)`.
fn call_argument<'a>(expression: &'a str, function: &str) -> Option<&'a str> {
    let rest = expression.strip_prefix(function)?;
    let rest = rest.trim_start().strip_prefix('(')?;
    rest.trim_end().strip_suffix(')').map(str::trim)
}

/// Splits an expression at its first top-level comparison operator.
fn split_comparison(expression: &str) -> Option<(&str, CompareOp, &str)> {
    let bytes = expression.as_bytes();
    let mut quote: Option<u8> = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) => {
                if byte == open {
                    quote = None;
                }
            }
            None => {
                if byte == b'"' || byte == b'\'' {
                    quote = Some(byte);
                } else {
                    let two = expression.get(index..index + 2);
                    if let Some(op) = two.and_then(|slice| match slice {
                        "==" => Some(CompareOp::Eq),
                        "!=" => Some(CompareOp::Ne),
                        "<=" => Some(CompareOp::Le),
                        ">=" => Some(CompareOp::Ge),
                        _ => None,
                    }) {
                        return Some((&expression[..index], op, &expression[index + 2..]));
                    }
                    if byte == b'<' {
                        return Some((
                            &expression[..index],
                            CompareOp::Lt,
                            &expression[index + 1..],
                        ));
                    }
                    if byte == b'>' {
                        return Some((
                            &expression[..index],
                            CompareOp::Gt,
                            &expression[index + 1..],
                        ));
                    }
                }
            }
        }
        index += 1;
    }
    None
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => f.write_str("always"),
            Self::Never => f.write_str("never"),
            Self::All(children) => write_joined(f, children, " and "),
            Self::Any(children) => write_joined(f, children, " or "),
            Self::Not(inner) => write!(f, "not ({inner})"),
            Self::Compare { left, op, right } => write!(f, "{left} {op} {right}"),
            Self::Truthy(path) => write!(f, "{path}"),
            Self::Defined(path) => write!(f, "defined({path})"),
            Self::AnyOf { path, values } => write!(f, "{path} in [{}]", join_values(values)),
            Self::NoneOf { path, values } => write!(f, "{path} not in [{}]", join_values(values)),
        }
    }
}

/// Renders children joined by `separator`, parenthesised.
fn write_joined(
    f: &mut fmt::Formatter<'_>,
    children: &[Predicate],
    separator: &str,
) -> fmt::Result {
    f.write_str("(")?;
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            f.write_str(separator)?;
        }
        write!(f, "{child}")?;
    }
    f.write_str(")")
}

/// Renders a comma-separated value list.
fn join_values(values: &[FactValue]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

impl std::str::FromStr for Predicate {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_expression(value)
    }
}

impl serde::Serialize for Predicate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_node().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Predicate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Predicate {
    fn schema_name() -> String {
        "Predicate".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject::default();
        schema.subschemas().any_of = Some(vec![
            <String>::json_schema(generator),
            <bool>::json_schema(generator),
            <Vec<Node>>::json_schema(generator),
            <std::collections::BTreeMap<String, Node>>::json_schema(generator),
        ]);
        schema.metadata().description = Some(
            "A condition over facts: the compact expression form (`tests.unit.failed == 0`), a \
             list (implicit `all`), or a mapping using `all`, `any`, `not`, `none` or a fact path \
             with an operator constraint."
                .to_owned(),
        );
        schema.into()
    }
}

/// The result of evaluating a predicate, with the reason when it is not satisfied.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct PredicateOutcome {
    /// The predicate as written, in canonical compact form.
    pub expression: String,
    /// The result.
    pub truth: Truth,
    /// The leaves responsible for a non-satisfied result, empty when satisfied.
    pub causes: Vec<LeafOutcome>,
}

impl PredicateOutcome {
    /// `true` when the predicate holds.
    pub fn is_satisfied(&self) -> bool {
        self.truth.is_satisfied()
    }

    /// Fact paths that nothing has observed yet.
    pub fn missing_facts(&self) -> Vec<&FactPath> {
        let mut paths = Vec::new();
        for cause in &self.causes {
            for path in &cause.missing {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        paths
    }
}

/// One leaf condition that prevented a predicate from being satisfied.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct LeafOutcome {
    /// The leaf, in compact form.
    pub expression: String,
    /// Its truth value.
    pub truth: Truth,
    /// Facts it read that had values.
    pub observed: Vec<(FactPath, FactValue)>,
    /// Facts it read that nothing has observed.
    pub missing: Vec<FactPath>,
    /// Why the comparison could not be made, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// `true` when this leaf appears under a negation, so holding is the problem.
    pub negated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{FactStore, Scales};

    fn store(facts: &[(&str, FactValue)]) -> FactStore {
        let mut store = FactStore::new();
        for (path, value) in facts {
            store.set_path(path, value.clone());
        }
        store
    }

    fn parse(input: &str) -> Predicate {
        Predicate::parse_expression(input).expect("parses")
    }

    #[test]
    fn parses_the_compact_forms() {
        assert_eq!(
            parse("tests.unit.failed == 0"),
            Predicate::Compare {
                left: Operand::Fact("tests.unit.failed".parse().expect("path")),
                op: CompareOp::Eq,
                right: Operand::Literal(FactValue::count(0)),
            }
        );
        assert_eq!(
            parse("specification.satisfied"),
            Predicate::Truthy("specification.satisfied".parse().expect("path"))
        );
        assert_eq!(
            parse("defined(deployment.previous_revision)"),
            Predicate::Defined("deployment.previous_revision".parse().expect("path"))
        );
    }

    #[test]
    fn reads_a_dotted_right_hand_side_as_a_fact_and_a_bare_word_as_text() {
        let compared = parse("error_rate < service.slo.error_threshold");
        let Predicate::Compare { right, .. } = &compared else {
            panic!("expected a comparison, got {compared:?}");
        };
        assert!(matches!(right, Operand::Fact(_)), "{right:?}");

        let equals = parse("test.result == failed");
        let Predicate::Compare { right, .. } = &equals else {
            panic!("expected a comparison, got {equals:?}");
        };
        assert_eq!(right, &Operand::Literal(FactValue::text("failed")));

        let quoted = parse("release.version == \"1.2.3\"");
        let Predicate::Compare { right, .. } = &quoted else {
            panic!("expected a comparison, got {quoted:?}");
        };
        assert_eq!(right, &Operand::Literal(FactValue::text("1.2.3")));
    }

    #[test]
    fn unobserved_facts_are_unknown_not_false() {
        let facts = store(&[]);
        assert_eq!(
            parse("tests.unit.failed == 0").evaluate(&facts),
            Truth::Unknown
        );

        let observed = store(&[("tests.unit.failed", FactValue::count(2))]);
        assert_eq!(
            parse("tests.unit.failed == 0").evaluate(&observed),
            Truth::False
        );
    }

    #[test]
    fn kleene_conjunction_keeps_false_ahead_of_unknown() {
        let facts = store(&[("a.failed", FactValue::count(1))]);
        let predicate = Predicate::all(vec![parse("a.failed == 0"), parse("b.failed == 0")]);
        assert_eq!(
            predicate.evaluate(&facts),
            Truth::False,
            "one observed failure decides the conjunction, whatever the unobserved half holds"
        );

        let partial = store(&[("a.failed", FactValue::count(0))]);
        assert_eq!(
            predicate.evaluate(&partial),
            Truth::Unknown,
            "the unobserved half must stay Unknown, not collapse to False — invariant 5"
        );
    }

    #[test]
    fn explains_only_the_failing_children_of_a_conjunction() {
        let facts = store(&[
            ("tests.unit.failed", FactValue::count(0)),
            ("tests.contract.failed", FactValue::count(3)),
        ]);
        let predicate = Predicate::all(vec![
            parse("tests.unit.failed == 0"),
            parse("tests.contract.failed == 0"),
            parse("static_analysis.errors == 0"),
        ]);

        let outcome = predicate.outcome(&facts);
        assert_eq!(outcome.truth, Truth::False);
        let expressions: Vec<&str> = outcome
            .causes
            .iter()
            .map(|c| c.expression.as_str())
            .collect();
        assert_eq!(
            expressions,
            vec!["tests.contract.failed == 0", "static_analysis.errors == 0"]
        );
        assert_eq!(outcome.causes[0].truth, Truth::False);
        assert_eq!(outcome.causes[1].truth, Truth::Unknown);
        assert_eq!(
            outcome
                .missing_facts()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["static_analysis.errors"]
        );
    }

    #[test]
    fn ordering_text_needs_a_declared_scale() {
        let mut facts = store(&[("risk", FactValue::text("high"))]);
        let predicate = parse("risk >= medium");
        assert_eq!(predicate.evaluate(&facts), Truth::Unknown);

        let mut scales = Scales::default();
        scales.insert(
            "risk",
            ["low", "medium", "high"].map(ToOwned::to_owned).to_vec(),
        );
        facts.set_scales(scales);
        assert_eq!(predicate.evaluate(&facts), Truth::True);
    }

    #[test]
    fn round_trips_through_document_form() {
        let source = Predicate::all(vec![
            parse("tests.unit.failed == 0"),
            Predicate::any(vec![parse("service.health == healthy"), parse("recovered")]),
            Predicate::not(parse("change.architectural")),
            Predicate::AnyOf {
                path: "task.kind".parse().expect("path"),
                values: vec![FactValue::text("feature"), FactValue::text("bugfix")],
            },
        ]);
        let round_tripped = Predicate::from_node(&source.to_node()).expect("re-parses");
        assert_eq!(round_tripped, source);
    }

    #[test]
    fn parses_the_structured_mapping_forms() {
        let node = Node::Map(
            [(
                "task.kind".to_owned(),
                Node::Map(
                    [(
                        "any_of".to_owned(),
                        Node::Seq(vec![Node::from("feature"), Node::from("bugfix")]),
                    )]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        );
        let predicate = Predicate::from_node(&node).expect("parses");
        let facts = store(&[("task.kind", FactValue::text("bugfix"))]);
        assert_eq!(predicate.evaluate(&facts), Truth::True);

        let other = store(&[("task.kind", FactValue::text("release"))]);
        assert_eq!(predicate.evaluate(&other), Truth::False);
    }

    #[test]
    fn negating_unknown_leaves_it_unknown() {
        // Invariant 5, asserted as the whole table rather than as a sample: `not Unknown == True`
        // would mean `not deployment.failed` permits a transition *because* nothing has run, which
        // is the collapse of unobserved into false that the third value exists to prevent.
        for (input, expected) in [
            (Truth::True, Truth::False),
            (Truth::False, Truth::True),
            (Truth::Unknown, Truth::Unknown),
        ] {
            assert_eq!(
                input.not(),
                expected,
                "not {input:?} must be {expected:?}, was {:?}",
                input.not()
            );
        }
    }

    #[test]
    fn conjunction_follows_the_kleene_table_in_all_nine_rows() {
        for (left, right, expected) in [
            (Truth::True, Truth::True, Truth::True),
            (Truth::True, Truth::False, Truth::False),
            (Truth::True, Truth::Unknown, Truth::Unknown),
            (Truth::False, Truth::True, Truth::False),
            (Truth::False, Truth::False, Truth::False),
            // `False` dominates `Unknown`: a failed suite is a failure whether or not the rest ran.
            (Truth::False, Truth::Unknown, Truth::False),
            (Truth::Unknown, Truth::True, Truth::Unknown),
            (Truth::Unknown, Truth::False, Truth::False),
            (Truth::Unknown, Truth::Unknown, Truth::Unknown),
        ] {
            assert_eq!(
                left.and(right),
                expected,
                "{left:?} and {right:?} must be {expected:?}, was {:?}",
                left.and(right)
            );
        }
    }

    #[test]
    fn disjunction_follows_the_kleene_table_in_all_nine_rows() {
        for (left, right, expected) in [
            (Truth::True, Truth::True, Truth::True),
            (Truth::True, Truth::False, Truth::True),
            // `True` dominates `Unknown`: one branch that holds is enough, unobserved or not.
            (Truth::True, Truth::Unknown, Truth::True),
            (Truth::False, Truth::True, Truth::True),
            (Truth::False, Truth::False, Truth::False),
            (Truth::False, Truth::Unknown, Truth::Unknown),
            (Truth::Unknown, Truth::True, Truth::True),
            (Truth::Unknown, Truth::False, Truth::Unknown),
            (Truth::Unknown, Truth::Unknown, Truth::Unknown),
        ] {
            assert_eq!(
                left.or(right),
                expected,
                "{left:?} or {right:?} must be {expected:?}, was {:?}",
                left.or(right)
            );
        }
    }

    #[test]
    fn only_true_satisfies_a_transition() {
        assert!(Truth::True.is_satisfied());
        assert!(!Truth::False.is_satisfied());
        assert!(
            !Truth::Unknown.is_satisfied(),
            "unknown never advances a workflow"
        );
    }

    #[test]
    fn a_predicate_nested_past_the_limit_is_refused_rather_than_overflowing_the_stack() {
        // The string form: one YAML scalar, so nothing above this parser bounds it. 10 000 `not`
        // prefixes overflowed an 8 MiB stack before this bound existed.
        let stacked = format!("{}a.b", "not ".repeat(10_000));
        let error = Predicate::parse_expression(&stacked).expect_err("nesting past the limit");
        assert!(
            matches!(
                error,
                ParseError::TooDeep {
                    kind: "predicate",
                    limit: MAX_PREDICATE_DEPTH,
                    ..
                }
            ),
            "the refusal names the construct and the limit: {error:?}"
        );

        // The structured form: nested mappings.
        let mut node = Node::Bool(true);
        for _ in 0..(MAX_PREDICATE_DEPTH + 2) {
            node = Node::Map([("not".to_owned(), node)].into_iter().collect());
        }
        let error = Predicate::from_node(&node).expect_err("nesting past the limit");
        assert!(
            matches!(
                error,
                ParseError::TooDeep {
                    kind: "predicate",
                    limit: MAX_PREDICATE_DEPTH,
                    ..
                }
            ),
            "the refusal names the construct and the limit: {error:?}"
        );
    }

    #[test]
    fn the_two_predicate_forms_share_one_depth_budget() {
        // `{not: {not: "not not a.b"}}` is one predicate written two ways. Two counters would let a
        // document alternate between the forms and buy twice the depth, which is the whole bound.
        let half = MAX_PREDICATE_DEPTH / 2;
        let mut node = Node::Text(format!("{}a.b", "not ".repeat(half + 2)));
        for _ in 0..(half + 2) {
            node = Node::Map([("not".to_owned(), node)].into_iter().collect());
        }
        let error = Predicate::from_node(&node).expect_err("the two forms together exceed 32");
        assert!(
            matches!(
                error,
                ParseError::TooDeep {
                    kind: "predicate",
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_predicate_as_deep_as_anybody_writes_is_still_accepted() {
        // The failure mode of a depth bound is refusing a good document. This is deeper than
        // anything in `protocols/` or `examples/`, and it parses.
        let written = "
            all:
              - any:
                  - all:
                      - not: change.architectural
                      - service.health == healthy
                  - risk: {gte: medium}
              - task.kind: {any_of: [feature, bugfix]}
        ";
        let node: Node = serde_yaml::from_str(written).expect("well formed");
        Predicate::from_node(&node).expect("a realistic predicate");

        // And the whole budget, exactly, one level short of the refusal.
        let mut at_limit = Node::Bool(true);
        for _ in 0..MAX_PREDICATE_DEPTH {
            at_limit = Node::Map([("not".to_owned(), at_limit)].into_iter().collect());
        }
        assert_eq!(
            Predicate::from_node(&at_limit).expect("at the limit"),
            Predicate::Always,
            "{MAX_PREDICATE_DEPTH} levels is the limit, not one past it"
        );
    }

    #[test]
    fn rejects_unknown_operators_and_bad_paths() {
        let node = Node::Map(
            [(
                "risk".to_owned(),
                Node::Map(
                    [("approximately".to_owned(), Node::from("medium"))]
                        .into_iter()
                        .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        );
        let error = Predicate::from_node(&node).expect_err("unknown operator");
        assert!(error.to_string().contains("unknown operator"), "{error}");
        assert!(Predicate::parse_expression("== 0").is_err());
        assert!(Predicate::parse_expression("").is_err());
    }
}
