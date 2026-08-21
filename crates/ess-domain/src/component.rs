//! Software decomposition: which component owns which domain, and what it accepts and publishes.
//!
//! A component is a *logical* boundary, not a deployment decision (design §5). `invoice-service`
//! owning `billing.invoice` says the invoice context is one unit of ownership; whether it ships as
//! its own process or as a module inside one binary is [`crate::topology`]'s business, and changing
//! that answer must not change this file.
//!
//! What a component declares is checkable, and worth checking:
//!
//! | rule | code |
//! |---|---|
//! | it owns a domain nothing declares | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | it accepts a command nothing declares | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | it publishes an event nothing declares | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | it accepts a command another component owns the domain of | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | two components own the same domain | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | it owns nothing, accepts nothing and publishes nothing | [`EmptyDeclaration`](ValidationCode::EmptyDeclaration) |
//! | it is [`reached_by: network`](Reach::Network) and no route follows — it accepts no command and owns no domain that declares a view | [`EmptyDeclaration`](ValidationCode::EmptyDeclaration) |
//!
//! # Where each rule lives
//!
//! The sixth row is the only one a component can answer alone, so it is [`ComponentSpec::validate`]
//! and it runs during conversion: an empty component never reaches a
//! [`Specification`](crate::spec::Specification). The three reference rules need the domains,
//! commands and events the whole specification declares — [`ComponentSpec::validate_references`].
//! The two conflicts need the *other* components as well, which no component has in hand, so they
//! are [`validate_components`]. So does the last row, for a third reason: a view is declared inside
//! a domain, so "does anything I own project a row" is a question about the domains and not about
//! the component. That function runs all six.
//!
//! # Ownership is what makes a conflict a conflict
//!
//! Design §9 draws the system as one graph, and the edge that matters here is `COMMAND` →
//! *handled by* → `COMPONENT` → *modifies* → `DOMAIN STATE`. A command has one handler, and the
//! handler is the component that owns the domain whose state the command changes. So two
//! components both claiming `billing.invoice` — or a component accepting `billing.invoice.X` while
//! another owns `billing.invoice` — are two well-formed statements that cannot both hold, which is
//! what [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) is for. Wave 2's
//! bindings make it load-bearing rather than tidy: a binding's `invoke.command` has to resolve to
//! one destination, and it cannot if two components answer.
//!
//! # What this deliberately does not refuse
//!
//! Each of these is a shape an author may legitimately want, and §20's rejection list — which names
//! "components accepting undefined commands" and nothing else about components — does not ask for
//! it. Refusing something with no legal way to express it is worse than a rule left unwritten, so
//! each stays legal until evidence says otherwise.
//!
//! **A domain no component owns.** §5 says component responsibility "is logical; it is not yet a
//! deployment decision", and wave 1 shipped a whole specification with no components at all. A
//! model that has domains and has not been decomposed yet is a legitimate state; if an unowned
//! domain were an error, adding the *first* component to a specification would retroactively
//! invalidate every domain that has none.
//!
//! **A command accepted from a domain nobody owns.** Nothing else claims the handler edge, so there
//! is no contradiction — only an ownership statement that has not been written. This is also what
//! makes partial decomposition work: the commands of an undecomposed domain still have a handler.
//!
//! **An event published from a domain the component does not own.** Acceptance is a *destination* —
//! §9 gives a command exactly one handler, so a second claim is ambiguous. Publication is a
//! *source*: §9's `EVENT` → *triggers* → `COMMAND` edge does not ask who published, and §12 pairs a
//! publisher with a consumer by the event's identity rather than by component. §6's outer surface
//! is explicitly where "event topics" and "external APIs" live, so a component that translates and
//! re-publishes is a shape the design contemplates. Two publishers of one event is therefore not a
//! statement that cannot hold, and it is not refused here.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};

use crate::name::{Naming, QualifiedName};

