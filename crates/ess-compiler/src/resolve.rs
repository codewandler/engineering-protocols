//! Turning a validated specification into the IR, or into every reason it cannot be.
//!
//! Accumulating, like every other pass in this repository: an author who has to re-run the compiler
//! to discover the second problem is an author running it ten times to learn what one pass knew.
//!
//! # Who refuses what
//!
//! Every cross-cutting rule in this model is enforced in [`ess_domain`], by
//! [`Specification::validate`] — undeclared events, view sources, ownership, component surfaces,
//! topology, and a binding's mapping. That is where they were written and where they are tested, and
//! this pass does not restate them. What it adds is two things they never had: a **code** and a
//! **`file:line`**, through [`diagnose`].
//!
//! This pass refuses exactly one category on its own account: **a reference it has to resolve in
//! order to mint a handle.** An entity's identity type, a view's source entity, an actor's grant:
//! there is no `EntityHandle` to put in a view for a name nothing declares, so the reference is
//! resolved here and refused here, under the code `ess-domain` uses for the same defect. [`compile`] cannot put a `CommandHandle` in the IR for a name nothing
//! declares, so it says so, using the same code the bridge would have produced for the same defect
//! at the same location. One defect keeps one code whichever half noticed it, and in the file
//! pipeline `assemble` fails first, so an author never sees two messages.
//!
//! Where the two overlap in *implementation* — a type reference, a command's events, a mapping's
//! types — the overlap is forced: a `Specification` is a struct with public fields, so "the domain
//! crate already checked this" is a convention, and replacing conventions with types is what this
//! crate is for. Where it is not forced, this pass does not check. An uninhabitable type is
//! representable in the IR, so it is `ess-domain`'s alone.
//!
//! For the layers whose references this pass delegates entirely — components and the topology — an
//! unresolvable reference makes the compilation *off-contract*: the domain crate's verdict on those
//! two layers is bridged in, so the IR never comes back quietly missing a component.
//!
//! # Cascades are suppressed
//!
//! A reference to something that *is* declared but did not resolve produces no diagnostic of its
//! own. The reason it did not resolve was reported where it happened, and a component reported as
//! "accepts an undeclared command" because that command's input names a misspelt type sends the
//! reader to the wrong file.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use aep_domain::error::{ValidationCode, ValidationErrors};
use ess_domain::actor::ActorSpec;
use ess_domain::binding::{BindingName, BindingSpec, MappingSource};
use ess_domain::command::{
    CommandSpec, Effect, ErrorSpec, EventSpec, Outcome, OutcomeCondition, Subject,
};
use ess_domain::component::{ComponentName, ComponentSpec};
use ess_domain::entity::{EntitySpec, StateMachine};
use ess_domain::name::QualifiedName;
use ess_domain::spec::Specification;
use ess_domain::system::Source;
use ess_domain::topology::Workload;
use ess_domain::types::{is_assignable, Field, NamedType, TypeBody, TypeRef, TypeRegistry};
use ess_domain::view::ViewSpec;

use crate::diagnostic::{Code, Detail, Diagnostic, Diagnostics, Severity};
use crate::ir::{
    ActorHandle, CommandHandle, ComponentHandle, DomainHandle, EntityHandle, ErrorHandle, EssIr,
    EventHandle, ResolvedActor, ResolvedBinding, ResolvedBody, ResolvedCommand, ResolvedComponent,
    ResolvedCondition, ResolvedConversion, ResolvedDomain, ResolvedEffect, ResolvedEntity,
    ResolvedError, ResolvedEvent, ResolvedField, ResolvedMapping, ResolvedMappingValue,
    ResolvedOutcome, ResolvedSubject, ResolvedType, ResolvedTypeRef, ResolvedView,
    ResolvedWorkload, TypeHandle, ViewHandle,
};
use crate::source::{Location, SourceMap, Span};

/// The code space, and every rejection either half of the compiler reports.
///
/// A code is a **pair of closed enumerations**: [`codes::family`] says which layer the defect is in,
/// [`codes::class`] says what kind of defect it is.
///
/// ```text
/// ESS-BINDING-002
///     ^^^^^^^ family::BINDING — where
///             ^^^ class::TYPE_MISMATCH — what
/// ```
///
/// That is what stops two rules colliding into one code: a number is not something a reader of this
/// file picks, it is one of twelve, and the build refuses two named codes with the same pair
/// outright (see the `const` assertion below).
///
/// It is also what lets **one defect keep one code whichever half found it**. `ess-domain` refuses a
/// mapping whose two types disagree; so does [`compile`], when it is handed a `Specification` that
/// crate never validated. Both are `ESS-BINDING-002`, because [`diagnose`] maps
/// `type_mismatch` in a `binding.…` location onto the same pair. A consumer matching on codes cannot
/// tell which half ran, and does not need to.
///
/// # Design §20, and who enforces it
///
/// | design §20 | code | rule lives in |
/// |---|---|---|
/// | references to missing types | `ESS-<layer>-001` | `ess-domain`; also here, because a handle cannot be minted without it |
/// | events referencing undefined values | [`EVENT_UNDECLARED_REFERENCE`](codes::EVENT_UNDECLARED_REFERENCE) | both, as above |
/// | commands referencing undefined events | [`COMMAND_UNDECLARED_REFERENCE`](codes::COMMAND_UNDECLARED_REFERENCE) | both, as above |
/// | invalid source/target binding mappings | [`MAPPING_TYPE_MISMATCH`](codes::MAPPING_TYPE_MISMATCH) and its neighbours | both, as above |
/// | components accepting undefined commands | `ESS-COMPONENT-001` | `ess-domain`, `validate_components` |
/// | topology references to missing components | `ESS-TOPOLOGY-001` | `ess-domain`, `validate_topology` |
/// | forbidden dependency cycles | [`UNINHABITABLE_TYPE`](codes::UNINHABITABLE_TYPE) | `ess-domain`, as `self_reference` |
/// | unreachable states, invalid transitions | `ESS-ENTITY-011` | `ess-domain`, wave 1; delegated here |
/// | views exposing missing fields | `ESS-VIEW-001` | `ess-domain`; also here, for the source and the projected field types |
/// | contradictory invariants | — | nowhere; see below |
///
/// Every §20 bullet except the last is enforced. Most are enforced in `ess-domain`, where the rule
/// was already tested, and reach a reader with a code and a `file:line` through
/// [`diagnose`]. The ones this pass also checks are the ones it cannot do its own
/// job without: it has to *mint a handle*, and it has none to mint for a name nothing declares.
///
/// # Entities, views and actors
///
/// The three constructs added in wave 2 divide the same way, and neither half gained a rule:
///
/// | reference | who resolves it | code |
/// |---|---|---|
/// | an entity's identity type and field types | here, to mint [`TypeHandle`]s | [`ENTITY_UNDECLARED_REFERENCE`](codes::ENTITY_UNDECLARED_REFERENCE) |
/// | a view's source entity, and its projected field types | here, to mint an [`EntityHandle`] and [`TypeHandle`]s | [`VIEW_UNDECLARED_REFERENCE`](codes::VIEW_UNDECLARED_REFERENCE) |
/// | an actor's `may` grant | here, to mint a [`CommandHandle`] | [`ACTOR_UNDECLARED_REFERENCE`](codes::ACTOR_UNDECLARED_REFERENCE) |
/// | an outcome's subject: the entity it changes, and the transition it takes | here, to mint an [`EntityHandle`] and to carry the move itself | [`COMMAND_UNDECLARED_REFERENCE`](codes::COMMAND_UNDECLARED_REFERENCE) |
/// | a transition no outcome takes | `ess-domain`, `validate_lifecycle_causes` | `ESS-ENTITY-005`, as `missing_causation` |
/// | the event a binding escalates with | here, to mint an [`EventHandle`] | [`BINDING_UNDECLARED_REFERENCE`](codes::BINDING_UNDECLARED_REFERENCE) |
/// | an `escalate` that names no event at all | `ess-domain`, `BindingSpec::validate` | `ESS-BINDING-005`, as `missing_declaration` |
/// | a projected field the source entity does not have, or whose type disagrees with it | `ess-domain`, `ViewSpec::validate` | `ESS-VIEW-001`, `ESS-VIEW-002` |
/// | a lifecycle's states: unknown, unreachable, dead-ended, duplicated | `ess-domain`, `StateMachine::validate_at` | `ESS-ENTITY-011`, `ESS-ENTITY-006` |
/// | an invariant reading a field the entity does not have | `ess-domain`, `EntitySpec::validate` | `ESS-ENTITY-003` |
///
/// An actor grant is not a §20 bullet of its own. It is refused as a reference with nothing behind
/// it — the same reading `ess-domain`'s `ActorSpec::validate` takes, which is why both produce
/// `ESS-ACTOR-001` and a consumer cannot tell which half ran.
///
/// A lifecycle naming a state it does not declare is refused by *nothing here*. It is settled when
/// `RawStateMachine` is converted, so an assembled specification cannot carry one; a `Specification`
/// built field by field can, and that makes the compilation off-contract — the entity stays out of
/// the IR and `Resolver::report_off_contract` says so — rather than being restated as a rule with a
/// second code.
///
/// Contradictory invariants are not refused by anything. Deciding that `amount >= 0` and
/// `amount < 0` cannot both hold needs a solver; a cheap syntactic subset would refuse some
/// contradictions, miss most, and leave an author no way to predict which — worse than a stated gap.
pub mod codes {
    use crate::diagnostic::Code;

    /// Which layer a defect is in — the `BINDING` in `ESS-BINDING-002`.
    pub mod family {
        /// The specification as a whole, or something with no better home.
        pub const SPEC: &str = "SPEC";
        /// A bounded context.
        pub const DOMAIN: &str = "DOMAIN";
        /// A named type, or a declared conversion between two.
        pub const TYPE: &str = "TYPE";
        /// An entity or its lifecycle.
        pub const ENTITY: &str = "ENTITY";
        /// A command.
        pub const COMMAND: &str = "COMMAND";
        /// An event.
        pub const EVENT: &str = "EVENT";
        /// A declared error.
        pub const ERROR: &str = "ERROR";
        /// A view.
        pub const VIEW: &str = "VIEW";
        /// An actor.
        pub const ACTOR: &str = "ACTOR";
        /// A binding, including its mapping.
        pub const BINDING: &str = "BINDING";
        /// A component.
        pub const COMPONENT: &str = "COMPONENT";
        /// The topology.
        pub const TOPOLOGY: &str = "TOPOLOGY";

        /// Every family, in the order a specification is read.
        pub const ALL: &[&str] = &[
            SPEC, DOMAIN, TYPE, ENTITY, COMMAND, EVENT, ERROR, VIEW, ACTOR, BINDING, COMPONENT,
            TOPOLOGY,
        ];
    }

