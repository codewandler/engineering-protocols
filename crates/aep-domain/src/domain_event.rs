//! Domain events: immutable facts about what happened to engineering entities.
//!
//! This is not the same stream as [`crate::event`]. That one records what a single protocol
//! *execution* did — states entered, capabilities refused, evidence submitted — and its
//! envelope is sequenced within one execution. This one records what happened to the entities
//! an organisation actually cares about: a story was started, a design was approved, a release
//! was promoted. A domain event outlives the execution that produced it, and is meaningful to
//! somebody who has never heard of the workflow that emitted it.
//!
//! # An event is not an audit record
//!
//! The two are constantly confused, and the confusion is expensive, because it produces a
//! system that can tell you a change happened but not who was refused.
//!
//! | | domain event | audit record |
//! |---|---|---|
//! | asserts | *what occurred* | *who caused it, and what was decided* |
//! | exists when the command succeeded | yes | yes |
//! | exists when the command was **denied** | **no** | **yes** |
//! | may be replayed to rebuild state | yes | no |
//! | may be redacted | no — it is a fact | yes (§57) |
//!
//! A denied `ApproveDesign` produces an audit record and **no event**: nothing occurred, but
//! somebody tried and the system decided, and that decision is exactly what an incident review
//! six months later needs to see. Emitting a "design approval denied" *event* would put a
//! non-fact into the fact stream, and every consumer replaying it would have to know to ignore
//! it. See §55 for the rejected-attempt audit rules.
//!
//! # Correlation is not causation
//!
//! Two different questions, so two different fields (§38):
//!
//! * **correlation** — *what belongs to the same overall activity?* One id, shared by everything
//!   downstream of one user request.
//! * **causation** — *what directly caused this one thing?* A single immediate parent.
//!
//! Neither can be derived from the other. Correlation alone gives you a bag of records with no
//! order; causation alone gives you a chain with no idea which activity it served.
//!
//! ```text
//! USER REQUEST                         correlation = C42
//!       │
//!       ▼
//! ApproveDesign command  CA            correlation = C42
//!       │
//!       ▼
//! aep.design.approved/v1  event A1     correlation = C42   causation = CA
//!       │
//!       ▼
//! protocol decision D1                 correlation = C42   causation = A1
//!       │
//!       ▼
//! StartImplementation command CB       correlation = C42   causation = D1
//!       │
//!       ▼
//! aep.story.started/v1   event B1      correlation = C42   causation = CB
//! ```
//!
//! Every record in that chain carries `C42`, so the whole activity can be pulled back out of a
//! log with one query; each carries exactly one direct cause, so the order can be reconstructed
//! without timestamps. §51 is the same picture read forwards: an event lets the protocol observe
//! that a requirement is now satisfied, which produces a decision, which produces the next
//! command.
//!
//! # Why this type deserializes
//!
//! Unlike the document types in this crate, [`DomainEventEnvelope`] implements
//! [`Deserialize`](serde::Deserialize). It is a wire record, not a document: it arrives already
//! constructed from a backend that emitted it, and there is no raw-then-validate stage to put
//! between the two. [`DomainEventEnvelope::validate`] is therefore a check a consumer may run on
//! a record it received, not a parse boundary.

use std::fmt;
use std::str::FromStr;

use crate::artifact::RelationKind;
use crate::capability::Environment;
use crate::entity::{ActorRef, EntityRef, EntityRevision, EntityType};
use crate::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use crate::ids::{CommandId, CorrelationId, EventId, ExecutionId, RelationId};
use crate::node::Node;
use crate::time::Timestamp;

/// A versioned domain event name, written `<namespace>.<subject>.<verb>/v<version>`.
///
/// The wire name is the contract. A consumer that has never seen this crate must be able to
/// route on it, which is why the version is in the name: `aep.design.approved/v2` is a
/// different event from `aep.design.approved/v1`, not the same event with new fields, and a
/// consumer written against v1 can ignore it rather than misread it.
///
/// The vocabulary is **open**. The named constructors below cover what AEP itself defines, but
/// any well-formed name parses, so an organisation can emit `acme.contract.signed/v1` without
/// changing this crate. Unknown names carry a [`DomainEvent::Custom`] payload.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct DomainEventType {
    namespace: String,
    subject: String,
    verb: String,
    version: u32,
}

