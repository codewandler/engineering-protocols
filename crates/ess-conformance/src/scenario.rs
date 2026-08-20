//! The canonical scenario IR: what a conformance suite *is*, before anything runs one.
//!
//! Design §21. A [`ConformanceSuite`] is the serialisable definition of every check a specification
//! obliges an implementation to pass. It executes nothing, reaches no target, holds no clock and
//! knows nothing about Rust: it is the stable bridge between ESS semantics and whatever technology
//! eventually executes them.
//!
//! ```text
//! EssIr → ConformanceSuite → Rust runner
//!                          → a future HTTP runner
//!                          → a future certification runner in another language
//! ```
//!
//! # Why the definition comes before the runner
//!
//! Design §22, and it is the whole reason this module exists on its own. Generate Rust tests
//! straight from an [`EssIr`] and *the first runner becomes the semantic
//! definition by accident* — the answer to "what does this specification require?" is then buried in
//! whichever executor happened to be written first, and the second executor is a rewrite rather than
//! a port. So the suite is data, it outlives the process that produced it, and a runner is a
//! consumer of it.
//!
//! # Every reference here is a name, never a handle
//!
//! An `EssIr` handle is valid only inside the IR that minted it; using one against a different IR
//! **panics by design** — "a handle belongs to the IR that minted it"
//! (`crates/ess-compiler/src/ir.rs`). Inside one process that is a programming mistake, not a
//! specification's problem. A committed suite is not inside one process: it is written to a file,
//! drift-checked in CI, read back on a later checkout by a later build, and referred to by scenario
//! id from a fault matrix.
//!
//! So every reference a suite holds is a stable ESS semantic name — [`CommandRef`], [`OutcomeRef`],
//! [`EventRef`], [`ViewRef`], [`EssSemanticRef`] — resolvable against any compilation of the same
//! specification. No handle, no index into a `Vec`, no slot number. The rule is not enforced by a
//! convention: every type here parses from text, so
//! `a_suite_parses_from_text_alone_without_an_ir` builds a whole suite with no `EssIr` in scope at
//! all, which a handle cannot survive — it has no public constructor.
//!
//! Minting a name *from* a handle is what the generator does, and `From<&CommandHandle>` and its
//! siblings are that one-way door: a handle goes in, a name comes out, and nothing carries the
//! handle onward.
//!
//! # JSON, one document, semantic ids
//!
//! Three decisions this module takes, each recorded where it is implemented:
//!
//! | decision | where | in one line |
//! |---|---|---|
//! | the format is JSON | [`ConformanceSuite::to_canonical_json`] | the repository's canonical form already, and the one format every future runner can parse |
//! | one document per specification, not one per component | [`ConformanceSuite`] | a binding scenario crosses two components and belongs to neither |
//! | a scenario id is a semantic name | [`ScenarioId`] | a counter renumbers the world when one outcome is inserted |
//!
//! # Determinism
//!
//! Design §37. Same IR in, byte-identical suite out: [`BTreeMap`] and [`BTreeSet`] only, no clock,
//! no RNG, canonical serialisation, trailing newline. Sentences like that are worth nothing
//! unasserted, so `tests/suite.rs` serialises the same suite twice and compares bytes, and reads
//! this crate's own sources for the tokens that would break the claim.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use aep_domain::error::ParseError;
use aep_domain::evidence::SpecDigest;
use aep_domain::node::Node;
use aep_domain::predicate::Predicate;
use ess_compiler::ir::{
    ActorHandle, CommandHandle, ComponentHandle, DomainHandle, EntityHandle, ErrorHandle, EssIr,
    EventHandle, TypeHandle, ViewHandle,
};
use ess_domain::binding::BindingName;
use ess_domain::command::OutcomeName;
use ess_domain::component::ComponentName;
use ess_domain::entity::StateName;
use ess_domain::name::{QualifiedName, Version};
use ess_domain::types::Primitive;

// ---- the suite -----------------------------------------------------------------------------

/// Every check one specification obliges an implementation to pass.
///
/// # One document, not one per component
///
/// `ess-gen` writes `OpenAPI` and `AsyncAPI` **per component**, because a contract is a promise one
/// component makes. A conformance suite is not: `notify-on-invoice-created` starts with a command
/// `invoice-service` accepts and ends with an event `email-service` publishes, so a per-component
/// filing has no drawer for it and would either duplicate the scenario or drop it. Two further
/// reasons point the same way — [`SuiteProvenance`] is a fact about the specification, so N files
/// would hold N copies of one digest with nothing keeping them in step, and a fault matrix refers to
/// scenario ids across the whole system, so a reader chasing one id would have to know which file to
/// open first.
///
/// # Keyed by id, not a list
///
/// Design §21 sketches `scenarios: Vec<ConformanceScenario>`. This is a [`BTreeMap`] instead, and
/// the difference buys two properties the sketch leaves to a convention: two scenarios about the
/// same thing in the same way **cannot** both be in a suite ([`ConformanceSuite::insert`] refuses
/// the second), and the file's order is the id order rather than the order a generator happened to
/// walk its input. A scenario therefore does not repeat its own id as a field — the id is the
/// heading it appears under, which is also why inserting one outcome adds one keyed block to the
/// artifact and moves nothing else.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConformanceSuite {
    /// Which specification this suite checks, and what produced it.
    pub provenance: SuiteProvenance,
    /// Every scenario, by the name of what it is about.
    pub scenarios: BTreeMap<ScenarioId, ConformanceScenario>,
}

impl ConformanceSuite {
    /// An empty suite for one specification.
    pub fn new(provenance: SuiteProvenance) -> Self {
        Self {
            provenance,
            scenarios: BTreeMap::new(),
        }
    }

    /// Adds a scenario, refusing a second one under the same id.
    ///
    /// Refusing rather than replacing, because the two scenarios disagree about what one construct
    /// requires and silently keeping the later one publishes a suite with a check nobody asked for
    /// and without the one they did. The id comes back so the caller can name it in a diagnostic.
    pub fn insert(
        &mut self,
        id: ScenarioId,
        scenario: ConformanceScenario,
    ) -> Result<(), ScenarioId> {
        if self.scenarios.contains_key(&id) {
            return Err(id);
        }
        self.scenarios.insert(id, scenario);
        Ok(())
    }

    /// The scenario about `id`, if the suite has one.
    pub fn scenario(&self, id: &ScenarioId) -> Option<&ConformanceScenario> {
        self.scenarios.get(id)
    }

    /// How many scenarios the suite holds.
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// `true` when the suite checks nothing.
    ///
    /// Which is a finding, not a state to render: a specification that produces no scenario has
    /// nothing an implementation can be held to.
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }

    /// Every construct any scenario in this suite depends on.
    ///
    /// The union of every [`ConformanceScenario::source`]. What a later wave's semantic diff asks
    /// first: *did this change touch anything this suite's result rests on?*
    pub fn dependencies(&self) -> BTreeSet<&EssSemanticRef> {
        self.scenarios
            .values()
            .flat_map(|scenario| scenario.source.iter())
            .collect()
    }

    /// The suite as canonical JSON, with a trailing newline.
    ///
    /// # Why JSON
    ///
    /// Three reasons, in the order they decided it. It is **already this repository's canonical
    /// form**: [`EssIr::to_canonical_json`] is `serde_json::to_string_pretty` plus a newline, and the
    /// committed artifacts under `generated/` that a drift check compares are JSON, so a suite in any
    /// other format would be the one file in that tree with a different definition of "unchanged".
    /// It is **parseable by every future runner** — design §22's whole promise is a suite an HTTP
    /// runner or a certification runner in another language can read, and JSON is the format that
    /// needs no library nobody has. And it has **one canonical spelling here**, where YAML does not:
    /// `serde_yaml` decides quoting, folding and block style by heuristics on the value, so two
    /// versions of one suite can differ in bytes without differing in meaning — which is exactly what
    /// a byte-comparison drift check cannot tolerate.
    ///
    /// Canonical means the same three things it means for the IR: key order comes from [`BTreeMap`],
    /// the indentation is `serde_json`'s two spaces, and the last byte is a newline, because a file
    /// without one shows up as modified in the next diff.
    ///
    /// # Panics
    ///
    /// It does not. `serde_json` has exactly one error of its own — a map key that is not a string —
    /// and the only maps here are keyed by [`ScenarioId`], which serialises as one, and by `String`.
    /// The `unwrap_or_else` names the impossible case rather than hiding it. A [`Node`] holding a
    /// non-finite float is written as `null`, which is the same known defect
    /// [`EssIr::to_canonical_json`] records; it is a defect in what the model accepts on input, not
    /// in this function.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("a conformance suite serialises: {error}"));
        json.push('\n');
        json
    }

    /// Reads a suite back from canonical JSON.
    ///
    /// The other half of design §49's step-1 acceptance — *a suite serialized in one process
    /// resolves in another*. Every name is parsed on the way in, so a suite naming something that is
    /// not a well-formed ESS name is refused here rather than at the first step that tries to use
    /// it. Whether those names still exist in a specification is a different question, and one only
    /// an `EssIr` can answer.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

// ---- provenance ----------------------------------------------------------------------------

