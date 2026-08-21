//! What a delta invalidates, and why — the closure from a change to everything standing on it.
//!
//! Design §23, §24, §25 and §26, and the first consumer is the one wave 4 already built: a
//! [`ConformanceSuite`] records, per scenario, the **set of constructs it depends on**, so a delta
//! plus a committed suite answers *which scenarios does this change put back to owed*.
//!
//! ```text
//!   EssIr before ──┐                     ┌── SemanticDependencyGraph (union of both)
//!                  ├── diff ── EssDelta ─┤
//!   EssIr after ───┘                     └── closure per change ── ImpactPath
//!                                                     │
//!                        ConformanceSuite.source ─────┴── Invalidation
//! ```
//!
//! # Invalidation fails closed, and the type says so rather than a rule someone remembers
//!
//! Gate G19 binds conformance evidence to the specification digest it was produced against: when the
//! model moves, every requirement it satisfied goes back to owed. Correct, and blunt. This module
//! **refines** G19 and cannot replace it — it may narrow what has to be re-established, and it may
//! never say a scenario survived.
//!
//! The design goes the other way. Its §33 verdict vocabulary includes *still valid* and its §26
//! counts invalidated records as a subset, which would put a delta engine in the position of
//! deciding which prior results stand. That is the choice this wave reversed, on the asymmetry of
//! what a missed dependency edge costs: failing closed costs a re-run that was not needed, and
//! failing open costs a task closing on evidence produced against a specification that has since
//! moved. Those are not comparable errors.
//!
//! Five mechanisms make the polarity structural rather than remembered. Each one is a thing you
//! cannot write, not a thing you must not:
//!
//! | # | mechanism | what it forecloses |
//! |---|---|---|
//! | 1 | [`Invalidation`] has two variants, [`Whole`](Invalidation::Whole) and [`Narrowed`](Invalidation::Narrowed), and **no third**. There is no `StillValid`, no `Unaffected`, no `survived` field and no method that returns one | a caller cannot read a survival claim out of this module, because none is representable |
//! | 2 | the only combinator is [`Invalidation::joined`], a join on a lattice whose top is `Whole`. There is no meet, no difference, and nothing that removes a scenario from a set | processing one more change can only ever widen what is owed |
//! | 3 | [`SemanticChange::subject`] returns `Option`, and the `None` arm — a change to the specification itself, which names no construct — is [`Whole`](Invalidation::Whole) | a change the graph cannot seed a closure from cannot fall through as *nothing* |
//! | 4 | a suite whose dependency set names a construct **neither** revision's graph has a node for is [`Whole`](Invalidation::Whole) | an incomplete graph walk cannot silently make a scenario unreachable, which is the one way a narrowing could be wrong and look right |
//! | 5 | [`impact()`] runs the comparison itself, from the same two [`EssIr`]s it builds the graph from, and refuses a suite whose digest is not the `before` revision's | a delta and a graph cannot be about different pairs, and a suite cannot be narrowed against a revision it was not produced from |
//! | 6 | [`impact()`] compares the canonical form of the construct families the delta does **not** read — entities, commands, views, bindings, conversions, workloads — and any difference there is [`Whole`](Invalidation::Whole) | a change to an uncompared construct (an outcome's guard, a payload mapping, a lifecycle) cannot arrive as an empty delta narrowing to nothing, which would be a survival claim about a model that moved |
//!
//! `tests/impact.rs` breaks each of 3, 4, 5 and 6 and watches it fail.
//!
//! # Wave 7: the same answer about generated artifacts
//!
//! Since W7.1 every generated artifact carries the digest of the model slice it derives from
//! ([`ess_gen::provenance::ModelSlice`]), and this module answers the artifact-granularity
//! question beside the scenario one: **which artifacts are owed regeneration**. The polarity is
//! identical and enforced the same way — [`ArtifactAnswer`] has `Whole` and `Narrowed` and no
//! third variant, an artifact absent from the answer was *not reached*, and everything the
//! analysis cannot follow is owed, stated as such: a change with no construct to seed from owes
//! every artifact (mechanism 3), a slice resting on an ungraphed construct owes every artifact
//! (mechanism 4), a move in an uncompared family owes every artifact (mechanism 6), and a
//! committed artifact whose provenance cannot be read or whose contract digest its slice does not
//! compute is owed outright ([`ArtifactObligation`]). `tests/artifacts.rs` proves the narrowing on
//! the fixture pair — a strict subset, with the path explaining each member — and breaks every
//! fail-closed arm from outside.
//!
//! # Why there is no `RawEssImpact`
//!
//! Invariant 2 applies to documents that are **read back**. A delta is one — it is committed, quoted
//! and re-read by a later process, so `ess-diff/1` has a raw pair that re-derives every id and
//! relation. An impact report is not: it is a function of a delta and a suite, both of which are
//! themselves read back through checked doors, so a reader who wants to trust one re-runs it in less
//! time than it takes to validate it. [`ConformanceReport`](ess_conformance::ConformanceReport)
//! made the same call for the same reason.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use aep_domain::evidence::SpecDigest;
use ess_compiler::ir::EssIr;
use ess_conformance::scenario::{
    ConformanceScenario, ConformanceSuite, EssSemanticRef, ScenarioId, SuiteProvenance,
};
use ess_domain::name::QualifiedName;

use ess_gen::provenance::ModelSlice;

use crate::change::{ActorChange, ChangeId, SemanticChange};
use crate::delta::EssDelta;
use crate::diff::{diff, DiffRefusal};
use crate::graph::{DependencyEdge, ImpactClass, Reach, SemanticDependencyGraph};

/// The shape this report is written in.
///
/// A label rather than a parsed type, and that is the difference between this and
/// [`DeltaFormat`](crate::DeltaFormat): a format version earns a parser when something reads the
/// document back and has to refuse a shape it does not understand. Nothing reads an impact report
/// back — see the module documentation — so a parser here would be a refusal that cannot fire, and
/// the word is carried anyway because a consumer keying on it costs nothing.
///
/// `/2` since wave 7: the document gained an `artifacts` section, and `suite` plus
/// `invalidation` became optional — a report can now be about the generated tree alone. The label
/// still has no parser, for the reason above, and it is bumped anyway because a label that stays
/// at `/1` across a shape change is a label that lies to the one consumer who does key on it.
pub const IMPACT_FORMAT: &str = "ess-impact/2";