impl DomainEventType {
    /// Builds an event type from its parts.
    ///
    /// The namespace may be dotted (`acme.payments`); the subject and the verb may not, because
    /// the dot is what separates them.
    pub fn new(
        namespace: impl AsRef<str>,
        subject: impl AsRef<str>,
        verb: impl AsRef<str>,
        version: u32,
    ) -> Result<Self, ParseError> {
        let namespace = namespace.as_ref();
        let subject = subject.as_ref();
        let verb = verb.as_ref();
        let rendered = format!("{namespace}.{subject}.{verb}/v{version}");
        let reject =
            |reason: String| ParseError::identifier("domain event type", &rendered, reason);

        if version == 0 {
            return Err(reject("event type versions start at 1".to_owned()));
        }
        for (part, label, dotted) in [
            (namespace, "namespace", true),
            (subject, "subject", false),
            (verb, "verb", false),
        ] {
            if part.is_empty() {
                return Err(reject(format!("the {label} must not be empty")));
            }
            let separators: &[char] = if dotted { &['.', '-'] } else { &['-'] };
            let well_formed = part.split(separators).all(|segment| {
                !segment.is_empty()
                    && segment.chars().all(|character| {
                        character.is_ascii_lowercase() || character.is_ascii_digit()
                    })
            });
            if !well_formed {
                let allowance = if dotted { ", optionally dotted" } else { "" };
                return Err(reject(format!(
                    "the {label} must be lower-case kebab-case{allowance}"
                )));
            }
        }
        Ok(Self {
            namespace: namespace.to_owned(),
            subject: subject.to_owned(),
            verb: verb.to_owned(),
            version,
        })
    }

    /// Parses `<namespace>.<subject>.<verb>/v<version>`.
    ///
    /// Any well-formed name is accepted, named or not: the point of a versioned wire name is
    /// that adding a vocabulary entry is not a code change for anybody downstream.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let reject =
            |reason: &str| ParseError::identifier("domain event type", value, reason.to_owned());

        let (qualified, version) = value
            .rsplit_once('/')
            .ok_or_else(|| reject("expected `<namespace>.<subject>.<verb>/v<version>`"))?;
        let version = version
            .strip_prefix('v')
            .ok_or_else(|| reject("the version is written `v1`"))?
            .parse::<u32>()
            .map_err(|_| reject("the version must be an integer"))?;
        let (head, verb) = qualified
            .rsplit_once('.')
            .ok_or_else(|| reject("expected a subject and a verb, as in `aep.story.created/v1`"))?;
        let (namespace, subject) = head.rsplit_once('.').ok_or_else(|| {
            reject("expected a namespace, a subject and a verb, as in `aep.story.created/v1`")
        })?;
        Self::new(namespace, subject, verb, version)
    }

    /// The namespace, such as `aep`.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The subject noun, such as `story`.
    ///
    /// This is the *kind* of thing the event is about, not which one; the entity itself is
    /// [`DomainEventEnvelope::subject`].
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The verb, such as `created` or `submitted-for-review`.
    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// The major version of the payload shape.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// `true` when both name the same event, ignoring version.
    ///
    /// Useful for a consumer that wants "any version of design approval" without enumerating
    /// versions it has not been told about.
    pub fn same_family(&self, other: &Self) -> bool {
        self.namespace == other.namespace
            && self.subject == other.subject
            && self.verb == other.verb
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str =
        "^[a-z0-9][a-z0-9.-]*\\.[a-z0-9-]+\\.[a-z0-9-]+/v[1-9][0-9]*$";
}

/// Declares the event types AEP itself names, keeping the wire literal in exactly one place.
macro_rules! named_domain_event_types {
    ($($(#[$meta:meta])* $constructor:ident => $wire:literal),* $(,)?) => {
        impl DomainEventType {
            $(
                $(#[$meta])*
                pub fn $constructor() -> Self {
                    Self::parse($wire).expect("a named event type is well formed")
                }
            )*

            /// Every event type AEP names, in wire form.
            ///
            /// A name absent from this list is not invalid — it is somebody else's.
            pub const NAMED: &'static [&'static str] = &[$($wire),*];

            /// Every event type AEP names, parsed.
            pub fn named() -> Vec<Self> {
                vec![$(Self::$constructor()),*]
            }
        }
    };
}

named_domain_event_types! {
    /// `aep.story.created/v1` — a story exists that did not exist before.
    story_created => "aep.story.created/v1",
    /// `aep.story.started/v1` — work on a story began.
    story_started => "aep.story.started/v1",
    /// `aep.design.submitted-for-review/v1` — a design revision entered review.
    design_submitted_for_review => "aep.design.submitted-for-review/v1",
    /// `aep.design.approved/v1` — a review concluded that a design revision may proceed.
    design_approved => "aep.design.approved/v1",
    /// `aep.adr.accepted/v1` — a decision record became binding.
    adr_accepted => "aep.adr.accepted/v1",
    /// `aep.entity.updated/v1` — an entity changed, moving from one revision to the next.
    entity_updated => "aep.entity.updated/v1",
    /// `aep.entity.archived/v1` — an entity was withdrawn from use without being deleted (§43).
    entity_archived => "aep.entity.archived/v1",
    /// `aep.entity.superseded/v1` — a newer entity took over from this one.
    entity_superseded => "aep.entity.superseded/v1",
    /// `aep.relation.created/v1` — an edge was added to the entity graph.
    relation_created => "aep.relation.created/v1",
    /// `aep.relation.removed/v1` — an edge was withdrawn from the entity graph.
    relation_removed => "aep.relation.removed/v1",
    /// `aop.incident.mitigated/v1` — customer impact stopped, whether or not the cause is fixed.
    incident_mitigated => "aop.incident.mitigated/v1",
    /// `aop.release.promoted/v1` — a release moved from one environment to the next.
    release_promoted => "aop.release.promoted/v1",
}

impl fmt::Display for DomainEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}/v{}",
            self.namespace, self.subject, self.verb, self.version
        )
    }
}

