//! The documents in force.
//!
//! A [`Registry`] holds validated protocols, principles, workflows, profiles, artifact lifecycles
//! and driver step maps, indexed by the id **declared inside each document** — never by filename,
//! so moving a file cannot change what a profile resolves to.
//!
//! It also owns the cross-document checks. Individual documents validate themselves in isolation
//! (`TryFrom`); only the registry can see that a profile references a principle nobody wrote, or
//! that a protocol declares evidence no verifier can establish.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::artifact::{ArtifactKind, ArtifactLifecycle, LifecycleRegistry};
use aep_domain::capability::Capability;
use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};
use aep_domain::evidence::EvidenceKind;
use aep_domain::ids::{PrincipleId, ProfileId, ProtocolId, WorkflowId};
use aep_domain::predicate::Predicate;
use aep_domain::principle::Principle;
use aep_domain::profile::Profile;
use aep_domain::protocol::{is_supported_major, Protocol};
use aep_domain::requirement::RequirementSet;
use aep_domain::version::{
    MajorVersion, PrincipleRef, ProfileVersionedRef, ProtocolRef, WorkflowRef,
};
use aep_domain::workflow::Workflow;
use aep_driver_spec::map::{StepMap, StepMapId};

/// How deep an `extends` chain may go before it is treated as a loop.
const MAX_EXTENDS_DEPTH: usize = 8;