/// Why a delta and a suite cannot be compared at all.
///
/// Every variant is a state a person can reach by pointing the command at the wrong file, and each
/// one is refused rather than answered — because the plausible answer in each case is the dangerous
/// one. Narrowing against the wrong suite produces a short list of scenarios that looks exactly like
/// a correct short list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "refused", rename_all = "kebab-case")]
pub enum ImpactRefusal {
    /// The two specifications are not two revisions of one system.
    ///
    /// Carried through from [`diff`] rather than restated, so the one rule has one spelling.
    Pair {
        /// What the comparison refused, and why.
        #[serde(flatten)]
        refusal: DiffRefusal,
    },
    /// The suite checks a different system from the one being compared.
    SuiteFromAnotherSystem {
        /// The system the suite names.
        suite: String,
        /// The system the two revisions are of.
        revisions: QualifiedName,
    },
    /// The suite was not produced from the `before` revision.
    ///
    /// The refusal that keeps a narrowing honest. A suite records the digest of the model it was
    /// synthesised from, and prior results were produced against *that* model; narrowing against any
    /// other one answers a question about a specification nobody has.
    SuiteFromAnotherRevision {
        /// The model the suite was produced from.
        suite: SpecDigest,
        /// The model being compared from.
        before: SpecDigest,
        /// The model being compared to — named because "you gave me the new suite" is the mistake
        /// this refusal catches most often, and a reader should not have to work that out.
        after: SpecDigest,
    },
    /// The suite's contract digest is not what the `--from` model's slice rule computes.
    ///
    /// The suite's `spec_digest` matched, so the model is the right one — which leaves exactly one
    /// way to reach this state: the digest was written by something other than the synthesiser, a
    /// hand edit or a corruption. A document whose claim of derivation is false is refused rather
    /// than narrowed, for the reason a suite from another revision is: the short list it would
    /// produce looks exactly like a correct short list.
    SuiteContractMismatch {
        /// What the suite claims its slice digest is.
        suite: SpecDigest,
        /// What the `--from` model's whole-model slice computes.
        expected: String,
    },
}

impl fmt::Display for ImpactRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pair { refusal } => write!(f, "{refusal}"),
            Self::SuiteFromAnotherSystem { suite, revisions } => write!(
                f,
                "the suite checks `{suite}` and these are two revisions of `{revisions}`: a suite \
                 says what one system's implementation owes, and it cannot say it about another"
            ),
            Self::SuiteContractMismatch { suite, expected } => write!(
                f,
                "this suite claims contract digest {suite}, and the `--from` model's slice rule \
                 computes {expected}: the claim of derivation is false — a hand edit or a \
                 corruption — and narrowing on a false claim would produce a short list that looks \
                 exactly like a correct one"
            ),
            Self::SuiteFromAnotherRevision {
                suite,
                before,
                after,
            } => {
                if suite == after {
                    write!(
                        f,
                        "this suite was produced from the `--to` revision ({after}), not the \
                         `--from` one ({before}): what a change invalidates is a question about the \
                         results you already have, so the suite has to be the one they were \
                         produced against"
                    )
                } else {
                    write!(
                        f,
                        "this suite was produced from model {suite}, and the `--from` revision is \
                         {before}: narrowing against a suite from a third revision would answer a \
                         question about a specification nobody has"
                    )
                }
            }
        }
    }
}

impl std::error::Error for ImpactRefusal {}

/// One reason one construct is impacted: the change, what it reached, and the edges between.
///
/// Design §24. An impact nobody can explain is an impact nobody will act on, so the path is part of
/// the value rather than something a renderer reconstructs. `edges` runs **from the change outward
/// to the target**, so reading it top to bottom is reading the argument in the order it is made.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImpactPath {
    /// Which change reached it.
    pub change: ChangeId,
    /// What was reached.
    pub target: EssSemanticRef,
    /// The edges crossed, from the changed construct to the target. Empty when they are the same.
    edges: Vec<DependencyEdge>,
}

impl ImpactPath {
    /// How far the target is from the change.
    ///
    /// Derived from the path rather than stored beside it, for the reason
    /// [`SemanticChange::relation`] is: two fields describing one fact are two fields that can
    /// disagree. The document carries the answer all the same, written from this method.
    #[must_use]
    pub fn class(&self) -> ImpactClass {
        ImpactClass::of(self.edges.len())
    }

    /// The edges crossed, from the changed construct outward.
    pub fn edges(&self) -> &[DependencyEdge] {
        &self.edges
    }

    /// The path as one line per hop, each already indented, ending in a newline.
    ///
    /// Empty for a directly changed construct: there is nothing between it and the change, and a
    /// line saying so would be a line that reads as a hop.
    pub fn explain(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        for edge in &self.edges {
            let _ = writeln!(out, "      -> {edge}");
        }
        out
    }
}

/// A path as the document writes it: the derived class beside the edges that derive it.
#[derive(serde::Serialize)]
struct WrittenPath<'a> {
    /// Which change reached the target.
    change: &'a ChangeId,
    /// What was reached.
    target: &'a EssSemanticRef,
    /// How far away it is.
    class: ImpactClass,
    /// The edges crossed, from the change outward.
    edges: &'a [DependencyEdge],
}

/// Writes each path with the class its own length derives.
fn serialize_paths<S: serde::Serializer>(
    paths: &[ImpactPath],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq as _;

    let mut sequence = serializer.serialize_seq(Some(paths.len()))?;
    for path in paths {
        sequence.serialize_element(&WrittenPath {
            change: &path.change,
            target: &path.target,
            class: path.class(),
            edges: &path.edges,
        })?;
    }
    sequence.end()
}

/// Why one scenario is owed again.
///
/// One reason per change that reached it — the shortest path to the nearest construct in the
/// scenario's own dependency set. Not every reason: a scenario that rests on five constructs one
/// change reached is explained by the closest of the five, and printing the other four is the
/// blast-radius report design §24 was written against, wearing a longer coat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScenarioImpact {
    /// Why, in canonical order: by change, then by what it reached.
    #[serde(serialize_with = "serialize_paths")]
    reasons: Vec<ImpactPath>,
}

impl ScenarioImpact {
    /// Why this scenario is owed again, one reason per change that reached it.
    pub fn reasons(&self) -> &[ImpactPath] {
        &self.reasons
    }

    /// Folds another set of reasons in, keeping canonical order.
    ///
    /// Extend and sort, never replace: the reasons for one scenario accumulate across changes, and
    /// a merge that dropped one would be a scenario explained by fewer changes than moved it.
    fn absorb(&mut self, other: Self) {
        self.reasons.extend(other.reasons);
        self.reasons.sort();
        self.reasons.dedup();
    }
}

