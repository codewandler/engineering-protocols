//! The whole specification: one system, its domains, and every type in it.
//!
//! A [`SystemSpec`] is what the rest of the toolchain compiles, generates from and checks against.
//! It carries three things a domain cannot know on its own:
//!
//! | | why it lives here |
//! |---|---|
//! | [`FormatVersion`] | the language the document is written in, refused rather than guessed |
//! | [`TypeRegistry`] | every type from every domain, so a reference resolves without knowing who declared it |
//! | the domain list | so [`SystemSpec::owner_of`] can answer who owns a name — the question every later validation asks |
//!
//! # A specification is not a file
//!
//! Design §24 asks for large specifications to be decomposable:
//!
//! ```text
//! ess/
//! ├── system.yaml          <- the header: system, version, format
//! └── domains/
//!     ├── invoice.yaml
//!     └── email.yaml
//! ```
//!
//! So the unit this module models is the [`SpecPart`] — one source's contribution — and a
//! [`Manifest`] of parts that [`SystemSpec::merge`] compiles into one graph. Every declaration
//! remembers which [`Source`] it came from, so a name declared twice is refused with both files
//! named rather than with one silently winning.
//!
//! No file is opened here. A [`Source`] is a label, not a path to read: reading a directory belongs
//! to the compiler crate, and keeping it out means this crate stays clock-free, IO-free and
//! testable from string literals.
//!
//! # The version that matters
//!
//! Design §23 versions several things. Two of them are here: the **format** (`ess/1`) and the
//! **system** (`billing/v3`). A format this build does not implement is refused with an instruction
//! to upgrade the tooling, because a later format may mean something different by the same words —
//! the same rule, and the same reasoning, as `aep_domain::protocol::is_supported_major`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};

#[cfg(test)]
use crate::domain::RawDomainSpec;
use crate::domain::{DomainSpec, MemberKind};
use crate::name::{Naming, QualifiedName, Version};
use crate::types::{NamedType, TypeBody, TypeRef, TypeRegistry};

/// Specification format major versions this build implements.
pub const SUPPORTED_FORMATS: &[u32] = &[1];

/// `true` when this build implements `format`.
pub fn is_supported_format(format: FormatVersion) -> bool {
    SUPPORTED_FORMATS.contains(&format.major())
}

/// The version of the specification language a document is written in — `ess/1`.
///
/// Only the major part exists, and an unknown one is refused rather than interpreted. A document
/// written against `ess/2` may use the same words for different things, and a reader that guesses
/// produces a specification nobody wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct FormatVersion(u32);

impl FormatVersion {
    /// The first, and so far only, format.
    pub const V1: Self = Self(1);

    /// How a format version is written.
    pub const PREFIX: &'static str = "ess/";

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^ess/[1-9][0-9]*$";

    /// Builds a format version. Zero is rejected: it invites "unversioned".
    pub fn new(major: u32) -> Result<Self, ParseError> {
        if major == 0 {
            return Err(ParseError::reference(
                "specification format",
                "ess/0",
                "format versions start at 1",
            ));
        }
        Ok(Self(major))
    }

    /// The numeric part.
    pub const fn major(self) -> u32 {
        self.0
    }

    /// `true` when this build implements it.
    pub fn is_supported(self) -> bool {
        is_supported_format(self)
    }

    /// Parses `ess/1`.
    ///
    /// The shape is checked here; whether the version is one this build implements is a validation
    /// question, so that a document from the future is refused with a code and a location rather
    /// than with a parse error nobody can match on.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let digits = value.strip_prefix(Self::PREFIX).ok_or_else(|| {
            ParseError::reference(
                "specification format",
                value,
                "a format is written `ess/1`; this is not an ESS document",
            )
        })?;
        // `u32::from_str` would accept `ess/01` and `ess/+1`, which the published pattern
        // `^ess/[1-9][0-9]*$` rejects — a parser looser than its own schema means a document the
        // editor calls invalid and the tool accepts.
        let major = crate::name::parse_major(digits).ok_or_else(|| {
            ParseError::reference(
                "specification format",
                value,
                "expected a whole number after `ess/`, without a leading zero",
            )
        })?;
        Self::new(major)
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

impl FromStr for FormatVersion {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<FormatVersion> for String {
    fn from(value: FormatVersion) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for FormatVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for FormatVersion {
    fn schema_name() -> String {
        "FormatVersion".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("The specification language version, such as `ess/1`.".to_owned());
        schema.into()
    }
}

/// Where one part of a specification came from.
///
/// A label, not a handle: this crate never opens it. `system.yaml`, `domains/invoice.yaml`, or
/// `<document>` for a specification that arrived as one string. Its only job is to make a
/// cross-file conflict nameable — "declared in both of these" is a fixable diagnostic, "declared
/// twice" is not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Source(String);

impl Source {
    /// The label for a specification that arrived as a single document.
    pub const DOCUMENT: &'static str = "<document>";

    /// Labels a part.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The label for a single-document specification.
    pub fn document() -> Self {
        Self::new(Self::DOCUMENT)
    }

    /// The label itself.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Source {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Source {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// The system-level declarations, which exactly one source carries.
///
/// Kept apart from [`SpecPart`] because a `domains/invoice.yaml` has none of them: it contributes
/// domains to a system it does not name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SpecHeader {
    /// The system's namespace — `billing`.
    pub name: QualifiedName,
    /// The system's own version — `billing/v3`.
    pub version: Version,
    /// The specification language the document is written in.
    pub format: FormatVersion,
    /// What the system is called on the wire, and what a person is shown.
    pub naming: Naming,
    /// What the system is, in one paragraph.
    pub summary: Option<String>,
}

/// One source's contribution to a specification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SpecPart {
    /// Where it came from.
    pub source: Source,
    /// The system-level declarations, when this is the source that carries them.
    pub header: Option<SpecHeader>,
    /// The domains it declares. Several sources may contribute to one domain.
    pub domains: Vec<DomainSpec>,
    /// Types that belong to the system rather than to any one domain.
    pub types: Vec<NamedType>,
}

impl SpecPart {
    /// An empty contribution from `source`.
    pub fn new(source: impl Into<Source>) -> Self {
        Self {
            source: source.into(),
            header: None,
            domains: Vec::new(),
            types: Vec::new(),
        }
    }

    /// Makes this the source that declares the system.
    #[must_use]
    pub fn with_header(mut self, header: SpecHeader) -> Self {
        self.header = Some(header);
        self
    }

    /// Adds a domain.
    #[must_use]
    pub fn with_domain(mut self, domain: DomainSpec) -> Self {
        self.domains.push(domain);
        self
    }

