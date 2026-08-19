//! The entity bodies AOP adds, and the two status ladders they climb.
//!
//! Three types, because three are what the shipped documents need: an [`Incident`] for
//! `profiles/incident-standard.yaml`, a [`Release`] for `profiles/release-progressive.yaml`, and a
//! [`Runbook`], which is the artifact an incident consumes and a postmortem amends.
//!
//! # Bodies are typed here and untyped on the wire
//!
//! The interaction contract moves [`Node`] bodies, because a generic backend cannot know what an
//! incident is (§48). [`EntityBody`] is the bridge: an application holds an `Entity<Incident>`, the
//! contract moves an `Entity<Node>`, and [`EntityBody::to_node`] / [`EntityBody::from_node`] are the
//! only crossing. Every field therefore has to survive the round trip, which is what the tests in
//! this module check.
//!
//! # Why a status ladder refuses shortcuts
//!
//! Each rung of a ladder is where one output of the work gets written down. Permitting a jump would
//! not speed anything up — the entity would reach the same terminal status with the intervening
//! records missing, and the missing records are the entire product of an incident response.

use std::collections::BTreeMap;
use std::fmt;

use aep_domain::capability::Environment;
use aep_domain::entity::{optional_text, required_text, EntityBody, EntityType};
use aep_domain::error::ParseError;
use aep_domain::facts::Number;
use aep_domain::ids::ServiceId;
use aep_domain::node::Node;
use aep_domain::review::Severity;
use aep_domain::time::Timestamp;

/// The namespace every AOP entity type sits in.
pub const NAMESPACE: &str = "aop";

/// Builds one of this crate's entity types.
///
/// The parts are literals that satisfy [`EntityType::new`]'s charset rule, so the failure branch is
/// unreachable. It panics rather than returning a `Result` because a malformed type here is a bug
/// in this file, not a condition a caller could handle: the alternative is an entity type nobody
/// can address, reported at every call site instead of once.
fn entity_type(name: &str) -> EntityType {
    EntityType::new(NAMESPACE, name, 1).unwrap_or_else(|error| {
        panic!("`{NAMESPACE}.{name}/v1` is not a well-formed type: {error}")
    })
}

/// Renders a timestamp for the wire.
///
/// [`Number`] is IEEE-double backed, and epoch milliseconds stay inside the exactly representable
/// integer range until the year 287396, so a document round trip is lossless. Writing the digits as
/// text would dodge the cast at the cost of spelling a timestamp differently here from everywhere
/// else in the protocol, where [`Timestamp`] serialises as a number.
fn timestamp_node(at: Timestamp) -> Node {
    // The wrap this lint warns about needs a timestamp roughly 292 million years out.
    #[allow(clippy::cast_possible_wrap)]
    let millis = at.epoch_millis() as i64;
    Node::Number(Number::from(millis))
}

/// Reads a timestamp back from a wire number.
fn timestamp_from(value: &Node, location: &str) -> Result<Timestamp, ParseError> {
    let Node::Number(number) = value else {
        return Err(ParseError::shape(
            location.to_owned(),
            "epoch milliseconds as a number",
            value.type_name(),
        ));
    };
    if !number.is_integral() || number.get() < 0.0 {
        return Err(ParseError::shape(
            location.to_owned(),
            "whole, non-negative epoch milliseconds",
            number.to_string(),
        ));
    }
    // Guarded directly above: whole, non-negative, and `is_integral` bounds the magnitude below
    // 2^53, so neither the truncation nor the sign loss the lints warn about can occur.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let millis = number.get() as u64;
    Ok(Timestamp::from_epoch_millis(millis))
}

/// Reads a required timestamp field.
fn required_timestamp(node: &Node, field: &str, type_name: &str) -> Result<Timestamp, ParseError> {
    let location = format!("{type_name}.{field}");
    let value = node
        .as_map()
        .and_then(|entries| entries.get(field))
        .ok_or_else(|| {
            ParseError::shape(
                location.clone(),
                "epoch milliseconds as a number",
                "nothing",
            )
        })?;
    timestamp_from(value, &location)
}

