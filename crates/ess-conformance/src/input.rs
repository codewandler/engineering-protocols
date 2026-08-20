//! The flattener: a candidate command input, projected into a [`FactSource`].
//!
//! # The bridge that was missing
//!
//! `ess-domain` refuses a `when` whose **first** path segment is not an input field name, and says
//! in the same breath that a deeper path such as `amount.amount` "walks into a named struct, and
//! resolving that belongs with the IR". `ess-compiler` records the other half: a predicate travels
//! parsed but not resolved, and the deep-path rule is one `ess-domain` "deliberately leaves open".
//! Between the two, a guard could be written, validated, compiled and projected without anything
//! ever asking what it reads.
//!
//! [`flatten`] answers that. It walks the command's declared input types beside the candidate's
//! value tree and binds one fact per scalar leaf, so `amount.amount` on a `Money`-typed input field
//! is a bound [`FactValue`] rather than a hope.
//!
//! # The deep-path rule this implements
//!
//! One rule, applied to the resolved type at each step. A newtype is transparent because it wraps a
//! representation rather than naming a member — there is no segment to spell for the inside of one —
//! and the same walk therefore reaches `price.amount` through `Priced = newtype of Money`.
//!
//! | at | with a segment left | with none left |
//! |---|---|---|
//! | `Optional<T>` | walk into `T`; the segment is not consumed | walk into `T` |
//! | a newtype | walk into what it wraps; the segment is not consumed | walk into what it wraps |
//! | a struct | consume it as a field name | not a scalar: `a struct` |
//! | a primitive | nothing to consume: the segment is undeclared | a scalar |
//! | an enum | nothing to consume: the segment is undeclared | a scalar, as text |
//! | a union | not a scalar: `a union` | not a scalar: `a union` |
//! | `List<T>` | not a scalar: `a list` | not a scalar: `a list` |
//! | `Map<K, V>` | not a scalar: `a map` | not a scalar: `a map` |
//!
//! **Its limits, named rather than discovered later.** A union is not projected *at all*, not even
//! its tag — which is a `String` a fact could hold, and which a later wave may decide to bind as
//! `payee.kind`. A list and a map are not projected because a fact path has no index and no key
//! selector, so `lines.0.quantity` is not a path this model can spell. And the walk is bounded at
//! [`MAX_TYPE_DEPTH`], because nothing in the workspace refuses a type that refers to itself.
//!
//! # A candidate that is not a value of the input's type is refused here
//!
//! Before any predicate is evaluated. The alternative is that a misshapen candidate binds no fact,
//! the guard evaluates to `Unknown`, and the refusal blames the specification for a defect in the
//! caller — which is the same misattribution the crate exists to prevent, pointing the other way.
//! [`ShapeErrors`] accumulates, as every other validation in this workspace does.

use std::collections::BTreeMap;
use std::fmt;

use aep_domain::facts::{FactPath, FactSource, FactStore, FactValue, Scales};
use aep_domain::node::Node;
use aep_domain::predicate::{Operand, Predicate, Truth};
use ess_compiler::ir::{EssIr, ResolvedBody, ResolvedCommand, ResolvedField, ResolvedTypeRef};
use ess_domain::types::{Primitive, MAX_TYPE_DEPTH};

use crate::decision::{Decision, Reason, Unevaluable, UnknownCause};

/// Projects a candidate command input into the facts a guard reads.
///
/// The candidate is a map from input field name to value: `Node` rather than a value type of this
/// crate's own, because the workspace already has one format-neutral dynamic value and a second
/// would be a second place for `Map<String, Money>` to mean something slightly different.
///
/// Refuses a candidate that is not a value of the command's declared input type, accumulating every
/// mismatch rather than stopping at the first.
pub fn flatten<'ir>(
    ir: &'ir EssIr,
    command: &'ir ResolvedCommand,
    candidate: &BTreeMap<String, Node>,
) -> Result<InputFacts<'ir>, ShapeErrors> {
    let mut facts = FactStore::new();
    let mut errors = Vec::new();

    for field in &command.input {
        let Ok(path) = FactPath::new(&field.name) else {
            errors.push(ShapeError::UnnameableField {
                field: field.name.clone(),
            });
            continue;
        };
        match candidate.get(&field.name) {
            Some(value) => project(
                ir,
                &field.type_ref,
                value,
                &path,
                0,
                &mut facts,
                &mut errors,
            ),
            None if field.type_ref.is_optional() => {}
            None => errors.push(ShapeError::MissingField {
                at: String::new(),
                field: field.name.clone(),
            }),
        }
    }
    for supplied in candidate.keys() {
        if command.input_field(supplied).is_none() {
            errors.push(ShapeError::UndeclaredField {
                at: String::new(),
                field: supplied.clone(),
            });
        }
    }

    if errors.is_empty() {
        Ok(InputFacts {
            ir,
            command,
            facts,
            scales: Scales::default(),
        })
    } else {
        Err(ShapeErrors(errors))
    }
}