/// Every document available for resolution.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    protocols: BTreeMap<(ProtocolId, MajorVersion), Protocol>,
    principles: BTreeMap<PrincipleId, Principle>,
    workflows: BTreeMap<WorkflowId, Workflow>,
    profiles: BTreeMap<ProfileId, Profile>,
    lifecycles: LifecycleRegistry,
    step_maps: BTreeMap<StepMapId, StepMap>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a protocol. A second document for the same id and major version is an error.
    pub fn insert_protocol(&mut self, protocol: Protocol) -> Result<(), ValidationError> {
        let key = (protocol.id.clone(), protocol.version);
        if self.protocols.contains_key(&key) {
            return Err(duplicate("protocol", &format!("{}/{}", key.0, key.1)));
        }
        self.protocols.insert(key, protocol);
        Ok(())
    }

    /// Adds a principle.
    pub fn insert_principle(&mut self, principle: Principle) -> Result<(), ValidationError> {
        if self.principles.contains_key(&principle.id) {
            return Err(duplicate("principle", principle.id.as_str()));
        }
        self.principles.insert(principle.id.clone(), principle);
        Ok(())
    }

    /// Adds a workflow.
    pub fn insert_workflow(&mut self, workflow: Workflow) -> Result<(), ValidationError> {
        if self.workflows.contains_key(&workflow.id) {
            return Err(duplicate("workflow", workflow.id.as_str()));
        }
        self.workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    /// Adds a profile.
    pub fn insert_profile(&mut self, profile: Profile) -> Result<(), ValidationError> {
        if self.profiles.contains_key(&profile.id) {
            return Err(duplicate("profile", profile.id.as_str()));
        }
        self.profiles.insert(profile.id.clone(), profile);
        Ok(())
    }

    /// Adds an artifact lifecycle.
    pub fn insert_lifecycle(
        &mut self,
        lifecycle: ArtifactLifecycle,
    ) -> Result<(), ValidationError> {
        let Some(kind) = lifecycle.kind.clone() else {
            return Err(ValidationError::new(
                ValidationCode::EmptyDeclaration,
                "lifecycle.kind",
                "a lifecycle document must name the artifact kind it governs",
            )
            .with_hint("add `kind: design`"));
        };
        if self.lifecycles.for_kind_exact(&kind).is_some() {
            return Err(duplicate("lifecycle", kind.as_str()));
        }
        self.lifecycles.insert(kind, lifecycle);
        Ok(())
    }

    /// Adds a driver's step map.
    ///
    /// Structural validation has already happened; what the registry adds is the half only it can
    /// see — that the workflow the map pins is in the tree, at the major version it pinned. That
    /// check lives in [`Registry::validate`] rather than here, because a map may be inserted before
    /// the workflow it names has been read.
    pub fn insert_step_map(&mut self, map: StepMap) -> Result<(), ValidationError> {
        if self.step_maps.contains_key(&map.id) {
            return Err(duplicate("step map", map.id.as_str()));
        }
        self.step_maps.insert(map.id.clone(), map);
        Ok(())
    }

    /// The step map registered under `id`.
    pub fn step_map(&self, id: &StepMapId) -> Option<&StepMap> {
        self.step_maps.get(id)
    }

    /// Every registered step map.
    pub fn step_maps(&self) -> impl Iterator<Item = &StepMap> {
        self.step_maps.values()
    }

    /// The protocol registered for this reference, at exactly its major version.
    pub fn protocol(&self, reference: &ProtocolRef) -> Option<&Protocol> {
        self.protocols
            .get(&(reference.protocol().clone(), reference.major()))
    }

    /// The principle registered under this reference's id, if its version is acceptable.
    pub fn principle(&self, reference: &PrincipleRef) -> Option<&Principle> {
        self.principles
            .get(reference.id())
            .filter(|principle| reference.accepts(principle.version))
    }

    /// The workflow registered under this reference's id, if its version is acceptable.
    pub fn workflow(&self, reference: &WorkflowRef) -> Option<&Workflow> {
        self.workflows
            .get(reference.id())
            .filter(|workflow| reference.accepts(workflow.version))
    }

    /// The profile registered under this reference's id, if its version is acceptable.
    pub fn profile(&self, reference: &ProfileVersionedRef) -> Option<&Profile> {
        self.profiles
            .get(reference.id())
            .filter(|profile| reference.accepts(profile.version))
    }

    /// The artifact lifecycles, for validating a manifest.
    pub fn lifecycles(&self) -> &LifecycleRegistry {
        &self.lifecycles
    }

    /// Every registered protocol.
    pub fn protocols(&self) -> impl Iterator<Item = &Protocol> {
        self.protocols.values()
    }

    /// Every registered principle.
    pub fn principles(&self) -> impl Iterator<Item = &Principle> {
        self.principles.values()
    }

    /// Every registered workflow.
    pub fn workflows(&self) -> impl Iterator<Item = &Workflow> {
        self.workflows.values()
    }

    /// Every registered profile.
    pub fn profiles(&self) -> impl Iterator<Item = &Profile> {
        self.profiles.values()
    }

    /// How many documents are registered.
    pub fn len(&self) -> usize {
        self.protocols.len()
            + self.principles.len()
            + self.workflows.len()
            + self.profiles.len()
            + self.lifecycles.len()
            + self.step_maps.len()
    }

    /// `true` when nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The protocol for `reference`, with its whole `extends` chain merged in.
    pub fn resolved_protocol(&self, reference: &ProtocolRef) -> Result<Protocol, ValidationErrors> {
        let mut chain = Vec::new();
        let mut current = reference.clone();

        loop {
            let Some(protocol) = self.protocol(&current) else {
                return Err(missing(
                    ValidationCode::UnknownProtocol,
                    "protocol",
                    &current.to_string(),
                    self.protocols
                        .keys()
                        .map(|(id, major)| format!("{id}/{major}"))
                        .collect(),
                )
                .into());
            };
            if !is_supported_major(protocol.version) {
                return Err(ValidationError::new(
                    ValidationCode::UnsupportedProtocolVersion,
                    format!("protocol {current}"),
                    format!(
                        "this build does not implement major version {}",
                        protocol.version
                    ),
                )
                .into());
            }
            if chain.len() >= MAX_EXTENDS_DEPTH {
                return Err(ValidationError::new(
                    ValidationCode::UnknownProtocol,
                    format!("protocol {reference}.extends"),
                    format!("the extends chain is longer than {MAX_EXTENDS_DEPTH}, which means it loops"),
                )
                .into());
            }
            chain.push(protocol);
            match &protocol.extends {
                Some(base) => current = base.clone(),
                None => break,
            }
        }

        // Merge from the base upwards, so a derived protocol's own values win.
        let mut merged = chain
            .pop()
            .expect("the chain always holds at least the referenced protocol")
            .clone();
        while let Some(derived) = chain.pop() {
            merged = derived.extend(&merged);
        }
        Ok(merged)
    }

    /// The profile for `reference`, with its whole `extends` chain merged in.
    pub fn resolved_profile(
        &self,
        reference: &ProfileVersionedRef,
    ) -> Result<Profile, ValidationErrors> {
        let mut chain = Vec::new();
        let mut current = reference.clone();

        loop {
            let Some(profile) = self.profile(&current) else {
                return Err(missing(
                    ValidationCode::UnknownProfile,
                    "profile",
                    &current.to_string(),
                    self.profiles.keys().map(ToString::to_string).collect(),
                )
                .into());
            };
            if chain.len() >= MAX_EXTENDS_DEPTH {
                return Err(ValidationError::new(
                    ValidationCode::UnknownProfile,
                    format!("profile {reference}.extends"),
                    format!("the extends chain is longer than {MAX_EXTENDS_DEPTH}, which means it loops"),
                )
                .into());
            }
            chain.push(profile);
            match &profile.extends {
                Some(base) => current = base.clone(),
                None => break,
            }
        }

        let mut merged = chain
            .pop()
            .expect("the chain always holds at least the referenced profile")
            .clone();
        while let Some(derived) = chain.pop() {
            merged = derived.extend(&merged);
        }
        Ok(merged)
    }

    /// Checks every registered document against the others.
    ///
    /// Returns every problem found, so one run tells the whole story.
    pub fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        for protocol in self.protocols.values() {
            let reference = protocol.reference();
            match self.resolved_protocol(&reference) {
                Ok(resolved) => {
                    for kind in &resolved.evidence_kinds {
                        if !resolved.can_establish(*kind) {
                            errors.push(
                                ValidationError::new(
                                    ValidationCode::NoVerifierForEvidence,
                                    format!("protocol {reference}.evidence_kinds"),
                                    format!("`{kind}` is declared but no declared verifier can establish it"),
                                )
                                .with_hint(format!(
                                    "declare one of: {}",
                                    kind.default_verifiers()
                                        .iter()
                                        .map(ToString::to_string)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )),
                            );
                        }
                    }
                    for capability in &resolved.approval_floor {
                        if !resolved.declares_capability(capability) {
                            errors.push(ValidationError::new(
                                ValidationCode::UndeclaredCapability,
                                format!("protocol {reference}.approval_floor"),
                                format!(
                                    "`{capability}` is in the approval floor but is not declared"
                                ),
                            ));
                        }
                    }
                }
                Err(chain_errors) => errors.extend(chain_errors),
            }
        }

        for profile in self.profiles.values() {
            errors.extend(self.validate_profile(profile));
        }

        for map in self.step_maps.values() {
            errors.extend(self.validate_step_map(map));
        }

        errors
    }

    /// Checks one step map against the workflow it pins.
    ///
    /// A major bump orphans a map with no new code: the lookup filters on `WorkflowRef::accepts`,
    /// which for a pinned reference is equality, so a map pinned to `/1` against a registry holding
    /// `version: 2` resolves to `None`. This turns that `None` into an accumulating validation
    /// error naming the map, the pin and the version that is actually present — not a warning and
    /// not a fallback.
    fn validate_step_map(&self, map: &StepMap) -> ValidationErrors {
        let location = format!("step map {}", map.id);
        let Some(workflow) = self.workflow(map.workflow.reference()) else {
            let present = self.workflows.get(map.workflow.id());
            let error = match present {
                Some(workflow) => ValidationError::new(
                    ValidationCode::VersionMismatch,
                    location,
                    format!(
                        "the map pins `{}` and the tree holds `{}` at version {}",
                        map.workflow, workflow.id, workflow.version
                    ),
                )
                .with_hint(
                    "a major version exists because the change could not be expressed additively, \
                     so the map is rewritten against the new state graph rather than migrated",
                ),
                None => ValidationError::new(
                    ValidationCode::UnknownWorkflow,
                    location,
                    format!("no workflow `{}` is in the tree", map.workflow.id()),
                ),
            };
            return ValidationErrors::from(error);
        };
        map.cross_validate(workflow)
    }

    /// Checks one profile against the protocol, workflow and principles it names.
    fn validate_profile(&self, profile: &Profile) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let location = format!("profile {}", profile.id);

        let resolved =
            match self.resolved_profile(&ProfileVersionedRef::unpinned(profile.id.clone())) {
                Ok(resolved) => resolved,
                Err(chain_errors) => return chain_errors,
            };

        let protocol = match self.resolved_protocol(&resolved.protocol) {
            Ok(protocol) => protocol,
            Err(protocol_errors) => {
                errors.extend(protocol_errors);
                return errors;
            }
        };

        match &resolved.workflow {
            Some(reference) => {
                if let Some(workflow) = self.workflow(reference) {
                    errors.extend(Self::validate_workflow_against(workflow, &protocol));
                } else {
                    errors.push(missing(
                        ValidationCode::UnknownWorkflow,
                        "workflow",
                        &reference.to_string(),
                        self.workflows.keys().map(ToString::to_string).collect(),
                    ));
                }
            }
            None => errors.push(ValidationError::new(
                ValidationCode::UnknownWorkflow,
                format!("{location}.workflow"),
                "no workflow is named, and none is inherited",
            )),
        }

        for reference in &resolved.principles {
            match self.principles.get(reference.id()) {
                None => errors.push(missing(
                    ValidationCode::UnknownPrinciple,
                    "principle",
                    &reference.to_string(),
                    self.principles.keys().map(ToString::to_string).collect(),
                )),
                Some(principle) if !reference.accepts(principle.version) => {
                    errors.push(ValidationError::new(
                        ValidationCode::VersionMismatch,
                        format!("{location}.principles"),
                        format!(
                            "`{reference}` is pinned, but the registry holds version {}",
                            principle.version
                        ),
                    ));
                }
                Some(principle) => {
                    errors.extend(check_principle_vocabulary(principle, &protocol));
                }
            }
        }

        for capability in resolved.capabilities.mentioned() {
            if !protocol.declares_capability(capability) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredCapability,
                        format!("{location}.capabilities"),
                        format!(
                            "`{capability}` is not declared by protocol {}",
                            resolved.protocol
                        ),
                    )
                    .with_hint("add it to the protocol's `capabilities`, or remove it here"),
                );
            }
        }

        errors.extend(check_requirements(
            &resolved.completion,
            &protocol,
            &format!("{location}.completion"),
        ));

        errors
    }

    /// Checks a workflow's guards and state requirements against a protocol's vocabulary.
    fn validate_workflow_against(workflow: &Workflow, protocol: &Protocol) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        for state in workflow.states.values() {
            let location = format!("workflow {}.states.{}", workflow.id, state.id);
            errors.extend(check_requirements(&state.requires, protocol, &location));
            for capability in state.capabilities.mentioned() {
                if !protocol.declares_capability(capability) {
                    errors.push(ValidationError::new(
                        ValidationCode::UndeclaredCapability,
                        format!("{location}.capabilities"),
                        format!(
                            "`{capability}` is not declared by protocol {}",
                            protocol.reference()
                        ),
                    ));
                }
            }
        }

        for transition in &workflow.transitions {
            let location = format!(
                "workflow {}.transitions[{} -> {}]",
                workflow.id, transition.from, transition.to
            );
            errors.extend(check_predicate(&transition.when, protocol, &location));
            errors.extend(check_requirements(
                &transition.requires,
                protocol,
                &location,
            ));
        }

        errors
    }
}

