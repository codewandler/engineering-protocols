//! Views: what a system promises can be observed about it, and how soon.
//!
//! # Why consistency is part of the model
//!
//! §4.6 calls a view a "stable observable" and §18's conformance scenario asserts one immediately
//! after the command that caused it. If the view is a projection — the normal case — that assertion
//! passes on a laptop and flakes in CI, and the fix everyone reaches for is a sleep, which makes the
//! suite a test of the machine it runs on rather than of the system.
//!
//! So a view declares its [`Consistency`], and [`ViewSpec::assertion_style`] turns it into the block
//! a generated scenario must use:
//!
//! | [`Consistency`] | [`AssertionStyle`] | scenario |
//! |---|---|---|
//! | [`ReadYourWrites`](Consistency::ReadYourWrites) | [`Expect`](AssertionStyle::Expect) | `expect:` — the view is current the moment the command returns |
//! | [`Eventual`](Consistency::Eventual) | [`Eventually`](AssertionStyle::Eventually) | `eventually:` — the runner retries until the projection catches up |
//!
//! The default is [`Eventual`](Consistency::Eventual), and the asymmetry is the reason: declaring
//! read-your-writes and being wrong produces a suite that fails at random, which costs a person a
//! day and eventually costs the suite its credibility; declaring eventual and being wrong produces a
//! suite that is slower and still correct. The cheap mistake is the default.
//!
//! # What each rejection is called
//!
//! [`ValidationCode`] belongs to `aep-domain` and is closed to this crate, so ESS reuses the nearest
//! protocol code rather than opening a parallel vocabulary:
//!
//! | rule | code |
//! |---|---|
//! | a view projects nothing | [`EmptyDeclaration`](ValidationCode::EmptyDeclaration) |
//! | its source entity, a field it projects, or a projected field's type is not declared | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | a filter compares a field to a value the field's type does not have | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | a projected field's type disagrees with the entity's | [`TypeMismatch`](ValidationCode::TypeMismatch) |
//! | a filter reads something the source does not have | [`UnobservableFact`](ValidationCode::UnobservableFact) |
//! | a field is projected twice | [`DuplicateDeclaration`](ValidationCode::DuplicateDeclaration) |

use std::collections::BTreeMap;
use std::fmt;

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};
use aep_domain::facts::{FactPath, FactValue};
use aep_domain::predicate::{Operand, Predicate};

use crate::name::{Naming, QualifiedName};
use crate::types::{Field, TypeBody, TypeRef, TypeRegistry};

/// How soon a view reflects a command that has already returned.
///
/// This decides whether a generated scenario asserts the view with `expect` or with `eventually` —
/// which is the entire reason the field exists. Getting it wrong in the model is how a generated
/// suite acquires a sleep.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Consistency {
    /// The view is current as soon as the command that changed it returns.
    ///
    /// Only for an implementation that reads the same store it wrote, or one that carries a
    /// consistency token from the write to the read.
    ReadYourWrites,
    /// The view catches up some time after the command returns.
    ///
    /// The default, because assuming read-your-writes and being wrong produces a flaky suite,
    /// while assuming eventual and being wrong produces a slower but correct one.
    #[default]
    Eventual,
}

impl Consistency {
    /// The consistency as written in a document.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadYourWrites => "read_your_writes",
            Self::Eventual => "eventual",
        }
    }

    /// How a generated scenario must assert a view with this consistency.
    pub fn assertion_style(self) -> AssertionStyle {
        match self {
            Self::ReadYourWrites => AssertionStyle::Expect,
            Self::Eventual => AssertionStyle::Eventually,
        }
    }
}

impl fmt::Display for Consistency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The block a generated conformance scenario puts a view assertion in (§18).
///
/// Derived from the model rather than chosen per assertion by whoever writes the scenario, because a
/// choice made per assertion is a choice made wrong eventually.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AssertionStyle {
    /// `expect:` — assert once, immediately after the command.
    Expect,
    /// `eventually:` — retry until the projection catches up, with no sleep and no fixed delay.
    Eventually,
}

impl AssertionStyle {
    /// The scenario key this style writes, such as `eventually`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expect => "expect",
            Self::Eventually => "eventually",
        }
    }

    /// `true` when the runner must retry rather than assert once.
    pub fn is_retried(self) -> bool {
        self == Self::Eventually
    }
}