/// The facts one candidate input projects, and the guards they decide.
///
/// Holds the IR as well as the facts, because refusing well needs the *types*: "nothing is bound at
/// `amount.vat`" is the same observation whether the path names a field the candidate left out or a
/// field no type declares, and those two need opposite responses.
#[derive(Debug, Clone)]
pub struct InputFacts<'ir> {
    ir: &'ir EssIr,
    command: &'ir ResolvedCommand,
    facts: FactStore,
    scales: Scales,
}

impl<'ir> InputFacts<'ir> {
    /// Declares the ordered scales non-numeric comparisons are read against.
    ///
    /// Empty by default, and that default is a fact about the model rather than a placeholder: the
    /// ESS specification language has no scale vocabulary, so nothing an author writes can order two
    /// text values. Every `<`, `<=`, `>` and `>=` between two of them is therefore
    /// [`Reason::TextNotOrdered`] until something outside the specification — an AEP protocol, whose
    /// `scales:` this takes — supplies the ordering.
    #[must_use]
    pub fn with_scales(mut self, scales: Scales) -> Self {
        self.scales = scales;
        self
    }

    /// The command whose input this is.
    pub fn command(&self) -> &'ir ResolvedCommand {
        self.command
    }

    /// Every fact the candidate bound, in path order.
    pub fn facts(&self) -> &FactStore {
        &self.facts
    }

    /// Decides `predicate` against this candidate.
    ///
    /// `True` is [`Satisfied`](Decision::Satisfied), `False` is [`Refuted`](Decision::Refuted), and
    /// `Unknown` is [`Unevaluable`](Decision::Unevaluable) — never `Refuted`. The two differ in what
    /// a caller should do next, and there is no reading of §11's "prove or evaluate" on which an
    /// undecidable guard counts as a guard this candidate failed.
    pub fn decide(&self, predicate: &Predicate) -> Decision {
        match predicate.evaluate(self) {
            Truth::True => Decision::Satisfied,
            Truth::False => Decision::Refuted(predicate.outcome(self)),
            Truth::Unknown => {
                let mut causes = Vec::new();
                self.explain(predicate, &mut causes);
                if causes.is_empty() {
                    causes.push(UnknownCause {
                        expression: predicate.to_string(),
                        reason: Reason::Unclassified,
                    });
                }
                Decision::Unevaluable(Unevaluable {
                    predicate: predicate.to_string(),
                    command: self.command.name.to_string(),
                    causes,
                })
            }
        }
    }

    /// Collects every leaf that evaluates to `Unknown`, with the reason it does.
    ///
    /// Walks the whole tree rather than only the leaves that decided the result. A conjunction of
    /// two undecidable leaves is two defects, and reporting one of them sends the reader back for
    /// the other — invariant 3's reasoning, applied to a refusal instead of a validation.
    fn explain(&self, predicate: &Predicate, causes: &mut Vec<UnknownCause>) {
        match predicate {
            Predicate::Always | Predicate::Never => {}
            Predicate::All(children) | Predicate::Any(children) => {
                for child in children {
                    self.explain(child, causes);
                }
            }
            Predicate::Not(inner) => self.explain(inner, causes),
            leaf => {
                if leaf.evaluate(self) == Truth::Unknown {
                    self.explain_leaf(leaf, causes);
                }
            }
        }
    }

    /// The reason one undecided leaf could not be decided.
    fn explain_leaf(&self, leaf: &Predicate, causes: &mut Vec<UnknownCause>) {
        let expression = leaf.to_string();
        let mut push = |reason| {
            causes.push(UnknownCause {
                expression: expression.clone(),
                reason,
            });
        };
        match leaf {
            // The three leaves that read one path: `Unknown` means nothing is bound there.
            // `Defined` is not among them — it reports `False` for an unbound path, by design.
            Predicate::Truthy(path)
            | Predicate::AnyOf { path, .. }
            | Predicate::NoneOf { path, .. } => push(self.explain_path(path)),
            Predicate::Compare { left, op, right } => {
                let mut unresolved = false;
                for operand in [left, right] {
                    if let Operand::Fact(path) = operand {
                        if self.fact(path).is_none() {
                            unresolved = true;
                            push(self.explain_path(path));
                        }
                    }
                }
                if unresolved {
                    return;
                }
                // Both sides resolved, so the evaluator's ordering branch is what returned
                // `Unknown`. Its two cases, in its own order: two texts no scale orders, and a
                // pairing that has no ordering at all.
                let (Some(left_value), Some(right_value)) =
                    (self.resolve(left), self.resolve(right))
                else {
                    push(Reason::Unclassified);
                    return;
                };
                if !op.needs_ordering() {
                    push(Reason::Unclassified);
                    return;
                }
                match (&left_value, &right_value) {
                    (FactValue::Text(left_text), FactValue::Text(right_text)) => {
                        push(Reason::TextNotOrdered {
                            left: left_text.clone(),
                            right: right_text.clone(),
                        });
                    }
                    _ => push(Reason::TypesNotOrdered {
                        left: left_value.type_name(),
                        right: right_value.type_name(),
                    }),
                }
            }
            Predicate::Always
            | Predicate::Never
            | Predicate::All(_)
            | Predicate::Any(_)
            | Predicate::Not(_)
            | Predicate::Defined(_) => push(Reason::Unclassified),
        }
    }

    /// One operand's value, or `None` when it reads a path nothing bound.
    fn resolve(&self, operand: &Operand) -> Option<FactValue> {
        match operand {
            Operand::Fact(path) => self.fact(path),
            Operand::Literal(value) => Some(value.clone()),
        }
    }

    /// Why a path nothing bound is unbound, resolved against the command's input types.
    ///
    /// This is the whole difference between "supply a value" and "fix the specification", and it is
    /// only answerable with the types in hand — which is why [`InputFacts`] carries the IR.
    fn explain_path(&self, path: &FactPath) -> Reason {
        match resolve_path(self.ir, &self.command.input, path) {
            Target::Scalar => Reason::ValueAbsent { path: path.clone() },
            Target::Aggregate(holds) => Reason::PathNotScalar {
                path: path.clone(),
                holds,
            },
            Target::Undeclared(segment) => Reason::PathNotDeclared {
                path: path.clone(),
                segment,
            },
            Target::TooDeep => Reason::TypeTooDeep {
                path: path.clone(),
                limit: MAX_TYPE_DEPTH,
            },
        }
    }
}