/// Reads an optional timestamp field, treating an absent key and an explicit null alike.
fn optional_timestamp(
    node: &Node,
    field: &str,
    type_name: &str,
) -> Result<Option<Timestamp>, ParseError> {
    match node.as_map().and_then(|entries| entries.get(field)) {
        None | Some(Node::Null) => Ok(None),
        Some(value) => timestamp_from(value, &format!("{type_name}.{field}")).map(Some),
    }
}

/// Renders a list of strings for the wire.
fn text_list_node(items: &[String]) -> Node {
    Node::Seq(items.iter().cloned().map(Node::Text).collect())
}

/// Reads a list of strings, accepting a bare scalar as a one-element list.
///
/// The leniency is [`Node::as_seq_or_single`]'s and is deliberate throughout the repository:
/// documents are written by people, who reasonably write `hypotheses: the cache is cold` where the
/// schema says a list. An absent key is an empty list rather than an error, because "no hypothesis
/// yet" is the ordinary state of a freshly detected incident.
fn text_list(node: &Node, field: &str, type_name: &str) -> Result<Vec<String>, ParseError> {
    let Some(value) = node.as_map().and_then(|entries| entries.get(field)) else {
        return Ok(Vec::new());
    };
    value
        .as_seq_or_single()
        .into_iter()
        .map(|item| {
            item.as_text().map(ToOwned::to_owned).ok_or_else(|| {
                ParseError::shape(format!("{type_name}.{field}"), "a string", item.type_name())
            })
        })
        .collect()
}

/// Reads a [`Severity`] back from its document spelling.
fn severity_from(value: &str, location: &str) -> Result<Severity, ParseError> {
    match value {
        "info" => Ok(Severity::Info),
        "low" => Ok(Severity::Low),
        "medium" => Ok(Severity::Medium),
        "high" => Ok(Severity::High),
        "critical" => Ok(Severity::Critical),
        other => Err(ParseError::shape(
            location.to_owned(),
            "one of info, low, medium, high, critical",
            other.to_owned(),
        )),
    }
}

/// Where an incident has got to.
///
/// The first five rungs are the past tense of the first five states of
/// `workflows/incidents/standard.yaml`; `resolved` is where an incident arrives once verification
/// and learning are done. The workflow is the plan an execution is held to and ends when the
/// execution ends; the status is what survives on the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IncidentStatus {
    /// Something is wrong and there is an observation that says so.
    Detected,
    /// Scope and severity are established.
    Triaged,
    /// A hypothesis has been tested and held.
    Diagnosed,
    /// The bleeding has stopped, though service may still be degraded.
    Mitigated,
    /// The service is serving normally again.
    Recovered,
    /// Recovery has been verified and what was learned is written down.
    Resolved,
}