impl fmt::Display for AssertionStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What [`ViewSpec::validate`] needs to know about the entities a view may project.
///
/// A trait rather than a reference to the entity model, so that this module does not depend on the
/// shape of a type it does not own, and so that a caller can supply whatever it already has.
///
/// The field list an implementation returns is the entity's **observable surface**, not only its
/// declared fields: it must include the identity, and the state where the entity has a state
/// machine. A view projects and filters all three — `InvoiceById` names `invoice_id`, and
/// `OutstandingInvoices` filters on `state` — and a view is the one place where the difference
/// between them does not matter.
///
/// The state must carry the entity's own synthesised state type
/// ([`EntitySpec::state_type`](crate::entity::EntitySpec::state_type)) rather than a bare string:
/// that is what lets [`ViewSpec::validate`] check `state == Issued` against the lifecycle's own
/// names, so a filter that can never match is refused instead of generated.
pub trait EntityFields {
    /// The observable fields of the entity with this name, or `None` when no such entity exists.
    fn entity_fields(&self, name: &QualifiedName) -> Option<&[Field]>;

    /// Every declared entity name, for a diagnostic that says what was available instead.
    fn entity_names(&self) -> Vec<String>;
}

impl EntityFields for BTreeMap<QualifiedName, Vec<Field>> {
    fn entity_fields(&self, name: &QualifiedName) -> Option<&[Field]> {
        self.get(name).map(Vec::as_slice)
    }

    fn entity_names(&self) -> Vec<String> {
        self.keys().map(ToString::to_string).collect()
    }
}

/// A declared projection of an entity: the part of it the outside world is promised.
///
/// A view does not require the implementation to use CQRS (§4.6). It says what can be observed, not
/// how the observation is served.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(into = "RawViewSpec")]
pub struct ViewSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// The entity it projects.
    pub source: QualifiedName,
    /// What it exposes, in declaration order.
    pub fields: Vec<Field>,
    /// Which instances it contains. Absent means all of them.
    pub filter: Option<Predicate>,
    /// How soon it reflects a command that has already returned.
    pub consistency: Consistency,
    /// What it is called on the wire and shown as.
    pub naming: Naming,
}

impl ViewSpec {
    /// The projected field with this name.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// How a generated scenario must assert this view.
    ///
    /// Exposed on the model so no generator has to decide it: an `eventual` view asserted with
    /// `expect` races the projection, and the usual repair — a sleep — makes the suite test the
    /// machine it runs on.
    pub fn assertion_style(&self) -> AssertionStyle {
        self.consistency.assertion_style()
    }