/// Checks that a principle only uses vocabulary the protocol declares.
fn check_principle_vocabulary(principle: &Principle, protocol: &Protocol) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    let location = format!("principle {}", principle.id);

    errors.extend(check_predicate(
        &principle.applicability,
        protocol,
        &format!("{location}.applies_when"),
    ));

    for obligation in &principle.obligations {
        errors.extend(check_requirements(
            &obligation.requires,
            protocol,
            &format!("{location}.obligations.{}", obligation.id),
        ));
    }

    for requirement in &principle.evidence {
        if !protocol.declares_evidence(requirement.kind) {
            errors.push(undeclared_evidence(
                requirement.kind,
                protocol,
                &format!("{location}.evidence"),
            ));
        } else if !protocol.can_establish(requirement.kind) {
            errors.push(ValidationError::new(
                ValidationCode::NoVerifierForEvidence,
                format!("{location}.evidence"),
                format!(
                    "`{}` is required but protocol {} declares no verifier that can establish it",
                    requirement.kind,
                    protocol.reference()
                ),
            ));
        }
    }

    for requirement in &principle.verification {
        if !protocol.verifiers.contains(&requirement.verifier) {
            errors.push(
                ValidationError::new(
                    ValidationCode::NoVerifierForEvidence,
                    format!("{location}.verification"),
                    format!(
                        "verifier `{}` is required but protocol {} does not declare it",
                        requirement.verifier,
                        protocol.reference()
                    ),
                )
                .with_hint("declare the verifier in the protocol, or require a different one"),
            );
        }
    }

    for capability in principle.capabilities.mentioned() {
        if !protocol.declares_capability(capability) {
            errors.push(ValidationError::new(
                ValidationCode::UndeclaredCapability,
                format!("{location}.capabilities"),
                format!(
                    "`{capability}` is not declared by protocol {}",
                    protocol.reference()
                ),
            ));
        }
    }

    errors
}