impl FactSource for InputFacts<'_> {
    fn fact(&self, path: &FactPath) -> Option<FactValue> {
        self.facts.fact(path)
    }

    fn scales(&self) -> &Scales {
        &self.scales
    }
}

/// Where a fact path lands in a set of declared fields.
///
/// Public because the same question is asked of two different surfaces. A guard reads a *command's
/// input*, and an entity invariant read against a *view* asks the identical question of the fields
/// that view publishes: does `total.amount` land on something a fact value can hold, or on nothing
/// at all? One walk answers both, and a second one written beside it would be a second opinion about
/// what `Optional<Money>` exposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A primitive or an enum: something a fact value can hold.
    Scalar,
    /// A construct no fact value can hold, named as it reads in a diagnostic.
    Aggregate(&'static str),
    /// The segment that names nothing.
    Undeclared(String),
    /// The walk exceeded [`MAX_TYPE_DEPTH`].
    TooDeep,
}

impl Target {
    /// `true` only where the path lands on something a predicate can compare.
    ///
    /// Which is what "this surface publishes what the predicate reads" means: the other three cases
    /// all end in a predicate that evaluates to `Unknown`, and `Unknown` refuses.
    pub fn is_scalar(&self) -> bool {
        matches!(self, Self::Scalar)
    }
}

