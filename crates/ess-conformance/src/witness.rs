//! Where a scenario's concrete values come from, and where none can be produced.
//!
//! Design §11. A scenario needs an actual command input, and the rule it is built under is one
//! sentence:
//!
//! > Never generate an arbitrary value and claim it satisfies an outcome predicate unless the
//! > generator can prove or evaluate that it does.
//!
//! [`crate::input`] makes the second half available, so this module does not have to prove anything:
//! it produces candidates, and [`InputFacts::decide`](crate::InputFacts::decide) says whether one
//! reaches the branch. What is left is choosing candidates that are *worth* deciding, in a bounded,
//! deterministic order, without a constraint solver — which §11 names as a later extension and not a
//! requirement of the first closed loop.
//!
//! # The strategy, in four rules
//!
//! 1. **One base witness per command**, built from the declared input types alone. Every field is
//!    filled, optionals included, so a guard reading one is decided rather than
//!    [`Unknown`](crate::Reason::ValueAbsent).
//! 2. **A text witness is its own fact path.** `contact` carries `"contact"` and
//!    `alternate_contact` carries `"alternate_contact"`, so two same-typed fields are never
//!    interchangeable — which is the only way a swapped binding mapping is a detectable fault rather
//!    than an invisible one (`examples/oracle-fixture/README.md`).
//! 3. **Alternatives come from the guard, not from imagination.** The values a candidate varies are
//!    the fact paths the guard actually reads, and the values it tries are the literals the guard
//!    itself writes, one either side. `amount.amount > 0` is met by `1` and refuted by `0`, and
//!    neither number was invented.
//! 4. **Bounded.** At most [`MAX_CANDIDATES`] inputs per outcome. Exhausting them is a refusal that
//!    says how many were tried, never a longer search.
//!
//! # What has no witness
//!
//! A type that refers to itself, and a field whose name cannot be spelled as a fact path. Both are
//! [`WitnessGap`], and both are reported rather than worked around. Everything else in the model has
//! a value: a list is `[]`, a map is `{}`, a union is its first variant in the encoding
//! `ess-gen` publishes, and an enum is its first declared variant.
//!
//! **A witness is only as good as the type it is built from.** `currency: String` with no invariant
//! gets the text `"amount.currency"`, because that is every value the specification permits. An
//! implementation that refuses it is enforcing a rule the specification does not state, and the
//! suite catching that is the suite working.
//!
//! **A whole number is written `1.0` in the artifact.** [`Node::Number`] holds an
//! [`aep_domain::facts::Number`], which is an `f64`, so an `Integer` witness serialises with a
//! fractional part it does not have. That is the value type's shape rather than a choice here — the
//! flattener refuses a genuinely fractional value for an `Integer`
//! ([`crate::input`]) — but a runner handing this straight to a JSON-typed target has to normalise
//! it, and that is worth knowing before the runner is written.
//!
//! # This walk mirrors the flattener's
//!
//! [`crate::input`] walks a declared type beside a candidate *value* to bind facts; this walks the
//! same type to *build* one. Same table, opposite directions, no shared step to factor out — with
//! one difference that matters: nothing inside a union is recorded as a leaf, because a fact path
//! cannot reach inside one, so no guard can be varied there.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use aep_domain::facts::{FactPath, FactValue, Number};
use aep_domain::node::Node;
use aep_domain::predicate::{Operand, Predicate};
use ess_compiler::ir::{EssIr, ResolvedBody, ResolvedCommand, ResolvedTypeRef};
use ess_domain::types::{Primitive, MAX_TYPE_DEPTH};

/// How many candidate inputs one outcome is tried against before synthesis refuses.
///
/// A bound rather than a budget to spend: §11 asks for a constrained strategy, and the honest
/// failure of one is "the values I know how to try did not satisfy this guard", reported with the
/// number. A larger number would turn a specification that needs a solver into a slower build that
/// still cannot say so.
pub const MAX_CANDIDATES: usize = 64;

/// The wire key a union variant's value is carried under.
///
/// `{"kind": "person", "value": …}` — the encoding `ess-gen` publishes in
/// `generated/schema/types/billing.invoice.Payee.schema.json`, read from there rather than decided
/// again here, because a witness in a second encoding is a witness no target can accept.
const UNION_VALUE: &str = "value";

