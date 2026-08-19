//! The query side: read-only, and answerable by any backend.
//!
//! The queries here are the ones the design specification asks for by name (§45): all stories with
//! status ready, all approved designs for an epic, all ADRs related to a design, everything
//! correlated with one activity, everything caused by one command, all changes to an entity between
//! two revisions.
//!
//! None of them assumes an index, a join or a query language. A backend answers them however it can.

use std::collections::BTreeMap;

use aep_domain::artifact::RelationKind;
use aep_domain::entity::{
    ActorRef, Entity, EntityId, EntityLocator, EntityRef, EntityRevision, EntityType,
};
use aep_domain::ids::{AuditId, CommandId, CorrelationId, RelationId};
use aep_domain::node::Node;
use aep_domain::time::Timestamp;

use crate::consistency::QueryConsistency;
use crate::error::QueryError;
use crate::registry::TypeDescriptor;

/// An entity as the contract carries it: metadata plus an untyped body.
///
/// Untyped on purpose. The generic contract moves entities without knowing what a design *is*; a
/// typed SDK layer deserialises the body into a domain type when it wants one.
pub type EntityEnvelope = Entity<Node>;

/// An opaque position in a result set.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Cursor(pub String);

/// One page of results.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
    /// The results.
    pub items: Vec<T>,
    /// Where to continue from, when there is more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
}

impl<T> Page<T> {
    /// A page that is the whole answer.
    pub fn complete(items: Vec<T>) -> Self {
        Self { items, next: None }
    }

    /// How many results this page holds.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` when this page holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// `true` when more results follow.
    pub fn has_more(&self) -> bool {
        self.next.is_some()
    }
}

/// Which entities to return.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EntityQuery {
    /// Only this type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<EntityType>,
    /// Only entities whose locator is in this organisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation: Option<String>,
    /// Only entities whose locator is in this space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    /// Only entities whose body has these exact values at these keys.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub matching: BTreeMap<String, Node>,
    /// Only entities related to this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_to: Option<EntityRef>,
    /// The relation to follow, when `related_to` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<RelationKind>,
    /// How many at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Where to continue from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Cursor>,
    /// How fresh the answer has to be.
    #[serde(default)]
    pub consistency: QueryConsistency,
}

impl EntityQuery {
    /// Every entity of `entity_type`.
    pub fn of_type(entity_type: EntityType) -> Self {
        Self {
            entity_type: Some(entity_type),
            ..Self::default()
        }
    }

    /// Narrows to entities whose body has `value` at `key`.
    #[must_use]
    pub fn matching(mut self, key: impl Into<String>, value: Node) -> Self {
        self.matching.insert(key.into(), value);
        self
    }

    /// Narrows to entities `relation`-related to `entity`.
    #[must_use]
    pub fn related_to(mut self, entity: EntityRef, relation: RelationKind) -> Self {
        self.related_to = Some(entity);
        self.relation = Some(relation);
        self
    }

    /// Demands a view at least as fresh as a previous write.
    #[must_use]
    pub fn with_consistency(mut self, consistency: QueryConsistency) -> Self {
        self.consistency = consistency;
        self
    }
}

/// A relation in the entity graph.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    /// Its identity, so it can be removed by name.
    pub id: RelationId,
    /// What the edge means.
    pub kind: RelationKind,
    /// Where it starts.
    pub source: EntityRef,
    /// Where it points.
    pub target: EntityRef,
    /// When it was created.
    pub created_at: Timestamp,
    /// Who created it.
    pub created_by: ActorRef,
}

/// Which relations to return.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RelationQuery {
    /// Only relations from this entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<EntityRef>,
    /// Only relations to this entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<EntityRef>,
    /// Only this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<RelationKind>,
    /// How many at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Where to continue from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Cursor>,
    /// How fresh the answer has to be.
    #[serde(default)]
    pub consistency: QueryConsistency,
}

impl RelationQuery {
    /// Relations leaving `entity`.
    pub fn from(entity: EntityRef) -> Self {
        Self {
            source: Some(entity),
            ..Self::default()
        }
    }

    /// Relations arriving at `entity` — which is how "what supersedes this ADR?" is asked.
    pub fn to(entity: EntityRef) -> Self {
        Self {
            target: Some(entity),
            ..Self::default()
        }
    }

    /// Narrows to one relation kind.
    #[must_use]
    pub fn of_kind(mut self, kind: RelationKind) -> Self {
        self.kind = Some(kind);
        self
    }
}

/// One step in an entity's history.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RevisionRecord {
    /// Which revision this describes.
    pub revision: EntityRevision,
    /// When it happened.
    pub at: Timestamp,
    /// Who authorised it.
    pub actor: ActorRef,
    /// What ran, when that differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor: Option<ActorRef>,
    /// The command that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<CommandId>,
    /// The audit record covering it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<AuditId>,
}