/// Resolves a fact path against a set of declared fields, without any value in hand.
///
/// The fields of a command's input, of a view's projection, or of anything else the model declares
/// as a flat list of named, typed members.
pub fn resolve_path(ir: &EssIr, fields: &[ResolvedField], path: &FactPath) -> Target {
    let segments = path.segments();
    // `FactPath::new` refuses an empty path, so there is always a first segment.
    let (root, rest) = segments
        .split_first()
        .expect("a fact path has at least one segment");
    match fields.iter().find(|field| &field.name == root) {
        Some(field) => walk(ir, &field.type_ref, rest, 0),
        None => Target::Undeclared(root.clone()),
    }
}

/// The deep-path rule, as the module table states it.
fn walk(ir: &EssIr, type_ref: &ResolvedTypeRef, rest: &[String], depth: usize) -> Target {
    if depth > MAX_TYPE_DEPTH {
        return Target::TooDeep;
    }
    let leaf = |rest: &[String], target: Target| match rest.split_first() {
        None => target,
        Some((segment, _)) => Target::Undeclared(segment.clone()),
    };
    match type_ref {
        ResolvedTypeRef::Optional { of } => walk(ir, of, rest, depth + 1),
        ResolvedTypeRef::List { .. } => Target::Aggregate("a list"),
        ResolvedTypeRef::Map { .. } => Target::Aggregate("a map"),
        ResolvedTypeRef::Primitive { .. } => leaf(rest, Target::Scalar),
        ResolvedTypeRef::Declared { name } => match &ir.named_type(name).body {
            ResolvedBody::Newtype { of, .. } => walk(ir, of, rest, depth + 1),
            ResolvedBody::Enum { .. } => leaf(rest, Target::Scalar),
            ResolvedBody::Union { .. } => Target::Aggregate("a union"),
            ResolvedBody::Struct { fields, .. } => match rest.split_first() {
                None => Target::Aggregate("a struct"),
                Some((segment, tail)) => match fields.iter().find(|it| &it.name == segment) {
                    Some(field) => walk(ir, &field.type_ref, tail, depth + 1),
                    None => Target::Undeclared(segment.clone()),
                },
            },
        },
    }
}

