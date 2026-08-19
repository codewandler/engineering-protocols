//! The reference scenario from the design specification (§104).
//!
//! Nineteen steps that together prove the contract does more than CRUD: identity, relations,
//! revisions, review against an exact revision, history, audit by entity and by correlation, actor
//! versus executor, causal links, idempotent replay, a stale-revision conflict, and the audit record
//! a *refused* command leaves behind.
//!
//! A backend that passes this has demonstrated the shape of the whole design.

use aep_backend_memory::MemoryBackend;
use aep_contract::command::{
    CausationRef, CommandContext, CommandEnvelope, CommandOutcome, CommandService,
};
use aep_contract::consistency::QueryConsistency;
use aep_contract::error::CommandError;
use aep_contract::query::{AuditQuery, EntityQuery, QueryService, RelationQuery};
use aep_contract::testing::block_on;
use aep_domain::artifact::RelationKind;
use aep_domain::audit::AuditKind;
use aep_domain::command::{ApproveDesign, Command, CreateEntity, CreateRelation, UpdateEntity};
use aep_domain::entity::{EntityRef, EntityRevision, EntityType, VersionedEntityRef};
use aep_domain::node::Node;
use aep_domain::time::Timestamp;

/// The activity every command in this scenario belongs to.
const CORRELATION: &str = "corr-passkeys";

/// Builds a command context for one attempt.
fn context(request: &str, key: &str, at: u64) -> CommandContext {
    CommandContext::new(
        request.parse().expect("request id"),
        key.parse().expect("idempotency key"),
        "human:alice".parse().expect("actor"),
        CORRELATION.parse().expect("correlation id"),
        Timestamp::from_epoch_millis(at),
    )
    .executed_by("agent:opus-5".parse().expect("actor"))
}

/// Wraps a command in an envelope.
fn envelope(id: &str, payload: Command, context: CommandContext) -> CommandEnvelope<Command> {
    let command_type = payload.kind().as_str().to_owned();
    let target = payload.target();
    let expected = payload.expected_revision();
    let mut envelope = CommandEnvelope::new(
        id.parse().expect("command id"),
        command_type,
        payload,
        context,
    );
    envelope.target = target;
    envelope.expected_revision = expected;
    envelope
}

/// A body with a title and a status.
fn body(title: &str, status: &str) -> Node {
    Node::Map(
        [
            ("title".to_owned(), Node::from(title)),
            ("status".to_owned(), Node::from(status)),
        ]
        .into(),
    )
}

/// Creates an entity and returns the reference it was created at.
fn create(
    backend: &MemoryBackend,
    command_id: &str,
    at: u64,
    entity_type: &str,
    locator: &str,
    title: &str,
    status: &str,
) -> VersionedEntityRef {
    let payload = Command::CreateEntity(CreateEntity {
        entity_type: entity_type.parse::<EntityType>().expect("type"),
        locator: locator.parse().expect("locator"),
        data: body(title, status),
    });
    let result = block_on(backend.execute(envelope(
        command_id,
        payload,
        context(
            &format!("req-{command_id}"),
            &format!("key-{command_id}"),
            at,
        ),
    )))
    .expect("the entity is created");
    assert_eq!(result.outcome, CommandOutcome::Accepted);
    result.affected.first().cloned().expect("a created entity")
}

/// Relates two entities.
fn relate(
    backend: &MemoryBackend,
    command_id: &str,
    at: u64,
    kind: RelationKind,
    source: &EntityRef,
    target: &EntityRef,
) {
    let payload = Command::CreateRelation(CreateRelation {
        kind,
        source: source.clone(),
        target: target.clone(),
    });
    block_on(backend.execute(envelope(
        command_id,
        payload,
        context(
            &format!("req-{command_id}"),
            &format!("key-{command_id}"),
            at,
        ),
    )))
    .expect("the relation is created");
}