    /// What kind of defect it is — the `002` in `ESS-BINDING-002`.
    ///
    /// One number per kind, across every family, so `-002` means the same thing in a binding as in a
    /// view. A reader learns twelve numbers once.
    pub mod class {
        /// It names something nothing declares.
        pub const UNDECLARED: u16 = 1;
        /// Two types that have to agree do not, and no conversion says they may.
        pub const TYPE_MISMATCH: u16 = 2;
        /// It reads something its subject does not have.
        pub const UNREADABLE: u16 = 3;
        /// Two declarations contradict each other.
        pub const CONFLICT: u16 = 4;
        /// Something required is absent.
        pub const MISSING: u16 = 5;
        /// The same thing is declared twice.
        pub const DUPLICATE: u16 = 6;
        /// It declares nothing, so it names nothing.
        pub const EMPTY: u16 = 7;
        /// It requires itself, so no value of it can exist.
        pub const SELF_REFERENCE: u16 = 8;
        /// A construct or a version this build does not implement.
        pub const UNSUPPORTED: u16 = 9;
        /// It nearly names something that is declared.
        pub const MISSPELLED: u16 = 10;
        /// A lifecycle that cannot run: a dead end, an unreachable state, an unknown one.
        pub const LIFECYCLE: u16 = 11;
        /// A rule with no class of its own yet.
        ///
        /// The bridge maps a `#[non_exhaustive]` enum, so this exists to keep that mapping total
        /// rather than to be used.
        pub const OTHER: u16 = 12;

        /// Every class, in code order.
        pub const ALL: &[u16] = &[
            UNDECLARED,
            TYPE_MISMATCH,
            UNREADABLE,
            CONFLICT,
            MISSING,
            DUPLICATE,
            EMPTY,
            SELF_REFERENCE,
            UNSUPPORTED,
            MISSPELLED,
            LIFECYCLE,
            OTHER,
        ];
    }

    /// Declares each named code once, and builds [`ALL`] from the same line, so the list cannot fall
    /// behind the constants — the failure `aep-domain`'s `validation_codes!` macro exists to
    /// prevent, where five codes were emitted and missing from the list the tests iterate.
    macro_rules! codes {
        (
            $(
                $(#[$attribute:meta])*
                $name:ident = $family:expr, $class:expr;
            )*
        ) => {
            $(
                $(#[$attribute])*
                pub const $name: Code = Code::new($family, $class);
            )*

            /// Every code with a rule of its own, in declaration order.
            ///
            /// Not every code in the space: [`diagnose`](super::diagnose) can produce any
            /// [`family`] paired with any [`class`], because it carries `ess-domain`'s whole rule set
            /// across. These are the ones this crate documents a rule for.
            pub const ALL: &[Code] = &[ $( $name, )* ];
        };
    }

    codes! {
        /// A reference in a layer `ess-domain` validates does not resolve, and that crate reported
        /// nothing — so this pass cannot build the handle and says only that.
        ///
        /// A backstop. Reaching it means a `Specification` was built field by field and is
        /// inconsistent in a way `Specification::validate` does not check.
        UNVALIDATED_SPECIFICATION = family::SPEC, class::UNDECLARED;

        /// A type, or a declared conversion, names a type nothing declares.
        UNDECLARED_TYPE = family::TYPE, class::UNDECLARED;

        /// No value of this type can exist: it requires itself, with nothing that can terminate the
        /// recursion.
        ///
        /// Bridged from `ess-domain`'s `self_reference`, which owns the rule. Design §20's
        /// "forbidden dependency cycles", read on the type graph — and read as inhabitability
        /// rather than as cycles, because a cycle is not the defect: a union whose other variant is
        /// an `Integer` may refer to itself all it likes.
        UNINHABITABLE_TYPE = family::TYPE, class::SELF_REFERENCE;

        /// An entity names a type, or a domain, that nothing declares.
        ///
        /// Its identity's type and every field's type, because each becomes a `TypeHandle`. Its
        /// lifecycle is not in here: a state is not a reference out of the entity, so there is no
        /// handle to mint for one.
        ENTITY_UNDECLARED_REFERENCE = family::ENTITY, class::UNDECLARED;

        /// A view names a source entity, a type, or a domain, that nothing declares.
        VIEW_UNDECLARED_REFERENCE = family::VIEW, class::UNDECLARED;

        /// An actor grants a command, or names a domain, that nothing declares.
        ///
        /// The failure worth catching about a grant: it reads as an authorization decision and
        /// authorizes nothing, because nothing refuses a request on account of it and no generated
        /// permission matrix has a row for it.
        ACTOR_UNDECLARED_REFERENCE = family::ACTOR, class::UNDECLARED;

        /// A command names an event, an error, a type or a domain that nothing declares.
        ///
        /// One code, because the repair is one thing — fix the name — and which name it is arrives
        /// as a field: [`Detail::Undeclared`](crate::diagnostic::Detail::Undeclared) carries what
        /// was looked for and what was available.
        COMMAND_UNDECLARED_REFERENCE = family::COMMAND, class::UNDECLARED;

        /// An event names a type, or a domain, that nothing declares.
        EVENT_UNDECLARED_REFERENCE = family::EVENT, class::UNDECLARED;

        /// An error names a type, or a domain, that nothing declares.
        ERROR_UNDECLARED_REFERENCE = family::ERROR, class::UNDECLARED;

        /// A binding names an event, a command, or a command input, that nothing declares.
        ///
        /// Two events can be named: the one it reacts to, and the one its `escalate` emits. Which
        /// it was arrives as a field rather than as a second code, because the repair is the same
        /// one — fix the name, or declare the event.
        BINDING_UNDECLARED_REFERENCE = family::BINDING, class::UNDECLARED;

        /// A mapping's two types differ and no conversion between them is declared.
        ///
        /// The pair lands on `ESS-BINDING-002`, which is design §29's worked example of a diagnostic
        /// a coding agent can repair from. That is not a coincidence arranged by numbering: `002` is
        /// [`class::TYPE_MISMATCH`] everywhere.
        MAPPING_TYPE_MISMATCH = family::BINDING, class::TYPE_MISMATCH;

        /// A mapping reads a field the triggering event does not carry.
        MAPPING_READS_UNDECLARED_FIELD = family::BINDING, class::UNREADABLE;

        /// A required input of the invoked command is left unmapped.
        ///
        /// It shares `ESS-BINDING-005` with `ess-domain`'s other `missing_declaration` about a
        /// binding — an `escalate` that names no event — because a code names a *kind* of defect
        /// and not a rule: both are a key the document did not write that what it did write makes
        /// required, and both are repaired by writing it.
        UNMAPPED_COMMAND_INPUT = family::BINDING, class::MISSING;
    }

    /// `true` when two families are the same string.
    ///
    /// Hand-rolled because comparing `str` is not available in a `const fn`, and the check below has
    /// to run at compile time to be worth anything.
    const fn same(left: &str, right: &str) -> bool {
        let (left, right) = (left.as_bytes(), right.as_bytes());
        if left.len() != right.len() {
            return false;
        }
        let mut index = 0;
        while index < left.len() {
            if left[index] != right[index] {
                return false;
            }
            index += 1;
        }
        true
    }

    /// `true` when no two codes share a family and a number.
    const fn distinct(codes: &[Code]) -> bool {
        let mut outer = 0;
        while outer < codes.len() {
            let mut inner = outer + 1;
            while inner < codes.len() {
                if codes[outer].number == codes[inner].number
                    && same(codes[outer].family, codes[inner].family)
                {
                    return false;
                }
                inner += 1;
            }
            outer += 1;
        }
        true
    }

    // A collision is a compile error, not a test failure. Two rules under one code is invisible to
    // the harness that matches on codes — it would report the wrong repair and be right about the
    // code — so the build refuses to produce it at all.
    const _: () = assert!(distinct(ALL), "two ESS diagnostic codes collide");
}

/// Where a declaration is written, when that can be answered honestly.
///
/// `serde_yaml` gives a line for a *syntax* error and nothing for a semantic one, so the line has to
/// be recovered by finding the declaration's own text in the file it was read from. That is a
/// heuristic, and [`crate::source`]'s honesty rule is enforced mechanically here: a needle is
/// located **only when it occurs exactly once across every file searched**. Two occurrences means
/// the compiler cannot tell which the author meant, and a confidently wrong line is worse than none,
/// because the reader edits there.
///
/// Needles are supplied most-specific-first — the misspelt type reference, then the declaration that
/// contains it. Falling back to the declaration's line is coarse; it is never wrong.
pub struct Locator<'a> {
    sources: &'a SourceMap,
    labels: Vec<String>,
    /// What [`Self::unique`] already answered.
    ///
    /// One needle costs a scan of every registered file's whole text, and the search cannot stop
    /// early: "occurs exactly once *across all files*" is only known once every file has been read
    /// to the end. Validation accumulates (invariant 3), so a specification with one badly named
    /// type produces one diagnostic per use of it, and every one of those asks for the same needle —
    /// which is where the repeated work actually is. A [`BTreeMap`], like everything else in this
    /// crate: an unordered map is banned here outright (invariant 9, and
    /// `tests/billing.rs` reads the sources for one), and a memo is no exception even though
    /// nothing iterates it.
    ///
    /// This removes the repeats. It does not make the search sublinear: *n* distinct needles still
    /// cost *n* passes over the sources, and fixing that needs a substring index rather than a
    /// cache.
    located: RefCell<BTreeMap<String, Option<(String, Location)>>>,
}

impl<'a> Locator<'a> {
    /// Searches these files.
    ///
    /// The labels are the keys the [`SourceMap`] was filled with. They are passed in because a
    /// [`SourceMap`] cannot currently be enumerated; with no labels every span still carries its
    /// document path, just not a line.
    pub fn new(sources: &'a SourceMap, labels: &[impl AsRef<str>]) -> Self {
        Self {
            sources,
            labels: labels
                .iter()
                .map(|label| label.as_ref().to_owned())
                .collect(),
            located: RefCell::new(BTreeMap::new()),
        }
    }

    /// A span for `path`, located at the first needle that occurs exactly once.
    pub fn span(&self, path: impl Into<String>, needles: &[String]) -> Span {
        let path = path.into();
        for needle in needles {
            if let Some((source, location)) = self.unique(needle) {
                return Span {
                    source,
                    path,
                    located: Some(location),
                };
            }
        }
        Span {
            source: Source::DOCUMENT.to_owned(),
            path,
            located: None,
        }
    }

    /// The one place `needle` occurs, or `None` when it occurs nowhere or more than once.
    ///
    /// Answered once per needle for the life of the locator; see [`Self::located`].
    fn unique(&self, needle: &str) -> Option<(String, Location)> {
        if let Some(remembered) = self.located.borrow().get(needle) {
            return remembered.clone();
        }
        let answer = self.scan(needle);
        self.located
            .borrow_mut()
            .insert(needle.to_owned(), answer.clone());
        answer
    }

