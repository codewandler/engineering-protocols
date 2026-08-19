//! The typed entity bodies development work adds to the entity model.
//!
//! Four bodies, each an [`EntityBody`]: the interaction contract keeps moving untyped
//! [`Node`]s, and an application still gets to hold a `Specification` (§48). The conversion is
//! the whole of the type's wire behaviour, so each body is written as a `to_node`/`from_node`
//! pair with a round-trip test — a field that survives writing but not reading is a silent data
//! loss, and it is cheaper to catch here than in a backend.
//!
//! # Why these are not `Deserialize`
//!
//! None of these types implement [`serde::Deserialize`]. They are validated types: the only way
//! to obtain one from a document is [`EntityBody::from_node`], which parses identifiers and
//! statuses rather than accepting whatever the wire said. Adding a derive to save a conversion
//! would put a second, unchecked door into the same room.
//!
//! # Strictness
//!
//! An unknown field is ignored — a newer writer may know a field this build does not, and
//! refusing the whole entity for that would make every additive change a breaking one. A known
//! field that is present in the wrong shape is *rejected*, because the alternative is dropping
//! it and reporting success.

use std::collections::BTreeMap;

use aep_domain::artifact::{ArtifactStatus, Revision};
use aep_domain::entity::{
    optional_text, required_text, EntityBody, EntityId, EntityRef, EntityType,
};
use aep_domain::error::ParseError;
use aep_domain::evidence::TestSuite;
use aep_domain::facts::Number;
use aep_domain::node::Node;

/// The versioned type name of [`Specification`], as it appears on the wire.
pub const SPECIFICATION_TYPE: &str = "adp.specification/v1";
/// The versioned type name of [`TestPlan`].
pub const TEST_PLAN_TYPE: &str = "adp.test-plan/v1";
/// The versioned type name of [`AcceptanceCriteria`].
pub const ACCEPTANCE_CRITERIA_TYPE: &str = "adp.acceptance-criteria/v1";
/// The versioned type name of [`ChangeSet`].
pub const CHANGE_TYPE: &str = "adp.change/v1";

// The dotted prefixes parse errors are reported under. They drop the `/v1` the wire name carries:
// a diagnostic points at a place in a document (`adp.change.files_changed`), and reading
// `adp.change/v1.files_changed` costs the reader a second to parse for no information.
/// Where a [`Specification`] problem is reported.
const SPECIFICATION_LOCATION: &str = "adp.specification";
/// Where a [`TestPlan`] problem is reported.
const TEST_PLAN_LOCATION: &str = "adp.test-plan";
/// Where an [`AcceptanceCriteria`] problem is reported.
const ACCEPTANCE_CRITERIA_LOCATION: &str = "adp.acceptance-criteria";
/// Where a [`ChangeSet`] problem is reported.
const CHANGE_LOCATION: &str = "adp.change";

/// The largest integer an `f64` carries exactly.
///
/// A count beyond it has stopped being a count and started being a rounding artefact, so
/// [`ChangeSet::files_changed`] refuses it rather than reading back a different number from the
/// one that was written.
const MAX_EXACT_COUNT: f64 = 9_007_199_254_740_992.0;

/// One numbered thing a specification demands, and what establishes it.
///
/// `verified_by` is the link that turns a requirement from prose into something checkable: it
/// names the verification claim or verifier that decides the requirement — `contract`,
/// `verification.invariant.passed`, a test selector. It is optional because a specification is
/// written before its verifiers exist; a requirement that still has no `verified_by` when the
/// work reaches verification is exactly the gap worth seeing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Requirement {
    /// A stable label within the specification, such as `R-3`.
    pub id: String,
    /// What must be true.
    pub statement: String,
    /// What establishes it, when something does.
    pub verified_by: Option<String>,
}

