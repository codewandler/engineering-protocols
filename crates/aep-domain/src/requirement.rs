//! Requirements: what must be true before something happens.
//!
//! A predicate answers a question about facts. A requirement is the richer thing a principle
//! or a workflow state actually wants to say:
//!
//! ```yaml
//! requires:
//!   predicates:
//!     - tests.unit.failed == 0
//!   evidence:
//!     - kind: test_result
//!       independent: true          # an agent asserting it does not count
//!   artifacts:
//!     - kind: design
//!       status: approved
//!       fresh: true                # approved against the version that exists now
//!   reviews:
//!     - subject_kind: design
//!       result: approved
//!       human: true
//!   approvals:
//!     - security-review
//!   conditional:
//!     - when: {change.architectural: true}
//!       require:
//!         artifacts:
//!           - kind: architecture-design
//!             status: approved
//!           - kind: architecture-decision-record
//! ```
//!
//! Requirements evaluate against a [`RequirementContext`] — facts, the artifact graph and the
//! evidence log — and report per-item outcomes, which is what turns "task incomplete" into a
//! list a person or an agent can act on.
//!
//! # Unknown versus false
//!
//! An unmet requirement is normally [`Truth::Unknown`]: the design is not approved *yet*. It
//! becomes [`Truth::False`] only when something observed contradicts it — a rejected review, a
//! denied approval, a superseded artifact — because that distinction decides whether the right
//! move is to wait, or to go and change something.

use std::collections::BTreeMap;
use std::fmt;

use crate::artifact::{
    Artifact, ArtifactGraph, ArtifactKind, ArtifactRef, ArtifactStatus, RelationKind,
};
use crate::error::ParseError;
use crate::evidence::{ApprovalDecision, Evidence, EvidenceKind, EvidenceRecord};
use crate::facts::FactSource;
use crate::ids::{ApprovalId, SubjectRef};
use crate::node::Node;
use crate::predicate::{Predicate, Truth};
use crate::review::{ReviewDisposition, ReviewResult};
use crate::verification::Verifier;

/// What the engine needs in order to decide whether requirements are met.
pub trait RequirementContext {
    /// The facts observed so far.
    fn facts(&self) -> &dyn FactSource;

    /// The artifact graph.
    fn artifacts(&self) -> &ArtifactGraph;

    /// Every piece of evidence submitted so far, in submission order.
    fn evidence(&self) -> &[EvidenceRecord];
}

/// Which flavour of requirement an outcome came from.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RequirementFlavour {
    /// A predicate over facts.
    Predicate,
    /// Evidence of a given kind.
    Evidence,
    /// An artifact in the graph.
    Artifact,
    /// A review outcome.
    Review,
    /// A human approval.
    Approval,
    /// A requirement that only applies under a condition.
    Conditional,
}

/// The result of checking one requirement.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct RequirementOutcome {
    /// Which flavour of requirement this was.
    pub flavour: RequirementFlavour,
    /// The requirement, in one line.
    pub requirement: String,
    /// Whether it is met.
    pub truth: Truth,
    /// What was observed, or what is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl RequirementOutcome {
    fn new(flavour: RequirementFlavour, requirement: String, truth: Truth) -> Self {
        Self {
            flavour,
            requirement,
            truth,
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// `true` when this requirement is met.
    pub fn is_satisfied(&self) -> bool {
        self.truth.is_satisfied()
    }
}

impl fmt::Display for RequirementOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mark = match self.truth {
            Truth::True => '\u{2713}',
            Truth::False => '\u{2717}',
            Truth::Unknown => '?',
        };
        write!(f, "{mark} {}", self.requirement)?;
        if let Some(detail) = &self.detail {
            write!(f, " — {detail}")?;
        }
        Ok(())
    }
}

/// The result of checking a whole requirement set.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct RequirementReport {
    /// The conjunction of every item.
    pub truth: Truth,
    /// One entry per requirement checked.
    pub items: Vec<RequirementOutcome>,
}

impl RequirementReport {
    /// A report for an empty requirement set.
    pub fn satisfied() -> Self {
        Self {
            truth: Truth::True,
            items: Vec::new(),
        }
    }

    /// `true` when every requirement is met.
    pub fn is_satisfied(&self) -> bool {
        self.truth.is_satisfied()
    }

    /// The requirements that are not met.
    pub fn unmet(&self) -> impl Iterator<Item = &RequirementOutcome> {
        self.items.iter().filter(|item| !item.is_satisfied())
    }

    /// Absorbs another report.
    pub fn extend(&mut self, other: Self) {
        self.truth = self.truth.and(other.truth);
        self.items.extend(other.items);
    }
}

/// Evidence that must have been produced.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct EvidenceRequirement {
    /// Which kind of evidence.
    pub kind: EvidenceKind,
    /// How many records are needed.
    pub at_least: usize,
    /// What the evidence must be about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectRef>,
    /// Which verifier must have produced it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verifier: Option<Verifier>,
    /// Whether the producer must be something other than an agent.
    ///
    /// This is what "generation and verification are separate" reduces to mechanically: an
    /// agent's own claim that the tests passed does not satisfy the requirement.
    pub independent: bool,
}