    /// Adds a system-level type.
    #[must_use]
    pub fn with_type(mut self, declared: NamedType) -> Self {
        self.types.push(declared);
        self
    }
}

/// Every source that makes up one specification.
///
/// The manifest is what a directory of `.yaml` files becomes once something has read them. It has
/// no opinion about where they were: the compiler crate produces one from a directory, a test
/// produces one from string literals, and both compile the same way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    parts: Vec<SpecPart>,
}

impl Manifest {
    /// An empty manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a part.
    pub fn push(&mut self, part: SpecPart) {
        self.parts.push(part);
    }

    /// Adds a part, for builder-style assembly.
    #[must_use]
    pub fn with(mut self, part: SpecPart) -> Self {
        self.push(part);
        self
    }

    /// Every source in the manifest, in the order it was added.
    pub fn sources(&self) -> impl Iterator<Item = &Source> + '_ {
        self.parts.iter().map(|part| &part.source)
    }

    /// The parts.
    pub fn parts(&self) -> &[SpecPart] {
        &self.parts
    }

    /// How many sources contribute.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// `true` when nothing contributes.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Compiles every part into one specification.
    pub fn compile(self) -> Result<SystemSpec, ValidationErrors> {
        SystemSpec::merge(self.parts)
    }
}

impl FromIterator<SpecPart> for Manifest {
    fn from_iter<I: IntoIterator<Item = SpecPart>>(parts: I) -> Self {
        Self {
            parts: parts.into_iter().collect(),
        }
    }
}

/// A whole specification: one system, its domains, and every type in it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SystemSpec {
    /// The system's namespace — `billing`. Every domain sits inside it.
    pub name: QualifiedName,
    /// The system's own version — `billing/v3` (design §23).
    pub version: Version,
    /// The specification language it is written in. Always one this build implements: an unknown
    /// format is refused during validation, so a validated specification cannot carry one.
    pub format: FormatVersion,
    /// Its bounded contexts, in declaration order.
    pub domains: Vec<DomainSpec>,
    /// Every type in the system, from every domain, indexed by name.
    ///
    /// Domains keep their own declarations; this registry is the index that lets a reference in one
    /// domain resolve against a type declared in another without knowing where it lives.
    pub types: TypeRegistry,
    /// What the system is called on the wire, and what a person is shown.
    pub naming: Naming,
    /// What the system is, in one paragraph.
    pub summary: Option<String>,
}

impl SystemSpec {
    /// The system at its own version — `billing/v3`.
    pub fn reference(&self) -> String {
        format!("{}/{}", self.name, self.version)
    }

    /// The domain with this name.
    pub fn domain(&self, name: &QualifiedName) -> Option<&DomainSpec> {
        self.domains.iter().find(|domain| &domain.name == name)
    }

    /// The domain that declares `name`.
    ///
    /// The question every later validation asks: a binding checks that the event it listens for is
    /// owned by somebody, a generator asks which module a command belongs in, a diagnostic asks who
    /// to blame. Ownership is by **declaration**, not by namespace containment, so a name that sits
    /// under `billing.invoice` and is declared by nobody has no owner — which is the answer that
    /// makes a dangling reference visible instead of plausible.
    ///
    /// The domain's own name is not one of its members; look that up with [`SystemSpec::domain`].
    pub fn owner_of(&self, name: &QualifiedName) -> Option<&DomainSpec> {
        self.domains.iter().find(|domain| domain.declares(name))
    }

    /// What `name` is declared as, and by which domain.
    pub fn declaration_of(&self, name: &QualifiedName) -> Option<(&DomainSpec, MemberKind)> {
        self.domains
            .iter()
            .find_map(|domain| domain.kind_of(name).map(|kind| (domain, kind)))
    }

    /// Compiles several sources into one specification.
    ///
    /// This is the only way a [`SystemSpec`] is built — a single-document specification is a
    /// manifest of one — so there is exactly one place where duplicate declarations, an unsupported
    /// format and unresolved types are refused.
    ///
    /// Rejections:
    ///
    /// * no source declares the system, or two do;
    /// * a format this build does not implement;
    /// * a name declared by two sources, or by two domains, with both named;
    /// * a domain outside the system's namespace, or equal to it;
    /// * a type reference that resolves to nothing, or a cycle of them.
    ///
    /// Sources may contribute to the same domain — that is what makes `domains/invoice.yaml` and
    /// `domains/invoice-tax.yaml` legal — but not to the same declaration.
    pub fn merge(parts: impl IntoIterator<Item = SpecPart>) -> Result<Self, ValidationErrors> {
        let (system, errors) = Self::merge_reporting(parts);
        match system {
            Some(system) => errors.into_result(system),
            None => Err(errors),
        }
    }

    /// As [`SystemSpec::merge`], but hands back the graph it built as well as what is wrong with it.
    ///
    /// A failed merge still produces a usable system for every failure except a missing header, and
    /// the caller that assembles members needs one: without it, an unsupported format would hide
    /// every broken reference in the specification and the author would fix one problem per run.
    /// `None` therefore means only "no source declares the system" — with no namespace, no domain
    /// list and no type registry, there is nothing left to check anything against.
    pub(crate) fn merge_reporting(
        parts: impl IntoIterator<Item = SpecPart>,
    ) -> (Option<Self>, ValidationErrors) {
        let parts: Vec<SpecPart> = parts.into_iter().collect();
        let mut assembly = Assembly::default();
        let header = assembly.resolve_header(&parts);

        for part in parts {
            assembly.absorb(part);
        }

        let Some(header) = header else {
            return (None, assembly.errors);
        };

        assembly.check_format(&header);
        assembly.check_namespace(&header.name);
        let errors = &mut assembly.errors;
        errors.extend(DomainSpec::validate_all(&assembly.domains));

        let types = build_registry(&assembly.types, &assembly.domains, errors);

        let system = Self {
            name: header.name,
            version: header.version,
            format: header.format,
            domains: assembly.domains,
            types,
            naming: header.naming,
            summary: header.summary,
        };
        (Some(system), assembly.errors)
    }
}

/// Indexes every type in the system and checks that each one's references resolve.
fn build_registry(
    system_types: &[NamedType],
    domains: &[DomainSpec],
    errors: &mut ValidationErrors,
) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    for declared in system_types
        .iter()
        .chain(domains.iter().flat_map(|domain| domain.types.iter()))
    {
        if let Err(error) = registry.insert(declared.clone()) {
            errors.push(error);
        }
    }

    for declared in registry.iter() {
        for dependency in declared.dependencies() {
            errors.extend(registry.resolve(
                &TypeRef::Named(dependency.clone()),
                &format!("types.{}", declared.name),
            ));
        }
    }

    check_inhabitation(&registry, errors);

    registry
}