/// Why a whole answer is owed — every scenario of the suite, or every generated artifact.
///
/// Each variant is the fail-closed escape hatch: a change the closure cannot follow, a dependency
/// set resting on something the graph cannot see, a family the delta does not compare. None is an
/// error — the analysis worked and the honest answer is *all of it*, which is exactly what G19
/// would have said. One vocabulary for both questions on purpose: what makes a suite un-narrowable
/// makes the artifact tree un-narrowable by the same argument, and two enums would let the two
/// answers drift.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum WholeAnswer {
    /// A change is about the specification itself, and no construct in a suite names one.
    ///
    /// [`SemanticChange::subject`] is `None` for a
    /// [`System`](SemanticChange::System) change, because
    /// [`EssSemanticRef`] has no variant for the system as a whole — it is not a construct declared
    /// *inside* the specification. There is therefore no node to seed a closure at, and the
    /// alternative to owing everything is owing nothing.
    SystemChanged {
        /// The change that did it.
        change: ChangeId,
    },
    /// A dependency set rests on a construct neither revision's graph has a node for.
    ///
    /// A scenario — or an artifact's slice — naming something the walk does not build a node for
    /// can never be reached by any closure, so narrowing would quietly leave it out of every
    /// answer. That is the one shape in which a narrowing is wrong and looks right, so it is not
    /// narrowed at all.
    UngraphedDependency {
        /// What the suite named.
        construct: EssSemanticRef,
    },
    /// The model moved in a family this comparison does not read.
    ///
    /// The delta compares six families; entities, commands, views, bindings, conversions and the
    /// topology are deliberately not among them (wave 5's boundary, W7.2's work). For those, this
    /// engine checks canonical **equality** only: equal means nothing there moved and the
    /// narrowing stands, different means *something* moved that no change entry can name — an
    /// outcome's guard, a payload mapping, a lifecycle — and a closure cannot be seeded at a
    /// construct the delta does not know changed. So it is not narrowed at all. When W7.2 teaches
    /// the delta a family, changes there start arriving as entries and stop landing here — the
    /// arm shrinks by construction rather than by being remembered.
    UncomparedFamilyChanged,
}

impl fmt::Display for WholeAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemChanged { change } => write!(
                f,
                "`{change}` is about the specification itself, which no dependency set names as a \
                 construct — so nothing can be narrowed away from it"
            ),
            Self::UngraphedDependency { construct } => write!(
                f,
                "a dependency set names {construct}, which neither revision's dependency graph has \
                 a node for: no closure could ever reach it, and leaving it out of every answer is \
                 the one way a narrowing is wrong and looks right"
            ),
            Self::UncomparedFamilyChanged => f.write_str(
                "the model moved in a family this delta does not compare — an entity, a command, \
                 a view, a binding, a conversion or the topology — so no change entry can name \
                 what moved, and no closure can be seeded at it",
            ),
        }
    }
}

/// What a delta puts back to owed.
///
/// Two variants and no third. There is deliberately no way to say a scenario survived — see the
/// module documentation — so the strongest thing this type can express about a scenario that is not
/// listed is that *this analysis did not reach it*, which leaves gate G19 exactly where it was.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "invalidates", rename_all = "kebab-case")]
pub enum Invalidation {
    /// Every scenario the suite holds.
    Whole {
        /// Why narrowing was not available.
        because: WholeAnswer,
    },
    /// These scenarios, each with the reasons that reached it.
    Narrowed {
        /// By scenario id, in the suite's own order.
        scenarios: BTreeMap<ScenarioId, ScenarioImpact>,
    },
}

impl Invalidation {
    /// Nothing owed yet — the identity of [`Self::joined`].
    ///
    /// Crate-private on purpose. A caller who could start from "nothing is owed" and then forget to
    /// join anything into it would have manufactured a survival claim out of an empty accumulator,
    /// which is precisely the fail-open answer this module has no vocabulary for.
    pub(crate) fn nothing() -> Self {
        Self::Narrowed {
            scenarios: BTreeMap::new(),
        }
    }

    /// Both, joined: the least answer that covers each.
    ///
    /// A join on a lattice whose top is [`Whole`](Self::Whole), so `Whole` absorbs everything and
    /// two narrowings union. **There is no meet and no difference**, here or anywhere in this crate:
    /// adding a change to a delta can widen what is owed and can never shrink it, which is what
    /// makes "a closure may narrow, and may never mark something still valid" a property of the
    /// algebra rather than a rule in a comment.
    #[must_use]
    pub fn joined(self, other: Self) -> Self {
        match (self, other) {
            (whole @ Self::Whole { .. }, _) | (_, whole @ Self::Whole { .. }) => whole,
            (
                Self::Narrowed {
                    scenarios: mut left,
                },
                Self::Narrowed {
                    scenarios: right, ..
                },
            ) => {
                for (id, impact) in right {
                    left.entry(id)
                        .and_modify(|existing| existing.absorb(impact.clone()))
                        .or_insert(impact);
                }
                Self::Narrowed { scenarios: left }
            }
        }
    }

    /// Every scenario of `suite` that is owed again.
    ///
    /// The only question this type answers. There is no `still_valid`, and asking for the complement
    /// of this set against the suite is a caller's decision to overrule G19 rather than something
    /// this module hands them.
    pub fn owed<'a>(&'a self, suite: &'a ConformanceSuite) -> BTreeSet<&'a ScenarioId> {
        match self {
            Self::Whole { .. } => suite.scenarios.keys().collect(),
            Self::Narrowed { scenarios } => scenarios.keys().collect(),
        }
    }

    /// Why one scenario is owed again, where the answer is a narrowing.
    ///
    /// `None` for [`Whole`](Self::Whole), and that is the honest answer rather than a missing one:
    /// the whole suite is owed because narrowing was not available, so there is no per-scenario path
    /// to give.
    pub fn reasons(&self, scenario: &ScenarioId) -> Option<&ScenarioImpact> {
        match self {
            Self::Whole { .. } => None,
            Self::Narrowed { scenarios } => scenarios.get(scenario),
        }
    }

    /// `true` when the whole suite is owed.
    pub fn is_whole(&self) -> bool {
        matches!(self, Self::Whole { .. })
    }
}