impl EvidenceRequirement {
    /// A requirement for at least one record of `kind`.
    pub fn of_kind(kind: EvidenceKind) -> Self {
        Self {
            kind,
            at_least: 1,
            subject: None,
            verifier: None,
            independent: false,
        }
    }

    /// Parses the document form: a bare kind name, or a mapping.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(kind) => Ok(Self::of_kind(EvidenceKind::parse(kind)?)),
            Node::Map(entries) => {
                let kind = entries
                    .get("kind")
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        ParseError::shape("requires.evidence[]", "a `kind` field", "no `kind`")
                    })
                    .and_then(EvidenceKind::parse)?;
                Ok(Self {
                    kind,
                    at_least: number_field(entries, "at_least")?.unwrap_or(1),
                    subject: match entries.get("subject").and_then(Node::as_text) {
                        Some(subject) => Some(SubjectRef::new(subject)?),
                        None => None,
                    },
                    verifier: match entries.get("verifier").and_then(Node::as_text) {
                        Some(verifier) => Some(Verifier::parse(verifier)?),
                        None => None,
                    },
                    independent: bool_field(entries, "independent")?.unwrap_or(false),
                })
            }
            other => Err(ParseError::shape(
                "requires.evidence[]",
                "an evidence kind or a mapping",
                other.type_name(),
            )),
        }
    }

    /// `true` when `record` counts towards this requirement.
    pub fn matches(&self, record: &EvidenceRecord) -> bool {
        if record.kind() != self.kind {
            return false;
        }
        if let Some(subject) = &self.subject {
            if record.subject.as_ref() != Some(subject) {
                return false;
            }
        }
        if self.independent && record.producer.is_agent() {
            return false;
        }
        if let Some(verifier) = &self.verifier {
            let by_verifier = matches!(
                &record.producer,
                crate::evidence::Producer::Verifier { verifier: actual } if actual == verifier
            );
            let by_tool = match (&verifier, &record.provenance.tool) {
                (Verifier::ExternalTool(expected), Some(actual)) => expected == actual,
                _ => false,
            };
            if !by_verifier && !by_tool {
                return false;
            }
        }
        true
    }

    /// Checks this requirement.
    fn evaluate(&self, context: &dyn RequirementContext) -> RequirementOutcome {
        let matching = context
            .evidence()
            .iter()
            .filter(|record| self.matches(record))
            .count();
        let truth = Truth::from_bool(matching >= self.at_least);
        let outcome = RequirementOutcome::new(
            RequirementFlavour::Evidence,
            self.to_string(),
            if truth.is_satisfied() {
                Truth::True
            } else {
                Truth::Unknown
            },
        );
        if truth.is_satisfied() {
            outcome
        } else {
            outcome.with_detail(format!(
                "{matching} of {} required record(s) submitted",
                self.at_least
            ))
        }
    }
}

impl fmt::Display for EvidenceRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evidence {}", self.kind)?;
        if self.at_least != 1 {
            write!(f, " x{}", self.at_least)?;
        }
        if let Some(subject) = &self.subject {
            write!(f, " for {subject}")?;
        }
        if let Some(verifier) = &self.verifier {
            write!(f, " from {verifier}")?;
        }
        if self.independent {
            f.write_str(" (independent)")?;
        }
        Ok(())
    }
}

/// A relationship an artifact must have.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct RelationRequirement {
    /// Which relation.
    pub kind: RelationKind,
    /// What kind of thing it must point at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_kind: Option<ArtifactKind>,
}

impl fmt::Display for RelationRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target_kind {
            Some(kind) => write!(f, "{} a {kind}", self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}

/// An artifact that must exist in the graph.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ArtifactRequirement {
    /// Which kind of artifact; kinds that specialise it also count.
    pub kind: ArtifactKind,
    /// Which status it must have reached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ArtifactStatus>,
    /// How many are needed.
    pub at_least: usize,
    /// A relationship it must have.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<RelationRequirement>,
    /// Whether it must not be retired.
    pub fresh: bool,
}

impl ArtifactRequirement {
    /// A requirement for at least one artifact of `kind`.
    pub fn of_kind(kind: ArtifactKind) -> Self {
        Self {
            kind,
            status: None,
            at_least: 1,
            relation: None,
            fresh: true,
        }
    }

