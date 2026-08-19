//! Turning an artifact manifest into entities and relations.
//!
//! A manifest is the human-facing spelling of the graph the interaction contract stores:
//! `design:passkeys-auth` with `{designs: spec:passkeys-auth}` is an entity of type
//! `aep.design/v1` with a `designs` relation to another entity. This module converts one into the
//! other **through [`CommandService`]**, never by writing to the store, and that is the whole point
//! of it:
//!
//! * a manifest that seeds is a manifest that is expressible through the contract, which is a
//!   claim worth checking rather than assuming;
//! * everything it creates gets history, events and audit records exactly like anything else, so
//!   `protocol entity history` and `protocol audit` have something real to show.
//!
//! Command ids and idempotency keys are derived from the artifact id, so seeding the same manifest
//! into the same backend twice is a replay rather than a second set of entities.
//!
//! # Two passes
//!
//! Entities first, relations second. A manifest may declare an edge to an artifact that appears
//! later in the file — `design:passkeys-auth` sorts before `spec:passkeys-auth` — and a
//! single-pass seeder would have to either drop that edge or reorder the manifest behind the
//! reader's back.

use std::collections::BTreeMap;

use aep_contract::command::{CommandContext, CommandEnvelope, CommandService};
use aep_contract::error::CommandError;
use aep_contract::testing::block_on;
use aep_domain::artifact::{Artifact, ArtifactGraph, ArtifactId};
use aep_domain::command::{Command, CreateEntity, CreateRelation};
use aep_domain::entity::{ActorRef, EntityId, EntityLocator, EntityRef};
use aep_domain::error::ParseError;
use aep_domain::ids::{CommandId, CorrelationId, IdempotencyKey, RequestId};
use aep_domain::node::Node;
use aep_domain::time::Timestamp;

use crate::command::STATUS_KEY;
use crate::MemoryBackend;

/// The activity every seeding command belongs to.
///
/// Fixed rather than generated: `protocol audit --correlation seed-manifest` is how a reader asks
/// for "everything this run did", and a correlation id nobody can predict cannot be typed.
pub const SEED_CORRELATION: &str = "seed-manifest";

/// What one seeding run produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeedReport {
    /// How many entities the manifest maps to.
    pub entities: usize,
    /// How many relations were issued between them.
    pub relations: usize,
    /// The identity each artifact was stored under, so a caller can map its manifest back.
    pub by_id: BTreeMap<ArtifactId, EntityId>,
}

/// Seeds `backend` with the artifacts in `graph`, addressed under `organisation`/`space`.
///
/// Every artifact becomes an entity of its kind's entity type, at
/// `ep://<organisation>/<space>/<kind>/<name>`; every artifact relation becomes a relation command
/// once all the entities exist.
///
/// Fails — rather than dropping an edge — when a relation points at an artifact the manifest does
/// not declare. A dangling edge means the manifest is describing a graph it does not contain, and
/// seeding a partial graph would make the missing artifact look like a deliberate absence.
pub fn from_manifest(
    backend: &MemoryBackend,
    graph: &ArtifactGraph,
    organisation: &str,
    space: &str,
    at: Timestamp,
    actor: &ActorRef,
) -> Result<SeedReport, CommandError> {
    let mut report = SeedReport::default();

    for artifact in graph.artifacts() {
        let id = create_entity(backend, artifact, organisation, space, at, actor)?;
        report.by_id.insert(artifact.id.clone(), id);
        report.entities += 1;
    }

    for artifact in graph.artifacts() {
        let source = report
            .by_id
            .get(&artifact.id)
            .expect("the first pass stored every artifact in the manifest")
            .clone();
        for relation in &artifact.relations {
            let target_id = relation.target.id();
            let target =
                report
                    .by_id
                    .get(target_id)
                    .cloned()
                    .ok_or_else(|| CommandError::Conflict {
                        reason: format!(
                            "`{}` declares `{} {target_id}`, but the manifest does not declare \
                             `{target_id}`",
                            artifact.id, relation.kind
                        ),
                    })?;
            let payload = Command::CreateRelation(CreateRelation {
                kind: relation.kind,
                source: EntityRef::new(source.clone()),
                target: EntityRef::new(target),
            });
            let name = format!(
                "seed-rel-{}-{}-{}",
                token(&artifact.id),
                relation.kind.as_str(),
                token(target_id)
            );
            block_on(backend.execute(envelope(&name, payload, at, actor)?))?;
            report.relations += 1;
        }
    }

    Ok(report)
}

