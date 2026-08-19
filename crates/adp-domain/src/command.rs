//! The commands development work adds to the protocol's vocabulary of change.
//!
//! ADP adds four, and adds nothing else to the command side: everything generic — creating an
//! entity, updating ordinary fields, relating, archiving, superseding — is already
//! [`aep_domain::Command`] and stays there. What is here is the set of *development* transitions
//! whose conditions a field assignment cannot express (§42):
//!
//! ```text
//! adp.story.start/v1             a story moves into work, and somebody owns it
//! adp.story.complete/v1          a story is done, and this change is what did it
//! adp.test-plan.record/v1        a subject acquires the plan that will judge it
//! adp.specification.satisfy/v1   a specification is declared met, on this evidence
//! ```
//!
//! Each one names *what makes it true* — an assignee, a change, a plan, an evidence set — and
//! that reference is the thing an engine can check. `PATCH status = "complete"` cannot be
//! checked at all: the backend sees a field name and a value, and has no question left to ask.
//!
//! # Every one of them is revision-guarded
//!
//! [`Command::expected_revision`] returns a revision rather than an option, because
//! all four name a [`VersionedEntityRef`]. That is deliberate (§41): completing `story@4` is a
//! claim that 4 is still current, so a story that moved on while the work was in flight makes
//! the completion fail instead of landing on a story nobody read. There is no unguarded
//! development command to fall back to.
//!
//! # They ride the generic envelope
//!
//! [`aep_contract::CommandEnvelope`] is generic in its payload, so an ADP command travels through
//! the same boundary, with the same idempotency, provenance and audit, as an AEP one. Nothing in
//! this module needs a second command service.

use std::fmt;
use std::str::FromStr;

use aep_domain::capability::Capability;
use aep_domain::entity::{ActorRef, EntityRef, EntityRevision, VersionedEntityRef};
use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};

/// The versioned wire name of a development command.
///
/// Versioned for the same reason AEP's are (§36): the name is a published interface, so a backend
/// that speaks `adp.story.start/v1` keeps speaking it after a `v2` adds a field, and a client that
/// only knows `v1` is told so rather than having its payload reinterpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CommandKind {
    /// Move a story into work under a named owner.
    StartStory,
    /// Record that a story is done, and what did it.
    CompleteStory,
    /// Attach the test plan that will judge a subject.
    RecordTestPlan,
    /// Declare a specification satisfied, on named evidence.
    SatisfySpecification,
}

impl CommandKind {
    /// Every development command kind, in workflow order.
    ///
    /// The order is the order the work happens in — start, plan, satisfy, complete — so a
    /// generated vocabulary listing reads as the sequence somebody would actually perform.
    pub const ALL: &'static [Self] = &[
        Self::StartStory,
        Self::RecordTestPlan,
        Self::SatisfySpecification,
        Self::CompleteStory,
    ];

    /// The versioned name as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartStory => "adp.story.start/v1",
            Self::CompleteStory => "adp.story.complete/v1",
            Self::RecordTestPlan => "adp.test-plan.record/v1",
            Self::SatisfySpecification => "adp.specification.satisfy/v1",
        }
    }

    /// Parses a versioned command name.
    ///
    /// The rejection lists the whole ADP vocabulary. It deliberately does not fall back to AEP's:
    /// a caller that meant `aep.entity.update/v1` should be told it is looking in the development
    /// profile's command set, not handed a base-protocol command it did not ask this crate for.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                ParseError::reference(
                    "development command",
                    value,
                    format!(
                        "expected one of {}",
                        Self::ALL
                            .iter()
                            .map(|kind| kind.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })
    }
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CommandKind {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for CommandKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for CommandKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for CommandKind {
    /// Published as `DevelopmentCommandKind`, not `CommandKind`: one generated schema bundle can
    /// carry both vocabularies, and two definitions under the same name would collide.
    fn schema_name() -> String {
        "DevelopmentCommandKind".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            enum_values: Some(
                Self::ALL
                    .iter()
                    .map(|kind| serde_json::Value::String(kind.as_str().to_owned()))
                    .collect(),
            ),
            ..Default::default()
        };
        schema.metadata().description = Some(
            "The versioned name of a development command, such as `adp.story.start/v1`.".to_owned(),
        );
        schema.into()
    }
}