    /// [`Self::unique`], actually reading the files.
    fn scan(&self, needle: &str) -> Option<(String, Location)> {
        let mut found: Option<(String, Location)> = None;
        for label in &self.labels {
            let Some(text) = self.sources.get(label) else {
                continue;
            };
            let mut occurrences = text.match_indices(needle);
            let Some((index, _)) = occurrences.next() else {
                continue;
            };
            if occurrences.next().is_some() || found.is_some() {
                return None;
            }
            found = Some((label.clone(), location_of(text, index)));
        }
        found
    }
}

/// The line and column a byte offset falls on, counted as an editor counts them.
fn location_of(text: &str, index: usize) -> Location {
    let before = &text[..index];
    let last_line = before.rsplit_once('\n').map_or(before, |(_, rest)| rest);
    Location {
        line: before.matches('\n').count() + 1,
        column: last_line.chars().count() + 1,
    }
}

/// `ess-domain`'s refusals, as diagnostics with codes and source lines.
///
/// The bridge, and deliberately not a second implementation. Every cross-cutting rule in this model
/// is enforced by [`Specification::validate`], where it is already tested; what that produces is a
/// [`ValidationError`](aep_domain::error::ValidationError) with a document path and prose. What §29 asks for is a code, a span and a
/// structured body. This adds the first two — the family from the location's layer, the class from
/// the [`ValidationCode`], the line by finding the declaration in the file it was read from.
///
/// # What it cannot add
///
/// The structured body. A [`ValidationError`](aep_domain::error::ValidationError) carries its facts as a sentence, and parsing two types
/// back out of a sentence is exactly what §29 says a consumer must not have to do — so it would be no
/// better done here. Diagnostics with typed details are the ones [`compile`] produces itself; the
/// bridged ones carry the message, the hint, the span and the `ess-domain` code they came from.
/// Closing that gap means giving [`ValidationError`](aep_domain::error::ValidationError) structured fields, in `ess-domain`.
pub fn diagnose(errors: &ValidationErrors, sources: &SourceMap) -> Diagnostics {
    diagnose_locating(errors, sources, &[] as &[&str])
}

/// [`diagnose`], searching `files` for the line each refusal belongs on.
pub fn diagnose_locating(
    errors: &ValidationErrors,
    sources: &SourceMap,
    files: &[impl AsRef<str>],
) -> Diagnostics {
    bridge(errors, &Locator::new(sources, files))
}

/// [`diagnose`], against a locator that already exists.
fn bridge(errors: &ValidationErrors, locator: &Locator<'_>) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    for error in errors.as_slice() {
        diagnostics.push(Diagnostic {
            code: Code::new(family_of(&error.location), class_of(error.code)),
            severity: Severity::Error,
            message: error.message.clone(),
            details: vec![Detail::Note {
                text: format!("`ess-domain` refuses this as `{}`", error.code.as_str()),
            }],
            hint: error.hint.clone(),
            span: Some(locator.span(error.location.clone(), &needles_for(&error.location))),
        });
    }
    diagnostics
}

/// Which layer a document path is in.
///
/// `ess-domain` writes a path two ways — `binding.notify-on-invoice-created.mapping.recipient` and
/// `component invoice-service` — so both separators are read, and an unrecognised head means the
/// specification as a whole rather than a guess.
fn family_of(location: &str) -> &'static str {
    match location
        .split(['.', ' ', '['])
        .next()
        .unwrap_or_default()
        .trim_end_matches('s')
    {
        "type" | "conversion" => codes::family::TYPE,
        "entitie" | "entity" => codes::family::ENTITY,
        "command" => codes::family::COMMAND,
        "event" => codes::family::EVENT,
        "error" => codes::family::ERROR,
        "view" => codes::family::VIEW,
        "actor" => codes::family::ACTOR,
        "binding" => codes::family::BINDING,
        "component" => codes::family::COMPONENT,
        "topology" => codes::family::TOPOLOGY,
        "domain" => codes::family::DOMAIN,
        _ => codes::family::SPEC,
    }
}

/// What kind of defect a `ess-domain` code reports.
///
/// The enum is `#[non_exhaustive]`, so the mapping ends in [`class::OTHER`](codes::class::OTHER)
/// rather than in a match this crate has to be recompiled to keep total.
fn class_of(code: ValidationCode) -> u16 {
    use ValidationCode as Refused;

    match code {
        Refused::UndeclaredReference
        | Refused::UndeclaredCapability
        | Refused::UndeclaredEvidenceKind
        | Refused::UnknownPrinciple
        | Refused::UnknownProfile
        | Refused::UnknownWorkflow
        | Refused::UnknownProtocol => codes::class::UNDECLARED,
        Refused::TypeMismatch | Refused::EventPayloadMismatch | Refused::VersionMismatch => {
            codes::class::TYPE_MISMATCH
        }
        Refused::UnobservableFact => codes::class::UNREADABLE,
        Refused::ConflictingDeclaration
        | Refused::CapabilityConflict
        | Refused::RefusalMutatedState => codes::class::CONFLICT,
        Refused::MissingDeclaration
        | Refused::NonExhaustiveBranches
        | Refused::MissingCausation
        | Refused::IncompleteEventSubject => codes::class::MISSING,
        Refused::DuplicateDeclaration
        | Refused::DuplicateTransition
        | Refused::DuplicatePrinciple => codes::class::DUPLICATE,
        Refused::EmptyDeclaration | Refused::EmptyChange | Refused::EmptyWorkflow => {
            codes::class::EMPTY
        }
        Refused::SelfReference => codes::class::SELF_REFERENCE,
        Refused::UnsupportedConstruct
        | Refused::UnsupportedFormatVersion
        | Refused::UnsupportedProtocolVersion => codes::class::UNSUPPORTED,
        Refused::MisspelledReference => codes::class::MISSPELLED,
        Refused::UnknownState
        | Refused::UnknownInitialState
        | Refused::DeadEndState
        | Refused::UnreachableState
        | Refused::UnknownPhase => codes::class::LIFECYCLE,
        _ => codes::class::OTHER,
    }
}

/// The keys a document path passes *through*, which never name a declaration.
///
/// Used to stop reading a path at the point it stops being a name — `binding.<id>.mapping.<target>`
/// names a binding, not a binding called `mapping`.
const STRUCTURAL: &[&str] = &[
    "mapping",
    "on_failure",
    "escalate",
    "delivery",
    "input",
    "fields",
    "outcomes",
    "emits",
    "error",
    "when",
    "external",
    "creates",
    "moves",
    "updates",
    "filter",
    "states",
    "terminal",
    "initial",
    "transitions",
    "requires",
    "replicas",
    "min",
    "max",
    "naming",
    "invariants",
    "identity",
    "lifecycle",
    "may",
    "source",
    "consistency",
    "tag",
    "variants",
    "of",
    "types",
    "domains",
    "workloads",
    "system",
    "conversions",
    "entities",
    "commands",
    "events",
    "errors",
    "views",
    "actors",
    "components",
    "bindings",
    "topology",
];

/// Needles for a document path, most specific first.
///
/// Guessing wrongly is safe: [`Locator`] only reports a line for a needle that occurs exactly once,
/// so a bad guess produces no line rather than the wrong one.
fn needles_for(location: &str) -> Vec<String> {
    let tokens: Vec<&str> = location
        .split(['.', ' ', '[', ']'])
        .filter(|token| !token.is_empty())
        .collect();
    let mut needles = Vec::new();

    // The last segment, when it is a key an author wrote — `recipient:`, `invoice-service:`.
    if let Some(last) = tokens.last() {
        let key_like = last
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase());
        if key_like && !STRUCTURAL.contains(last) {
            needles.push(format!("{last}:"));
        }
    }

    // The declaration the path is about: everything after the layer, up to the first structural key.
    let declared: Vec<&str> = tokens
        .iter()
        .skip(1)
        .take_while(|token| !STRUCTURAL.contains(token))
        .copied()
        .collect();
    if !declared.is_empty() {
        let declared = declared.join(".");
        needles.push(format!("name: {declared}"));
        needles.push(format!("id: {declared}"));
        needles.push(format!("component: {declared}"));
    }
    needles
}

/// Resolves a specification, or reports every reason it does not resolve.
///
/// The `sources` are only for diagnostics: pass [`SourceMap::new`] and every diagnostic still
/// carries its document path, just not a line number. To get line numbers, name the files —
/// [`compile_locating`].
pub fn compile(specification: &Specification, sources: &SourceMap) -> Result<EssIr, Diagnostics> {
    compile_locating(specification, sources, &[] as &[&str])
}

/// [`compile`], searching `files` for the line each diagnostic belongs on.
///
/// A second entry point rather than an argument to [`compile`], because the labels are redundant:
/// they are the keys of the [`SourceMap`] itself, which has no accessor for them yet. When it grows
/// one, [`compile`] forwards to this with those keys and this function stops being useful.
pub fn compile_locating(
    specification: &Specification,
    sources: &SourceMap,
    files: &[impl AsRef<str>],
) -> Result<EssIr, Diagnostics> {
    Resolver::new(specification, Locator::new(sources, files)).run()
}

/// What looking a reference up found.
///
/// Three answers, not two, because "declared but unresolved" must not be reported: the reason it did
/// not resolve was reported where it happened.
enum Found<H> {
    /// It resolved.
    Handle(H),
    /// It is declared, and something else about it did not resolve.
    Unresolved,
    /// Nothing declares it.
    Missing,
}

/// One compilation.
struct Resolver<'a> {
    spec: &'a Specification,
    locator: Locator<'a>,
    /// Every type a reference may name: the system's registry, plus the enum each entity's lifecycle
    /// forms.
    ///
    /// The same registry `Specification::validate` resolves against. Leaving the lifecycle enums out
    /// would make the compiler refuse a field the domain crate had just accepted, and an author
    /// would have no way to tell which of the two was wrong.
    registry: TypeRegistry,
    /// Set when a reference in a layer `ess-domain` validates names something undeclared.
    off_contract: bool,
    diagnostics: Diagnostics,
}

impl<'a> Resolver<'a> {
    fn new(spec: &'a Specification, locator: Locator<'a>) -> Self {
        let mut registry = spec.system.types.clone();
        for entity in spec.entities.values() {
            // A collision between a lifecycle enum and a declared type is wave 1's rejection, and it
            // has already been reported by the time a `Specification` exists.
            let _ = registry.insert(entity.state_type());
        }
        Self {
            spec,
            locator,
            registry,
            off_contract: false,
            diagnostics: Diagnostics::new(),
        }
    }