/// A construct of the input that no safe value could be produced for.
///
/// Two of them exist, and neither is a defect in this module: a type that refers to itself has no
/// finite value, and a field name that is not a fact path is a `CommandSpec` no document can
/// produce. Both travel as a refusal (§11) rather than as a value that would have been a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessGap {
    /// Where in the input it sits, as the path a guard would read it by.
    pub path: String,
    /// The type that has no witness, rendered.
    pub type_ref: String,
    /// Why it has none.
    pub reason: &'static str,
}

impl fmt::Display for WitnessGap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is `{}`, which {}",
            self.path, self.type_ref, self.reason
        )
    }
}

/// What a fact path lands on in a command's input, when a candidate can vary it.
///
/// Only the four shapes a [`FactValue`] can hold, because those are the only ones a guard can
/// compare — the flattener binds nothing else, so varying anything else changes no decision.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Leaf {
    /// A `Decimal` or an `Integer`; `integral` when a fractional value would be refused.
    Number {
        /// `true` for `Integer`, where `1.5` is not a value of the type.
        integral: bool,
    },
    /// Anything a fact reads as text.
    Text,
    /// A `Boolean`.
    Bool,
    /// An enum, whose alternatives are its own declared variants.
    Enum {
        /// The variants, in declaration order.
        variants: Vec<String>,
    },
}

/// Every candidate input for one command, in the order synthesis tries them.
///
/// The first is always the base witness, so a guard that any well-typed input satisfies is decided
/// on the first try and the committed suite carries the plainest values the specification allows.
///
/// # Errors
///
/// [`WitnessGap`] when some field of the input has no safe value at all.
pub fn candidates(
    ir: &EssIr,
    command: &ResolvedCommand,
    guards: &[&Predicate],
) -> Result<Vec<BTreeMap<String, Node>>, WitnessGap> {
    let mut builder = Builder::new(ir);
    let base = builder.input(command, &BTreeMap::new())?;

    let mut ladders: Vec<(FactPath, Vec<Node>)> = Vec::new();
    for path in read_paths(guards) {
        let Some(leaf) = builder.leaves.get(&path) else {
            // A path the guard reads and the input does not bind: a list, a map, the inside of a
            // union, or a segment no type declares. No value this module chooses changes that, and
            // `InputFacts::decide` is what names which of them it is.
            continue;
        };
        let alternatives = alternatives(leaf, &literals_at(guards, &path));
        if !alternatives.is_empty() {
            ladders.push((path, alternatives));
        }
    }

    let mut inputs = vec![base];
    for index in 1..combinations(&ladders) {
        let mut overrides = BTreeMap::new();
        let mut remaining = index;
        for (path, alternatives) in &ladders {
            let radix = alternatives.len() + 1;
            let chosen = remaining % radix;
            remaining /= radix;
            if chosen > 0 {
                overrides.insert(path.clone(), alternatives[chosen - 1].clone());
            }
        }
        inputs.push(builder.input(command, &overrides)?);
    }
    Ok(inputs)
}

/// How many candidates the ladders describe, capped at [`MAX_CANDIDATES`].
///
/// Each ladder has one more position than it has alternatives, because position zero is "leave the
/// base value alone". The product is saturating: a command with many varied fields is bounded, not
/// overflowed.
fn combinations(ladders: &[(FactPath, Vec<Node>)]) -> usize {
    ladders
        .iter()
        .fold(1usize, |total, (_, alternatives)| {
            total.saturating_mul(alternatives.len() + 1)
        })
        .min(MAX_CANDIDATES)
}

/// Every fact path the guards read, in path order.
fn read_paths(guards: &[&Predicate]) -> BTreeSet<FactPath> {
    guards
        .iter()
        .flat_map(|guard| guard.fact_paths())
        .cloned()
        .collect()
}

/// Every literal the guards compare `path` against, in the order they write them.
fn literals_at(guards: &[&Predicate], path: &FactPath) -> Vec<FactValue> {
    let mut found = Vec::new();
    for guard in guards {
        collect_literals(guard, path, &mut found);
    }
    found
}

/// The walk behind [`literals_at`].
fn collect_literals(predicate: &Predicate, path: &FactPath, found: &mut Vec<FactValue>) {
    match predicate {
        Predicate::All(children) | Predicate::Any(children) => {
            for child in children {
                collect_literals(child, path, found);
            }
        }
        Predicate::Not(inner) => collect_literals(inner, path, found),
        Predicate::Compare { left, op: _, right } => {
            for (operand, other) in [(left, right), (right, left)] {
                if matches!(operand, Operand::Fact(read) if read == path) {
                    if let Operand::Literal(value) = other {
                        found.push(value.clone());
                    }
                }
            }
        }
        Predicate::AnyOf { path: read, values } | Predicate::NoneOf { path: read, values } => {
            if read == path {
                found.extend(values.iter().cloned());
            }
        }
        Predicate::Always | Predicate::Never | Predicate::Truthy(_) | Predicate::Defined(_) => {}
    }
}