/// Where a suite came from, and what produced it.
///
/// Design §23. A passing conformance result means nothing without these: *conformant* is a claim
/// about one implementation, against one specification, checked by one suite, and each of those
/// moves independently.
///
/// # This reuses `ess-gen`'s provenance rather than restating it
///
/// [`ess_gen::Provenance`] already answers "which specification produced this artifact", and every
/// projection under `generated/` carries it. [`SuiteProvenance::of`] therefore *derives* from it:
/// the system, the specification version, the digest and the compiler version are read from a
/// [`ess_gen::Provenance`] built for the same IR, never computed a second way. Two provenance types
/// in one repository are two answers to one question, and the point of provenance is that there is
/// one.
///
/// What is added is what a suite has and a projection does not: the version of the thing that
/// synthesised the scenarios, and the format of the document itself.
///
/// # Why the fields are not `ess_gen::Provenance` itself
///
/// A suite is read back (§21, §49). [`ess_gen::Provenance`] cannot be: its `compiler_version` and
/// `generator_version` are `&'static str`, which no parsed document can produce without leaking, and
/// it derives `Serialize` alone. Holding one here would make the write path reuse and the read path
/// impossible — so the shared facts are copied from it once, at the only place that can go wrong,
/// and `the_suite_records_the_same_model_digest_the_projections_do` fails if the two ever disagree.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SuiteProvenance {
    /// The format this document is written in — what a runner checks before reading the rest.
    pub suite_version: SuiteFormat,
    /// The system the suite checks.
    pub system: String,
    /// The version of that system's specification, such as `v3`.
    pub specification_version: String,
    /// A digest of the resolved model the suite was generated from.
    ///
    /// The field the record is worth anything for. `billing/v3` is a label two different resolutions
    /// can share; a digest is not, and
    /// [`EssConformanceResult::attests`](aep_domain::evidence::EssConformanceResult::attests) decides
    /// on this and never on the label.
    pub spec_digest: SpecDigest,
    /// The build that resolved the specification.
    pub compiler_version: String,
    /// The build that writes the projections under `generated/`.
    pub generator_version: String,
    /// The build that synthesised these scenarios.
    ///
    /// Design's open decision D4, taken as its default: two fields, because once the synthesizer is
    /// not `ess-gen` a report that cannot say which oracle produced a verdict is not reproducible —
    /// which is the entire purpose of the field.
    pub synthesizer_version: String,
}

impl SuiteProvenance {
    /// This crate's version: the build that synthesises scenarios.
    pub const SYNTHESIZER_VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Derives provenance from the model a suite is generated from.
    ///
    /// # Panics
    ///
    /// If `ess-gen`'s digest stops being a digest. It is the full 64 lower-case hexadecimal characters of a SHA-256
    /// by construction — the length [`SpecDigest::MAX_LENGTH`] documents as what `ess-gen` writes —
    /// so the panic is how the two crates disagreeing becomes visible immediately rather than as an
    /// evidence record the protocol engine silently refuses later.
    pub fn of(ir: &EssIr) -> Self {
        let projection = ess_gen::Provenance::of(ir);
        let spec_digest = SpecDigest::new(projection.source_digest.as_str()).unwrap_or_else(|error| {
            panic!(
                "`ess-gen` writes a digest `aep-domain` accepts: {error}; the two have drifted, and \
                 a conformance record carrying an unparsable digest attests nothing"
            )
        });
        Self {
            suite_version: SuiteFormat::CURRENT,
            system: projection.system,
            specification_version: projection.specification_version,
            spec_digest,
            compiler_version: projection.compiler_version.clone(),
            generator_version: projection.generator_version.clone(),
            synthesizer_version: Self::SYNTHESIZER_VERSION.to_owned(),
        }
    }
}

/// Suite format major versions this build implements.
pub const SUPPORTED_SUITE_FORMATS: &[u32] = &[1];

/// The version of the *document shape* a suite is written in — `ess-conformance/1`.
///
/// The first thing a runner reads and the first thing it can refuse. A later format may mean
/// something different by the same words, and a reader that guesses executes a suite nobody wrote —
/// the same rule, and the same reasoning, as
/// [`FormatVersion`](ess_domain::system::FormatVersion) for the specification language.
///
/// # Why this is what `suite_version` means
///
/// Design §23 asks for a `suite_version` beside the spec and compiler versions. A suite's *contents*
/// are already identified exactly by [`SuiteProvenance::spec_digest`] and
/// [`SuiteProvenance::synthesizer_version`] together — the suite is a function of those two — so a
/// hand-maintained content version beside them would be a third answer to a question already
/// answered twice, and the one that drifts. What is genuinely unversioned without this is the shape
/// of the document, which is what a future runner in another language must check before parsing.
///
/// It is separate from the specification language's format on purpose: `ess/1` versions what an
/// author writes, this versions what a synthesizer writes, and tying them together would force one
/// to move whenever the other did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct SuiteFormat(Version);

impl SuiteFormat {
    /// The first, and so far only, suite format.
    pub const CURRENT: Self = Self(Version::V1);

    /// How a suite format is written.
    pub const PREFIX: &'static str = "ess-conformance/";

    /// The numeric part.
    pub fn major(self) -> u32 {
        self.0.get()
    }

    /// `true` when this build implements it.
    pub fn is_supported(self) -> bool {
        SUPPORTED_SUITE_FORMATS.contains(&self.major())
    }

    /// Parses `ess-conformance/1`.
    ///
    /// The digits are read by [`Version::parse`] rather than by a rule written again here: `01`,
    /// `+1`, `0` and `4294967296` are refused because that function refuses them, and a second
    /// spelling of one version is two documents that disagree textually and agree semantically.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let digits = value.strip_prefix(Self::PREFIX).ok_or_else(|| {
            ParseError::reference(
                "suite format",
                value,
                format!("suite formats are written `{}1`", Self::PREFIX),
            )
        })?;
        Version::parse(&format!("v{digits}"))
            .map(Self)
            .map_err(|_| {
                ParseError::reference(
                    "suite format",
                    value,
                    "expected a whole number after the prefix, without a leading zero",
                )
            })
    }
}

impl fmt::Display for SuiteFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0.get())
    }
}

impl From<SuiteFormat> for String {
    fn from(value: SuiteFormat) -> Self {
        value.to_string()
    }
}

impl FromStr for SuiteFormat {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> serde::Deserialize<'de> for SuiteFormat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

// ---- identity ------------------------------------------------------------------------------

/// What a scenario is about — and therefore what it is called.
///
/// # A name, never a counter
///
/// Design §37, and it is the correction that matters most in this module. A counter is stable
/// "across unchanged input", which sounds like enough. Insert one outcome and every scenario after
/// it is renumbered: the committed suite re-keys wholesale, a fault matrix's references rot, every
/// stored report names scenarios that no longer exist, and a semantic diff of two specifications
/// cannot line yesterday's result up with today's scenario — which is the whole basis of deciding
/// whether prior evidence still holds.
///
/// So an id is derived from the construct it exercises. It is a name, so it is diffable, greppable
/// and stable under every change that does not touch what it names, and **two scenarios about the
/// same thing in the same way are the same id** — which is why [`ConformanceSuite`] can key its
/// scenarios by it.
///
/// # How it renders
///
/// Slash-separated segments, subject first, exactly as design §23 writes one:
///
/// ```text
/// billing.invoice.CreateInvoice/outcome/rejected
/// billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled
/// billing.invoice.Invoice/state/Paid/refuses/billing.invoice.CancelInvoice
/// billing.invoice.Invoice/invariant/after/billing.invoice.CreateInvoice/accepted
/// notify-on-invoice-created/binding/on-failure
/// ```
///
/// Subject first because that is the order a person searches in — `grep CreateInvoice` finds every
/// scenario about that command, and sorting groups them together in a report, in the artifact and in
/// a diff. The second segment is a keyword from a closed set (`outcome`, `transition`, `state`,
/// `invariant`, `binding`), which is what makes [`ScenarioId::parse`] total: no ESS name may contain
/// a `/`, so splitting is unambiguous.
///
/// [`Ord`] is defined on the rendered form rather than derived, so the order of the keys in the
/// committed file is plain lexicographic order — any tool that sorts the keys of the JSON object
/// reproduces the file byte for byte instead of producing a diff.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScenarioId {
    /// One declared outcome of one command: `billing.invoice.CreateInvoice/outcome/rejected`.
    ///
    /// The primary unit (§10). An `external` outcome needs no separate spelling — it is still that
    /// command's outcome, reached by a [`ScenarioStep::ConfigureExternalOutcome`] rather than by an
    /// input.
    Outcome {
        /// The branch this scenario proves.
        outcome: OutcomeRef,
    },
    /// One declared transition, taken by the outcome that drives it.
    ///
    /// `billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled`. The driver
    /// is part of the identity because a transition may be reachable by more than one command
    /// outcome, and two scenarios that reach one state by different verbs are two scenarios.
    Transition {
        /// The move being proven.
        transition: TransitionRef,
        /// The command outcome that takes it.
        by: OutcomeRef,
    },
    /// A command that must not be honoured in a state: `…Invoice/state/Paid/refuses/…CancelInvoice`.
    ///
    /// The absence of a transition is itself semantics (§19), and absence has no name of its own to
    /// borrow — so this id names the state and the command whose meeting is refused.
    Refusal {
        /// The entity holding the state.
        entity: EntityRef,
        /// The state it is in.
        state: StateName,
        /// The command that must not move it.
        command: CommandRef,
    },
    /// What must hold of an entity after a branch changed one (§20).
    ///
    /// `billing.invoice.Invoice/invariant/after/billing.invoice.CreateInvoice/accepted`. The
    /// *entity* is the subject, because that is whose invariants these are, and the outcome is part
    /// of the identity because §20 evaluates them **after a state-changing command** — the same
    /// entity checked after two different commands is two checks, and one of them can fail while
    /// the other passes.
    ///
    /// # Why an invariant does not name itself here
    ///
    /// An entity may declare several and the model gives none of them a name, so the only shapes
    /// available would be a position (`…/invariant/0`) or the statement itself. A position is the
    /// counter this whole type exists to refuse — insert an invariant above another and every
    /// scenario after it re-keys. The statement is a sentence, not a name: it changes when an author
    /// rewrites `total.amount >= 0` as `not (total.amount < 0)`, which re-keys a check whose meaning
    /// did not move.
    ///
    /// So one scenario carries every invariant the entity declares, and adding one adds a step
    /// rather than a key. What that costs is granularity in a report — a failure names the scenario
    /// and the runner names which assertion inside it failed — and what it buys is that a stored
    /// result still lines up with today's scenario after an invariant is added, which is what a
    /// semantic diff needs from an id.
    Invariant {
        /// Whose invariants.
        entity: EntityRef,
        /// The branch after which they must hold.
        after: OutcomeRef,
    },
    /// A binding's observable behaviour: `notify-on-invoice-created/binding/flow`.
    Binding {
        /// Which binding.
        binding: BindingRef,
        /// Which of its clauses.
        aspect: BindingAspect,
    },
}

