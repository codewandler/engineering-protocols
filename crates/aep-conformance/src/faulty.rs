//! A backend that is wrong in exactly one way.
//!
//! A conformance suite that passes everything is not a suite, and nothing about reading one tells
//! you whether it would catch anything. [`FaultyBackend`] wraps a working backend and injects a
//! single, specific fault; the crate's own tests then assert that the suite responsible for that
//! property fails — and that the others still pass, so a fault does not simply break everything.
//!
//! Each fault is an *observable* misbehaviour, not a broken internal: the wrapper can only perturb
//! what goes in and what comes out, which is the same position a real backend's clients are in.
//!
//! ```text
//! Fault::ReplayApplies          →  the idempotency suite must fail
//! Fault::IgnoreExpectedRevision →  the concurrency suite must fail
//! Fault::DropRejectionAudit     →  the rejected-audit suite must fail
//! ```

use aep_contract::command::{CommandEnvelope, CommandResult, CommandService};
use aep_contract::consistency::QueryConsistency;
use aep_contract::error::{CommandError, QueryError};
use aep_contract::query::{
    AuditQuery, EntityEnvelope, EntityQuery, Page, QueryService, Relation, RelationQuery,
    RevisionRecord,
};
use aep_contract::registry::TypeDescriptor;
use aep_domain::audit::AuditRecord;
use aep_domain::command::Command;
use aep_domain::entity::{ActorRef, EntityId, EntityLocator, EntityRef, EntityType};
use aep_domain::ids::IdempotencyKey;
use aep_domain::node::Node;

/// One way a backend can be wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Fault {
    /// A replayed command applies again instead of returning the original result.
    ReplayApplies,
    /// A stale write merges instead of being refused.
    IgnoreExpectedRevision,
    /// A command reports none of the entities it changed.
    DropAffected,
    /// A command reports none of the events it emitted.
    DropEvents,
    /// Refusals leave no audit record.
    DropRejectionAudit,
    /// Nothing leaves an audit record.
    DropAudit,
    /// Audit records lose the activity they belonged to.
    ScrambleCorrelation,
    /// Audit records lose what caused them.
    DropCausation,
    /// Entities lose who created them.
    ForgetProvenance,
    /// A read that demands a later view is answered from whatever is at hand.
    AnswerStaleReads,
    /// Query filters are accepted and ignored.
    IgnoreQueryFilters,
    /// History keeps only the most recent revision.
    LoseHistory,
    /// Relations are not returned.
    DropRelations,
    /// Archived entities disappear, which is deletion by another name.
    HideArchived,
    /// No type can be described.
    UndescribeTypes,
}

impl Fault {
    /// Every fault this wrapper can inject.
    pub const ALL: &'static [Self] = &[
        Self::ReplayApplies,
        Self::IgnoreExpectedRevision,
        Self::DropAffected,
        Self::DropEvents,
        Self::DropRejectionAudit,
        Self::DropAudit,
        Self::ScrambleCorrelation,
        Self::DropCausation,
        Self::ForgetProvenance,
        Self::AnswerStaleReads,
        Self::IgnoreQueryFilters,
        Self::LoseHistory,
        Self::DropRelations,
        Self::HideArchived,
        Self::UndescribeTypes,
    ];

    /// The suite that exists to catch this fault.
    pub fn caught_by(self) -> &'static str {
        match self {
            Self::ReplayApplies => "idempotency",
            Self::IgnoreExpectedRevision => "concurrency",
            Self::DropAffected => "command-execution",
            Self::DropEvents => "events",
            Self::DropRejectionAudit => "rejected-audit",
            Self::DropAudit => "audit",
            Self::ScrambleCorrelation => "correlation",
            Self::DropCausation => "causation",
            Self::ForgetProvenance => "provenance",
            Self::AnswerStaleReads => "consistency",
            Self::IgnoreQueryFilters => "query",
            Self::LoseHistory => "history",
            Self::DropRelations => "relations",
            Self::HideArchived => "immutability",
            Self::UndescribeTypes => "type-registry",
        }
    }

    /// What goes wrong, in one line, for a report.
    pub fn describe(self) -> &'static str {
        match self {
            Self::ReplayApplies => "a replayed command is applied a second time",
            Self::IgnoreExpectedRevision => "a stale write overwrites newer state",
            Self::DropAffected => "a command does not report what it changed",
            Self::DropEvents => "a command does not report the events it emitted",
            Self::DropRejectionAudit => "a refused command leaves no trace",
            Self::DropAudit => "nothing is audited",
            Self::ScrambleCorrelation => "audit records cannot be reassembled into one activity",
            Self::DropCausation => "the immediate cause of a step is lost",
            Self::ForgetProvenance => "an entity does not say who created it",
            Self::AnswerStaleReads => "a read-your-writes demand is ignored",
            Self::IgnoreQueryFilters => "a query filter is accepted and ignored",
            Self::LoseHistory => "earlier revisions are forgotten",
            Self::DropRelations => "edges are not returned",
            Self::HideArchived => "an archived entity disappears",
            Self::UndescribeTypes => "no type can be described",
        }
    }
}

/// A backend wrapped so that exactly one property fails to hold.
#[derive(Debug)]
pub struct FaultyBackend<B> {
    inner: B,
    fault: Fault,
}

impl<B> FaultyBackend<B> {
    /// Wraps `inner` so that `fault` is injected.
    pub fn new(inner: B, fault: Fault) -> Self {
        Self { inner, fault }
    }

    /// Which fault this backend has.
    pub fn fault(&self) -> Fault {
        self.fault
    }

