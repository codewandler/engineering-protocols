//! The universal entity model.
//!
//! Everything the protocol reasons about — a story, a design, an ADR, a review, a piece of evidence,
//! a principle — is an **entity**: a typed node with stable identity, a logical address, a revision
//! and provenance.
//!
//! # Why identity is not the id people type
//!
//! `AUTH-142` is a key. It is unique inside one tracker, it changes when work moves between systems,
//! and two organisations can hold it for different things. An [`EntityId`] is opaque and canonical;
//! `AUTH-142` becomes part of an [`EntityLocator`], which is an address rather than a name.
//!
//! What this buys, concretely: two repositories can reference the same design and mean it, an
//! approval can name the exact revision it approved, and a retried command can be recognised as the
//! same command rather than creating a second design.
//!
//! ```text
//! id        01K2R8JD3ZJME72AJGQY67E5F8      canonical, opaque, never reused
//! locator   ep://acme/payments/design/passkeys-auth
//! type      aep.design/v1                   decides schema, commands, lifecycle, relations
//! revision  7                               what an approval pins to
//! ```
//!
//! # Actor and executor
//!
//! [`ActorRef`] answers *on whose behalf*, which is not the same question as
//! [`Producer`](crate::evidence::Producer)'s *what produced this observation*. An audit trail needs
//! both: `actor: human:alice, executor: agent:release-agent-17` is the ordinary case, and collapsing
//! them loses the ability to answer either question.

use std::fmt;
use std::str::FromStr;

use crate::error::ParseError;
use crate::time::Timestamp;

/// The scheme every entity locator uses.
pub const LOCATOR_SCHEME: &str = "ep";

/// Canonical identity of an entity: opaque, stable and never reused.
///
/// The representation is deliberately not interpreted. Implementations may use ULIDs, `UUIDv7` or
/// organisation's own opaque tokens; nothing in the protocol may parse meaning out of one, because
/// the moment it does, identity has silently become a key again.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct EntityId(String);

impl EntityId {
    /// The shortest identifier that is plausibly opaque rather than a hand-typed key.
    ///
    /// A heuristic, deliberately: nothing can prove an identifier is opaque, but a minimum length
    /// catches the one mistake that actually happens — putting `AUTH-142` in as identity — and the
    /// rejection says where the key belongs instead. ULIDs are 26 characters, UUIDs 36.
    const MIN_LENGTH: usize = 12;
    /// An upper bound, so an identifier cannot be used as a smuggled payload.
    const MAX_LENGTH: usize = 128;

    /// Parses an entity identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        let value: String = value.into();
        let reject = |reason: String| Err(ParseError::identifier("entity", &value, reason));

        if value.len() < Self::MIN_LENGTH {
            return reject(format!(
                "must be at least {} characters; a short identifier is a key, and keys are not \
                 canonical identity — put it in the locator instead",
                Self::MIN_LENGTH
            ));
        }
        if value.len() > Self::MAX_LENGTH {
            return reject(format!("must be at most {} characters", Self::MAX_LENGTH));
        }
        for character in value.chars() {
            if !(character.is_ascii_alphanumeric() || character == '-' || character == '_') {
                return reject(format!("contains disallowed character {character:?}"));
            }
        }
        Ok(Self(value))
    }

    /// The identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[A-Za-z0-9_-]{12,128}$";
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityId({})", self.0)
    }
}

impl FromStr for EntityId {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> serde::Deserialize<'de> for EntityId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for EntityId {
    fn schema_name() -> String {
        "EntityId".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Opaque canonical identity of an entity; never interpreted.".to_owned());
        schema.into()
    }
}

/// How many times an entity has changed.
///
/// Revisions start at 1 and increase by one per accepted mutation. They are what
/// `expected_revision` compares against, and what an approval pins to, so they must be
/// monotonic — this is not a content hash and not a source-control revision (see
/// [`Revision`](crate::artifact::Revision) for that).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct EntityRevision(u64);

impl EntityRevision {
    /// The revision of a newly created entity.
    pub const INITIAL: Self = Self(1);

    /// Builds a revision. Zero is rejected: it would make "unset" and "never changed" the same.
    pub fn new(value: u64) -> Result<Self, ParseError> {
        if value == 0 {
            return Err(ParseError::reference(
                "revision",
                "0",
                "entity revisions start at 1",
            ));
        }
        Ok(Self(value))
    }