    /// Checks everything that can be checked without knowing what the domain declares.
    fn validate_shape(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let at = |suffix: &str| format!("view.{}.{suffix}", self.name);

        if self.fields.is_empty() {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    at("fields"),
                    format!(
                        "`{}` projects no fields, so it observes nothing and no scenario can \
                         assert anything about it",
                        self.name
                    ),
                )
                .with_hint("project at least the identity a scenario looks the entity up by"),
            );
        }

        let mut seen = std::collections::BTreeSet::new();
        for (index, field) in self.fields.iter().enumerate() {
            if !seen.insert(field.name.as_str()) {
                errors.push(ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    at(&format!("fields[{index}]")),
                    format!("field `{}` is projected more than once", field.name),
                ));
            }
        }

        errors
    }

    /// Checks this view against the types and entities the system declares.
    ///
    /// Refuses a source entity that does not exist, a projected field the source does not have, a
    /// projected field whose type the source's cannot fill, a filter reading a field that is not on
    /// the source, and a filter comparing a field to a value its type does not have.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        entities: &impl EntityFields,
    ) -> Result<(), ValidationErrors> {
        let mut errors = self.validate_shape();
        let at = |suffix: &str| format!("view.{}.{suffix}", self.name);

        let Some(source_fields) = entities.entity_fields(&self.source) else {
            errors.push(
                ValidationError::new(
                    ValidationCode::UndeclaredReference,
                    at("source"),
                    format!(
                        "`{}` is not a declared entity, so there is nothing for `{}` to project",
                        self.source, self.name
                    ),
                )
                .with_hint(format!(
                    "declared entities: {}",
                    join(entities.entity_names())
                )),
            );
            // Every remaining check reads the source's fields, so reporting them against an entity
            // that does not exist would be noise, not accumulation.
            return errors.into_result(());
        };

        let known = |name: &str| source_fields.iter().find(|field| field.name == name);

        for (index, field) in self.fields.iter().enumerate() {
            errors.extend(types.resolve(&field.type_ref, &at(&format!("fields[{index}].type"))));

            let Some(source_field) = known(&field.name) else {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        at(&format!("fields[{index}]")),
                        format!(
                            "`{}` has no field `{}`, so `{}` promises an observation nothing \
                             produces",
                            self.source, field.name, self.name
                        ),
                    )
                    .with_hint(format!(
                        "fields of `{}`: {}",
                        self.source,
                        join(source_fields.iter().map(|field| field.name.clone()))
                    )),
                );
                continue;
            };

            if !crate::types::is_assignable(&source_field.type_ref, &field.type_ref) {
                errors.push(ValidationError::new(
                    ValidationCode::TypeMismatch,
                    at(&format!("fields[{index}].type")),
                    format!(
                        "`{}` projects `{}` as {}, but `{}` declares it as {}",
                        self.name, field.name, field.type_ref, self.source, source_field.type_ref
                    ),
                ));
            }
        }

        if let Some(filter) = &self.filter {
            // Only the first segment is resolved: a deeper path such as `total.amount` walks into a
            // named struct, and resolving that belongs with the IR, which knows every type.
            for path in filter.fact_paths() {
                let root = path.namespace();
                if known(root).is_none() {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnobservableFact,
                            at("filter"),
                            format!(
                                "`{path}` reads `{root}`, which `{}` does not have; a view cannot \
                                 select on something its source never observes",
                                self.source
                            ),
                        )
                        .with_hint(format!(
                            "fields of `{}`: {}",
                            self.source,
                            join(source_fields.iter().map(|field| field.name.clone()))
                        )),
                    );
                }
            }

            errors.extend(self.validate_filter_values(filter, types, source_fields));
        }

        errors.into_result(())
    }

    /// Checks every value a filter compares against what the field it compares can hold.
    ///
    /// Only enumerations are checkable: they are the one type whose values the specification lists,
    /// and the state an entity's lifecycle synthesises is one. `state == Issed` is otherwise a
    /// filter that selects nothing, and a generated conformance scenario that can never match is
    /// indistinguishable from one that can.
    fn validate_filter_values(
        &self,
        filter: &Predicate,
        types: &TypeRegistry,
        source_fields: &[Field],
    ) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        for (path, value) in compared_values(filter) {
            // As above: a deeper path walks into a named struct, and resolving that belongs with
            // the IR. A root the source does not have is already reported as unobservable.
            if path.segments().len() != 1 {
                continue;
            }
            let Some(field) = source_fields
                .iter()
                .find(|field| field.name == path.namespace())
            else {
                continue;
            };
            let Some((declared, variants)) = enumeration(types, &field.type_ref) else {
                continue;
            };

            let compared = value.to_string();
            if !variants.contains(&compared) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        format!("view.{}.filter", self.name),
                        format!(
                            "`{}` compares `{}` to `{compared}`, which `{declared}` does not have, \
                             so the filter selects nothing whatever the system does",
                            self.name, field.name
                        ),
                    )
                    .with_hint(format!("values of `{declared}`: {}", join(variants.iter()))),
                );
            }
        }

        errors
    }
}

/// The enumeration a type reference resolves to, with the name to report it under.
///
/// `Optional` is unwrapped: a value that may be absent is still one of the same names when it is
/// there. Anything else — a primitive, a struct, a list, a type nothing declares — has no listed
/// values, so there is nothing to check a comparison against.
fn enumeration<'a>(
    types: &'a TypeRegistry,
    reference: &TypeRef,
) -> Option<(&'a QualifiedName, &'a [String])> {
    match reference {
        TypeRef::Optional(inner) => enumeration(types, inner),
        TypeRef::Named(name) => {
            let declared = types.get(name)?;
            match &declared.body {
                TypeBody::Enum { variants } => Some((&declared.name, variants.as_slice())),
                _ => None,
            }
        }
        TypeRef::Primitive(_) | TypeRef::List(_) | TypeRef::Map(_, _) => None,
    }
}

/// Every fact a predicate compares to a literal, paired with that literal.
fn compared_values(predicate: &Predicate) -> Vec<(&FactPath, &FactValue)> {
    let mut found = Vec::new();
    collect_compared_values(predicate, &mut found);
    found
}