/// What a component is made of, as a document says it.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawComponentSpec {
    /// Its name — a single segment, like `invoice-service`.
    #[serde(alias = "component")]
    pub name: String,
    /// The domains it owns.
    #[serde(default)]
    pub owns: RawComponentOwns,
    /// The commands it accepts.
    #[serde(default)]
    pub accepts: RawComponentSurface,
    /// The events it publishes.
    #[serde(default)]
    pub publishes: RawComponentSurface,
    /// Who reaches its accepted commands and the views its domains declare.
    ///
    /// Unstated is [`Reach::InProcess`] — the shape every specification here had before the word
    /// existed, and the one a reader assumes when nothing says otherwise.
    #[serde(default)]
    pub reached_by: Reach,
    /// What it is called on the wire and shown as.
    #[serde(default)]
    pub naming: Naming,
    /// What it is, in one line.
    #[serde(default)]
    pub summary: Option<String>,
}

/// Where the callers of a component's surface are.
///
/// # A fact about the system, not a protocol
///
/// The model has no transport construct and this is not one: neither variant names a wire format,
/// a port, a path or a verb. What it states is who issues the commands — a question a specification
/// answers about its own design, and the one thing a generator cannot infer. `invoice-service` and
/// `email-service` in the normative example are reached in process, and that is why the one
/// transport their bindings determine is an in-process log; a component reached across a network
/// has callers that are not deployed with it, and its surface has to exist on a wire.
///
/// # What follows from `network`, and what does not
///
/// Which wire is *derived*, never chosen: this repository projects exactly one contract for a
/// component's command surface — the `OpenAPI` document `ess-gen` writes for every component — and
/// an `OpenAPI` document is an HTTP contract. So a `network` surface is served over HTTP because
/// that is the only contract that already exists for it, and a synthesised server that spoke
/// anything else would contradict the document committed beside it. No variant here says `http`,
/// and adding one would be the transport DSL design §7 sketches and this model has not taken.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// Every caller is deployed with it, so the surface never leaves the process.
    ///
    /// The default, and the only reading of silence that does not invent a deployment: a
    /// specification that has not said where its callers are has not said its surface crosses
    /// anything.
    #[default]
    InProcess,
    /// At least one caller is not deployed with it, so the surface crosses a process boundary.
    ///
    /// The declaration that makes the published `OpenAPI` document load-bearing rather than
    /// documentary: something outside has to issue these commands, and the only way it can is over
    /// the contract.
    Network,
}

impl Reach {
    /// The two words `reached_by:` accepts, in the order [`Reach`] declares them.
    pub const ALL: &'static [Reach] = &[Reach::InProcess, Reach::Network];

    /// The word a document writes.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::Network => "network",
        }
    }

    /// `true` where the surface never leaves the process — the unstated default.
    ///
    /// Used by the IR to leave the default out of the resolved model's serialisation, so a
    /// specification that says nothing about reach digests exactly as it did before the word
    /// existed.
    pub fn is_in_process(self) -> bool {
        matches!(self, Self::InProcess)
    }
}

/// What a component owns.
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawComponentOwns {
    /// The domains, by qualified name.
    #[serde(default)]
    pub domains: Vec<QualifiedName>,
}

/// The commands a component accepts, or the events it publishes.
///
/// One shape for both because the document spells them the same way (§5) and a second shape would be
/// a second thing to keep in step.
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawComponentSurface {
    /// Commands, when this is an `accepts:` block.
    #[serde(default)]
    pub commands: Vec<QualifiedName>,
    /// Events, when this is a `publishes:` block.
    #[serde(default)]
    pub events: Vec<QualifiedName>,
}

/// A component: one unit of ownership.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ComponentSpec {
    /// Its name.
    pub name: ComponentName,
    /// The domains it owns.
    pub owns: BTreeSet<QualifiedName>,
    /// The commands it accepts.
    pub accepts: BTreeSet<QualifiedName>,
    /// The events it publishes.
    pub publishes: BTreeSet<QualifiedName>,
    /// Where the callers of its surface are.
    pub reached_by: Reach,
    /// What it is called on the wire, and what a person is shown.
    pub naming: Naming,
}

/// A component's name.
///
/// Not a [`QualifiedName`]: a component is not inside a domain, and giving it a dotted name would
/// invite the reading that `billing.invoice-service` belongs to `billing.invoice`. It becomes a
/// workload name, a container name and a metrics label, so it is spelt the way those are.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct ComponentName(String);