/// Whether a type reference is inhabited, given what is known to be inhabited so far.
///
/// `Optional`, `List` and `Map` are inhabited unconditionally: an absent value and an empty
/// collection are base cases, so a type that reaches itself only through one of them still has
/// values and a generator over it still terminates.
fn reference_inhabited(reference: &TypeRef, inhabited: &BTreeSet<QualifiedName>) -> bool {
    match reference {
        TypeRef::Primitive(_) | TypeRef::Optional(_) | TypeRef::List(_) | TypeRef::Map(_, _) => {
            true
        }
        TypeRef::Named(name) => inhabited.contains(name),
    }
}

/// Whether a declaration is inhabited, given what is known to be inhabited so far.
///
/// The union case is the whole point of this being about inhabitation rather than about cycles: a
/// union needs **one** variant that can be built, not all of them. `Expr = union {leaf: Integer,
/// pair: Pair}` with `Pair = struct {left: Expr, right: Expr}` describes a perfectly ordinary
/// expression tree — every value of it bottoms out in a `leaf` — and a rule that refused every cycle
/// of names would refuse it.
fn body_inhabited(declared: &NamedType, inhabited: &BTreeSet<QualifiedName>) -> bool {
    match &declared.body {
        TypeBody::Newtype { of, .. } => reference_inhabited(of, inhabited),
        // Every field has to be buildable; a struct cannot omit one.
        TypeBody::Struct { fields, .. } => fields
            .iter()
            .all(|field| reference_inhabited(&field.type_ref, inhabited)),
        // A variant is a name, not a reference to another type; an enum with no variants is refused
        // by `NamedType`'s own conversion, so anything reaching here has at least one.
        TypeBody::Enum { .. } => true,
        // One buildable variant is enough.
        TypeBody::Union { variants, .. } => variants
            .values()
            .any(|variant| reference_inhabited(variant, inhabited)),
    }
}