/// Walks a predicate, because a filter is as often `any: [...]` as a single comparison.
fn collect_compared_values<'a>(
    predicate: &'a Predicate,
    found: &mut Vec<(&'a FactPath, &'a FactValue)>,
) {
    match predicate {
        Predicate::All(children) | Predicate::Any(children) => {
            for child in children {
                collect_compared_values(child, found);
            }
        }
        Predicate::Not(inner) => collect_compared_values(inner, found),
        Predicate::Compare { left, right, .. } => match (left, right) {
            (Operand::Fact(path), Operand::Literal(value))
            | (Operand::Literal(value), Operand::Fact(path)) => found.push((path, value)),
            _ => {}
        },
        Predicate::AnyOf { path, values } | Predicate::NoneOf { path, values } => {
            found.extend(values.iter().map(|value| (path, value)));
        }
        Predicate::Always | Predicate::Never | Predicate::Truthy(_) | Predicate::Defined(_) => {}
    }
}

/// Renders a list of names for a diagnostic, saying so when the list is empty.
fn join<T: fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let rendered: Vec<String> = items.into_iter().map(|item| format!("`{item}`")).collect();
    if rendered.is_empty() {
        "none are declared".to_owned()
    } else {
        rendered.join(", ")
    }
}

/// A view as written in a document, before validation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawViewSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// The entity it projects.
    pub source: QualifiedName,
    /// What it exposes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    /// Which instances it contains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Predicate>,
    /// How soon it reflects a command that has already returned. Defaults to `eventual`.
    #[serde(default)]
    pub consistency: Consistency,
    /// What it is called on the wire and shown as.
    #[serde(default, skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

impl TryFrom<RawViewSpec> for ViewSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawViewSpec) -> Result<Self, Self::Error> {
        let spec = Self {
            name: raw.name,
            source: raw.source,
            fields: raw.fields,
            filter: raw.filter,
            consistency: raw.consistency,
            naming: raw.naming,
        };
        spec.validate_shape().into_result(spec)
    }
}