/// Checks a requirement set's predicates, evidence kinds and artifact kinds.
pub(crate) fn check_requirements(
    requirements: &RequirementSet,
    protocol: &Protocol,
    location: &str,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();

    for predicate in requirements.all_predicates() {
        errors.extend(check_predicate(predicate, protocol, location));
    }

    for kind in requirements.required_evidence_kinds() {
        if protocol.declares_evidence(kind) {
            if !protocol.can_establish(kind) {
                errors.push(ValidationError::new(
                    ValidationCode::NoVerifierForEvidence,
                    location.to_owned(),
                    format!(
                        "`{kind}` is required but protocol {} declares no verifier that can \
                         establish it",
                        protocol.reference()
                    ),
                ));
            }
        } else {
            errors.push(undeclared_evidence(kind, protocol, location));
        }
    }

    if !protocol.artifact_kinds.is_empty() {
        for requirement in &requirements.artifacts {
            if !declares_artifact_kind(protocol, &requirement.kind) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredEvidenceKind,
                        location.to_owned(),
                        format!(
                            "artifact kind `{}` is required but protocol {} does not declare it",
                            requirement.kind,
                            protocol.reference()
                        ),
                    )
                    .with_hint("add it to the protocol's `artifact_kinds`"),
                );
            }
        }
    }

    errors
}