/// Reports every type no value can inhabit.
///
/// A least-fixpoint rather than a cycle search: start with nothing known to be inhabited, then keep
/// marking declarations whose requirements are met until nothing new can be marked. Whatever is left
/// unmarked cannot be built — which is design §20's forbidden dependency cycle, stated as the
/// property that actually matters instead of as the shape that usually causes it.
///
/// Stating it the other way round was a real defect, not a stylistic difference: treating every
/// union variant as required refused the expression tree above.
fn check_inhabitation(registry: &TypeRegistry, errors: &mut ValidationErrors) {
    let mut inhabited: BTreeSet<QualifiedName> = BTreeSet::new();
    loop {
        let mut grew = false;
        for declared in registry.iter() {
            if !inhabited.contains(&declared.name) && body_inhabited(declared, &inhabited) {
                inhabited.insert(declared.name.clone());
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    for declared in registry.iter() {
        if inhabited.contains(&declared.name) {
            continue;
        }
        // An undeclared dependency is already reported as an unresolved reference, and a type that
        // is uninhabited only because one of its fields names nothing would be a second message
        // about the same mistake.
        if names_something_undeclared(declared, registry) {
            continue;
        }
        // Naming what it is waiting for, not just that it is stuck: a fixpoint knows which
        // requirements were never met, and "requires X, which also cannot be built" is the sentence
        // an author can act on. Bare "this cannot exist" leaves them to find the other end.
        let blockers = unmet_requirements(declared, &inhabited);
        errors.push(
            ValidationError::new(
                ValidationCode::SelfReference,
                format!("types.{}", declared.name),
                format!(
                    "no value of `{}` can exist: it requires {}, which cannot be built either",
                    declared.name,
                    blockers
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
            )
            .with_hint(
                "break the requirement, put one step behind `Optional` or `List`, which have a \
                 base case, or give a union a variant that does not recurse",
            ),
        );
    }
}

/// The named types a declaration needs and which are themselves uninhabited.
///
/// For a struct, the fields that are stuck; for a union, every variant, because a union is only
/// stuck when all of them are. Sorted and deduplicated so the message is stable.
fn unmet_requirements(
    declared: &NamedType,
    inhabited: &BTreeSet<QualifiedName>,
) -> Vec<QualifiedName> {
    let mut blocked: BTreeSet<QualifiedName> = BTreeSet::new();
    let mut consider = |reference: &TypeRef| {
        if let TypeRef::Named(name) = reference {
            if !inhabited.contains(name) {
                blocked.insert(name.clone());
            }
        }
    };

    match &declared.body {
        TypeBody::Newtype { of, .. } => consider(of),
        TypeBody::Struct { fields, .. } => {
            for field in fields {
                consider(&field.type_ref);
            }
        }
        TypeBody::Enum { .. } => {}
        TypeBody::Union { variants, .. } => {
            for variant in variants.values() {
                consider(variant);
            }
        }
    }

    blocked.into_iter().collect()
}

/// Whether a declaration references a name the registry does not hold.
///
/// The inner walk recurses once per generic wrapper and does not count: a [`TypeRef`] cannot nest
/// past [`MAX_TYPE_DEPTH`](crate::types::MAX_TYPE_DEPTH), which its parser enforces, and this walk
/// stops at a `Named` leaf rather than following it into the registry.
fn names_something_undeclared(declared: &NamedType, registry: &TypeRegistry) -> bool {
    fn undeclared(reference: &TypeRef, registry: &TypeRegistry) -> bool {
        match reference {
            TypeRef::Named(name) => registry.get(name).is_none(),
            TypeRef::Optional(inner) | TypeRef::List(inner) => undeclared(inner, registry),
            TypeRef::Map(_, value) => undeclared(value, registry),
            TypeRef::Primitive(_) => false,
        }
    }

    match &declared.body {
        TypeBody::Newtype { of, .. } => undeclared(of, registry),
        TypeBody::Struct { fields, .. } => fields
            .iter()
            .any(|field| undeclared(&field.type_ref, registry)),
        TypeBody::Enum { .. } => false,
        TypeBody::Union { variants, .. } => variants
            .values()
            .any(|variant| undeclared(variant, registry)),
    }
}

/// Who declared a name, and from where.
#[derive(Debug, Clone)]
struct Claim {
    source: Source,
    owner: Option<QualifiedName>,
    kind: MemberKind,
}

impl Claim {
    /// How the claimant reads in a message.
    fn owner_label(&self) -> String {
        self.owner.as_ref().map_or_else(
            || "the system itself".to_owned(),
            |owner| format!("`{owner}`"),
        )
    }
}

/// The state of a merge in progress.
#[derive(Debug, Default)]
struct Assembly {
    domains: Vec<DomainSpec>,
    index: BTreeMap<QualifiedName, usize>,
    claims: BTreeMap<QualifiedName, Claim>,
    types: Vec<NamedType>,
    errors: ValidationErrors,
}

impl Assembly {
    /// Finds the one source that declares the system, refusing zero and refusing two.
    fn resolve_header(&mut self, parts: &[SpecPart]) -> Option<SpecHeader> {
        let mut headers = parts
            .iter()
            .filter_map(|part| part.header.as_ref().map(|header| (&part.source, header)));

        let (first_source, first) = headers.next().or_else(|| {
            self.errors.push(
                // Nothing here references anything: a required declaration is simply absent,
                // which is a different fault from a declaration that declares nothing.
                ValidationError::new(
                    ValidationCode::MissingDeclaration,
                    "system",
                    "no source declares the system".to_owned(),
                )
                .with_hint(
                    "exactly one source carries `system:`, `version:` and `format:`; the others \
                     contribute domains to it",
                ),
            );
            None
        })?;

        for (source, other) in headers {
            self.errors.push(
                ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    "system",
                    format!(
                        "the system is declared in `{first_source}` as `{}` and in `{source}` as \
                         `{}`",
                        first.name, other.name
                    ),
                )
                .with_hint("exactly one source declares the system"),
            );
        }

        Some(first.clone())
    }

    /// Refuses a format this build does not implement.
    fn check_format(&mut self, header: &SpecHeader) {
        if !header.format.is_supported() {
            self.errors.push(
                ValidationError::new(
                    // A specification format is not a protocol version, and a harness that treats
                    // them as one will go looking for the wrong document to upgrade.
                    ValidationCode::UnsupportedFormatVersion,
                    "system.format",
                    format!(
                        "this build implements specification format(s) {SUPPORTED_FORMATS:?}, not \
                         `{}`",
                        header.format
                    ),
                )
                .with_hint(
                    "upgrade the tooling that reads it rather than reinterpreting the document: a \
                     later format may mean something different by the same words",
                ),
            );
        }
    }

    /// Refuses a domain or a system-level type that does not sit inside the system.
    fn check_namespace(&mut self, system: &QualifiedName) {
        for declared in &self.types {
            if !declared.name.is_within(system) {
                // Declared, and declared in the wrong place: a conflict between two well-formed
                // declarations, not a reference with nothing behind it.
                self.errors.push(ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    "system.types",
                    format!(
                        "`{}` is not inside the system `{system}`, so this specification cannot \
                         declare it",
                        declared.name
                    ),
                ));
            }
        }

        for domain in &self.domains {
            if &domain.name == system {
                self.errors.push(
                    ValidationError::new(
                        ValidationCode::SelfReference,
                        format!("domain {}", domain.name),
                        format!(
                            "`{}` is the system itself, not a domain inside it",
                            domain.name
                        ),
                    )
                    .with_hint(format!(
                        "give it a namespace of its own, such as `{system}.core`"
                    )),
                );
            } else if !domain.name.is_within(system) {
                self.errors.push(
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        format!("domain {}", domain.name),
                        format!("`{}` is not inside the system `{system}`", domain.name),
                    )
                    .with_hint(format!(
                        "a domain of `{system}` is named `{system}.something`"
                    )),
                );
            }
        }
    }

    /// Folds one source's contribution into the graph.
    fn absorb(&mut self, part: SpecPart) {
        let source = part.source;
        for declared in part.types {
            if self.claim(&declared.name, MemberKind::Type, None, &source) {
                self.types.push(declared);
            }
        }
        for domain in part.domains {
            self.absorb_domain(domain, &source);
        }
    }

    /// Folds one domain's declarations into the domain of that name, creating it if this is the
    /// first source to mention it.
    fn absorb_domain(&mut self, domain: DomainSpec, source: &Source) {
        let owner = domain.name.clone();
        let position = if let Some(position) = self.index.get(&owner).copied() {
            let existing = &self.domains[position];
            if !domain.naming.is_empty()
                && !existing.naming.is_empty()
                && existing.naming != domain.naming
            {
                self.errors.push(
                    // Several sources contributing to one domain is the supported case, so nothing
                    // is declared twice here; what cannot hold is two names for the same domain.
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        format!("domain {owner}.naming"),
                        format!("`{owner}` is named differently in `{source}` and elsewhere"),
                    )
                    .with_hint("one source names a domain; the others contribute to it"),
                );
            }
            position
        } else {
            let mut fresh = DomainSpec::new(owner.clone());
            fresh.naming = domain.naming.clone();
            self.domains.push(fresh);
            self.index.insert(owner.clone(), self.domains.len() - 1);
            self.domains.len() - 1
        };

        if self.domains[position].naming.is_empty() {
            self.domains[position].naming = domain.naming;
        }

        for declared in domain.types {
            if self.claim(&declared.name, MemberKind::Type, Some(&owner), source) {
                self.domains[position].types.push(declared);
            }
        }
        self.absorb_names(
            position,
            domain.entities,
            MemberKind::Entity,
            &owner,
            source,
        );
        self.absorb_names(
            position,
            domain.commands,
            MemberKind::Command,
            &owner,
            source,
        );
        self.absorb_names(position, domain.events, MemberKind::Event, &owner, source);
        self.absorb_names(position, domain.views, MemberKind::View, &owner, source);
        self.absorb_names(position, domain.errors, MemberKind::Error, &owner, source);
        self.absorb_names(position, domain.actors, MemberKind::Actor, &owner, source);
    }

    /// Adds every name of one kind that is not already claimed.
    fn absorb_names(
        &mut self,
        position: usize,
        names: Vec<QualifiedName>,
        kind: MemberKind,
        owner: &QualifiedName,
        source: &Source,
    ) {
        for name in names {
            if !self.claim(&name, kind, Some(owner), source) {
                continue;
            }
            let domain = &mut self.domains[position];
            match kind {
                MemberKind::Type => unreachable!("types are absorbed as declarations"),
                MemberKind::Entity => domain.entities.push(name),
                MemberKind::Command => domain.commands.push(name),
                MemberKind::Event => domain.events.push(name),
                MemberKind::View => domain.views.push(name),
                MemberKind::Error => domain.errors.push(name),
                MemberKind::Actor => domain.actors.push(name),
            }
        }
    }

    /// Records who declares `name`, refusing a second claim and naming both sources.
    ///
    /// A refused claim is dropped rather than added, so the merged graph never contains a name
    /// twice and the same conflict is not then reported a second time by
    /// [`DomainSpec::validate_all`].
    fn claim(
        &mut self,
        name: &QualifiedName,
        kind: MemberKind,
        owner: Option<&QualifiedName>,
        source: &Source,
    ) -> bool {
        let claim = Claim {
            source: source.clone(),
            owner: owner.cloned(),
            kind,
        };
        let Some(first) = self.claims.get(name) else {
            self.claims.insert(name.clone(), claim);
            return true;
        };

        let location = owner.map_or_else(
            || format!("system.{}", kind.field()),
            |owner| format!("domain {}.{}", owner, kind.field()),
        );
        let message = if first.source == claim.source {
            format!(
                "`{name}` is declared twice in `{source}`, as a {} and as a {kind}",
                first.kind
            )
        } else {
            format!(
                "`{name}` is declared in `{}` and in `{source}`",
                first.source
            )
        };
        self.errors.push(
            ValidationError::new(ValidationCode::DuplicateDeclaration, location, message)
                .with_hint(format!(
                    "claimed by {} and by {}; a name has exactly one owner",
                    first.owner_label(),
                    claim.owner_label()
                )),
        );
        false
    }
}