/// Names one generated artifact in an impact answer.
///
/// Three kinds because the repository commits three kinds of derived output, at the granularity
/// each is regenerated at: a projection is one file, a suite is one document, and a synthesised
/// workspace is one tree `protocol ess synthesize` rewrites whole — so listing its files one by
/// one would name a hundred artifacts where there is one decision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(tag = "artifact", rename_all = "kebab-case")]
pub enum ArtifactId {
    /// A projection under the generated tree, by its path relative to that tree's root.
    Projection {
        /// Where it is, `/`-separated.
        path: String,
    },
    /// The conformance suite the specification obliges.
    Suite,
    /// A synthesised workspace.
    Workspace {
        /// Its root, relative to the generated tree — `rust/billing` — or `rust` when no
        /// committed tree was read and the workspaces are being answered for as one obligation.
        path: String,
    },
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection { path } => write!(f, "projection {path}"),
            Self::Suite => f.write_str("conformance suite"),
            Self::Workspace { path } => write!(f, "workspace {path}"),
        }
    }
}

/// Why one artifact is owed regeneration.
///
/// Every variant is an obligation and none is a verdict of health: there is no `Current`, no
/// `Verified` and no way to say an artifact stands, for the same reason [`Invalidation`] has no
/// `StillValid` — an artifact absent from the answer was not reached by this analysis, which is
/// not a claim about it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "owed", rename_all = "kebab-case")]
pub enum ArtifactObligation {
    /// The delta reached the artifact's slice; the paths explain it, one per change.
    SliceMoved {
        /// Why, in canonical order: by change, then by the slice member it reached.
        #[serde(serialize_with = "serialize_paths")]
        reasons: Vec<ImpactPath>,
    },
    /// The committed artifact carries no provenance this analysis can read.
    ///
    /// Including every artifact written before wave 7, which carries a model digest and no
    /// contract digest: one digest is not provenance, and an unreadable claim is treated exactly
    /// like a false one.
    ProvenanceUnreadable,
    /// The committed artifact's contract digest is not what its slice computes against `--from`.
    ///
    /// A false claim about derivation — a hand edit, a stale regeneration, a corruption. What the
    /// artifact would owe if its claim were true is unknowable, so it owes regeneration outright.
    ContractMismatch {
        /// What the committed artifact claims.
        committed: String,
        /// What its slice computes against the `--from` model.
        expected: String,
    },
    /// A committed file this analysis cannot account for: the `--from` model derives nothing at
    /// its path.
    Unfollowed,
    /// The `--from` model derives it and the committed tree does not hold it.
    Missing,
}

/// Which generated artifacts a delta puts back to owed.
///
/// The same two-variant shape as [`Invalidation`], for the same reason: there is no third variant,
/// no complement operation and no way to read a survival claim out of it. `Whole` answers reuse
/// [`WholeAnswer`] — what makes a suite un-narrowable makes the artifact tree un-narrowable by the
/// same argument.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "answer", rename_all = "kebab-case")]
pub enum ArtifactAnswer {
    /// Every artifact the model derives.
    Whole {
        /// Why narrowing was not available.
        because: WholeAnswer,
    },
    /// These artifacts, each with the obligation that reached it.
    Narrowed {
        /// By artifact, in canonical order.
        #[serde(serialize_with = "serialize_owed")]
        owed: BTreeMap<ArtifactId, ArtifactObligation>,
    },
}

impl ArtifactAnswer {
    /// How many artifacts are owed, out of `total` the model derives.
    fn owed_count(&self, total: usize) -> usize {
        match self {
            Self::Whole { .. } => total,
            Self::Narrowed { owed } => owed.len(),
        }
    }

    /// The obligation one artifact carries, where the answer is a narrowing.
    ///
    /// `None` for [`Whole`](Self::Whole): the whole tree is owed because narrowing was not
    /// available, so there is no per-artifact reason to give.
    pub fn obligation(&self, artifact: &ArtifactId) -> Option<&ArtifactObligation> {
        match self {
            Self::Whole { .. } => None,
            Self::Narrowed { owed } => owed.get(artifact),
        }
    }

    /// `true` when every artifact is owed.
    pub fn is_whole(&self) -> bool {
        matches!(self, Self::Whole { .. })
    }

    /// The owed artifacts, where the answer is a narrowing.
    pub fn owed(&self) -> Option<&BTreeMap<ArtifactId, ArtifactObligation>> {
        match self {
            Self::Whole { .. } => None,
            Self::Narrowed { owed } => Some(owed),
        }
    }
}

/// One owed artifact as the document writes it: the id's fields and the obligation's, one object.
#[derive(serde::Serialize)]
struct WrittenArtifact<'a> {
    /// Which artifact.
    #[serde(flatten)]
    id: &'a ArtifactId,
    /// What it owes.
    #[serde(flatten)]
    obligation: &'a ArtifactObligation,
}

/// Writes the owed map as a sequence, because an [`ArtifactId`] is a value and not a string, and a
/// JSON map key must be a string.
fn serialize_owed<S: serde::Serializer>(
    owed: &BTreeMap<ArtifactId, ArtifactObligation>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq as _;

    let mut sequence = serializer.serialize_seq(Some(owed.len()))?;
    for (id, obligation) in owed {
        sequence.serialize_element(&WrittenArtifact { id, obligation })?;
    }
    sequence.end()
}

/// The committed generated tree, as whoever holds a filesystem read it.
///
/// A value rather than a path, so this crate stays a pure function of its inputs: the CLI walks
/// the directory and hands the bytes in, and a test hands in a map — the same discipline that
/// keeps the comparison itself inside [`impact()`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedTree {
    /// Every committed file, by path relative to the tree root, `/`-separated.
    pub files: BTreeMap<String, String>,
}

/// Counts a delta and a closure produce, without pretending to know effort.
///
/// Design §26, restricted to what this slice can count. The four §26 names that are absent —
/// `public_contracts_changed`, `state_machine_changes`, `binding_changes`, `topology_changes` —
/// each count a construct family the delta does not compare yet, so each would be a number that is
/// zero because nothing produces it rather than because nothing happened. A metric that cannot move
/// is worse than a missing one: it reads as evidence.
///
/// `conformance_scenarios_invalidated` is §26's `conformance_scenarios_potentially_affected` with
/// the word changed, and the word is the wave's decision: *potentially affected* is a hedge that
/// leaves someone to decide which of them really were, and this engine does not make that offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Churn {
    /// How many semantic changes the delta holds.
    pub semantic_changes_total: usize,
    /// How many constructs a change is directly about.
    pub semantic_elements_directly_changed: usize,
    /// How many reference one of those directly.
    pub semantic_elements_directly_dependent: usize,
    /// How many reach one through at least one other.
    pub semantic_elements_transitively_impacted: usize,
    /// How many components are reached at all.
    pub components_impacted: usize,
    /// How many changes add or remove an actor's authority to invoke a command.
    pub actor_grants_changed: usize,
    /// How many scenarios the suite holds. Absent when no suite was given: a zero here would read
    /// as "nothing owed", which is a claim, where absence is only a question not asked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance_scenarios_total: Option<usize>,
    /// How many of them are owed again. Absent exactly when the total is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance_scenarios_invalidated: Option<usize>,
    /// How many generated artifacts the `--from` model derives.
    pub generated_artifacts_total: usize,
    /// How many of them are owed regeneration.
    pub generated_artifacts_owed: usize,
}