impl fmt::Debug for DomainEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DomainEventType({self})")
    }
}

impl FromStr for DomainEventType {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<DomainEventType> for String {
    fn from(value: DomainEventType) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for DomainEventType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for DomainEventType {
    fn schema_name() -> String {
        "DomainEventType".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "A versioned domain event name, written `<namespace>.<subject>.<verb>/v<version>`."
                .to_owned(),
        );
        schema.metadata().examples = Self::NAMED
            .iter()
            .map(|value| serde_json::Value::String((*value).to_owned()))
            .collect();
        schema.into()
    }
}

/// What a domain event asserts.
///
/// Each variant carries only what its event is a fact *about* — who did it, when, and which
/// activity it belonged to live on [`DomainEventEnvelope`], not here, so the same payload can be
/// replayed from a log without dragging its transport context along.
///
/// The enum is `#[non_exhaustive]` and ends in [`Custom`](DomainEvent::Custom): the vocabulary is
/// open by design, and a consumer must be able to survive an event it has never seen.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "event", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DomainEvent {
    /// A story was created.
    StoryCreated {
        /// The new story.
        story: EntityRef,
        /// What it was called at creation.
        title: String,
    },
    /// Work on a story began.
    StoryStarted {
        /// Which story.
        story: EntityRef,
        /// The revision the start produced.
        revision: EntityRevision,
        /// Who picked it up, when that is not the actor who issued the command.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assignee: Option<ActorRef>,
    },
    /// A design revision was submitted for review.
    DesignSubmittedForReview {
        /// Which design.
        design: EntityRef,
        /// The exact revision under review; a later revision is not covered by it.
        revision: EntityRevision,
        /// The review that was opened.
        review: EntityRef,
    },
    /// A review approved a design revision.
    DesignApproved {
        /// Which design.
        design: EntityRef,
        /// The exact revision approved.
        revision: EntityRevision,
        /// The review that approved it.
        review: EntityRef,
        /// Who approved. Named here as well as on the envelope: an approval recorded without
        /// the approver is not evidence of anything.
        approver: ActorRef,
    },
    /// An architecture decision record became binding.
    AdrAccepted {
        /// Which record.
        adr: EntityRef,
        /// The revision accepted.
        revision: EntityRevision,
        /// Decisions this one replaces.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        supersedes: Vec<EntityRef>,
    },
    /// An entity changed.
    EntityUpdated {
        /// Which entity.
        entity: EntityRef,
        /// What it is, so a consumer can route without resolving the entity first.
        entity_type: EntityType,
        /// The revision before the change.
        from_revision: EntityRevision,
        /// The revision the change produced.
        to_revision: EntityRevision,
        /// Which fields changed (§56). Names only: values may be sensitive, and this stream is
        /// not redactable.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        changed: Vec<String>,
    },
    /// An entity was archived: withdrawn from use, not deleted (§43).
    EntityArchived {
        /// Which entity.
        entity: EntityRef,
        /// The revision at which it was archived.
        revision: EntityRevision,
        /// Why, when a reason was given.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// A newer entity took over from this one.
    EntitySuperseded {
        /// Which entity was superseded.
        entity: EntityRef,
        /// The revision at which it was superseded.
        revision: EntityRevision,
        /// What replaces it.
        superseded_by: EntityRef,
    },
    /// An edge was added to the entity graph.
    RelationCreated {
        /// Identity of the edge, so it can be removed later.
        relation: RelationId,
        /// What the edge means.
        kind: RelationKind,
        /// The source entity.
        from: EntityRef,
        /// The target entity.
        to: EntityRef,
    },
    /// An edge was withdrawn from the entity graph.
    RelationRemoved {
        /// Identity of the edge that was removed.
        relation: RelationId,
        /// What the edge meant.
        kind: RelationKind,
        /// The source entity.
        from: EntityRef,
        /// The target entity.
        to: EntityRef,
    },
    /// An incident stopped affecting users, whether or not the cause is fixed.
    IncidentMitigated {
        /// Which incident.
        incident: EntityRef,
        /// The revision the mitigation produced.
        revision: EntityRevision,
        /// What was done, when it was recorded.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mitigation: Option<String>,
    },
    /// A release moved from one environment to the next.
    ReleasePromoted {
        /// Which release.
        release: EntityRef,
        /// The revision the promotion produced.
        revision: EntityRevision,
        /// Where it came from.
        from: Environment,
        /// Where it went.
        to: Environment,
    },
    /// An event this crate does not name.
    ///
    /// The escape hatch that keeps the vocabulary open: an organisation emits its own type and
    /// its own payload, and everything about the envelope — correlation, causation, actor,
    /// validation — still applies.
    Custom {
        /// The organisation's own event type.
        event_type: DomainEventType,
        /// Its payload, uninterpreted.
        data: Node,
    },
}

impl DomainEvent {
    /// The wire type this payload is.
    ///
    /// The envelope declares a type as well; [`DomainEventEnvelope::validate`] checks the two
    /// agree, which is what stops a payload from being routed as something it is not.
    pub fn event_type(&self) -> DomainEventType {
        match self {
            Self::StoryCreated { .. } => DomainEventType::story_created(),
            Self::StoryStarted { .. } => DomainEventType::story_started(),
            Self::DesignSubmittedForReview { .. } => DomainEventType::design_submitted_for_review(),
            Self::DesignApproved { .. } => DomainEventType::design_approved(),
            Self::AdrAccepted { .. } => DomainEventType::adr_accepted(),
            Self::EntityUpdated { .. } => DomainEventType::entity_updated(),
            Self::EntityArchived { .. } => DomainEventType::entity_archived(),
            Self::EntitySuperseded { .. } => DomainEventType::entity_superseded(),
            Self::RelationCreated { .. } => DomainEventType::relation_created(),
            Self::RelationRemoved { .. } => DomainEventType::relation_removed(),
            Self::IncidentMitigated { .. } => DomainEventType::incident_mitigated(),
            Self::ReleasePromoted { .. } => DomainEventType::release_promoted(),
            Self::Custom { event_type, .. } => event_type.clone(),
        }
    }

