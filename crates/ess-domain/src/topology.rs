//! Semantic runtime requirements: how many of a component must run, and what it needs to run.
//!
//! Design §8. Deliberately *semantic*: `replicas.min: 2` says the system is not correct with one
//! instance, which is a statement about the design. It is not a Kubernetes manifest, and nothing in
//! this wave generates one — the point of writing it down now is that "topology references a
//! component nobody declared" becomes checkable.
//!
//! The distinction is the same one that runs through the whole model: a component is a unit of
//! ownership, a workload is a statement about running it, and conflating them is how a domain model
//! turns into a description of a deployment.
//!
//! | rule | code |
//! |---|---|
//! | a workload names no declared component | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | a replica floor of zero | [`TypeMismatch`](ValidationCode::TypeMismatch) |
//! | a replica ceiling below the floor | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | a stateful workload with a replica floor above one | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | a requirement that is not one `kind: name` pair | [`TypeMismatch`](ValidationCode::TypeMismatch) |
//! | a resource with no kind or no name | [`EmptyDeclaration`](ValidationCode::EmptyDeclaration) |
//! | two workloads for one component | [`DuplicateDeclaration`](ValidationCode::DuplicateDeclaration) |
//! | a workload indexed under a name it does not claim | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//!
//! # Which half runs where
//!
//! [`Topology::validate`] is everything decidable from the topology alone, and conversion runs it.
//! [`validate_topology`] is the half that needs the component list, and *only* that half: a
//! specification converts its topology before it validates it, so a function that ran both would
//! report every replica floor twice.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};

use crate::component::ComponentName;

/// The topology, as a document says it.
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawTopology {
    /// One entry per component that runs.
    #[serde(default)]
    pub workloads: BTreeMap<String, RawWorkload>,
}

/// One workload, as a document says it.
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawWorkload {
    /// How many instances.
    #[serde(default)]
    pub replicas: Option<RawReplicas>,
    /// Whether an instance holds state that outlives a request.
    #[serde(default)]
    pub stateless: Option<bool>,
    /// What it needs in order to run.
    #[serde(default)]
    pub requires: Vec<RawResource>,
}

/// A replica range.
#[derive(Debug, Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawReplicas {
    /// The floor. Below this the system is not correct, not merely slower.
    pub min: u32,
    /// The ceiling, when there is one.
    #[serde(default)]
    pub max: Option<u32>,
}

/// Something a workload needs, as a document says it.
///
/// Written as a single-entry mapping — `- postgres: invoice-store` — because that is how §8 spells
/// it and it reads as "a postgres called invoice-store".
///
/// A map rather than a pair because that is what a mapping parses as, which means the document can
/// say things this type accepts and the model does not: `{}` is a requirement with nothing in it,
/// and `{postgres: a, redis: b}` is one list item that reads as one requirement and means two.
/// Both are refused in the conversion, because both are gone by the time it finishes.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct RawResource(pub BTreeMap<String, String>);

/// What a workload needs in order to run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Resource {
    /// What kind — `postgres`, `publish`, `cache`.
    pub kind: String,
    /// Which one — `invoice-store`, `invoice-events`.
    pub name: String,
}

/// How many instances of a workload must run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Replicas {
    /// The floor.
    pub min: u32,
    /// The ceiling, when there is one.
    pub max: Option<u32>,
}

impl Default for Replicas {
    /// One instance, no ceiling.
    ///
    /// A floor rather than a range: a component with nothing said about it still has to run once,
    /// and defaulting to zero would make an unmentioned component silently absent.
    fn default() -> Self {
        Self { min: 1, max: None }
    }
}

/// One component's runtime requirements.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Workload {
    /// The component this runs.
    pub component: ComponentName,
    /// How many instances.
    pub replicas: Replicas,
    /// Whether an instance holds state that outlives a request.
    pub stateless: bool,
    /// What it needs.
    pub requires: Vec<Resource>,
}

impl Workload {
    /// Everything decidable from one workload, reported against `location`.
    ///
    /// Not public: a workload is only ever reached through the topology that indexes it, and the
    /// document path it is reported against is the topology's to build.
    fn check(&self, location: &str, errors: &mut ValidationErrors) {
        if self.replicas.min == 0 {
            errors.push(
                ValidationError::new(
                    ValidationCode::TypeMismatch,
                    format!("{location}.replicas.min"),
                    format!(
                        "a floor of zero says `{}` need not run at all",
                        self.component
                    ),
                )
                .with_hint(
                    "a floor is how many instances the system needs to be correct; if that is \
                     none, the workload is the thing to delete",
                ),
            );
        }

        if let Some(max) = self.replicas.max {
            if max < self.replicas.min {
                errors.push(
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        format!("{location}.replicas"),
                        format!(
                            "a ceiling of {max} below a floor of {}: no number of instances \
                             satisfies both",
                            self.replicas.min
                        ),
                    )
                    .with_hint("an exact instance count is `min` and `max` set to the same number"),
                );
            }
        }