    /// Every pass, in an order where each has what the one before it produced.
    fn run(mut self) -> Result<EssIr, Diagnostics> {
        let types = self.types();
        let conversions = self.conversions();
        let entities = self.entities(&types);
        let events = self.events();
        let errors = self.errors();
        let commands = self.commands(&events, &errors, &entities);
        let views = self.views(&entities);
        let actors = self.actors(&commands);
        let components = self.components(&commands, &events);
        let bindings = self.bindings(&events, &commands);
        let workloads = self.workloads(&components);
        let domains = self.domains(&Members {
            types: &types,
            entities: &entities,
            commands: &commands,
            events: &events,
            errors: &errors,
            views: &views,
            actors: &actors,
        });
        self.report_off_contract();

        if self.diagnostics.has_errors() {
            return Err(self.diagnostics);
        }
        Ok(EssIr {
            system: self.spec.system.name.clone(),
            version: self.spec.system.version,
            naming: self.spec.system.naming.clone(),
            summary: self.spec.system.summary.clone(),
            domains,
            types,
            conversions,
            entities,
            commands,
            events,
            errors,
            views,
            actors,
            bindings,
            components,
            workloads,
        })
    }

    // ---- reporting ------------------------------------------------------------------------

    /// Records one refusal.
    fn refuse(
        &mut self,
        code: Code,
        message: String,
        details: Vec<Detail>,
        hint: Option<&str>,
        span: Span,
    ) {
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            message,
            details,
            hint: hint.map(ToOwned::to_owned),
            span: Some(span),
        });
    }

    /// Reports what `ess-domain` says, when a layer that crate validates did not resolve.
    ///
    /// Bridged rather than restated. The rule has one implementation, and this exists so that the IR
    /// cannot come back quietly missing a component the caller declared: whatever
    /// [`Specification::validate`] says about it becomes a diagnostic with the same code any other
    /// path would have given it.
    fn report_off_contract(&mut self) {
        if !self.off_contract {
            return;
        }
        // Only the layers this pass delegates. Bridging everything `validate` says would report a
        // misspelt type twice — once by this pass, which needed it to mint a handle, and once by the
        // domain crate, which is not a second defect.
        let mut bridged = Diagnostics::new();
        for diagnostic in bridge(&self.spec.validate(), &self.locator).as_slice() {
            let delegated = matches!(
                diagnostic.code.family,
                codes::family::COMPONENT | codes::family::TOPOLOGY
            );
            if delegated {
                bridged.push(diagnostic.clone());
            }
        }
        if !bridged.is_empty() {
            self.diagnostics.extend(bridged);
            return;
        }
        // The backstop: a reference did not resolve and the domain crate reported nothing about it.
        // Returning `Ok` here would be an IR that is quietly missing something.
        let span = self
            .locator
            .span("system", &[format!("system: {}", self.spec.system.name)]);
        self.refuse(
            codes::UNVALIDATED_SPECIFICATION,
            format!(
                "`{}` names something nothing declares, and this pass cannot build a handle for it",
                self.spec.system.name
            ),
            Vec::new(),
            Some(
                "compile what `Specification::assemble` returned; a specification built field by \
                 field has not been validated",
            ),
            span,
        );
    }

    /// The names of every declared type, for a reader who mistyped one.
    fn declared_types(&self) -> Vec<String> {
        self.registry
            .iter()
            .map(|declared| declared.name.to_string())
            .collect()
    }

    // ---- types ----------------------------------------------------------------------------

    /// Every declared type, with the references in its body resolved.
    fn types(&mut self) -> BTreeMap<QualifiedName, ResolvedType> {
        // Cloned out of the registry first: resolving a body reports diagnostics, which needs
        // `&mut self` while the registry is being read.
        let declared: Vec<NamedType> = self.registry.iter().cloned().collect();
        let mut resolved = BTreeMap::new();
        for declared in declared {
            let path = format!("types.{}", declared.name);
            let needles = vec![format!("name: {}", declared.name)];
            if let Some(body) = self.body(codes::UNDECLARED_TYPE, &declared, &path, &needles) {
                resolved.insert(
                    declared.name.clone(),
                    ResolvedType {
                        name: declared.name,
                        body,
                        naming: declared.naming,
                    },
                );
            }
        }
        resolved
    }

    /// One type's body, or `None` when something in it does not resolve.
    fn body(
        &mut self,
        code: Code,
        declared: &NamedType,
        path: &str,
        needles: &[String],
    ) -> Option<ResolvedBody> {
        match &declared.body {
            TypeBody::Newtype { of, invariants } => {
                let mut of_needles = vec![format!("of: {of}")];
                of_needles.extend_from_slice(needles);
                let subject = declared.name.to_string();
                let of = self.type_ref(code, of, &subject, path, &of_needles)?;
                Some(ResolvedBody::Newtype {
                    of,
                    invariants: invariants.clone(),
                })
            }
            TypeBody::Struct { fields, invariants } => {
                let fields = self.fields(code, fields, &declared.name, path, needles)?;
                Some(ResolvedBody::Struct {
                    fields,
                    invariants: invariants.clone(),
                })
            }
            TypeBody::Enum { variants } => Some(ResolvedBody::Enum {
                variants: variants.clone(),
            }),
            TypeBody::Union { tag, variants } => {
                let mut resolved = BTreeMap::new();
                let mut complete = true;
                for (variant, reference) in variants {
                    let subject = format!("{}.{variant}", declared.name);
                    let mut variant_needles = vec![format!("{variant}: {reference}")];
                    variant_needles.extend_from_slice(needles);
                    match self.type_ref(code, reference, &subject, path, &variant_needles) {
                        Some(reference) => {
                            resolved.insert(variant.clone(), reference);
                        }
                        None => complete = false,
                    }
                }
                complete.then(|| ResolvedBody::Union {
                    tag: tag.clone(),
                    variants: resolved,
                })
            }
        }
    }

    /// A list of fields, reporting every one that does not resolve rather than the first.
    fn fields(
        &mut self,
        code: Code,
        fields: &[Field],
        owner: &QualifiedName,
        path: &str,
        needles: &[String],
    ) -> Option<Vec<ResolvedField>> {
        let mut resolved = Vec::with_capacity(fields.len());
        let mut complete = true;
        for field in fields {
            let subject = format!("{owner}.{}", field.name);
            let mut field_needles = vec![format!("type: {}", field.type_ref)];
            field_needles.extend_from_slice(needles);
            match self.type_ref(
                code,
                &field.type_ref,
                &subject,
                &format!("{path}.{}", field.name),
                &field_needles,
            ) {
                Some(type_ref) => resolved.push(ResolvedField {
                    name: field.name.clone(),
                    type_ref,
                    naming: field.naming.clone(),
                }),
                None => complete = false,
            }
        }
        complete.then_some(resolved)
    }

    /// One type reference, resolved leaf by leaf.
    ///
    /// `code` is the family of whatever holds the reference — a command's input is
    /// `ESS-COMMAND-001`, a type's own body is `ESS-TYPE-001` — so that the code says where to look
    /// and matches what `ess-domain` would have reported for the same defect at the same location.
    ///
    /// Recurses as deep as the reference nests, and does not count: a [`TypeRef`] that reached this
    /// crate came through [`TypeRef::parse`], which refuses past
    /// [`MAX_TYPE_DEPTH`](ess_domain::types::MAX_TYPE_DEPTH). Counting here would be a second bound
    /// on the same document, checked after the one that already refused it.
    fn type_ref(
        &mut self,
        code: Code,
        reference: &TypeRef,
        subject: &str,
        path: &str,
        needles: &[String],
    ) -> Option<ResolvedTypeRef> {
        match reference {
            TypeRef::Primitive(primitive) => Some(ResolvedTypeRef::Primitive { name: *primitive }),
            TypeRef::Named(name) => {
                if self.registry.get(name).is_none() {
                    let available = self.declared_types();
                    let span = self.locator.span(path.to_owned(), needles);
                    self.refuse(
                        code,
                        format!("`{subject}` is typed as something nothing declares"),
                        vec![
                            Detail::Typed {
                                subject: subject.to_owned(),
                                type_ref: reference.to_string(),
                                requires: true,
                            },
                            Detail::Undeclared {
                                name: name.clone(),
                                expected: "type",
                                available,
                            },
                        ],
                        Some("declare the type, or point this at one that exists"),
                        span,
                    );
                    return None;
                }
                Some(ResolvedTypeRef::Declared {
                    name: TypeHandle::new(name.clone()),
                })
            }
            TypeRef::Optional(inner) => Some(ResolvedTypeRef::Optional {
                of: Box::new(self.type_ref(code, inner, subject, path, needles)?),
            }),
            TypeRef::List(inner) => Some(ResolvedTypeRef::List {
                of: Box::new(self.type_ref(code, inner, subject, path, needles)?),
            }),
            TypeRef::Map(key, value) => Some(ResolvedTypeRef::Map {
                key: *key,
                value: Box::new(self.type_ref(code, value, subject, path, needles)?),
            }),
        }
    }

    /// Every declared crossing, with both ends resolved.
    fn conversions(&mut self) -> Vec<ResolvedConversion> {
        let declared: Vec<_> = self.spec.conversions.iter().cloned().collect();
        let mut resolved = Vec::with_capacity(declared.len());
        for conversion in declared {
            let path = format!("conversions.{} -> {}", conversion.from, conversion.to);
            let needles = vec![
                format!("from: {}", conversion.from),
                format!("to: {}", conversion.to),
            ];
            let subject = format!("the conversion from `{}`", conversion.from);
            let code = codes::UNDECLARED_TYPE;
            let from = self.type_ref(code, &conversion.from, &subject, &path, &needles);
            let to = self.type_ref(code, &conversion.to, &subject, &path, &needles);
            if let (Some(from), Some(to)) = (from, to) {
                resolved.push(ResolvedConversion {
                    from,
                    to,
                    because: conversion.because.clone(),
                });
            }
        }
        resolved
    }

    // ---- lookups --------------------------------------------------------------------------

    /// An event, as a handle.
    fn event_of(
        &self,
        name: &QualifiedName,
        events: &BTreeMap<QualifiedName, ResolvedEvent>,
    ) -> Found<EventHandle> {
        if events.contains_key(name) {
            Found::Handle(EventHandle::new(name.clone()))
        } else if self.spec.events.contains_key(name) {
            Found::Unresolved
        } else {
            Found::Missing
        }
    }

    /// An entity, as a handle.
    fn entity_of(
        &self,
        name: &QualifiedName,
        entities: &BTreeMap<QualifiedName, ResolvedEntity>,
    ) -> Found<EntityHandle> {
        if entities.contains_key(name) {
            Found::Handle(EntityHandle::new(name.clone()))
        } else if self.spec.entities.contains_key(name) {
            Found::Unresolved
        } else {
            Found::Missing
        }
    }

    /// A command, as a handle.
    fn command_of(
        &self,
        name: &QualifiedName,
        commands: &BTreeMap<QualifiedName, ResolvedCommand>,
    ) -> Found<CommandHandle> {
        if commands.contains_key(name) {
            Found::Handle(CommandHandle::new(name.clone()))
        } else if self.spec.commands.contains_key(name) {
            Found::Unresolved
        } else {
            Found::Missing
        }
    }

    /// An error, as a handle.
    fn error_of(
        &self,
        name: &QualifiedName,
        errors: &BTreeMap<QualifiedName, ResolvedError>,
    ) -> Found<ErrorHandle> {
        if errors.contains_key(name) {
            Found::Handle(ErrorHandle::new(name.clone()))
        } else if self.spec.errors.contains_key(name) {
            Found::Unresolved
        } else {
            Found::Missing
        }
    }

    /// Refuses a reference to something nothing declares.
    fn refuse_undeclared(
        &mut self,
        code: Code,
        message: String,
        name: &QualifiedName,
        expected: &'static str,
        available: Vec<String>,
        span: Span,
    ) {
        self.refuse(
            code,
            message,
            vec![Detail::Undeclared {
                name: name.clone(),
                expected,
                available,
            }],
            None,
            span,
        );
    }

    // ---- members --------------------------------------------------------------------------

    /// The domain that owns a name, as a handle.
    ///
    /// A member with no owner is refused in *its own* family — `ESS-COMMAND-001` for a command —
    /// because that is where `ess-domain` reports it too, and one defect keeps one code.
    fn owner(&mut self, code: Code, name: &QualifiedName, kind: &str) -> Option<DomainHandle> {
        if let Some(domain) = self.spec.system.owner_of(name) {
            return Some(DomainHandle::new(domain.name.clone()));
        }
        let available = self.declared_domains();
        let span = self
            .locator
            .span(format!("{kind}s.{name}"), &[format!("name: {name}")]);
        self.refuse(
            code,
            format!("`{name}` is inside no declared domain"),
            vec![Detail::Undeclared {
                name: name.clone(),
                expected: "member of a declared domain",
                available,
            }],
            Some("a member belongs to a bounded context; declare it under the domain that owns it"),
            span,
        );
        None
    }

    /// The names of every declared domain.
    fn declared_domains(&self) -> Vec<String> {
        self.spec
            .system
            .domains
            .iter()
            .map(|domain| domain.name.to_string())
            .collect()
    }

    /// Every event, with its payload resolved.
    fn events(&mut self) -> BTreeMap<QualifiedName, ResolvedEvent> {
        let declared: Vec<EventSpec> = self.spec.events.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for event in declared {
            let path = format!("events.{}", event.name);
            let needles = vec![format!("name: {}", event.name)];
            let code = codes::EVENT_UNDECLARED_REFERENCE;
            let fields = self.fields(code, &event.fields, &event.name, &path, &needles);
            let domain = self.owner(code, &event.name, "event");
            if let (Some(fields), Some(domain)) = (fields, domain) {
                resolved.insert(
                    event.name.clone(),
                    ResolvedEvent {
                        name: event.name,
                        domain,
                        fields,
                        naming: event.naming,
                    },
                );
            }
        }
        resolved
    }

    /// Every error, with its payload resolved.
    fn errors(&mut self) -> BTreeMap<QualifiedName, ResolvedError> {
        let declared: Vec<ErrorSpec> = self.spec.errors.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for error in declared {
            let path = format!("errors.{}", error.name);
            let needles = vec![format!("name: {}", error.name)];
            let code = codes::ERROR_UNDECLARED_REFERENCE;
            let fields = self.fields(code, &error.fields, &error.name, &path, &needles);
            let domain = self.owner(code, &error.name, "error");
            if let (Some(fields), Some(domain)) = (fields, domain) {
                resolved.insert(
                    error.name.clone(),
                    ResolvedError {
                        name: error.name,
                        domain,
                        summary: error.summary,
                        fields,
                    },
                );
            }
        }
        resolved
    }

    /// Every command, with its input and every outcome resolved.
    fn commands(
        &mut self,
        events: &BTreeMap<QualifiedName, ResolvedEvent>,
        errors: &BTreeMap<QualifiedName, ResolvedError>,
        entities: &BTreeMap<QualifiedName, ResolvedEntity>,
    ) -> BTreeMap<QualifiedName, ResolvedCommand> {
        let declared: Vec<CommandSpec> = self.spec.commands.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for command in declared {
            let path = format!("commands.{}", command.name);
            let needles = vec![format!("name: {}", command.name)];
            let code = codes::COMMAND_UNDECLARED_REFERENCE;
            let input = self.fields(code, &command.input, &command.name, &path, &needles);
            let outcomes = self.outcomes(&command, events, errors, entities, &path, &needles);
            let domain = self.owner(code, &command.name, "command");
            if let (Some(input), Some(outcomes), Some(domain)) = (input, outcomes, domain) {
                resolved.insert(
                    command.name.clone(),
                    ResolvedCommand {
                        name: command.name,
                        domain,
                        input,
                        outcomes,
                        naming: command.naming,
                    },
                );
            }
        }
        resolved
    }

    /// One command's outcomes, with the events and errors they name resolved.
    fn outcomes(
        &mut self,
        command: &CommandSpec,
        events: &BTreeMap<QualifiedName, ResolvedEvent>,
        errors: &BTreeMap<QualifiedName, ResolvedError>,
        entities: &BTreeMap<QualifiedName, ResolvedEntity>,
        path: &str,
        needles: &[String],
    ) -> Option<Vec<ResolvedOutcome>> {
        let mut resolved = Vec::with_capacity(command.outcomes.len());
        let mut complete = true;
        for outcome in &command.outcomes {
            let path = format!("{path}.outcomes.{}", outcome.name.as_str());
            let mut emits = Vec::with_capacity(outcome.emits.len());
            for event in &outcome.emits {
                match self.event_of(event, events) {
                    Found::Handle(handle) => emits.push(handle),
                    Found::Unresolved => complete = false,
                    Found::Missing => {
                        complete = false;
                        let available = self.spec.events.keys().map(ToString::to_string).collect();
                        let span = self.locator.span(path.clone(), needles);
                        self.refuse_undeclared(
                            codes::COMMAND_UNDECLARED_REFERENCE,
                            format!("`{}` emits an event nothing declares", command.name),
                            event,
                            "event",
                            available,
                            span,
                        );
                    }
                }
            }
            let error = match &outcome.error {
                None => None,
                Some(name) => match self.error_of(name, errors) {
                    Found::Handle(handle) => Some(handle),
                    Found::Unresolved => {
                        complete = false;
                        None
                    }
                    Found::Missing => {
                        complete = false;
                        let available = self.spec.errors.keys().map(ToString::to_string).collect();
                        let span = self.locator.span(path.clone(), needles);
                        self.refuse_undeclared(
                            codes::COMMAND_UNDECLARED_REFERENCE,
                            format!("`{}` reports an error nothing declares", command.name),
                            name,
                            "error",
                            available,
                            span,
                        );
                        None
                    }
                },
            };
            let mut subject = None;
            if let Some(declared) = &outcome.subject {
                subject = self.subject(command, outcome, declared, entities, &path);
                if subject.is_none() {
                    complete = false;
                }
            }
            resolved.push(ResolvedOutcome {
                name: outcome.name.clone(),
                condition: condition_of(outcome),
                subject,
                test_strategy: outcome.test_strategy(),
                emits,
                error,
                summary: outcome.summary.clone(),
            });
        }
        complete.then_some(resolved)
    }

    /// One outcome's subject: the entity it acts on, and the move it takes.
    ///
    /// Two references to resolve, and both are resolved here for the same reason a view's source is:
    /// there is no [`EntityHandle`] to put in an outcome for an entity nothing declares, and no
    /// [`Transition`](ess_domain::entity::Transition) to carry for a move the entity's lifecycle does
    /// not have. `ess-domain`'s `validate_lifecycle_causes` refuses both under
    /// `undeclared_reference`, which is the code this bridges to, so one defect keeps one code
    /// whichever half noticed it.
    ///
    /// The *causation* rule — a transition no outcome takes — is deliberately not restated here. An
    /// uncaused transition is perfectly representable in the IR, so by this pass's own doctrine it
    /// belongs to `ess-domain` alone and reaches a reader through [`diagnose`].
    fn subject(
        &mut self,
        command: &CommandSpec,
        outcome: &Outcome,
        subject: &Subject,
        entities: &BTreeMap<QualifiedName, ResolvedEntity>,
        path: &str,
    ) -> Option<ResolvedSubject> {
        let verb = subject.effect.verb();
        let needles = vec![
            format!("{verb}: {}", subject.entity),
            format!("name: {}", command.name),
        ];
        let entity = match self.entity_of(&subject.entity, entities) {
            Found::Handle(handle) => handle,
            Found::Unresolved => return None,
            Found::Missing => {
                let available = self.spec.entities.keys().map(ToString::to_string).collect();
                let span = self.locator.span(format!("{path}.{verb}"), &needles);
                self.refuse_undeclared(
                    codes::COMMAND_UNDECLARED_REFERENCE,
                    format!(
                        "outcome `{}` of `{}` {verb} `{}`, which is not a declared entity",
                        outcome.name, command.name, subject.entity
                    ),
                    &subject.entity,
                    "entity",
                    available,
                    span,
                );
                return None;
            }
        };

        let effect = match subject.effect.transition() {
            None if matches!(subject.effect, Effect::Creates) => ResolvedEffect::Creates,
            None => ResolvedEffect::Updates,
            Some(named) => {
                let declared = entities
                    .get(&subject.entity)
                    .and_then(|resolved| resolved.lifecycle.transition(named));
                let Some(transition) = declared else {
                    let available = entities
                        .get(&subject.entity)
                        .map(|resolved| {
                            resolved
                                .lifecycle
                                .transitions
                                .iter()
                                .map(|declared| declared.name.clone())
                                .collect()
                        })
                        .unwrap_or_default();
                    let span = self.locator.span(format!("{path}.moves"), &needles);
                    self.refuse_undeclared(
                        codes::COMMAND_UNDECLARED_REFERENCE,
                        format!(
                            "outcome `{}` of `{}` takes `{named}`, which `{}` does not declare as a \
                             transition",
                            outcome.name, command.name, subject.entity
                        ),
                        &subject.entity.child(named),
                        "transition",
                        available,
                        span,
                    );
                    return None;
                };
                ResolvedEffect::Moves {
                    transition: transition.clone(),
                }
            }
        };

        Some(ResolvedSubject { entity, effect })
    }

    // ---- entities, views and actors --------------------------------------------------------

    /// Every entity, with its identity, its fields and its lifecycle resolved.
    ///
    /// The lifecycle travels as `ess-domain`'s own [`StateMachine`], so the transitions, the initial
    /// state and the terminal set survive together with the state list — which is what a diagram
    /// with arrows in it is made of. What this pass adds is the two handles an entity needs: the
    /// types its fields are, and the enum its states form.
    fn entities(
        &mut self,
        types: &BTreeMap<QualifiedName, ResolvedType>,
    ) -> BTreeMap<QualifiedName, ResolvedEntity> {
        let declared: Vec<EntitySpec> = self.spec.entities.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for entity in declared {
            let path = format!("entities.{}", entity.name);
            let needles = vec![format!("name: {}", entity.name)];
            let code = codes::ENTITY_UNDECLARED_REFERENCE;
            let identity = self
                .fields(
                    code,
                    std::slice::from_ref(&entity.identity),
                    &entity.name,
                    &format!("{path}.identity"),
                    &needles,
                )
                .and_then(|resolved| resolved.into_iter().next());
            let fields = self.fields(code, &entity.fields, &entity.name, &path, &needles);
            let domain = self.owner(code, &entity.name, "entity");
            let state_type = self.state_type(&entity, types);
            let lifecycle = self.lifecycle(&entity);
            let (Some(identity), Some(fields), Some(domain), Some(state_type), Some(lifecycle)) =
                (identity, fields, domain, state_type, lifecycle)
            else {
                continue;
            };
            resolved.insert(
                entity.name.clone(),
                ResolvedEntity {
                    name: entity.name,
                    domain,
                    identity,
                    fields,
                    state_type,
                    lifecycle,
                    invariants: entity.invariants,
                    naming: entity.naming,
                },
            );
        }
        resolved
    }

    /// The enum an entity's lifecycle forms, as a handle.
    ///
    /// Synthesised by [`EntitySpec::state_type`] and inserted into this pass's registry, so it is
    /// resolved like any other declared type and a projection emits the states from one place. It is
    /// absent only when a declared type has claimed the same name, which is wave 1's rejection and
    /// already reported by the time a `Specification` exists.
    fn state_type(
        &mut self,
        entity: &EntitySpec,
        types: &BTreeMap<QualifiedName, ResolvedType>,
    ) -> Option<TypeHandle> {
        let name = entity.state_type().name;
        if types.contains_key(&name) {
            return Some(TypeHandle::new(name));
        }
        self.off_contract = true;
        None
    }

    /// One entity's lifecycle, checked to name only states it declares.
    ///
    /// Not a rule of this pass's own: `RawStateMachine`'s conversion owns it, with the codes design
    /// §20 asks for, and restating it here would be a second refusal for one defect. What this does
    /// is refuse to *hold* a lifecycle whose transition points at a phantom state — the compilation
    /// goes off-contract, `ess-domain`'s verdict is bridged in, and the IR does not come back with a
    /// state diagram containing an arrow to nowhere.
    fn lifecycle(&mut self, entity: &EntitySpec) -> Option<StateMachine> {
        let machine = &entity.states;
        let declared = |state| machine.states.contains(state);
        let sound = declared(&machine.initial)
            && machine.terminal.iter().all(declared)
            && machine
                .transitions
                .iter()
                .all(|transition| transition.from.iter().all(declared) && declared(&transition.to));
        if !sound {
            self.off_contract = true;
            return None;
        }
        Some(machine.clone())
    }

    /// Every view, with its source entity and its projected fields resolved.
    ///
    /// The consistency and the assertion style come across unchanged, and the style is the domain
    /// crate's own answer rather than one computed here: `expect` against a projection is a race,
    /// and the repair everybody reaches for is a sleep.
    fn views(
        &mut self,
        entities: &BTreeMap<QualifiedName, ResolvedEntity>,
    ) -> BTreeMap<QualifiedName, ResolvedView> {
        let declared: Vec<ViewSpec> = self.spec.views.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for view in declared {
            let path = format!("views.{}", view.name);
            let needles = vec![format!("name: {}", view.name)];
            let code = codes::VIEW_UNDECLARED_REFERENCE;
            let fields = self.fields(code, &view.fields, &view.name, &path, &needles);
            let domain = self.owner(code, &view.name, "view");
            let source = match self.entity_of(&view.source, entities) {
                Found::Handle(handle) => Some(handle),
                Found::Unresolved => None,
                Found::Missing => {
                    let available = self.spec.entities.keys().map(ToString::to_string).collect();
                    let span = self.locator.span(format!("{path}.source"), &needles);
                    self.refuse_undeclared(
                        code,
                        format!(
                            "`{}` projects `{}`, which is not a declared entity",
                            view.name, view.source
                        ),
                        &view.source,
                        "entity",
                        available,
                        span,
                    );
                    None
                }
            };
            let (Some(fields), Some(domain), Some(source)) = (fields, domain, source) else {
                continue;
            };
            resolved.insert(
                view.name.clone(),
                ResolvedView {
                    name: view.name,
                    domain,
                    source,
                    fields,
                    filter: view.filter,
                    consistency: view.consistency,
                    assertion_style: view.consistency.assertion_style(),
                    naming: view.naming,
                },
            );
        }
        resolved
    }

    /// Every actor, with every grant resolved to the command it names.
    fn actors(
        &mut self,
        commands: &BTreeMap<QualifiedName, ResolvedCommand>,
    ) -> BTreeMap<QualifiedName, ResolvedActor> {
        let declared: Vec<ActorSpec> = self.spec.actors.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for actor in declared {
            let path = format!("actors.{}", actor.name);
            let needles = vec![format!("name: {}", actor.name)];
            let code = codes::ACTOR_UNDECLARED_REFERENCE;
            let domain = self.owner(code, &actor.name, "actor");
            let mut may = BTreeSet::new();
            let mut complete = true;
            for granted in &actor.may {
                match self.command_of(granted, commands) {
                    Found::Handle(handle) => {
                        may.insert(handle);
                    }
                    Found::Unresolved => complete = false,
                    Found::Missing => {
                        complete = false;
                        let available =
                            self.spec.commands.keys().map(ToString::to_string).collect();
                        let span = self.locator.span(format!("{path}.may"), &needles);
                        self.refuse_undeclared(
                            code,
                            format!(
                                "`{}` may invoke `{granted}`, which no domain declares as a command",
                                actor.name
                            ),
                            granted,
                            "command",
                            available,
                            span,
                        );
                    }
                }
            }
            let (Some(domain), true) = (domain, complete) else {
                continue;
            };
            resolved.insert(
                actor.name.clone(),
                ResolvedActor {
                    name: actor.name,
                    domain,
                    may,
                    naming: actor.naming,
                },
            );
        }
        resolved
    }

    // ---- components and topology ----------------------------------------------------------

    /// Every component, with its ownership and surface resolved.
    ///
    /// The rejections here belong to `ess-domain`'s `validate_components`, so nothing is refused
    /// twice: a reference to something undeclared marks the compilation off-contract and the
    /// component stays out of the IR.
    fn components(
        &mut self,
        commands: &BTreeMap<QualifiedName, ResolvedCommand>,
        events: &BTreeMap<QualifiedName, ResolvedEvent>,
    ) -> BTreeMap<ComponentName, ResolvedComponent> {
        let declared: Vec<ComponentSpec> = self.spec.components.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for component in declared {
            let mut complete = true;
            let mut owns = BTreeSet::new();
            for domain in &component.owns {
                if self.spec.system.domains.iter().any(|d| d.name == *domain) {
                    owns.insert(DomainHandle::new(domain.clone()));
                } else {
                    complete = false;
                    self.off_contract = true;
                }
            }
            let mut accepts = BTreeSet::new();
            for command in &component.accepts {
                match self.command_of(command, commands) {
                    Found::Handle(handle) => {
                        accepts.insert(handle);
                    }
                    Found::Unresolved => complete = false,
                    Found::Missing => {
                        complete = false;
                        self.off_contract = true;
                    }
                }
            }
            let mut publishes = BTreeSet::new();
            for event in &component.publishes {
                match self.event_of(event, events) {
                    Found::Handle(handle) => {
                        publishes.insert(handle);
                    }
                    Found::Unresolved => complete = false,
                    Found::Missing => {
                        complete = false;
                        self.off_contract = true;
                    }
                }
            }
            if complete {
                resolved.insert(
                    component.name.clone(),
                    ResolvedComponent {
                        name: component.name,
                        owns,
                        accepts,
                        publishes,
                        naming: component.naming,
                    },
                );
            }
        }
        resolved
    }

    /// Every workload, with the component it runs resolved.
    ///
    /// "The topology names a component nobody declared" is `ess-domain`'s `validate_topology`, so it
    /// is not refused again here.
    fn workloads(
        &mut self,
        components: &BTreeMap<ComponentName, ResolvedComponent>,
    ) -> BTreeMap<ComponentName, ResolvedWorkload> {
        let declared: Vec<Workload> = self.spec.topology.workloads.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for workload in declared {
            let name = workload.component.clone();
            if !components.contains_key(&name) {
                if !self.spec.components.contains_key(&name) {
                    self.off_contract = true;
                }
                continue;
            }
            resolved.insert(
                name.clone(),
                ResolvedWorkload {
                    component: ComponentHandle::new(name),
                    replicas: workload.replicas,
                    stateless: workload.stateless,
                    requires: workload.requires,
                },
            );
        }
        resolved
    }

    // ---- bindings -------------------------------------------------------------------------

    /// Every binding, with its trigger, its target and its mapping resolved.
    fn bindings(
        &mut self,
        events: &BTreeMap<QualifiedName, ResolvedEvent>,
        commands: &BTreeMap<QualifiedName, ResolvedCommand>,
    ) -> BTreeMap<BindingName, ResolvedBinding> {
        let declared: Vec<BindingSpec> = self.spec.bindings.values().cloned().collect();
        let mut resolved = BTreeMap::new();
        for binding in declared {
            let path = format!("bindings.{}", binding.name);
            let needles = vec![
                format!("id: {}", binding.name),
                format!("name: {}", binding.name),
            ];
            let event = self.event_of(&binding.event, events);
            if matches!(event, Found::Missing) {
                let available = self.spec.events.keys().map(ToString::to_string).collect();
                let span = self.locator.span(path.clone(), &needles);
                self.refuse_undeclared(
                    codes::BINDING_UNDECLARED_REFERENCE,
                    format!("binding `{}` reacts to an undeclared event", binding.name),
                    &binding.event,
                    "event",
                    available,
                    span,
                );
            }
            // Resolved beside the trigger and the target, because it is the same kind of reference:
            // a binding that escalated into an event nobody declares would put a fact in the IR
            // that nothing reading the IR could look up.
            let mut escalation = None;
            let mut escalation_resolved = true;
            if let Some(emitted) = &binding.escalation {
                match self.event_of(emitted, events) {
                    Found::Handle(handle) => escalation = Some(handle),
                    Found::Unresolved => escalation_resolved = false,
                    Found::Missing => {
                        escalation_resolved = false;
                        let available = self.spec.events.keys().map(ToString::to_string).collect();
                        let mut escalation_needles = vec![format!("emits: {emitted}")];
                        escalation_needles.extend_from_slice(&needles);
                        let span = self.locator.span(
                            format!("{path}.on_failure.escalate.emits"),
                            &escalation_needles,
                        );
                        self.refuse_undeclared(
                            codes::BINDING_UNDECLARED_REFERENCE,
                            format!(
                                "binding `{}` escalates into an undeclared event",
                                binding.name
                            ),
                            emitted,
                            "event",
                            available,
                            span,
                        );
                    }
                }
            }
            let command = self.command_of(&binding.command, commands);
            if matches!(command, Found::Missing) {
                let available = self.spec.commands.keys().map(ToString::to_string).collect();
                let span = self.locator.span(path.clone(), &needles);
                self.refuse_undeclared(
                    codes::BINDING_UNDECLARED_REFERENCE,
                    format!("binding `{}` invokes an undeclared command", binding.name),
                    &binding.command,
                    "command",
                    available,
                    span,
                );
            }
            let (Found::Handle(event_handle), Found::Handle(command_handle)) = (event, command)
            else {
                continue;
            };
            // An escalation that did not resolve keeps the whole binding out, rather than putting
            // one in that says it escalates and cannot say into what: that is the shape G2 exists
            // to remove, and reintroducing it here would only move it.
            if !escalation_resolved {
                continue;
            }
            let Some(mapping) = self.mapping(
                &binding,
                &events[&binding.event],
                &commands[&binding.command],
            ) else {
                continue;
            };
            resolved.insert(
                binding.name.clone(),
                ResolvedBinding {
                    name: binding.name,
                    event: event_handle,
                    command: command_handle,
                    mapping,
                    delivery: binding.delivery,
                    failure: binding.failure,
                    escalation,
                    naming: binding.naming,
                },
            );
        }
        resolved
    }

    /// One binding's mapping, in the command's input order.
    ///
    /// The check design §20 calls out as needing to be "strongly typed", and the one place two
    /// independently written declarations have to agree about a type: the event, the command and the
    /// conversion registry are all needed at once, so no single declaration can decide it.
    fn mapping(
        &mut self,
        binding: &BindingSpec,
        event: &ResolvedEvent,
        command: &ResolvedCommand,
    ) -> Option<Vec<ResolvedMapping>> {
        let mut complete = true;
        for target in binding.mapping.keys() {
            if command.input_field(target).is_none() {
                let taken = names(command.input.iter().map(|field| field.name.clone()));
                self.refuse_mapping(
                    binding,
                    codes::BINDING_UNDECLARED_REFERENCE,
                    format!(
                        "binding `{}` fills `{target}`, which `{}` does not take",
                        binding.name, command.name
                    ),
                    vec![Detail::Note {
                        text: format!("`{}` takes: {taken}", command.name),
                    }],
                    target,
                );
                complete = false;
            }
        }

        let mut resolved = Vec::new();
        for input in &command.input {
            match binding.mapping.get(&input.name) {
                None => {
                    // An optional input left unmapped is a decision, not an omission: the command
                    // says the value may be absent.
                    if input.type_ref.is_optional() {
                        continue;
                    }
                    self.refuse_mapping(
                        binding,
                        codes::UNMAPPED_COMMAND_INPUT,
                        format!(
                            "binding `{}` leaves `{}.{}` unfilled",
                            binding.name, command.name, input.name
                        ),
                        vec![Detail::Typed {
                            subject: format!("{}.{}", command.name, input.name),
                            type_ref: input.type_ref.to_string(),
                            requires: true,
                        }],
                        &input.name,
                    );
                    complete = false;
                }
                Some(MappingSource::Literal { value }) => resolved.push(ResolvedMapping {
                    target: input.name.clone(),
                    target_type: input.type_ref.clone(),
                    value: ResolvedMappingValue::Literal {
                        value: value.clone(),
                    },
                    conversion: None,
                }),
                Some(MappingSource::EventField { field }) => {
                    let field = field.clone();
                    match self.mapped_field(binding, event, command, input, &field) {
                        Some(mapped) => resolved.push(mapped),
                        None => complete = false,
                    }
                }
            }
        }
        complete.then_some(resolved)
    }

    /// One mapping from an event field onto a command input.
    fn mapped_field(
        &mut self,
        binding: &BindingSpec,
        event: &ResolvedEvent,
        command: &ResolvedCommand,
        input: &ResolvedField,
        field: &str,
    ) -> Option<ResolvedMapping> {
        let Some(source) = event.field(field) else {
            let carried = names(event.fields.iter().map(|field| field.name.clone()));
            self.refuse_mapping(
                binding,
                codes::MAPPING_READS_UNDECLARED_FIELD,
                format!(
                    "binding `{}` reads `{}.{field}`, which the event does not carry",
                    binding.name, event.name
                ),
                vec![Detail::Note {
                    text: format!("`{}` carries: {carried}", event.name),
                }],
                &input.name,
            );
            return None;
        };

        let value = ResolvedMappingValue::EventField {
            field: field.to_owned(),
            type_ref: source.type_ref.clone(),
        };
        let from = spec_type_ref(&source.type_ref);
        let to = spec_type_ref(&input.type_ref);
        let conversion = if is_assignable(&from, &to) {
            None
        } else if let Some(crossing) = self
            .spec
            .conversions
            .iter()
            .find(|crossing| crossing.from == from && crossing.to == to)
        {
            Some(crossing.because.clone())
        } else {
            // Design §29's worked example, field for field: the two subjects, the two types, and
            // which way the conversion nobody declared would have to go.
            self.refuse_mapping(
                binding,
                codes::MAPPING_TYPE_MISMATCH,
                format!("binding `{}` is invalid", binding.name),
                vec![
                    Detail::Typed {
                        subject: format!("{}.{field}", event.name),
                        type_ref: source.type_ref.to_string(),
                        requires: false,
                    },
                    Detail::Typed {
                        subject: format!("{}.{}", command.name, input.name),
                        type_ref: input.type_ref.to_string(),
                        requires: true,
                    },
                    Detail::Note {
                        text: format!(
                            "no conversion from `{}` to `{}` is declared",
                            source.type_ref, input.type_ref
                        ),
                    },
                ],
                &input.name,
            );
            return None;
        };

        Some(ResolvedMapping {
            target: input.name.clone(),
            target_type: input.type_ref.clone(),
            value,
            conversion,
        })
    }

    /// Refuses one mapping entry, pointing at the line the entry is written on.
    fn refuse_mapping(
        &mut self,
        binding: &BindingSpec,
        code: Code,
        message: String,
        details: Vec<Detail>,
        target: &str,
    ) {
        let mut needles = Vec::new();
        if let Some(source) = binding.mapping.get(target) {
            needles.push(match source {
                MappingSource::EventField { field } => {
                    format!("{target}: {}{field}", MappingSource::EVENT_PREFIX)
                }
                MappingSource::Literal { value } => format!("{target}: {value}"),
            });
        }
        needles.push(format!("id: {}", binding.name));
        needles.push(format!("name: {}", binding.name));
        let span = self.locator.span(
            format!("bindings.{}.mapping.{target}", binding.name),
            &needles,
        );
        self.refuse(
            code,
            message,
            details,
            Some(
                "a mapping fills every required input of the command it invokes, from a field the \
                 event carries or from a literal",
            ),
            span,
        );
    }

    // ---- domains --------------------------------------------------------------------------

    /// Every domain, listing what the IR holds of it.
    ///
    /// A roster entry for a member that did not resolve is left out rather than carried as a name:
    /// the member's own refusal was already reported, and the alternative is a domain that claims to
    /// own something the IR cannot hand back.
    fn domains(&mut self, members: &Members<'_>) -> BTreeMap<QualifiedName, ResolvedDomain> {
        let mut resolved = BTreeMap::new();
        for domain in &self.spec.system.domains {
            resolved.insert(
                domain.name.clone(),
                ResolvedDomain {
                    name: domain.name.clone(),
                    naming: domain.naming.clone(),
                    types: members
                        .types
                        .keys()
                        .filter(|name| name.is_within(&domain.name))
                        .map(|name| TypeHandle::new(name.clone()))
                        .collect(),
                    entities: domain
                        .entities
                        .iter()
                        .filter(|name| members.entities.contains_key(*name))
                        .map(|name| EntityHandle::new(name.clone()))
                        .collect(),
                    commands: domain
                        .commands
                        .iter()
                        .filter(|name| members.commands.contains_key(*name))
                        .map(|name| CommandHandle::new(name.clone()))
                        .collect(),
                    events: domain
                        .events
                        .iter()
                        .filter(|name| members.events.contains_key(*name))
                        .map(|name| EventHandle::new(name.clone()))
                        .collect(),
                    errors: domain
                        .errors
                        .iter()
                        .filter(|name| members.errors.contains_key(*name))
                        .map(|name| ErrorHandle::new(name.clone()))
                        .collect(),
                    views: domain
                        .views
                        .iter()
                        .filter(|name| members.views.contains_key(*name))
                        .map(|name| ViewHandle::new(name.clone()))
                        .collect(),
                    actors: domain
                        .actors
                        .iter()
                        .filter(|name| members.actors.contains_key(*name))
                        .map(|name| ActorHandle::new(name.clone()))
                        .collect(),
                },
            );
        }
        resolved
    }
}