/// `true` when the protocol declares this artifact kind, or a kind it specialises.
fn declares_artifact_kind(protocol: &Protocol, kind: &ArtifactKind) -> bool {
    kind.lineage()
        .iter()
        .any(|candidate| protocol.artifact_kinds.contains(candidate))
}

/// Checks that every fact a predicate reads is declared observable.
///
/// This is what catches a typo in a completion condition. Without it, `tests.unit.faild == 0` is
/// simply never satisfiable, and nothing says why.
pub(crate) fn check_predicate(
    predicate: &Predicate,
    protocol: &Protocol,
    location: &str,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    for path in predicate.fact_paths() {
        if !protocol.is_observable(path) {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnobservableFact,
                    location.to_owned(),
                    format!(
                        "`{path}` is not declared observable by protocol {}",
                        protocol.reference()
                    ),
                )
                .with_hint(format!(
                    "declared families: {}",
                    protocol
                        .observables
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
        }
    }
    errors
}

/// Builds an undeclared-evidence error.
fn undeclared_evidence(kind: EvidenceKind, protocol: &Protocol, location: &str) -> ValidationError {
    ValidationError::new(
        ValidationCode::UndeclaredEvidenceKind,
        location.to_owned(),
        format!(
            "`{kind}` is not declared by protocol {}",
            protocol.reference()
        ),
    )
    .with_hint(format!(
        "declared kinds: {}",
        protocol
            .evidence_kinds
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Builds a duplicate-document error.
fn duplicate(kind: &str, id: &str) -> ValidationError {
    ValidationError::new(
        ValidationCode::DuplicatePrinciple,
        format!("{kind} {id}"),
        format!("a second {kind} document declares the id `{id}`"),
    )
    .with_hint("identifiers come from document contents, so two files cannot share one")
}

/// Builds a missing-document error listing what is available.
fn missing(
    code: ValidationCode,
    kind: &str,
    reference: &str,
    available: BTreeSet<String>,
) -> ValidationError {
    let error = ValidationError::new(
        code,
        format!("{kind} {reference}"),
        format!("no {kind} document declares `{reference}`"),
    );
    if available.is_empty() {
        error.with_hint(format!("no {kind} documents are loaded at all"))
    } else {
        error.with_hint(format!(
            "available: {}",
            available.into_iter().collect::<Vec<_>>().join(", ")
        ))
    }
}

/// Capabilities a policy grants outright, for the approval-floor check.
pub(crate) fn granted_outright(
    policy: &aep_domain::CapabilityPolicy,
) -> impl Iterator<Item = &Capability> {
    policy.allow.iter().filter(move |capability| {
        policy.decide(capability) == aep_domain::CapabilityDecision::Allowed
    })
}