impl ScenarioId {
    /// The keyword that follows the subject, per variant.
    const OUTCOME: &'static str = "outcome";
    const TRANSITION: &'static str = "transition";
    const STATE: &'static str = "state";
    const INVARIANT: &'static str = "invariant";
    const BINDING: &'static str = "binding";

    /// Reads an id back from its rendered form.
    ///
    /// Total against [`Display`](fmt::Display), which
    /// `a_scenario_id_round_trips_through_its_rendered_form` asserts for every variant: an id
    /// written into a fault matrix, a report or a terminal is an id this can turn back into the
    /// construct it names.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let reject = |reason: &str| ParseError::identifier("scenario id", value, reason.to_owned());
        let parts: Vec<&str> = value.split('/').collect();
        let name = |raw: &str| QualifiedName::new(raw).map_err(|_| reject("has a malformed name"));

        match parts.as_slice() {
            [command, Self::OUTCOME, outcome] => Ok(Self::Outcome {
                outcome: OutcomeRef::new(
                    CommandRef::new(name(command)?),
                    OutcomeName::new(outcome)
                        .map_err(|_| reject("has a malformed outcome name"))?,
                ),
            }),
            [entity, Self::TRANSITION, transition, "by", command, outcome] => {
                Ok(Self::Transition {
                    transition: TransitionRef::new(EntityRef::new(name(entity)?), transition)
                        .map_err(|_| reject("has a malformed transition name"))?,
                    by: OutcomeRef::new(
                        CommandRef::new(name(command)?),
                        OutcomeName::new(outcome)
                            .map_err(|_| reject("has a malformed outcome name"))?,
                    ),
                })
            }
            [entity, Self::STATE, state, "refuses", command] => Ok(Self::Refusal {
                entity: EntityRef::new(name(entity)?),
                state: StateName::new(state).map_err(|_| reject("has a malformed state name"))?,
                command: CommandRef::new(name(command)?),
            }),
            [entity, Self::INVARIANT, "after", command, outcome] => Ok(Self::Invariant {
                entity: EntityRef::new(name(entity)?),
                after: OutcomeRef::new(
                    CommandRef::new(name(command)?),
                    OutcomeName::new(outcome)
                        .map_err(|_| reject("has a malformed outcome name"))?,
                ),
            }),
            [binding, Self::BINDING, aspect] => Ok(Self::Binding {
                binding: BindingRef::new(
                    BindingName::new(binding)
                        .map_err(|_| reject("has a malformed binding name"))?,
                ),
                aspect: BindingAspect::parse(aspect).map_err(|()| {
                    reject(&format!(
                        "names no binding aspect; expected one of {}",
                        BindingAspect::expected()
                    ))
                })?,
            }),
            _ => Err(reject(
                "is not a scenario name; expected `<command>/outcome/<name>`, \
                 `<entity>/transition/<name>/by/<command>/<outcome>`, \
                 `<entity>/state/<state>/refuses/<command>`, \
                 `<entity>/invariant/after/<command>/<outcome>` or `<binding>/binding/<aspect>`",
            )),
        }
    }
}

impl fmt::Display for ScenarioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Outcome { outcome } => write!(
                f,
                "{}/{}/{}",
                outcome.command,
                Self::OUTCOME,
                outcome.outcome
            ),
            Self::Transition { transition, by } => write!(
                f,
                "{}/{}/{}/by/{}/{}",
                transition.entity,
                Self::TRANSITION,
                transition.transition,
                by.command,
                by.outcome
            ),
            Self::Refusal {
                entity,
                state,
                command,
            } => write!(f, "{entity}/{}/{state}/refuses/{command}", Self::STATE),
            Self::Invariant { entity, after } => write!(
                f,
                "{entity}/{}/after/{}/{}",
                Self::INVARIANT,
                after.command,
                after.outcome
            ),
            Self::Binding { binding, aspect } => {
                write!(f, "{binding}/{}/{aspect}", Self::BINDING)
            }
        }
    }
}

impl PartialOrd for ScenarioId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScenarioId {
    /// By the rendered name, not by the variant.
    ///
    /// So the keys of the committed JSON object are in plain lexicographic order and every scenario
    /// about one command sits together. A derived ordering would group by variant first, which no
    /// reader of the file can see and no tool that re-sorts the keys would reproduce.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl FromStr for ScenarioId {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<ScenarioId> for String {
    fn from(value: ScenarioId) -> Self {
        value.to_string()
    }
}

impl serde::Serialize for ScenarioId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for ScenarioId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Which clause of a binding a scenario proves.
///
/// A binding is four claims in one declaration — *when this event occurs, invoke this command, with
/// this mapping, delivered this way, and on failure do this* — and each of them fails on its own. A
/// single `binding` scenario would report one verdict for four checks, so a dropped binding and a
/// swapped mapping would arrive as the same red line; and §26 asks a fault matrix to name the *one*
/// scenario each deliberate defect breaks, which it cannot do if there is one scenario per binding.
///
/// They are also not equally demanding of a target, which is the second reason they are separate
/// keys. [`Flow`](Self::Flow) is provable from events alone, which §16 requires; [`Mapping`](Self::Mapping)
/// needs the invocation itself, which §16 permits as additional evidence and refuses to require of
/// every implementation. Filing them apart is what lets a target that cannot expose an invocation
/// report `unsupported` for that one scenario (§28) instead of failing the flow it can prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingAspect {
    /// The event arrives, the command runs, and what the model says follows is observable (§16).
    Flow,
    /// Each of the command's inputs receives the value the binding's mapping names (§16).
    Mapping,
    /// The same event delivered twice does not stop the declared consequence happening (§17).
    Delivery,
    /// The command does not run, and the declared failure policy is observable (§18).
    OnFailure,
}

impl BindingAspect {
    /// Every aspect, with how it is written — what a walk over the four iterates.
    ///
    /// Public for the reason `aep_conformance::Fault::ALL` is, and design §25 says it in as many
    /// words: it is what makes a fault matrix a *matrix* rather than a list someone has to remember
    /// to extend. Synthesis walks it to produce every scenario a binding obliges, and a later meta
    /// test walks the same constant to assert each one is caught by something.
    ///
    /// It is a list rather than the definition: rendering and parsing are total matches over the
    /// variants, so an aspect added without a spelling does not compile, and
    /// `every_binding_aspect_is_in_the_list_that_is_walked_to_produce_them` fails if one is added
    /// without being added here.
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Flow, Self::Flow.written()),
        (Self::Mapping, Self::Mapping.written()),
        (Self::Delivery, Self::Delivery.written()),
        (Self::OnFailure, Self::OnFailure.written()),
    ];

    /// How this aspect is written, everywhere it is written.
    const fn written(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Mapping => "mapping",
            Self::Delivery => "delivery",
            Self::OnFailure => "on-failure",
        }
    }

    /// Reads an aspect back. A word that is none of them is not an aspect this build knows.
    fn parse(value: &str) -> Result<Self, ()> {
        match value {
            "flow" => Ok(Self::Flow),
            "mapping" => Ok(Self::Mapping),
            "delivery" => Ok(Self::Delivery),
            "on-failure" => Ok(Self::OnFailure),
            _ => Err(()),
        }
    }

    /// The words a suite may use, for a diagnostic that lists them.
    fn expected() -> String {
        Self::ALL
            .iter()
            .map(|(_, written)| format!("`{written}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for BindingAspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.written())
    }
}

impl<'de> serde::Deserialize<'de> for BindingAspect {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(|()| {
            serde::de::Error::custom(format!(
                "{raw:?} names no binding aspect; expected one of {}",
                Self::expected()
            ))
        })
    }
}

// ---- a scenario ----------------------------------------------------------------------------

/// One check, as a sequence of steps over an isolated execution context.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConformanceScenario {
    /// What this scenario proves, in one line, for the person reading a report.
    pub purpose: ScenarioPurpose,
    /// What to do, in order.
    pub steps: Vec<ScenarioStep>,
    /// Every construct this scenario's result depends on.
    ///
    /// **Not what caused it to exist.** Design §37's second correction: `derived_from` as originally
    /// sketched lists the command and outcome a scenario was generated from, and what a later
    /// consumer needs is what the scenario *depends on* — the types its input mentions, the entity it
    /// moves, the view it asserts, the event it expects. The two differ exactly where it matters: a
    /// scenario built from `CreateInvoice/rejected` asserting an `InvalidAmount` payload depends on
    /// `Money`, and if `Money` gains a field that scenario's stored result is stale while a
    /// `derived_from` naming only the outcome says nothing.
    ///
    /// Collecting it costs nothing during generation, because the generator has just walked every
    /// one of those constructs to build the scenario. Reconstructing it afterwards means
    /// regenerating the suite.
    ///
    /// A [`BTreeSet`], so the same construct reached by two paths appears once and the order does not
    /// depend on the walk.
    pub source: BTreeSet<EssSemanticRef>,
}

impl ConformanceScenario {
    /// Builds a scenario.
    pub fn new(
        purpose: ScenarioPurpose,
        steps: impl IntoIterator<Item = ScenarioStep>,
        source: impl IntoIterator<Item = EssSemanticRef>,
    ) -> Self {
        Self {
            purpose,
            steps: steps.into_iter().collect(),
            source: source.into_iter().collect(),
        }
    }

    /// `true` when this scenario depends on `construct`.
    pub fn depends_on(&self, construct: &EssSemanticRef) -> bool {
        self.source.contains(construct)
    }
}