impl IncidentStatus {
    /// The ladder, in order. Also the full vocabulary: nothing sits outside it.
    pub const ALL: &'static [Self] = &[
        Self::Detected,
        Self::Triaged,
        Self::Diagnosed,
        Self::Mitigated,
        Self::Recovered,
        Self::Resolved,
    ];

    /// Where a new incident starts.
    pub const INITIAL: Self = Self::Detected;

    /// The status as written in documents.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detected => "detected",
            Self::Triaged => "triaged",
            Self::Diagnosed => "diagnosed",
            Self::Mitigated => "mitigated",
            Self::Recovered => "recovered",
            Self::Resolved => "resolved",
        }
    }

    /// Parses a status, naming the whole ladder when it fails.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .iter()
            .copied()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| {
                ParseError::reference(
                    "incident status",
                    value,
                    format!(
                        "expected one of {}",
                        Self::ALL
                            .iter()
                            .map(|status| status.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })
    }

    /// `true` when an incident may move from `from` to `to`.
    ///
    /// One rung at a time, forwards only. Refusing `detected -> resolved` is the point of having a
    /// ladder at all: each rung is where one output of the response is recorded — the blast radius
    /// at `triaged`, the tested hypothesis at `diagnosed`, the action taken at `mitigated`, the
    /// health observation at `recovered`. An incident that jumps straight to `resolved` closes with
    /// none of them written down; nothing records what was learned, and the next occurrence of the
    /// same fault starts from nothing. The shortcut does not save time, it discards the only
    /// durable product of the response.
    ///
    /// Going backwards is refused too. A mitigation that did not hold is new information about a
    /// live incident, not a return to an earlier one, and re-running `diagnosed` would overwrite the
    /// record of the first diagnosis with the second.
    pub fn permits(from: Self, to: Self) -> bool {
        Self::ALL
            .iter()
            .position(|status| *status == from)
            .and_then(|index| Self::ALL.get(index + 1))
            .is_some_and(|next| *next == to)
    }

    /// The statuses this one may move to.
    pub fn successors(self) -> Vec<Self> {
        Self::ALL
            .iter()
            .copied()
            .filter(|candidate| Self::permits(self, *candidate))
            .collect()
    }

    /// `true` when the incident is over.
    pub fn is_terminal(self) -> bool {
        self == Self::Resolved
    }
}

impl fmt::Display for IncidentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a release has got to.
///
/// The ladder is the past tense of `workflows/releases/progressive.yaml`, with one status that is
/// not a rung: [`ReleaseStatus::RolledBack`] leaves the ladder sideways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReleaseStatus {
    /// Suites green, contracts intact: a candidate, deployed nowhere.
    Qualified,
    /// Running in staging.
    Staged,
    /// Serving a small, real slice of production traffic.
    Canary,
    /// Measured against its service objectives on real traffic.
    Observed,
    /// Serving the whole of production.
    Promoted,
    /// Confirmed healthy under full traffic, not merely reported deployed.
    Verified,
    /// Undone; the previous revision is serving.
    RolledBack,
}

impl ReleaseStatus {
    /// The forward ladder. [`ReleaseStatus::RolledBack`] is deliberately not on it.
    pub const LADDER: &'static [Self] = &[
        Self::Qualified,
        Self::Staged,
        Self::Canary,
        Self::Observed,
        Self::Promoted,
        Self::Verified,
    ];

    /// Every status, the ladder plus the way off it.
    pub const ALL: &'static [Self] = &[
        Self::Qualified,
        Self::Staged,
        Self::Canary,
        Self::Observed,
        Self::Promoted,
        Self::Verified,
        Self::RolledBack,
    ];

    /// Where a new release starts.
    pub const INITIAL: Self = Self::Qualified;

    /// The status as written in documents.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qualified => "qualified",
            Self::Staged => "staged",
            Self::Canary => "canary",
            Self::Observed => "observed",
            Self::Promoted => "promoted",
            Self::Verified => "verified",
            Self::RolledBack => "rolled_back",
        }
    }

    /// Parses a status, naming the whole vocabulary when it fails.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .iter()
            .copied()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| {
                ParseError::reference(
                    "release status",
                    value,
                    format!(
                        "expected one of {}",
                        Self::ALL
                            .iter()
                            .map(|status| status.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })
    }

    /// `true` when something of this release is running somewhere, so there is a rollback to make.
    ///
    /// `qualified` is not: a build that has passed its gates has been deployed nowhere, and a
    /// rollback with nothing deployed is a no-op wearing the name of a recovery — it would record a
    /// remediation that did not happen. Every later rung is, staging included, because a staging
    /// deployment that has to be undone is still a rollback and still worth in the record.
    pub fn is_deployed(self) -> bool {
        matches!(
            self,
            Self::Staged | Self::Canary | Self::Observed | Self::Promoted | Self::Verified
        )
    }

    /// `true` when a release may move from `from` to `to`.
    ///
    /// One rung forward along the ladder, or sideways to `rolled_back` from anything deployed.
    /// Nothing leaves `rolled_back`: the way forward from a rollback is a new release that
    /// re-qualifies, not a resumed rollout, and a transition out of it would let a revision reach
    /// production again on the strength of gates it passed before the failure.
    pub fn permits(from: Self, to: Self) -> bool {
        if to == Self::RolledBack {
            return from.is_deployed();
        }
        Self::LADDER
            .iter()
            .position(|status| *status == from)
            .and_then(|index| Self::LADDER.get(index + 1))
            .is_some_and(|next| *next == to)
    }

    /// The statuses this one may move to.
    pub fn successors(self) -> Vec<Self> {
        Self::ALL
            .iter()
            .copied()
            .filter(|candidate| Self::permits(self, *candidate))
            .collect()
    }

    /// `true` when the release is over, whichever way it ended.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Verified | Self::RolledBack)
    }
}