    /// The entity this event is primarily about, when there is one.
    ///
    /// A relation event has none on purpose: an edge is not an entity, and nominating one of its
    /// two ends as *the* subject would make `from` and `to` asymmetric in a way the graph is not.
    /// Both ends are in the payload instead.
    pub fn subject(&self) -> Option<EntityRef> {
        match self {
            Self::StoryCreated { story, .. } | Self::StoryStarted { story, .. } => {
                Some(story.clone())
            }
            Self::DesignSubmittedForReview { design, .. } | Self::DesignApproved { design, .. } => {
                Some(design.clone())
            }
            Self::AdrAccepted { adr, .. } => Some(adr.clone()),
            Self::EntityUpdated { entity, .. }
            | Self::EntityArchived { entity, .. }
            | Self::EntitySuperseded { entity, .. } => Some(entity.clone()),
            Self::IncidentMitigated { incident, .. } => Some(incident.clone()),
            Self::ReleasePromoted { release, .. } => Some(release.clone()),
            Self::RelationCreated { .. } | Self::RelationRemoved { .. } | Self::Custom { .. } => {
                None
            }
        }
    }

    /// The revision of [`subject`](Self::subject) this event is a fact about.
    ///
    /// Paired with `subject` so that an envelope built from a payload cannot violate "a subject
    /// implies a revision": whenever one is `Some`, so is the other. A creation reports
    /// [`EntityRevision::INITIAL`], because that is what "newly created" means.
    pub fn subject_revision(&self) -> Option<EntityRevision> {
        match self {
            Self::StoryCreated { .. } => Some(EntityRevision::INITIAL),
            Self::StoryStarted { revision, .. }
            | Self::DesignSubmittedForReview { revision, .. }
            | Self::DesignApproved { revision, .. }
            | Self::AdrAccepted { revision, .. }
            | Self::EntityArchived { revision, .. }
            | Self::EntitySuperseded { revision, .. }
            | Self::IncidentMitigated { revision, .. }
            | Self::ReleasePromoted { revision, .. } => Some(*revision),
            Self::EntityUpdated { to_revision, .. } => Some(*to_revision),
            Self::RelationCreated { .. } | Self::RelationRemoved { .. } | Self::Custom { .. } => {
                None
            }
        }
    }
}

/// One domain event with everything needed to place it in an activity (§50).
///
/// Named `DomainEventEnvelope` rather than `EventEnvelope` because [`crate::event::EventEnvelope`]
/// already exists and means something else — a position in one execution's audit stream. A call
/// site should never have to work out which of the two it is holding.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct DomainEventEnvelope {
    /// Identity of this event. Never reused, so a consumer can deduplicate.
    pub event_id: EventId,
    /// What kind of event this is, in wire form.
    pub event_type: DomainEventType,
    /// The entity the event is about, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<EntityRef>,
    /// The revision of `subject` the event is a fact about.
    ///
    /// Required whenever `subject` is present: an event that says an entity changed but not
    /// which revision it produced cannot be replayed against history, and cannot be checked
    /// against an approval that pinned a revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_revision: Option<EntityRevision>,
    /// What the event asserts.
    pub payload: DomainEvent,
    /// The command that caused it, when one did.
    ///
    /// Optional because not every fact comes from a command: a backend may report an
    /// externally-observed change. When it is present, `causation` must name it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<CommandId>,
    /// What activity this belongs to. Shared by every record downstream of one request (§38).
    pub correlation_id: CorrelationId,
    /// What directly caused this one event (§38).
    ///
    /// A [`String`] for now. It will become a typed `CausationRef` once the audit module lands,
    /// which is what will let a reader tell a causing command from a causing event or protocol
    /// decision without parsing the id. Until then the convention is §38's: the bare id of the
    /// direct cause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation: Option<String>,
    /// The protocol execution that produced it, when a protocol was running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<ExecutionId>,
    /// When it occurred.
    pub occurred_at: Timestamp,
    /// Who it is attributed to.
    pub actor: ActorRef,
}