/// What a scenario proves, in one line.
///
/// Prose, and deliberately not identity: [`ScenarioId`] is what a report, a fault matrix and a diff
/// key on, so improving this wording re-keys nothing. One line because it is printed beside a
/// verdict; a paragraph there is a paragraph nobody reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct ScenarioPurpose(String);

impl ScenarioPurpose {
    /// The longest accepted purpose.
    pub const MAX_LENGTH: usize = 200;

    /// Builds one, refusing what cannot be printed on one line.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref().trim();
        let reject = |reason: &str| {
            Err(ParseError::identifier(
                "scenario purpose",
                value,
                reason.to_owned(),
            ))
        };
        if value.is_empty() {
            return reject("must say what the scenario proves");
        }
        if value.chars().any(char::is_control) {
            return reject("is one line: a report prints it beside a verdict");
        }
        if value.chars().count() > Self::MAX_LENGTH {
            return reject("is longer than a line; the reasoning belongs in the design document");
        }
        Ok(Self(value.to_owned()))
    }

    /// The purpose as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScenarioPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ScenarioPurpose> for String {
    fn from(value: ScenarioPurpose) -> Self {
        value.0
    }
}

impl FromStr for ScenarioPurpose {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> serde::Deserialize<'de> for ScenarioPurpose {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

/// What one scenario calls one instance while it runs.
///
/// A slot, not an ESS name. The identity itself does not exist until the run that creates it, and
/// design §37 puts every source of variation on the runner's side — so a suite that carried a
/// literal id would be a suite whose meaning changed with the target it ran against, and re-running
/// it would collide with the row the last run left behind. What the suite carries instead is *which*
/// instance: the one bound by the [`CaptureInstance`](ScenarioStep::CaptureInstance) step earlier in
/// the same scenario.
///
/// Lower-kebab, for the reason [`OutcomeName`] is: this name reaches a generated test, a report and
/// a diagnostic, and one spelling in the model is one spelling in all three.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct InstanceName(String);

impl InstanceName {
    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";

    /// Parses one.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let reject = |reason: &str| {
            Err(ParseError::identifier(
                "instance name",
                value,
                reason.to_owned(),
            ))
        };
        if value.is_empty() {
            return reject("must not be empty");
        }
        if !value.starts_with(|character: char| character.is_ascii_lowercase()) {
            return reject("must start with a lower-case letter; instance names are lower-kebab");
        }
        if !value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        }) {
            return reject("may hold only lower-case letters, digits and hyphens");
        }
        if value.ends_with('-') || value.contains("--") {
            return reject("has an empty segment");
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstanceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<InstanceName> for String {
    fn from(value: InstanceName) -> Self {
        value.0
    }
}

impl FromStr for InstanceName {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> serde::Deserialize<'de> for InstanceName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

/// One value a scenario supplies to a command's input, or requires one to have carried.
///
/// Three cases, because a suite can decide one of them and not the other two. A
/// [`Literal`](Self::Literal) is a value synthesis chose and *decided* against the branch's guard;
/// an [`Instance`](Self::Instance) is the identity of something an earlier step brought into
/// existence, which no generator can know and every lifecycle scenario needs; and an
/// [`Observed`](Self::Observed) is a value the run itself produced and published in an event, which
/// is what a binding's mapping is written in terms of.
///
/// Tagged rather than untagged, deliberately: a declared struct may perfectly well have a field
/// called `instance`, and an untagged encoding would read such a value as a reference. A tag is two
/// extra keys in the artifact and no ambiguity at all.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScenarioValue {
    /// A value the suite chose.
    Literal {
        /// The value.
        value: Node,
    },
    /// The identity of the instance bound under this name earlier in the scenario.
    Instance {
        /// Which instance.
        instance: InstanceName,
    },
    /// Whatever this event carried in this field, earlier in this scenario.
    ///
    /// The shape a binding's mapping has: `recipient: event.customer_email` says the command's
    /// `recipient` receives *the value the triggering event published*, and no generator knows what
    /// that value is — the upstream implementation chose it. Naming the event and the field is a
    /// reading of the binding, where a literal would be a guess at what the upstream produced.
    ///
    /// It refers to an observation earlier in the same scenario, exactly as
    /// [`Instance`](Self::Instance) refers to an earlier [`CaptureInstance`](ScenarioStep::CaptureInstance):
    /// a suite that carried the value itself would be a suite whose meaning changed with the target
    /// it ran against.
    Observed {
        /// The event that published it.
        event: EventRef,
        /// The field of that event's payload.
        field: String,
    },
}

impl ScenarioValue {
    /// A literal value.
    pub fn literal(value: Node) -> Self {
        Self::Literal { value }
    }

    /// The identity of a bound instance.
    pub fn instance(name: InstanceName) -> Self {
        Self::Instance { instance: name }
    }

    /// Whatever an event carried in one of its fields.
    pub fn observed(event: EventRef, field: impl Into<String>) -> Self {
        Self::Observed {
            event,
            field: field.into(),
        }
    }

    /// The value, where it is one the suite decided.
    pub fn as_literal(&self) -> Option<&Node> {
        match self {
            Self::Literal { value } => Some(value),
            Self::Instance { .. } | Self::Observed { .. } => None,
        }
    }
}

// ---- payload shapes -------------------------------------------------------------------------

/// What the specification declares a payload is made of, flattened to the leaves that can be
/// checked with nothing but the suite in hand.
///
/// # Why a shape and not values
///
/// An event declares its fields and their types; the command that publishes it declares its input.
/// **Nothing in the model relates one to the other** — no construct says where a payload field's
/// value comes from — so `InvoiceCreated.amount == CreateInvoice.amount` is a name match, and a name
/// match is the inference this crate refuses everywhere else. Asserting a *value* would therefore be
/// a guess dressed as a reading, and a suite that guesses fails correct implementations.
///
/// What survives that argument is the half the model does state: the payload's declared fields are
/// present, and each holds a value of the type it was declared with. That is what this carries, and
/// it is why [`ScenarioStep::ExpectEvent`] has both a `payload` (values, always a reading of
/// something else the model said) and a `shape` (types, always available).
///
/// # Flattened, and why the leaves are dotted paths
///
/// The same deep-path rule [`flatten`](crate::flatten) applies to a command's input, so a payload
/// and an input are held to one reading of what `Optional<Money>` exposes rather than to two:
/// a newtype is transparent, an `Optional` marks its leaves absentable, a struct contributes its
/// fields under `parent.child`, and a list, a map and a union contribute one leaf that says only
/// what container it is. `billing.invoice.InvoiceCreated` therefore flattens to `invoice_id`,
/// `customer_email`, `amount.amount` and `amount.currency`.
///
/// # An undeclared field is not refused
///
/// Only what the specification declares is required. An implementation that publishes an extra
/// field is not contradicting anything the model says — an event's payload is not a closed set here
/// — so a suite that refused one would be enforcing a rule no document wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct PayloadShape {
    leaves: BTreeMap<String, LeafShape>,
}

impl PayloadShape {
    /// A shape that requires nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requires `path` to hold what `leaf` describes.
    pub fn insert(&mut self, path: impl Into<String>, leaf: LeafShape) {
        self.leaves.insert(path.into(), leaf);
    }

    /// Every leaf, by the dotted path that reaches it.
    pub fn leaves(&self) -> &BTreeMap<String, LeafShape> {
        &self.leaves
    }

    /// `true` when the declaration reaches no checkable leaf at all.
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

/// One leaf of a declared payload: what may sit there, and whether anything has to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeafShape {
    /// What the declared type admits.
    #[serde(flatten)]
    pub holds: Holds,
    /// Whether the declaration permits it to be absent.
    ///
    /// `true` where an `Optional` was walked through on the way down, including one wrapping the
    /// struct this leaf is a field of: if the whole value is absent, so is every leaf under it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

impl LeafShape {
    /// A leaf that must be present.
    pub fn required(holds: Holds) -> Self {
        Self {
            holds,
            optional: false,
        }
    }

    /// The same leaf, permitted to be absent.
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Whether `value` is one the declaration admits there.
    ///
    /// `None` means nothing was published at that path, which conforms only where the declaration
    /// said it might be absent.
    pub fn admits(&self, value: Option<&Node>) -> bool {
        match value {
            None | Some(Node::Null) => self.optional,
            Some(value) => self.holds.admits(value),
        }
    }
}

/// What a declared type admits at one leaf.
///
/// The five cases [`flatten`](crate::flatten)'s table already distinguishes, carried as data so that
/// a runner holding only a suite can check them — including one written in another language, which
/// is the whole point of §21. A `DeclaredTypeRef` here would have been a name a runner cannot
/// resolve without the specification, and a suite that only means something beside the document it
/// came from is a cache rather than an artifact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "holds", rename_all = "snake_case")]
pub enum Holds {
    /// A primitive, after every newtype on the way down has been walked through.
    Primitive {
        /// Which one.
        kind: Primitive,
    },
    /// One of a fixed set of names, as text.
    Enum {
        /// The names, in declaration order.
        variants: Vec<String>,
    },
    /// An ordered sequence. Its elements are not described: a path has no index to name one by.
    List,
    /// A mapping with primitive keys. Its values are not described, for the same reason.
    Map,
    /// A tagged union. Only that it is a mapping is checked, exactly as
    /// [`flatten`](crate::flatten) checks one.
    Union,
}

impl Holds {
    /// Whether `value` is of the shape this admits.
    ///
    /// The primitive table is [`crate::input`]'s own, called rather than restated: a payload and a
    /// command input disagreeing about whether `1.5` is an `Integer` would be two readings of one
    /// rule, and the second one is wrong eventually.
    pub fn admits(&self, value: &Node) -> bool {
        match self {
            Self::Primitive { kind } => crate::input::primitive_value(*kind, value).is_some(),
            Self::Enum { variants } => value
                .as_text()
                .is_some_and(|text| variants.iter().any(|variant| variant == text)),
            Self::List => matches!(value, Node::Seq(_)),
            Self::Map | Self::Union => matches!(value, Node::Map(_)),
        }
    }
}

impl fmt::Display for Holds {
    /// Reads as the tail of "`InvoiceCreated.amount.amount` holds …".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive { kind } => write!(f, "a {kind}"),
            Self::Enum { variants } => write!(f, "one of {}", variants.join(", ")),
            Self::List => f.write_str("a list"),
            Self::Map => f.write_str("a mapping"),
            Self::Union => f.write_str("a tagged union, as a mapping"),
        }
    }
}