/// A system header with its domains inline, for writing a manifest in a test as one string.
///
/// **This is not a document shape the tooling reads.** The specification document is
/// [`RawSpecFile`](crate::spec::RawSpecFile) — the type the JSON Schema is generated from, the CLI
/// parses and `examples/billing/` is written in. The two disagree about what `domains:` means: here
/// it is a list of domain objects carrying member *names*, there it is the header's roster of
/// domain *names*, so `examples/billing/system.yaml` does not parse as one of these. Two public
/// types that look like the same document and are not is a trap, and this one was reachable and
/// read by nothing, so it is compiled out of the shipped crate.
///
/// Unknown fields are refused — the component, interaction and topology layers (design §5, §7, §8)
/// are not modelled yet, and a document carrying them should be told so rather than have them
/// silently dropped.
#[cfg(test)]
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSystemSpec {
    /// The specification language, defaulting to `ess/1`.
    #[serde(default)]
    pub format: Option<FormatVersion>,
    /// The system's namespace. Present in exactly one source.
    #[serde(default, alias = "name")]
    pub system: Option<QualifiedName>,
    /// The system's own version, defaulting to `v1`.
    #[serde(default)]
    pub version: Option<Version>,
    /// The domains this source declares.
    #[serde(default)]
    pub domains: Vec<RawDomainSpec>,
    /// Types belonging to the system rather than to any one domain.
    #[serde(default)]
    pub types: Vec<crate::types::RawNamedType>,
    /// What the system is called on the wire, and what a person is shown.
    #[serde(default)]
    pub naming: Naming,
    /// What the system is, in one paragraph.
    #[serde(default)]
    pub summary: Option<String>,
}

#[cfg(test)]
impl RawSystemSpec {
    /// Turns one parsed document into the contribution it makes.
    ///
    /// The domains are validated here, where the document's own structure is still the subject.
    /// Everything that needs to see the other sources happens in [`SystemSpec::merge`].
    pub(crate) fn into_part(self, source: impl Into<Source>) -> Result<SpecPart, ValidationErrors> {
        let (part, errors) = self.split(source);
        errors.into_result(part)
    }

    /// What this document contributes, and everything wrong with it.
    ///
    /// Separate from [`RawSystemSpec::into_part`] so that a document with one unreadable domain
    /// still reaches [`SystemSpec::merge`]. Otherwise a misplaced command would hide an unsupported
    /// format, and the author would fix one problem only to be told about the next.
    fn split(self, source: impl Into<Source>) -> (SpecPart, ValidationErrors) {
        let source = source.into();
        let mut errors = ValidationErrors::new();

        if self.system.is_none()
            && (self.format.is_some()
                || self.version.is_some()
                || self.summary.is_some()
                || !self.naming.is_empty())
        {
            errors.push(
                // A required declaration is absent; nothing here is a reference.
                ValidationError::new(
                    ValidationCode::MissingDeclaration,
                    format!("{source}.system"),
                    "this source sets system-level fields but does not declare `system:`"
                        .to_owned(),
                )
                .with_hint(
                    "the source that carries the format and version is the one that names the \
                     system",
                ),
            );
        }

        let header = self.system.map(|name| SpecHeader {
            name,
            version: self.version.unwrap_or(Version::V1),
            format: self.format.unwrap_or(FormatVersion::V1),
            naming: self.naming,
            summary: self.summary,
        });

        let mut domains = Vec::with_capacity(self.domains.len());
        for raw in self.domains {
            match DomainSpec::try_from(raw) {
                Ok(domain) => domains.push(domain),
                Err(found) => errors.extend(found),
            }
        }

        let mut types = Vec::with_capacity(self.types.len());
        for raw in self.types {
            match NamedType::try_from(raw) {
                Ok(declared) => types.push(declared),
                Err(found) => errors.extend(found),
            }
        }

        (
            SpecPart {
                source,
                header,
                domains,
                types,
            },
            errors,
        )
    }
}