/// Binds one fact per scalar leaf of `value`, guided by `type_ref`.
fn project(
    ir: &EssIr,
    type_ref: &ResolvedTypeRef,
    value: &Node,
    path: &FactPath,
    depth: usize,
    facts: &mut FactStore,
    errors: &mut Vec<ShapeError>,
) {
    if depth > MAX_TYPE_DEPTH {
        errors.push(ShapeError::TooDeep {
            at: path.to_string(),
            limit: MAX_TYPE_DEPTH,
        });
        return;
    }
    let wrong = |errors: &mut Vec<ShapeError>, expected: String| {
        errors.push(ShapeError::WrongShape {
            at: path.to_string(),
            expected,
            found: value.type_name(),
        });
    };

    match type_ref {
        // An absent optional binds nothing, which is a fact about the candidate rather than an
        // error in it. The guard reading it then refuses as `ValueAbsent`.
        ResolvedTypeRef::Optional { of } => {
            if !matches!(value, Node::Null) {
                project(ir, of, value, path, depth + 1, facts, errors);
            }
        }
        ResolvedTypeRef::Primitive { name } => match primitive_value(*name, value) {
            Some(fact) => facts.set(path.clone(), fact),
            None => wrong(errors, name.to_string()),
        },
        // Checked for shape and projected not at all: a fact path has no index or key selector, so
        // no path can name an element.
        ResolvedTypeRef::List { .. } => {
            if !matches!(value, Node::Seq(_)) {
                wrong(errors, format!("{type_ref}"));
            }
        }
        ResolvedTypeRef::Map { .. } => {
            if !matches!(value, Node::Map(_)) {
                wrong(errors, format!("{type_ref}"));
            }
        }
        ResolvedTypeRef::Declared { name } => {
            let declared = ir.named_type(name);
            match &declared.body {
                // Transparent: a newtype wraps a representation rather than naming a member, so
                // there is no segment for the inside of one and the path does not grow.
                ResolvedBody::Newtype { of, .. } => {
                    project(ir, of, value, path, depth + 1, facts, errors);
                }
                ResolvedBody::Enum { variants } => match value.as_text() {
                    Some(text) if variants.iter().any(|variant| variant == text) => {
                        facts.set(path.clone(), FactValue::text(text));
                    }
                    _ => wrong(errors, format!("one of {}", variants.join(", "))),
                },
                // Shape only, as for a list: the tag is a text a fact could hold, and binding it is
                // a decision this gate does not take.
                ResolvedBody::Union { .. } => {
                    if !matches!(value, Node::Map(_)) {
                        wrong(errors, format!("{name} as a mapping"));
                    }
                }
                ResolvedBody::Struct { fields, .. } => {
                    let Some(entries) = value.as_map() else {
                        wrong(errors, format!("{name} as a mapping"));
                        return;
                    };
                    for field in fields {
                        let child = path.child(&field.name);
                        match entries.get(&field.name) {
                            Some(inner) => {
                                project(
                                    ir,
                                    &field.type_ref,
                                    inner,
                                    &child,
                                    depth + 1,
                                    facts,
                                    errors,
                                );
                            }
                            None if field.type_ref.is_optional() => {}
                            None => errors.push(ShapeError::MissingField {
                                at: path.to_string(),
                                field: field.name.clone(),
                            }),
                        }
                    }
                    for supplied in entries.keys() {
                        if !fields.iter().any(|field| &field.name == supplied) {
                            errors.push(ShapeError::UndeclaredField {
                                at: path.to_string(),
                                field: supplied.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// The fact value a primitive-typed node projects to, or `None` when the node is the wrong shape.
///
/// Crate-visible rather than private because [`Holds::admits`](crate::Holds::admits) asks the same
/// question of an event's payload. One table, so that a payload and a command input cannot come to
/// different conclusions about whether `1.5` is an `Integer`.
pub(crate) fn primitive_value(primitive: Primitive, value: &Node) -> Option<FactValue> {
    match (primitive, value) {
        (Primitive::Boolean, Node::Bool(flag)) => Some(FactValue::Bool(*flag)),
        (Primitive::Decimal, Node::Number(number)) => Some(FactValue::Number(*number)),
        // An integer that is not integral is refused rather than rounded: a candidate binding `1.5`
        // to an `Integer` would decide `quantity == 1` differently from the system it is testing.
        (Primitive::Integer, Node::Number(number)) if number.is_integral() => {
            Some(FactValue::Number(*number))
        }
        (
            Primitive::String
            | Primitive::Timestamp
            | Primitive::Duration
            | Primitive::Uuid
            | Primitive::Bytes,
            Node::Text(text),
        ) => Some(FactValue::text(text)),
        _ => None,
    }
}

/// A candidate that is not a value of the command's declared input type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeError {
    /// The candidate supplied nothing for a field that is not optional.
    MissingField {
        /// The path of the value that should have carried it; empty at the input's root.
        at: String,
        /// The field's name.
        field: String,
    },
    /// The candidate supplied a field no type declares.
    UndeclaredField {
        /// The path of the value that carried it; empty at the input's root.
        at: String,
        /// The name supplied.
        field: String,
    },
    /// A value is not of the shape its declared type calls for.
    WrongShape {
        /// Where it sits.
        at: String,
        /// What the type calls for.
        expected: String,
        /// What the candidate carried.
        found: &'static str,
    },
    /// Projection walked deeper than [`MAX_TYPE_DEPTH`], which a type referring to itself is the
    /// only way to do.
    TooDeep {
        /// Where it gave up.
        at: String,
        /// The limit it exceeded.
        limit: usize,
    },
    /// An input field whose name cannot be spelled as a fact path, so no predicate could read it.
    ///
    /// Unreachable from a document — the parser holds a field name to
    /// [`Field::PATTERN`](ess_domain::types::Field::PATTERN), which is stricter than a fact path
    /// segment. Reachable from a `CommandSpec` built in code, which is how every fixture in this
    /// workspace's compiler tests is built.
    UnnameableField {
        /// The name that cannot be a path.
        field: String,
    },
}

impl fmt::Display for ShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { at, field } => {
                write!(f, "{}: nothing supplied for `{field}`", at_or_root(at))
            }
            Self::UndeclaredField { at, field } => {
                write!(f, "{}: `{field}` is not declared there", at_or_root(at))
            }
            Self::WrongShape {
                at,
                expected,
                found,
            } => write!(f, "{at}: expected {expected}, found {found}"),
            Self::TooDeep { at, limit } => {
                write!(f, "{at}: walked deeper than {limit} types")
            }
            Self::UnnameableField { field } => {
                write!(f, "`{field}` cannot be spelled as a fact path")
            }
        }
    }
}

/// How a path reads in a message when it is the input's root.
fn at_or_root(at: &str) -> &str {
    if at.is_empty() {
        "the input"
    } else {
        at
    }
}

/// Every way a candidate failed to be a value of the input's type.
///
/// Accumulates, so a candidate with three wrong fields reports three — invariant 3, on the path a
/// runner takes rather than the one a document does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeErrors(Vec<ShapeError>);

impl ShapeErrors {
    /// Every mismatch, in the order the input declares the fields it is about.
    pub fn iter(&self) -> impl Iterator<Item = &ShapeError> {
        self.0.iter()
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` when there are none, which [`flatten`] never returns.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ShapeErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ShapeErrors {}

#[cfg(test)]
mod tests {
    use super::*;

    use aep_domain::facts::Number;

    fn number(value: f64) -> Node {
        Node::Number(Number::new(value).expect("a finite number"))
    }

    #[test]
    fn every_primitive_projects_to_the_one_fact_value_that_can_hold_it() {
        let text = Node::Text("x".to_owned());
        let table: [(Primitive, Node, Option<FactValue>); 8] = [
            (
                Primitive::Boolean,
                Node::Bool(true),
                Some(FactValue::Bool(true)),
            ),
            (
                Primitive::Decimal,
                number(1.5),
                Some(FactValue::Number(Number::new(1.5).expect("finite"))),
            ),
            (
                Primitive::Integer,
                number(2.0),
                Some(FactValue::Number(Number::from(2_i64))),
            ),
            (Primitive::String, text.clone(), Some(FactValue::text("x"))),
            (Primitive::Uuid, text.clone(), Some(FactValue::text("x"))),
            (
                Primitive::Timestamp,
                text.clone(),
                Some(FactValue::text("x")),
            ),
            (
                Primitive::Duration,
                text.clone(),
                Some(FactValue::text("x")),
            ),
            (Primitive::Bytes, text, Some(FactValue::text("x"))),
        ];
        for (primitive, node, expected) in table {
            assert_eq!(
                primitive_value(primitive, &node),
                expected,
                "{primitive} projects from {}",
                node.type_name()
            );
        }
    }

    #[test]
    fn a_primitive_refuses_a_node_of_the_wrong_shape_rather_than_coercing_it() {
        assert_eq!(
            primitive_value(Primitive::Boolean, &Node::Text("true".to_owned())),
            None,
            "the string `true` is not a boolean; accepting it would let a candidate decide \
             `express == true` in a way the system under test does not"
        );
        assert_eq!(
            primitive_value(Primitive::Integer, &number(1.5)),
            None,
            "rounding here would decide `quantity == 1` differently from the system under test"
        );
        assert_eq!(primitive_value(Primitive::String, &number(1.0)), None);
        assert_eq!(primitive_value(Primitive::Decimal, &Node::Null), None);
    }

    #[test]
    fn shape_errors_render_one_per_line_and_name_the_input_root_by_name() {
        let errors = ShapeErrors(vec![
            ShapeError::MissingField {
                at: String::new(),
                field: "currency".to_owned(),
            },
            ShapeError::UndeclaredField {
                at: "amount".to_owned(),
                field: "vat".to_owned(),
            },
        ]);
        let rendered = errors.to_string();

        assert_eq!(errors.len(), 2);
        assert!(!errors.is_empty());
        assert_eq!(rendered.lines().count(), 2);
        assert!(
            rendered.contains("the input: nothing supplied for `currency`"),
            "an empty path is the input itself, not a blank: {rendered}"
        );
        assert!(rendered.contains("amount: `vat` is not declared there"));
    }
}