impl From<ViewSpec> for RawViewSpec {
    fn from(view: ViewSpec) -> Self {
        Self {
            name: view.name,
            source: view.source,
            fields: view.fields,
            filter: view.filter,
            consistency: view.consistency,
            naming: view.naming,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::types::{NamedType, Primitive, TypeBody, TypeRef};

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    /// The §31 example's types, as `examples/billing/domains/invoice.yaml` declares them, plus the
    /// enum `Invoice`'s lifecycle synthesises — which is what `Specification::validate` puts in the
    /// registry a view is checked against.
    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        for (declared, body) in [
            (
                "billing.invoice.InvoiceId",
                TypeBody::Newtype {
                    of: TypeRef::Primitive(Primitive::Uuid),
                    invariants: Vec::new(),
                },
            ),
            (
                "billing.invoice.Email",
                TypeBody::Newtype {
                    of: TypeRef::Primitive(Primitive::String),
                    invariants: Vec::new(),
                },
            ),
            (
                "billing.invoice.Money",
                TypeBody::Struct {
                    fields: vec![
                        Field::new("amount", TypeRef::Primitive(Primitive::Decimal)),
                        Field::new("currency", TypeRef::Primitive(Primitive::String)),
                    ],
                    invariants: Vec::new(),
                },
            ),
            (
                "billing.invoice.Invoice.State",
                TypeBody::Enum {
                    variants: vec![
                        "Draft".to_owned(),
                        "Issued".to_owned(),
                        "Paid".to_owned(),
                        "Cancelled".to_owned(),
                    ],
                },
            ),
        ] {
            registry
                .insert(NamedType {
                    name: name(declared),
                    body,
                    naming: Naming::default(),
                })
                .expect("new");
        }
        registry
    }

    /// `Invoice`'s observable surface, as `EntitySpec::observable_fields` builds it: its identity,
    /// its fields, and its state typed as its own lifecycle.
    fn entities() -> BTreeMap<QualifiedName, Vec<Field>> {
        [(
            name("billing.invoice.Invoice"),
            vec![
                Field::new(
                    "invoice_id",
                    TypeRef::Named(name("billing.invoice.InvoiceId")),
                ),
                Field::new(
                    "customer_email",
                    TypeRef::Named(name("billing.invoice.Email")),
                ),
                Field::new("total", TypeRef::Named(name("billing.invoice.Money"))),
                Field::new(
                    "state",
                    TypeRef::Named(name("billing.invoice.Invoice.State")),
                ),
            ],
        )]
        .into()
    }

    const INVOICE_BY_ID: &str = r"
name: billing.invoice.InvoiceById
source: billing.invoice.Invoice
consistency: eventual
fields:
  - name: invoice_id
    type: billing.invoice.InvoiceId
  - name: total
    type: billing.invoice.Money
";

    fn invoice_by_id() -> ViewSpec {
        let raw: RawViewSpec = serde_yaml::from_str(INVOICE_BY_ID).expect("parses");
        ViewSpec::try_from(raw).expect("a valid view")
    }

    fn refuse(view: &ViewSpec) -> ValidationErrors {
        view.validate(&registry(), &entities())
            .expect_err("expected a refusal")
    }

    #[test]
    fn a_projection_makes_a_scenario_assert_eventually_rather_than_immediately() {
        let view = invoice_by_id();
        assert!(view.validate(&registry(), &entities()).is_ok());

        assert_eq!(view.consistency, Consistency::Eventual);
        assert_eq!(view.assertion_style(), AssertionStyle::Eventually);
        assert_eq!(
            view.assertion_style().as_str(),
            "eventually",
            "the generated scenario must put this view under `eventually:`, never under `expect:`"
        );
        assert!(
            view.assertion_style().is_retried(),
            "a projection is retried until it catches up; a sleep would test the machine instead"
        );
    }

    #[test]
    fn a_read_your_writes_view_is_asserted_immediately() {
        let mut view = invoice_by_id();
        view.consistency = Consistency::ReadYourWrites;

        assert_eq!(view.assertion_style(), AssertionStyle::Expect);
        assert_eq!(view.assertion_style().as_str(), "expect");
        assert!(!view.assertion_style().is_retried());
        assert_eq!(view.consistency.to_string(), "read_your_writes");
    }

    #[test]
    fn a_view_that_does_not_declare_its_consistency_is_eventual() {
        let raw: RawViewSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvoiceById
source: billing.invoice.Invoice
fields:
  - name: total
    type: billing.invoice.Money
",
        )
        .expect("parses");
        let view = ViewSpec::try_from(raw).expect("valid");

        assert_eq!(
            view.consistency,
            Consistency::Eventual,
            "the cheap mistake is the default: a wrong `eventual` is slow, a wrong \
             `read_your_writes` is flaky"
        );
        assert_eq!(view.assertion_style(), AssertionStyle::Eventually);
    }

    #[test]
    fn a_view_round_trips_through_yaml() {
        let view = invoice_by_id();
        let rendered = serde_yaml::to_string(&view).expect("serialises");
        let reparsed: RawViewSpec = serde_yaml::from_str(&rendered).expect("re-parses");
        let round_tripped = ViewSpec::try_from(reparsed).expect("still valid");

        assert_eq!(round_tripped, view, "{rendered}");
        assert!(
            rendered.contains("consistency: eventual"),
            "consistency survives the document form, or a generator loses it: {rendered}"
        );
    }