/// What a delta invalidates, with the paths that explain it.
///
/// Carries the delta rather than referring to one, because the first question a reader of an impact
/// report has is *what changed* — design §24's complaint is precisely about a report that says
/// something is affected without saying by what. One document answers both.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EssImpact {
    /// The shape this document is written in.
    pub format: &'static str,
    /// What moved.
    pub delta: EssDelta,
    /// Which suite was narrowed, and what produced it. Absent when no suite was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<SuiteProvenance>,
    /// Every construct any change reached, with the path that explains it.
    #[serde(serialize_with = "serialize_paths")]
    impacts: Vec<ImpactPath>,
    /// What the suite owes again. Absent exactly when `suite` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalidation: Option<Invalidation>,
    /// Which generated artifacts are owed regeneration.
    pub artifacts: ArtifactAnswer,
    /// Deterministic counts.
    pub churn: Churn,
}

impl EssImpact {
    /// Every construct any change reached, in canonical order: by change, then by construct.
    pub fn impacts(&self) -> &[ImpactPath] {
        &self.impacts
    }

    /// The impacts one change accounts for.
    pub fn impacts_of<'a>(&'a self, change: &'a ChangeId) -> impl Iterator<Item = &'a ImpactPath> {
        self.impacts
            .iter()
            .filter(move |path| &path.change == change)
    }

    /// The report as canonical JSON, with a trailing newline.
    ///
    /// # Panics
    ///
    /// It does not, for the reason [`EssDelta::to_canonical_json`] does not: `serde_json` has one
    /// error of its own — a map key that is not a string — and the only map in this document is
    /// keyed by [`ScenarioId`], which serialises as one. The `unwrap_or_else` names the impossible
    /// case rather than hiding it.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("the impact report serialises: {error}"));
        json.push('\n');
        json
    }
}

/// What moving from `before` to `after` invalidates: the suite's scenarios when a suite is given,
/// and the generated artifacts always.
///
/// The comparison is run here rather than taken as an argument, and that is mechanism 5 of the
/// fail-closed list in the module documentation: a caller cannot hand in a delta computed from one
/// pair and a graph built from another, because there is nowhere to hand a delta in. The artifact
/// inventory is derived here too, from the same `before` model, for the same reason.
///
/// `tree` is the committed generated tree, when the caller has one: each artifact's stamped
/// provenance is then checked against what its slice computes, and an artifact whose claim cannot
/// be read or does not hold is owed outright — stated as such, never silently narrowed past.
///
/// # Errors
///
/// [`ImpactRefusal`] when the two specifications are not two revisions of one system, or when the
/// given suite was not produced from the `before` revision or carries a contract digest its model
/// does not compute.
pub fn impact(
    before: &EssIr,
    after: &EssIr,
    suite: Option<&ConformanceSuite>,
    tree: Option<&GeneratedTree>,
) -> Result<EssImpact, ImpactRefusal> {
    let delta = diff(before, after).map_err(|refusal| ImpactRefusal::Pair { refusal })?;

    if let Some(suite) = suite {
        if suite.provenance.system != delta.before.system.to_string() {
            return Err(ImpactRefusal::SuiteFromAnotherSystem {
                suite: suite.provenance.system.clone(),
                revisions: delta.before.system.clone(),
            });
        }
        if suite.provenance.spec_digest != delta.before.spec_digest {
            return Err(ImpactRefusal::SuiteFromAnotherRevision {
                suite: suite.provenance.spec_digest.clone(),
                before: delta.before.spec_digest.clone(),
                after: delta.after.spec_digest.clone(),
            });
        }
        let expected = ess_gen::Provenance::of(before).contract_digest;
        if suite.provenance.contract_digest.as_str() != expected {
            return Err(ImpactRefusal::SuiteContractMismatch {
                suite: suite.provenance.contract_digest.clone(),
                expected,
            });
        }
    }

    let graph = SemanticDependencyGraph::of(before).merged(&SemanticDependencyGraph::of(after));
    // Mechanism 6. The families the delta does not compare are checked for canonical equality, and
    // any difference owes everything: an empty delta over two different models would otherwise be
    // a narrowing to nothing, which is a survival claim nothing here is entitled to make. One
    // comparison feeds both answers — the suite's and the artifacts' — because it is one fact.
    let uncompared_moved = uncompared_families(before) != uncompared_families(after);

    let mut impacts = Vec::new();
    let invalidation = suite.map(|suite| {
        let (walked, mut invalidation) = analyse(&delta, &graph, suite);
        impacts = walked;
        if uncompared_moved {
            invalidation = invalidation.joined(Invalidation::Whole {
                because: WholeAnswer::UncomparedFamilyChanged,
            });
        }
        invalidation
    });
    if suite.is_none() {
        // The construct walk does not need a suite; only the scenario intersection does.
        impacts = construct_impacts(&delta, &graph);
    }

    let inventory = artifact_inventory(before, tree);
    let artifacts = analyse_artifacts(&delta, &graph, before, &inventory, tree, uncompared_moved);

    let churn = churn(
        &delta,
        &impacts,
        invalidation.as_ref(),
        suite,
        &artifacts,
        inventory.len(),
    );

    Ok(EssImpact {
        format: IMPACT_FORMAT,
        delta,
        suite: suite.map(|suite| suite.provenance.clone()),
        impacts,
        invalidation,
        artifacts,
        churn,
    })
}