// ---- steps ---------------------------------------------------------------------------------

/// One thing a scenario does or requires.
///
/// # The vocabulary is closed
///
/// Thirteen steps, and a suite may contain nothing else. That is the point of a scenario IR: a check
/// a runner can perform but this vocabulary cannot express is a semantic the specification does not
/// have, and adding a step here is a decision about what an ESS *means* — not a convenience for one
/// runner. When synthesis cannot express what a construct requires, §18's rule applies: refuse, and
/// say the model is incomplete. Do not reach for an implementation-specific assertion.
///
/// Ten of them are the ones design §21 lists. Three are not, each added when a section of the design
/// could not be written without it, and each argued on the variant itself:
///
/// | step | what could not be said without it | §|
/// |---|---|---|
/// | [`CaptureInstance`](Self::CaptureInstance) | which instance the second command in a sequence acts on | §19 |
/// | [`RedeliverEvent`](Self::RedeliverEvent) | that the same event may arrive twice | §17 |
/// | [`ExpectInvocation`](Self::ExpectInvocation) | which value a binding's mapping put in which input | §16 |
///
/// # Negative assertions are first class
///
/// [`ExpectNoEvent`](ScenarioStep::ExpectNoEvent) and
/// [`ViewExpectation::Excludes`] are steps, not the absence of steps. A happy-path-only suite is
/// non-conformant with the design (§10), and "nothing else happened" is exactly the check a wrong
/// implementation passes by accident when nobody writes it down.
///
/// # Steps run in order, and an assertion is about the command before it
///
/// A scenario is a sequence, and every step after an
/// [`ExecuteCommand`](ScenarioStep::ExecuteCommand) is about *that* invocation:
/// [`ExpectOutcome`](ScenarioStep::ExpectOutcome), [`ExpectError`](ScenarioStep::ExpectError),
/// [`ExpectEvent`](ScenarioStep::ExpectEvent) and
/// [`ExpectNoEvent`](ScenarioStep::ExpectNoEvent) all read the result of the last command executed,
/// which is exactly what §9's `SemanticCommandResult` hands a runner. The same reading is already
/// how [`ExpectView`](ScenarioStep::ExpectView) relates to the
/// [`QueryView`](ScenarioStep::QueryView) before it.
///
/// The rule became load-bearing when scenarios grew arrangements. A lifecycle scenario cancels an
/// invoice in order to *reach* `Cancelled` and then requires that cancelling it again publishes
/// nothing — and `InvoiceCancelled` was legitimately observed earlier in that same scenario. Read
/// per-command the two are different claims; read per-scenario they contradict each other.
///
/// # Nothing here holds a clock, a token or an id
///
/// Design §37 draws that boundary: the runner owns every source of variation and hands it to the
/// target. A suite that carried a deadline would be a suite whose meaning changed with the machine
/// it ran on, and a consistency token does not exist until the run that mints it — which is why
/// [`QueryView`](ScenarioStep::QueryView) says *which view*, and the runner supplies the freshness.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum ScenarioStep {
    /// Force an outcome the input cannot decide (§12).
    ///
    /// For `external` outcomes only: no predicate over a recipient and a template says whether a
    /// provider will accept the mail, so the suite injects the answer instead of inventing an input
    /// that produces it. This is a **test adapter control**, not a runtime capability the
    /// specification claims — the system stays as non-deterministic as it really is.
    ///
    /// The command is not a separate field: an [`OutcomeRef`] already names it, and a second copy is
    /// a second thing that can disagree.
    ConfigureExternalOutcome {
        /// The outcome the adapter must produce next.
        force: OutcomeRef,
    },
    /// Invoke a command (§9).
    ExecuteCommand {
        /// Which command.
        command: CommandRef,
        /// As whom, where the specification grants commands to actors.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actor: Option<ActorRef>,
        /// The input, by declared field name.
        ///
        /// A [`ScenarioValue`] per field: either a [`Node`] tree — the workspace's one
        /// format-neutral dynamic value, and the same shape [`flatten`](crate::flatten) already
        /// projects into the facts a guard reads — or a reference to an instance an earlier step
        /// bound. A value type of this module's own would be a second place for `Map<String, Money>`
        /// to mean something slightly different; a *reference* is not a value at all, which is why
        /// it is a second case rather than a special [`Node`].
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        input: BTreeMap<String, ScenarioValue>,
    },
    /// Require that the command took this declared branch (§10).
    ///
    /// A declared refusal is an outcome, and asserting it is how a suite catches an implementation
    /// that surfaces domain semantics as an untyped infrastructure error (§9).
    ExpectOutcome {
        /// The branch.
        outcome: OutcomeRef,
    },
    /// Require the declared error, and what it carries.
    ExpectError {
        /// Which declared error.
        error: ErrorRef,
        /// The payload fields to compare, by name.
        ///
        /// Partial by design: only the named fields are compared, because a specification does not
        /// determine every value an implementation may legitimately put in one.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        fields: BTreeMap<String, Node>,
    },
    /// Require that this event was observed (§13), carrying what it declares it carries.
    ExpectEvent {
        /// Which event.
        event: EventRef,
        /// The payload fields to compare, by name. Partial, as [`ExpectError`](Self::ExpectError) is.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        payload: BTreeMap<String, Node>,
        /// The declared fields that must be present, and what each must hold.
        ///
        /// Separate from `payload` because the two are different kinds of claim, and only one of
        /// them is free: a *value* is assertable only where some construct says where it comes
        /// from, and a *type* is declared outright. [`PayloadShape`] argues it in full.
        #[serde(default, skip_serializing_if = "PayloadShape::is_empty")]
        shape: PayloadShape,
    },
    /// Require that this event was **not** observed.
    ///
    /// The assertion §10 makes first class: `CreateInvoice` refused must not emit `InvoiceCreated`.
    /// Without it a suite passes against an implementation that emits everything and refuses
    /// nothing.
    ExpectNoEvent {
        /// The event that must not appear.
        event: EventRef,
    },
    /// Bind the identity of the instance the last command created, so later steps can name it.
    ///
    /// The eleventh step, and the one §19 could not be written without: a lifecycle scenario is a
    /// *sequence* of commands over one instance, and every step after the first has to say which
    /// instance it acts on. The identity is read out of an event the creating branch emitted,
    /// because that is where the model says it is published —
    /// [`ResolvedInstance::Observed`](ess_compiler::ir::ResolvedInstance) — and because §9's command
    /// result already carries the events a command emitted, so this asks nothing new of a target.
    ///
    /// It is a step rather than a field on [`ExpectEvent`](Self::ExpectEvent) because binding is not
    /// an assertion: a runner that could not bind has failed to *arrange* the scenario, which is a
    /// different verdict from an expectation that did not hold.
    CaptureInstance {
        /// The name later steps refer to it by.
        instance: InstanceName,
        /// Whose instance it is.
        entity: EntityRef,
        /// The event carrying the identity.
        event: EventRef,
        /// The field of that event's payload.
        field: String,
    },
    /// Deliver an event the run has already published to its bindings a second time (§17).
    ///
    /// A **test adapter control**, exactly as [`ConfigureExternalOutcome`](Self::ConfigureExternalOutcome)
    /// is, and it exists because `delivery: at_least_once` is precisely the claim that this happens:
    /// the transport may deliver one occurrence more than once, and the handler has to survive it. A
    /// suite that never delivers twice never tests the only thing that word says, and a suite that
    /// re-ran the *upstream command* instead would be testing something else — two commands produce
    /// two occurrences, which every implementation handles by handling each once.
    ///
    /// It does not say which binding: an event goes to everything that reacts to it, and naming one
    /// would be a delivery the transport does not have.
    ///
    /// # What may not be asserted afterwards
    ///
    /// A count. §17 is explicit — under `at_least_once` a conformant implementation may produce
    /// duplicates, so "exactly one `EmailSent` exists" is a test that fails a correct target. What
    /// is required after a redelivery is what was required before it: the declared consequence is
    /// still observable.
    RedeliverEvent {
        /// The event to deliver again.
        event: EventRef,
    },
    /// Require that a binding invoked its command with these values (§16).
    ///
    /// The only assertion that can catch a **swapped mapping**. `recipient: event.contact` and
    /// `recipient: event.alternate_contact` are the same document shape, the same types and two
    /// different systems, and nothing downstream distinguishes them: the model relates a command's
    /// input to no event payload, so the value handed to the command is observable only at the
    /// invocation. Asserting a downstream event's field instead — because it happens to share the
    /// input's *name* — is the inference this crate refuses everywhere else.
    ///
    /// # This is the one step §16 warns about, and why it is still here
    ///
    /// §16 says command tracing "may be additional evidence, but it should not become a requirement
    /// for every implementation". So no flow scenario contains this step: a binding's flow is proved
    /// through the event the invoked command publishes, and nothing else. It appears only under
    /// [`BindingAspect::Mapping`], which is its own scenario id — so a target that cannot expose an
    /// invocation reports `unsupported` for that one scenario (§28) and still proves everything
    /// else. The alternative was to refuse the mapping check outright, which would leave the one
    /// clause of a binding that a document can get wrong *silently* unchecked.
    ExpectInvocation {
        /// Whose invocation.
        binding: BindingRef,
        /// The command it invokes.
        command: CommandRef,
        /// What each input must have received, by declared field name.
        ///
        /// Partial: only the fields the binding maps are named, because the mapping is the only
        /// claim the specification makes about this input.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        input: BTreeMap<String, ScenarioValue>,
    },
    /// Read a view (§14).
    ///
    /// No freshness field, deliberately. The specification already decided it: `read_your_writes`
    /// means the query must see the command that just returned, `eventual` means the assertion is an
    /// [`EventuallyView`](Self::EventuallyView) instead, and `ResolvedView::assertion_style` holds
    /// that decision so that no projection re-derives it. So the runner queries no older than the
    /// token the last executed command returned — `aep_contract::consistency::QueryConsistency`,
    /// which already ships — and the suite states no consistency of its own. A second pair of
    /// consistency types is exactly what §14 refuses.
    QueryView {
        /// Which view.
        view: ViewRef,
    },
    /// Require something of the view last read.
    ///
    /// It repeats the view rather than referring to "the last query" implicitly, so a step reads on
    /// its own and a runner can refuse an expectation that does not match the query before it —
    /// which is a suite defect worth naming rather than a silent mis-assertion.
    ExpectView {
        /// Which view.
        view: ViewRef,
        /// What must hold of it.
        expectation: ViewExpectation,
    },
    /// Require that this event is observed within the runner's deadline (§15).
    ///
    /// The bounded form of [`ExpectEvent`](Self::ExpectEvent), for an observation that cannot be
    /// required to have happened by the time the command returns. The deadline is the runner's, and
    /// nothing here sleeps (§40).
    EventuallyEvent {
        /// Which event.
        event: EventRef,
        /// The payload fields to compare, by name.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        payload: BTreeMap<String, Node>,
        /// The declared fields the occurrence must carry, as
        /// [`ExpectEvent`](Self::ExpectEvent) requires them.
        #[serde(default, skip_serializing_if = "PayloadShape::is_empty")]
        shape: PayloadShape,
    },
    /// Read a view until it satisfies the expectation, or the deadline expires (§14).
    ///
    /// One step where an immediate assertion is two, and that asymmetry is the semantics: retrying
    /// means re-running the query, so a split pair could not express it. This is the shape an
    /// `eventual` view demands — asserting one with [`ExpectView`](Self::ExpectView) races the
    /// projection, and the repair everyone reaches for is a sleep.
    EventuallyView {
        /// Which view.
        view: ViewRef,
        /// What must eventually hold of it.
        expectation: ViewExpectation,
    },
}