    /// Parses the document form: a bare kind name, or a mapping.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(kind) => Ok(Self::of_kind(ArtifactKind::parse(kind)?)),
            Node::Map(entries) => {
                let kind = entries
                    .get("kind")
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        ParseError::shape("requires.artifacts[]", "a `kind` field", "no `kind`")
                    })
                    .and_then(ArtifactKind::parse)?;
                let status = match entries.get("status").and_then(Node::as_text) {
                    Some(status) => Some(parse_status(status)?),
                    None => None,
                };
                let relation = match entries.get("relation") {
                    Some(Node::Text(relation)) => Some(RelationRequirement {
                        kind: RelationKind::parse(relation)?,
                        target_kind: None,
                    }),
                    Some(Node::Map(relation)) => {
                        let kind = relation
                            .get("kind")
                            .and_then(Node::as_text)
                            .ok_or_else(|| {
                                ParseError::shape(
                                    "requires.artifacts[].relation",
                                    "a `kind` field",
                                    "no `kind`",
                                )
                            })
                            .and_then(RelationKind::parse)?;
                        let target_kind = match relation.get("target_kind").and_then(Node::as_text)
                        {
                            Some(target) => Some(ArtifactKind::parse(target)?),
                            None => None,
                        };
                        Some(RelationRequirement { kind, target_kind })
                    }
                    Some(other) => {
                        return Err(ParseError::shape(
                            "requires.artifacts[].relation",
                            "a relation name or a mapping",
                            other.type_name(),
                        ))
                    }
                    None => None,
                };
                Ok(Self {
                    kind,
                    status,
                    at_least: number_field(entries, "at_least")?.unwrap_or(1),
                    relation,
                    fresh: bool_field(entries, "fresh")?.unwrap_or(true),
                })
            }
            other => Err(ParseError::shape(
                "requires.artifacts[]",
                "an artifact kind or a mapping",
                other.type_name(),
            )),
        }
    }

    /// `true` when `artifact` counts towards this requirement.
    pub fn matches(&self, artifact: &Artifact, graph: &ArtifactGraph) -> bool {
        if !artifact.is_kind(&self.kind) {
            return false;
        }
        if self.fresh && artifact.status.is_retired() {
            return false;
        }
        if let Some(status) = self.status {
            if !artifact.status.satisfies(status) {
                return false;
            }
        }
        if let Some(relation) = &self.relation {
            let mut targets = artifact.targets(relation.kind).peekable();
            if targets.peek().is_none() {
                return false;
            }
            if let Some(target_kind) = &relation.target_kind {
                let has_target = artifact.targets(relation.kind).any(|reference| {
                    graph
                        .resolve(reference)
                        .is_some_and(|target| target.is_kind(target_kind))
                });
                if !has_target {
                    return false;
                }
            }
        }
        true
    }

    /// Checks this requirement.
    fn evaluate(&self, context: &dyn RequirementContext) -> RequirementOutcome {
        let graph = context.artifacts();
        let matching = graph
            .artifacts()
            .filter(|artifact| self.matches(artifact, graph))
            .count();
        if matching >= self.at_least {
            return RequirementOutcome::new(
                RequirementFlavour::Artifact,
                self.to_string(),
                Truth::True,
            );
        }

        // A retired artifact of the right kind is a contradiction rather than an absence:
        // waiting will not produce an approved design out of a rejected one.
        let retired: Vec<&Artifact> = graph
            .of_kind(&self.kind)
            .filter(|artifact| artifact.status.is_retired())
            .collect();
        let present: Vec<String> = graph
            .of_kind(&self.kind)
            .map(|artifact| format!("{} ({})", artifact.id, artifact.status))
            .collect();

        let truth = if !retired.is_empty() && present.len() == retired.len() {
            Truth::False
        } else {
            Truth::Unknown
        };

        let detail = if present.is_empty() {
            format!("no {} artifact is declared", self.kind)
        } else {
            format!("declared: {}", present.join(", "))
        };
        RequirementOutcome::new(RequirementFlavour::Artifact, self.to_string(), truth)
            .with_detail(detail)
    }
}

impl fmt::Display for ArtifactRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "artifact {}", self.kind)?;
        if let Some(status) = self.status {
            write!(f, " ({status})")?;
        }
        if self.at_least != 1 {
            write!(f, " x{}", self.at_least)?;
        }
        if let Some(relation) = &self.relation {
            write!(f, " which {relation}")?;
        }
        Ok(())
    }
}

/// A review that must have happened.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ReviewRequirement {
    /// What kind of thing must have been reviewed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<ArtifactKind>,
    /// The specific artifact that must have been reviewed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<ArtifactRef>,
    /// What the review must have concluded.
    pub result: ReviewDisposition,
    /// Whether a person must have done it.
    pub human: bool,
    /// Whether the review must apply to the artifact's current version.
    pub fresh: bool,
}

impl ReviewRequirement {
    /// A requirement for an approving review of `subject_kind`.
    pub fn approved(subject_kind: ArtifactKind) -> Self {
        Self {
            subject_kind: Some(subject_kind),
            subject: None,
            result: ReviewDisposition::Approved,
            human: false,
            fresh: true,
        }
    }