/// The canonical form of everything the delta does not compare, for mechanism 6's equality check.
///
/// Serialisation is the comparison because equality is all that is asked: which construct differs
/// and in which direction is exactly the question wave 5 deliberately does not answer for these
/// families, and D-1 already settled that canonical equality is the decidable, cheap fragment. The
/// keys and every map inside are ordered (`BTreeMap` throughout the IR), so two calls over equal
/// models produce equal bytes.
fn uncompared_families(ir: &EssIr) -> String {
    serde_json::to_string(&serde_json::json!({
        "bindings": ir.bindings,
        "commands": ir.commands,
        "conversions": ir.conversions,
        "entities": ir.entities,
        "views": ir.views,
        "workloads": ir.workloads,
    }))
    .unwrap_or_else(|error| {
        panic!("the IR serialises, as every projection already relies on: {error}")
    })
}

/// The closure itself: one walk per change, joined.
///
/// Crate-private and taking its three inputs explicitly, so a test can reach the two fail-closed
/// arms — a change with no construct to seed from, and a suite resting on an ungraphed construct —
/// without a pair of specifications built to produce them.
pub(crate) fn analyse(
    delta: &EssDelta,
    graph: &SemanticDependencyGraph,
    suite: &ConformanceSuite,
) -> (Vec<ImpactPath>, Invalidation) {
    let mut invalidation = Invalidation::nothing();

    // Mechanism 4, before any change is looked at: a suite resting on a construct the graph has no
    // node for can never be narrowed correctly, whatever the changes turn out to be.
    for construct in suite.dependencies() {
        if !graph.nodes().contains(construct) {
            invalidation = invalidation.joined(Invalidation::Whole {
                because: WholeAnswer::UngraphedDependency {
                    construct: construct.clone(),
                },
            });
        }
    }

    for change in delta.changes() {
        let id = change.id();
        // Mechanism 3. A change to the specification itself names no construct, so there is nothing
        // to seed a closure at — and the alternative to owing everything is owing nothing.
        let Some(subject) = change.subject() else {
            invalidation = invalidation.joined(Invalidation::Whole {
                because: WholeAnswer::SystemChanged { change: id },
            });
            continue;
        };

        let reach = graph.closure(&subject);
        let mut scenarios: BTreeMap<ScenarioId, ScenarioImpact> = BTreeMap::new();
        for (scenario_id, scenario) in &suite.scenarios {
            if let Some(reason) = nearest_reason(&reach, scenario, &id) {
                scenarios.insert(
                    scenario_id.clone(),
                    ScenarioImpact {
                        reasons: vec![reason],
                    },
                );
            }
        }
        invalidation = invalidation.joined(Invalidation::Narrowed { scenarios });
    }

    (construct_impacts(delta, graph), invalidation)
}

/// Every construct any change reached, with the path that explains it — the walk that needs no
/// suite, split out so a report without one still carries its impacts.
fn construct_impacts(delta: &EssDelta, graph: &SemanticDependencyGraph) -> Vec<ImpactPath> {
    let mut impacts: Vec<ImpactPath> = Vec::new();
    for change in delta.changes() {
        let id = change.id();
        let Some(subject) = change.subject() else {
            continue;
        };
        let reach = graph.closure(&subject);
        for target in reach.constructs() {
            impacts.push(ImpactPath {
                change: id.clone(),
                target: target.clone(),
                edges: reach
                    .path(target)
                    .expect("a reached construct has the path it was reached by")
                    .to_vec(),
            });
        }
    }
    impacts.sort();
    impacts.dedup();
    impacts
}

/// Every generated artifact the `before` model derives, each with the slice it derives from.
///
/// The projections are generated in memory — the same call `protocol ess generate` makes, so the
/// inventory cannot disagree with the committed tree about what exists. The suite and the
/// synthesised workspaces enter as whole-model artifacts without being synthesised: their slices
/// are the whole model by construction (a suite holds a scenario for every construct that obliges
/// one; a workspace is one tree, regenerated whole), so nothing about their content is needed to
/// answer for them.
///
/// Workspace entries follow the tree when one was read — one per committed `rust/<root>/` — and
/// collapse to a single `rust` obligation when none was: without a tree there is nothing to name
/// the roots by, and one honest obligation beats a guessed list.
fn artifact_inventory(
    before: &EssIr,
    tree: Option<&GeneratedTree>,
) -> BTreeMap<ArtifactId, ModelSlice> {
    let mut inventory: BTreeMap<ArtifactId, ModelSlice> = BTreeMap::new();
    let projections = ess_gen::generate_all(before)
        .unwrap_or_else(|defect| panic!("the projections of a resolved model generate: {defect}"));
    for (path, artifact) in projections {
        inventory.insert(ArtifactId::Projection { path }, artifact.slice);
    }
    inventory.insert(ArtifactId::Suite, ModelSlice::WholeModel);

    match tree {
        Some(tree) => {
            let mut roots: BTreeSet<String> = BTreeSet::new();
            for path in tree.files.keys() {
                if let Some(rest) = path.strip_prefix("rust/") {
                    if let Some((root, _)) = rest.split_once('/') {
                        roots.insert(format!("rust/{root}"));
                    }
                }
            }
            for root in roots {
                inventory.insert(ArtifactId::Workspace { path: root }, ModelSlice::WholeModel);
            }
        }
        None => {
            inventory.insert(
                ArtifactId::Workspace {
                    path: "rust".to_owned(),
                },
                ModelSlice::WholeModel,
            );
        }
    }
    inventory
}