#[cfg(test)]
impl TryFrom<RawSystemSpec> for SystemSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawSystemSpec) -> Result<Self, Self::Error> {
        let (part, mut errors) = raw.split(Source::document());
        match Self::merge([part]) {
            Ok(system) => errors.into_result(system),
            Err(found) => {
                errors.extend(found);
                Err(errors)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Primitive, TypeBody};

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    fn system(yaml: &str) -> Result<SystemSpec, ValidationErrors> {
        let raw: RawSystemSpec = serde_yaml::from_str(yaml).expect("document parses");
        SystemSpec::try_from(raw)
    }

    /// Whatever a document was refused for, or nothing when it was accepted.
    fn system_errors(yaml: &str) -> ValidationErrors {
        system(yaml).err().unwrap_or_default()
    }

    fn part(source: &str, yaml: &str) -> SpecPart {
        let raw: RawSystemSpec = serde_yaml::from_str(yaml).expect("document parses");
        raw.into_part(source).expect("the document is well formed")
    }

    fn newtype(qualified: &str, of: Primitive) -> NamedType {
        NamedType {
            name: name(qualified),
            body: TypeBody::Newtype {
                of: TypeRef::Primitive(of),
                invariants: Vec::new(),
            },
            naming: Naming::default(),
        }
    }

    const BILLING: &str = r"
format: ess/1
system: billing
version: v1
summary: Invoicing, and the mail it sends.
domains:
  - domain: billing.invoice
    entities: [Invoice]
    commands: [CreateInvoice]
    events: [InvoiceCreated]
    views: [InvoiceById]
    errors: [InvalidAmount]
  - domain: billing.email
    commands: [SendEmail]
    events: [EmailSent]
";

    #[test]
    fn a_format_version_round_trips_and_nothing_else_is_read_as_one() {
        assert_eq!(FormatVersion::parse("ess/1").expect("parses").major(), 1);
        assert_eq!(FormatVersion::V1.to_string(), "ess/1");
        assert_eq!(
            FormatVersion::parse("ess/7").expect("parses").to_string(),
            "ess/7"
        );

        let wrong_language = FormatVersion::parse("openapi/3").expect_err("not an ESS document");
        assert!(
            wrong_language.to_string().contains("written `ess/1`"),
            "{wrong_language}"
        );
        assert!(
            FormatVersion::parse("ess/0").is_err(),
            "zero invites `unversioned`"
        );
        assert!(
            FormatVersion::parse("1").is_err(),
            "the prefix is part of the spelling"
        );
        assert!(FormatVersion::V1.is_supported());
        assert!(!FormatVersion::parse("ess/2")
            .expect("parses")
            .is_supported());
    }

    #[test]
    fn a_document_in_a_later_format_is_refused_rather_than_guessed_at() {
        let errors = system(
            r"
format: ess/2
system: billing
",
        )
        .expect_err("a format this build does not implement");

        assert!(
            errors.contains(ValidationCode::UnsupportedFormatVersion),
            "{errors}"
        );
        let error = &errors.as_slice()[0];
        assert_eq!(error.location, "system.format");
        assert!(error.message.contains("ess/2"), "{error}");
        assert!(
            error
                .hint
                .as_deref()
                .expect("a hint")
                .contains("upgrade the tooling"),
            "the remedy is to upgrade, never to reinterpret: {error}"
        );
    }

    #[test]
    fn a_system_is_assembled_from_one_document() {
        let spec = system(BILLING).expect("validates");

        assert_eq!(spec.reference(), "billing/v1");
        assert_eq!(spec.format, FormatVersion::V1);
        assert_eq!(spec.domains.len(), 2);
        assert_eq!(
            spec.summary.as_deref(),
            Some("Invoicing, and the mail it sends.")
        );
        assert_eq!(
            spec.domain(&name("billing.invoice"))
                .expect("declared")
                .commands,
            vec![name("billing.invoice.CreateInvoice")]
        );
        assert!(spec.domain(&name("billing.shipping")).is_none());
    }

    #[test]
    fn owner_of_answers_for_a_name_in_each_domain_and_for_one_that_is_owned_by_nobody() {
        let spec = system(BILLING).expect("validates");

        assert_eq!(
            spec.owner_of(&name("billing.invoice.CreateInvoice"))
                .map(|domain| domain.name.to_string()),
            Some("billing.invoice".to_owned())
        );
        assert_eq!(
            spec.owner_of(&name("billing.email.EmailSent"))
                .map(|domain| domain.name.to_string()),
            Some("billing.email".to_owned())
        );
        assert!(
            spec.owner_of(&name("billing.invoice.CancelInvoice"))
                .is_none(),
            "a name inside a domain's namespace that nobody declared has no owner; that is what \
             makes a dangling reference visible"
        );
        assert!(
            spec.owner_of(&name("billing.invoice")).is_none(),
            "a domain is not one of its own members"
        );
        assert_eq!(
            spec.declaration_of(&name("billing.invoice.InvoiceById"))
                .map(|(_, kind)| kind),
            Some(MemberKind::View)
        );
    }

    #[test]
    fn a_name_declared_in_two_domains_is_refused() {
        // Built rather than parsed: read from a document, `billing.email` claiming a
        // `billing.invoice` name is refused one step earlier, by the namespace rule. That is the
        // point — the two rules together leave no way for one name to have two owners.
        let invoice = DomainSpec {
            events: vec![name("billing.invoice.InvoiceCreated")],
            ..DomainSpec::new(name("billing.invoice"))
        };
        let email = DomainSpec {
            events: vec![name("billing.invoice.InvoiceCreated")],
            ..DomainSpec::new(name("billing.email"))
        };

        let errors = Manifest::new()
            .with(part("system.yaml", "system: billing"))
            .with(SpecPart::new("domains/invoice.yaml").with_domain(invoice))
            .with(SpecPart::new("domains/email.yaml").with_domain(email))
            .compile()
            .expect_err("two owners for one event");

        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        let error = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::DuplicateDeclaration)
            .expect("the contested claim");
        assert!(
            error.message.contains("billing.invoice.InvoiceCreated"),
            "the refusal names what is contested: {error}"
        );
        let hint = error.hint.as_deref().expect("a hint");
        assert!(
            hint.contains("`billing.invoice`") && hint.contains("`billing.email`"),
            "and both claimants: {error}"
        );
    }

    #[test]
    fn a_member_outside_its_domains_namespace_is_refused() {
        let errors = system(
            r"
system: billing
domains:
  - domain: billing.invoice
    commands: [billing.email.SendEmail]
",
        )
        .expect_err("not this domain's command");

        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
        assert!(
            errors
                .to_string()
                .contains("is not inside `billing.invoice`"),
            "{errors}"
        );
    }

    #[test]
    fn a_domain_outside_the_system_is_refused() {
        let errors = system(
            r"
system: billing
domains:
  - domain: shipping.parcel
    commands: [DispatchParcel]
",
        )
        .expect_err("not this system's domain");

        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
        assert!(
            errors
                .to_string()
                .contains("is not inside the system `billing`"),
            "{errors}"
        );
    }

    #[test]
    fn a_domain_that_is_the_system_itself_is_refused() {
        let errors = system(
            r"
system: billing
domains:
  - domain: billing
    commands: [CreateInvoice]
",
        )
        .expect_err("the system is not one of its own domains");

        assert!(errors.contains(ValidationCode::SelfReference), "{errors}");
        assert!(
            errors.to_string().contains("is the system itself"),
            "{errors}"
        );
    }