impl Requirement {
    /// A requirement nothing verifies yet.
    pub fn new(id: impl Into<String>, statement: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            statement: statement.into(),
            verified_by: None,
        }
    }

    /// The same requirement, with what verifies it.
    #[must_use]
    pub fn verified_by(mut self, verifier: impl Into<String>) -> Self {
        self.verified_by = Some(verifier.into());
        self
    }

    /// Renders the requirement for the wire.
    pub fn to_node(&self) -> Node {
        let mut entries = BTreeMap::from([
            ("id".to_owned(), Node::from(self.id.as_str())),
            ("statement".to_owned(), Node::from(self.statement.as_str())),
        ]);
        if let Some(verified_by) = &self.verified_by {
            entries.insert("verified_by".to_owned(), Node::from(verified_by.as_str()));
        }
        Node::Map(entries)
    }

    /// Reads a requirement back.
    pub fn from_node(node: &Node, location: &str) -> Result<Self, ParseError> {
        Ok(Self {
            id: required_text(node, "id", location)?,
            statement: required_text(node, "statement", location)?,
            verified_by: optional_text(node, "verified_by"),
        })
    }
}

/// What the work must do, in terms a verifier can disagree with.
///
/// The development profile's completion condition is `specification.satisfied`, not "the tests
/// pass" — a passing suite cannot tell you it covers the wrong thing. That condition needs a
/// specification whose requirements are individually addressable, which is why
/// [`Requirement`] is a list of identified statements rather than a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specification {
    /// What it is called.
    pub title: String,
    /// What it is about, in one paragraph.
    pub summary: String,
    /// The individual demands, each addressable by id.
    pub requirements: Vec<Requirement>,
    /// Where it stands in the published specification lifecycle.
    pub status: ArtifactStatus,
}

impl Specification {
    /// A draft specification with no requirements yet.
    pub fn new(title: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            summary: summary.into(),
            requirements: Vec::new(),
            status: ArtifactStatus::Draft,
        }
    }

    /// The requirement with this id, if the specification states one.
    pub fn requirement(&self, id: &str) -> Option<&Requirement> {
        self.requirements
            .iter()
            .find(|requirement| requirement.id == id)
    }
}

impl EntityBody for Specification {
    fn entity_type() -> EntityType {
        adp_type("specification")
    }

    fn to_node(&self) -> Node {
        Node::Map(BTreeMap::from([
            ("title".to_owned(), Node::from(self.title.as_str())),
            ("summary".to_owned(), Node::from(self.summary.as_str())),
            (
                "requirements".to_owned(),
                Node::Seq(self.requirements.iter().map(Requirement::to_node).collect()),
            ),
            ("status".to_owned(), status_node(self.status)),
        ]))
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        let entries = mapping(node, SPECIFICATION_LOCATION)?;
        let requirements = sequence(entries, "requirements", SPECIFICATION_LOCATION)?
            .iter()
            .enumerate()
            .map(|(index, item)| {
                Requirement::from_node(
                    item,
                    &format!("{SPECIFICATION_LOCATION}.requirements[{index}]"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            title: required_text(node, "title", SPECIFICATION_LOCATION)?,
            summary: required_text(node, "summary", SPECIFICATION_LOCATION)?,
            requirements,
            status: status(entries, SPECIFICATION_LOCATION)?,
        })
    }
}

/// How the work will be shown to do what the specification says.
///
/// Written *before* the implementation — the workflow's `establish_verifiers → implement`
/// transition is guarded on `test.exists` — so the interesting field is `subject`: a test plan
/// that does not say what it tests cannot be evidence for anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPlan {
    /// What it is called.
    pub title: String,
    /// What it tests: a specification, an acceptance-criteria set or a story.
    pub subject: EntityRef,
    /// Which bodies of tests will run.
    pub suites: Vec<TestSuite>,
    /// The verification claims the plan intends to establish, such as `invariant` or
    /// `postcondition`.
    ///
    /// Claim ids are singular and shared with the fact vocabulary: a claim recorded here is read
    /// back as `verification.<claim>.passed`, so `invariant` and `invariants` are different
    /// claims and evidence for one does not satisfy a requirement for the other.
    pub claims: Vec<String>,
    /// What must hold before the plan counts as discharged.
    pub exit_criteria: Vec<String>,
    /// Where it stands.
    pub status: ArtifactStatus,
}

impl TestPlan {
    /// A draft plan for `subject`, with nothing planned yet.
    pub fn new(title: impl Into<String>, subject: EntityRef) -> Self {
        Self {
            title: title.into(),
            subject,
            suites: Vec::new(),
            claims: Vec::new(),
            exit_criteria: Vec::new(),
            status: ArtifactStatus::Draft,
        }
    }
}

impl EntityBody for TestPlan {
    fn entity_type() -> EntityType {
        adp_type("test-plan")
    }

