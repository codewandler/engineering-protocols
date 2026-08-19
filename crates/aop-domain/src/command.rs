//! The five operations commands.
//!
//! Same shape as [`aep_domain::command::Command`], and deliberately so: AOP adds vocabulary to the
//! one mutation boundary rather than opening a second one. An operational change is a command, is
//! authorised by a capability, is audited, and is refused whole or applied whole, exactly as a
//! design approval is.
//!
//! # Why these five and not a status field
//!
//! `PATCH status = "mitigated"` is a field assignment, and a backend receiving one can check
//! nothing beyond the field's spelling (§42). [`MitigateIncident`] carries the action and says
//! whether it can be undone, so `reversible-changes` and `blast-radius-limitation` have something
//! to read. [`ResolveIncident`] carries the verification, so `verify-after-action` is checkable
//! rather than assumed. [`PromoteRelease`] carries the approval, so the record of *what authorised
//! a production deployment* sits on the deployment itself and not in a chat log.
//!
//! # What is checkable here and what is not
//!
//! [`Command::validate`] runs without a backend, so it sees only the command. It catches the three
//! contradictions a command can hold on its own; whether the verification actually verified this
//! incident, whether the approval is fresh, and whether the status ladder permits the move all need
//! stored state and belong to the layer that has it.

use std::fmt;
use std::str::FromStr;

use aep_domain::capability::{Capability, Environment};
use aep_domain::entity::{ActorRef, EntityRef, EntityRevision, VersionedEntityRef};
use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};

/// The versioned wire name of an operations command, such as `aop.incident.resolve/v1`.
///
/// Versioned for the same reason AEP's are (§36): a backend that implements
/// `aop.release.promote/v1` keeps implementing it after `v2` adds a field, and a client that only
/// speaks `v1` is told so rather than having its payload reinterpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CommandKind {
    /// Take responsibility for a live incident.
    AcknowledgeIncident,
    /// Record an action taken against production to stop the bleeding.
    MitigateIncident,
    /// Close an incident on the strength of a verification.
    ResolveIncident,
    /// Move a release into an environment.
    PromoteRelease,
    /// Return an environment to an earlier revision.
    RollbackRelease,
}

impl CommandKind {
    /// Every operations command kind, incidents before releases.
    pub const ALL: &'static [Self] = &[
        Self::AcknowledgeIncident,
        Self::MitigateIncident,
        Self::ResolveIncident,
        Self::PromoteRelease,
        Self::RollbackRelease,
    ];

    /// The versioned name as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AcknowledgeIncident => "aop.incident.acknowledge/v1",
            Self::MitigateIncident => "aop.incident.mitigate/v1",
            Self::ResolveIncident => "aop.incident.resolve/v1",
            Self::PromoteRelease => "aop.release.promote/v1",
            Self::RollbackRelease => "aop.release.rollback/v1",
        }
    }

    /// Parses a versioned command name.
    ///
    /// The rejection lists the whole operations vocabulary. It does not list AEP's: a caller who
    /// sent `aep.entity.update/v1` here has routed to the wrong vocabulary rather than misspelled
    /// one, and pasting nine unrelated names into the message would hide that.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                ParseError::reference(
                    "command",
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
    fn schema_name() -> String {
        "AopCommandKind".to_owned()
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
            "The versioned name of an operations command, such as `aop.incident.resolve/v1`."
                .to_owned(),
        );
        schema.into()
    }
}

/// Taking responsibility for a live incident.
///
/// Names a revision because acknowledgement is a claim about the incident as it currently reads: if
/// triage has rewritten the blast radius since the responder looked, the acknowledgement should
/// fail and be re-made against what is now there, not land silently on a different incident.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AcknowledgeIncident {
    /// The exact revision being acknowledged.
    pub incident: VersionedEntityRef,
    /// Who is taking it.
    pub responder: ActorRef,
}