// Nineteen numbered steps that belong together: splitting them would lose the fact that each one
// depends on the state the previous left behind.
#[allow(clippy::too_many_lines)]
#[test]
fn the_reference_scenario() {
    let backend = MemoryBackend::new();

    // 1–2. Create a story, and read it back by identity.
    let story = create(
        &backend,
        "cmd-1",
        1_000,
        "aep.story/v1",
        "ep://acme/payments/story/AUTH-142",
        "Add passkey authentication",
        "active",
    );
    let fetched = block_on(backend.get(&story.unversioned(), QueryConsistency::Current))
        .expect("the story is readable");
    assert_eq!(fetched.metadata.revision, EntityRevision::INITIAL);
    assert_eq!(fetched.metadata.locator.key(), "AUTH-142");

    // 3–4. A specification, related to the story.
    let specification = create(
        &backend,
        "cmd-2",
        2_000,
        "aep.specification/v1",
        "ep://acme/payments/specification/passkeys-auth",
        "Passkey authentication",
        "approved",
    );
    relate(
        &backend,
        "cmd-3",
        3_000,
        RelationKind::Specifies,
        &specification.unversioned(),
        &story.unversioned(),
    );

    // 5–6. A design, related to the specification.
    let design = create(
        &backend,
        "cmd-4",
        4_000,
        "aep.design/v1",
        "ep://acme/payments/design/passkeys-auth",
        "Passkey authentication design",
        "in_review",
    );
    relate(
        &backend,
        "cmd-5",
        5_000,
        RelationKind::Designs,
        &design.unversioned(),
        &specification.unversioned(),
    );

    // 7. A review of the design, at the revision it actually reviewed.
    let review = create(
        &backend,
        "cmd-6",
        6_000,
        "aep.review-result/v1",
        "ep://acme/payments/review-result/design-passkeys-auth",
        "Design review",
        "active",
    );
    relate(
        &backend,
        "cmd-7",
        7_000,
        RelationKind::Reviews,
        &review.unversioned(),
        &design.unversioned(),
    );

    // 8. Approve the design — a semantic command, which can check things a patch cannot.
    let approve = Command::ApproveDesign(ApproveDesign {
        design: design.clone(),
        review: review.unversioned(),
    });
    let approval = block_on(backend.execute(envelope(
        "cmd-8",
        approve.clone(),
        context("req-8", "key-8", 8_000).caused_by(CausationRef("cmd-6".to_owned())),
    )))
    .expect("the design is approved");
    assert_eq!(approval.outcome, CommandOutcome::Accepted);
    let approved_at = approval
        .revision_of(&design.unversioned())
        .expect("the design moved on");
    assert_eq!(
        approved_at.get(),
        2,
        "approval is a change, so the revision advances"
    );

    // 9. The approved state is visible, at least as fresh as the write that produced it.
    let after = block_on(backend.get(
        &design.unversioned(),
        QueryConsistency::at_least(approval.consistency.clone()),
    ))
    .expect("read-your-writes");
    let Node::Map(fields) = &after.data else {
        panic!("the body is a mapping");
    };
    assert_eq!(fields.get("status"), Some(&Node::from("approved")));

    // 10. Relations, in both directions.
    let designs = block_on(backend.relations(&RelationQuery::from(design.unversioned())))
        .expect("relations are queryable");
    assert_eq!(designs.len(), 1);
    assert_eq!(designs.items[0].kind, RelationKind::Designs);

    let reviewed_by = block_on(
        backend.relations(&RelationQuery::to(design.unversioned()).of_kind(RelationKind::Reviews)),
    )
    .expect("the inverse question is answerable");
    assert_eq!(reviewed_by.len(), 1, "which review is about this design?");

    // 11. History.
    let history = block_on(backend.history(&design.unversioned())).expect("history is kept");
    assert_eq!(history.len(), 2, "created, then approved");
    assert_eq!(history[0].revision, EntityRevision::INITIAL);
    assert_eq!(history[1].revision, approved_at);

    // 12–13. Audit, by entity and by activity.
    let by_entity = block_on(backend.audit(&AuditQuery::for_entity(design.unversioned())))
        .expect("audit is queryable");
    assert!(
        by_entity.len() >= 2,
        "creation and approval are both recorded"
    );

    let by_correlation = block_on(backend.audit(&AuditQuery::for_correlation(
        CORRELATION.parse().expect("correlation id"),
    )))
    .expect("audit is queryable by activity");
    assert!(
        by_correlation.len() >= 8,
        "every command in this activity is reconstructable from one identifier, found {}",
        by_correlation.len()
    );

    // 14. Actor and executor are both recorded, and are not the same.
    let record = by_correlation
        .items
        .iter()
        .find(|record| {
            record
                .command_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "cmd-8")
        })
        .expect("the approval is in the trail");
    assert_eq!(record.actor.to_string(), "human:alice");
    assert_eq!(
        record.executor.as_ref().map(ToString::to_string),
        Some("agent:opus-5".to_owned()),
        "who authorised and what ran are different questions"
    );

    // 15. Causal links.
    assert!(
        record.causation.is_some(),
        "the approval names what caused it"
    );
    assert_eq!(record.kind, AuditKind::CommandAccepted);

    // 16. Replaying the same logical command applies nothing twice.
    let replay = block_on(backend.execute(envelope(
        "cmd-8",
        approve,
        context("req-8-retry", "key-8", 9_000),
    )))
    .expect("a replay is not an error");
    assert_eq!(replay.outcome, CommandOutcome::Replayed);
    assert_eq!(
        replay.affected, approval.affected,
        "a replay returns the original result rather than a second approval"
    );
    let unchanged = block_on(backend.get(&design.unversioned(), QueryConsistency::Current))
        .expect("still readable");
    assert_eq!(
        unchanged.metadata.revision, approved_at,
        "the replay did not advance the revision"
    );

    // 17–18. A stale revision is a typed conflict, not a silent overwrite.
    let stale = Command::UpdateEntity(UpdateEntity {
        target: design.unversioned(),
        changes: [("title".to_owned(), Node::from("Rewritten behind your back"))].into(),
    });
    let error = block_on(
        backend.execute(
            envelope("cmd-9", stale, context("req-9", "key-9", 10_000))
                .expecting(EntityRevision::INITIAL),
        ),
    )
    .expect_err("the design has moved on");
    match &error {
        CommandError::RevisionConflict {
            expected, actual, ..
        } => {
            assert_eq!(expected.get(), 1);
            assert_eq!(actual.get(), 2);
        }
        other => panic!("expected a revision conflict, got {other:?}"),
    }
    assert_eq!(error.code(), "revision_conflict");

    let untouched = block_on(backend.get(&design.unversioned(), QueryConsistency::Current))
        .expect("still readable");
    let Node::Map(fields) = &untouched.data else {
        panic!("the body is a mapping");
    };
    assert_eq!(
        fields.get("title"),
        Some(&Node::from("Passkey authentication design")),
        "a refused command changes nothing"
    );

    // 19. And the refusal is in the trail.
    let rejections = block_on(backend.audit(
        &AuditQuery::for_correlation(CORRELATION.parse().expect("correlation id")).rejected(),
    ))
    .expect("rejections are queryable");
    assert_eq!(rejections.len(), 1, "exactly the one refused command");
    let rejection = &rejections.items[0];
    assert_eq!(rejection.kind, AuditKind::CommandRejected);
    assert!(rejection.change.is_none(), "a refusal changed nothing");
    let decision = rejection
        .decision
        .as_ref()
        .expect("a refusal has a decision");
    assert!(!decision.allowed);
    assert_eq!(decision.rule.as_deref(), Some("revision_conflict"));
}

