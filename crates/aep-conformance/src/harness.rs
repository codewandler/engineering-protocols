//! Driving a backend from a suite.
//!
//! Suites are written against this, not against a concrete backend, so the same suite runs against
//! an in-memory store and against something with a network behind it.
//!
//! The harness deliberately does very little: it builds envelopes with fresh identifiers, drives the
//! futures, and offers a few shorthands for the entities every suite needs. Anything cleverer would
//! start hiding the behaviour under test.

use std::cell::Cell;

use aep_contract::command::{CommandContext, CommandEnvelope, CommandResult, CommandService};
use aep_contract::error::CommandError;
use aep_contract::query::QueryService;
use aep_contract::testing::block_on;
use aep_domain::audit::AuditRecord;
use aep_domain::command::{Command, CreateEntity};
use aep_domain::entity::{ActorRef, EntityLocator, EntityRef, EntityType, VersionedEntityRef};
use aep_domain::ids::{CommandId, CorrelationId, IdempotencyKey, RequestId};
use aep_domain::node::Node;
use aep_domain::time::Timestamp;

/// What a suite needs from a backend: both surfaces, over the AEP command and audit vocabulary.
///
/// Stated as one bound so a suite signature does not have to repeat it, and so a backend that
/// implements only half the contract fails to compile against the suites rather than failing them at
/// run time with a confusing message.
pub trait Backend:
    CommandService<Command = Command> + QueryService<AuditRecord = AuditRecord>
{
}

impl<T> Backend for T where
    T: CommandService<Command = Command> + QueryService<AuditRecord = AuditRecord>
{
}

/// Where a suite's entities live.
pub const ORGANISATION: &str = "conformance";
/// The space a suite's entities live in.
pub const SPACE: &str = "suite";

/// Builds commands with fresh identifiers, and drives them.
#[derive(Debug)]
pub struct Harness {
    label: String,
    actor: ActorRef,
    executor: ActorRef,
    correlation: CorrelationId,
    sequence: Cell<u64>,
    clock: Cell<u64>,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new("conformance")
    }
}