    fn to_node(&self) -> Node {
        Node::Map(BTreeMap::from([
            ("title".to_owned(), Node::from(self.title.as_str())),
            ("subject".to_owned(), reference_node(&self.subject)),
            (
                "suites".to_owned(),
                Node::Seq(
                    self.suites
                        .iter()
                        .map(|suite| Node::from(suite.as_str()))
                        .collect(),
                ),
            ),
            ("claims".to_owned(), text_node(&self.claims)),
            ("exit_criteria".to_owned(), text_node(&self.exit_criteria)),
            ("status".to_owned(), status_node(self.status)),
        ]))
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        let entries = mapping(node, TEST_PLAN_LOCATION)?;
        let suites = sequence(entries, "suites", TEST_PLAN_LOCATION)?
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let name = item.as_text().ok_or_else(|| {
                    ParseError::shape(
                        format!("{TEST_PLAN_LOCATION}.suites[{index}]"),
                        "a suite name",
                        item.type_name(),
                    )
                })?;
                TestSuite::parse(name)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            title: required_text(node, "title", TEST_PLAN_LOCATION)?,
            subject: reference(entries, "subject", TEST_PLAN_LOCATION)?,
            suites,
            claims: text_list(entries, "claims", TEST_PLAN_LOCATION)?,
            exit_criteria: text_list(entries, "exit_criteria", TEST_PLAN_LOCATION)?,
            status: status(entries, TEST_PLAN_LOCATION)?,
        })
    }
}

/// The conditions under which a story is accepted.
///
/// Distinct from a [`Specification`] on purpose: a specification says what the system must do,
/// acceptance criteria say when *this piece of work* is finished. They diverge in practice, and
/// collapsing them loses the ability to say that a story is done while its specification is not
/// yet fully satisfied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceCriteria {
    /// The story being accepted.
    pub story: EntityRef,
    /// Each condition, in the order they are meant to be read.
    pub criteria: Vec<String>,
    /// Where it stands.
    pub status: ArtifactStatus,
}

impl AcceptanceCriteria {
    /// A draft set of criteria for `story`.
    pub fn new(story: EntityRef) -> Self {
        Self {
            story,
            criteria: Vec::new(),
            status: ArtifactStatus::Draft,
        }
    }
}

impl EntityBody for AcceptanceCriteria {
    fn entity_type() -> EntityType {
        adp_type("acceptance-criteria")
    }

    fn to_node(&self) -> Node {
        Node::Map(BTreeMap::from([
            ("story".to_owned(), reference_node(&self.story)),
            ("criteria".to_owned(), text_node(&self.criteria)),
            ("status".to_owned(), status_node(self.status)),
        ]))
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        let entries = mapping(node, ACCEPTANCE_CRITERIA_LOCATION)?;
        Ok(Self {
            story: reference(entries, "story", ACCEPTANCE_CRITERIA_LOCATION)?,
            criteria: text_list(entries, "criteria", ACCEPTANCE_CRITERIA_LOCATION)?,
            status: status(entries, ACCEPTANCE_CRITERIA_LOCATION)?,
        })
    }
}