/// What resolved, for the pass that writes each domain's roster.
///
/// One argument rather than seven: a roster lists every kind of member, and a parameter list that
/// grows with the model is a parameter list someone eventually passes in the wrong order.
struct Members<'a> {
    types: &'a BTreeMap<QualifiedName, ResolvedType>,
    entities: &'a BTreeMap<QualifiedName, ResolvedEntity>,
    commands: &'a BTreeMap<QualifiedName, ResolvedCommand>,
    events: &'a BTreeMap<QualifiedName, ResolvedEvent>,
    errors: &'a BTreeMap<QualifiedName, ResolvedError>,
    views: &'a BTreeMap<QualifiedName, ResolvedView>,
    actors: &'a BTreeMap<QualifiedName, ResolvedActor>,
}

/// The domain's spelling of a resolved reference.
///
/// Private, and it stays private: the IR points one way, and a public route back to a name would be
/// a route back to the question this crate answers. It exists because
/// [`ConversionRegistry`](ess_domain::types::ConversionRegistry) is asked about
/// [`TypeRef`]s, and re-resolving a field to obtain one would be a second resolution path.
///
/// Unbounded recursion on a bounded tree: this walks a [`ResolvedTypeRef`], whose depth is the
/// parsed [`TypeRef`]'s depth, which [`TypeRef::parse`] refuses past
/// [`MAX_TYPE_DEPTH`](ess_domain::types::MAX_TYPE_DEPTH).
fn spec_type_ref(reference: &ResolvedTypeRef) -> TypeRef {
    match reference {
        ResolvedTypeRef::Primitive { name } => TypeRef::Primitive(*name),
        ResolvedTypeRef::Declared { name } => TypeRef::Named(name.name().clone()),
        ResolvedTypeRef::Optional { of } => TypeRef::Optional(Box::new(spec_type_ref(of))),
        ResolvedTypeRef::List { of } => TypeRef::List(Box::new(spec_type_ref(of))),
        ResolvedTypeRef::Map { key, value } => TypeRef::Map(*key, Box::new(spec_type_ref(value))),
    }
}