    /// Parses the document form.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(kind) => Ok(Self::approved(ArtifactKind::parse(kind)?)),
            Node::Map(entries) => {
                let subject_kind = match entries
                    .get("subject_kind")
                    .or_else(|| entries.get("kind"))
                    .and_then(Node::as_text)
                {
                    Some(kind) => Some(ArtifactKind::parse(kind)?),
                    None => None,
                };
                let subject = match entries.get("subject").and_then(Node::as_text) {
                    Some(subject) => Some(ArtifactRef::parse(subject)?),
                    None => None,
                };
                if subject_kind.is_none() && subject.is_none() {
                    return Err(ParseError::shape(
                        "requires.reviews[]",
                        "`subject_kind` or `subject`",
                        "neither",
                    ));
                }
                let result = match entries.get("result").and_then(Node::as_text) {
                    Some("approved") | None => ReviewDisposition::Approved,
                    Some("changes_requested") => ReviewDisposition::ChangesRequested,
                    Some("rejected") => ReviewDisposition::Rejected,
                    Some(other) => {
                        return Err(ParseError::shape(
                            "requires.reviews[].result",
                            "approved, changes_requested or rejected",
                            other.to_owned(),
                        ))
                    }
                };
                Ok(Self {
                    subject_kind,
                    subject,
                    result,
                    human: bool_field(entries, "human")?.unwrap_or(false),
                    fresh: bool_field(entries, "fresh")?.unwrap_or(true),
                })
            }
            other => Err(ParseError::shape(
                "requires.reviews[]",
                "an artifact kind or a mapping",
                other.type_name(),
            )),
        }
    }

    /// `true` when `review` is about what this requirement is about.
    fn is_about(&self, review: &ReviewResult, graph: &ArtifactGraph) -> bool {
        if let Some(subject) = &self.subject {
            if review.subject.id() != subject.id() {
                return false;
            }
        }
        if let Some(kind) = &self.subject_kind {
            let declared = review
                .subject_kind
                .as_ref()
                .is_some_and(|actual| actual.is_a(kind));
            let from_graph = graph
                .resolve(&review.subject)
                .is_some_and(|artifact| artifact.is_kind(kind));
            if !declared && !from_graph {
                return false;
            }
        }
        true
    }

    /// Checks this requirement.
    fn evaluate(&self, context: &dyn RequirementContext) -> RequirementOutcome {
        let graph = context.artifacts();
        let mut best: Option<(&ReviewResult, bool)> = None;

        for record in context.evidence() {
            let Evidence::Review(review) = &record.value else {
                continue;
            };
            if !self.is_about(review, graph) {
                continue;
            }
            if self.human && !review.reviewer.is_human() {
                continue;
            }
            let stale = self.fresh
                && graph
                    .resolve(&review.subject)
                    .is_some_and(|artifact| !review.covers(artifact));
            let matches = review.disposition == self.result
                && !stale
                && (self.result != ReviewDisposition::Approved || review.is_clean_approval());
            if matches {
                return RequirementOutcome::new(
                    RequirementFlavour::Review,
                    self.to_string(),
                    Truth::True,
                );
            }
            if best.is_none() || stale {
                best = Some((review, stale));
            }
        }

        match best {
            Some((review, true)) => {
                RequirementOutcome::new(RequirementFlavour::Review, self.to_string(), Truth::False)
                    .with_detail(format!(
                        "the {} review of {} was given against a different version",
                        review.disposition, review.subject
                    ))
            }
            Some((review, false)) => {
                RequirementOutcome::new(RequirementFlavour::Review, self.to_string(), Truth::False)
                    .with_detail(format!(
                        "the review of {} concluded {}{}",
                        review.subject,
                        review.disposition,
                        if review.blocking_findings().next().is_some() {
                            format!(
                                " with {} blocking finding(s)",
                                review.blocking_findings().count()
                            )
                        } else {
                            String::new()
                        }
                    ))
            }
            None => RequirementOutcome::new(
                RequirementFlavour::Review,
                self.to_string(),
                Truth::Unknown,
            )
            .with_detail("no such review has been recorded".to_owned()),
        }
    }
}

impl fmt::Display for ReviewRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("review")?;
        if let Some(subject) = &self.subject {
            write!(f, " of {subject}")?;
        } else if let Some(kind) = &self.subject_kind {
            write!(f, " of a {kind}")?;
        }
        write!(f, " is {}", self.result)?;
        if self.human {
            f.write_str(" (by a person)")?;
        }
        Ok(())
    }
}

/// An approval that must have been granted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ApprovalRequirement {
    /// Which approval.
    pub approval: ApprovalId,
    /// Whether a person must have granted it.
    pub human: bool,
}

impl ApprovalRequirement {
    /// A requirement for `approval`, granted by anyone.
    pub fn new(approval: ApprovalId) -> Self {
        Self {
            approval,
            human: false,
        }
    }