impl fmt::Display for ReleaseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A live service impairment and the response to it.
///
/// The three timestamps are separate fields rather than one "last changed" because the intervals
/// between them are the numbers an operations review actually asks for: detection to mitigation is
/// how long customers were affected, mitigation to resolution is how long the follow-up took, and a
/// single mutable timestamp answers neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incident {
    /// What is wrong, in one line.
    pub title: String,
    /// How bad it is, on the protocol's shared severity scale.
    pub severity: Severity,
    /// Which service is impaired.
    pub service: ServiceId,
    /// How far the response has got.
    pub status: IncidentStatus,
    /// When the impairment was first observed.
    pub detected_at: Timestamp,
    /// When the bleeding stopped, once it has.
    pub mitigated_at: Option<Timestamp>,
    /// When the incident was closed, once it has been.
    pub resolved_at: Option<Timestamp>,
    /// What was thought to be the cause, in the order the theories were raised.
    ///
    /// Kept as a list, and kept after the fact: `hypothesis-driven-diagnosis` requires a hypothesis
    /// to be tested before it is acted on, and the theories that were tested and failed are what
    /// stop the next responder from re-testing them.
    pub hypotheses: Vec<String>,
    /// Who and what is affected, as far as it is known.
    pub blast_radius: Option<String>,
}

impl EntityBody for Incident {
    fn entity_type() -> EntityType {
        entity_type("incident")
    }

    fn to_node(&self) -> Node {
        let mut entries = BTreeMap::new();
        entries.insert("title".to_owned(), Node::from(self.title.clone()));
        entries.insert("severity".to_owned(), Node::from(self.severity.as_str()));
        entries.insert("service".to_owned(), Node::from(self.service.as_str()));
        entries.insert("status".to_owned(), Node::from(self.status.as_str()));
        entries.insert("detected_at".to_owned(), timestamp_node(self.detected_at));
        if let Some(at) = self.mitigated_at {
            entries.insert("mitigated_at".to_owned(), timestamp_node(at));
        }
        if let Some(at) = self.resolved_at {
            entries.insert("resolved_at".to_owned(), timestamp_node(at));
        }
        if !self.hypotheses.is_empty() {
            entries.insert("hypotheses".to_owned(), text_list_node(&self.hypotheses));
        }
        if let Some(blast_radius) = &self.blast_radius {
            entries.insert("blast_radius".to_owned(), Node::from(blast_radius.clone()));
        }
        Node::Map(entries)
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "aop.incident";
        Ok(Self {
            title: required_text(node, "title", TYPE_NAME)?,
            severity: severity_from(
                &required_text(node, "severity", TYPE_NAME)?,
                "aop.incident.severity",
            )?,
            service: ServiceId::new(required_text(node, "service", TYPE_NAME)?)?,
            status: IncidentStatus::parse(&required_text(node, "status", TYPE_NAME)?)?,
            detected_at: required_timestamp(node, "detected_at", TYPE_NAME)?,
            mitigated_at: optional_timestamp(node, "mitigated_at", TYPE_NAME)?,
            resolved_at: optional_timestamp(node, "resolved_at", TYPE_NAME)?,
            hypotheses: text_list(node, "hypotheses", TYPE_NAME)?,
            blast_radius: optional_text(node, "blast_radius"),
        })
    }
}