        // Two instances of something that holds state past a request, and nothing in the model says
        // how they share it — §8 has no vocabulary for that, so the document is claiming something
        // it cannot express. Refused rather than tolerated because both readings *are* expressible:
        // say the state is not per-instance, or say one instance is enough.
        if !self.stateless && self.replicas.min > 1 {
            errors.push(
                ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    location,
                    format!(
                        "`{}` holds state that outlives a request and needs at least {} instances, \
                         and nothing says how they share it",
                        self.component, self.replicas.min
                    ),
                )
                .with_hint(
                    "either the state is not per-instance (`stateless: true`) or one instance is \
                     enough (`replicas.min: 1`); a store they share is a `requires:` entry",
                ),
            );
        }

        for resource in &self.requires {
            // Located at `requires` and not at an index: the list is sorted so that the same
            // requirements always serialise the same way, so an index here would name a different
            // line than the one the author wrote.
            let message = match (
                resource.kind.trim().is_empty(),
                resource.name.trim().is_empty(),
            ) {
                (true, true) => "a requirement with neither a kind nor a name".to_owned(),
                (true, false) => format!(
                    "`{}` is required but nothing says what kind of thing it is",
                    resource.name
                ),
                (false, true) => format!(
                    "a `{}` is required but nothing says which one",
                    resource.kind
                ),
                (false, false) => continue,
            };
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.requires"),
                    message,
                )
                .with_hint(
                    "a requirement is a kind and the name of one of them, such as \
                     `- postgres: invoice-store`",
                ),
            );
        }
    }
}

/// The system's runtime shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Topology {
    /// One entry per component that runs, keyed by component.
    pub workloads: BTreeMap<ComponentName, Workload>,
}

impl TryFrom<RawTopology> for Topology {
    type Error = ValidationErrors;

    fn try_from(raw: RawTopology) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let mut workloads = BTreeMap::new();

        for (name, workload) in raw.workloads {
            let component = match ComponentName::new(&name) {
                Ok(component) => component,
                Err(error) => {
                    errors.push(ValidationError::new(
                        ValidationCode::TypeMismatch,
                        format!("topology.workloads.{name}"),
                        error.to_string(),
                    ));
                    continue;
                }
            };

            let replicas = workload
                .replicas
                .map_or_else(Replicas::default, |raw| Replicas {
                    min: raw.min,
                    max: raw.max,
                });

            let mut requires = Vec::with_capacity(workload.requires.len());
            for (index, resource) in workload.requires.into_iter().enumerate() {
                let location = format!("topology.workloads.{component}.requires[{index}]");
                let count = resource.0.len();
                if count == 0 {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::EmptyDeclaration,
                            location,
                            "a requirement that names nothing",
                        )
                        .with_hint("one `kind: name` pair, such as `- postgres: invoice-store`"),
                    );
                    continue;
                }
                if count > 1 {
                    // §8 writes one pair per list item, so this document reads as one requirement
                    // and means several. Reported here because the conversion below flattens it and
                    // then nothing downstream can tell the two spellings apart.
                    let kinds = resource.0.keys().cloned().collect::<Vec<_>>().join(", ");
                    errors.push(
                        ValidationError::new(
                            ValidationCode::TypeMismatch,
                            location,
                            format!(
                                "one requirement holding {count} pairs ({kinds}), which reads as \
                                 one thing and means {count}"
                            ),
                        )
                        .with_hint(
                            "one `kind: name` pair per list item; split this into separate entries",
                        ),
                    );
                }
                requires.extend(
                    resource
                        .0
                        .into_iter()
                        .map(|(kind, name)| Resource { kind, name }),
                );
            }
            // Ordered by content rather than by document order, so the same requirements always
            // serialise the same way whichever order they were written in (W2.4).
            requires.sort();

            workloads.insert(
                component.clone(),
                Workload {
                    component,
                    replicas,
                    // Not required, unlike a binding's `on_failure` (review F3): §8's own
                    // `email-service` and §31's whole reference topology state a replica floor and
                    // no `stateless:`, so requiring the word would refuse the design's examples.
                    // The asymmetry with `on_failure` is real: there, defaulting decides what
                    // happens when delivery fails, and `drop` has to be typed. Here it decides
                    // nothing — `true` is "no claim on state was made", and the claim that *is*
                    // consequential, `false`, is still a word someone has to write.
                    stateless: workload.stateless.unwrap_or(true),
                    requires,
                },
            );
        }

        let topology = Self { workloads };
        errors.extend(topology.validate());
        errors.into_result(topology)
    }
}