/// A recorded implementation: what changed, and what it was for.
///
/// This is the entity [`CompleteStory`](crate::command::CompleteStory) points at, and it is why
/// completing a story is more than a status assignment — the change is addressable, so "which
/// specification did this satisfy?" has an answer that survives the branch being deleted.
///
/// It carries no `status`, and its type descriptor says it is not mutable, for the same reason a
/// review result is not: it records what happened at a moment. A record that can be improved
/// afterwards is not provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// The source-control revision it landed as, when it has landed.
    ///
    /// Optional because the record is written by whatever observed the change, which does not
    /// always know the final commit — a squash or a rebase assigns it later. This is an
    /// [`aep_domain::artifact::Revision`], a commit-shaped label, not the entity revision that
    /// optimistic concurrency compares.
    pub revision: Option<Revision>,
    /// How many files it touched.
    pub files_changed: usize,
    /// What it did, in one line.
    pub summary: String,
    /// What it was for: the specifications, criteria or designs it realises.
    pub implements: Vec<EntityRef>,
}

impl ChangeSet {
    /// A change that has not been attributed to a revision yet.
    pub fn new(summary: impl Into<String>, files_changed: usize) -> Self {
        Self {
            revision: None,
            files_changed,
            summary: summary.into(),
            implements: Vec::new(),
        }
    }

    /// The same change, attributed to a source-control revision.
    #[must_use]
    pub fn at_revision(mut self, revision: Revision) -> Self {
        self.revision = Some(revision);
        self
    }
}

impl EntityBody for ChangeSet {
    fn entity_type() -> EntityType {
        adp_type("change")
    }

    fn to_node(&self) -> Node {
        let mut entries = BTreeMap::from([
            (
                "files_changed".to_owned(),
                Node::Number(Number::from(self.files_changed)),
            ),
            ("summary".to_owned(), Node::from(self.summary.as_str())),
            (
                "implements".to_owned(),
                Node::Seq(self.implements.iter().map(reference_node).collect()),
            ),
        ]);
        if let Some(revision) = &self.revision {
            entries.insert("revision".to_owned(), Node::from(revision.as_str()));
        }
        Node::Map(entries)
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        let entries = mapping(node, CHANGE_LOCATION)?;
        Ok(Self {
            revision: optional_text(node, "revision").map(Revision::new),
            files_changed: count(entries, "files_changed", CHANGE_LOCATION)?,
            summary: required_text(node, "summary", CHANGE_LOCATION)?,
            implements: reference_list(entries, "implements", CHANGE_LOCATION)?,
        })
    }
}

/// The ADP type with this name, at version 1.
///
/// Panics only on a malformed literal in this file, which the type-name test catches.
fn adp_type(name: &str) -> EntityType {
    EntityType::new("adp", name, 1).expect("the ADP type names in this crate are well formed")
}

/// The entries of a body, or a shape error naming the type that expected them.
fn mapping<'a>(node: &'a Node, type_name: &str) -> Result<&'a BTreeMap<String, Node>, ParseError> {
    node.as_map()
        .ok_or_else(|| ParseError::shape(type_name, "a mapping", node.type_name()))
}

/// A list field's items.
///
/// Absent and null both read as empty, which is what makes an empty list round-trip; anything
/// else that is not a sequence is an error rather than a silent empty list.
fn sequence<'a>(
    entries: &'a BTreeMap<String, Node>,
    field: &str,
    type_name: &str,
) -> Result<&'a [Node], ParseError> {
    match entries.get(field) {
        None | Some(Node::Null) => Ok(&[]),
        Some(Node::Seq(items)) => Ok(items),
        Some(other) => Err(ParseError::shape(
            format!("{type_name}.{field}"),
            "a sequence",
            other.type_name(),
        )),
    }
}

/// A list of strings.
fn text_list(
    entries: &BTreeMap<String, Node>,
    field: &str,
    type_name: &str,
) -> Result<Vec<String>, ParseError> {
    sequence(entries, field, type_name)?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.as_text().map(ToOwned::to_owned).ok_or_else(|| {
                ParseError::shape(
                    format!("{type_name}.{field}[{index}]"),
                    "a string",
                    item.type_name(),
                )
            })
        })
        .collect()
}

/// A required entity reference.
fn reference(
    entries: &BTreeMap<String, Node>,
    field: &str,
    type_name: &str,
) -> Result<EntityRef, ParseError> {
    let location = format!("{type_name}.{field}");
    let text = entries
        .get(field)
        .and_then(Node::as_text)
        .ok_or_else(|| ParseError::shape(location, "an entity id", "nothing"))?;
    Ok(EntityRef::new(EntityId::new(text)?))
}