/// Operational instructions for one service, written before they are needed.
///
/// `verification` is a separate list from `steps` on purpose. A runbook whose steps end with "the
/// service should recover" hands the reader no way to tell recovery from the absence of a new alert,
/// which is the `verify-after-action` principle's whole complaint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runbook {
    /// What the runbook is for, in one line.
    pub title: String,
    /// Which service it operates.
    pub service: ServiceId,
    /// The symptom or situation that makes this the right runbook to open.
    pub when_to_use: String,
    /// What to do, in order.
    pub steps: Vec<String>,
    /// How to tell that it worked, as observations rather than expectations.
    pub verification: Vec<String>,
    /// Who to wake when it does not work.
    pub escalation: Option<String>,
}

impl EntityBody for Runbook {
    fn entity_type() -> EntityType {
        entity_type("runbook")
    }

    fn to_node(&self) -> Node {
        let mut entries = BTreeMap::new();
        entries.insert("title".to_owned(), Node::from(self.title.clone()));
        entries.insert("service".to_owned(), Node::from(self.service.as_str()));
        entries.insert(
            "when_to_use".to_owned(),
            Node::from(self.when_to_use.clone()),
        );
        if !self.steps.is_empty() {
            entries.insert("steps".to_owned(), text_list_node(&self.steps));
        }
        if !self.verification.is_empty() {
            entries.insert(
                "verification".to_owned(),
                text_list_node(&self.verification),
            );
        }
        if let Some(escalation) = &self.escalation {
            entries.insert("escalation".to_owned(), Node::from(escalation.clone()));
        }
        Node::Map(entries)
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "aop.runbook";
        Ok(Self {
            title: required_text(node, "title", TYPE_NAME)?,
            service: ServiceId::new(required_text(node, "service", TYPE_NAME)?)?,
            when_to_use: required_text(node, "when_to_use", TYPE_NAME)?,
            steps: text_list(node, "steps", TYPE_NAME)?,
            verification: text_list(node, "verification", TYPE_NAME)?,
            escalation: optional_text(node, "escalation"),
        })
    }
}

/// One revision on its way to an environment.
///
/// `previous_revision` is what makes a rollback a plan rather than an intention. Both shipped
/// rollback routes in `workflows/releases/progressive.yaml` guard on
/// `deployment.previous_revision.exists`, and this is the field that answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// What is being released, in whatever form the source system names a revision.
    pub revision: String,
    /// Where it is going.
    pub environment: Environment,
    /// How it is being rolled out, such as `canary-10` or `blue-green`.
    pub strategy: Option<String>,
    /// How far the rollout has got.
    pub status: ReleaseStatus,
    /// What was serving before, which is what a rollback returns to.
    pub previous_revision: Option<String>,
}

impl EntityBody for Release {
    fn entity_type() -> EntityType {
        entity_type("release")
    }

    fn to_node(&self) -> Node {
        let mut entries = BTreeMap::new();
        entries.insert("revision".to_owned(), Node::from(self.revision.clone()));
        entries.insert(
            "environment".to_owned(),
            Node::from(self.environment.as_str()),
        );
        entries.insert("status".to_owned(), Node::from(self.status.as_str()));
        if let Some(strategy) = &self.strategy {
            entries.insert("strategy".to_owned(), Node::from(strategy.clone()));
        }
        if let Some(previous) = &self.previous_revision {
            entries.insert("previous_revision".to_owned(), Node::from(previous.clone()));
        }
        Node::Map(entries)
    }