/// Issues the `CreateEntity` command for one artifact and returns the identity it was stored under.
fn create_entity(
    backend: &MemoryBackend,
    artifact: &Artifact,
    organisation: &str,
    space: &str,
    at: Timestamp,
    actor: &ActorRef,
) -> Result<EntityId, CommandError> {
    let locator = EntityLocator::new(
        organisation,
        space,
        artifact.kind.as_str(),
        locator_key(artifact.id.name()),
    )
    .map_err(|error| CommandError::Conflict {
        reason: format!("`{}` cannot be given a locator: {error}", artifact.id),
    })?;

    let payload = Command::CreateEntity(CreateEntity {
        entity_type: artifact.kind.entity_type(),
        locator,
        data: body(artifact),
    });
    let name = format!("seed-{}", token(&artifact.id));
    let result = block_on(backend.execute(envelope(&name, payload, at, actor)?))?;

    result
        .affected
        .first()
        .map(|reference| reference.id.clone())
        .ok_or_else(|| CommandError::Conflict {
            reason: format!("creating `{}` reported no entity", artifact.id),
        })
}

/// The body an artifact is stored with.
///
/// Only what the manifest actually states: a title key holding an empty string would be a claim
/// the manifest never made.
fn body(artifact: &Artifact) -> Node {
    let mut fields: BTreeMap<String, Node> = BTreeMap::new();
    fields.insert(STATUS_KEY.to_owned(), Node::from(artifact.status.as_str()));
    for (key, value) in [
        ("title", artifact.metadata.title.as_deref()),
        ("summary", artifact.metadata.summary.as_deref()),
        ("owner", artifact.metadata.owner.as_deref()),
    ] {
        if let Some(value) = value {
            fields.insert(key.to_owned(), Node::from(value));
        }
    }
    if let Some(version) = &artifact.version {
        fields.insert("version".to_owned(), Node::from(version.as_str()));
    }
    Node::Map(fields)
}

/// Wraps a seeding command, deriving its identifiers from `name` so a replay is recognisable.
fn envelope(
    name: &str,
    payload: Command,
    at: Timestamp,
    actor: &ActorRef,
) -> Result<CommandEnvelope<Command>, CommandError> {
    let command_id = CommandId::new(name).map_err(|error| unusable(name, &error))?;
    let idempotency_key = IdempotencyKey::new(name).map_err(|error| unusable(name, &error))?;
    let request_id =
        RequestId::new(format!("req-{name}")).map_err(|error| unusable(name, &error))?;
    let context = CommandContext::new(
        request_id,
        idempotency_key,
        actor.clone(),
        CorrelationId::new(SEED_CORRELATION).expect("a built-in correlation id is well formed"),
        at,
    );

    let command_type = payload.kind().as_str().to_owned();
    let target = payload.target();
    let expected_revision = payload.expected_revision();
    let mut envelope = CommandEnvelope::new(command_id, command_type, payload, context);
    envelope.target = target;
    envelope.expected_revision = expected_revision;
    Ok(envelope)
}

/// Reports an artifact id that cannot be turned into a command identifier.
fn unusable(name: &str, error: &ParseError) -> CommandError {
    CommandError::Conflict {
        reason: format!("`{name}` is not a usable command identifier: {error}"),
    }
}