    /// Parses the document form: a bare approval id, or a mapping.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(id) => Ok(Self::new(ApprovalId::new(id.as_str())?)),
            Node::Map(entries) => {
                let approval = entries
                    .get("approval")
                    .or_else(|| entries.get("id"))
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        ParseError::shape(
                            "requires.approvals[]",
                            "an `approval` field",
                            "no `approval`",
                        )
                    })
                    .and_then(ApprovalId::new)?;
                Ok(Self {
                    approval,
                    human: bool_field(entries, "human")?.unwrap_or(false),
                })
            }
            other => Err(ParseError::shape(
                "requires.approvals[]",
                "an approval id or a mapping",
                other.type_name(),
            )),
        }
    }

    /// Checks this requirement.
    fn evaluate(&self, context: &dyn RequirementContext) -> RequirementOutcome {
        let mut denied = false;
        for record in context.evidence() {
            let Evidence::Approval(approval) = &record.value else {
                continue;
            };
            if approval.approval != self.approval {
                continue;
            }
            if self.human && !approval.approver.is_human() {
                continue;
            }
            match approval.decision {
                ApprovalDecision::Granted => {
                    return RequirementOutcome::new(
                        RequirementFlavour::Approval,
                        self.to_string(),
                        Truth::True,
                    )
                }
                ApprovalDecision::Denied => denied = true,
            }
        }

        if denied {
            RequirementOutcome::new(RequirementFlavour::Approval, self.to_string(), Truth::False)
                .with_detail("the approval was refused")
        } else {
            RequirementOutcome::new(
                RequirementFlavour::Approval,
                self.to_string(),
                Truth::Unknown,
            )
            .with_detail("no approval has been recorded")
        }
    }
}

impl fmt::Display for ApprovalRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "approval {}", self.approval)?;
        if self.human {
            f.write_str(" (by a person)")?;
        }
        Ok(())
    }
}

/// Requirements that only apply when a condition holds.
///
/// This is where governance stops being a convention: *if* the change is architectural, *then*
/// an architecture design and an ADR are required — checkable, and not dependent on anyone
/// remembering the rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ConditionalRequirement {
    /// When these requirements apply.
    pub when: Predicate,
    /// What is then required.
    pub require: Box<RequirementSet>,
}

impl ConditionalRequirement {
    /// Checks this requirement.
    fn evaluate(&self, context: &dyn RequirementContext) -> Vec<RequirementOutcome> {
        match self.when.evaluate(context.facts()) {
            Truth::False => vec![RequirementOutcome::new(
                RequirementFlavour::Conditional,
                format!("if {} then …", self.when),
                Truth::True,
            )
            .with_detail("does not apply")],
            Truth::Unknown => vec![RequirementOutcome::new(
                RequirementFlavour::Conditional,
                format!("if {} then …", self.when),
                Truth::Unknown,
            )
            .with_detail(format!(
                "cannot tell whether this applies: {} is unobserved",
                self.when
            ))],
            Truth::True => {
                let mut items = vec![RequirementOutcome::new(
                    RequirementFlavour::Conditional,
                    format!("if {} then …", self.when),
                    Truth::True,
                )
                .with_detail("applies")];
                items.extend(self.require.evaluate(context).items);
                items
            }
        }
    }
}

/// Everything that must hold at one point in a workflow.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct RequirementSet {
    /// Conditions over facts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predicates: Vec<Predicate>,
    /// Evidence that must exist.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRequirement>,
    /// Artifacts that must exist.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactRequirement>,
    /// Reviews that must have happened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviews: Vec<ReviewRequirement>,
    /// Approvals that must have been granted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<ApprovalRequirement>,
    /// Requirements that apply only under a condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditional: Vec<ConditionalRequirement>,
}

/// Keys that introduce a structured requirement rather than a fact predicate.
const STRUCTURED_KEYS: &[&str] = &[
    "predicates",
    "evidence",
    "artifacts",
    "reviews",
    "approvals",
    "approval",
    "conditional",
    "conditional_requirements",
];

impl RequirementSet {
    /// An empty set, which is satisfied by anything.
    pub fn empty() -> Self {
        Self::default()
    }