#[test]
fn a_locator_resolves_to_identity_and_is_unique() {
    let backend = MemoryBackend::new();
    let story = create(
        &backend,
        "cmd-1",
        1_000,
        "aep.story/v1",
        "ep://acme/payments/story/AUTH-142",
        "Add passkey authentication",
        "active",
    );

    let resolved = block_on(
        backend.resolve(
            &"ep://acme/payments/story/AUTH-142"
                .parse()
                .expect("locator"),
        ),
    )
    .expect("the locator resolves");
    assert_eq!(resolved, story.id);

    // A second entity at the same address would make the locator ambiguous.
    let duplicate = Command::CreateEntity(CreateEntity {
        entity_type: "aep.story/v1".parse().expect("type"),
        locator: "ep://acme/payments/story/AUTH-142"
            .parse()
            .expect("locator"),
        data: body("A different story", "draft"),
    });
    let error = block_on(backend.execute(envelope(
        "cmd-2",
        duplicate,
        context("req-2", "key-2", 2_000),
    )))
    .expect_err("the address is taken");
    assert_eq!(error.code(), "conflict");
}

#[test]
fn reusing_an_idempotency_key_for_a_different_command_is_refused() {
    let backend = MemoryBackend::new();
    create(
        &backend,
        "cmd-1",
        1_000,
        "aep.story/v1",
        "ep://acme/payments/story/AUTH-142",
        "Add passkey authentication",
        "active",
    );

    let other = Command::CreateEntity(CreateEntity {
        entity_type: "aep.story/v1".parse().expect("type"),
        locator: "ep://acme/payments/story/AUTH-143"
            .parse()
            .expect("locator"),
        data: body("Another story", "draft"),
    });
    // Same key, different logical command: accepting this would make the key meaningless.
    let error = block_on(backend.execute(envelope(
        "cmd-2",
        other,
        context("req-2", "key-cmd-1", 2_000),
    )))
    .expect_err("the key belongs to another command");
    assert_eq!(error.code(), "idempotency_mismatch");
}