impl DomainEventEnvelope {
    /// An event caused by `command`.
    ///
    /// This is the constructor to reach for: §50 requires that an event caused by a command
    /// reference that command as its direct cause, and this is the only way to get that right
    /// without remembering to. The type, subject and revision are taken from the payload, so
    /// they cannot contradict it.
    pub fn from_command(
        event_id: EventId,
        command: CommandId,
        correlation_id: CorrelationId,
        payload: DomainEvent,
        occurred_at: Timestamp,
        actor: ActorRef,
    ) -> Self {
        let causation = command.to_string();
        Self {
            event_id,
            event_type: payload.event_type(),
            subject: payload.subject(),
            entity_revision: payload.subject_revision(),
            payload,
            command_id: Some(command),
            correlation_id,
            causation: Some(causation),
            execution_id: None,
            occurred_at,
            actor,
        }
    }

    /// An event with no causing command.
    ///
    /// Deliberately the awkward name: an event without a command is the exception — a change
    /// observed in an external system, or a fact a service asserts on its own schedule — and it
    /// should be visible at the call site that this one has no command behind it. Attach a cause
    /// with [`caused_by`](Self::caused_by) where one is known.
    pub fn without_command(
        event_id: EventId,
        correlation_id: CorrelationId,
        payload: DomainEvent,
        occurred_at: Timestamp,
        actor: ActorRef,
    ) -> Self {
        Self {
            event_id,
            event_type: payload.event_type(),
            subject: payload.subject(),
            entity_revision: payload.subject_revision(),
            payload,
            command_id: None,
            correlation_id,
            causation: None,
            execution_id: None,
            occurred_at,
            actor,
        }
    }

    /// Records what directly caused this event.
    ///
    /// For an envelope built by [`from_command`](Self::from_command) the cause is already the
    /// command; overriding it with anything else makes the envelope invalid, because §50 says
    /// the command *is* the direct cause.
    #[must_use]
    pub fn caused_by(mut self, causation: impl Into<String>) -> Self {
        self.causation = Some(causation.into());
        self
    }

    /// Records the protocol execution this event was produced in.
    #[must_use]
    pub fn in_execution(mut self, execution: ExecutionId) -> Self {
        self.execution_id = Some(execution);
        self
    }

    /// Checks the envelope against the invariants §50 states.
    ///
    /// Three things, all of which are silent corruption rather than a crash if they are wrong:
    ///
    /// 1. the declared `event_type` matches the payload — otherwise consumers route on a lie;
    /// 2. `subject` and `entity_revision` are present together — an entity fact with no revision
    ///    cannot be placed in that entity's history, and a revision with no entity belongs to
    ///    nothing;
    /// 3. an event that names a causing command names that command as its direct cause.
    ///
    /// Errors accumulate: a record with three problems reports three.
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();

        let declared = &self.event_type;
        let actual = self.payload.event_type();
        if *declared != actual {
            errors.push(
                ValidationError::new(
                    ValidationCode::EventPayloadMismatch,
                    "event.event_type",
                    format!("the envelope declares `{declared}` but carries a `{actual}` payload"),
                )
                .with_hint(
                    "build the envelope with `DomainEventEnvelope::from_command`, which takes \
                     the type from the payload",
                ),
            );
        }

        match (&self.subject, self.entity_revision) {
            (Some(subject), None) => errors.push(
                ValidationError::new(
                    ValidationCode::IncompleteEventSubject,
                    "event.entity_revision",
                    format!("the event is about entity {subject} but names no revision of it"),
                )
                .with_hint(
                    "an entity fact without a revision cannot be placed in that entity's \
                     history, nor checked against an approval that pinned a revision",
                ),
            ),
            (None, Some(revision)) => errors.push(
                ValidationError::new(
                    ValidationCode::IncompleteEventSubject,
                    "event.subject",
                    format!("the event names revision {revision} but no entity it belongs to"),
                )
                .with_hint("name the subject entity, or drop the revision"),
            ),
            _ => {}
        }