impl Topology {
    /// `true` when nothing is stated.
    pub fn is_empty(&self) -> bool {
        self.workloads.is_empty()
    }

    /// Everything checkable without the rest of the specification.
    ///
    /// Conversion runs this, so [`validate_topology`] deliberately does not.
    pub fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let mut claimed: BTreeSet<&ComponentName> = BTreeSet::new();

        for (indexed_under, workload) in &self.workloads {
            let location = format!("topology.workloads.{indexed_under}");

            // The key and `Workload::component` are one fact written twice, so a topology built in
            // code rather than parsed can have them disagree — and then a projection that reads the
            // field runs a component under the name the index gave to a different one.
            //
            // Two identical keys in a *document* are not caught here and cannot be: serde_yaml
            // keeps the last of them, so the first workload is gone before this type exists. That
            // one has to be refused where the text is read.
            if indexed_under != &workload.component {
                errors.push(
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        location.clone(),
                        format!(
                            "indexed under `{indexed_under}` but declares component `{}`",
                            workload.component
                        ),
                    )
                    .with_hint(
                        "a workload is keyed by the component it runs; the two cannot differ",
                    ),
                );
            }

            if !claimed.insert(&workload.component) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DuplicateDeclaration,
                        location.clone(),
                        format!(
                            "`{}` already has a workload, and two runtime shapes for one component \
                             cannot both hold",
                            workload.component
                        ),
                    )
                    .with_hint(
                        "one workload per component; something that runs two ways is two components",
                    ),
                );
            }

            workload.check(&location, &mut errors);
        }

        errors
    }
}