/// What must hold of a view.
///
/// Rows are matched by the fields the suite names, not by a key: a view declares a filter and the
/// fields it projects, and nothing in the model declares query parameters. Inventing them here would
/// be the kind of invention §11 refuses; when the model gains them, this gains a variant.
///
/// # A field may name the instance the scenario made, and that is what makes it an assertion
///
/// The values are [`ScenarioValue`]s, not [`Node`]s, for the same reason
/// [`ExecuteCommand`](ScenarioStep::ExecuteCommand)'s input is: the identity a scenario is about is
/// the target's to assign, and no generator knows it. With literals only, `Contains {}` means "the
/// view holds *a* row" and `Excludes {}` means "the view holds none at all" — correct only because
/// isolation is per scenario (§8), and wrong the moment a target is shared with anything else,
/// which §8 permits a target to be as long as scenarios do not interfere.
///
/// Naming the instance costs a runner exactly what an instance-valued command input already costs
/// it: the reference has to be resolved against what earlier steps bound before the comparison, and
/// a reference nothing bound is a **suite** defect rather than a failed assertion. A runner that
/// understood only literals could execute no scenario in this vocabulary already, so nothing new is
/// required of one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "expect", rename_all = "snake_case")]
pub enum ViewExpectation {
    /// A row with these field values is present.
    Contains {
        /// The fields to match, by name.
        fields: BTreeMap<String, ScenarioValue>,
    },
    /// No row with these field values is present.
    ///
    /// A cancelled invoice that stays in `OutstandingInvoices` is a defect the positive assertion
    /// cannot see.
    Excludes {
        /// The fields to match, by name.
        fields: BTreeMap<String, ScenarioValue>,
    },
    /// Every row the view holds satisfies this predicate, and it holds at least one.
    ///
    /// What §20 needs and equality cannot express. An entity invariant states a *range* —
    /// `total.amount >= 0` — where [`Contains`](Self::Contains) can only say which value a field
    /// has, so there is no set of field matches that means the same thing. The predicate is carried
    /// whole, in the model's own [`Predicate`] type, so it serialises with the suite and a runner in
    /// another language reads the same expression the specification wrote.
    ///
    /// # "At least one" is part of the assertion
    ///
    /// Deliberately, and it is the difference between a check and a formality: *every row of an
    /// empty view satisfies everything*, so a target that publishes nothing would pass an invariant
    /// check that made no such demand. Synthesis only emits this against a view whose filter it has
    /// decided **does** hold the entity the scenario just changed, so the demand is one the
    /// specification already licenses.
    ///
    /// # `Unknown` refuses; it does not retry
    ///
    /// Evaluating a predicate against a row is three-valued (invariant 5), and only `True` passes.
    /// `False` is a failed assertion — the implementation contradicted the specification. `Unknown`
    /// is neither: it means the row could not answer the question, which is a defect in the
    /// specification or in what the view publishes, and no amount of re-reading changes it. So a
    /// runner that meets `Unknown` **stops and reports the predicate and the row**, even inside an
    /// [`EventuallyView`](ScenarioStep::EventuallyView), where the temptation is to keep polling
    /// until the deadline and then report a timeout — which is the same defect wearing a slower,
    /// less legible costume. Synthesis closes the common cause in advance: it emits this only where
    /// every path the predicate reads is a field the view publishes.
    Satisfies {
        /// The condition every row must satisfy.
        predicate: Predicate,
    },
}

// ---- names -----------------------------------------------------------------------------------

/// Declares one semantic reference per line: the name it wraps, and the handle it is minted from.
///
/// Generated rather than written out eleven times, for the reason the compiler generates its handle
/// accessors: the parts are one claim, and hand-written copies drift one at a time. Each reference
/// serialises as the name itself, and **parses** from it — which is what makes a suite readable in a
/// process that has no [`EssIr`].
macro_rules! semantic_refs {
    (
        $(
            $(#[$attribute:meta])*
            $reference:ident($inner:ty) from $handle:ty, $what:literal;
        )*
    ) => {
        $(
            $(#[$attribute])*
            ///
            /// A stable ESS name, never a handle: it resolves against any compilation of the same
            /// specification, where a handle is valid only inside the one that minted it.
            #[derive(
                Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize,
            )]
            #[serde(transparent)]
            pub struct $reference($inner);

            impl $reference {
                #[doc = concat!("Names ", $what, ".")]
                pub fn new(name: $inner) -> Self {
                    Self(name)
                }

                /// The name it carries.
                pub fn name(&self) -> &$inner {
                    &self.0
                }
            }

            impl fmt::Display for $reference {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", self.0)
                }
            }

            impl From<&$handle> for $reference {
                /// Mints a name from a resolved handle: the one-way door.
                ///
                /// A handle goes in and a name comes out, so a generator holding resolved references
                /// can record them in a suite that outlives the IR — and nothing carries the handle
                /// onward, because there is no field here to put one in.
                fn from(handle: &$handle) -> Self {
                    Self(handle.name().clone())
                }
            }

            impl std::str::FromStr for $reference {
                type Err = ParseError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    <$inner>::new(value).map(Self)
                }
            }

            impl<'de> serde::Deserialize<'de> for $reference {
                fn deserialize<D: serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<Self, D::Error> {
                    let raw = String::deserialize(deserializer)?;
                    <$inner>::new(raw).map(Self).map_err(serde::de::Error::custom)
                }
            }
        )*
    };
}

semantic_refs! {
    /// A bounded context.
    DomainRef(QualifiedName) from DomainHandle, "a bounded context";
    /// A declared type.
    ///
    /// Named `DeclaredTypeRef` because `TypeRef` is taken by
    /// [`ess_domain::types::TypeRef`], which is a different thing: a type
    /// *expression* — `Optional<List<Money>>` — where this is the name of one declaration.
    DeclaredTypeRef(QualifiedName) from TypeHandle, "a declared type";
    /// An entity.
    EntityRef(QualifiedName) from EntityHandle, "an entity";
    /// A command.
    CommandRef(QualifiedName) from CommandHandle, "a command";
    /// An event.
    EventRef(QualifiedName) from EventHandle, "an event";
    /// A declared error.
    ErrorRef(QualifiedName) from ErrorHandle, "a declared error";
    /// A view.
    ViewRef(QualifiedName) from ViewHandle, "a view";
    /// An actor.
    ActorRef(QualifiedName) from ActorHandle, "an actor";
    /// A component.
    ComponentRef(ComponentName) from ComponentHandle, "a component";
}

/// A binding.
///
/// Not in the generated family above: a binding has no handle in the IR to be minted from — it is
/// keyed by [`BindingName`] on `EssIr::bindings` — so there is no one-way door to generate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct BindingRef(BindingName);

impl BindingRef {
    /// Names a binding.
    pub fn new(name: BindingName) -> Self {
        Self(name)
    }

    /// The name it carries.
    pub fn name(&self) -> &BindingName {
        &self.0
    }
}

impl fmt::Display for BindingRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for BindingRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        BindingName::new(value).map(Self)
    }
}

impl<'de> serde::Deserialize<'de> for BindingRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        BindingName::new(raw)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// One branch of one command: `billing.invoice.CreateInvoice` / `accepted`.
///
/// The pair, because an outcome name alone means nothing — `rejected` is declared by three commands
/// in the billing example, and a reference that could not tell them apart would make a fault matrix
/// ambiguous exactly where it has to be precise.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct OutcomeRef {
    /// The command that declares it.
    pub command: CommandRef,
    /// Which branch.
    pub outcome: OutcomeName,
}