    /// `true` when nothing is required.
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
            && self.evidence.is_empty()
            && self.artifacts.is_empty()
            && self.reviews.is_empty()
            && self.approvals.is_empty()
            && self.conditional.is_empty()
    }

    /// The number of requirements.
    pub fn len(&self) -> usize {
        self.predicates.len()
            + self.evidence.len()
            + self.artifacts.len()
            + self.reviews.len()
            + self.approvals.len()
            + self.conditional.len()
    }

    /// Absorbs another set.
    pub fn extend(&mut self, other: Self) {
        self.predicates.extend(other.predicates);
        self.evidence.extend(other.evidence);
        self.artifacts.extend(other.artifacts);
        self.reviews.extend(other.reviews);
        self.approvals.extend(other.approvals);
        self.conditional.extend(other.conditional);
    }

    /// Every evidence kind this set requires, including inside conditionals.
    pub fn required_evidence_kinds(&self) -> Vec<EvidenceKind> {
        let mut kinds: Vec<EvidenceKind> = self.evidence.iter().map(|item| item.kind).collect();
        for conditional in &self.conditional {
            kinds.extend(conditional.require.required_evidence_kinds());
        }
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    }

    /// Every predicate in this set, including inside conditionals.
    pub fn all_predicates(&self) -> Vec<&Predicate> {
        let mut predicates: Vec<&Predicate> = self.predicates.iter().collect();
        for conditional in &self.conditional {
            predicates.push(&conditional.when);
            predicates.extend(conditional.require.all_predicates());
        }
        predicates
    }

    /// Checks every requirement.
    pub fn evaluate(&self, context: &dyn RequirementContext) -> RequirementReport {
        let mut items = Vec::new();

        for predicate in &self.predicates {
            let outcome = predicate.outcome(context.facts());
            let detail = if outcome.is_satisfied() {
                None
            } else {
                Some(describe_causes(&outcome))
            };
            let mut item = RequirementOutcome::new(
                RequirementFlavour::Predicate,
                predicate.to_string(),
                outcome.truth,
            );
            item.detail = detail;
            items.push(item);
        }
        for requirement in &self.evidence {
            items.push(requirement.evaluate(context));
        }
        for requirement in &self.artifacts {
            items.push(requirement.evaluate(context));
        }
        for requirement in &self.reviews {
            items.push(requirement.evaluate(context));
        }
        for requirement in &self.approvals {
            items.push(requirement.evaluate(context));
        }
        for requirement in &self.conditional {
            items.extend(requirement.evaluate(context));
        }

        let truth = items
            .iter()
            .fold(Truth::True, |accumulated, item| accumulated.and(item.truth));
        RequirementReport { truth, items }
    }

    /// Parses the document form.
    ///
    /// A string or list is read as predicates. A mapping may use the structured keys
    /// (`predicates`, `evidence`, `artifacts`, `reviews`, `approvals`, `conditional`); any
    /// other key is read as a fact-path predicate, so `{change.architectural: true}` works
    /// wherever a requirement set is expected.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        let mut set = Self::empty();
        match node {
            Node::Null => {}
            Node::Text(_) | Node::Bool(_) => set.predicates.push(Predicate::from_node(node)?),
            Node::Seq(items) => {
                for item in items {
                    set.predicates.push(Predicate::from_node(item)?);
                }
            }
            Node::Map(entries) => {
                let mut predicate_entries: BTreeMap<String, Node> = BTreeMap::new();
                for (key, value) in entries {
                    match key.as_str() {
                        "predicates" => {
                            for item in value.as_seq_or_single() {
                                set.predicates.push(Predicate::from_node(item)?);
                            }
                        }
                        "evidence" => {
                            for item in value.as_seq_or_single() {
                                set.evidence.push(EvidenceRequirement::from_node(item)?);
                            }
                        }
                        // The singular spellings read naturally in a one-item requirement and
                        // are accepted alongside the plural ones.
                        "artifacts" | "artifact" => {
                            for item in value.as_seq_or_single() {
                                set.artifacts.push(ArtifactRequirement::from_node(item)?);
                            }
                        }
                        "reviews" | "review" => {
                            for item in value.as_seq_or_single() {
                                set.reviews.push(ReviewRequirement::from_node(item)?);
                            }
                        }
                        "approvals" | "approval" => {
                            for item in value.as_seq_or_single() {
                                set.approvals.push(ApprovalRequirement::from_node(item)?);
                            }
                        }
                        "conditional" | "conditional_requirements" => {
                            for item in value.as_seq_or_single() {
                                set.conditional.push(parse_conditional(item)?);
                            }
                        }
                        other => {
                            predicate_entries.insert(other.to_owned(), value.clone());
                        }
                    }
                }
                if !predicate_entries.is_empty() {
                    set.predicates
                        .push(Predicate::from_node(&Node::Map(predicate_entries))?);
                }
            }
            Node::Number(number) => {
                return Err(ParseError::shape(
                    "requires",
                    "a predicate, a list or a mapping",
                    format!("the number {number}"),
                ))
            }
        }
        Ok(set)
    }
}

/// Parses one conditional requirement.
fn parse_conditional(node: &Node) -> Result<ConditionalRequirement, ParseError> {
    let Some(entries) = node.as_map() else {
        return Err(ParseError::shape(
            "requires.conditional[]",
            "a mapping with `when` and `require`",
            node.type_name(),
        ));
    };
    let when = entries
        .get("when")
        .ok_or_else(|| ParseError::shape("requires.conditional[]", "a `when` field", "no `when`"))
        .and_then(Predicate::from_node)?;
    let require = entries
        .get("require")
        .or_else(|| entries.get("requires"))
        .ok_or_else(|| {
            ParseError::shape(
                "requires.conditional[]",
                "a `require` field",
                "no `require`",
            )
        })
        .and_then(RequirementSet::from_node)?;
    Ok(ConditionalRequirement {
        when,
        require: Box::new(require),
    })
}

/// Reads an optional non-negative integer field.
fn number_field(entries: &BTreeMap<String, Node>, key: &str) -> Result<Option<usize>, ParseError> {
    match entries.get(key) {
        None | Some(Node::Null) => Ok(None),
        Some(Node::Number(number)) if number.is_integral() && number.get() >= 0.0 => {
            // Guarded above: integral and non-negative.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let count = number.get() as usize;
            Ok(Some(count))
        }
        Some(other) => Err(ParseError::shape(
            key.to_owned(),
            "a non-negative integer",
            other.type_name(),
        )),
    }
}