    /// The numeric value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The revision after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl fmt::Display for EntityRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A versioned entity type, written `<namespace>.<name>/v<version>`.
///
/// The type is what makes a generic contract usable: it decides the schema, which commands may
/// target the entity, which lifecycle it follows, which relations it may have and whether it is
/// mutable. A harness can therefore ask "what is a design?" instead of hard-coding one.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct EntityType {
    namespace: String,
    name: String,
    version: u32,
}

impl EntityType {
    /// Builds a type.
    pub fn new(
        namespace: impl AsRef<str>,
        name: impl AsRef<str>,
        version: u32,
    ) -> Result<Self, ParseError> {
        let namespace = namespace.as_ref();
        let name = name.as_ref();
        let rendered = format!("{namespace}.{name}/v{version}");
        let reject =
            |reason: &str| ParseError::identifier("entity type", &rendered, reason.to_owned());

        if version == 0 {
            return Err(reject("type versions start at 1"));
        }
        for (part, label) in [(namespace, "namespace"), (name, "name")] {
            if part.is_empty() {
                return Err(reject(&format!("the {label} must not be empty")));
            }
            let valid = part.split(['.', '-']).all(|segment| {
                !segment.is_empty()
                    && segment.chars().all(|character| {
                        character.is_ascii_lowercase() || character.is_ascii_digit()
                    })
            });
            if !valid {
                return Err(reject(&format!(
                    "the {label} must be lower-case kebab-case, optionally dotted"
                )));
            }
        }
        Ok(Self {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            version,
        })
    }

    /// Parses `<namespace>.<name>/v<version>`.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let reject = |reason: &str| ParseError::identifier("entity type", value, reason.to_owned());

        let (qualified, version) = value
            .rsplit_once('/')
            .ok_or_else(|| reject("expected `<namespace>.<name>/v<version>`"))?;
        let version = version
            .strip_prefix('v')
            .ok_or_else(|| reject("the version is written `v1`"))?
            .parse::<u32>()
            .map_err(|_| reject("the version must be an integer"))?;
        let (namespace, name) = qualified
            .rsplit_once('.')
            .ok_or_else(|| reject("expected a namespace, as in `aep.design/v1`"))?;
        Self::new(namespace, name, version)
    }

    /// The namespace, such as `aep`.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The name, such as `design`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The major version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// `true` when both name a type with the same meaning, ignoring version.
    pub fn same_family(&self, other: &Self) -> bool {
        self.namespace == other.namespace && self.name == other.name
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[a-z0-9][a-z0-9.-]*\\.[a-z0-9-]+/v[1-9][0-9]*$";
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}/v{}", self.namespace, self.name, self.version)
    }
}

impl fmt::Debug for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityType({self})")
    }
}

impl FromStr for EntityType {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<EntityType> for String {
    fn from(value: EntityType) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for EntityType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for EntityType {
    fn schema_name() -> String {
        "EntityType".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("A versioned entity type, written `<namespace>.<name>/v<version>`.".to_owned());
        schema.metadata().examples = ["aep.design/v1", "aep.story/v1", "aop.incident/v1"]
            .iter()
            .map(|value| serde_json::Value::String((*value).to_owned()))
            .collect();
        schema.into()
    }
}

/// The logical address of an entity: `ep://acme/payments/design/passkeys-auth`.
///
/// A locator is not a storage URL. It means "resolve this engineering entity", and a backend may
/// answer from a repository, a tracker, a wiki or a database. That indirection is what lets an
/// organisation adopt the protocol without moving anything.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct EntityLocator {
    organisation: String,
    space: String,
    kind: String,
    key: String,
}

impl EntityLocator {
    /// Builds a locator.
    pub fn new(
        organisation: impl AsRef<str>,
        space: impl AsRef<str>,
        kind: impl AsRef<str>,
        key: impl AsRef<str>,
    ) -> Result<Self, ParseError> {
        let locator = Self {
            organisation: organisation.as_ref().to_owned(),
            space: space.as_ref().to_owned(),
            kind: kind.as_ref().to_owned(),
            key: key.as_ref().to_owned(),
        };
        let rendered = locator.to_string();
        for (segment, label) in [
            (&locator.organisation, "organisation"),
            (&locator.space, "space"),
            (&locator.kind, "kind"),
            (&locator.key, "key"),
        ] {
            if segment.is_empty() {
                return Err(ParseError::identifier(
                    "locator",
                    &rendered,
                    format!("the {label} must not be empty"),
                ));
            }
            for character in segment.chars() {
                if !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')) {
                    return Err(ParseError::identifier(
                        "locator",
                        &rendered,
                        format!("the {label} contains disallowed character {character:?}"),
                    ));
                }
            }
        }
        Ok(locator)
    }