/// The artifact half of the analysis: which artifacts the delta owes, or why all of them are.
///
/// The same mechanisms as [`analyse`], in the same order, because they are the same argument at a
/// different granularity: a change the closure cannot follow owes everything (mechanism 3), a
/// slice resting on an ungraphed construct owes everything (mechanism 4), a family the delta does
/// not compare owes everything (mechanism 6) — and only then is anything narrowed. A committed
/// artifact whose provenance cannot be read, or whose contract digest is not what its slice
/// computes, is owed before its slice is even consulted: a claim that cannot be checked and a
/// claim that is false get the same answer.
/// The committed tree's claims, checked artifact by artifact — every failure an obligation.
///
/// A projection the tree lacks is owed its creation; a workspace speaks through its `plan.json`
/// (the granularity the tree is regenerated at) and one that cannot is owed as unreadable; a
/// stamped digest that is not what the artifact's slice computes against `before` is owed as a
/// false claim; and a committed file the model derives nothing at is owed as unfollowable —
/// absence from the inventory is not knowledge about a file, and the one fail-closed thing to say
/// about it is that standing still is not something it has established. The suite is deliberately
/// not here: its claims are checked — and refused, not owed — through the `--suite` door.
fn verify_committed(
    before: &EssIr,
    inventory: &BTreeMap<ArtifactId, ModelSlice>,
    tree: &GeneratedTree,
    owed: &mut BTreeMap<ArtifactId, ArtifactObligation>,
) {
    let mint = ess_gen::provenance::ProvenanceMint::new(before);
    for (id, slice) in inventory {
        let committed = match id {
            ArtifactId::Projection { path } => match tree.files.get(path) {
                None => {
                    owed.insert(id.clone(), ArtifactObligation::Missing);
                    continue;
                }
                Some(contents) => contents,
            },
            ArtifactId::Workspace { path } => match tree.files.get(&format!("{path}/plan.json")) {
                None => {
                    owed.insert(id.clone(), ArtifactObligation::ProvenanceUnreadable);
                    continue;
                }
                Some(contents) => contents,
            },
            ArtifactId::Suite => continue,
        };
        match ess_gen::Provenance::read_digests(committed) {
            None => {
                owed.insert(id.clone(), ArtifactObligation::ProvenanceUnreadable);
            }
            Some(read) => {
                let expected = mint.digest_of(slice);
                if read.contract_digest != expected {
                    owed.insert(
                        id.clone(),
                        ArtifactObligation::ContractMismatch {
                            committed: read.contract_digest,
                            expected,
                        },
                    );
                }
            }
        }
    }
    for path in tree.files.keys() {
        let inside_workspace = inventory.keys().any(|id| {
            matches!(id, ArtifactId::Workspace { path: root }
                if path == root || path.starts_with(&format!("{root}/")))
        });
        if inside_workspace {
            continue;
        }
        let id = ArtifactId::Projection { path: path.clone() };
        if !inventory.contains_key(&id) {
            owed.insert(id, ArtifactObligation::Unfollowed);
        }
    }
}

fn analyse_artifacts(
    delta: &EssDelta,
    graph: &SemanticDependencyGraph,
    before: &EssIr,
    inventory: &BTreeMap<ArtifactId, ModelSlice>,
    tree: Option<&GeneratedTree>,
    uncompared_moved: bool,
) -> ArtifactAnswer {
    if uncompared_moved {
        return ArtifactAnswer::Whole {
            because: WholeAnswer::UncomparedFamilyChanged,
        };
    }
    for change in delta.changes() {
        if change.subject().is_none() {
            return ArtifactAnswer::Whole {
                because: WholeAnswer::SystemChanged {
                    change: change.id(),
                },
            };
        }
    }
    for slice in inventory.values() {
        if let ModelSlice::Constructs { seeds } = slice {
            for seed in seeds {
                if !graph.nodes().contains(seed) {
                    return ArtifactAnswer::Whole {
                        because: WholeAnswer::UngraphedDependency {
                            construct: seed.clone(),
                        },
                    };
                }
            }
        }
    }

    // Every change names a subject from here on — the System arm above returned.
    let subjects: Vec<(ChangeId, EssSemanticRef)> = delta
        .changes()
        .iter()
        .filter_map(|change| change.subject().map(|subject| (change.id(), subject)))
        .collect();

    let mut owed: BTreeMap<ArtifactId, ArtifactObligation> = BTreeMap::new();
    if let Some(tree) = tree {
        verify_committed(before, inventory, tree, &mut owed);
    }

    for (id, slice) in inventory {
        if owed.contains_key(id) {
            // Already owed for a stronger reason than a narrowing could give.
            continue;
        }
        let mut reasons: Vec<ImpactPath> = Vec::new();
        match slice {
            ModelSlice::WholeModel => {
                // Every construct is in a whole-model slice, the changed one included, at no
                // distance: the empty path is the honest explanation, exactly as it is for a
                // directly changed construct in a scenario's answer.
                for (change, subject) in &subjects {
                    reasons.push(ImpactPath {
                        change: change.clone(),
                        target: subject.clone(),
                        edges: Vec::new(),
                    });
                }
            }
            ModelSlice::Constructs { seeds } => {
                let members = graph.slice(seeds);
                for (change, subject) in &subjects {
                    if let Some(edges) = members.get(subject) {
                        reasons.push(ImpactPath {
                            change: change.clone(),
                            target: subject.clone(),
                            edges: edges.clone(),
                        });
                    }
                }
            }
        }
        if !reasons.is_empty() {
            reasons.sort();
            reasons.dedup();
            owed.insert(id.clone(), ArtifactObligation::SliceMoved { reasons });
        }
    }

    ArtifactAnswer::Narrowed { owed }
}

/// The closest construct in a scenario's dependency set that this closure reached.
///
/// Closest by [`ImpactClass`] first and by construct name second, both of which are total orders, so
/// two runs cannot choose two different explanations for one scenario. `None` when the closure
/// reached nothing the scenario rests on — which is not a claim that the scenario survived, only
/// that this change did not reach it.
fn nearest_reason(
    reach: &Reach,
    scenario: &ConformanceScenario,
    change: &ChangeId,
) -> Option<ImpactPath> {
    scenario
        .source
        .iter()
        .filter_map(|construct| {
            reach.path(construct).map(|edges| ImpactPath {
                change: change.clone(),
                target: construct.clone(),
                edges: edges.to_vec(),
            })
        })
        .min_by(|left, right| {
            left.edges
                .len()
                .cmp(&right.edges.len())
                .then_with(|| left.target.cmp(&right.target))
        })
}

/// The counts, from the delta and the closures that have already been computed.
fn churn(
    delta: &EssDelta,
    impacts: &[ImpactPath],
    invalidation: Option<&Invalidation>,
    suite: Option<&ConformanceSuite>,
    artifacts: &ArtifactAnswer,
    artifacts_total: usize,
) -> Churn {
    // One class per construct: the strongest one any change gives it. A type that one change is
    // about and another merely reaches is counted once, as changed.
    let mut strongest: BTreeMap<&EssSemanticRef, ImpactClass> = BTreeMap::new();
    for path in impacts {
        let class = path.class();
        strongest
            .entry(&path.target)
            .and_modify(|held| *held = (*held).min(class))
            .or_insert(class);
    }

    let count = |wanted: ImpactClass| strongest.values().filter(|class| **class == wanted).count();

    Churn {
        semantic_changes_total: delta.len(),
        semantic_elements_directly_changed: count(ImpactClass::DirectlyChanged),
        semantic_elements_directly_dependent: count(ImpactClass::DirectlyDependent),
        semantic_elements_transitively_impacted: count(ImpactClass::TransitivelyImpacted),
        components_impacted: strongest
            .keys()
            .filter(|construct| matches!(construct, EssSemanticRef::Component { .. }))
            .count(),
        actor_grants_changed: delta
            .changes()
            .iter()
            .filter(|change| {
                matches!(
                    change,
                    SemanticChange::Actor {
                        changed: ActorChange::GrantAdded { .. } | ActorChange::GrantRemoved { .. },
                        ..
                    }
                )
            })
            .count(),
        conformance_scenarios_total: suite.map(ConformanceSuite::len),
        conformance_scenarios_invalidated: suite
            .zip(invalidation)
            .map(|(suite, invalidation)| invalidation.owed(suite).len()),
        generated_artifacts_total: artifacts_total,
        generated_artifacts_owed: artifacts.owed_count(artifacts_total),
    }
}