/// The values one leaf is tried at, beside its base value, in the order they are tried.
///
/// Derived from what the guard writes, never from a range this module imagines. The one addition is
/// `0` and `-1` for a number: a guard that compares two facts writes no literal at all, and those
/// two are the values that decide sign and truthiness — which is what `> 0`, `>= 0` and a bare
/// truthiness test are made of.
fn alternatives(leaf: &Leaf, literals: &[FactValue]) -> Vec<Node> {
    let mut values = Vec::new();
    let mut push = |value: Node| {
        if !values.contains(&value) {
            values.push(value);
        }
    };
    match leaf {
        Leaf::Number { integral } => {
            let base = number(BASE_NUMBER);
            let mut numbers: Vec<f64> = Vec::new();
            for literal in literals {
                if let Some(number) = literal.as_number() {
                    numbers.extend([number.get(), number.get() + 1.0, number.get() - 1.0]);
                }
            }
            numbers.extend([0.0, -1.0]);
            for value in numbers {
                let Ok(candidate) = Number::new(value) else {
                    continue;
                };
                if (!*integral || candidate.is_integral()) && candidate != base {
                    push(Node::Number(candidate));
                }
            }
        }
        Leaf::Bool => push(Node::Bool(!BASE_BOOL)),
        Leaf::Text => {
            for literal in literals {
                if let Some(text) = literal.as_text() {
                    push(Node::Text(text.to_owned()));
                }
            }
        }
        Leaf::Enum { variants } => {
            for variant in variants.iter().skip(1) {
                push(Node::Text(variant.clone()));
            }
        }
    }
    values
}

/// The number every numeric witness starts at.
const BASE_NUMBER: f64 = 1.0;

/// The boolean every boolean witness starts at.
const BASE_BOOL: bool = true;

/// The instant every `Timestamp` witness carries. A constant, so nothing here reads a clock.
const BASE_TIMESTAMP: &str = "2020-01-01T00:00:00Z";

/// The length every `Duration` witness carries.
const BASE_DURATION: &str = "PT1S";

/// The bytes every `Bytes` witness carries: one zero byte, base64.
const BASE_BYTES: &str = "AA==";

/// Builds one input from the declared types, recording where a candidate could vary it.
struct Builder<'ir> {
    ir: &'ir EssIr,
    /// Every scalar the input holds, by the fact path that reads it.
    leaves: BTreeMap<FactPath, Leaf>,
}

impl<'ir> Builder<'ir> {
    fn new(ir: &'ir EssIr) -> Self {
        Self {
            ir,
            leaves: BTreeMap::new(),
        }
    }

    /// One whole command input, with `overrides` applied at the paths they name.
    fn input(
        &mut self,
        command: &ResolvedCommand,
        overrides: &BTreeMap<FactPath, Node>,
    ) -> Result<BTreeMap<String, Node>, WitnessGap> {
        let mut input = BTreeMap::new();
        for field in &command.input {
            let path = FactPath::new(&field.name).map_err(|_| WitnessGap {
                path: field.name.clone(),
                type_ref: field.type_ref.to_string(),
                reason: "is named in a way no fact path can spell, so no guard could read it",
            })?;
            let value = self.value(&field.type_ref, &path, overrides, 0, true)?;
            input.insert(field.name.clone(), value);
        }
        Ok(input)
    }