    /// Parses `ep://<organisation>/<space>/<kind>/<key>`.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let reject = |reason: &str| ParseError::identifier("locator", value, reason.to_owned());

        let body = value
            .strip_prefix(&format!("{LOCATOR_SCHEME}://"))
            .ok_or_else(|| reject("must begin with `ep://`"))?;
        let segments: Vec<&str> = body.split('/').collect();
        let [organisation, space, kind, key] = segments.as_slice() else {
            return Err(reject(
                "expected `ep://<organisation>/<space>/<kind>/<key>`, four segments",
            ));
        };
        Self::new(organisation, space, kind, key)
    }

    /// The organisation, such as `acme`.
    pub fn organisation(&self) -> &str {
        &self.organisation
    }

    /// The space — a repository, product or project — such as `payments`.
    pub fn space(&self) -> &str {
        &self.space
    }

    /// The kind segment, such as `design`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The key people actually type, such as `AUTH-142`.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str =
        "^ep://[A-Za-z0-9._-]+/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$";
}

impl fmt::Display for EntityLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{LOCATOR_SCHEME}://{}/{}/{}/{}",
            self.organisation, self.space, self.kind, self.key
        )
    }
}

impl fmt::Debug for EntityLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityLocator({self})")
    }
}

impl FromStr for EntityLocator {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<EntityLocator> for String {
    fn from(value: EntityLocator) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for EntityLocator {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for EntityLocator {
    fn schema_name() -> String {
        "EntityLocator".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "Logical address of an entity: `ep://<organisation>/<space>/<kind>/<key>`. Not a \
             storage URL."
                .to_owned(),
        );
        schema.into()
    }
}

/// A reference to the current state of an entity.
#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct EntityRef {
    /// Which entity.
    pub id: EntityId,
}

impl EntityRef {
    /// A reference to `id`.
    pub fn new(id: EntityId) -> Self {
        Self { id }
    }

    /// Pins this reference to a revision.
    pub fn at(&self, revision: EntityRevision) -> VersionedEntityRef {
        VersionedEntityRef {
            id: self.id.clone(),
            revision,
        }
    }
}

impl fmt::Display for EntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl fmt::Debug for EntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityRef({})", self.id)
    }
}

impl From<EntityId> for EntityRef {
    fn from(id: EntityId) -> Self {
        Self::new(id)
    }
}

/// A reference to one exact revision of an entity, written `<id>@<revision>`.
///
/// The distinction from [`EntityRef`] is the point: a review approves *this* revision, and saying so
/// is what stops the approval from silently covering the next one.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct VersionedEntityRef {
    /// Which entity.
    pub id: EntityId,
    /// Which revision of it.
    pub revision: EntityRevision,
}

impl VersionedEntityRef {
    /// A reference to `revision` of `id`.
    pub fn new(id: EntityId, revision: EntityRevision) -> Self {
        Self { id, revision }
    }

    /// Parses `<id>@<revision>`.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let (id, revision) = value.rsplit_once('@').ok_or_else(|| {
            ParseError::identifier(
                "versioned entity",
                value,
                "expected `<id>@<revision>`".to_owned(),
            )
        })?;
        let revision = revision.parse::<u64>().map_err(|_| {
            ParseError::identifier(
                "versioned entity",
                value,
                "the revision must be an integer".to_owned(),
            )
        })?;
        Ok(Self {
            id: EntityId::new(id)?,
            revision: EntityRevision::new(revision)?,
        })
    }

    /// This reference without its revision.
    pub fn unversioned(&self) -> EntityRef {
        EntityRef::new(self.id.clone())
    }

    /// `true` when this reference names the current revision of `metadata`.
    pub fn is_current(&self, metadata: &EntityMetadata) -> bool {
        metadata.id == self.id && metadata.revision == self.revision
    }
}

