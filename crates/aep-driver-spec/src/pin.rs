//! The mandatory workflow pin, as a type.
//!
//! A step map names states and orders steps inside them: it is an instruction sheet for a specific
//! state graph, so an unpinned one is an instruction sheet for whatever happens to be in the tree.
//! A profile that does not pin is saying *"whatever this workflow becomes, I still mean it"*, which
//! is a reasonable thing for a policy document to say; a step map saying it is a document nobody
//! can hold to anything.
//!
//! # Why a newtype rather than a rule in the validator
//!
//! [`WorkflowRef::major`](aep_domain::version::WorkflowRef::major) is an `Option`,
//! [`accepts`](aep_domain::version::WorkflowRef::accepts) returns `true` for an unpinned reference,
//! and the pattern `WorkflowRef` publishes makes the version group optional — which its
//! `JsonSchema` implementation writes verbatim into `schemas/generated/driver-steps.schema.json`.
//! So a validator-only rule would publish a schema that accepts `workflow: adp/default` while the
//! loader refuses it: an editor telling an author their map is fine, and a loader disagreeing.
//! That is invariant 1 inverted, and review finding **F6** is what caught it.
//!
//! [`ProtocolRef`](aep_domain::version::ProtocolRef) is the type-level precedent and not merely a
//! rhetorical one: it holds a non-optional major version and publishes a pattern with no optional
//! group. This is that type, for workflows.

use std::fmt;

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};
use aep_domain::ids::WorkflowId;
use aep_domain::version::{MajorVersion, WorkflowRef};

/// A workflow reference that names a major version, such as `adp/default/1`.
///
/// Obtained only by validating a [`WorkflowRef`], which is what makes possession of one the
/// evidence that the pin is there.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct PinnedWorkflowRef(WorkflowRef);

impl PinnedWorkflowRef {
    /// The pattern published in generated JSON Schema.
    ///
    /// [`WorkflowRef::PATTERN`] with the version group made **required**, which is the whole point
    /// of the type.
    pub const PATTERN: &'static str = "^(workflow:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*/[1-9][0-9]*$";

    /// Builds a pin from an identifier and a major version, for a caller that has both already.
    pub fn new(id: WorkflowId, major: MajorVersion) -> Self {
        Self(WorkflowRef::new(id, Some(major)))
    }

    /// The workflow's identifier.
    pub fn id(&self) -> &WorkflowId {
        self.0.id()
    }

    /// The pinned major version, which a value of this type always has.
    pub fn major(&self) -> MajorVersion {
        self.0
            .major()
            .expect("a pinned reference carries a major version")
    }

    /// The underlying reference, for the registry lookups that take one.
    pub fn reference(&self) -> &WorkflowRef {
        &self.0
    }

    /// `true` when this pin accepts a workflow at `candidate`.
    ///
    /// Equality, because the pin is mandatory: the orphaning of a map whose workflow reached a new
    /// major is this function returning `false`, and it needs no new code to happen.
    pub fn accepts(&self, candidate: MajorVersion) -> bool {
        self.0.accepts(candidate)
    }
}

impl TryFrom<WorkflowRef> for PinnedWorkflowRef {
    type Error = ValidationErrors;

    fn try_from(reference: WorkflowRef) -> Result<Self, Self::Error> {
        if reference.major().is_none() {
            return Err(ValidationErrors::from(
                ValidationError::new(
                    ValidationCode::MissingDeclaration,
                    "driver-steps.workflow",
                    format!(
                        "`{reference}` does not pin a major version, and a step map is written \
                         against a specific one"
                    ),
                )
                .with_hint(format!(
                    "write `{reference}/1`, naming the major version this map's states and step \
                     order were written against"
                )),
            ));
        }
        Ok(Self(reference))
    }
}

impl fmt::Display for PinnedWorkflowRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for PinnedWorkflowRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PinnedWorkflowRef({})", self.0)
    }
}

impl From<PinnedWorkflowRef> for String {
    fn from(value: PinnedWorkflowRef) -> Self {
        value.to_string()
    }
}

impl schemars::JsonSchema for PinnedWorkflowRef {
    fn schema_name() -> String {
        "PinnedWorkflowRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "Reference to a workflow at a major version, such as `adp/default/1`. The version is \
             mandatory: a step map is an instruction sheet for one state graph."
                .to_owned(),
        );
        schema.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(value: &str) -> WorkflowRef {
        value.parse().expect("a workflow reference")
    }

    #[test]
    fn a_pinned_reference_validates_and_keeps_its_spelling() {
        let pin = PinnedWorkflowRef::try_from(reference("adp/default/1")).expect("pinned");
        assert_eq!(pin.to_string(), "adp/default/1");
        assert_eq!(pin.id().as_str(), "adp/default");
        assert_eq!(pin.major(), MajorVersion::V1);
    }

    #[test]
    fn an_unpinned_reference_is_refused_with_a_code_and_a_repair() {
        let errors = PinnedWorkflowRef::try_from(reference("adp/default")).expect_err("refused");
        assert_eq!(errors.len(), 1);
        assert!(errors.contains(ValidationCode::MissingDeclaration));
        let error = &errors.as_slice()[0];
        assert_eq!(error.location, "driver-steps.workflow");
        assert!(
            error
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("adp/default/1")),
            "the refusal names the document that has to be written, not just the rule"
        );
    }

    #[test]
    fn the_published_pattern_requires_the_version_group_the_reference_type_leaves_optional() {
        // The defect F6 names is these two disagreeing, so both are asserted here rather than one.
        assert!(WorkflowRef::PATTERN.contains("(/[1-9][0-9]*)?"));
        assert!(!PinnedWorkflowRef::PATTERN.contains("(/[1-9][0-9]*)?"));
        assert!(PinnedWorkflowRef::PATTERN.ends_with("/[1-9][0-9]*$"));
    }

    #[test]
    fn a_pin_accepts_only_its_own_major() {
        let pin = PinnedWorkflowRef::try_from(reference("adp/default/1")).expect("pinned");
        assert!(pin.accepts(MajorVersion::V1));
        assert!(!pin.accepts(MajorVersion::new(2).expect("a major version")));
    }
}