        if let Some(command) = &self.command_id {
            let expected = command.to_string();
            match &self.causation {
                Some(cause) if *cause == expected => {}
                Some(cause) => errors.push(
                    ValidationError::new(
                        ValidationCode::MissingCausation,
                        "event.causation",
                        format!(
                            "the event was caused by command `{command}` but names `{cause}` as \
                             its direct cause"
                        ),
                    )
                    .with_hint(
                        "an event caused by a command references that command as its direct \
                         cause; the earlier decision is reachable through the command",
                    ),
                ),
                None => errors.push(
                    ValidationError::new(
                        ValidationCode::MissingCausation,
                        "event.causation",
                        format!(
                            "the event was caused by command `{command}` but names no direct cause"
                        ),
                    )
                    .with_hint("use `DomainEventEnvelope::from_command`"),
                ),
            }
        }

        errors.into_result(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityId;
    use crate::facts::Number;
    use std::collections::BTreeMap;

    fn entity(name: &str) -> EntityRef {
        EntityRef::new(EntityId::new(format!("{name}-000000000000")).expect("entity id"))
    }

    fn revision(value: u64) -> EntityRevision {
        EntityRevision::new(value).expect("revision")
    }

    fn correlation() -> CorrelationId {
        CorrelationId::new("C42").expect("correlation id")
    }

    fn command() -> CommandId {
        CommandId::new("CA").expect("command id")
    }

    fn event_id() -> EventId {
        EventId::new("A1").expect("event id")
    }

    fn actor() -> ActorRef {
        ActorRef::parse("human:alice").expect("actor")
    }

    fn design_approved() -> DomainEvent {
        DomainEvent::DesignApproved {
            design: entity("design"),
            revision: revision(3),
            review: entity("review"),
            approver: actor(),
        }
    }

    fn one_of_each() -> Vec<DomainEvent> {
        vec![
            DomainEvent::StoryCreated {
                story: entity("story"),
                title: "Passkey sign-in".to_owned(),
            },
            DomainEvent::StoryStarted {
                story: entity("story"),
                revision: revision(2),
                assignee: Some(ActorRef::parse("agent:impl-7").expect("actor")),
            },
            DomainEvent::DesignSubmittedForReview {
                design: entity("design"),
                revision: revision(3),
                review: entity("review"),
            },
            design_approved(),
            DomainEvent::AdrAccepted {
                adr: entity("adr"),
                revision: revision(1),
                supersedes: vec![entity("oldadr")],
            },
            DomainEvent::EntityUpdated {
                entity: entity("story"),
                entity_type: EntityType::parse("aep.story/v1").expect("entity type"),
                from_revision: revision(4),
                to_revision: revision(5),
                changed: vec!["title".to_owned()],
            },
            DomainEvent::EntityArchived {
                entity: entity("story"),
                revision: revision(6),
                reason: Some("duplicate".to_owned()),
            },
            DomainEvent::EntitySuperseded {
                entity: entity("design"),
                revision: revision(7),
                superseded_by: entity("newdesign"),
            },
            DomainEvent::RelationCreated {
                relation: RelationId::new("rel-1").expect("relation id"),
                kind: RelationKind::Designs,
                from: entity("design"),
                to: entity("story"),
            },
            DomainEvent::RelationRemoved {
                relation: RelationId::new("rel-1").expect("relation id"),
                kind: RelationKind::Designs,
                from: entity("design"),
                to: entity("story"),
            },
            DomainEvent::IncidentMitigated {
                incident: entity("incident"),
                revision: revision(2),
                mitigation: Some("traffic shifted away from eu-west-1".to_owned()),
            },
            DomainEvent::ReleasePromoted {
                release: entity("release"),
                revision: revision(2),
                from: Environment::Staging,
                to: Environment::Production,
            },
        ]
    }

    #[test]
    fn every_named_event_type_round_trips_through_its_wire_name() {
        for wire in DomainEventType::NAMED {
            let parsed = DomainEventType::parse(wire).expect("a named type parses");
            assert_eq!(parsed.to_string(), *wire);

            let json = serde_json::to_value(&parsed).expect("serialises");
            assert_eq!(json, serde_json::Value::String((*wire).to_owned()));

            let back: DomainEventType = serde_json::from_value(json).expect("deserialises");
            assert_eq!(back, parsed);
        }

        let constructed: Vec<String> = DomainEventType::named()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            constructed,
            DomainEventType::NAMED
                .iter()
                .map(|wire| (*wire).to_owned())
                .collect::<Vec<_>>(),
            "the named constructors and the wire list must not drift apart"
        );
    }

    #[test]
    fn an_organisation_may_emit_an_event_type_this_crate_does_not_name() {
        let custom = DomainEventType::parse("acme.contract.signed/v2").expect("well formed");
        assert_eq!(custom.namespace(), "acme");
        assert_eq!(custom.subject(), "contract");
        assert_eq!(custom.verb(), "signed");
        assert_eq!(custom.version(), 2);
        assert!(
            !DomainEventType::NAMED.contains(&custom.to_string().as_str()),
            "the point is that it is not one of ours"
        );

        let dotted =
            DomainEventType::parse("acme.payments.invoice.issued/v1").expect("dotted namespace");
        assert_eq!(dotted.namespace(), "acme.payments");
        assert_eq!(dotted.subject(), "invoice");
    }