    fn from_node(node: &Node) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "aop.release";
        Ok(Self {
            revision: required_text(node, "revision", TYPE_NAME)?,
            environment: Environment::parse(&required_text(node, "environment", TYPE_NAME)?)?,
            strategy: optional_text(node, "strategy"),
            status: ReleaseStatus::parse(&required_text(node, "status", TYPE_NAME)?)?,
            previous_revision: optional_text(node, "previous_revision"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ServiceId {
        ServiceId::new("checkout-api").expect("a well-formed service id")
    }

    fn full_incident() -> Incident {
        Incident {
            title: "Checkout returns 503 for a third of requests".to_owned(),
            severity: Severity::High,
            service: service(),
            status: IncidentStatus::Mitigated,
            detected_at: Timestamp::from_epoch_millis(1_700_000_000_000),
            mitigated_at: Some(Timestamp::from_epoch_millis(1_700_000_900_000)),
            resolved_at: None,
            hypotheses: vec![
                "the payment provider is rate limiting us".to_owned(),
                "the connection pool is exhausted".to_owned(),
            ],
            blast_radius: Some("card checkout in eu-west-1; wallet checkout unaffected".to_owned()),
        }
    }

    fn bare_incident() -> Incident {
        Incident {
            title: "Elevated error rate".to_owned(),
            severity: Severity::Low,
            service: service(),
            status: IncidentStatus::INITIAL,
            detected_at: Timestamp::from_epoch_millis(1_700_000_000_000),
            mitigated_at: None,
            resolved_at: None,
            hypotheses: Vec::new(),
            blast_radius: None,
        }
    }

    fn runbook() -> Runbook {
        Runbook {
            title: "Drain a checkout node".to_owned(),
            service: service(),
            when_to_use: "one node is serving errors while its peers are healthy".to_owned(),
            steps: vec![
                "remove the node from the load balancer".to_owned(),
                "capture a heap dump before restarting anything".to_owned(),
            ],
            verification: vec!["error rate on the remaining nodes is back under 0.1%".to_owned()],
            escalation: Some("payments on-call, then the platform lead".to_owned()),
        }
    }

    fn release() -> Release {
        Release {
            revision: "9f2c1ab".to_owned(),
            environment: Environment::Production,
            strategy: Some("canary-10".to_owned()),
            status: ReleaseStatus::Canary,
            previous_revision: Some("3ad77e0".to_owned()),
        }
    }

    #[test]
    fn the_entity_types_are_the_versioned_names_the_specification_lists() {
        assert_eq!(Incident::entity_type().to_string(), "aop.incident/v1");
        assert_eq!(Runbook::entity_type().to_string(), "aop.runbook/v1");
        assert_eq!(Release::entity_type().to_string(), "aop.release/v1");
    }

    #[test]
    fn an_incident_round_trips_through_a_node_with_every_field_set() {
        let incident = full_incident();
        let node = incident.to_node();
        let read = Incident::from_node(&node).expect("an incident this crate wrote reads back");
        assert_eq!(read, incident, "the round trip lost or changed a field");
    }

    #[test]
    fn an_incident_round_trips_when_every_optional_field_is_absent() {
        let incident = bare_incident();
        let node = incident.to_node();
        assert!(
            node.as_map()
                .is_some_and(|entries| !entries.contains_key("mitigated_at")
                    && !entries.contains_key("blast_radius")
                    && !entries.contains_key("hypotheses")),
            "an absent optional is omitted rather than written as null: {node}"
        );
        assert_eq!(
            Incident::from_node(&node).expect("reads back"),
            incident,
            "an omitted optional must read back as absent, not as an error"
        );
    }

    #[test]
    fn a_runbook_round_trips_through_a_node() {
        let runbook = runbook();
        let read = Runbook::from_node(&runbook.to_node()).expect("reads back");
        assert_eq!(read, runbook);
        assert_eq!(
            read.verification.len(),
            1,
            "verification is a list of its own, not folded into the steps"
        );
    }

    #[test]
    fn a_release_round_trips_through_a_node() {
        let release = release();
        let read = Release::from_node(&release.to_node()).expect("reads back");
        assert_eq!(read, release);
        assert_eq!(
            read.previous_revision.as_deref(),
            Some("3ad77e0"),
            "the previous revision is what a rollback returns to; losing it loses the rollback"
        );
    }

    #[test]
    fn an_incident_body_missing_a_required_field_says_which_one() {
        let mut node = full_incident().to_node();
        let Node::Map(entries) = &mut node else {
            panic!("an incident renders as a mapping");
        };
        entries.remove("service");

        let error = Incident::from_node(&node).expect_err("service is required");
        assert!(
            error.to_string().contains("aop.incident.service"),
            "the rejection should name the field: {error}"
        );
    }

    #[test]
    fn an_unknown_incident_status_is_rejected_and_the_ladder_named() {
        let mut node = full_incident().to_node();
        let Node::Map(entries) = &mut node else {
            panic!("an incident renders as a mapping");
        };
        entries.insert("status".to_owned(), Node::from("closed"));

        let error = Incident::from_node(&node).expect_err("`closed` is not on the ladder");
        let message = error.to_string();
        assert!(message.contains("closed"), "{message}");
        assert!(message.contains("detected"), "{message}");
        assert!(message.contains("resolved"), "{message}");
    }

    #[test]
    fn an_unknown_severity_is_rejected_and_the_scale_named() {
        let mut node = full_incident().to_node();
        let Node::Map(entries) = &mut node else {
            panic!("an incident renders as a mapping");
        };
        entries.insert("severity".to_owned(), Node::from("sev1"));

        let error = Incident::from_node(&node).expect_err("`sev1` is not on the severity scale");
        let message = error.to_string();
        assert!(message.contains("aop.incident.severity"), "{message}");
        assert!(message.contains("critical"), "{message}");
    }

    #[test]
    fn a_hypothesis_written_as_a_bare_string_is_read_as_a_one_element_list() {
        let mut node = bare_incident().to_node();
        let Node::Map(entries) = &mut node else {
            panic!("an incident renders as a mapping");
        };
        entries.insert("hypotheses".to_owned(), Node::from("the cache is cold"));

        let read = Incident::from_node(&node).expect("a scalar is accepted where a list is meant");
        assert_eq!(read.hypotheses, vec!["the cache is cold".to_owned()]);
    }

    #[test]
    fn a_hypothesis_that_is_not_text_is_refused_rather_than_stringified() {
        let mut node = bare_incident().to_node();
        let Node::Map(entries) = &mut node else {
            panic!("an incident renders as a mapping");
        };
        entries.insert("hypotheses".to_owned(), Node::Seq(vec![Node::Bool(true)]));

        let error = Incident::from_node(&node).expect_err("a boolean is not a hypothesis");
        assert!(
            error.to_string().contains("aop.incident.hypotheses"),
            "{error}"
        );
    }

    #[test]
    fn a_timestamp_survives_the_round_trip_through_a_document_number() {
        // The largest instant the protocol can meet before doubles stop representing milliseconds
        // exactly; if this stops round-tripping, the wire form has to change, not the assertion.
        let far_future = Timestamp::from_epoch_millis(9_007_199_254_740_991);
        let mut incident = bare_incident();
        incident.detected_at = far_future;

        let read = Incident::from_node(&incident.to_node()).expect("reads back");
        assert_eq!(read.detected_at, far_future);
        assert_eq!(read.detected_at.epoch_millis(), 9_007_199_254_740_991);
    }

    #[test]
    fn a_timestamp_that_is_not_whole_milliseconds_is_refused() {
        let mut node = bare_incident().to_node();
        let Node::Map(entries) = &mut node else {
            panic!("an incident renders as a mapping");
        };
        entries.insert(
            "detected_at".to_owned(),
            Node::Number(Number::new(1.5).expect("1.5 is a number")),
        );

        let error = Incident::from_node(&node).expect_err("half a millisecond is not an instant");
        let message = error.to_string();
        assert!(message.contains("aop.incident.detected_at"), "{message}");
        assert!(message.contains("whole"), "{message}");
    }

    #[test]
    fn an_incident_climbs_its_ladder_one_rung_at_a_time() {
        for pair in IncidentStatus::ALL.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            assert!(
                IncidentStatus::permits(from, to),
                "{from} should be able to reach {to}"
            );
        }
        assert_eq!(
            IncidentStatus::Detected.successors(),
            vec![IncidentStatus::Triaged]
        );
        assert!(IncidentStatus::Resolved.is_terminal());
        assert!(IncidentStatus::Resolved.successors().is_empty());
    }

    #[test]
    fn an_incident_cannot_go_straight_from_detected_to_resolved() {
        assert!(
            !IncidentStatus::permits(IncidentStatus::Detected, IncidentStatus::Resolved),
            "the shortcut closes the incident with nothing recorded about what was learned"
        );
        assert!(!IncidentStatus::permits(
            IncidentStatus::Detected,
            IncidentStatus::Mitigated
        ));
    }

    #[test]
    fn an_incident_does_not_go_backwards() {
        assert!(
            !IncidentStatus::permits(IncidentStatus::Mitigated, IncidentStatus::Diagnosed),
            "a failed mitigation is new information, not a return to the earlier record"
        );
        assert!(
            !IncidentStatus::permits(IncidentStatus::Triaged, IncidentStatus::Triaged),
            "standing still is not a transition"
        );
    }

    #[test]
    fn a_release_climbs_its_ladder_one_rung_at_a_time() {
        for pair in ReleaseStatus::LADDER.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            assert!(
                ReleaseStatus::permits(from, to),
                "{from} should be able to reach {to}"
            );
        }
        assert!(
            !ReleaseStatus::permits(ReleaseStatus::Staged, ReleaseStatus::Promoted),
            "skipping the canary and the observation is what progressive delivery exists to stop"
        );
        assert!(ReleaseStatus::Verified.is_terminal());
    }

    #[test]
    fn a_release_can_be_rolled_back_from_anything_that_is_deployed() {
        for status in ReleaseStatus::ALL {
            let expected = status.is_deployed();
            assert_eq!(
                ReleaseStatus::permits(*status, ReleaseStatus::RolledBack),
                expected,
                "wrong rollback reachability from {status}"
            );
        }
        assert!(ReleaseStatus::permits(
            ReleaseStatus::Verified,
            ReleaseStatus::RolledBack
        ));
    }

    #[test]
    fn a_qualified_release_cannot_be_rolled_back_because_nothing_is_deployed() {
        assert!(!ReleaseStatus::Qualified.is_deployed());
        assert!(
            !ReleaseStatus::permits(ReleaseStatus::Qualified, ReleaseStatus::RolledBack),
            "a rollback with nothing deployed records a remediation that did not happen"
        );
    }

    #[test]
    fn a_rolled_back_release_is_terminal() {
        assert!(ReleaseStatus::RolledBack.is_terminal());
        assert!(
            ReleaseStatus::RolledBack.successors().is_empty(),
            "the way forward from a rollback is a new release, not a resumed rollout"
        );
    }

    #[test]
    fn every_status_has_a_distinct_document_spelling_that_parses_back() {
        let mut seen: Vec<&str> = Vec::new();
        for status in IncidentStatus::ALL {
            let rendered = status.as_str();
            assert!(!seen.contains(&rendered), "two statuses spell {rendered:?}");
            seen.push(rendered);
            assert_eq!(IncidentStatus::parse(rendered).expect(rendered), *status);
            assert_eq!(status.to_string(), rendered);
        }
        let mut seen: Vec<&str> = Vec::new();
        for status in ReleaseStatus::ALL {
            let rendered = status.as_str();
            assert!(!seen.contains(&rendered), "two statuses spell {rendered:?}");
            seen.push(rendered);
            assert_eq!(ReleaseStatus::parse(rendered).expect(rendered), *status);
            assert_eq!(status.to_string(), rendered);
        }
    }
}