/// A list for a reader, in the order it was given.
fn names(values: impl IntoIterator<Item = String>) -> String {
    let listed: Vec<String> = values.into_iter().collect();
    if listed.is_empty() {
        return "nothing".to_owned();
    }
    listed.join(", ")
}

/// The IR's spelling of an outcome's condition.
fn condition_of(outcome: &Outcome) -> ResolvedCondition {
    match &outcome.condition {
        OutcomeCondition::When(predicate) => ResolvedCondition::When {
            predicate: predicate.clone(),
        },
        OutcomeCondition::Otherwise => ResolvedCondition::Otherwise,
        OutcomeCondition::External { cause } => ResolvedCondition::External {
            cause: cause.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The register is the interface a harness matches on, so its shape is asserted rather than
    /// assumed. The uniqueness of the codes themselves is a compile-time assertion above; this is
    /// the part that has to be observed at runtime.
    #[test]
    fn every_code_renders_as_its_family_and_number() {
        // `ESS-BINDING-002` is the code design §29 prints. It is reached by pairing, not by
        // counting: the binding layer with the type-mismatch class.
        assert_eq!(codes::MAPPING_TYPE_MISMATCH.to_string(), "ESS-BINDING-002");
        assert_eq!(codes::UNDECLARED_TYPE.to_string(), "ESS-TYPE-001");
        assert_eq!(codes::UNINHABITABLE_TYPE.to_string(), "ESS-TYPE-008");
        // One number per kind across every family: `-001` is a reference to something undeclared
        // wherever it is written, so a reader learns it once.
        assert_eq!(
            codes::ENTITY_UNDECLARED_REFERENCE.to_string(),
            "ESS-ENTITY-001"
        );
        assert_eq!(codes::VIEW_UNDECLARED_REFERENCE.to_string(), "ESS-VIEW-001");
        assert_eq!(
            codes::ACTOR_UNDECLARED_REFERENCE.to_string(),
            "ESS-ACTOR-001"
        );
        for code in codes::ALL {
            let rendered = code.to_string();
            assert!(rendered.starts_with("ESS-"), "{rendered}");
            assert_eq!(
                rendered.rsplit('-').next().map(str::len),
                Some(3),
                "{rendered}"
            );
        }
    }

    #[test]
    fn the_register_lists_every_code_it_declares() {
        for code in [
            codes::UNVALIDATED_SPECIFICATION,
            codes::UNDECLARED_TYPE,
            codes::UNINHABITABLE_TYPE,
            codes::ENTITY_UNDECLARED_REFERENCE,
            codes::VIEW_UNDECLARED_REFERENCE,
            codes::ACTOR_UNDECLARED_REFERENCE,
            codes::COMMAND_UNDECLARED_REFERENCE,
            codes::EVENT_UNDECLARED_REFERENCE,
            codes::ERROR_UNDECLARED_REFERENCE,
            codes::BINDING_UNDECLARED_REFERENCE,
            codes::MAPPING_TYPE_MISMATCH,
            codes::MAPPING_READS_UNDECLARED_FIELD,
            codes::UNMAPPED_COMMAND_INPUT,
        ] {
            assert!(codes::ALL.contains(&code), "{code} is not in ALL");
        }
    }

    #[test]
    fn every_named_code_is_a_family_paired_with_a_class() {
        // The property that makes a collision impossible rather than merely unlikely: a code is not
        // a number someone chose, it is one of twelve classes in one of twelve families.
        for code in codes::ALL {
            assert!(
                codes::family::ALL.contains(&code.family),
                "{code} has a family outside the space"
            );
            assert!(
                codes::class::ALL.contains(&code.number),
                "{code} has a class outside the space"
            );
        }
    }

    #[test]
    fn a_refusal_from_the_domain_crate_keeps_the_code_the_compiler_would_have_given_it() {
        // The bridge's whole point. `ess-domain` refuses a mapping whose types disagree; this pass
        // would have called that `ESS-BINDING-002`; a consumer must not be able to tell which ran.
        let errors = ValidationErrors::new().with(
            aep_domain::error::ValidationError::new(
                ValidationCode::TypeMismatch,
                "binding.notify-on-ordered.mapping.recipient",
                "`shop.orders.Email` is not `shop.orders.Address`",
            )
            .with_hint("declare the crossing"),
        );

        let diagnostics = diagnose(&errors, &SourceMap::new());
        assert!(
            diagnostics.contains(codes::MAPPING_TYPE_MISMATCH),
            "{diagnostics}"
        );
        let diagnostic = &diagnostics.as_slice()[0];
        assert_eq!(diagnostic.hint.as_deref(), Some("declare the crossing"));
        assert_eq!(
            diagnostic.span.as_ref().expect("a span").path,
            "binding.notify-on-ordered.mapping.recipient",
            "the document path survives even with no file to search"
        );
    }

    #[test]
    fn a_refusal_is_filed_under_the_layer_its_document_path_names() {
        for (location, expected) in [
            ("types.shop.Money", codes::family::TYPE),
            (
                "command.shop.Place.outcomes.accepted.emits",
                codes::family::COMMAND,
            ),
            ("binding.notify.mapping.recipient", codes::family::BINDING),
            ("component invoice-service", codes::family::COMPONENT),
            (
                "topology.workloads.invoice-service",
                codes::family::TOPOLOGY,
            ),
            ("entity shop.Order", codes::family::ENTITY),
            ("view.shop.Orders.filter", codes::family::VIEW),
            ("system.domains", codes::family::SPEC),
        ] {
            assert_eq!(family_of(location), expected, "{location}");
        }
    }

    #[test]
    fn a_bridged_refusal_is_located_by_the_declaration_its_path_names() {
        let mut sources = SourceMap::new();
        sources.insert(
            "components.yaml",
            "bindings:\n  - id: notify-on-ordered\n    mapping:\n      recipient: event.customer_email\n",
        );
        let errors = ValidationErrors::new().with(aep_domain::error::ValidationError::new(
            ValidationCode::TypeMismatch,
            "binding.notify-on-ordered.mapping.recipient",
            "the two types disagree",
        ));

        let diagnostics = diagnose_locating(&errors, &sources, &["components.yaml"]);
        let span = diagnostics.as_slice()[0].span.as_ref().expect("a span");
        assert_eq!(
            span.located.expect("the mapping entry was found").line,
            4,
            "the line the entry is written on, not the top of the file"
        );
    }

    #[test]
    fn a_code_the_bridge_has_no_class_for_still_gets_one() {
        // `ValidationCode` is `#[non_exhaustive]`; a code added there must not stop this compiling or
        // start producing codes outside the space.
        for code in ValidationCode::ALL {
            let class = class_of(*code);
            assert!(
                codes::class::ALL.contains(&class),
                "{} maps outside the class space",
                code.as_str()
            );
        }
    }

    /// One file, in the shape a specification is written in.
    fn sources() -> SourceMap {
        let mut sources = SourceMap::new();
        sources.insert(
            "domains/invoice.yaml",
            "types:\n  - name: billing.invoice.Email\n    kind: newtype\n    of: String\n\nevents:\n  - name: billing.invoice.InvoiceCreated\n    fields:\n      - name: customer_email\n        type: billing.invoice.Email\n",
        );
        sources
    }

    #[test]
    fn a_declaration_written_once_is_located_at_its_own_line_and_column() {
        let sources = sources();
        let locator = Locator::new(&sources, &["domains/invoice.yaml"]);
        let span = locator.span(
            "types.billing.invoice.Email",
            &["name: billing.invoice.Email".to_owned()],
        );

        assert_eq!(span.source, "domains/invoice.yaml");
        let located = span.located.expect("the needle occurs once");
        // `  - name: …` — the column is where the needle starts, which is where the reader looks.
        assert_eq!((located.line, located.column), (2, 5));
        assert_eq!(span.to_string(), "domains/invoice.yaml:2:5");
    }

    #[test]
    fn a_needle_that_occurs_twice_is_not_located_because_the_wrong_line_is_worse_than_none() {
        let mut sources = SourceMap::new();
        sources.insert("a.yaml", "type: String\ntype: String\n");
        let locator = Locator::new(&sources, &["a.yaml"]);

        let span = locator.span("types.x", &["type: String".to_owned()]);
        assert!(
            span.located.is_none(),
            "two occurrences cannot both be the one meant"
        );
        assert_eq!(span.to_string(), "<document> (types.x)");
    }

    #[test]
    fn the_second_needle_is_tried_when_the_first_is_ambiguous() {
        let mut sources = SourceMap::new();
        sources.insert(
            "a.yaml",
            "types:\n  - name: shop.A\n    of: String\n  - name: shop.B\n    of: String\n",
        );
        let locator = Locator::new(&sources, &["a.yaml"]);

        // `of: String` twice, `name: shop.B` once: the fallback is coarser and still right.
        let span = locator.span(
            "types.shop.B",
            &["of: String".to_owned(), "name: shop.B".to_owned()],
        );
        assert_eq!(
            span.located.expect("the fallback needle occurs once").line,
            4
        );
    }

    #[test]
    fn a_needle_in_two_files_is_not_located_because_one_of_them_is_wrong() {
        let mut sources = SourceMap::new();
        sources.insert("a.yaml", "name: shop.Thing\n");
        sources.insert("b.yaml", "name: shop.Thing\n");
        let locator = Locator::new(&sources, &["a.yaml", "b.yaml"]);

        assert!(locator
            .span("types.shop.Thing", &["name: shop.Thing".to_owned()])
            .located
            .is_none());
    }

    #[test]
    fn with_no_files_named_a_span_still_carries_the_document_path() {
        let sources = sources();
        let locator = Locator::new(&sources, &[] as &[&str]);

        let span = locator.span(
            "events.billing.invoice.InvoiceCreated",
            &["name: billing.invoice.InvoiceCreated".to_owned()],
        );
        assert!(span.located.is_none());
        assert_eq!(span.path, "events.billing.invoice.InvoiceCreated");
    }
}