/// A command-identifier fragment derived from an artifact id, such as `design-passkeys-auth`.
///
/// Deterministic, because that is what makes a second seeding run a replay: the same artifact id
/// must produce the same command id every time, in any process.
fn token(id: &ArtifactId) -> String {
    let mut token = String::new();
    for character in id.to_string().chars() {
        if character.is_ascii_alphanumeric() {
            token.push(character);
        } else if !token.is_empty() && !token.ends_with('-') {
            // Identifiers reject leading, trailing and repeated separators, so runs collapse.
            token.push('-');
        }
    }
    let trimmed = token.trim_end_matches('-');
    if trimmed.is_empty() {
        "artifact".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The locator key an artifact name maps to.
///
/// Artifact names allow `/`, locator segments do not, so anything a locator cannot carry becomes
/// `-`. Two artifacts that collide here are refused by the backend as a duplicate address rather
/// than silently sharing one entity.
fn locator_key(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use aep_contract::consistency::QueryConsistency;
    use aep_contract::query::{QueryService, RelationQuery};
    use aep_domain::artifact::{
        ArtifactKind, ArtifactLocation, ArtifactMetadata, ArtifactRef, ArtifactStatus,
        ArtifactVersion, RelationKind,
    };

    use super::*;

    /// The actor every test seeds as.
    fn actor() -> ActorRef {
        "service:protocol-cli".parse().expect("actor")
    }

    /// An artifact at an inline location, which keeps the fixtures about the graph.
    fn artifact(id: &str, kind: ArtifactKind, status: ArtifactStatus) -> Artifact {
        Artifact::new(
            id.parse().expect("artifact id"),
            kind,
            status,
            ArtifactLocation::Inline,
        )
    }

    /// The shape of `examples/development-passkeys/artifacts.yaml`: six artifacts, five edges.
    fn example() -> ArtifactGraph {
        let mut prd = artifact(
            "prd:passkeys",
            ArtifactKind::ProductRequirements,
            ArtifactStatus::Active,
        );
        prd.metadata = ArtifactMetadata {
            title: Some("Passwordless sign-in".to_owned()),
            ..ArtifactMetadata::default()
        };

        let story = artifact(
            "story:AUTH-141",
            ArtifactKind::Story,
            ArtifactStatus::Active,
        )
        .with_relation(
            RelationKind::DerivedFrom,
            ArtifactRef::unpinned("prd:passkeys".parse().expect("artifact id")),
        );

        let specification = artifact(
            "spec:passkeys-auth",
            ArtifactKind::Specification,
            ArtifactStatus::Approved,
        )
        .with_relation(
            RelationKind::Specifies,
            ArtifactRef::unpinned("story:AUTH-141".parse().expect("artifact id")),
        );

        let mut design = artifact(
            "design:passkeys-auth",
            ArtifactKind::Design,
            ArtifactStatus::Approved,
        )
        .with_relation(
            RelationKind::Designs,
            ArtifactRef::unpinned("spec:passkeys-auth".parse().expect("artifact id")),
        );
        design.version = Some(ArtifactVersion::new("7"));

        let adr = artifact(
            "adr:0042",
            ArtifactKind::ArchitectureDecisionRecord,
            ArtifactStatus::Accepted,
        )
        .with_relation(
            RelationKind::Decides,
            ArtifactRef::unpinned("design:passkeys-auth".parse().expect("artifact id")),
        );

        let review = artifact(
            "review:design-passkeys-auth",
            ArtifactKind::ReviewResult,
            ArtifactStatus::Active,
        )
        .with_relation(
            RelationKind::Reviews,
            ArtifactRef::unpinned("design:passkeys-auth".parse().expect("artifact id")),
        );

        ArtifactGraph::build([prd, story, specification, design, adr, review])
            .expect("the example graph is valid")
    }

    /// Seeds `graph` into a fresh backend.
    fn seed(graph: &ArtifactGraph) -> (MemoryBackend, SeedReport) {
        let backend = MemoryBackend::new();
        let report = from_manifest(
            &backend,
            graph,
            "acme",
            "payments",
            Timestamp::from_epoch_millis(1_000),
            &actor(),
        )
        .expect("the manifest seeds");
        (backend, report)
    }

    #[test]
    fn every_artifact_becomes_an_entity_of_its_kinds_type() {
        let (backend, report) = seed(&example());

        assert_eq!(report.entities, 6, "one entity per artifact");
        assert_eq!(backend.len(), 6);
        assert_eq!(report.by_id.len(), 6);

        let design = block_on(
            backend.resolve(
                &"ep://acme/payments/design/passkeys-auth"
                    .parse()
                    .expect("locator"),
            ),
        )
        .expect("the design is addressable by its locator");
        assert_eq!(
            Some(&design),
            report
                .by_id
                .get(&"design:passkeys-auth".parse().expect("artifact id")),
            "the report maps the manifest back to what was stored"
        );

        let entity = block_on(backend.get(&EntityRef::new(design), QueryConsistency::Current))
            .expect("the design is readable");
        assert_eq!(entity.metadata.entity_type.to_string(), "aep.design/v1");
        let Node::Map(fields) = &entity.data else {
            panic!("the body is a mapping");
        };
        assert_eq!(fields.get(STATUS_KEY), Some(&Node::from("approved")));
        assert_eq!(fields.get("version"), Some(&Node::from("7")));
        assert!(
            !fields.contains_key("title"),
            "the manifest states no title for the design, so the entity claims none"
        );
    }

    #[test]
    fn a_relation_to_an_artifact_declared_later_still_resolves() {
        // `design:passkeys-auth` is seeded before `spec:passkeys-auth`, so this edge only works
        // because relations are issued in a second pass.
        let (backend, report) = seed(&example());
        assert_eq!(report.relations, 5);

        let design = report
            .by_id
            .get(&"design:passkeys-auth".parse().expect("artifact id"))
            .expect("the design was seeded")
            .clone();
        let specification = report
            .by_id
            .get(&"spec:passkeys-auth".parse().expect("artifact id"))
            .expect("the specification was seeded")
            .clone();

        let outgoing =
            block_on(backend.relations(&RelationQuery::from(EntityRef::new(design.clone()))))
                .expect("relations are queryable");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing.items[0].kind, RelationKind::Designs);
        assert_eq!(outgoing.items[0].target.id, specification);

        let incoming = block_on(backend.relations(&RelationQuery::to(EntityRef::new(design))))
            .expect("the inverse question is answerable");
        assert_eq!(
            incoming.len(),
            2,
            "the ADR decides it and the review reviews it"
        );
    }

    #[test]
    fn seeding_twice_replays_rather_than_duplicating() {
        let graph = example();
        let (backend, first) = seed(&graph);

        let second = from_manifest(
            &backend,
            &graph,
            "acme",
            "payments",
            Timestamp::from_epoch_millis(2_000),
            &actor(),
        )
        .expect("the second run is a replay, not a conflict");

        assert_eq!(first, second, "the same manifest maps to the same entities");
        assert_eq!(backend.len(), 6, "no second set of entities");
        assert_eq!(
            backend.with_store(|store| store.relations().count()),
            5,
            "no second set of relations"
        );

        let design = EntityRef::new(
            first
                .by_id
                .get(&"design:passkeys-auth".parse().expect("artifact id"))
                .expect("the design was seeded")
                .clone(),
        );
        let history = block_on(backend.history(&design)).expect("history is kept");
        assert_eq!(
            history.len(),
            1,
            "a replay is not a change, so it adds no revision"
        );
    }

    #[test]
    fn a_relation_target_the_manifest_does_not_declare_is_reported() {
        // Built by hand: `ArtifactGraph::build` rejects a dangling edge, and the point here is what
        // seeding does when one reaches it anyway.
        let mut graph = ArtifactGraph::new();
        graph.insert(
            artifact(
                "design:passkeys-auth",
                ArtifactKind::Design,
                ArtifactStatus::Draft,
            )
            .with_relation(
                RelationKind::Designs,
                ArtifactRef::unpinned("spec:missing".parse().expect("artifact id")),
            ),
        );

        let backend = MemoryBackend::new();
        let error = from_manifest(
            &backend,
            &graph,
            "acme",
            "payments",
            Timestamp::from_epoch_millis(1_000),
            &actor(),
        )
        .expect_err("the edge points at nothing");

        assert_eq!(error.code(), "conflict");
        assert!(
            error.to_string().contains("spec:missing"),
            "the report names the artifact that is missing: {error}"
        );
        assert_eq!(
            backend.with_store(|store| store.relations().count()),
            0,
            "the edge is refused rather than dropped in silence"
        );
    }

    #[test]
    fn an_identifier_is_derived_from_the_artifact_id_and_stays_addressable() {
        assert_eq!(
            token(&"design:passkeys-auth".parse().expect("artifact id")),
            "design-passkeys-auth"
        );
        assert_eq!(token(&"adr:0042".parse().expect("artifact id")), "adr-0042");
        assert_eq!(
            token(&"docs/design:a//b".parse().expect("artifact id")),
            "docs-design-a-b",
            "runs of separators collapse, because identifiers reject empty segments"
        );
        assert!(CommandId::new(format!(
            "seed-{}",
            token(&"docs/design:a//b".parse().expect("artifact id"))
        ))
        .is_ok());

        assert_eq!(locator_key("passkeys/auth"), "passkeys-auth");
        assert_eq!(locator_key("AUTH-141"), "AUTH-141");
    }
}