impl OutcomeRef {
    /// Names one branch of one command.
    pub fn new(command: CommandRef, outcome: OutcomeName) -> Self {
        Self { command, outcome }
    }
}

impl fmt::Display for OutcomeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.command, self.outcome)
    }
}

/// One declared move of one entity's lifecycle: `billing.invoice.Invoice` / `settle`.
///
/// The entity is part of it because a transition is declared *inside* a lifecycle and has no
/// qualified name of its own — the model never spells `billing.invoice.Invoice.State.settle`, and
/// inventing one here would be a name no other tool could resolve.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TransitionRef {
    /// The entity whose lifecycle declares it.
    pub entity: EntityRef,
    /// The move's own name, such as `settle`.
    #[serde(deserialize_with = "deserialize_transition_name")]
    pub transition: String,
}

impl TransitionRef {
    /// Names one move of one entity, refusing a name a lifecycle cannot declare.
    pub fn new(entity: EntityRef, transition: impl AsRef<str>) -> Result<Self, ParseError> {
        Ok(Self {
            entity,
            transition: transition_name(transition.as_ref())?,
        })
    }
}

impl fmt::Display for TransitionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.entity, self.transition)
    }
}

/// Checks that a transition name is a single qualified-name segment.
///
/// The same rule [`Transition::new`](ess_domain::entity::Transition::new) applies when a
/// specification is parsed — written again here only because `ess-domain` keeps its helper private.
/// If that ever changes, delete this and call it: the rule has one owner, and this is a reader of it
/// rather than a second opinion.
fn transition_name(value: &str) -> Result<String, ParseError> {
    let parsed = QualifiedName::new(value)?;
    if parsed.segments().len() != 1 {
        return Err(ParseError::identifier(
            "transition name",
            value,
            "must be a single segment; the entity supplies the rest".to_owned(),
        ));
    }
    Ok(parsed.to_string())
}

/// Serde entry point for [`transition_name`], so a malformed move is refused while the suite is read.
fn deserialize_transition_name<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
    transition_name(&raw).map_err(serde::de::Error::custom)
}

/// Any construct of a specification, by name.
///
/// The element of a scenario's dependency set. One closed vocabulary rather than a string with a
/// convention, so "which scenarios depend on `billing.invoice.Money`?" is a question a later wave's
/// semantic diff can ask by matching values rather than by parsing prose.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EssSemanticRef {
    /// A bounded context.
    Domain {
        /// Which one.
        name: DomainRef,
    },
    /// A declared type — the one a scenario's input or payload mentions, however deeply.
    Type {
        /// Which one.
        name: DeclaredTypeRef,
    },
    /// An entity a scenario creates, moves or reads.
    Entity {
        /// Which one.
        name: EntityRef,
    },
    /// A command a scenario invokes.
    Command {
        /// Which one.
        name: CommandRef,
    },
    /// A branch a scenario requires.
    Outcome {
        /// Which one.
        name: OutcomeRef,
    },
    /// An event a scenario expects, or requires not to happen.
    Event {
        /// Which one.
        name: EventRef,
    },
    /// A declared error a scenario requires.
    Error {
        /// Which one.
        name: ErrorRef,
    },
    /// A view a scenario asserts.
    View {
        /// Which one.
        name: ViewRef,
    },
    /// An actor a scenario acts as.
    Actor {
        /// Which one.
        name: ActorRef,
    },
    /// A move a scenario takes.
    Transition {
        /// Which one.
        name: TransitionRef,
    },
    /// A binding a scenario's flow crosses.
    Binding {
        /// Which one.
        name: BindingRef,
    },
    /// A component a scenario's flow crosses.
    Component {
        /// Which one.
        name: ComponentRef,
    },
}

impl fmt::Display for EssSemanticRef {
    /// `command billing.invoice.CreateInvoice`, as design §23 writes one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain { name } => write!(f, "domain {name}"),
            Self::Type { name } => write!(f, "type {name}"),
            Self::Entity { name } => write!(f, "entity {name}"),
            Self::Command { name } => write!(f, "command {name}"),
            Self::Outcome { name } => write!(f, "outcome {name}"),
            Self::Event { name } => write!(f, "event {name}"),
            Self::Error { name } => write!(f, "error {name}"),
            Self::View { name } => write!(f, "view {name}"),
            Self::Actor { name } => write!(f, "actor {name}"),
            Self::Transition { name } => write!(f, "transition {name}"),
            Self::Binding { name } => write!(f, "binding {name}"),
            Self::Component { name } => write!(f, "component {name}"),
        }
    }
}

/// Declares `From<X> for EssSemanticRef` per reference kind, so collecting a dependency set while
/// generating is `.into()` rather than a match a caller writes.
macro_rules! semantic_ref_from {
    ($($reference:ident => $variant:ident;)*) => {
        $(
            impl From<$reference> for EssSemanticRef {
                fn from(name: $reference) -> Self {
                    Self::$variant { name }
                }
            }
        )*
    };
}

semantic_ref_from! {
    DomainRef => Domain;
    DeclaredTypeRef => Type;
    EntityRef => Entity;
    CommandRef => Command;
    OutcomeRef => Outcome;
    EventRef => Event;
    ErrorRef => Error;
    ViewRef => View;
    ActorRef => Actor;
    TransitionRef => Transition;
    BindingRef => Binding;
    ComponentRef => Component;
}

#[cfg(test)]
mod tests {
    use aep_domain::facts::Number;

    use super::*;

    /// `billing.invoice.CreateInvoice`, parsed.
    fn command(name: &str) -> CommandRef {
        CommandRef::new(QualifiedName::new(name).expect("a valid qualified name"))
    }

    /// One branch of one command.
    fn outcome(command_name: &str, branch: &str) -> OutcomeRef {
        OutcomeRef::new(
            command(command_name),
            OutcomeName::new(branch).expect("a valid outcome name"),
        )
    }

    #[test]
    fn a_declared_leaf_admits_what_its_type_admits_and_absence_only_where_the_type_permits_it() {
        // The claim `PayloadShape` is worth anything for: a field declared `Decimal` and carrying a
        // string is a contradiction of the specification, and a field the declaration marks
        // `Optional` is not one when it is missing. Both directions, because a check that admitted
        // everything would pass exactly as green as one that checked the right thing.
        let amount = LeafShape::required(Holds::Primitive {
            kind: Primitive::Decimal,
        });
        assert!(amount.admits(Some(&Node::Number(Number::new(1.5).expect("finite")))));
        assert!(!amount.admits(Some(&Node::Text("1.5".to_owned()))));
        assert!(
            !amount.admits(None),
            "a field the declaration does not mark optional has to be there"
        );
        assert!(
            !amount.admits(Some(&Node::Null)),
            "and null is absence spelled differently, not a decimal"
        );
        assert!(amount.clone().optional().admits(None));

        // An `Integer` that is not integral is refused rather than rounded — `crate::input`'s rule,
        // called rather than restated, so a payload and a command input cannot disagree about it.
        let count = LeafShape::required(Holds::Primitive {
            kind: Primitive::Integer,
        });
        assert!(!count.admits(Some(&Node::Number(Number::new(1.5).expect("finite")))));

        let channel = LeafShape::required(Holds::Enum {
            variants: vec!["Email".to_owned(), "Post".to_owned()],
        });
        assert!(channel.admits(Some(&Node::Text("Post".to_owned()))));
        assert!(
            !channel.admits(Some(&Node::Text("Fax".to_owned()))),
            "a closed set of names is closed, which is the whole reason to declare one"
        );

        // The three containers say only what container they are: a fact path has no index and no
        // key selector, so nothing deeper is describable and nothing deeper is claimed.
        assert!(LeafShape::required(Holds::List).admits(Some(&Node::Seq(Vec::new()))));
        assert!(!LeafShape::required(Holds::List).admits(Some(&Node::Map(BTreeMap::new()))));
        assert!(LeafShape::required(Holds::Map).admits(Some(&Node::Map(BTreeMap::new()))));
        assert!(LeafShape::required(Holds::Union).admits(Some(&Node::Map(BTreeMap::new()))));
        assert!(!LeafShape::required(Holds::Union).admits(Some(&Node::Text("person".to_owned()))));
    }