/// Recording an action taken against production to stop the bleeding.
///
/// `reversible` is carried rather than inferred because the workflow cannot infer it. The `mitigate`
/// state in `workflows/incidents/standard.yaml` is marked `irreversible: false` on the strength of
/// the mitigations it is *meant* to reach for — a flag, a config revert, shedding load. Purging a
/// cache, rewriting corrupt rows or revoking credentials are irreversible steps wearing the same
/// label, and only the actor knows which one this is.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MitigateIncident {
    /// The exact revision being mitigated.
    pub incident: VersionedEntityRef,
    /// What was done, in one line, for the audit trail and the postmortem.
    pub action: String,
    /// Whether this action can be undone.
    pub reversible: bool,
}

/// Closing an incident on the strength of a recorded verification.
///
/// The verification reference is what separates this from `status = "resolved"`. It is the thing an
/// engine checks: that it exists, that it concerns this incident, and that it passed. The
/// `incident.standard` profile does not consider an incident finished until `recovery_verified` is
/// true, and this command names the record that makes it true.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ResolveIncident {
    /// The exact revision being resolved.
    pub incident: VersionedEntityRef,
    /// The verification that recovery held.
    pub verification: EntityRef,
}

/// Moving a release into an environment.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct PromoteRelease {
    /// The exact revision being promoted.
    pub release: VersionedEntityRef,
    /// Where it is going.
    pub to: Environment,
    /// The approval that authorised it, required for production.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<EntityRef>,
}

/// Returning an environment to an earlier revision.
///
/// The revision is named rather than left as "the previous one", because "the previous one" is a
/// question about state at the moment the rollback runs and the answer changes if a second
/// deployment has landed in between. Naming it makes the command say what it will produce.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RollbackRelease {
    /// The exact revision being rolled back.
    pub release: VersionedEntityRef,
    /// What to return to.
    pub to_revision: String,
}

/// An operational state-changing operation.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "command", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Command {
    /// Take responsibility for a live incident.
    AcknowledgeIncident(AcknowledgeIncident),
    /// Record an action taken against production.
    MitigateIncident(MitigateIncident),
    /// Close an incident on the strength of a verification.
    ResolveIncident(ResolveIncident),
    /// Move a release into an environment.
    PromoteRelease(PromoteRelease),
    /// Return an environment to an earlier revision.
    RollbackRelease(RollbackRelease),
}

impl Command {
    /// The versioned command type, for the envelope and for routing.
    pub fn kind(&self) -> CommandKind {
        match self {
            Self::AcknowledgeIncident(_) => CommandKind::AcknowledgeIncident,
            Self::MitigateIncident(_) => CommandKind::MitigateIncident,
            Self::ResolveIncident(_) => CommandKind::ResolveIncident,
            Self::PromoteRelease(_) => CommandKind::PromoteRelease,
            Self::RollbackRelease(_) => CommandKind::RollbackRelease,
        }
    }

    /// The entity this command mutates.
    ///
    /// Always present, unlike AEP's: every operations command acts on something that already
    /// exists. An incident is created by `aep.entity.create/v1` before anybody can acknowledge it.
    pub fn target(&self) -> EntityRef {
        match self {
            Self::AcknowledgeIncident(AcknowledgeIncident { incident, .. })
            | Self::MitigateIncident(MitigateIncident { incident, .. })
            | Self::ResolveIncident(ResolveIncident { incident, .. }) => incident.unversioned(),
            Self::PromoteRelease(PromoteRelease { release, .. })
            | Self::RollbackRelease(RollbackRelease { release, .. }) => release.unversioned(),
        }
    }

    /// The revision this command asserts the target is currently at.
    ///
    /// Every one of them asserts one, because every one of them is a decision taken by looking at
    /// the entity (§41). Resolving `incident@4` is a claim that 4 is still current; if triage has
    /// since raised the severity, the resolution must fail rather than close an incident whose
    /// current text nobody read.
    pub fn expected_revision(&self) -> EntityRevision {
        match self {
            Self::AcknowledgeIncident(AcknowledgeIncident { incident, .. })
            | Self::MitigateIncident(MitigateIncident { incident, .. })
            | Self::ResolveIncident(ResolveIncident { incident, .. }) => incident.revision,
            Self::PromoteRelease(PromoteRelease { release, .. })
            | Self::RollbackRelease(RollbackRelease { release, .. }) => release.revision,
        }
    }