impl ComponentName {
    /// What a component name looks like.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";

    /// Parses one.
    pub fn new(value: impl AsRef<str>) -> Result<Self, aep_domain::error::ParseError> {
        let value = value.as_ref();
        let valid = !value.is_empty()
            && value.starts_with(|c: char| c.is_ascii_lowercase())
            && !value.ends_with('-')
            && !value.contains("--")
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid {
            return Err(aep_domain::error::ParseError::identifier(
                "component name",
                value,
                "a component name is lower-case words joined by single hyphens, such as \
                 `invoice-service`; it becomes a workload name and a metrics label"
                    .to_owned(),
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl schemars::JsonSchema for ComponentName {
    fn schema_name() -> String {
        "ComponentName".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("A component's name, such as `invoice-service`.".to_owned());
        schema.into()
    }
}

impl TryFrom<RawComponentSpec> for ComponentSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawComponentSpec) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let name = match ComponentName::new(&raw.name) {
            Ok(name) => name,
            Err(error) => {
                return Err(ValidationErrors::new().with(ValidationError::new(
                    ValidationCode::TypeMismatch,
                    format!("component {}", raw.name),
                    error.to_string(),
                )));
            }
        };

        let component = Self {
            name,
            owns: raw.owns.domains.into_iter().collect(),
            accepts: raw.accepts.commands.into_iter().collect(),
            publishes: raw.publishes.events.into_iter().collect(),
            reached_by: raw.reached_by,
            naming: Naming {
                summary: raw.naming.summary.or(raw.summary),
                ..raw.naming
            },
        };

        errors.extend(component.validate());
        errors.into_result(component)
    }
}

impl ComponentSpec {
    /// Everything checkable without the rest of the specification.
    pub fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.owns.is_empty() && self.accepts.is_empty() && self.publishes.is_empty() {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("component {}", self.name),
                    format!(
                        "`{}` owns nothing, accepts nothing and publishes nothing",
                        self.name
                    ),
                )
                .with_hint(
                    "a component that does nothing is a name; delete it or give it a domain",
                ),
            );
        }
        errors
    }

    /// Checks that a surface declared reachable from outside has something to serve.
    ///
    /// `projecting` is the set of domains that declare at least one view, which is what makes this
    /// a whole-specification rule rather than one a component could answer alone: a view lives
    /// inside a domain document, and a component holds only the domain's name.
    ///
    /// [`Reach::Network`] is a promise that something outside issues commands here or reads a row
    /// from here. A component that accepts no command and owns no domain projecting a view has no
    /// route to serve, so the promise is a contract with no operations in it — an endpoint list
    /// nobody can call, published as if it could be. It is refused rather than emitted empty,
    /// because an empty surface is the shape a rename leaves behind and it reads exactly like a
    /// component whose commands have not been written yet.
    pub fn validate_reach(&self, projecting: &BTreeSet<QualifiedName>) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.reached_by != Reach::Network {
            return errors;
        }
        if !self.accepts.is_empty() || self.owns.iter().any(|domain| projecting.contains(domain)) {
            return errors;
        }
        errors.push(
            ValidationError::new(
                ValidationCode::EmptyDeclaration,
                format!("component {}", self.name),
                format!(
                    "`{}` is reached over a network and serves nothing: it accepts no command,                      and no domain it owns declares a view",
                    self.name
                ),
            )
            .with_hint(
                "a surface reachable from outside is one somebody calls; give it a command to                  accept or a view to project, or say `reached_by: in_process`",
            ),
        );
        errors
    }

    /// Checks every reference this component makes against what the specification declares.
    ///
    /// Three references, one rule each: a domain it owns, a command it accepts and an event it
    /// publishes each have to name something that exists. A component is the layer a reader — or a
    /// coding agent — goes to for "what am I building, and what talks to it", so a reference with
    /// nothing behind it reads as a work item for something nobody declared. It is usually a
    /// rename that happened on one side only, which is why the hint lists what was available.
    ///
    /// Whether another component has already claimed the same ground is [`validate_components`].
    pub fn validate_references(
        &self,
        domains: &BTreeSet<QualifiedName>,
        commands: &BTreeSet<QualifiedName>,
        events: &BTreeSet<QualifiedName>,
    ) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        for (field, verb, kind, plural, declared, referenced) in [
            (
                "owns.domains",
                "owns",
                "a domain",
                "domains",
                domains,
                &self.owns,
            ),
            (
                "accepts.commands",
                "accepts",
                "a command",
                "commands",
                commands,
                &self.accepts,
            ),
            (
                "publishes.events",
                "publishes",
                "an event",
                "events",
                events,
                &self.publishes,
            ),
        ] {
            for name in referenced {
                if declared.contains(name) {
                    continue;
                }
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        format!("component {}.{field}", self.name),
                        format!(
                            "`{}` {verb} `{name}`, which nothing declares as {kind}",
                            self.name
                        ),
                    )
                    .with_hint(available(plural, declared)),
                );
            }
        }
        errors
    }
}