/// Reads an optional boolean field.
fn bool_field(entries: &BTreeMap<String, Node>, key: &str) -> Result<Option<bool>, ParseError> {
    match entries.get(key) {
        None | Some(Node::Null) => Ok(None),
        Some(Node::Bool(value)) => Ok(Some(*value)),
        Some(other) => Err(ParseError::shape(
            key.to_owned(),
            "a boolean",
            other.type_name(),
        )),
    }
}

/// Parses an artifact status, accepting `in-review` alongside `in_review`.
fn parse_status(value: &str) -> Result<ArtifactStatus, ParseError> {
    let normalised = value.replace('-', "_");
    ArtifactStatus::ALL
        .iter()
        .copied()
        .find(|status| status.as_str() == normalised)
        .ok_or_else(|| {
            ParseError::shape(
                "status",
                format!(
                    "one of {}",
                    ArtifactStatus::ALL
                        .iter()
                        .map(|status| status.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                value.to_owned(),
            )
        })
}

/// Summarises why a predicate is not satisfied.
fn describe_causes(outcome: &crate::predicate::PredicateOutcome) -> String {
    let mut parts = Vec::new();
    for cause in &outcome.causes {
        let observed = cause
            .observed
            .iter()
            .map(|(path, value)| format!("{path} = {value}"))
            .collect::<Vec<_>>();
        if !observed.is_empty() {
            parts.push(observed.join(", "));
        }
        if !cause.missing.is_empty() {
            parts.push(format!(
                "unobserved: {}",
                cause
                    .missing
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(note) = &cause.note {
            parts.push(note.clone());
        }
    }
    if parts.is_empty() {
        outcome.truth.as_str().to_owned()
    } else {
        parts.join("; ")
    }
}

impl<'de> serde::Deserialize<'de> for RequirementSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl<'de> serde::Deserialize<'de> for EvidenceRequirement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactRequirement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl<'de> serde::Deserialize<'de> for ReviewRequirement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl<'de> serde::Deserialize<'de> for ApprovalRequirement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

/// Documents which mapping keys are structured requirements rather than fact predicates.
///
/// Exposed so that tooling can warn about a near-miss key such as `artefacts`.
pub fn structured_requirement_keys() -> &'static [&'static str] {
    STRUCTURED_KEYS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactId, ArtifactLocation};
    use crate::evidence::{Producer, TestResult, TestSuite};
    use crate::facts::{FactStore, FactValue, Scales};
    use crate::ids::EvidenceId;
    use crate::review::{ReviewResult, Reviewer};
    use crate::time::Timestamp;

    struct Context {
        facts: FactStore,
        artifacts: ArtifactGraph,
        evidence: Vec<EvidenceRecord>,
    }

    impl Context {
        fn new() -> Self {
            Self {
                facts: FactStore::new(),
                artifacts: ArtifactGraph::new(),
                evidence: Vec::new(),
            }
        }

        fn with_evidence(mut self, producer: Producer, evidence: Evidence) -> Self {
            let index = self.evidence.len();
            self.facts.extend_facts(evidence.facts());
            self.evidence.push(EvidenceRecord::new(
                EvidenceId::new(format!("e{index}")).expect("id"),
                Timestamp::from_epoch_millis(index as u64),
                producer,
                evidence,
            ));
            self
        }

        fn with_artifact(mut self, artifact: Artifact) -> Self {
            self.artifacts.insert(artifact);
            self.facts.extend(self.artifacts.facts());
            self
        }
    }

    impl RequirementContext for Context {
        fn facts(&self) -> &dyn FactSource {
            &self.facts
        }

        fn artifacts(&self) -> &ArtifactGraph {
            &self.artifacts
        }

        fn evidence(&self) -> &[EvidenceRecord] {
            &self.evidence
        }
    }

    fn design(status: ArtifactStatus) -> Artifact {
        Artifact::new(
            ArtifactId::new("design:passkeys").expect("id"),
            ArtifactKind::Design,
            status,
            ArtifactLocation::Inline,
        )
    }

    fn parse(yaml: &str) -> RequirementSet {
        let node: Node = serde_yaml::from_str(yaml).expect("yaml parses");
        RequirementSet::from_node(&node).expect("requirements parse")
    }

    #[test]
    fn missing_evidence_is_unknown_and_agent_evidence_does_not_count_as_independent() {
        let requirement = parse("evidence:\n  - kind: test_result\n    independent: true");
        let empty = Context::new();
        assert_eq!(requirement.evaluate(&empty).truth, Truth::Unknown);

        let by_agent = Context::new().with_evidence(
            Producer::Agent {
                id: "opus".to_owned(),
            },
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 3)),
        );
        assert_eq!(
            requirement.evaluate(&by_agent).truth,
            Truth::Unknown,
            "an agent asserting its own test run is not independent evidence"
        );

        let by_runner = Context::new().with_evidence(
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 3)),
        );
        assert!(requirement.evaluate(&by_runner).is_satisfied());
    }

    #[test]
    fn an_artifact_requirement_reads_the_kind_hierarchy_and_status_ladder() {
        let requirement = parse("artifacts:\n  - kind: design\n    status: approved");
        assert!(requirement
            .evaluate(&Context::new().with_artifact(design(ArtifactStatus::Approved)))
            .is_satisfied());
        assert!(
            requirement
                .evaluate(&Context::new().with_artifact(design(ArtifactStatus::Implemented)))
                .is_satisfied(),
            "implemented is downstream of approved"
        );
        assert_eq!(
            requirement
                .evaluate(&Context::new().with_artifact(design(ArtifactStatus::Draft)))
                .truth,
            Truth::Unknown
        );
    }

    #[test]
    fn a_rejected_artifact_is_false_rather_than_merely_unobserved() {
        let requirement = parse("artifacts:\n  - kind: design\n    status: approved");
        let report =
            requirement.evaluate(&Context::new().with_artifact(design(ArtifactStatus::Rejected)));
        assert_eq!(
            report.truth,
            Truth::False,
            "waiting will not turn a rejected design into an approved one"
        );
    }

    #[test]
    fn a_stale_approval_does_not_satisfy_a_fresh_review_requirement() {
        let mut artifact = design(ArtifactStatus::Approved);
        artifact.version = Some(crate::artifact::ArtifactVersion::new("7"));

        let review = |version: &str| {
            Evidence::Review(ReviewResult {
                subject: ArtifactRef::parse("design:passkeys").expect("reference"),
                subject_kind: Some(ArtifactKind::Design),
                reviewer: Reviewer::Human {
                    id: "ada".to_owned(),
                },
                disposition: ReviewDisposition::Approved,
                findings: Vec::new(),
                reviewed_version: Some(crate::artifact::ArtifactVersion::new(version)),
                reviewed_revision: None,
            })
        };

        let requirement =
            parse("reviews:\n  - subject_kind: design\n    result: approved\n    human: true");

        let stale = Context::new()
            .with_artifact(artifact.clone())
            .with_evidence(
                Producer::Human {
                    id: "ada".to_owned(),
                },
                review("3"),
            );
        let report = requirement.evaluate(&stale);
        assert_eq!(report.truth, Truth::False);
        assert!(
            report.items[0]
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("different version"),
            "{:?}",
            report.items
        );

        let current = Context::new().with_artifact(artifact).with_evidence(
            Producer::Human {
                id: "ada".to_owned(),
            },
            review("7"),
        );
        assert!(requirement.evaluate(&current).is_satisfied());
    }

    #[test]
    fn conditional_requirements_apply_only_when_the_condition_holds() {
        let requirement = parse(
            "conditional:\n  - when: {change.architectural: true}\n    require:\n      artifacts:\n        - kind: architecture-design\n          status: approved",
        );

        let mut not_architectural = Context::new();
        not_architectural
            .facts
            .set_path("change.architectural", FactValue::bool(false));
        assert!(
            requirement.evaluate(&not_architectural).is_satisfied(),
            "a non-architectural change owes no architecture design"
        );

        let mut architectural = Context::new();
        architectural
            .facts
            .set_path("change.architectural", FactValue::bool(true));
        assert_eq!(requirement.evaluate(&architectural).truth, Truth::Unknown);

        let unknown = Context::new();
        let report = requirement.evaluate(&unknown);
        assert_eq!(
            report.truth,
            Truth::Unknown,
            "if we cannot tell whether the rule applies, we cannot call it met"
        );
    }

    #[test]
    fn an_unrecognised_mapping_key_becomes_a_fact_predicate() {
        let requirement = parse("risk: {gte: medium}");
        assert_eq!(requirement.predicates.len(), 1);

        let mut facts = FactStore::new();
        facts.set_path("risk", FactValue::text("high"));
        let mut scales = Scales::default();
        scales.insert(
            "risk",
            ["low", "medium", "high"].map(ToOwned::to_owned).to_vec(),
        );
        facts.set_scales(scales);

        let mut context = Context::new();
        context.facts = facts;
        assert!(requirement.evaluate(&context).is_satisfied());
    }

    #[test]
    fn a_bare_list_is_read_as_predicates() {
        let requirement = parse("- tests.unit.failed == 0\n- specification.satisfied");
        assert_eq!(requirement.predicates.len(), 2);
        assert!(requirement.evidence.is_empty());
    }

    #[test]
    fn reports_list_every_unmet_requirement_with_a_reason() {
        let requirement = parse(
            "predicates:\n  - tests.unit.failed == 0\nartifacts:\n  - kind: design\n    status: approved\napprovals:\n  - security-review",
        );
        let report = requirement.evaluate(&Context::new());
        assert_eq!(report.items.len(), 3);
        assert_eq!(report.unmet().count(), 3);
        for item in report.unmet() {
            assert!(item.detail.is_some(), "{item}");
        }
        let rendered = report
            .items
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("approval security-review"), "{rendered}");
    }
}