/// Moving a story into work under a named owner.
///
/// The assignee is part of the command rather than a later field update because "in progress with
/// nobody on it" is a state the audit trail should not be able to reach.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct StartStory {
    /// The exact revision of the story being picked up.
    pub story: VersionedEntityRef,
    /// Who is doing it — a person, an agent or a service, never an unattributed "someone".
    pub assignee: ActorRef,
}

/// Recording that a story is done, and what did it.
///
/// The change reference is what separates this from `status = "complete"`: it makes the
/// implementation addressable, so six months later "what actually shipped for this story?" is a
/// lookup rather than an archaeology exercise in a merge log.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CompleteStory {
    /// The exact revision of the story being completed.
    pub story: VersionedEntityRef,
    /// The recorded change that completed it, an [`adp.change/v1`](crate::body::ChangeSet).
    pub change: EntityRef,
}

/// Attaching the test plan that will judge a subject.
///
/// Recorded against the subject — the specification, criteria or story the plan tests — because
/// that is the entity whose state changes: it goes from "nothing decides whether this is done" to
/// "this does". The workflow guards `establish_verifiers → implement` on a test existing, and this
/// is the command that makes it so.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RecordTestPlan {
    /// The exact revision of the thing being planned for.
    pub subject: VersionedEntityRef,
    /// The plan, an [`adp.test-plan/v1`](crate::body::TestPlan).
    pub plan: EntityRef,
}

/// Declaring a specification satisfied, on named evidence.
///
/// `specification.satisfied` is a completion condition of every standard development profile, and
/// this is the only command that asserts it. The evidence list is the assertion's substance:
/// verification reports, test plans discharged, reviews. An engine can ask of each item whether it
/// exists, what it verified and whether it covers the revision named here — none of which can be
/// asked of a boolean.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SatisfySpecification {
    /// The exact revision being declared satisfied.
    pub specification: VersionedEntityRef,
    /// What establishes it. Never empty — see [`Command::validate`].
    pub evidence: Vec<EntityRef>,
}

/// A development state change.
///
/// The generic half of the vocabulary stays in [`aep_domain::Command`]; these are the transitions
/// whose conditions only make sense for construction work.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "command", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Command {
    /// Move a story into work.
    StartStory(StartStory),
    /// Record that a story is done.
    CompleteStory(CompleteStory),
    /// Attach the test plan that will judge a subject.
    RecordTestPlan(RecordTestPlan),
    /// Declare a specification satisfied.
    SatisfySpecification(SatisfySpecification),
}

impl Command {
    /// The versioned command type, for the envelope and for routing.
    pub fn kind(&self) -> CommandKind {
        match self {
            Self::StartStory(_) => CommandKind::StartStory,
            Self::CompleteStory(_) => CommandKind::CompleteStory,
            Self::RecordTestPlan(_) => CommandKind::RecordTestPlan,
            Self::SatisfySpecification(_) => CommandKind::SatisfySpecification,
        }
    }

    /// The entity this command mutates.
    ///
    /// Not an option, unlike [`aep_domain::Command::target`]: every development command changes
    /// something that already exists. Bringing an entity into being is `aep.entity.create/v1`, and
    /// that is the command with nothing to target.
    ///
    /// Note where the target is for [`Self::RecordTestPlan`]: the *subject*, not the plan. The
    /// plan is the evidence being attached; the subject is the entity whose state changes and
    /// whose authorisation therefore applies.
    pub fn target(&self) -> EntityRef {
        match self {
            Self::StartStory(StartStory { story, .. })
            | Self::CompleteStory(CompleteStory { story, .. }) => story.unversioned(),
            Self::RecordTestPlan(RecordTestPlan { subject, .. }) => subject.unversioned(),
            Self::SatisfySpecification(SatisfySpecification { specification, .. }) => {
                specification.unversioned()
            }
        }
    }

    /// The revision this command asserts the target is currently at.
    ///
    /// Always present, which is the guarantee: there is no development command that can land on a
    /// revision nobody looked at. No separate `expected_revision` field is accepted either — two
    /// sources for one concurrency assertion is one source too many (§41).
    pub fn expected_revision(&self) -> EntityRevision {
        match self {
            Self::StartStory(StartStory { story, .. })
            | Self::CompleteStory(CompleteStory { story, .. }) => story.revision,
            Self::RecordTestPlan(RecordTestPlan { subject, .. }) => subject.revision,
            Self::SatisfySpecification(SatisfySpecification { specification, .. }) => {
                specification.revision
            }
        }
    }