impl fmt::Display for VersionedEntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.revision)
    }
}

impl fmt::Debug for VersionedEntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VersionedEntityRef({self})")
    }
}

impl FromStr for VersionedEntityRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<VersionedEntityRef> for String {
    fn from(value: VersionedEntityRef) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for VersionedEntityRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for VersionedEntityRef {
    fn schema_name() -> String {
        "VersionedEntityRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some("^[A-Za-z0-9_-]{12,128}@[1-9][0-9]*$".to_owned());
        schema.metadata().description =
            Some("A reference to one exact revision of an entity, `<id>@<revision>`.".to_owned());
        schema.into()
    }
}

/// Who an action is attributed to.
///
/// Distinct from [`Producer`](crate::evidence::Producer): a producer made an observation, an actor
/// bears responsibility. `actor: human:alice, executor: agent:release-agent-17` says a person
/// authorised the change and an agent carried it out, and an audit trail needs to keep the two apart.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub enum ActorRef {
    /// A person.
    Human(String),
    /// An agent.
    Agent(String),
    /// A service acting on its own schedule.
    Service(String),
    /// The system itself, for actions with no external cause.
    System,
}

impl ActorRef {
    /// Parses `human:alice`, `agent:planning-agent`, `service:release-controller` or `system`.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let reject = |reason: &str| ParseError::identifier("actor", value, reason.to_owned());

        if value == "system" {
            return Ok(Self::System);
        }
        let (kind, name) = value
            .split_once(':')
            .ok_or_else(|| reject("expected `<kind>:<name>`, such as `human:alice`"))?;
        if name.is_empty() {
            return Err(reject("the name must not be empty"));
        }
        for character in name.chars() {
            if !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '@')) {
                return Err(reject(&format!("the name contains {character:?}")));
            }
        }
        match kind {
            "human" => Ok(Self::Human(name.to_owned())),
            "agent" => Ok(Self::Agent(name.to_owned())),
            "service" => Ok(Self::Service(name.to_owned())),
            other => Err(reject(&format!(
                "unknown actor kind {other:?}; expected human, agent, service or `system`"
            ))),
        }
    }

    /// `true` when a person bears responsibility for this.
    pub fn is_human(&self) -> bool {
        matches!(self, Self::Human(_))
    }

    /// `true` when an agent is acting.
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent(_))
    }

    /// The name, without the kind prefix.
    pub fn name(&self) -> &str {
        match self {
            Self::Human(name) | Self::Agent(name) | Self::Service(name) => name,
            Self::System => "system",
        }
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^(system|(human|agent|service):[A-Za-z0-9._@-]+)$";
}

impl fmt::Display for ActorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human(name) => write!(f, "human:{name}"),
            Self::Agent(name) => write!(f, "agent:{name}"),
            Self::Service(name) => write!(f, "service:{name}"),
            Self::System => f.write_str("system"),
        }
    }
}

impl fmt::Debug for ActorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActorRef({self})")
    }
}

impl FromStr for ActorRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<ActorRef> for String {
    fn from(value: ActorRef) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for ActorRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ActorRef {
    fn schema_name() -> String {
        "ActorRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "Who an action is attributed to: `human:alice`, `agent:x`, `service:y`, `system`."
                .to_owned(),
        );
        schema.into()
    }
}

/// How an entity came to exist and change.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EntityProvenance {
    /// Who created it.
    pub created_by: ActorRef,
    /// What ran, when that differs from who authorised it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_executor: Option<ActorRef>,
    /// Who changed it last.
    pub updated_by: ActorRef,
    /// What ran for the last change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_executor: Option<ActorRef>,
    /// The source revision it describes, for generated entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<crate::artifact::Revision>,
}

impl EntityProvenance {
    /// Provenance for something `actor` just created.
    pub fn created_by(actor: ActorRef) -> Self {
        Self {
            created_by: actor.clone(),
            created_executor: None,
            updated_by: actor,
            updated_executor: None,
            source_revision: None,
        }
    }