    #[test]
    fn every_domains_types_land_in_one_registry() {
        let spec = system(
            r"
system: billing
types:
  - name: billing.Money
    kind: newtype
    of: Decimal
domains:
  - domain: billing.invoice
    types:
      - name: InvoiceId
        kind: newtype
        of: Uuid
  - domain: billing.email
    types:
      - name: Address
        kind: newtype
        of: String
",
        )
        .expect("validates");

        assert_eq!(
            spec.types.len(),
            3,
            "system-level and domain-local, in one index"
        );
        assert!(spec.types.get(&name("billing.invoice.InvoiceId")).is_some());
        assert!(spec.types.get(&name("billing.email.Address")).is_some());
        assert!(spec.types.get(&name("billing.Money")).is_some());
        assert_eq!(
            spec.owner_of(&name("billing.Money")),
            None,
            "a system-level type belongs to no domain"
        );
    }

    #[test]
    fn a_type_reference_that_resolves_to_nothing_is_refused() {
        let errors = system(
            r"
system: billing
domains:
  - domain: billing.invoice
    types:
      - name: Line
        kind: struct
        fields:
          - name: amount
            type: billing.Money
",
        )
        .expect_err("`billing.Money` was never declared");

        assert!(
            errors.contains(ValidationCode::UndeclaredReference),
            "{errors}"
        );
        let error = &errors.as_slice()[0];
        assert_eq!(error.location, "types.billing.invoice.Line");
        assert!(error.message.contains("billing.Money"), "{error}");
    }

    #[test]
    fn two_partial_specifications_merge_into_one_graph() {
        let manifest = Manifest::new()
            .with(part(
                "system.yaml",
                r"
format: ess/1
system: billing
version: v2
",
            ))
            .with(part(
                "domains/invoice.yaml",
                r"
domains:
  - domain: billing.invoice
    commands: [CreateInvoice]
    events: [InvoiceCreated]
",
            ))
            .with(part(
                "domains/email.yaml",
                r"
domains:
  - domain: billing.email
    commands: [SendEmail]
",
            ));

        assert_eq!(manifest.len(), 3);
        assert_eq!(
            manifest
                .sources()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["system.yaml", "domains/invoice.yaml", "domains/email.yaml"]
        );

        let spec = manifest.compile().expect("three files, one graph");
        assert_eq!(spec.reference(), "billing/v2");
        assert_eq!(spec.domains.len(), 2);
        assert_eq!(
            spec.owner_of(&name("billing.email.SendEmail"))
                .map(|domain| domain.name.to_string()),
            Some("billing.email".to_owned())
        );
    }

    #[test]
    fn two_sources_may_contribute_to_the_same_domain() {
        let spec = Manifest::new()
            .with(part("system.yaml", "system: billing"))
            .with(part(
                "domains/invoice.yaml",
                r"
domains:
  - domain: billing.invoice
    commands: [CreateInvoice]
",
            ))
            .with(part(
                "domains/invoice-tax.yaml",
                r"
domains:
  - domain: billing.invoice
    commands: [ApplyTax]
",
            ))
            .compile()
            .expect("one domain, split across two files, is the point of \u{a7}24");

        let invoice = spec.domain(&name("billing.invoice")).expect("declared");
        assert_eq!(
            invoice.commands,
            vec![
                name("billing.invoice.CreateInvoice"),
                name("billing.invoice.ApplyTax")
            ]
        );
        assert_eq!(spec.domains.len(), 1, "one domain, not two");
    }