    /// One value of `type_ref`, at `path`.
    ///
    /// `record` is false inside a union, where no fact path reaches — so nothing there is offered
    /// to a candidate ladder that could not change a decision anyway.
    fn value(
        &mut self,
        type_ref: &ResolvedTypeRef,
        path: &FactPath,
        overrides: &BTreeMap<FactPath, Node>,
        depth: usize,
        record: bool,
    ) -> Result<Node, WitnessGap> {
        if depth > MAX_TYPE_DEPTH {
            return Err(WitnessGap {
                path: path.to_string(),
                type_ref: type_ref.to_string(),
                reason: "refers to itself, so it has no finite value to send",
            });
        }
        match type_ref {
            // Transparent, exactly as the flattener reads them: neither an optional nor a newtype
            // has a segment of its own, so the path does not grow and an optional is filled rather
            // than left out — an absent value is a guard nothing can decide.
            ResolvedTypeRef::Optional { of } => self.value(of, path, overrides, depth + 1, record),
            ResolvedTypeRef::List { .. } => Ok(Node::Seq(Vec::new())),
            ResolvedTypeRef::Map { .. } => Ok(Node::Map(BTreeMap::new())),
            ResolvedTypeRef::Primitive { name } => {
                if record {
                    self.leaves.insert(path.clone(), Leaf::of_primitive(*name));
                }
                Ok(overrides
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| primitive_value(*name, path)))
            }
            ResolvedTypeRef::Declared { name } => {
                // Read through the IR reference rather than through `self`, so what comes back
                // lives as long as the IR and the recursive calls below can still borrow `self`.
                let ir = self.ir;
                match &ir.named_type(name).body {
                    ResolvedBody::Newtype { of, .. } => {
                        self.value(of, path, overrides, depth + 1, record)
                    }
                    ResolvedBody::Enum { variants } => {
                        if record {
                            self.leaves.insert(
                                path.clone(),
                                Leaf::Enum {
                                    variants: variants.clone(),
                                },
                            );
                        }
                        let first = variants.first().cloned().unwrap_or_default();
                        Ok(overrides.get(path).cloned().unwrap_or(Node::Text(first)))
                    }
                    ResolvedBody::Union { tag, variants } => {
                        let Some((label, variant)) = variants.iter().next() else {
                            return Ok(Node::Map(BTreeMap::new()));
                        };
                        let inner = self.value(variant, path, overrides, depth + 1, false)?;
                        Ok(Node::Map(BTreeMap::from([
                            (tag.clone(), Node::Text(label.clone())),
                            (UNION_VALUE.to_owned(), inner),
                        ])))
                    }
                    ResolvedBody::Struct { fields, .. } => {
                        let mut value = BTreeMap::new();
                        for field in fields {
                            let child = path.child(&field.name);
                            let inner =
                                self.value(&field.type_ref, &child, overrides, depth + 1, record)?;
                            value.insert(field.name.clone(), inner);
                        }
                        Ok(Node::Map(value))
                    }
                }
            }
        }
    }
}

impl Leaf {
    /// What a candidate may vary a primitive to.
    fn of_primitive(primitive: Primitive) -> Self {
        match primitive {
            Primitive::Boolean => Self::Bool,
            Primitive::Integer => Self::Number { integral: true },
            Primitive::Decimal => Self::Number { integral: false },
            Primitive::String
            | Primitive::Timestamp
            | Primitive::Duration
            | Primitive::Uuid
            | Primitive::Bytes => Self::Text,
        }
    }
}

/// The base value of one primitive, at one path.
fn primitive_value(primitive: Primitive, path: &FactPath) -> Node {
    match primitive {
        Primitive::Boolean => Node::Bool(BASE_BOOL),
        Primitive::Integer | Primitive::Decimal => Node::Number(number(BASE_NUMBER)),
        Primitive::Timestamp => Node::Text(BASE_TIMESTAMP.to_owned()),
        Primitive::Duration => Node::Text(BASE_DURATION.to_owned()),
        Primitive::Bytes => Node::Text(BASE_BYTES.to_owned()),
        Primitive::Uuid => Node::Text(uuid(path)),
        // The path itself, so two fields of one type never carry one value — see rule 2.
        Primitive::String => Node::Text(path.to_string()),
    }
}

/// A number that is known to be finite.
fn number(value: f64) -> Number {
    Number::new(value).unwrap_or_else(|error| panic!("a witness is a finite number: {error}"))
}

/// A syntactically well-formed UUID, derived from the path it sits at.
///
/// Derived rather than fixed so two `Uuid` fields of one input differ, and derived by hashing rather
/// than counting so inserting a field renumbers nothing. Version 4 and the standard variant, because
/// a target that parses it should not have to accept a shape no generator would produce.
///
/// It identifies nothing, and synthesis never asks it to: an outcome that needs an instance that
/// already exists is refused before a witness is built.
fn uuid(path: &FactPath) -> String {
    let digest = fnv1a(path.to_string().as_bytes()) & 0xffff_ffff_ffff;
    format!("00000000-0000-4000-8000-{digest:012x}")
}