/// A list of entity references.
fn reference_list(
    entries: &BTreeMap<String, Node>,
    field: &str,
    type_name: &str,
) -> Result<Vec<EntityRef>, ParseError> {
    sequence(entries, field, type_name)?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let text = item.as_text().ok_or_else(|| {
                ParseError::shape(
                    format!("{type_name}.{field}[{index}]"),
                    "an entity id",
                    item.type_name(),
                )
            })?;
            Ok(EntityRef::new(EntityId::new(text)?))
        })
        .collect()
}

/// A required artifact status.
fn status(entries: &BTreeMap<String, Node>, type_name: &str) -> Result<ArtifactStatus, ParseError> {
    let location = format!("{type_name}.status");
    let Some(text) = entries.get("status").and_then(Node::as_text) else {
        return Err(ParseError::shape(location, "a status", "nothing"));
    };
    ArtifactStatus::ALL
        .iter()
        .copied()
        .find(|status| status.as_str() == text)
        .ok_or_else(|| {
            ParseError::shape(
                location,
                "an artifact status, such as `draft` or `approved`",
                format!("{text:?}"),
            )
        })
}

/// A required non-negative whole count.
fn count(
    entries: &BTreeMap<String, Node>,
    field: &str,
    type_name: &str,
) -> Result<usize, ParseError> {
    let location = format!("{type_name}.{field}");
    let value = match entries.get(field) {
        Some(Node::Number(number)) => number.get(),
        Some(other) => {
            return Err(ParseError::shape(location, "a count", other.type_name()));
        }
        None => return Err(ParseError::shape(location, "a count", "nothing")),
    };
    if value < 0.0 || value.fract() != 0.0 || value > MAX_EXACT_COUNT {
        return Err(ParseError::shape(
            location,
            "a whole, non-negative count",
            format!("{value}"),
        ));
    }
    // Guarded immediately above: non-negative, integral, and inside the range `f64` represents
    // exactly, so the conversion cannot lose or invent a file.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let counted = value as usize;
    Ok(counted)
}

/// A status, as written in documents.
fn status_node(status: ArtifactStatus) -> Node {
    Node::from(status.as_str())
}

/// A reference, as written in documents: the bare identifier.
fn reference_node(reference: &EntityRef) -> Node {
    Node::from(reference.id.as_str())
}