    /// The single capability this command requires.
    ///
    /// # Which incident commands write production
    ///
    /// Only [`Command::MitigateIncident`] does. It is the one command here that reaches into the
    /// running system — a flag flipped, a config reverted, load shed — so it requires
    /// `production.write`, which `incident.standard` puts behind `require_approval`. That gate
    /// firing on exactly this command and no other is the design: the profile grants an agent full
    /// sight of production and no ability to change it unattended.
    ///
    /// [`Command::AcknowledgeIncident`] and [`Command::ResolveIncident`] change only the incident
    /// record, and take `production.read`. Not "nothing": both make a claim *about* production —
    /// "I am on this", "this is over" — and an actor who cannot observe production cannot honestly
    /// make either. Requiring the read capability keeps a caller that has been denied all sight of
    /// production from declaring an incident resolved.
    ///
    /// # Why the release commands are environment-scoped
    ///
    /// `deployment.create` and `deployment.rollback` take an environment, and an unscoped grant
    /// means *every* environment, production included. [`Command::PromoteRelease`] therefore
    /// requires the capability scoped to the environment it names, so promoting to staging is
    /// satisfied by `release.progressive`'s outright `deployment.create:staging` grant while
    /// promoting to production hits the same profile's `require_approval` entry.
    ///
    /// [`Command::RollbackRelease`] requires `deployment.rollback:production` specifically. It
    /// carries no environment of its own, and hard-coding the wildcard would be worse than
    /// hard-coding production: a required `deployment.rollback` with no environment is satisfied
    /// *only* by a wildcard grant, so a profile that carefully granted
    /// `deployment.rollback:production` would find every rollback refused. Production is the honest
    /// scope here — both rollback routes in `workflows/releases/progressive.yaml` are declared at
    /// `observe` and `promote`, and both of those states carry live traffic.
    pub fn required_capability(&self) -> Capability {
        match self {
            Self::AcknowledgeIncident(_) | Self::ResolveIncident(_) => Capability::ProductionRead,
            Self::MitigateIncident(_) => Capability::ProductionWrite,
            Self::PromoteRelease(PromoteRelease { to, .. }) => Capability::Deploy(to.clone()),
            Self::RollbackRelease(_) => Capability::Rollback(Environment::Production),
        }
    }

    /// A one-line description for audit records and explanations.
    pub fn summary(&self) -> String {
        match self {
            Self::AcknowledgeIncident(AcknowledgeIncident {
                incident,
                responder,
            }) => format!("acknowledge incident {incident} as {responder}"),
            Self::MitigateIncident(MitigateIncident {
                incident,
                action,
                reversible,
            }) => {
                // The reversibility goes in the summary because it is the fact a reader scanning an
                // incident's audit trail most needs and would otherwise have to open the payload for.
                let undo = if *reversible {
                    "reversible"
                } else {
                    "irreversible"
                };
                format!("mitigate incident {incident} ({undo}): {action}")
            }
            Self::ResolveIncident(ResolveIncident {
                incident,
                verification,
            }) => format!("resolve incident {incident} on verification {verification}"),
            Self::PromoteRelease(PromoteRelease {
                release,
                to,
                approval,
            }) => match approval {
                Some(approval) => {
                    format!("promote release {release} to {to} on approval {approval}")
                }
                None => format!("promote release {release} to {to}"),
            },
            Self::RollbackRelease(RollbackRelease {
                release,
                to_revision,
            }) => format!("roll release {release} back to revision {to_revision}"),
        }
    }