    #[test]
    fn a_payload_shape_round_trips_through_the_form_a_suite_is_stored_in() {
        // §21: a suite is read back by a later process, so a shape that only meant something to the
        // build that wrote it would be a cache. `optional: false` is left out of the artifact, which
        // is a default rather than an omission — this is what says the two are the same value.
        let mut shape = PayloadShape::new();
        shape.insert(
            "amount.amount",
            LeafShape::required(Holds::Primitive {
                kind: Primitive::Decimal,
            }),
        );
        shape.insert(
            "note",
            LeafShape::required(Holds::Primitive {
                kind: Primitive::String,
            })
            .optional(),
        );

        let written = serde_json::to_string(&shape).expect("a shape serialises");
        assert!(
            !written.contains(
                r#""amount.amount":{"holds":"primitive","kind":"decimal","optional":false}"#
            ),
            "the default is left out rather than repeated on every leaf: {written}"
        );
        assert!(
            written.contains(r#""optional":true"#),
            "and the one that is not the default is written down: {written}"
        );
        assert_eq!(
            serde_json::from_str::<PayloadShape>(&written).expect("and reads back"),
            shape
        );
    }

    #[test]
    fn a_scenario_id_names_the_construct_it_exercises_rather_than_its_position() {
        let id = ScenarioId::Outcome {
            outcome: outcome("billing.invoice.CreateInvoice", "rejected"),
        };

        assert_eq!(
            id.to_string(),
            "billing.invoice.CreateInvoice/outcome/rejected",
            "design §23 writes this id exactly this way, and a fault matrix quotes it"
        );
        assert!(
            id.to_string().starts_with("billing.invoice.CreateInvoice"),
            "subject first, so `grep CreateInvoice` finds every scenario about the command"
        );
    }

    #[test]
    fn every_scenario_id_reads_back_from_the_form_a_report_prints() {
        // One of each variant, and one per binding aspect, because a rendering that cannot be
        // parsed back is a rendering that makes a fault matrix's references unusable — and it is
        // the variant nobody checks that breaks.
        let entity = EntityRef::new(QualifiedName::new("billing.invoice.Invoice").expect("valid"));
        let mut ids = vec![
            ScenarioId::Outcome {
                outcome: outcome("billing.invoice.CreateInvoice", "accepted"),
            },
            ScenarioId::Transition {
                transition: TransitionRef::new(entity.clone(), "settle").expect("valid"),
                by: outcome("billing.invoice.PayInvoice", "settled"),
            },
            ScenarioId::Refusal {
                entity: entity.clone(),
                state: StateName::new("Paid").expect("valid"),
                command: command("billing.invoice.CancelInvoice"),
            },
            ScenarioId::Invariant {
                entity,
                after: outcome("billing.invoice.CreateInvoice", "accepted"),
            },
        ];
        ids.extend(BindingAspect::ALL.map(|(aspect, _)| ScenarioId::Binding {
            binding: BindingRef::new(BindingName::new("notify-on-invoice-created").expect("valid")),
            aspect,
        }));
        assert_eq!(
            ids.len(),
            8,
            "five id shapes, and every aspect a binding scenario can be filed under"
        );

        for id in ids {
            let rendered = id.to_string();
            assert_eq!(
                ScenarioId::parse(&rendered).expect("the rendered form parses"),
                id,
                "`{rendered}` did not survive its own rendering"
            );
        }
    }

    #[test]
    fn every_binding_aspect_is_in_the_list_that_is_walked_to_produce_them() {
        // `ALL` is what synthesis iterates, so an aspect missing from it is a scenario nothing
        // produces and nothing refuses — the silent omission §36 exists to rule out, arriving as an
        // array that still compiles. The check is that every spelling `parse` accepts is in it, and
        // that each entry agrees with how the aspect renders.
        for (aspect, written) in BindingAspect::ALL {
            assert_eq!(aspect.to_string(), written, "`{written}` renders as itself");
            assert_eq!(
                BindingAspect::parse(written),
                Ok(aspect),
                "`{written}` reads back as the aspect it names"
            );
        }
        for written in ["flow", "mapping", "delivery", "on-failure"] {
            let aspect = BindingAspect::parse(written).expect("a word `parse` accepts");
            assert!(
                BindingAspect::ALL.contains(&(aspect, written)),
                "`{written}` parses and is not in `ALL`, so nothing would ever produce it"
            );
        }
        assert!(BindingAspect::parse("failure").is_err());
    }

    #[test]
    fn an_invariant_scenario_is_keyed_by_the_entity_and_the_branch_and_never_by_a_position() {
        // The property the last slice established, applied to the shape this one added: an id is a
        // semantic name, so adding a second invariant to the entity must not re-key the scenario
        // that carries the first. The fixture reaches that state by construction — the id has
        // nowhere to *put* an invariant — and the assertion is that the rendered form names only
        // the entity and the branch.
        let entity = EntityRef::new(QualifiedName::new("billing.invoice.Invoice").expect("valid"));
        let id = ScenarioId::Invariant {
            entity,
            after: outcome("billing.invoice.CreateInvoice", "accepted"),
        };

        assert_eq!(
            id.to_string(),
            "billing.invoice.Invoice/invariant/after/billing.invoice.CreateInvoice/accepted"
        );
        assert!(
            !id.to_string().contains("total.amount"),
            "an id that quoted the statement would re-key when an author rewrote it: {id}"
        );
        assert!(
            id.to_string().starts_with("billing.invoice.Invoice"),
            "the entity is the subject: `grep Invoice` finds what must hold of one"
        );
    }

    #[test]
    fn a_scenario_id_that_names_no_construct_is_refused() {
        for malformed in [
            "billing.invoice.CreateInvoice",
            "billing.invoice.CreateInvoice/outcome",
            "billing.invoice.CreateInvoice/branch/accepted",
            "billing.invoice.CreateInvoice/outcome/Accepted",
            "billing.invoice.Invoice/state/paid/refuses/billing.invoice.CancelInvoice",
            "billing.invoice.Invoice/invariant/0",
            "billing.invoice.Invoice/invariant/before/billing.invoice.CreateInvoice/accepted",
            "notify-on-invoice-created/binding/failure",
            "3",
        ] {
            let error = ScenarioId::parse(malformed).expect_err("refused");
            assert!(
                error.to_string().contains(malformed),
                "a refusal quotes what it refused: {error}"
            );
        }
    }

    #[test]
    fn two_scenarios_about_the_same_thing_in_the_same_way_are_one_id() {
        let first = ScenarioId::Outcome {
            outcome: outcome("billing.invoice.CreateInvoice", "accepted"),
        };
        let again = ScenarioId::parse("billing.invoice.CreateInvoice/outcome/accepted")
            .expect("the same id, arrived at from text");

        assert_eq!(first, again);
        assert_eq!(
            first.to_string(),
            again.to_string(),
            "an id built from the model and an id read from a stored report are the same key, or \
             no report can be compared with a run"
        );
    }

    #[test]
    fn the_ids_of_a_suite_sort_the_way_a_reader_sorts_the_file() {
        // Two variants and two subjects, so the assertion needs the ordering to be by the rendered
        // name: a derived `Ord` would group every `Outcome` before every `Transition` regardless of
        // subject, which is an order the file does not show and a re-sorting tool would not
        // reproduce.
        let entity = EntityRef::new(QualifiedName::new("billing.invoice.Invoice").expect("valid"));
        let transition = ScenarioId::Transition {
            transition: TransitionRef::new(entity, "settle").expect("valid"),
            by: outcome("billing.invoice.PayInvoice", "settled"),
        };
        let create = ScenarioId::Outcome {
            outcome: outcome("billing.invoice.CreateInvoice", "accepted"),
        };
        let pay = ScenarioId::Outcome {
            outcome: outcome("billing.invoice.PayInvoice", "settled"),
        };
        assert!(
            create.to_string() < transition.to_string() && transition.to_string() < pay.to_string(),
            "the fixture holds ids whose name order and variant order disagree"
        );

        let mut sorted = vec![pay.clone(), create.clone(), transition.clone()];
        sorted.sort();

        assert_eq!(
            sorted,
            vec![create, transition, pay],
            "the keys must come out in plain lexicographic order"
        );
    }

    #[test]
    fn a_suite_refuses_a_second_scenario_under_one_id() {
        let id = ScenarioId::Outcome {
            outcome: outcome("billing.invoice.CreateInvoice", "accepted"),
        };
        let scenario = ConformanceScenario::new(
            ScenarioPurpose::new("a positive amount is accepted").expect("valid"),
            [ScenarioStep::ExpectOutcome {
                outcome: outcome("billing.invoice.CreateInvoice", "accepted"),
            }],
            [],
        );
        let mut suite = ConformanceSuite::new(provenance());
        suite
            .insert(id.clone(), scenario.clone())
            .expect("the first one is accepted");
        assert_eq!(
            suite.len(),
            1,
            "the fixture holds the id before the rule is asked about it"
        );

        let refused = suite
            .insert(id.clone(), scenario)
            .expect_err("a second scenario under one id is refused");

        assert_eq!(refused, id, "the refusal names the id it collided on");
        assert_eq!(
            suite.len(),
            1,
            "and the first scenario is still the one in the suite"
        );
    }

    #[test]
    fn a_suite_format_from_a_later_build_is_refused_rather_than_guessed() {
        assert_eq!(SuiteFormat::CURRENT.to_string(), "ess-conformance/1");
        assert!(SuiteFormat::CURRENT.is_supported());

        let later = SuiteFormat::parse("ess-conformance/2").expect("well formed");
        assert!(
            !later.is_supported(),
            "a later format may mean something different by the same words"
        );

        for malformed in ["ess/1", "ess-conformance/01", "ess-conformance/0", "1"] {
            SuiteFormat::parse(malformed).expect_err(malformed);
        }
    }

    #[test]
    fn a_purpose_is_one_line_and_says_something() {
        assert_eq!(
            ScenarioPurpose::new("  a positive amount is accepted  ")
                .expect("valid")
                .as_str(),
            "a positive amount is accepted"
        );
        ScenarioPurpose::new("").expect_err("says nothing");
        ScenarioPurpose::new("two\nlines").expect_err("is not one line");
        ScenarioPurpose::new("x".repeat(ScenarioPurpose::MAX_LENGTH + 1)).expect_err("is a page");
    }

    #[test]
    fn a_transition_ref_refuses_a_name_no_lifecycle_can_declare() {
        let entity = EntityRef::new(QualifiedName::new("billing.invoice.Invoice").expect("valid"));

        TransitionRef::new(entity.clone(), "settle").expect("a single segment");
        let error = TransitionRef::new(entity, "Invoice.settle").expect_err("refused");

        assert!(
            error.to_string().contains("single segment"),
            "the refusal says which rule was broken: {error}"
        );
    }

    #[test]
    fn a_semantic_reference_renders_the_way_the_design_writes_one() {
        let reference: EssSemanticRef = command("billing.invoice.CreateInvoice").into();

        assert_eq!(
            reference.to_string(),
            "command billing.invoice.CreateInvoice"
        );
    }

    /// Provenance for a suite no `EssIr` was compiled for.
    fn provenance() -> SuiteProvenance {
        SuiteProvenance {
            suite_version: SuiteFormat::CURRENT,
            system: "billing".to_owned(),
            specification_version: "v3".to_owned(),
            spec_digest: SpecDigest::new(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .expect("a digest"),
            compiler_version: "0.1.0".to_owned(),
            generator_version: "0.1.0".to_owned(),
            synthesizer_version: "0.1.0".to_owned(),
        }
    }
}