/// A list of strings.
fn text_node(items: &[String]) -> Node {
    Node::Seq(items.iter().map(|item| Node::from(item.as_str())).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A specification identifier of the shape the entity model insists on: opaque, not a key.
    const SPEC_ID: &str = "01K2R8JD3ZJME72AJGQY67E5F8";
    /// A story identifier.
    const STORY_ID: &str = "01K2R8JD3ZJME72AJGQY67E5G9";
    /// A design identifier.
    const DESIGN_ID: &str = "01K2R8JD3ZJME72AJGQY67E5H0";

    fn reference_to(id: &str) -> EntityRef {
        EntityRef::new(EntityId::new(id).expect("test entity ids are well formed"))
    }

    fn specification() -> Specification {
        Specification {
            title: "Passkey authentication".to_owned(),
            summary: "Users authenticate with a device-bound key instead of a password.".to_owned(),
            requirements: vec![
                Requirement::new(
                    "R-1",
                    "A registration ceremony binds one key to one account",
                )
                .verified_by("verification.postcondition.passed"),
                Requirement::new("R-2", "A lost device can be de-registered by its owner"),
            ],
            status: ArtifactStatus::Approved,
        }
    }

    fn test_plan() -> TestPlan {
        TestPlan {
            title: "Passkey authentication test plan".to_owned(),
            subject: reference_to(SPEC_ID),
            suites: vec![TestSuite::Unit, TestSuite::Contract, TestSuite::Property],
            claims: vec!["postcondition".to_owned(), "invariant".to_owned()],
            exit_criteria: vec![
                "every requirement names a passing check".to_owned(),
                "no contract test fails".to_owned(),
            ],
            status: ArtifactStatus::Active,
        }
    }

    fn acceptance_criteria() -> AcceptanceCriteria {
        AcceptanceCriteria {
            story: reference_to(STORY_ID),
            criteria: vec![
                "a user with a registered key signs in without a password".to_owned(),
                "a user without one is offered the old path".to_owned(),
            ],
            status: ArtifactStatus::Approved,
        }
    }

    fn change_set() -> ChangeSet {
        ChangeSet {
            revision: Some(Revision::new("9f1c0b3")),
            files_changed: 12,
            summary: "Add the registration ceremony and its contract tests".to_owned(),
            implements: vec![reference_to(SPEC_ID), reference_to(DESIGN_ID)],
        }
    }

    #[test]
    fn each_body_declares_the_versioned_type_the_protocol_publishes() {
        assert_eq!(Specification::entity_type().to_string(), SPECIFICATION_TYPE);
        assert_eq!(TestPlan::entity_type().to_string(), TEST_PLAN_TYPE);
        assert_eq!(
            AcceptanceCriteria::entity_type().to_string(),
            ACCEPTANCE_CRITERIA_TYPE
        );
        assert_eq!(ChangeSet::entity_type().to_string(), CHANGE_TYPE);
    }

    #[test]
    fn a_specification_survives_the_round_trip_with_every_requirement() {
        let original = specification();
        let read = Specification::from_node(&original.to_node()).expect("a written body reads");
        assert_eq!(read, original);
        assert_eq!(read.requirements.len(), 2);
    }

    #[test]
    fn a_requirement_keeps_the_verifier_that_decides_it() {
        let read = Specification::from_node(&specification().to_node()).expect("reads");
        assert_eq!(
            read.requirement("R-1").expect("R-1 is stated").verified_by,
            Some("verification.postcondition.passed".to_owned()),
            "dropping `verified_by` would leave a requirement nothing can check"
        );
        assert_eq!(
            read.requirement("R-2").expect("R-2 is stated").verified_by,
            None,
            "a requirement with no verifier must not acquire one in transit"
        );
    }

    #[test]
    fn a_specification_that_does_not_say_where_it_stands_is_refused() {
        let mut entries = specification()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.remove("status");
        let error = Specification::from_node(&Node::Map(entries))
            .expect_err("a specification without a status is not readable");
        let message = error.to_string();
        assert!(message.contains("adp.specification.status"), "{message}");
        assert!(message.contains("nothing"), "{message}");
    }

    #[test]
    fn a_status_outside_the_vocabulary_is_refused_rather_than_guessed() {
        let mut entries = specification()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.insert("status".to_owned(), Node::from("nearly-approved"));
        let error = Specification::from_node(&Node::Map(entries))
            .expect_err("an invented status is not one");
        assert!(
            error.to_string().contains("nearly-approved"),
            "the rejection should quote what it found: {error}"
        );
    }

    #[test]
    fn a_field_this_build_does_not_know_is_ignored_rather_than_fatal() {
        let mut entries = specification()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.insert("risk_appetite".to_owned(), Node::from("low"));
        let read = Specification::from_node(&Node::Map(entries))
            .expect("a newer writer's field must not break an older reader");
        assert_eq!(read, specification());
    }

    #[test]
    fn a_requirement_list_that_is_not_a_list_is_refused_not_dropped() {
        let mut entries = specification()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.insert("requirements".to_owned(), Node::from("R-1, R-2"));
        let error = Specification::from_node(&Node::Map(entries))
            .expect_err("prose where a list belongs is a shape error");
        let message = error.to_string();
        assert!(
            message.contains("adp.specification.requirements"),
            "{message}"
        );
        assert!(message.contains("a sequence"), "{message}");
    }

    #[test]
    fn a_test_plan_survives_the_round_trip_with_its_suites_claims_and_criteria() {
        let original = test_plan();
        let read = TestPlan::from_node(&original.to_node()).expect("a written body reads");
        assert_eq!(read, original);
        assert_eq!(read.suites.len(), 3);
        assert_eq!(read.claims, vec!["postcondition", "invariant"]);
        assert_eq!(read.exit_criteria.len(), 2);
    }

    #[test]
    fn a_suite_the_vocabulary_does_not_name_still_round_trips() {
        let mut original = test_plan();
        original
            .suites
            .push(TestSuite::Named("golden-file".to_owned()));
        let read = TestPlan::from_node(&original.to_node()).expect("reads");
        assert_eq!(read.suites, original.suites);
    }

    #[test]
    fn a_test_plan_that_names_no_subject_is_refused() {
        let mut entries = test_plan()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.remove("subject");
        let error = TestPlan::from_node(&Node::Map(entries))
            .expect_err("a plan that does not say what it tests is not evidence for anything");
        assert!(
            error.to_string().contains("adp.test-plan.subject"),
            "{error}"
        );
    }

    #[test]
    fn a_subject_that_is_a_tracker_key_is_refused_as_identity() {
        let mut entries = test_plan()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.insert("subject".to_owned(), Node::from("AUTH-142"));
        let error = TestPlan::from_node(&Node::Map(entries)).expect_err("a key is not identity");
        assert!(
            error
                .to_string()
                .contains("keys are not canonical identity"),
            "{error}"
        );
    }

    #[test]
    fn acceptance_criteria_survive_the_round_trip_in_the_order_written() {
        let original = acceptance_criteria();
        let read = AcceptanceCriteria::from_node(&original.to_node()).expect("reads");
        assert_eq!(read, original);
        assert_eq!(read.criteria, original.criteria, "order is meaning here");
    }

    #[test]
    fn an_empty_criteria_list_round_trips_as_empty() {
        let original = AcceptanceCriteria::new(reference_to(STORY_ID));
        let read = AcceptanceCriteria::from_node(&original.to_node()).expect("reads");
        assert_eq!(read, original);
        assert!(read.criteria.is_empty());
    }

    #[test]
    fn a_change_set_survives_the_round_trip_with_its_revision_and_targets() {
        let original = change_set();
        let read = ChangeSet::from_node(&original.to_node()).expect("reads");
        assert_eq!(read, original);
        assert_eq!(
            read.implements.len(),
            2,
            "a change must keep what it was for"
        );
    }

    #[test]
    fn a_change_set_with_no_revision_yet_round_trips_without_acquiring_one() {
        let original = ChangeSet::new("Rename the ceremony module", 3);
        let read = ChangeSet::from_node(&original.to_node()).expect("reads");
        assert_eq!(read.revision, None);
        assert_eq!(read, original);
    }

    #[test]
    fn a_file_count_that_is_not_a_whole_number_is_refused() {
        let mut entries = change_set()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.insert(
            "files_changed".to_owned(),
            Node::Number(Number::new(2.5).expect("2.5 is a number")),
        );
        let error =
            ChangeSet::from_node(&Node::Map(entries)).expect_err("half a file did not change");
        let message = error.to_string();
        assert!(message.contains("adp.change.files_changed"), "{message}");
        assert!(message.contains("whole, non-negative"), "{message}");
    }

    #[test]
    fn a_negative_file_count_is_refused() {
        let mut entries = change_set()
            .to_node()
            .as_map()
            .expect("a body is a mapping")
            .clone();
        entries.insert(
            "files_changed".to_owned(),
            Node::Number(Number::new(-1.0).expect("-1 is a number")),
        );
        let error = ChangeSet::from_node(&Node::Map(entries))
            .expect_err("a count below zero is not a count");
        assert!(error.to_string().contains("non-negative"), "{error}");
    }

    #[test]
    fn a_body_that_is_not_a_mapping_names_the_type_that_expected_one() {
        let error = ChangeSet::from_node(&Node::from("a commit message"))
            .expect_err("a string is not a change");
        let message = error.to_string();
        assert!(message.contains(CHANGE_LOCATION), "{message}");
        assert!(message.contains("a mapping"), "{message}");
    }
}