    /// The single capability this command requires.
    ///
    /// All four need `artifact.write`, and the one worth explaining is [`Self::RecordTestPlan`].
    /// It is tempting to read "test plan" as `tests.execute`, but recording a plan runs nothing —
    /// it writes an artifact and attaches it to a subject. `tests.execute` is the right to *run*
    /// the suite, which is a separate grant a profile may withhold while still allowing planning.
    /// There is no `test-plan.record` capability either: neither `aep/1` nor `adp/1` declares one,
    /// and a command may not require a capability the protocol does not have — a profile could
    /// never grant it, so the command could never be authorised.
    pub fn required_capability(&self) -> Capability {
        match self {
            Self::StartStory(_)
            | Self::CompleteStory(_)
            | Self::RecordTestPlan(_)
            | Self::SatisfySpecification(_) => Capability::ArtifactWrite,
        }
    }

    /// `true` for every development command, because every one of them writes.
    ///
    /// Exhaustive rather than a bare `true` so a future read-only command has to answer the
    /// question here instead of inheriting an answer.
    pub fn is_mutating(&self) -> bool {
        match self {
            Self::StartStory(_)
            | Self::CompleteStory(_)
            | Self::RecordTestPlan(_)
            | Self::SatisfySpecification(_) => true,
        }
    }