    #[test]
    fn a_malformed_event_type_is_rejected_with_the_reason_it_failed() {
        for (value, expected) in [
            ("aep.story.created", "v<version>"),
            ("aep.story.created/1", "written `v1`"),
            ("aep.story.created/vx", "must be an integer"),
            (
                "aep.created/v1",
                "expected a namespace, a subject and a verb",
            ),
            ("created/v1", "expected a subject and a verb"),
            (
                "aep.Story.created/v1",
                "the subject must be lower-case kebab-case",
            ),
            (
                "aep.story.Created/v1",
                "the verb must be lower-case kebab-case",
            ),
            ("aep..created/v1", "the subject must not be empty"),
            ("aep.story.created/v0", "versions start at 1"),
        ] {
            let error = DomainEventType::parse(value).expect_err(value);
            assert!(
                error.to_string().contains(expected),
                "{value}: expected {expected:?}, got {error}"
            );
        }
    }

    #[test]
    fn a_type_matches_its_own_family_across_versions_but_not_another_event() {
        let v1 = DomainEventType::design_approved();
        let v2 = DomainEventType::new("aep", "design", "approved", 2).expect("v2");
        assert!(v1.same_family(&v2));
        assert_ne!(
            v1, v2,
            "a version bump is a different event, not the same one"
        );
        assert!(!v1.same_family(&DomainEventType::design_submitted_for_review()));
    }

    #[test]
    fn every_payload_declares_a_type_this_crate_names() {
        for payload in one_of_each() {
            let declared = payload.event_type().to_string();
            assert!(
                DomainEventType::NAMED.contains(&declared.as_str()),
                "{declared} is not in the named vocabulary"
            );
        }
    }

    #[test]
    fn an_envelope_whose_declared_type_contradicts_its_payload_is_rejected() {
        let mut envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::from_epoch_millis(1_700_000_000_000),
            actor(),
        );
        envelope.validate().expect("built from its payload");

