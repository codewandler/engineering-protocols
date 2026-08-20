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
//!
//! `tests/impact.rs` breaks each of 3, 4 and 5 and watches it fail.
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
pub const IMPACT_FORMAT: &str = "ess-impact/1";

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

/// Why a whole suite is owed again.
///
/// Both variants are the fail-closed escape hatch: a change the closure cannot follow, and a suite
/// resting on something the graph cannot see. Neither is an error — the analysis worked and the
/// honest answer is *all of it*, which is exactly what G19 would have said.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum WholeSuite {
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
    /// The suite rests on a construct neither revision's graph has a node for.
    ///
    /// A scenario naming something the walk does not build a node for can never be reached by any
    /// closure, so narrowing would quietly leave it out of every answer. That is the one shape in
    /// which a narrowing is wrong and looks right, so it is not narrowed at all.
    UngraphedDependency {
        /// What the suite named.
        construct: EssSemanticRef,
    },
}

impl fmt::Display for WholeSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemChanged { change } => write!(
                f,
                "`{change}` is about the specification itself, which no scenario names as a \
                 dependency — so nothing can be narrowed away from it"
            ),
            Self::UngraphedDependency { construct } => write!(
                f,
                "a scenario depends on {construct}, which neither revision's dependency graph has a \
                 node for: no closure could ever reach it, and leaving it out of every answer is \
                 the one way a narrowing is wrong and looks right"
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
        because: WholeSuite,
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
    /// How many scenarios the suite holds.
    pub conformance_scenarios_total: usize,
    /// How many of them are owed again.
    pub conformance_scenarios_invalidated: usize,
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
    /// Which suite was narrowed, and what produced it.
    pub suite: SuiteProvenance,
    /// Every construct any change reached, with the path that explains it.
    #[serde(serialize_with = "serialize_paths")]
    impacts: Vec<ImpactPath>,
    /// What the suite owes again.
    pub invalidation: Invalidation,
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

/// What moving from `before` to `after` invalidates in `suite`.
///
/// The comparison is run here rather than taken as an argument, and that is mechanism 5 of the
/// fail-closed list in the module documentation: a caller cannot hand in a delta computed from one
/// pair and a graph built from another, because there is nowhere to hand a delta in.
///
/// # Errors
///
/// [`ImpactRefusal`] when the two specifications are not two revisions of one system, or when the
/// suite was not produced from the `before` revision.
pub fn impact(
    before: &EssIr,
    after: &EssIr,
    suite: &ConformanceSuite,
) -> Result<EssImpact, ImpactRefusal> {
    let delta = diff(before, after).map_err(|refusal| ImpactRefusal::Pair { refusal })?;

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

    let graph = SemanticDependencyGraph::of(before).merged(&SemanticDependencyGraph::of(after));
    let (impacts, invalidation) = analyse(&delta, &graph, suite);
    let churn = churn(&delta, &impacts, &invalidation, suite);

    Ok(EssImpact {
        format: IMPACT_FORMAT,
        delta,
        suite: suite.provenance.clone(),
        impacts,
        invalidation,
        churn,
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
    let mut impacts: Vec<ImpactPath> = Vec::new();
    let mut invalidation = Invalidation::nothing();

    // Mechanism 4, before any change is looked at: a suite resting on a construct the graph has no
    // node for can never be narrowed correctly, whatever the changes turn out to be.
    for construct in suite.dependencies() {
        if !graph.nodes().contains(construct) {
            invalidation = invalidation.joined(Invalidation::Whole {
                because: WholeSuite::UngraphedDependency {
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
                because: WholeSuite::SystemChanged { change: id },
            });
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

    impacts.sort();
    impacts.dedup();
    (impacts, invalidation)
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

/// The counts, from the delta and the closure that has already been computed.
fn churn(
    delta: &EssDelta,
    impacts: &[ImpactPath],
    invalidation: &Invalidation,
    suite: &ConformanceSuite,
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
        conformance_scenarios_total: suite.len(),
        conformance_scenarios_invalidated: invalidation.owed(suite).len(),
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
            spec_digest: SpecDigest::new(digest).expect("sixteen lower-case hex characters"),
        }
    }

    /// A suite holding one scenario that depends on the constructs given, and on nothing else.
    fn suite_depending_on(source: impl IntoIterator<Item = EssSemanticRef>) -> ConformanceSuite {
        let mut suite = ConformanceSuite::new(SuiteProvenance {
            suite_version: SuiteFormat::CURRENT,
            system: "catalog".to_owned(),
            specification_version: "v1".to_owned(),
            spec_digest: SpecDigest::new("0123456789abcdef").expect("a digest"),
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
            revision("0123456789abcdef"),
            revision("fedcba9876543210"),
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
            revision("0123456789abcdef"),
            revision("fedcba9876543210"),
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
            because: WholeSuite::SystemChanged {
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