/// Checks the topology against what the rest of the specification declares.
///
/// This is design §20's "topology references to missing components", and it is the reason the
/// layer is modelled in a wave that generates no deployment artifact at all: a workload naming a
/// component nobody declared is either a rename that stopped halfway or a component that was never
/// written, and both are silent until something checks.
///
/// The cross-cutting half only. [`Topology::validate`] already ran during conversion, and a
/// function that ran both would report every replica floor twice.
///
/// # A declared component with no workload is not refused
///
/// Nothing in §8 gives an unmentioned component a meaning other than [`Replicas::default`] — one
/// instance, no ceiling — and a specification that states no topology at all has an empty one, so
/// the rule would refuse every specification that has not written its topology yet. Silence means
/// "runs once", not "was forgotten", until §8 grows a word for the difference.
pub fn validate_topology(
    topology: &Topology,
    components: &BTreeSet<ComponentName>,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();

    for (indexed_under, workload) in &topology.workloads {
        if components.contains(&workload.component) {
            continue;
        }
        let declared = if components.is_empty() {
            "no components are declared".to_owned()
        } else {
            format!(
                "declared: {}",
                components
                    .iter()
                    .map(ComponentName::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        errors.push(
            ValidationError::new(
                ValidationCode::UndeclaredReference,
                format!("topology.workloads.{indexed_under}"),
                format!(
                    "`{}` is not a declared component ({declared})",
                    workload.component
                ),
            )
            .with_hint(
                "a workload states how a component runs; declare the component or drop the workload",
            ),
        );
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §8's topology, as §8 writes it: two workloads, a floor of two on each, and `stateless`
    /// stated on one of them and left out on the other.
    const SECTION_EIGHT: &str = r"
workloads:
  invoice-service:
    replicas:
      min: 2
    stateless: true
    requires:
      - postgres: invoice-store
      - publish: invoice-events

  email-service:
    replicas:
      min: 2
";

    fn accept(yaml: &str) -> Topology {
        let raw: RawTopology = serde_yaml::from_str(yaml).expect("parses");
        Topology::try_from(raw).expect("a valid topology")
    }

    fn refuse(yaml: &str) -> ValidationErrors {
        let raw: RawTopology = serde_yaml::from_str(yaml).expect("parses");
        Topology::try_from(raw).expect_err("expected a refusal")
    }

    fn component(name: &str) -> ComponentName {
        ComponentName::new(name).expect("a valid component name")
    }

    fn declared(names: &[&str]) -> BTreeSet<ComponentName> {
        names.iter().copied().map(component).collect()
    }

    fn workload_for(name: &str) -> Workload {
        Workload {
            component: component(name),
            replicas: Replicas::default(),
            stateless: true,
            requires: Vec::new(),
        }
    }

    #[test]
    fn the_topology_from_section_eight_is_accepted_against_the_components_it_names() {
        let topology = accept(SECTION_EIGHT);

        assert_eq!(topology.workloads.len(), 2);
        assert!(!topology.is_empty());
        let errors = validate_topology(&topology, &declared(&["invoice-service", "email-service"]));
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn a_workload_that_states_nothing_runs_once_and_is_taken_to_hold_no_state() {
        let topology = accept("workloads:\n  email-service: {}\n");
        let workload = &topology.workloads[&component("email-service")];

        assert_eq!(workload.replicas, Replicas { min: 1, max: None });
        assert!(
            workload.stateless,
            "§8's `email-service` and §31's whole reference topology state a floor and no \
             `stateless:`, so the word cannot be required"
        );
    }

    #[test]
    fn a_workload_naming_a_component_nobody_declared_is_refused() {
        let topology = accept(SECTION_EIGHT);

        let errors = validate_topology(&topology, &declared(&["invoice-service"]));
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert_eq!(errors.len(), 1, "only the undeclared one: {errors}");
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`email-service` is not a declared component"),
            "{rendered}"
        );
        assert!(
            rendered.contains("declared: invoice-service"),
            "and what was available: {rendered}"
        );
    }

    #[test]
    fn every_workload_is_refused_when_nothing_declares_a_component_at_all() {
        let topology = accept(SECTION_EIGHT);

        let errors = validate_topology(&topology, &BTreeSet::new());
        assert_eq!(
            errors.len(),
            2,
            "one refusal per workload, not one for the topology: {errors}"
        );
        assert!(errors
            .as_slice()
            .iter()
            .all(|error| error.code == ValidationCode::UndeclaredReference));
    }

    #[test]
    fn a_declared_component_with_no_workload_is_not_refused() {
        let topology = accept("workloads:\n  invoice-service:\n    replicas:\n      min: 2\n");

        let errors = validate_topology(&topology, &declared(&["invoice-service", "email-service"]));
        assert!(
            errors.is_empty(),
            "an unmentioned component runs once by default; silence is not a forgotten workload: \
             {errors}"
        );
    }

    #[test]
    fn a_specification_that_states_no_topology_is_refused_for_nothing() {
        let topology = Topology::default();

        assert!(topology.is_empty());
        assert!(
            validate_topology(&topology, &declared(&["invoice-service"])).is_empty(),
            "every specification written before its topology would fail otherwise"
        );
    }

    #[test]
    fn a_replica_floor_of_zero_is_refused() {
        let errors = refuse("workloads:\n  email-service:\n    replicas:\n      min: 0\n");

        assert!(errors.contains(ValidationCode::TypeMismatch));
        assert_eq!(errors.len(), 1, "{errors}");
        let rendered = errors.to_string();
        assert!(
            rendered.contains("topology.workloads.email-service.replicas.min"),
            "{rendered}"
        );
        assert!(rendered.contains("need not run at all"), "{rendered}");
    }

    #[test]
    fn a_replica_ceiling_below_the_floor_is_refused() {
        let errors =
            refuse("workloads:\n  email-service:\n    replicas:\n      min: 3\n      max: 2\n");

        assert!(errors.contains(ValidationCode::ConflictingDeclaration));
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(
            errors.to_string().contains("no number of instances"),
            "{errors}"
        );
    }

    #[test]
    fn an_exact_instance_count_is_not_a_contradiction() {
        let topology =
            accept("workloads:\n  email-service:\n    replicas:\n      min: 2\n      max: 2\n");

        assert_eq!(
            topology.workloads[&component("email-service")].replicas,
            Replicas {
                min: 2,
                max: Some(2)
            }
        );
    }

    #[test]
    fn a_stateful_workload_that_needs_two_instances_is_refused() {
        let errors = refuse(
            "workloads:\n  invoice-service:\n    replicas:\n      min: 2\n    stateless: false\n",
        );

        assert!(errors.contains(ValidationCode::ConflictingDeclaration));
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(
            errors
                .to_string()
                .contains("nothing says how they share it"),
            "the refusal is about the missing third statement, not about state itself: {errors}"
        );
    }

    #[test]
    fn a_stateless_workload_that_needs_two_instances_is_the_ordinary_case() {
        let topology = accept(
            "workloads:\n  email-service:\n    replicas:\n      min: 2\n    stateless: true\n",
        );

        assert_eq!(
            topology.workloads[&component("email-service")].replicas.min,
            2
        );
    }

    #[test]
    fn a_single_instance_may_hold_state() {
        let topology = accept(
            "workloads:\n  invoice-service:\n    replicas:\n      min: 1\n    stateless: false\n",
        );

        assert!(!topology.workloads[&component("invoice-service")].stateless);
    }

    #[test]
    fn a_requirement_that_names_nothing_is_refused() {
        let errors = refuse("workloads:\n  email-service:\n    requires:\n      - {}\n");

        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(
            errors
                .to_string()
                .contains("topology.workloads.email-service.requires[0]"),
            "{errors}"
        );
    }

    #[test]
    fn a_requirement_that_does_not_say_which_instance_is_refused() {
        let errors =
            refuse("workloads:\n  email-service:\n    requires:\n      - postgres: \"\"\n");

        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert!(
            errors
                .to_string()
                .contains("a `postgres` is required but nothing says which one"),
            "{errors}"
        );
    }

    #[test]
    fn a_requirement_that_does_not_say_what_kind_of_thing_it_is_is_refused() {
        let errors =
            refuse("workloads:\n  email-service:\n    requires:\n      - \"\": invoice-store\n");

        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert!(
            errors.to_string().contains("`invoice-store` is required"),
            "{errors}"
        );
    }

    #[test]
    fn two_pairs_in_one_requirement_are_refused() {
        let errors = refuse(
            "workloads:\n  email-service:\n    requires:\n      - {postgres: invoice-store, redis: sessions}\n",
        );

        assert!(errors.contains(ValidationCode::TypeMismatch));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("reads as one thing and means 2"),
            "the document parses; what it says and what it means differ: {rendered}"
        );
        assert!(rendered.contains("postgres, redis"), "{rendered}");
    }

    #[test]
    fn requirements_are_ordered_by_content_so_the_same_topology_serialises_the_same_way() {
        let written_one_way = accept(
            "workloads:\n  invoice-service:\n    requires:\n      - postgres: invoice-store\n      - publish: invoice-events\n",
        );
        let written_the_other = accept(
            "workloads:\n  invoice-service:\n    requires:\n      - publish: invoice-events\n      - postgres: invoice-store\n",
        );

        assert_eq!(
            written_one_way, written_the_other,
            "document order is not part of what a topology means (W2.4)"
        );
    }

    #[test]
    fn two_workloads_claiming_one_component_are_refused() {
        let mut topology = Topology::default();
        topology.workloads.insert(
            component("invoice-service"),
            workload_for("invoice-service"),
        );
        topology
            .workloads
            .insert(component("email-service"), workload_for("invoice-service"));

        let errors = topology.validate();
        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("already has a workload"),
            "{errors}"
        );
    }

    #[test]
    fn a_workload_indexed_under_a_name_it_does_not_claim_is_refused() {
        let mut topology = Topology::default();
        topology
            .workloads
            .insert(component("invoice-service"), workload_for("email-service"));

        let errors = topology.validate();
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
        assert!(
            !errors.contains(ValidationCode::DuplicateDeclaration),
            "one workload is not a duplicate of anything: {errors}"
        );
    }

    #[test]
    fn a_workload_key_that_is_not_a_component_name_is_refused() {
        let errors = refuse("workloads:\n  Invoice_Service:\n    replicas:\n      min: 1\n");

        assert!(errors.contains(ValidationCode::TypeMismatch));
        assert!(
            errors.to_string().contains("component name"),
            "a workload is keyed by a component name, not by a free string: {errors}"
        );
    }

    #[test]
    fn the_cross_cutting_pass_does_not_repeat_what_conversion_already_reported() {
        let mut topology = Topology::default();
        let mut workload = workload_for("email-service");
        workload.replicas = Replicas { min: 0, max: None };
        topology
            .workloads
            .insert(component("email-service"), workload);

        assert!(topology.validate().contains(ValidationCode::TypeMismatch));
        assert!(
            validate_topology(&topology, &declared(&["email-service"])).is_empty(),
            "a specification validates a topology it has already converted; the local rules must \
             not fire twice"
        );
    }

    #[test]
    fn a_topology_with_several_problems_reports_all_of_them() {
        let errors = refuse(
            "workloads:\n  email-service:\n    replicas:\n      min: 0\n    requires:\n      - {}\n  invoice-service:\n    replicas:\n      min: 3\n      max: 2\n    stateless: false\n",
        );

        assert_eq!(
            errors.len(),
            4,
            "one run reports every problem, not the first: {errors}"
        );
        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert!(errors.contains(ValidationCode::TypeMismatch));
        assert!(errors.contains(ValidationCode::ConflictingDeclaration));
    }
}