/// FNV-1a, 64-bit.
///
/// Written out rather than taken as a dependency: `ess-gen` has `sha2` for a provenance digest that
/// is published and compared, and this is neither — it is a way to spread twelve hexadecimal digits
/// over the fields of one command, deterministically and with no clock. Twelve lines against a
/// crate in every downstream lockfile is the trade this repository already records making.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    use aep_domain::facts::FactValue;

    fn path(value: &str) -> FactPath {
        FactPath::new(value).expect("a valid fact path")
    }

    #[test]
    fn a_text_witness_is_its_own_path_so_two_fields_of_one_type_never_agree() {
        // The property `examples/oracle-fixture/` exists for: `contact` and `alternate_contact` are
        // both `oracle.order.Email`, so a binding that maps the wrong one is only detectable if the
        // two carry different values.
        let contact = primitive_value(Primitive::String, &path("contact"));
        let alternate = primitive_value(Primitive::String, &path("alternate_contact"));

        assert_eq!(contact, Node::Text("contact".to_owned()));
        assert_ne!(
            contact, alternate,
            "a constant witness would make a swapped mapping invisible"
        );
    }

    #[test]
    fn two_uuid_witnesses_differ_and_neither_moves_when_a_third_field_appears() {
        let first = primitive_value(Primitive::Uuid, &path("invoice_id"));
        let second = primitive_value(Primitive::Uuid, &path("order_id"));

        assert_ne!(first, second, "two ids of one input must not collide");
        assert_eq!(
            first,
            primitive_value(Primitive::Uuid, &path("invoice_id")),
            "a witness derived from a counter would move when a field is inserted before it"
        );
        let Node::Text(rendered) = first else {
            panic!("a uuid witness is text")
        };
        assert_eq!(rendered.len(), 36, "{rendered} is not uuid-shaped");
        assert_eq!(
            rendered.chars().nth(14),
            Some('4'),
            "the version nibble is 4: {rendered}"
        );
    }

    #[test]
    fn the_alternatives_for_a_number_are_the_guards_own_literals_either_side() {
        let alternatives = alternatives(
            &Leaf::Number { integral: false },
            &[FactValue::number(0.0).expect("finite")],
        );

        assert_eq!(
            alternatives,
            vec![Node::Number(number(0.0)), Node::Number(number(-1.0))],
            "the literal, then either side of it, with `0 + 1` dropped as the base value"
        );
        assert!(
            !alternatives.contains(&Node::Number(number(BASE_NUMBER))),
            "the base value is the first candidate, so repeating it wastes a try"
        );
    }

    #[test]
    fn an_integer_leaf_is_never_offered_a_fractional_candidate() {
        // The flattener refuses `1.5` for an `Integer` rather than rounding it, so a candidate that
        // carried one would be rejected as misshapen before any guard was decided.
        let literal = FactValue::number(0.5).expect("finite");
        let literals = std::slice::from_ref(&literal);
        let integral = alternatives(&Leaf::Number { integral: true }, literals);
        let decimal = alternatives(&Leaf::Number { integral: false }, literals);

        assert_eq!(
            integral,
            vec![Node::Number(number(0.0)), Node::Number(number(-1.0))],
            "only the two whole numbers survive"
        );
        assert!(
            decimal.contains(&Node::Number(number(0.5))),
            "and the fixture's literal is a candidate where the type allows it: {decimal:?}"
        );
    }

    #[test]
    fn an_enum_offers_every_variant_it_declares_and_the_first_one_only_once() {
        let alternatives = alternatives(
            &Leaf::Enum {
                variants: vec!["Email".to_owned(), "Post".to_owned(), "Portal".to_owned()],
            },
            &[],
        );

        assert_eq!(
            alternatives,
            vec![
                Node::Text("Post".to_owned()),
                Node::Text("Portal".to_owned())
            ],
            "`Email` is the base value, so the ladder holds the other two"
        );
    }

    #[test]
    fn the_candidate_count_is_bounded_however_many_fields_a_guard_reads() {
        let ladder = (
            path("a"),
            vec![Node::Bool(false), Node::Bool(true), Node::Null],
        );
        let many: Vec<(FactPath, Vec<Node>)> = (0..40).map(|_| ladder.clone()).collect();

        assert_eq!(
            combinations(&many),
            MAX_CANDIDATES,
            "4^40 must saturate rather than overflow or be enumerated"
        );
        assert_eq!(combinations(&[]), 1, "one base witness and nothing else");
    }
}