#[test]
fn nothing_is_ever_deleted() {
    let backend = MemoryBackend::new();
    let story = create(
        &backend,
        "cmd-1",
        1_000,
        "aep.story/v1",
        "ep://acme/payments/story/AUTH-142",
        "Add passkey authentication",
        "active",
    );

    let archive = Command::ArchiveEntity(aep_domain::command::ArchiveEntity {
        target: story.unversioned(),
        reason: Some("superseded by AUTH-150".to_owned()),
    });
    block_on(backend.execute(envelope("cmd-2", archive, context("req-2", "key-2", 2_000))))
        .expect("archiving is a state change");

    let after = block_on(backend.get(&story.unversioned(), QueryConsistency::Current))
        .expect("an archived entity is still readable");
    assert_eq!(after.metadata.revision.get(), 2);
    let Node::Map(fields) = &after.data else {
        panic!("the body is a mapping");
    };
    assert_eq!(fields.get("status"), Some(&Node::from("archived")));
    assert_eq!(backend.len(), 1, "archiving removes nothing");
}

#[test]
fn a_read_demanding_an_unknown_write_is_refused_rather_than_silently_satisfied() {
    let backend = MemoryBackend::new();
    create(
        &backend,
        "cmd-1",
        1_000,
        "aep.story/v1",
        "ep://acme/payments/story/AUTH-142",
        "Add passkey authentication",
        "active",
    );

    let from_elsewhere =
        aep_contract::consistency::ConsistencyToken::new("seq-999999999999").expect("token");
    let error = block_on(backend.query(
        &EntityQuery::default().with_consistency(QueryConsistency::at_least(from_elsewhere)),
    ))
    .expect_err("this store has not reached that point");
    assert_eq!(error.code(), "consistency_timeout");
}

#[test]
fn a_type_can_be_described_without_hard_coding_it() {
    let backend = MemoryBackend::new();

    let design = block_on(backend.describe_type(&"aep.design/v1".parse().expect("type")))
        .expect("the type is known");
    assert!(design.mutable);
    assert!(design.accepts("aep.design.approve/v1"));

    let review = block_on(backend.describe_type(&"aep.review-result/v1".parse().expect("type")))
        .expect("the type is known");
    assert!(
        !review.mutable,
        "a review that can be edited after the fact is not evidence"
    );

    let unknown = block_on(backend.describe_type(&"acme.widget/v1".parse().expect("type")))
        .expect_err("nothing declares that type");
    assert_eq!(unknown.code(), "not_found");
}