        envelope.event_type = DomainEventType::story_created();
        let errors = envelope
            .validate()
            .expect_err("the type now lies about the payload");
        assert!(
            errors.contains(ValidationCode::EventPayloadMismatch),
            "{errors}"
        );
        assert_eq!(errors.as_slice()[0].location, "event.event_type");
        assert!(
            errors.to_string().contains("aep.design.approved/v1"),
            "the error names the payload it actually carries: {errors}"
        );
    }

    #[test]
    fn a_subject_without_a_revision_is_rejected() {
        let mut envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::EPOCH,
            actor(),
        );
        envelope.entity_revision = None;

        let errors = envelope.validate().expect_err("subject without revision");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors.as_slice()[0].code,
            ValidationCode::IncompleteEventSubject
        );
        assert_eq!(errors.as_slice()[0].location, "event.entity_revision");
        assert!(errors.to_string().contains("names no revision"), "{errors}");
    }

    #[test]
    fn a_revision_without_a_subject_is_rejected() {
        let mut envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::EPOCH,
            actor(),
        );
        envelope.subject = None;

        let errors = envelope.validate().expect_err("revision without subject");
        assert_eq!(errors.as_slice()[0].location, "event.subject");
        assert!(
            errors.to_string().contains("no entity it belongs to"),
            "{errors}"
        );
    }

    #[test]
    fn from_command_records_the_command_as_the_direct_cause() {
        let envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::from_epoch_millis(7),
            actor(),
        );

        assert_eq!(envelope.command_id.as_ref().expect("command"), &command());
        assert_eq!(envelope.causation.as_deref(), Some("CA"));
        assert_eq!(envelope.event_type, DomainEventType::design_approved());
        assert_eq!(envelope.subject, Some(entity("design")));
        assert_eq!(envelope.entity_revision, Some(revision(3)));
        envelope.validate().expect("valid by construction");
    }

    #[test]
    fn an_event_that_names_a_command_but_a_different_cause_is_rejected() {
        let envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::EPOCH,
            actor(),
        )
        .caused_by("D1");

        let errors = envelope
            .validate()
            .expect_err("the command is the direct cause");
        assert_eq!(errors.as_slice()[0].location, "event.causation");
        assert!(errors.to_string().contains("`D1`"), "{errors}");
        assert!(errors.to_string().contains("`CA`"), "{errors}");
    }

    #[test]
    fn an_event_with_no_causing_command_needs_no_causation() {
        let envelope = DomainEventEnvelope::without_command(
            event_id(),
            correlation(),
            design_approved(),
            Timestamp::EPOCH,
            actor(),
        );
        assert!(envelope.command_id.is_none());
        assert!(envelope.causation.is_none());
        envelope
            .validate()
            .expect("a fact may be observed rather than commanded");

        let with_cause = envelope.caused_by("D1");
        assert_eq!(with_cause.causation.as_deref(), Some("D1"));
        with_cause.validate().expect("a non-command cause is fine");
    }

    #[test]
    fn correlation_and_causation_survive_a_serde_round_trip() {
        let envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::from_epoch_millis(1_700_000_000_000),
            actor(),
        )
        .in_execution(ExecutionId::new("exec-1").expect("execution id"));

        let json = serde_json::to_value(&envelope).expect("serialises");
        assert_eq!(json["correlation_id"], "C42");
        assert_eq!(json["causation"], "CA");
        assert_eq!(json["event_type"], "aep.design.approved/v1");
        assert_eq!(json["payload"]["event"], "design_approved");

        let back: DomainEventEnvelope = serde_json::from_value(json).expect("deserialises");
        assert_eq!(back, envelope);
        assert_eq!(back.correlation_id, correlation());
        back.validate().expect("still valid after a round trip");
    }

    #[test]
    fn a_custom_event_keeps_its_data() {
        let mut data = BTreeMap::new();
        data.insert("contract".to_owned(), Node::from("acme-2026-04"));
        data.insert("value".to_owned(), Node::Number(Number::from(42_i64)));

        let event_type = DomainEventType::parse("acme.contract.signed/v1").expect("well formed");
        let payload = DomainEvent::Custom {
            event_type: event_type.clone(),
            data: Node::Map(data),
        };
        assert_eq!(payload.event_type(), event_type);
        assert!(payload.subject().is_none());

        let envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            payload,
            Timestamp::EPOCH,
            actor(),
        );
        envelope
            .validate()
            .expect("an unknown type is not an invalid one");

        let json = serde_json::to_value(&envelope).expect("serialises");
        assert_eq!(json["event_type"], "acme.contract.signed/v1");
        assert_eq!(json["payload"]["data"]["contract"], "acme-2026-04");

        let back: DomainEventEnvelope = serde_json::from_value(json).expect("deserialises");
        let DomainEvent::Custom { data, .. } = &back.payload else {
            panic!("expected a custom payload, got {:?}", back.payload);
        };
        let entries = data.as_map().expect("a mapping");
        assert_eq!(entries["contract"].as_text(), Some("acme-2026-04"));
        assert_eq!(
            entries["value"],
            Node::Number(Number::from(42_i64)),
            "the data is carried through uninterpreted"
        );
    }

    #[test]
    fn a_relation_event_has_no_subject_because_a_relation_is_not_an_entity() {
        let payload = DomainEvent::RelationCreated {
            relation: RelationId::new("rel-1").expect("relation id"),
            kind: RelationKind::Designs,
            from: entity("design"),
            to: entity("story"),
        };
        assert!(payload.subject().is_none());
        assert!(payload.subject_revision().is_none());

        let envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            payload,
            Timestamp::EPOCH,
            actor(),
        );
        assert!(envelope.subject.is_none());
        envelope
            .validate()
            .expect("no subject means no revision is owed");
    }

    #[test]
    fn a_creation_event_pins_the_initial_revision_and_an_update_pins_the_one_it_produced() {
        let created = DomainEvent::StoryCreated {
            story: entity("story"),
            title: "Passkey sign-in".to_owned(),
        };
        assert_eq!(created.subject_revision(), Some(EntityRevision::INITIAL));

        let updated = DomainEvent::EntityUpdated {
            entity: entity("story"),
            entity_type: EntityType::parse("aep.story/v1").expect("entity type"),
            from_revision: revision(4),
            to_revision: revision(5),
            changed: vec!["title".to_owned()],
        };
        assert_eq!(
            updated.subject_revision(),
            Some(revision(5)),
            "an update is a fact about the revision it produced, not the one it replaced"
        );
    }

    #[test]
    fn an_envelope_reports_every_broken_invariant_at_once() {
        let mut envelope = DomainEventEnvelope::from_command(
            event_id(),
            command(),
            correlation(),
            design_approved(),
            Timestamp::EPOCH,
            actor(),
        );
        envelope.event_type = DomainEventType::story_created();
        envelope.entity_revision = None;
        envelope.causation = None;

        let errors = envelope.validate().expect_err("three problems");
        assert_eq!(errors.len(), 3, "{errors}");
        let locations: Vec<&str> = errors
            .as_slice()
            .iter()
            .map(|error| error.location.as_str())
            .collect();
        assert_eq!(
            locations,
            [
                "event.event_type",
                "event.entity_revision",
                "event.causation"
            ]
        );
    }

    #[test]
    fn every_payload_builds_an_envelope_that_validates() {
        for payload in one_of_each() {
            let expected = payload.event_type();
            let envelope = DomainEventEnvelope::from_command(
                event_id(),
                command(),
                correlation(),
                payload,
                Timestamp::EPOCH,
                actor(),
            );
            envelope
                .validate()
                .unwrap_or_else(|errors| panic!("{expected} should be valid: {errors}"));

            let json = serde_json::to_value(&envelope).expect("serialises");
            let back: DomainEventEnvelope = serde_json::from_value(json).expect("deserialises");
            assert_eq!(back, envelope, "{expected} does not round-trip");
        }
    }
}