    /// Checks what can be checked without a backend.
    ///
    /// Three refusals, and each is the case where the command contradicts what it claims to be.
    /// Validation accumulates, as everywhere else in the workspace.
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        match self {
            Self::PromoteRelease(PromoteRelease { to, approval, .. }) => {
                if to.is_production() && approval.is_none() {
                    // Two defences, both wanted, and they answer different questions.
                    //
                    // The protocol's approval floor decides what an *actor may hold*: it refuses to
                    // grant `deployment.create:production` outright, so the capability arrives only
                    // behind `require_approval`. This check decides what a *command may assert*: a
                    // promotion to production that names no approval is refused at the edge even
                    // when the actor's policy would have let it through, because the approval is
                    // also the audit record — without it the trail says a production deployment
                    // happened and cannot say on whose decision.
                    //
                    // Dropping either one leaves a hole. Policy alone lets an approved actor
                    // promote without recording which approval covered this promotion; this check
                    // alone lets any actor promote as long as it cites something.
                    errors.push(
                        ValidationError::new(
                            ValidationCode::ProductionWriteWithoutApproval,
                            "command.promote-release.approval",
                            "a promotion to production must name the approval that authorised it",
                        )
                        .with_hint(
                            "the capability policy decides whether this actor may deploy to \
                             production; the approval reference is what tells a later reader which \
                             decision this deployment rested on",
                        ),
                    );
                }
            }
            Self::MitigateIncident(MitigateIncident { action, .. }) => {
                if action.trim().is_empty() {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::EmptyChange,
                            "command.mitigate-incident.action",
                            "a mitigation must say what was done",
                        )
                        .with_hint(
                            "the action is the only record of what changed in production during \
                             the incident; an empty one leaves the postmortem reconstructing it \
                             from deployment logs",
                        ),
                    );
                }
            }
            Self::RollbackRelease(RollbackRelease { to_revision, .. }) => {
                if to_revision.trim().is_empty() {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::EmptyChange,
                            "command.rollback-release.to_revision",
                            "a rollback must name the revision it returns to",
                        )
                        .with_hint(
                            "`deployment.previous_revision.exists` is a precondition on both \
                             rollback routes; a rollback with no target cannot satisfy it and \
                             cannot be replayed",
                        ),
                    );
                }
            }
            Self::AcknowledgeIncident(_) | Self::ResolveIncident(_) => {}
        }
        errors.into_result(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use aep_domain::entity::EntityId;

    /// The incident under discussion throughout these tests.
    const INCIDENT: &str = "incident-checkout-0001";
    /// The release under discussion throughout these tests.
    const RELEASE: &str = "release-checkout-0042";

    fn reference(id: &str) -> EntityRef {
        EntityRef::new(EntityId::new(id).expect("test entity ids are well formed"))
    }

    fn revision(value: u64) -> EntityRevision {
        EntityRevision::new(value).expect("test revisions are non-zero")
    }

    fn incident_at(value: u64) -> VersionedEntityRef {
        reference(INCIDENT).at(revision(value))
    }

    fn release_at(value: u64) -> VersionedEntityRef {
        reference(RELEASE).at(revision(value))
    }

    /// One command of every kind, so a new variant makes the coverage test fail.
    fn samples() -> Vec<Command> {
        vec![
            Command::AcknowledgeIncident(AcknowledgeIncident {
                incident: incident_at(4),
                responder: ActorRef::parse("human:bea").expect("a valid actor"),
            }),
            Command::MitigateIncident(MitigateIncident {
                incident: incident_at(4),
                action: "disabled the wallet-checkout flag".to_owned(),
                reversible: true,
            }),
            Command::ResolveIncident(ResolveIncident {
                incident: incident_at(4),
                verification: reference("verification-recov-01"),
            }),
            Command::PromoteRelease(PromoteRelease {
                release: release_at(4),
                to: Environment::Production,
                approval: Some(reference("approval-release-042")),
            }),
            Command::RollbackRelease(RollbackRelease {
                release: release_at(4),
                to_revision: "3ad77e0".to_owned(),
            }),
        ]
    }

    #[test]
    fn the_sample_set_covers_every_command_kind() {
        let covered: BTreeSet<CommandKind> = samples().iter().map(Command::kind).collect();
        assert_eq!(
            covered.len(),
            CommandKind::ALL.len(),
            "the samples miss a command kind: {covered:?}"
        );
    }

    #[test]
    fn wire_names_are_the_versioned_names_the_specification_lists() {
        assert_eq!(
            CommandKind::AcknowledgeIncident.as_str(),
            "aop.incident.acknowledge/v1"
        );
        assert_eq!(
            CommandKind::MitigateIncident.as_str(),
            "aop.incident.mitigate/v1"
        );
        assert_eq!(
            CommandKind::ResolveIncident.as_str(),
            "aop.incident.resolve/v1"
        );
        assert_eq!(
            CommandKind::PromoteRelease.as_str(),
            "aop.release.promote/v1"
        );
        assert_eq!(
            CommandKind::RollbackRelease.as_str(),
            "aop.release.rollback/v1"
        );
    }

    #[test]
    fn every_wire_name_round_trips_through_parsing_and_serde() {
        for kind in CommandKind::ALL {
            let name = kind.as_str();
            assert!(
                name.starts_with("aop."),
                "{name} is not in the operations namespace"
            );
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
    fn a_command_from_another_vocabulary_is_rejected_and_this_one_named() {
        let error = CommandKind::parse("aep.design.approve/v1")
            .expect_err("an AEP command is not an AOP command");
        let message = error.to_string();
        assert!(message.contains("aep.design.approve/v1"), "{message}");
        assert!(message.contains("aop.incident.acknowledge/v1"), "{message}");
        assert!(message.contains("aop.release.rollback/v1"), "{message}");
    }

    #[test]
    fn there_is_no_command_that_closes_an_incident_without_naming_a_verification() {
        let error = CommandKind::parse("aop.incident.close/v1")
            .expect_err("closing an incident is spelled `resolve`, and it carries evidence");
        assert!(
            error.to_string().contains("aop.incident.resolve/v1"),
            "the rejection should point at the command that does exist: {error}"
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
            "command": "mitigate-incident",
            "incident": format!("{INCIDENT}@4"),
            "action": "restarted the pod",
            "reversible": false,
            "urgency": "high",
        });
        let error =
            serde_json::from_value::<Command>(json).expect_err("unknown fields are rejected");
        assert!(error.to_string().contains("urgency"), "{error}");
    }

    #[test]
    fn every_command_pins_the_revision_it_names_and_targets_the_entity_behind_it() {
        for command in samples() {
            assert_eq!(
                command.expected_revision(),
                revision(4),
                "`{}` should assert the revision it names",
                command.summary()
            );
            let target = command.target().to_string();
            assert!(
                target == INCIDENT || target == RELEASE,
                "`{}` targets {target}, which is neither the incident nor the release",
                command.summary()
            );
            assert!(
                !target.contains('@'),
                "the target is the entity; the revision is the separate assertion"
            );
        }
    }

    #[test]
    fn acknowledging_and_resolving_an_incident_need_only_the_right_to_observe_production() {
        for command in samples() {
            if matches!(
                command.kind(),
                CommandKind::AcknowledgeIncident | CommandKind::ResolveIncident
            ) {
                assert_eq!(
                    command.required_capability(),
                    Capability::ProductionRead,
                    "`{}` changes the incident record, not production",
                    command.summary()
                );
                assert!(
                    !command.required_capability().mutates_production(),
                    "`{}` must not trip the production-write gate",
                    command.summary()
                );
            }
        }
    }

    #[test]
    fn mitigating_is_the_one_incident_command_that_writes_production() {
        let mitigate = Command::MitigateIncident(MitigateIncident {
            incident: incident_at(4),
            action: "shed 20% of traffic at the edge".to_owned(),
            reversible: true,
        });
        assert_eq!(
            mitigate.required_capability(),
            Capability::ProductionWrite,
            "this is the command that reaches into the running system"
        );
        assert!(
            mitigate.required_capability().mutates_production(),
            "`incident.standard` puts production.write behind an approval; this is the command \
             that has to meet it"
        );
    }

    #[test]
    fn promoting_needs_deployment_create_scoped_to_the_environment_it_names() {
        let to_staging = Command::PromoteRelease(PromoteRelease {
            release: release_at(4),
            to: Environment::Staging,
            approval: None,
        });
        assert_eq!(
            to_staging.required_capability(),
            Capability::Deploy(Environment::Staging)
        );
        assert_eq!(
            to_staging.required_capability().to_string(),
            "deployment.create:staging",
            "the scope is what `release.progressive` grants outright"
        );

        let to_production = Command::PromoteRelease(PromoteRelease {
            release: release_at(4),
            to: Environment::Production,
            approval: Some(reference("approval-release-042")),
        });
        assert_eq!(
            to_production.required_capability(),
            Capability::Deploy(Environment::Production)
        );
        assert!(
            to_production.required_capability().mutates_production(),
            "a production promotion must be recognised as a production mutation"
        );
    }

    #[test]
    fn rolling_back_needs_deployment_rollback_in_production() {
        let rollback = Command::RollbackRelease(RollbackRelease {
            release: release_at(4),
            to_revision: "3ad77e0".to_owned(),
        });
        assert_eq!(
            rollback.required_capability(),
            Capability::Rollback(Environment::Production)
        );
        assert_eq!(
            rollback.required_capability().to_string(),
            "deployment.rollback:production",
            "this is the entry `release.progressive` puts behind an approval"
        );
        assert_ne!(
            rollback.required_capability(),
            Capability::Rollback(Environment::Any),
            "an unscoped requirement would be satisfiable only by a wildcard grant, which is the \
             grant the approval floor exists to prevent"
        );
    }

    #[test]
    fn every_required_capability_is_one_the_operations_protocol_declares() {
        // `protocols/aop/1.yaml` extends `aep/1` and adds no capabilities of its own, so every
        // capability named here has to parse out of AEP's vocabulary.
        for command in samples() {
            let rendered = command.required_capability().to_string();
            let parsed = Capability::parse(&rendered).unwrap_or_else(|error| {
                panic!("`{rendered}` is not a declared capability: {error}")
            });
            assert_eq!(parsed, command.required_capability());
        }
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
    fn a_promotion_to_production_without_an_approval_is_refused() {
        let command = Command::PromoteRelease(PromoteRelease {
            release: release_at(4),
            to: Environment::Production,
            approval: None,
        });
        let errors = command
            .validate()
            .expect_err("an unapproved production promotion is refused");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::ProductionWriteWithoutApproval);
        assert_eq!(error.location, "command.promote-release.approval");
        assert!(error.message.contains("approval"), "{error}");
        assert!(
            error
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("capability policy")),
            "the hint should say that this is the second of two defences, not the only one: {error}"
        );
    }

    #[test]
    fn a_promotion_to_staging_needs_no_approval() {
        let command = Command::PromoteRelease(PromoteRelease {
            release: release_at(4),
            to: Environment::Staging,
            approval: None,
        });
        command
            .validate()
            .expect("staging is where a broken deployment costs nothing but time");
    }

    #[test]
    fn a_mitigation_that_does_not_say_what_was_done_is_refused() {
        for action in ["", "   "] {
            let command = Command::MitigateIncident(MitigateIncident {
                incident: incident_at(4),
                action: action.to_owned(),
                reversible: false,
            });
            let errors = command
                .validate()
                .expect_err("an empty mitigation is refused");
            assert_eq!(errors.len(), 1);
            let error = &errors.as_slice()[0];
            assert_eq!(error.code, ValidationCode::EmptyChange);
            assert_eq!(error.location, "command.mitigate-incident.action");
            assert!(error.message.contains("what was done"), "{error}");
        }
    }

    #[test]
    fn a_rollback_that_names_no_revision_is_refused() {
        let command = Command::RollbackRelease(RollbackRelease {
            release: release_at(4),
            to_revision: String::new(),
        });
        let errors = command
            .validate()
            .expect_err("an empty rollback is refused");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::EmptyChange);
        assert_eq!(error.location, "command.rollback-release.to_revision");
        assert!(
            error
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("previous_revision")),
            "the hint should name the precondition this would fail: {error}"
        );
    }

    #[test]
    fn a_summary_names_what_the_command_touched() {
        let mitigate = Command::MitigateIncident(MitigateIncident {
            incident: incident_at(4),
            action: "purged the session cache".to_owned(),
            reversible: false,
        });
        let summary = mitigate.summary();
        assert!(summary.contains(INCIDENT), "{summary}");
        assert!(summary.contains("purged the session cache"), "{summary}");
        assert!(
            summary.contains("irreversible"),
            "an audit reader scanning summaries must see that this one cannot be undone: {summary}"
        );

        let promote = Command::PromoteRelease(PromoteRelease {
            release: release_at(4),
            to: Environment::Production,
            approval: Some(reference("approval-release-042")),
        });
        assert_eq!(
            promote.summary(),
            format!("promote release {RELEASE}@4 to production on approval approval-release-042")
        );
    }
}