#[cfg(test)]
mod tests {
    use aep_domain::evidence::SpecDigest;
    use ess_conformance::scenario::{DeclaredTypeRef, ScenarioPurpose, SuiteFormat, ViewRef};
    use ess_domain::name::Version;

    use super::*;
    use crate::change::{SystemChange, TypeChange};
    use crate::delta::EssRevisionRef;

    /// A qualified name for a fixture.
    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid qualified name")
    }

    /// A revision reference for a fixture.
    fn revision(digest: &str) -> EssRevisionRef {
        EssRevisionRef {
            system: name("catalog"),
            specification_version: Version::V1,
            spec_digest: SpecDigest::new(digest).expect("a full SHA-256 in lower-case hex"),
        }
    }

    /// A suite holding one scenario that depends on the constructs given, and on nothing else.
    fn suite_depending_on(source: impl IntoIterator<Item = EssSemanticRef>) -> ConformanceSuite {
        let mut suite = ConformanceSuite::new(SuiteProvenance {
            suite_version: SuiteFormat::CURRENT,
            system: "catalog".to_owned(),
            specification_version: "v1".to_owned(),
            spec_digest: SpecDigest::new(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .expect("a digest"),
            contract_digest: SpecDigest::new(
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
            )
            .expect("a digest"),
            compiler_version: "0.1.0".to_owned(),
            generator_version: "0.1.0".to_owned(),
            synthesizer_version: "0.1.0".to_owned(),
        });
        suite
            .insert(
                ScenarioId::parse("catalog.pricing.PublishPriceList/outcome/published")
                    .expect("a scenario id"),
                ConformanceScenario::new(
                    ScenarioPurpose::new("a fixture").expect("a purpose"),
                    Vec::new(),
                    source,
                ),
            )
            .expect("the suite is empty");
        suite
    }

    #[test]
    fn a_change_to_the_specification_itself_owes_the_whole_suite() {
        // Reaching the state this rule is load-bearing in takes care. The scenario has to depend on
        // **nothing**, so that mechanism 4 provably cannot fire and mechanism 3 is the only thing
        // standing between this fixture and an answer of zero — a scenario naming any construct at
        // all against an empty graph is caught by the other mechanism, and the test would then pass
        // with mechanism 3 deleted.
        let delta = EssDelta::new(
            revision("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            revision("fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"),
            vec![SemanticChange::System {
                subject: name("catalog"),
                changed: SystemChange::VersionChanged {
                    before: Version::V1,
                    after: Version::new(2).expect("v2"),
                },
            }],
        );
        let suite = suite_depending_on([]);
        let graph = SemanticDependencyGraph::of(&empty_ir());
        assert!(
            suite.dependencies().is_empty(),
            "the fixture must rest on nothing, or the ungraphed-dependency mechanism answers first"
        );

        let (_, invalidation) = analyse(&delta, &graph, &suite);

        assert!(
            invalidation.is_whole(),
            "a change with no construct to seed a closure at must owe everything: {invalidation:?}"
        );
        assert_eq!(invalidation.owed(&suite).len(), suite.len());
    }

    #[test]
    fn a_suite_resting_on_a_construct_the_graph_has_no_node_for_owes_the_whole_suite() {
        // Mechanism 4. The scenario depends on a view, the graph knows no views, and the one change
        // is about a type — so a narrowing would report zero scenarios, correctly by its own
        // arithmetic and wrong about the world.
        let delta = EssDelta::new(
            revision("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            revision("fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"),
            vec![SemanticChange::Type {
                subject: DeclaredTypeRef::new(name("catalog.pricing.Currency")),
                changed: TypeChange::VariantRemoved {
                    variant: "GBP".to_owned(),
                },
            }],
        );
        let suite =
            suite_depending_on([ViewRef::new(name("catalog.pricing.PriceListSummary")).into()]);
        let graph = SemanticDependencyGraph::of(&empty_ir());

        let (_, invalidation) = analyse(&delta, &graph, &suite);

        assert_eq!(
            invalidation.owed(&suite).len(),
            1,
            "the scenario is owed again: {invalidation:?}"
        );
        assert!(invalidation.is_whole());
    }

    #[test]
    fn a_whole_answer_absorbs_a_narrowing_whichever_way_round_they_are_joined() {
        // The lattice property mechanism 2 rests on, asserted rather than assumed: `Whole` is the
        // top, so processing one more change can only widen what is owed.
        let whole = Invalidation::Whole {
            because: WholeAnswer::SystemChanged {
                change: SemanticChange::System {
                    subject: name("catalog"),
                    changed: SystemChange::SummaryChanged {
                        before: None,
                        after: None,
                    },
                }
                .id(),
            },
        };
        let narrowed = Invalidation::Narrowed {
            scenarios: BTreeMap::new(),
        };

        assert!(whole.clone().joined(narrowed.clone()).is_whole());
        assert!(narrowed.joined(whole).is_whole());
    }

    /// A compiled specification with nothing in it.
    ///
    /// Built by hand rather than compiled, because these three tests are about the algebra and not
    /// about any specification: what they need is a graph that reaches nothing, so that a narrowing
    /// would report zero and the fail-closed arm is the only thing that can save it.
    fn empty_ir() -> EssIr {
        EssIr {
            system: name("catalog"),
            version: Version::V1,
            naming: ess_domain::name::Naming::default(),
            summary: None,
            domains: BTreeMap::new(),
            types: BTreeMap::new(),
            conversions: Vec::new(),
            entities: BTreeMap::new(),
            commands: BTreeMap::new(),
            events: BTreeMap::new(),
            errors: BTreeMap::new(),
            views: BTreeMap::new(),
            actors: BTreeMap::new(),
            bindings: BTreeMap::new(),
            components: BTreeMap::new(),
            workloads: BTreeMap::new(),
        }
    }
}