/// Which audit records to return.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AuditQuery {
    /// Only records about this entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<EntityRef>,
    /// Only records from this activity — how "show me everything about that release" is asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<CorrelationId>,
    /// Only records for this command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<CommandId>,
    /// Only records attributed to this actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<ActorRef>,
    /// Only records of this kind, by its wire name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Only records at or after this time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<Timestamp>,
    /// Only records before this time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<Timestamp>,
    /// Only rejected attempts.
    ///
    /// The query that makes §55 useful: "what did this agent try to do and get stopped from doing?"
    #[serde(default)]
    pub rejected_only: bool,
    /// How many at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Where to continue from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Cursor>,
}

impl AuditQuery {
    /// Everything belonging to one activity.
    pub fn for_correlation(correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id: Some(correlation_id),
            ..Self::default()
        }
    }

    /// Everything about one entity.
    pub fn for_entity(entity: EntityRef) -> Self {
        Self {
            entity: Some(entity),
            ..Self::default()
        }
    }

    /// Narrows to attempts that were refused.
    #[must_use]
    pub fn rejected(mut self) -> Self {
        self.rejected_only = true;
        self
    }
}

/// The read-only surface.
///
/// Every method is a question a harness or a person actually asks. None of them mutates: a backend
/// that changes state in a query has broken the contract even if nothing observable differs, because
/// the audit trail will not show it.
pub trait QueryService {
    /// The audit record type this backend returns.
    type AuditRecord;

    /// Fetches one entity.
    fn get(
        &self,
        reference: &EntityRef,
        consistency: QueryConsistency,
    ) -> impl std::future::Future<Output = Result<EntityEnvelope, QueryError>>;

    /// Resolves a logical address to an identity.
    fn resolve(
        &self,
        locator: &EntityLocator,
    ) -> impl std::future::Future<Output = Result<EntityId, QueryError>>;

    /// Finds entities.
    fn query(
        &self,
        query: &EntityQuery,
    ) -> impl std::future::Future<Output = Result<Page<EntityEnvelope>, QueryError>>;

    /// Finds relations.
    fn relations(
        &self,
        query: &RelationQuery,
    ) -> impl std::future::Future<Output = Result<Page<Relation>, QueryError>>;

    /// Returns an entity's revision history, oldest first.
    fn history(
        &self,
        reference: &EntityRef,
    ) -> impl std::future::Future<Output = Result<Vec<RevisionRecord>, QueryError>>;

    /// Returns audit records.
    fn audit(
        &self,
        query: &AuditQuery,
    ) -> impl std::future::Future<Output = Result<Page<Self::AuditRecord>, QueryError>>;

    /// Describes a type, so a harness need not hard-code what a design is.
    fn describe_type(
        &self,
        entity_type: &EntityType,
    ) -> impl std::future::Future<Output = Result<TypeDescriptor, QueryError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> EntityRef {
        EntityRef::new("01K2R8JD3ZJME72AJGQY67E5F8".parse().expect("id"))
    }

    #[test]
    fn a_page_reports_whether_more_follows() {
        let complete: Page<u32> = Page::complete(vec![1, 2, 3]);
        assert_eq!(complete.len(), 3);
        assert!(!complete.has_more());

        let partial = Page {
            items: vec![1],
            next: Some(Cursor("offset:1".to_owned())),
        };
        assert!(partial.has_more());
    }

    #[test]
    fn an_entity_query_composes_the_documented_questions() {
        let approved_designs = EntityQuery::of_type("aep.design/v1".parse().expect("type"))
            .matching("status", Node::from("approved"))
            .related_to(entity(), RelationKind::Designs);

        assert_eq!(
            approved_designs.entity_type.map(|t| t.to_string()),
            Some("aep.design/v1".to_owned())
        );
        assert_eq!(approved_designs.matching.len(), 1);
        assert_eq!(approved_designs.relation, Some(RelationKind::Designs));
    }

    #[test]
    fn relations_can_be_asked_in_both_directions() {
        let outgoing = RelationQuery::from(entity()).of_kind(RelationKind::Supersedes);
        assert_eq!(outgoing.source, Some(entity()));
        assert!(outgoing.target.is_none());

        // "What supersedes this ADR?" is the inverse question, and both must be askable.
        let incoming = RelationQuery::to(entity()).of_kind(RelationKind::Supersedes);
        assert_eq!(incoming.target, Some(entity()));
        assert!(incoming.source.is_none());
    }

    #[test]
    fn audit_can_be_asked_for_refused_attempts_only() {
        let query = AuditQuery::for_correlation("corr-42".parse().expect("id")).rejected();
        assert!(query.rejected_only);
        assert_eq!(
            query.correlation_id.map(|id| id.to_string()),
            Some("corr-42".to_owned())
        );
    }

    #[test]
    fn queries_carry_their_freshness_requirement() {
        let token = crate::consistency::ConsistencyToken::new("seq:9").expect("token");
        let query =
            EntityQuery::default().with_consistency(QueryConsistency::at_least(token.clone()));
        assert_eq!(query.consistency.token(), Some(&token));

        let json = serde_json::to_value(&query).expect("serialises");
        assert_eq!(json["consistency"]["consistency"], "at_least");
    }
}