impl Harness {
    /// A harness whose commands are attributed to `label`.
    ///
    /// Every identifier this harness generates carries the label, because all sixteen suites run
    /// against **one** backend: two suites that both asked for `key-000001` would have the second
    /// one's command recognised as a replay of the first's, and the resulting failures would look
    /// like backend bugs rather than like a collision between suites.
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            actor: format!("human:{label}").parse().unwrap_or(ActorRef::System),
            executor: format!("agent:{label}-runner")
                .parse()
                .unwrap_or(ActorRef::System),
            correlation: CorrelationId::new(format!("corr-{label}"))
                .unwrap_or_else(|_| CorrelationId::new("corr-conformance").expect("well formed")),
            sequence: Cell::new(0),
            clock: Cell::new(1_700_000_000_000),
        }
    }

    /// Who the harness acts as.
    pub fn actor(&self) -> &ActorRef {
        &self.actor
    }

    /// What the harness reports as having executed its commands.
    pub fn executor(&self) -> &ActorRef {
        &self.executor
    }

    /// The activity every command from this harness belongs to.
    pub fn correlation(&self) -> &CorrelationId {
        &self.correlation
    }

    /// The next sequence number, which every generated identifier is built from.
    fn tick(&self) -> u64 {
        let next = self.sequence.get() + 1;
        self.sequence.set(next);
        next
    }

    /// A timestamp that advances by a second per call, so ordering is observable without sleeping.
    pub fn now(&self) -> Timestamp {
        let now = self.clock.get();
        self.clock.set(now + 1_000);
        Timestamp::from_epoch_millis(now)
    }

    /// A fresh command context.
    pub fn context(&self) -> CommandContext {
        let sequence = self.tick();
        CommandContext::new(
            RequestId::new(format!("req-{}-{sequence:06}", self.label)).expect("well formed"),
            IdempotencyKey::new(format!("key-{}-{sequence:06}", self.label)).expect("well formed"),
            self.actor.clone(),
            self.correlation.clone(),
            self.now(),
        )
        .executed_by(self.executor.clone())
    }

    /// A context reusing `key`, for testing replay.
    pub fn context_with_key(&self, key: &IdempotencyKey) -> CommandContext {
        let mut context = self.context();
        context.idempotency_key = key.clone();
        context
    }

    /// A fresh command identifier.
    pub fn command_id(&self) -> CommandId {
        CommandId::new(format!("cmd-{}-{:06}", self.label, self.tick())).expect("well formed")
    }

    /// A locator nothing else in this run uses.
    pub fn locator(&self, kind: &str) -> EntityLocator {
        EntityLocator::new(
            ORGANISATION,
            SPACE,
            kind,
            format!("{kind}-{}-{:06}", self.label, self.tick()),
        )
        .expect("a generated locator is well formed")
    }

    /// Wraps a command, taking the target and revision assertion from the command itself.
    pub fn envelope(
        &self,
        command_id: CommandId,
        payload: Command,
        context: CommandContext,
    ) -> CommandEnvelope<Command> {
        let command_type = payload.kind().as_str().to_owned();
        let target = payload.target();
        let expected = payload.expected_revision();
        let mut envelope = CommandEnvelope::new(command_id, command_type, payload, context);
        envelope.target = target;
        envelope.expected_revision = expected;
        envelope
    }

    /// Executes a command, blocking until it answers.
    pub fn execute<B: Backend>(
        &self,
        backend: &B,
        envelope: CommandEnvelope<Command>,
    ) -> Result<CommandResult, CommandError> {
        block_on(backend.execute(envelope))
    }

    /// Executes a command built from a payload, with a fresh context.
    pub fn run<B: Backend>(
        &self,
        backend: &B,
        payload: Command,
    ) -> Result<CommandResult, CommandError> {
        let envelope = self.envelope(self.command_id(), payload, self.context());
        self.execute(backend, envelope)
    }

    /// Creates an entity of `entity_type` with a body of `fields`, returning where it landed.
    pub fn create<B: Backend>(
        &self,
        backend: &B,
        entity_type: &str,
        kind: &str,
        fields: &[(&str, Node)],
    ) -> Result<VersionedEntityRef, CommandError> {
        let entity_type: EntityType = entity_type
            .parse()
            .expect("a suite's entity type is well formed");
        let data = Node::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        );
        let result = self.run(
            backend,
            Command::CreateEntity(CreateEntity {
                entity_type,
                locator: self.locator(kind),
                data,
            }),
        )?;
        result
            .affected
            .first()
            .cloned()
            .ok_or_else(|| CommandError::Conflict {
                reason: "the backend accepted a creation but reported no affected entity"
                    .to_owned(),
            })
    }

    /// Creates a design, the entity most suites need.
    pub fn create_design<B: Backend>(
        &self,
        backend: &B,
    ) -> Result<VersionedEntityRef, CommandError> {
        self.create(
            backend,
            "aep.design/v1",
            "design",
            &[
                ("title", Node::from("A design under test")),
                ("status", Node::from("in_review")),
            ],
        )
    }

    /// Reads an entity's current body, or explains why it could not.
    pub fn read<B: Backend>(
        &self,
        backend: &B,
        reference: &EntityRef,
    ) -> Result<aep_contract::query::EntityEnvelope, String> {
        block_on(backend.get(
            reference,
            aep_contract::consistency::QueryConsistency::Current,
        ))
        .map_err(|error| error.to_string())
    }

    /// Reads a field from an entity's body.
    pub fn field<B: Backend>(
        &self,
        backend: &B,
        reference: &EntityRef,
        field: &str,
    ) -> Option<Node> {
        let entity = self.read(backend, reference).ok()?;
        entity
            .data
            .as_map()
            .and_then(|entries| entries.get(field))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn identifiers_are_fresh_and_timestamps_advance() {
        let harness = Harness::new("test");
        let first = harness.context();
        let second = harness.context();

        assert_ne!(first.request_id, second.request_id);
        assert_ne!(first.idempotency_key, second.idempotency_key);
        assert_eq!(first.correlation_id, second.correlation_id, "one activity");
        assert!(
            second.issued_at > first.issued_at,
            "ordering must be observable without sleeping"
        );
    }

    #[test]
    fn the_harness_creates_entities_a_suite_can_then_ask_about() {
        let backend = MemoryBackend::new();
        let harness = Harness::new("test");

        let design = harness.create_design(&backend).expect("created");
        assert_eq!(design.revision.get(), 1);

        let title = harness
            .field(&backend, &design.unversioned(), "title")
            .expect("the body is readable");
        assert_eq!(title, Node::from("A design under test"));
    }

    #[test]
    fn two_harnesses_never_collide_on_a_shared_backend() {
        // All sixteen suites run against one backend. A shared identifier would make one suite's
        // command look like a replay of another's, and the failure would be blamed on the backend.
        let first = Harness::new("alpha");
        let second = Harness::new("beta");

        assert_ne!(
            first.context().idempotency_key,
            second.context().idempotency_key
        );
        assert_ne!(first.command_id(), second.command_id());
        assert_ne!(
            first.locator("design").to_string(),
            second.locator("design").to_string()
        );
    }

    #[test]
    fn each_harness_has_its_own_activity() {
        let first = Harness::new("alpha");
        let second = Harness::new("beta");
        assert_ne!(first.correlation(), second.correlation());
        assert_ne!(first.actor().to_string(), second.actor().to_string());
    }

    #[test]
    fn the_executor_is_recorded_separately_from_the_actor() {
        let harness = Harness::new("test");
        let context = harness.context();
        assert_eq!(context.actor, *harness.actor());
        assert_eq!(context.executor.as_ref(), Some(harness.executor()));
        assert_ne!(context.actor.to_string(), harness.executor().to_string());
    }
}