    /// The backend underneath.
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B: CommandService<Command = Command>> CommandService for FaultyBackend<B> {
    type Command = Command;

    async fn execute(
        &self,
        mut command: CommandEnvelope<Self::Command>,
    ) -> Result<CommandResult, CommandError> {
        match self.fault {
            Fault::ReplayApplies => {
                // A fresh key on every attempt means the backend never recognises a replay.
                command.context.idempotency_key = IdempotencyKey::new(format!(
                    "{}-{}",
                    command.context.idempotency_key, command.context.request_id
                ))
                .unwrap_or_else(|_| command.context.idempotency_key.clone());
            }
            Fault::IgnoreExpectedRevision => {
                command.expected_revision = None;
            }
            _ => {}
        }

        let mut result = self.inner.execute(command).await?;
        match self.fault {
            Fault::DropAffected => result.affected.clear(),
            Fault::DropEvents => result.events.clear(),
            _ => {}
        }
        Ok(result)
    }
}

impl<B: QueryService<AuditRecord = AuditRecord>> QueryService for FaultyBackend<B> {
    type AuditRecord = AuditRecord;

    async fn get(
        &self,
        reference: &EntityRef,
        consistency: QueryConsistency,
    ) -> Result<EntityEnvelope, QueryError> {
        let consistency = self.weaken(consistency);
        let mut entity = match self.inner.get(reference, consistency).await {
            Ok(entity) => entity,
            Err(QueryError::ConsistencyTimeout { .. }) if self.fault == Fault::AnswerStaleReads => {
                self.inner.get(reference, QueryConsistency::Current).await?
            }
            Err(error) => return Err(error),
        };

        if self.fault == Fault::HideArchived && is_archived(&entity.data) {
            return Err(QueryError::NotFound {
                what: format!("entity `{reference}`"),
            });
        }
        if self.fault == Fault::ForgetProvenance {
            entity.metadata.provenance.created_by = ActorRef::System;
            entity.metadata.provenance.updated_by = ActorRef::System;
        }
        Ok(entity)
    }

    async fn resolve(&self, locator: &EntityLocator) -> Result<EntityId, QueryError> {
        self.inner.resolve(locator).await
    }

    async fn query(&self, query: &EntityQuery) -> Result<Page<EntityEnvelope>, QueryError> {
        let mut effective = query.clone();
        effective.consistency = self.weaken(query.consistency.clone());
        if self.fault == Fault::IgnoreQueryFilters {
            effective.entity_type = None;
            effective.matching.clear();
            effective.related_to = None;
            effective.relation = None;
            effective.organisation = None;
            effective.space = None;
        }

        match self.inner.query(&effective).await {
            Err(QueryError::ConsistencyTimeout { .. }) if self.fault == Fault::AnswerStaleReads => {
                let mut current = effective;
                current.consistency = QueryConsistency::Current;
                self.inner.query(&current).await
            }
            other => other,
        }
    }

    async fn relations(&self, query: &RelationQuery) -> Result<Page<Relation>, QueryError> {
        if self.fault == Fault::DropRelations {
            return Ok(Page::complete(Vec::new()));
        }
        self.inner.relations(query).await
    }

    async fn history(&self, reference: &EntityRef) -> Result<Vec<RevisionRecord>, QueryError> {
        let history = self.inner.history(reference).await?;
        if self.fault == Fault::LoseHistory {
            return Ok(history.into_iter().last().into_iter().collect());
        }
        Ok(history)
    }

    async fn audit(&self, query: &AuditQuery) -> Result<Page<Self::AuditRecord>, QueryError> {
        if self.fault == Fault::DropAudit {
            return Ok(Page::complete(Vec::new()));
        }
        let mut page = self.inner.audit(query).await?;
        match self.fault {
            Fault::DropRejectionAudit => page.items.retain(|record| !record.is_rejection()),
            Fault::ScrambleCorrelation => {
                for record in &mut page.items {
                    record.correlation_id =
                        aep_domain::ids::CorrelationId::new("corr-elsewhere").expect("well formed");
                }
            }
            Fault::DropCausation => {
                for record in &mut page.items {
                    record.causation = None;
                    record.command_id = None;
                }
            }
            _ => {}
        }
        Ok(page)
    }

    async fn describe_type(&self, entity_type: &EntityType) -> Result<TypeDescriptor, QueryError> {
        if self.fault == Fault::UndescribeTypes {
            return Err(QueryError::NotFound {
                what: format!("type `{entity_type}`"),
            });
        }
        self.inner.describe_type(entity_type).await
    }
}

impl<B> FaultyBackend<B> {
    /// Drops a freshness demand when the fault is to ignore them.
    fn weaken(&self, consistency: QueryConsistency) -> QueryConsistency {
        if self.fault == Fault::AnswerStaleReads {
            QueryConsistency::Current
        } else {
            consistency
        }
    }
}

/// `true` when a body says it has been archived.
fn is_archived(data: &Node) -> bool {
    data.as_map()
        .and_then(|entries| entries.get("status"))
        .and_then(Node::as_text)
        == Some("archived")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fault_names_the_suite_that_catches_it() {
        let suites = crate::suites::all();
        for fault in Fault::ALL {
            let name = fault.caught_by();
            assert!(
                suites.iter().any(|suite| suite.name == name),
                "{fault:?} claims to be caught by `{name}`, which is not a registered suite"
            );
            assert!(!fault.describe().is_empty());
        }
    }

    #[test]
    fn each_fault_is_claimed_by_at_most_one_suite() {
        let mut claimed: Vec<&str> = Fault::ALL.iter().map(|fault| fault.caught_by()).collect();
        claimed.sort_unstable();
        let mut unique = claimed.clone();
        unique.dedup();
        assert_eq!(
            claimed, unique,
            "two faults caught by one suite makes it impossible to say which property that suite \
             actually protects"
        );
    }
}