/// Everything checkable only against the rest of the specification.
///
/// Every rule in the module table except the empty-component one, which conversion already spent —
/// a [`ComponentSpec`] cannot exist without having passed [`ComponentSpec::validate`], so an empty
/// component never arrives here to be reported twice.
///
/// `domains`, `commands` and `events` are what the whole specification declares, so a component may
/// name anything any domain declares: §5's `email-service` accepts `email.SendEmail` and §7's
/// bindings cross contexts, so resolving a component's references against its own domains only
/// would refuse the design's own example.
///
/// Errors accumulate. One pass reports every broken reference and every conflict, so a document is
/// fixed once rather than four times.
pub fn validate_components(
    components: &BTreeMap<ComponentName, ComponentSpec>,
    domains: &BTreeSet<QualifiedName>,
    commands: &BTreeSet<QualifiedName>,
    events: &BTreeSet<QualifiedName>,
    projecting: &BTreeSet<QualifiedName>,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    for component in components.values() {
        errors.extend(component.validate_references(domains, commands, events));
        errors.extend(component.validate_reach(projecting));
    }

    let ownership = ownership(components, domains);

    for (domain, owners) in &ownership {
        if owners.len() < 2 {
            continue;
        }
        errors.push(
            ValidationError::new(
                ValidationCode::ConflictingDeclaration,
                format!("domain {domain}"),
                format!(
                    "`{domain}` is owned by more than one component: {}",
                    quoted(owners)
                ),
            )
            .with_hint(
                "a domain has one owning component — §9 gives its state one component that \
                 modifies it; split the domain, or give it to one of them and connect the other \
                 with a binding",
            ),
        );
    }

    for (name, component) in components {
        for command in &component.accepts {
            // A command nothing declares is already reported against this component, and the
            // conflict below is derived from it: it disappears the moment the typo is fixed, so
            // reporting it too would send a reader chasing an ownership problem that is not there.
            if !commands.contains(command) {
                continue;
            }
            let Some((domain, owners)) = owner_of(&ownership, command) else {
                continue;
            };
            if owners.contains(name) {
                continue;
            }
            errors.push(
                ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    format!("component {name}.accepts.commands"),
                    format!(
                        "`{name}` accepts `{command}`, and {} owns `{domain}`",
                        quoted(owners)
                    ),
                )
                .with_hint(format!(
                    "§9 gives a command one handler, and it is the component owning its domain: \
                     either move `{domain}` to `{name}`, or let the owner accept `{command}` and \
                     have `{name}` reach it through a binding"
                )),
            );
        }
    }

    errors
}

/// Which components claim which domain.
type Ownership<'a> = BTreeMap<&'a QualifiedName, BTreeSet<&'a ComponentName>>;

/// Indexes the ownership claims the components make.
///
/// Only claims on *declared* domains: a component owning a domain nothing declares is already
/// reported as a broken reference, and carrying it further would produce a second error derived from
/// the first — a conflict over a domain that does not exist.
fn ownership<'a>(
    components: &'a BTreeMap<ComponentName, ComponentSpec>,
    domains: &BTreeSet<QualifiedName>,
) -> Ownership<'a> {
    let mut owners: Ownership<'a> = BTreeMap::new();
    for component in components.values() {
        for domain in component.owns.iter().filter(|d| domains.contains(d)) {
            owners.entry(domain).or_default().insert(&component.name);
        }
    }
    owners
}