    #[test]
    fn a_view_of_an_entity_that_does_not_exist_is_refused() {
        let mut view = invoice_by_id();
        view.source = name("billing.invoice.Receipt");

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`billing.invoice.Receipt` is not a declared entity"),
            "{rendered}"
        );
        assert!(
            rendered.contains("billing.invoice.Invoice"),
            "and what was available: {rendered}"
        );
        assert_eq!(
            errors.len(),
            1,
            "with no source there is nothing to check the fields against: {rendered}"
        );
    }

    #[test]
    fn a_view_field_the_source_does_not_have_is_refused() {
        let mut view = invoice_by_id();
        view.fields.push(Field::new(
            "customer_name",
            TypeRef::Primitive(Primitive::String),
        ));

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("has no field `customer_name`"),
            "{rendered}"
        );
        assert!(
            rendered.contains("promises an observation nothing produces"),
            "{rendered}"
        );
        assert!(
            rendered.contains("`total`"),
            "and what the entity does have: {rendered}"
        );
    }

    #[test]
    fn a_view_field_whose_type_disagrees_with_the_entity_is_refused() {
        let mut view = invoice_by_id();
        view.fields = vec![Field::new("total", TypeRef::Primitive(Primitive::Decimal))];

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::TypeMismatch));
        assert!(
            errors
                .to_string()
                .contains("projects `total` as Decimal, but `billing.invoice.Invoice` declares it as billing.invoice.Money"),
            "the refusal must name both types: {errors}"
        );
    }

    #[test]
    fn a_view_field_may_widen_a_required_entity_field_to_an_optional_one() {
        let mut view = invoice_by_id();
        view.fields = vec![Field::new(
            "total",
            TypeRef::Optional(Box::new(TypeRef::Named(name("billing.invoice.Money")))),
        )];

        assert!(
            view.validate(&registry(), &entities()).is_ok(),
            "a value that is always present can fill a slot that may be absent"
        );
    }

    #[test]
    fn a_filter_reading_a_field_the_source_does_not_have_is_refused() {
        let mut view = invoice_by_id();
        view.filter = Some(Predicate::parse_expression("balance > 0").expect("parses"));

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::UnobservableFact));
        let rendered = errors.to_string();
        assert!(rendered.contains("reads `balance`"), "{rendered}");
        assert!(
            rendered.contains("cannot select on something its source never observes"),
            "{rendered}"
        );
    }

    #[test]
    fn a_filter_may_read_a_field_the_view_does_not_project() {
        let mut view = invoice_by_id();
        view.filter = Some(Predicate::parse_expression("state == Issued").expect("parses"));

        assert!(
            view.field("state").is_none(),
            "the fixture projects the total, not the state"
        );
        assert!(
            view.validate(&registry(), &entities()).is_ok(),
            "`OutstandingInvoices` selects on the state without exposing it, which is normal"
        );
    }

    #[test]
    fn a_filter_comparing_a_state_to_a_name_the_lifecycle_does_not_have_is_refused() {
        let mut view = invoice_by_id();
        view.filter = Some(Predicate::parse_expression("state == Issed").expect("parses"));

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("compares `state` to `Issed`"),
            "{rendered}"
        );
        assert!(
            rendered.contains("selects nothing whatever the system does"),
            "a filter nothing can satisfy generates a scenario nothing can fail: {rendered}"
        );
        assert!(
            rendered.contains("`Issued`"),
            "and the name that was meant: {rendered}"
        );
    }

    #[test]
    fn every_branch_of_a_composite_filter_is_checked_against_the_lifecycle() {
        let mut view = invoice_by_id();
        view.filter = Some(Predicate::any(vec![
            Predicate::parse_expression("state == Issued").expect("parses"),
            Predicate::parse_expression("state == Overdue").expect("parses"),
        ]));

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert_eq!(
            errors.len(),
            1,
            "the declared state is accepted and only the invented one is reported: {errors}"
        );
        assert!(errors.to_string().contains("`Overdue`"), "{errors}");
    }

    #[test]
    fn a_filter_on_a_field_whose_type_lists_no_values_is_left_to_the_ir() {
        let mut view = invoice_by_id();
        view.filter = Some(
            Predicate::parse_expression(r#"customer_email == "someone@example.com""#)
                .expect("parses"),
        );

        assert!(
            view.validate(&registry(), &entities()).is_ok(),
            "a newtype over a string has no set of names to check a comparison against"
        );
    }

    #[test]
    fn a_view_that_projects_nothing_is_refused() {
        let raw: RawViewSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvoiceById
source: billing.invoice.Invoice
",
        )
        .expect("parses");

        let errors = ViewSpec::try_from(raw).expect_err("a view with no fields");
        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert!(
            errors.to_string().contains("projects no fields"),
            "{errors}"
        );
    }

    #[test]
    fn a_field_projected_twice_is_refused() {
        let raw: RawViewSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvoiceById
source: billing.invoice.Invoice
fields:
  - name: total
    type: billing.invoice.Money
  - name: total
    type: billing.invoice.Money
",
        )
        .expect("parses");

        let errors = ViewSpec::try_from(raw).expect_err("a field projected twice");
        assert!(errors.contains(ValidationCode::DuplicateDeclaration));
        assert!(
            errors
                .to_string()
                .contains("`total` is projected more than once"),
            "{errors}"
        );
    }

    #[test]
    fn a_projected_field_must_name_a_declared_type() {
        let mut view = invoice_by_id();
        view.fields = vec![Field::new(
            "total",
            TypeRef::Named(name("billing.invoice.Amount")),
        )];

        let errors = refuse(&view);
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert!(
            errors
                .to_string()
                .contains("`billing.invoice.Amount` is not a declared type"),
            "{errors}"
        );
    }
}