    /// A one-line description for audit records and explanations.
    pub fn summary(&self) -> String {
        match self {
            Self::StartStory(StartStory { story, assignee }) => {
                format!("start {story} as {assignee}")
            }
            Self::CompleteStory(CompleteStory { story, change }) => {
                format!("complete {story} with change {change}")
            }
            Self::RecordTestPlan(RecordTestPlan { subject, plan }) => {
                format!("record test plan {plan} for {subject}")
            }
            Self::SatisfySpecification(SatisfySpecification {
                specification,
                evidence,
            }) => format!(
                "satisfy {specification} on {}",
                evidence
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// Checks what can be checked without a backend.
    ///
    /// Two refusals, and both are self-contradictions rather than policy:
    ///
    /// * a story completed by *itself* — the change reference points at the story, so the record
    ///   of what was done is the thing that was asked for, and the trail is a loop;
    /// * a specification satisfied by **nothing**. This is the claim the protocol exists to
    ///   refuse. An empty evidence list is not a weaker assertion than a full one, it is the same
    ///   assertion with nothing behind it, and accepting it makes `specification.satisfied`
    ///   unfalsifiable — every profile's completion condition would then be satisfiable by
    ///   asserting it.
    ///
    /// Everything else worth asking — does the change exist, does the evidence cover this
    /// revision, does the workflow permit the transition — needs stored state and belongs to the
    /// layer that has it. Validation accumulates, as everywhere else.
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        match self {
            Self::CompleteStory(CompleteStory { story, change }) => {
                if &story.unversioned() == change {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::SelfReference,
                            "command.complete-story.change",
                            format!("{story} cannot be the change that completes it"),
                        )
                        .with_hint(
                            "the change is the record of what was implemented; a story pointing \
                             at itself says only that it happened because it happened",
                        ),
                    );
                }
            }
            Self::SatisfySpecification(SatisfySpecification {
                specification,
                evidence,
            }) => {
                if evidence.is_empty() {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::EmptyChange,
                            "command.satisfy-specification.evidence",
                            format!("{specification} cannot be satisfied by an empty evidence set"),
                        )
                        .with_hint(
                            "name the verification reports, discharged test plans or reviews that \
                             establish it; a specification satisfied by nothing is an assertion, \
                             not a result",
                        ),
                    );
                }
            }
            Self::StartStory(_) | Self::RecordTestPlan(_) => {}
        }
        errors.into_result(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use aep_domain::entity::EntityId;

    use super::*;

    /// The story under discussion throughout these tests.
    const STORY: &str = "01K2R8JD3ZJME72AJGQY67E5F8";
    /// The specification it implements.
    const SPECIFICATION: &str = "01K2R8JD3ZJME72AJGQY67E5G9";
    /// The change that implemented it.
    const CHANGE: &str = "01K2R8JD3ZJME72AJGQY67E5H0";
    /// The test plan.
    const PLAN: &str = "01K2R8JD3ZJME72AJGQY67E5J1";
    /// A verification report.
    const REPORT: &str = "01K2R8JD3ZJME72AJGQY67E5K2";

    fn reference(id: &str) -> EntityRef {
        EntityRef::new(EntityId::new(id).expect("test entity ids are well formed"))
    }

    fn at(id: &str, revision: u64) -> VersionedEntityRef {
        reference(id).at(EntityRevision::new(revision).expect("test revisions are non-zero"))
    }

    fn actor() -> ActorRef {
        ActorRef::parse("agent:implementation-agent-7").expect("a valid actor")
    }

    /// One command of every kind, so a new variant makes the coverage test fail.
    fn samples() -> Vec<Command> {
        vec![
            Command::StartStory(StartStory {
                story: at(STORY, 3),
                assignee: actor(),
            }),
            Command::CompleteStory(CompleteStory {
                story: at(STORY, 3),
                change: reference(CHANGE),
            }),
            Command::RecordTestPlan(RecordTestPlan {
                subject: at(SPECIFICATION, 3),
                plan: reference(PLAN),
            }),
            Command::SatisfySpecification(SatisfySpecification {
                specification: at(SPECIFICATION, 3),
                evidence: vec![reference(REPORT), reference(PLAN)],
            }),
        ]
    }

    #[test]
    fn the_sample_set_covers_every_development_command_kind() {
        let covered: BTreeSet<CommandKind> = samples().iter().map(Command::kind).collect();
        assert_eq!(
            covered.len(),
            CommandKind::ALL.len(),
            "the samples miss a command kind: {covered:?}"
        );
    }

    #[test]
    fn wire_names_are_the_versioned_adp_names() {
        assert_eq!(CommandKind::StartStory.as_str(), "adp.story.start/v1");
        assert_eq!(CommandKind::CompleteStory.as_str(), "adp.story.complete/v1");
        assert_eq!(
            CommandKind::RecordTestPlan.as_str(),
            "adp.test-plan.record/v1"
        );
        assert_eq!(
            CommandKind::SatisfySpecification.as_str(),
            "adp.specification.satisfy/v1"
        );
    }

    #[test]
    fn every_wire_name_round_trips_through_parsing_and_serde() {
        for kind in CommandKind::ALL {
            let name = kind.as_str();
            assert_eq!(CommandKind::parse(name).expect(name), *kind);
            assert_eq!(name.parse::<CommandKind>().expect(name), *kind);
            assert_eq!(kind.to_string(), name);

            let json = serde_json::to_string(kind).expect("a command kind serializes");
            assert_eq!(json, format!("\"{name}\""));
            let read: CommandKind =
                serde_json::from_str(&json).expect("a command kind deserializes");
            assert_eq!(read, *kind);
        }
    }

    #[test]
    fn a_base_protocol_command_is_not_a_development_command() {
        let error = CommandKind::parse("aep.entity.update/v1")
            .expect_err("the base vocabulary is a different set");
        let message = error.to_string();
        assert!(message.contains("aep.entity.update/v1"), "{message}");
        assert!(message.contains("adp.story.start/v1"), "{message}");
        assert!(
            message.contains("adp.specification.satisfy/v1"),
            "{message}"
        );
    }

    #[test]
    fn every_command_round_trips_through_json_under_its_own_tag() {
        for command in samples() {
            let json = serde_json::to_value(&command).expect("a command serializes");
            let tag = json
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("no `command` tag in {json}"))
                .to_owned();
            assert!(
                tag.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "the tag {tag:?} is not kebab-case"
            );
            let read: Command = serde_json::from_value(json.clone())
                .unwrap_or_else(|error| panic!("cannot read back {json}: {error}"));
            assert_eq!(read, command, "the round trip changed {json}");
        }
    }

    #[test]
    fn a_payload_field_the_protocol_does_not_define_is_rejected() {
        let json = serde_json::json!({
            "command": "start-story",
            "story": format!("{STORY}@3"),
            "assignee": "agent:implementation-agent-7",
            "priority": "high",
        });
        let error =
            serde_json::from_value::<Command>(json).expect_err("unknown fields are rejected");
        assert!(error.to_string().contains("priority"), "{error}");
    }

    #[test]
    fn every_development_command_pins_the_revision_it_asserts() {
        for command in samples() {
            assert_eq!(
                command.expected_revision(),
                EntityRevision::new(3).expect("a non-zero revision"),
                "`{}` must not be able to land on an unread revision",
                command.summary()
            );
        }
    }

    #[test]
    fn recording_a_test_plan_targets_the_subject_not_the_plan() {
        let record = Command::RecordTestPlan(RecordTestPlan {
            subject: at(SPECIFICATION, 3),
            plan: reference(PLAN),
        });
        assert_eq!(record.target(), reference(SPECIFICATION));
        assert_ne!(
            record.target(),
            reference(PLAN),
            "the plan is what is attached; the subject is what changes"
        );
    }

    #[test]
    fn completing_a_stale_story_is_a_different_command_from_completing_the_current_one() {
        let stale = Command::CompleteStory(CompleteStory {
            story: at(STORY, 3),
            change: reference(CHANGE),
        });
        let current = Command::CompleteStory(CompleteStory {
            story: at(STORY, 7),
            change: reference(CHANGE),
        });
        assert_ne!(stale, current);
        assert_eq!(
            stale.target(),
            current.target(),
            "the concurrency assertion lives in the revision, not in the target"
        );
    }

    #[test]
    fn every_development_command_needs_artifact_write() {
        for command in samples() {
            assert_eq!(
                command.required_capability(),
                Capability::ArtifactWrite,
                "wrong capability for `{}`",
                command.summary()
            );
        }
    }

    #[test]
    fn recording_a_test_plan_does_not_need_the_right_to_run_tests() {
        let record = Command::RecordTestPlan(RecordTestPlan {
            subject: at(SPECIFICATION, 3),
            plan: reference(PLAN),
        });
        // Writing down how something will be tested runs nothing. A profile that withholds
        // `tests.execute` must still be able to plan.
        assert_ne!(record.required_capability(), Capability::TestExecution);
        assert_eq!(record.required_capability(), Capability::ArtifactWrite);
    }

    #[test]
    fn every_development_command_mutates_state() {
        for command in samples() {
            assert!(
                command.is_mutating(),
                "`{}` is a command, so it changes state",
                command.summary()
            );
        }
    }

    #[test]
    fn a_summary_names_what_the_command_touched() {
        let complete = Command::CompleteStory(CompleteStory {
            story: at(STORY, 3),
            change: reference(CHANGE),
        });
        let summary = complete.summary();
        assert!(summary.contains(STORY), "{summary}");
        assert!(summary.contains(CHANGE), "{summary}");

        let satisfy = Command::SatisfySpecification(SatisfySpecification {
            specification: at(SPECIFICATION, 3),
            evidence: vec![reference(REPORT), reference(PLAN)],
        });
        let summary = satisfy.summary();
        assert!(summary.contains(REPORT), "{summary}");
        assert!(
            summary.contains(PLAN),
            "an audit reader needs every piece of evidence named: {summary}"
        );
    }

    #[test]
    fn a_well_formed_command_has_nothing_to_report() {
        for command in samples() {
            command.validate().unwrap_or_else(|errors| {
                panic!("`{}` should validate: {errors}", command.summary())
            });
        }
    }

    #[test]
    fn a_story_cannot_be_the_change_that_completed_it() {
        let command = Command::CompleteStory(CompleteStory {
            story: at(STORY, 3),
            change: reference(STORY),
        });
        let errors = command
            .validate()
            .expect_err("a story completed by itself records nothing");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::SelfReference);
        assert_eq!(error.location, "command.complete-story.change");
        assert!(error.message.contains(STORY), "{error}");
    }

    #[test]
    fn a_specification_cannot_be_satisfied_by_nothing() {
        let command = Command::SatisfySpecification(SatisfySpecification {
            specification: at(SPECIFICATION, 3),
            evidence: Vec::new(),
        });
        let errors = command
            .validate()
            .expect_err("this is the claim the protocol exists to refuse");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::EmptyChange);
        assert_eq!(error.location, "command.satisfy-specification.evidence");
        assert!(
            error
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("evidence") || hint.contains("verification")),
            "the hint should say what to name instead: {error}"
        );
    }

    #[test]
    fn a_story_completed_by_a_different_entity_is_accepted() {
        // The refusal above must catch the loop and nothing else: the ordinary case is a change
        // entity distinct from the story, and it has to keep working.
        let command = Command::CompleteStory(CompleteStory {
            story: at(STORY, 3),
            change: reference(CHANGE),
        });
        assert!(command.validate().is_ok());
    }
}