/// The owned domain `name` sits in, most specific first.
///
/// Most specific rather than first found: `billing` and `billing.invoice` can both be owned, and
/// `billing.invoice.CreateInvoice` is the invoice context's command, not the outer one's.
fn owner_of<'a>(
    ownership: &'a Ownership<'a>,
    name: &QualifiedName,
) -> Option<(&'a QualifiedName, &'a BTreeSet<&'a ComponentName>)> {
    ownership
        .iter()
        .filter(|(domain, _)| name.is_within(domain))
        .max_by_key(|(domain, _)| domain.segments().len())
        .map(|(domain, owners)| (*domain, owners))
}

/// What was available, which is where a misspelling shows.
fn available(plural: &str, names: &BTreeSet<QualifiedName>) -> String {
    if names.is_empty() {
        return format!("no {plural} are declared anywhere in the specification");
    }
    format!(
        "declared {plural}: {}",
        names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Component names as they appear in a message, in name order.
fn quoted(names: &BTreeSet<&ComponentName>) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(yaml: &str) -> ComponentSpec {
        let raw: RawComponentSpec =
            serde_yaml::from_str(yaml).expect("the document is well formed");
        ComponentSpec::try_from(raw).expect("a component is valid on its own")
    }

    fn catalogue(
        specs: impl IntoIterator<Item = ComponentSpec>,
    ) -> BTreeMap<ComponentName, ComponentSpec> {
        specs
            .into_iter()
            .map(|spec| (spec.name.clone(), spec))
            .collect()
    }

    fn names<'a>(values: impl IntoIterator<Item = &'a str>) -> BTreeSet<QualifiedName> {
        values
            .into_iter()
            .map(|value| QualifiedName::new(value).expect("a valid name"))
            .collect()
    }

    /// §5's two contexts.
    fn domains() -> BTreeSet<QualifiedName> {
        names(["billing.invoice", "billing.email"])
    }

    /// The commands §5's components accept.
    fn commands() -> BTreeSet<QualifiedName> {
        names([
            "billing.invoice.CreateInvoice",
            "billing.invoice.MarkInvoicePaid",
            "billing.email.SendEmail",
        ])
    }

    /// The events §5's components publish.
    fn events() -> BTreeSet<QualifiedName> {
        names([
            "billing.invoice.InvoiceCreated",
            "billing.invoice.InvoicePaid",
            "billing.email.EmailSent",
            "billing.email.EmailFailed",
        ])
    }

    fn invoice_service() -> ComponentSpec {
        component(
            "\
component: invoice-service
owns:
  domains:
    - billing.invoice
accepts:
  commands:
    - billing.invoice.CreateInvoice
    - billing.invoice.MarkInvoicePaid
publishes:
  events:
    - billing.invoice.InvoiceCreated
    - billing.invoice.InvoicePaid
",
        )
    }

    fn email_service() -> ComponentSpec {
        component(
            "\
component: email-service
owns:
  domains:
    - billing.email
accepts:
  commands:
    - billing.email.SendEmail
publishes:
  events:
    - billing.email.EmailSent
    - billing.email.EmailFailed
",
        )
    }

    fn check(components: &BTreeMap<ComponentName, ComponentSpec>) -> ValidationErrors {
        validate_components(
            components,
            &domains(),
            &commands(),
            &events(),
            &projecting(),
        )
    }

    /// The domains that declare a view. `billing.email` declares none, which is what makes the
    /// reach rule's refusal reachable at all.
    fn projecting() -> BTreeSet<QualifiedName> {
        names(["billing.invoice"])
    }

    #[test]
    fn a_component_says_nothing_about_reach_unless_it_says_something_about_reach() {
        assert_eq!(
            invoice_service().reached_by,
            Reach::InProcess,
            "silence is not a deployment: a component that has not said where its callers are has \
             not said its surface crosses anything"
        );
    }

    #[test]
    fn a_component_reached_over_a_network_that_serves_nothing_is_refused() {
        // The fixture has to reach the state the rule is about: `billing.email` declares no view
        // (see `projecting`), and this component accepts no command, so both halves of the
        // disjunction are false and the refusal is the rule's rather than an accident.
        let component = component(
            "\
component: relay-service
owns:
  domains:
    - billing.email
publishes:
  events:
    - billing.email.EmailSent
reached_by: network
",
        );
        let email = QualifiedName::new("billing.email").expect("a valid name");
        assert!(
            component.accepts.is_empty() && !projecting().contains(&email),
            "the fixture must reach the state where the rule decides anything"
        );
        let errors = check(&catalogue([component]));
        let error = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::EmptyDeclaration)
            .expect("a network surface with no route is refused");
        assert!(
            error.message.contains("serves nothing"),
            "the refusal names what is missing: {}",
            error.message
        );
    }

    #[test]
    fn a_component_reached_over_a_network_that_only_projects_a_view_is_accepted() {
        // The read-only half, and the reason the rule is a disjunction rather than "accepts
        // nothing": a component that answers queries and takes no command is a legitimate surface.
        let component = component(
            "\
component: ledger-view
owns:
  domains:
    - billing.invoice
reached_by: network
",
        );
        assert!(
            component.accepts.is_empty(),
            "the fixture only holds if the component accepts nothing"
        );
        let errors = component.validate_reach(&projecting());
        assert!(
            errors.is_empty(),
            "a domain that projects a row is a surface: {errors}"
        );
    }

    #[test]
    fn a_reach_the_model_does_not_declare_is_refused_by_the_word() {
        let refused: Result<RawComponentSpec, _> = serde_yaml::from_str(
            "\
component: invoice-service
owns:
  domains:
    - billing.invoice
reached_by: grpc
",
        );
        let error = refused.expect_err("`grpc` is not one of the two words the model has");
        assert!(
            error.to_string().contains("in_process") && error.to_string().contains("network"),
            "the refusal lists the words that exist: {error}"
        );
    }

    #[test]
    fn the_components_from_the_design_document_validate() {
        let components = catalogue([invoice_service(), email_service()]);
        let errors = check(&components);
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn a_component_owning_a_domain_nothing_declares_is_refused() {
        let components = catalogue([component(
            "\
component: invoice-service
owns:
  domains:
    - billing.ivoice
",
        )]);
        let errors = check(&components);
        assert_eq!(errors.len(), 1, "{errors}");

        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(error.location, "component invoice-service.owns.domains");
        assert!(
            error.message.contains("owns `billing.ivoice`"),
            "the message names the reference that resolves to nothing: {error}"
        );
        assert!(
            error
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains("billing.invoice"),
            "the hint lists what was available, which is where the typo shows: {error}"
        );
    }

    #[test]
    fn a_component_accepting_a_command_nothing_declares_is_refused() {
        let components = catalogue([component(
            "\
component: invoice-service
owns:
  domains:
    - billing.invoice
accepts:
  commands:
    - billing.invoice.MarkInvoicePayed
",
        )]);
        let errors = check(&components);
        assert_eq!(errors.len(), 1, "{errors}");

        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(error.location, "component invoice-service.accepts.commands");
        assert!(
            error
                .message
                .contains("accepts `billing.invoice.MarkInvoicePayed`"),
            "{error}"
        );
    }

    #[test]
    fn a_component_publishing_an_event_nothing_declares_is_refused() {
        let components = catalogue([component(
            "\
component: invoice-service
owns:
  domains:
    - billing.invoice
publishes:
  events:
    - billing.invoice.InvoiceIssued
",
        )]);
        let errors = check(&components);
        assert_eq!(errors.len(), 1, "{errors}");

        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(error.location, "component invoice-service.publishes.events");
        assert!(
            error
                .message
                .contains("publishes `billing.invoice.InvoiceIssued`"),
            "{error}"
        );
    }

    #[test]
    fn every_reference_that_names_nothing_is_reported_not_just_the_first() {
        let components = catalogue([
            component(
                "\
component: invoice-service
owns:
  domains:
    - billing.invoice
    - billing.legder
accepts:
  commands:
    - billing.invoice.ArchiveInvoice
publishes:
  events:
    - billing.invoice.InvoiceArchived
",
            ),
            component(
                "\
component: email-service
owns:
  domains:
    - billing.emails
",
            ),
        ]);
        let errors = check(&components);
        assert_eq!(errors.len(), 4, "one pass reports all four: {errors}");
        assert!(
            errors
                .as_slice()
                .iter()
                .all(|error| error.code == ValidationCode::UndeclaredReference),
            "{errors}"
        );
        let rendered = errors.to_string();
        for missing in [
            "billing.legder",
            "billing.invoice.ArchiveInvoice",
            "billing.invoice.InvoiceArchived",
            "billing.emails",
        ] {
            assert!(
                rendered.contains(missing),
                "{missing} is missing: {rendered}"
            );
        }
    }

    #[test]
    fn a_component_accepting_a_command_another_component_owns_the_domain_of_is_refused() {
        let components = catalogue([
            invoice_service(),
            component(
                "\
component: email-service
owns:
  domains:
    - billing.email
accepts:
  commands:
    - billing.email.SendEmail
    - billing.invoice.MarkInvoicePaid
",
            ),
        ]);
        let errors = check(&components);
        assert_eq!(errors.len(), 1, "{errors}");

        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::ConflictingDeclaration);
        assert_eq!(error.location, "component email-service.accepts.commands");
        assert!(
            error
                .message
                .contains("accepts `billing.invoice.MarkInvoicePaid`")
                && error
                    .message
                    .contains("`invoice-service` owns `billing.invoice`"),
            "the message names both halves of the conflict, because either one could be the fix: \
             {error}"
        );
    }

    #[test]
    fn a_component_accepting_a_command_from_a_domain_no_component_owns_is_allowed() {
        // Half of the previous test's conflict removed: nothing else claims `billing.invoice`, so
        // nothing contradicts `email-service` handling its commands. §20 asks for undeclared
        // commands to be refused, not for a domain to be owned before its commands have a handler.
        let components = catalogue([component(
            "\
component: email-service
owns:
  domains:
    - billing.email
accepts:
  commands:
    - billing.email.SendEmail
    - billing.invoice.MarkInvoicePaid
",
        )]);
        let errors = check(&components);
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn two_components_owning_the_same_domain_is_refused() {
        let components = catalogue([
            invoice_service(),
            component(
                "\
component: billing-service
owns:
  domains:
    - billing.invoice
accepts:
  commands:
    - billing.invoice.CreateInvoice
",
            ),
        ]);
        let errors = check(&components);
        assert_eq!(
            errors.len(),
            1,
            "the double claim is the fault; each component accepting its own domain's commands is \
             not a second one: {errors}"
        );

        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::ConflictingDeclaration);
        assert_eq!(error.location, "domain billing.invoice");
        assert!(
            error.message.contains("`billing-service`")
                && error.message.contains("`invoice-service`"),
            "both claimants are named, in name order: {error}"
        );
    }

    #[test]
    fn a_declared_domain_no_component_owns_is_not_an_error() {
        // §5: responsibility is logical and "not yet a deployment decision". A model that has
        // domains and has not been decomposed yet is a state a specification is allowed to be in —
        // otherwise the first component ever written invalidates every domain without one.
        let components = catalogue([invoice_service()]);
        let errors = check(&components);
        assert!(
            errors.is_empty(),
            "`billing.email` is declared and unowned: {errors}"
        );
    }

    #[test]
    fn a_component_publishing_an_event_from_a_domain_it_does_not_own_is_allowed() {
        // Acceptance is a destination and publication is a source: §9 gives a command one handler,
        // so a second claim on it is ambiguous, while nothing downstream of an event asks who
        // published it. §6's outer surface is where a translating adapter lives.
        let components = catalogue([
            invoice_service(),
            component(
                "\
component: email-service
owns:
  domains:
    - billing.email
publishes:
  events:
    - billing.email.EmailSent
    - billing.invoice.InvoicePaid
",
            ),
        ]);
        let errors = check(&components);
        assert!(
            errors.is_empty(),
            "no rule refuses a second publisher: {errors}"
        );
    }

    #[test]
    fn a_misspelt_command_is_one_fault_and_reports_one_error() {
        // `billing.invoice.MarkInvoicePayed` is both undeclared and inside a domain another
        // component owns. The second reading is derived from the first and disappears with the
        // typo, so only the reference is reported.
        let components = catalogue([
            invoice_service(),
            component(
                "\
component: email-service
owns:
  domains:
    - billing.email
accepts:
  commands:
    - billing.invoice.MarkInvoicePayed
",
            ),
        ]);
        let errors = check(&components);
        assert_eq!(errors.len(), 1, "{errors}");
        assert_eq!(
            errors.as_slice()[0].code,
            ValidationCode::UndeclaredReference
        );
        assert!(
            !errors.contains(ValidationCode::ConflictingDeclaration),
            "a conflict over a command nobody declared sends the reader after the wrong repair: \
             {errors}"
        );
    }

    #[test]
    fn two_components_claiming_a_domain_nothing_declares_report_the_reference_and_not_a_conflict() {
        let components = catalogue([
            component(
                "\
component: invoice-service
owns:
  domains:
    - billing.ivoice
",
            ),
            component(
                "\
component: billing-service
owns:
  domains:
    - billing.ivoice
",
            ),
        ]);
        let errors = check(&components);
        assert_eq!(errors.len(), 2, "one per component, and no third: {errors}");
        assert!(
            !errors.contains(ValidationCode::ConflictingDeclaration),
            "a conflict over a domain that does not exist is not the problem to fix: {errors}"
        );
    }

    #[test]
    fn a_command_is_handled_by_the_owner_of_its_innermost_domain() {
        let domains = names(["billing.invoice", "billing.invoice.draft", "billing.email"]);
        let commands = names(["billing.invoice.draft.SubmitDraft"]);
        let components = catalogue([
            component(
                "\
component: invoice-service
owns:
  domains:
    - billing.invoice
",
            ),
            component(
                "\
component: draft-service
owns:
  domains:
    - billing.invoice.draft
accepts:
  commands:
    - billing.invoice.draft.SubmitDraft
",
            ),
        ]);
        let errors =
            validate_components(&components, &domains, &commands, &events(), &projecting());
        assert!(
            errors.is_empty(),
            "`billing.invoice.draft.SubmitDraft` sits in both namespaces, and the inner one owns \
             it: {errors}"
        );
    }

    #[test]
    fn a_component_that_owns_nothing_accepts_nothing_and_publishes_nothing_is_refused() {
        let raw: RawComponentSpec =
            serde_yaml::from_str("component: invoice-service\n").expect("well formed");
        let errors = ComponentSpec::try_from(raw).expect_err("a component that does nothing");
        assert_eq!(errors.len(), 1, "{errors}");
        assert_eq!(errors.as_slice()[0].code, ValidationCode::EmptyDeclaration);
        assert_eq!(errors.as_slice()[0].location, "component invoice-service");
    }

    #[test]
    fn a_component_name_spelt_like_a_type_is_refused() {
        let raw: RawComponentSpec = serde_yaml::from_str(
            "\
component: InvoiceService
owns:
  domains:
    - billing.invoice
",
        )
        .expect("well formed");
        let errors = ComponentSpec::try_from(raw).expect_err("not a component name");
        assert_eq!(errors.len(), 1, "{errors}");
        assert_eq!(errors.as_slice()[0].code, ValidationCode::TypeMismatch);
        assert!(
            errors.to_string().contains("workload name"),
            "the message says what the name becomes, which is why the charset is narrow: {errors}"
        );
    }

    #[test]
    fn a_components_one_line_summary_is_read_from_either_spelling() {
        let spec = component(
            "\
component: invoice-service
summary: Issues invoices and tracks payment.
owns:
  domains:
    - billing.invoice
",
        );
        assert_eq!(
            spec.naming.summary.as_deref(),
            Some("Issues invoices and tracks payment."),
            "a top-level `summary:` is the same statement as `naming.summary`"
        );
    }

    #[test]
    fn a_key_the_model_does_not_know_is_refused() {
        let error = serde_yaml::from_str::<RawComponentSpec>(
            "\
component: invoice-service
own:
  domains:
    - billing.invoice
",
        )
        .expect_err("`own` is nothing");
        assert!(
            error.to_string().contains("own"),
            "a misspelt key would otherwise be an ownership claim that silently does not exist: \
             {error}"
        );
    }
}