    #[test]
    fn the_same_command_declared_by_two_sources_is_refused_naming_both() {
        let errors = Manifest::new()
            .with(part("system.yaml", "system: billing"))
            .with(part(
                "domains/invoice.yaml",
                r"
domains:
  - domain: billing.invoice
    commands: [CreateInvoice]
",
            ))
            .with(part(
                "domains/invoice-tax.yaml",
                r"
domains:
  - domain: billing.invoice
    commands: [CreateInvoice]
",
            ))
            .compile()
            .expect_err("one command, two files");

        assert_eq!(errors.len(), 1, "one conflict, reported once: {errors}");
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::DuplicateDeclaration);
        assert_eq!(error.location, "domain billing.invoice.commands");
        assert!(
            error.message.contains("`domains/invoice.yaml`")
                && error.message.contains("`domains/invoice-tax.yaml`"),
            "a cross-file conflict is only fixable if the message names both files: {error}"
        );
    }

    #[test]
    fn a_system_level_type_declared_by_two_sources_is_refused_naming_both() {
        let errors = Manifest::new()
            .with(part("system.yaml", "system: billing"))
            .with(
                SpecPart::new("types/money.yaml")
                    .with_type(newtype("billing.Money", Primitive::Decimal)),
            )
            .with(
                SpecPart::new("types/money-again.yaml")
                    .with_type(newtype("billing.Money", Primitive::Decimal)),
            )
            .compile()
            .expect_err("one type, two files");

        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        let rendered = errors.to_string();
        assert!(rendered.contains("`types/money.yaml`"), "{rendered}");
        assert!(rendered.contains("`types/money-again.yaml`"), "{rendered}");
        assert!(
            rendered.contains("the system itself"),
            "and says who claims it: {rendered}"
        );
    }

    #[test]
    fn a_specification_with_no_header_is_refused() {
        let errors = Manifest::new()
            .with(part(
                "domains/invoice.yaml",
                r"
domains:
  - domain: billing.invoice
    commands: [CreateInvoice]
",
            ))
            .compile()
            .expect_err("nothing says what system this is");

        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(
            error.code,
            ValidationCode::MissingDeclaration,
            "nothing is referenced here; a required declaration is missing: {error}"
        );
        assert_eq!(error.location, "system");
        assert!(
            error.message.contains("no source declares the system"),
            "{error}"
        );
    }

    #[test]
    fn two_headers_are_refused_naming_both_sources() {
        let errors = Manifest::new()
            .with(part("system.yaml", "system: billing"))
            .with(part("other.yaml", "system: shipping"))
            .compile()
            .expect_err("two systems in one specification");

        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`system.yaml`") && rendered.contains("`other.yaml`"),
            "{rendered}"
        );
        assert!(rendered.contains("shipping"), "{rendered}");
    }

    #[test]
    fn a_fragment_that_sets_system_level_fields_without_naming_the_system_is_refused() {
        let raw: RawSystemSpec =
            serde_yaml::from_str("format: ess/1\nversion: v2\n").expect("document parses");
        let errors = raw
            .into_part("domains/invoice.yaml")
            .expect_err("a format without a system");

        assert!(
            errors.contains(ValidationCode::MissingDeclaration),
            "{errors}"
        );
        assert_eq!(errors.as_slice()[0].location, "domains/invoice.yaml.system");
    }

    #[test]
    fn a_specification_reports_every_problem_in_one_run() {
        let errors = system(
            r"
format: ess/9
system: billing
domains:
  - domain: shipping.parcel
    commands: [DispatchParcel]
  - domain: billing.invoice
    commands: [billing.email.SendEmail]
",
        )
        .expect_err("three unrelated problems");

        assert!(errors.len() >= 3, "one run, every problem: {errors}");
        assert!(
            errors.contains(ValidationCode::UnsupportedFormatVersion),
            "{errors}"
        );
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
    }

    #[test]
    fn a_wire_name_change_does_not_move_the_system() {
        let spec = system(
            r"
system: billing
naming:
  wire: billing-api
  display: Billing
domains:
  - domain: billing.invoice
    naming:
      wire: invoices
",
        )
        .expect("validates");

        assert_eq!(spec.naming.wire_or(&spec.name), "billing-api");
        assert_eq!(spec.naming.display_or(&spec.name), "Billing");
        assert_eq!(
            spec.name.to_string(),
            "billing",
            "the identity is untouched"
        );
        let invoice = spec.domain(&name("billing.invoice")).expect("declared");
        assert_eq!(invoice.naming.wire_or(&invoice.name), "invoices");
    }

    #[test]
    fn two_types_that_cannot_be_built_without_each_other_are_refused() {
        let errors = system(
            r"
system: billing
types:
  - name: billing.Loop
    kind: newtype
    of: billing.Loop2
  - name: billing.Loop2
    kind: newtype
    of: billing.Loop
",
        )
        .expect_err("neither has a value, and a generator over them does not terminate");

        assert!(errors.contains(ValidationCode::SelfReference), "{errors}");
        let error = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::SelfReference)
            .expect("the cycle");
        assert!(
            error.message.contains("`billing.Loop`") && error.message.contains("`billing.Loop2`"),
            "the refusal names the way round: {error}"
        );
    }

    #[test]
    fn an_expression_tree_is_not_a_forbidden_cycle() {
        // The defect this rule used to have. `Expr` reaches itself through `Pair`, so a check that
        // refused every cycle of required names refused it — but every value of `Expr` bottoms out
        // in a `leaf`, so the type is perfectly ordinary and a generator over it terminates. A union
        // needs *one* buildable variant, not all of them.
        let errors = system_errors(
            r"
system: billing
version: v1
types:
  - name: billing.Expr
    kind: union
    tag: kind
    variants:
      leaf: Integer
      pair: billing.Pair
  - name: billing.Pair
    kind: struct
    fields:
      - name: left
        type: billing.Expr
      - name: right
        type: billing.Expr
",
        );
        assert!(
            !errors.contains(ValidationCode::SelfReference),
            "an expression tree is a legitimate specification: {errors}"
        );
    }

    #[test]
    fn a_union_whose_every_variant_recurses_is_refused() {
        // The other side of the same rule, so it cannot be loosened into accepting everything.
        let errors = system_errors(
            r"
system: billing
version: v1
types:
  - name: billing.Endless
    kind: union
    tag: kind
    variants:
      left: billing.Endless
      right: billing.Pair
  - name: billing.Pair
    kind: struct
    fields:
      - name: only
        type: billing.Endless
",
        );
        assert!(errors.contains(ValidationCode::SelfReference), "{errors}");
    }

    #[test]
    fn a_type_that_wraps_itself_is_refused_once_and_not_followed() {
        let errors = system(
            r"
system: billing
types:
  - name: billing.Knot
    kind: struct
    fields:
      - name: inner
        type: billing.Knot
",
        )
        .expect_err("a struct that contains itself has no smallest value");

        assert_eq!(errors.len(), 1, "one cycle, reported once: {errors}");
        assert_eq!(errors.as_slice()[0].code, ValidationCode::SelfReference);
        assert_eq!(errors.as_slice()[0].location, "types.billing.Knot");
    }

    #[test]
    fn a_type_that_reaches_itself_only_through_a_base_case_is_allowed() {
        let spec = system(
            r"
system: billing
types:
  - name: billing.Node
    kind: struct
    fields:
      - name: parent
        type: Optional<billing.Node>
      - name: children
        type: List<billing.Node>
",
        )
        .expect("an absent value and an empty list are values, so the type has some");

        assert_eq!(spec.types.len(), 1);
    }

    #[test]
    fn an_actor_declared_by_a_domain_is_claimed_like_every_other_member() {
        let spec = system(
            r"
system: billing
domains:
  - domain: billing.invoice
    actors: [Customer]
",
        )
        .expect("validates");

        assert_eq!(
            spec.owner_of(&name("billing.invoice.Customer"))
                .map(|domain| domain.name.to_string()),
            Some("billing.invoice".to_owned()),
            "a merge that dropped actors would leave every one of them owned by nobody"
        );
        assert_eq!(
            spec.declaration_of(&name("billing.invoice.Customer"))
                .map(|(_, kind)| kind),
            Some(MemberKind::Actor)
        );
    }

    #[test]
    fn a_type_declared_by_a_domain_and_by_the_system_has_one_owner() {
        let errors = system(
            r"
system: billing
types:
  - name: billing.invoice.Money
    kind: newtype
    of: Decimal
domains:
  - domain: billing.invoice
    types:
      - name: Money
        kind: newtype
        of: Decimal
",
        )
        .expect_err("one type, two claimants");

        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("the system itself"),
            "the message says who else claims it: {errors}"
        );
    }
}

#[cfg(test)]
mod format_version_tests {
    use super::FormatVersion;

    #[test]
    fn the_format_pattern_accepts_exactly_what_the_parser_accepts() {
        // The pattern is what an editor enforces; the parser is what the build enforces. A document
        // one accepts and the other refuses is a contradiction this repository published itself.
        let matches = |value: &str| {
            value
                .strip_prefix(FormatVersion::PREFIX)
                .is_some_and(|digits| {
                    let mut characters = digits.chars();
                    characters
                        .next()
                        .is_some_and(|first| first.is_ascii_digit() && first != '0')
                        && characters.all(|character| character.is_ascii_digit())
                })
        };

        for accepted in ["ess/1", "ess/2", "ess/10"] {
            assert!(
                FormatVersion::parse(accepted).is_ok(),
                "{accepted} should parse"
            );
            assert!(
                matches(accepted),
                "the schema rejects `{accepted}`, which parses"
            );
        }
        for refused in [
            "ess/0", "ess/01", "ess/+1", "ess/", "1", "", "esss/1", "ess/1.0",
        ] {
            assert!(
                FormatVersion::parse(refused).is_err(),
                "{refused} should not parse"
            );
            assert!(
                !matches(refused),
                "the schema accepts `{refused}`, which the parser refuses"
            );
        }
    }
}