    /// Records a change by `actor`, optionally naming what executed it.
    #[must_use]
    pub fn updated(&self, actor: ActorRef, executor: Option<ActorRef>) -> Self {
        Self {
            created_by: self.created_by.clone(),
            created_executor: self.created_executor.clone(),
            updated_by: actor,
            updated_executor: executor,
            source_revision: self.source_revision.clone(),
        }
    }
}

/// What every entity carries, whatever its type.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EntityMetadata {
    /// Canonical identity.
    pub id: EntityId,
    /// Logical address.
    pub locator: EntityLocator,
    /// Versioned type.
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    /// How many times it has changed.
    pub revision: EntityRevision,
    /// When it was created.
    pub created_at: Timestamp,
    /// When it last changed.
    pub updated_at: Timestamp,
    /// Who made it and who changed it.
    pub provenance: EntityProvenance,
}

impl EntityMetadata {
    /// Metadata for a newly created entity.
    pub fn new(
        id: EntityId,
        locator: EntityLocator,
        entity_type: EntityType,
        at: Timestamp,
        actor: ActorRef,
    ) -> Self {
        Self {
            id,
            locator,
            entity_type,
            revision: EntityRevision::INITIAL,
            created_at: at,
            updated_at: at,
            provenance: EntityProvenance::created_by(actor),
        }
    }

    /// A reference to this entity's current state.
    pub fn reference(&self) -> EntityRef {
        EntityRef::new(self.id.clone())
    }

    /// A reference pinned to this entity's current revision.
    pub fn versioned_reference(&self) -> VersionedEntityRef {
        VersionedEntityRef::new(self.id.clone(), self.revision)
    }

    /// Advances to the next revision, recording who changed it.
    pub fn advance(&mut self, at: Timestamp, actor: ActorRef, executor: Option<ActorRef>) {
        self.revision = self.revision.next();
        self.updated_at = at;
        self.provenance = self.provenance.updated(actor, executor);
    }
}

/// An entity: metadata plus its typed body.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Entity<T> {
    /// What every entity carries.
    pub metadata: EntityMetadata,
    /// What this type carries.
    pub data: T,
}

impl<T> Entity<T> {
    /// Wraps `data` with `metadata`.
    pub fn new(metadata: EntityMetadata, data: T) -> Self {
        Self { metadata, data }
    }

    /// Its identity.
    pub fn id(&self) -> &EntityId {
        &self.metadata.id
    }

    /// Its current revision.
    pub fn revision(&self) -> EntityRevision {
        self.metadata.revision
    }

    /// Applies `change` to the body and advances the revision.
    pub fn update(
        &mut self,
        at: Timestamp,
        actor: ActorRef,
        executor: Option<ActorRef>,
        change: impl FnOnce(&mut T),
    ) {
        change(&mut self.data);
        self.metadata.advance(at, actor, executor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> EntityId {
        EntityId::new("01K2R8JD3ZJME72AJGQY67E5F8").expect("a ULID-shaped identifier")
    }

    #[test]
    fn an_entity_id_is_opaque_and_long_enough_not_to_be_a_key() {
        assert!(EntityId::new("01K2R8JD3ZJME72AJGQY67E5F8").is_ok());
        let error = EntityId::new("AUTH-142").expect_err("a tracker key is not identity");
        assert!(
            error
                .to_string()
                .contains("keys are not canonical identity"),
            "{error}"
        );
        assert!(EntityId::new("has spaces in it").is_err());
    }

    #[test]
    fn entity_types_round_trip_and_carry_a_version() {
        let parsed: EntityType = "aep.design/v1".parse().expect("parses");
        assert_eq!(parsed.namespace(), "aep");
        assert_eq!(parsed.name(), "design");
        assert_eq!(parsed.version(), 1);
        assert_eq!(parsed.to_string(), "aep.design/v1");

        let next: EntityType = "aep.design/v2".parse().expect("parses");
        assert!(parsed.same_family(&next));
        assert_ne!(parsed, next, "a version change is a different type");

        assert!(
            "design".parse::<EntityType>().is_err(),
            "no namespace or version"
        );
        assert!(
            "aep.design/1".parse::<EntityType>().is_err(),
            "the version needs its `v`"
        );
        assert!("aep.Design/v1".parse::<EntityType>().is_err(), "upper case");
    }

    #[test]
    fn locators_are_addresses_with_four_segments() {
        let locator: EntityLocator = "ep://acme/payments/design/passkeys-auth"
            .parse()
            .expect("parses");
        assert_eq!(locator.organisation(), "acme");
        assert_eq!(locator.space(), "payments");
        assert_eq!(locator.kind(), "design");
        assert_eq!(locator.key(), "passkeys-auth");
        assert_eq!(
            locator.to_string(),
            "ep://acme/payments/design/passkeys-auth"
        );

        assert!("https://acme.example/designs/1"
            .parse::<EntityLocator>()
            .is_err());
        assert!("ep://acme/payments/design"
            .parse::<EntityLocator>()
            .is_err());
    }

    #[test]
    fn a_versioned_reference_names_one_revision_and_an_unversioned_one_does_not() {
        let reference = EntityRef::new(id());
        let pinned = reference.at(EntityRevision::new(3).expect("revision"));
        assert_eq!(pinned.to_string(), format!("{}@3", id()));
        assert_eq!(pinned.unversioned(), reference);

        let round_tripped: VersionedEntityRef = pinned.to_string().parse().expect("parses");
        assert_eq!(round_tripped, pinned);
    }

    #[test]
    fn actors_distinguish_who_authorised_from_what_ran() {
        let alice: ActorRef = "human:alice".parse().expect("parses");
        let agent: ActorRef = "agent:release-agent-17".parse().expect("parses");
        assert!(alice.is_human());
        assert!(agent.is_agent());
        assert_eq!(
            "system".parse::<ActorRef>().expect("parses"),
            ActorRef::System
        );
        assert_eq!(agent.to_string(), "agent:release-agent-17");

        let error = "alice".parse::<ActorRef>().expect_err("no kind");
        assert!(error.to_string().contains("human:alice"), "{error}");
        assert!("robot:hal".parse::<ActorRef>().is_err(), "unknown kind");
    }

    #[test]
    fn a_revision_advances_and_records_who_changed_it() {
        let at = Timestamp::from_epoch_millis(1_700_000_000_000);
        let mut entity = Entity::new(
            EntityMetadata::new(
                id(),
                "ep://acme/payments/design/passkeys-auth"
                    .parse()
                    .expect("locator"),
                "aep.design/v1".parse().expect("type"),
                at,
                "human:alice".parse().expect("actor"),
            ),
            "the first draft".to_owned(),
        );
        assert_eq!(entity.revision(), EntityRevision::INITIAL);

        let later = Timestamp::from_epoch_millis(1_700_000_060_000);
        entity.update(
            later,
            "human:alice".parse().expect("actor"),
            Some("agent:opus-5".parse().expect("actor")),
            |data| data.push_str(", revised"),
        );

        assert_eq!(entity.revision().get(), 2);
        assert_eq!(entity.data, "the first draft, revised");
        assert_eq!(entity.metadata.updated_at, later);
        assert_eq!(
            entity.metadata.provenance.created_by.to_string(),
            "human:alice",
            "who created it does not change when someone else edits"
        );
        assert_eq!(
            entity
                .metadata
                .provenance
                .updated_executor
                .as_ref()
                .map(ToString::to_string),
            Some("agent:opus-5".to_owned()),
            "the agent that ran the change is recorded separately from who authorised it"
        );
    }

    #[test]
    fn a_pinned_reference_stops_being_current_when_the_entity_moves_on() {
        let at = Timestamp::from_epoch_millis(1);
        let mut metadata = EntityMetadata::new(
            id(),
            "ep://acme/payments/design/passkeys-auth"
                .parse()
                .expect("locator"),
            "aep.design/v1".parse().expect("type"),
            at,
            "human:alice".parse().expect("actor"),
        );
        let approved = metadata.versioned_reference();
        assert!(approved.is_current(&metadata));

        metadata.advance(at, "agent:opus-5".parse().expect("actor"), None);
        assert!(
            !approved.is_current(&metadata),
            "an approval of revision 1 must not follow the entity to revision 2"
        );
    }

    #[test]
    fn revisions_start_at_one() {
        assert!(EntityRevision::new(0).is_err());
        assert_eq!(EntityRevision::INITIAL.get(), 1);
        assert_eq!(EntityRevision::INITIAL.next().get(), 2);
    }
}
