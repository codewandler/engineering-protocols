//! Which construct rests on which, and the path that says so.
//!
//! Design §22. A delta answers *what moved*; this answers *what stands on what moved*, and it is
//! built from the same [`EssIr`] the delta compared, so nothing here reads a source file or a
//! projection.
//!
//! # The edge points from the dependent to the dependency
//!
//! `binding notify-on-invoice-created --reacts-to--> event billing.invoice.InvoiceCreated`, never
//! the other way round. That direction is the one an author writes — a declaration names what it
//! references — so building the graph is a walk rather than an inversion, and there is one place
//! where the arrow could be turned around by mistake instead of eleven.
//!
//! A closure then runs the edges **backwards**: given a construct that moved, every node with a path
//! *to* it is a node that rests on it. [`SemanticDependencyGraph::closure`] is that walk, and it
//! keeps the edges it crossed, which is what makes design §24's requirement — every impact
//! explainable by at least one path — a property of the type rather than of a renderer.
//!
//! [`SemanticDependencyGraph::slice`] is the same question asked from the other end: given the
//! constructs a generated artifact is *about*, everything those constructs rest on is the model
//! slice the artifact derives from. Wave 7 records a digest of that slice on every generated
//! artifact, which is why the walk lives here beside the IR rather than in `ess-diff` where wave 5
//! first built it: the crate that stamps a slice digest and the crate that asks "did this slice
//! move" must walk one graph, or the two answers drift.
//!
//! # Both revisions, unioned
//!
//! [`SemanticDependencyGraph::of`] reads one [`EssIr`]. Impact analysis uses the union of both
//! (§68), and that is not a convenience: a construct that was *removed* exists only in the `before`
//! model and one that was *added* only in the `after` one, so a graph built from either alone
//! reaches nothing for half the change kinds this slice produces. Reaching nothing is the fail-open
//! answer, so the union is the only safe one. [`SemanticDependencyGraph::merged`] does it, and a
//! union of edge sets can only ever reach *more*.
//!
//! # What is not an edge, and why
//!
//! | left out | why |
//! |---|---|
//! | a predicate's fact paths — an outcome's `when:`, an entity's invariants, a view's filter | a fact path is a name inside a construct, not a reference to one; reading `total.amount` as an edge to `Money` would mean parsing a path for meaning, which is invariant 13's rule one level up |
//! | a workload's component | there is no `EssSemanticRef` for a workload, and inventing one here would be a name no other tool in this workspace resolves |
//! | a conversion's two types | same: a conversion has no name to be a node under |
//!
//! Each is an absence rather than a silence: a change to any of the three is outside the six
//! families this slice compares, so no change can seed a closure that would have wanted them.
//!
//! # Determinism
//!
//! [`BTreeMap`] and [`BTreeSet`] throughout, so the walk order is the name order and a closure's
//! path is the same path on every machine. The breadth-first search takes the **shortest** path, and
//! ties are broken by the edge order, which is the node order — so two runs cannot pick two
//! different explanations for one impact.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::ir::{EssIr, ResolvedBody, ResolvedField, ResolvedInstance, ResolvedTypeRef};
use crate::refs::{
    ActorRef, BindingRef, CommandRef, ComponentRef, DeclaredTypeRef, DomainRef, EntityRef,
    ErrorRef, EssSemanticRef, EventRef, OutcomeRef, TransitionRef, ViewRef,
};

/// What one construct does to another.
///
/// A closed vocabulary, for design §10's reason applied to edges rather than to changes: a graph
/// whose edges are strings is a graph whose meaning is a convention, and "which of these paths is an
/// authority and which is a payload" becomes a question about spelling. Every variant below is
/// produced by [`SemanticDependencyGraph::of`] from a named field of the IR, and
/// `tests/graph.rs` reads the example specifications for one of each — because a relation nothing
/// mints is the defect class `docs/reviews/2026-08-20-guard-efficacy-review.md` was written about.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyRelation {
    /// A construct is declared inside a bounded context.
    DeclaredIn,
    /// A newtype wraps a declared type.
    Wraps,
    /// A construct has a field whose type reaches a declared type.
    FieldType,
    /// A union has a variant carrying a declared type.
    VariantType,
    /// An entity's `state` is typed by the enum its lifecycle synthesised.
    StateType,
    /// An entity's lifecycle declares a move.
    Moves,
    /// A view projects an entity.
    Projects,
    /// An outcome is a branch of a command.
    BranchOf,
    /// An outcome emits an event.
    Emits,
    /// An outcome reports a declared error.
    RefusesWith,
    /// An outcome acts on an entity.
    ActsOn,
    /// An outcome takes a declared move.
    Takes,
    /// An outcome reads the identity of what it created from an emitted event.
    Observes,
    /// An actor may invoke a command.
    MayInvoke,
    /// A binding reacts to an event.
    ReactsTo,
    /// A binding invokes a command.
    Invokes,
    /// A binding escalates into an event when its command does not run.
    EscalatesTo,
    /// A binding maps a value of a declared type into a command's input.
    Maps,
    /// A component owns a bounded context.
    Owns,
    /// A component accepts a command.
    Accepts,
    /// A component publishes an event.
    Publishes,
}

impl DependencyRelation {
    /// Every relation, for a check that each one is reachable from a real specification.
    ///
    /// Written from the same lines as the variants above, so a relation added without a walk that
    /// mints it fails `tests/graph.rs` rather than sitting in the vocabulary unproduced.
    pub const ALL: [Self; 21] = [
        Self::DeclaredIn,
        Self::Wraps,
        Self::FieldType,
        Self::VariantType,
        Self::StateType,
        Self::Moves,
        Self::Projects,
        Self::BranchOf,
        Self::Emits,
        Self::RefusesWith,
        Self::ActsOn,
        Self::Takes,
        Self::Observes,
        Self::MayInvoke,
        Self::ReactsTo,
        Self::Invokes,
        Self::EscalatesTo,
        Self::Maps,
        Self::Owns,
        Self::Accepts,
        Self::Publishes,
    ];

    /// The phrase a path reads with: `<dependent> <verb> <dependency>`.
    ///
    /// Present tense and active, because a path is read as a sentence by the person deciding whether
    /// to act on it — "binding notify-on-invoice-created reacts to event `InvoiceCreated`", not
    /// "`edge(reacts_to)`".
    pub const fn verb(self) -> &'static str {
        match self {
            Self::DeclaredIn => "is declared in",
            Self::Wraps => "wraps",
            Self::FieldType => "has a field of type",
            Self::VariantType => "has a variant carrying",
            Self::StateType => "takes its state from",
            Self::Moves => "is a move of",
            Self::Projects => "projects",
            Self::BranchOf => "is a branch of",
            Self::Emits => "emits",
            Self::RefusesWith => "refuses with",
            Self::ActsOn => "acts on",
            Self::Takes => "takes",
            Self::Observes => "reads the new identity from",
            Self::MayInvoke => "may invoke",
            Self::ReactsTo => "reacts to",
            Self::Invokes => "invokes",
            Self::EscalatesTo => "escalates into",
            Self::Maps => "maps a value of type",
            Self::Owns => "owns",
            Self::Accepts => "accepts",
            Self::Publishes => "publishes",
        }
    }
}

impl fmt::Display for DependencyRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.verb())
    }
}

/// One construct resting on another.
///
/// Field order is the sort order and the sentence order at once: a set of edges under one dependency
/// comes out ordered by the construct that depends on it, which is what makes a closure's choice of
/// path reproducible rather than a property of whichever walk filled the map.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct DependencyEdge {
    /// The construct that references the other.
    pub dependent: EssSemanticRef,
    /// What it does to it.
    pub relation: DependencyRelation,
    /// The construct it references.
    pub dependency: EssSemanticRef,
}

impl fmt::Display for DependencyEdge {
    /// `binding notify-on-invoice-created reacts to event billing.invoice.InvoiceCreated`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.dependent,
            self.relation.verb(),
            self.dependency
        )
    }
}

/// Every construct of a specification, and what each one rests on.
///
/// Built once per analysis and read many times, so the reverse index is stored rather than derived
/// per query: a closure asks "who depends on this" for every node it reaches, and computing that by
/// scanning every edge would make the walk quadratic in the size of the model for no gain in
/// clarity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDependencyGraph {
    /// Every construct, whether or not anything rests on it.
    nodes: BTreeSet<EssSemanticRef>,
    /// The edges arriving at each dependency — the direction a closure walks.
    dependents: BTreeMap<EssSemanticRef, BTreeSet<DependencyEdge>>,
}

impl SemanticDependencyGraph {
    /// The graph of one compiled specification.
    ///
    /// A walk, not a resolution: every reference in an [`EssIr`] is already a handle, so this reads
    /// each handle's [`name`](crate::ir::TypeHandle::name) and records an edge. No handle is carried into the
    /// graph — the nodes are [`EssSemanticRef`]s, which resolve against any compilation of the same
    /// specification, which is what lets the union of two revisions' graphs be one graph at all.
    #[must_use]
    pub fn of(ir: &EssIr) -> Self {
        let mut graph = Self {
            nodes: BTreeSet::new(),
            dependents: BTreeMap::new(),
        };

        graph.walk_domains(ir);
        graph.walk_types(ir);
        graph.walk_entities(ir);
        graph.walk_commands(ir);
        graph.walk_events(ir);
        graph.walk_errors(ir);
        graph.walk_views(ir);
        graph.walk_actors(ir);
        graph.walk_bindings(ir);
        graph.walk_components(ir);

        graph
    }

    /// This graph with another's nodes and edges added.
    ///
    /// Union, and only union. Impact analysis merges the `before` and `after` graphs, and a merge
    /// that dropped an edge present in one side would let a removed construct reach nothing — which
    /// is the fail-open answer this wave exists to refuse. There is no operation here that removes
    /// an edge.
    #[must_use]
    pub fn merged(mut self, other: &Self) -> Self {
        self.nodes.extend(other.nodes.iter().cloned());
        for (dependency, edges) in &other.dependents {
            self.dependents
                .entry(dependency.clone())
                .or_default()
                .extend(edges.iter().cloned());
        }
        self
    }

    /// Every construct in the graph.
    pub fn nodes(&self) -> &BTreeSet<EssSemanticRef> {
        &self.nodes
    }

    /// Every edge, in canonical order: by what is depended on, then by what depends on it.
    pub fn edges(&self) -> impl Iterator<Item = &DependencyEdge> {
        self.dependents.values().flatten()
    }

    /// How many edges there are.
    pub fn len(&self) -> usize {
        self.dependents.values().map(BTreeSet::len).sum()
    }

    /// `true` when nothing rests on anything — a specification of isolated declarations.
    pub fn is_empty(&self) -> bool {
        self.dependents.is_empty()
    }

    /// What rests on `dependency`, directly.
    pub fn dependents_of(
        &self,
        dependency: &EssSemanticRef,
    ) -> impl Iterator<Item = &DependencyEdge> {
        self.dependents.get(dependency).into_iter().flatten()
    }

    /// Everything that rests on `origin`, however far away, with the path that says so.
    ///
    /// Breadth-first, so the path kept for each construct is a **shortest** one — the shortest
    /// explanation is the one a reviewer can check — and the frontier is drained in name order, so
    /// two constructs at equal distance are visited in the same order every time. The origin is in
    /// the result with an empty path, because the construct that changed is impacted by definition
    /// and leaving it out would make a caller special-case it.
    #[must_use]
    pub fn closure(&self, origin: &EssSemanticRef) -> Reach {
        let mut reached: BTreeMap<EssSemanticRef, Vec<DependencyEdge>> = BTreeMap::new();
        reached.insert(origin.clone(), Vec::new());

        let mut frontier: VecDeque<EssSemanticRef> = VecDeque::new();
        frontier.push_back(origin.clone());

        while let Some(node) = frontier.pop_front() {
            let path = reached
                .get(&node)
                .expect("a node reaches the frontier only after its path is recorded")
                .clone();
            for edge in self.dependents_of(&node) {
                if reached.contains_key(&edge.dependent) {
                    continue;
                }
                let mut onward = path.clone();
                onward.push(edge.clone());
                reached.insert(edge.dependent.clone(), onward);
                frontier.push_back(edge.dependent.clone());
            }
        }

        Reach {
            origin: origin.clone(),
            reached,
        }
    }

    /// The model slice a set of seed constructs derives from: every construct any seed rests on,
    /// each with the edges that say so.
    ///
    /// [`Self::closure`] run the other way round — from a dependent towards what it depends on —
    /// plus one deliberate widening: **sub-constructs travel with their parents.** A command in the
    /// slice brings each of its outcomes and an entity each of its declared moves, although those
    /// edges point the other way (`outcome --is a branch of--> command`), because an outcome is
    /// part of its command's declaration: an artifact derived from a command was derived from its
    /// branches, and a slice that omitted them would stand still past a change to the error one of
    /// them refuses with. The two possible errors of a membership rule are not comparable — a
    /// too-big slice costs a regeneration nobody needed, a too-small one costs a false "still
    /// current" — so every doubt here is resolved by including more.
    ///
    /// The result maps every member to a shortest path from its nearest seed, ties broken in edge
    /// order, exactly as [`Self::closure`] does and for the same reason: two runs must not pick two
    /// different explanations for one membership. A seed maps to an empty path.
    #[must_use]
    pub fn slice(
        &self,
        seeds: &BTreeSet<EssSemanticRef>,
    ) -> BTreeMap<EssSemanticRef, Vec<DependencyEdge>> {
        // The forward index, derived on demand: the stored index answers "who rests on this", and
        // this walk asks "what does this rest on". Each node's edges are sorted so the breadth-first
        // tie-break is the edge order, not an accident of how the index was filled.
        let mut forward: BTreeMap<&EssSemanticRef, Vec<&DependencyEdge>> = BTreeMap::new();
        for edge in self.edges() {
            forward.entry(&edge.dependent).or_default().push(edge);
        }
        for edges in forward.values_mut() {
            edges.sort();
        }

        let mut reached: BTreeMap<EssSemanticRef, Vec<DependencyEdge>> = BTreeMap::new();
        let mut frontier: VecDeque<EssSemanticRef> = VecDeque::new();
        for seed in seeds {
            reached.insert(seed.clone(), Vec::new());
            frontier.push_back(seed.clone());
        }

        while let Some(node) = frontier.pop_front() {
            let path = reached
                .get(&node)
                .expect("a node reaches the frontier only after its path is recorded")
                .clone();
            let onward = |edges: &mut BTreeMap<EssSemanticRef, Vec<DependencyEdge>>,
                          frontier: &mut VecDeque<EssSemanticRef>,
                          next: &EssSemanticRef,
                          edge: &DependencyEdge| {
                if edges.contains_key(next) {
                    return;
                }
                let mut extended = path.clone();
                extended.push(edge.clone());
                edges.insert(next.clone(), extended);
                frontier.push_back(next.clone());
            };
            // Forward: what the node rests on.
            for edge in forward.get(&node).into_iter().flatten() {
                onward(&mut reached, &mut frontier, &edge.dependency, edge);
            }
            // Backward, for exactly two relations: the sub-constructs that are part of this
            // node's own declaration.
            for edge in self.dependents_of(&node) {
                if matches!(
                    edge.relation,
                    DependencyRelation::BranchOf | DependencyRelation::Moves
                ) {
                    onward(&mut reached, &mut frontier, &edge.dependent, edge);
                }
            }
        }

        reached
    }

    // ---- building ----------------------------------------------------------------------------

    /// Records one edge, and both of its ends as nodes.
    fn edge(
        &mut self,
        dependent: impl Into<EssSemanticRef>,
        relation: DependencyRelation,
        dependency: impl Into<EssSemanticRef>,
    ) {
        let edge = DependencyEdge {
            dependent: dependent.into(),
            relation,
            dependency: dependency.into(),
        };
        self.nodes.insert(edge.dependent.clone());
        self.nodes.insert(edge.dependency.clone());
        self.dependents
            .entry(edge.dependency.clone())
            .or_default()
            .insert(edge);
    }

    /// Records a construct that may have no edges at all — an actor that may invoke nothing, an
    /// enum nothing holds. Without this a leaf declaration would be absent from
    /// [`Self::nodes`], and a closure seeded at one would look like a closure seeded at a name the
    /// model does not have.
    fn node(&mut self, node: impl Into<EssSemanticRef>) {
        self.nodes.insert(node.into());
    }

    /// One edge per declared type a type reference reaches, including through `List` and `Map`.
    fn type_edges(
        &mut self,
        dependent: &EssSemanticRef,
        relation: DependencyRelation,
        type_ref: &ResolvedTypeRef,
    ) {
        for leaf in type_ref.named_leaves() {
            self.edge(dependent.clone(), relation, DeclaredTypeRef::from(leaf));
        }
    }

    /// One edge per declared type a field list reaches.
    fn field_edges(&mut self, dependent: &EssSemanticRef, fields: &[ResolvedField]) {
        for field in fields {
            self.type_edges(dependent, DependencyRelation::FieldType, &field.type_ref);
        }
    }

    /// Membership: every construct a bounded context declares is declared in it.
    ///
    /// Read from the domain's own member sets rather than from each construct's `domain` handle,
    /// because a [`ResolvedType`](crate::ir::ResolvedType) carries no `domain` field — the
    /// containment is recorded on the domain side only — and one rule that reads one place is better
    /// than two rules that have to agree.
    fn walk_domains(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.domains {
            let domain = DomainRef::new(name.clone());
            self.node(domain.clone());
            for handle in &resolved.types {
                self.edge(
                    DeclaredTypeRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
            for handle in &resolved.entities {
                self.edge(
                    EntityRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
            for handle in &resolved.commands {
                self.edge(
                    CommandRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
            for handle in &resolved.events {
                self.edge(
                    EventRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
            for handle in &resolved.errors {
                self.edge(
                    ErrorRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
            for handle in &resolved.views {
                self.edge(
                    ViewRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
            for handle in &resolved.actors {
                self.edge(
                    ActorRef::from(handle),
                    DependencyRelation::DeclaredIn,
                    domain.clone(),
                );
            }
        }
    }

    /// A type rests on the types its body reaches. An enum rests on nothing.
    fn walk_types(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.types {
            let subject: EssSemanticRef = DeclaredTypeRef::new(name.clone()).into();
            self.node(subject.clone());
            match &resolved.body {
                ResolvedBody::Newtype { of, .. } => {
                    self.type_edges(&subject, DependencyRelation::Wraps, of);
                }
                ResolvedBody::Struct { fields, .. } => self.field_edges(&subject, fields),
                // Nothing: a variant is a name, and a name is not a reference to a declaration.
                ResolvedBody::Enum { .. } => {}
                ResolvedBody::Union { variants, .. } => {
                    for payload in variants.values() {
                        self.type_edges(&subject, DependencyRelation::VariantType, payload);
                    }
                }
            }
        }
    }

    /// An entity rests on the types it holds and on the enum its lifecycle synthesised, and its
    /// declared moves rest on it.
    fn walk_entities(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.entities {
            let entity = EntityRef::new(name.clone());
            let subject: EssSemanticRef = entity.clone().into();
            self.node(subject.clone());

            self.type_edges(
                &subject,
                DependencyRelation::FieldType,
                &resolved.identity.type_ref,
            );
            self.field_edges(&subject, &resolved.fields);
            self.edge(
                subject.clone(),
                DependencyRelation::StateType,
                DeclaredTypeRef::from(&resolved.state_type),
            );

            for transition in &resolved.lifecycle.transitions {
                // A move has no qualified name of its own, so `TransitionRef` pairs it with the
                // entity that declares it — the same shape a conformance scenario records.
                let named = TransitionRef::new(entity.clone(), &transition.name)
                    .expect("a lifecycle's transition name is one qualified-name segment");
                self.edge(named, DependencyRelation::Moves, entity.clone());
            }
        }
    }

    /// A command rests on its input types; each outcome rests on the command and on everything the
    /// branch names.
    fn walk_commands(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.commands {
            let command = CommandRef::new(name.clone());
            let subject: EssSemanticRef = command.clone().into();
            self.node(subject.clone());
            self.field_edges(&subject, &resolved.input);

            for outcome in &resolved.outcomes {
                let branch: EssSemanticRef =
                    OutcomeRef::new(command.clone(), outcome.name.clone()).into();
                self.edge(
                    branch.clone(),
                    DependencyRelation::BranchOf,
                    command.clone(),
                );
                for emitted in &outcome.emits {
                    self.edge(
                        branch.clone(),
                        DependencyRelation::Emits,
                        EventRef::from(emitted),
                    );
                }
                if let Some(handle) = &outcome.error {
                    self.edge(
                        branch.clone(),
                        DependencyRelation::RefusesWith,
                        ErrorRef::from(handle),
                    );
                }
                // The `when:` predicate is deliberately not walked. See the module documentation:
                // a fact path is a name inside a construct, and reading one as a reference would be
                // parsing a path for meaning.
                if let Some(subject_of) = &outcome.subject {
                    let acted_on = EntityRef::from(&subject_of.entity);
                    self.edge(branch.clone(), DependencyRelation::ActsOn, acted_on.clone());
                    if let Some(transition) = subject_of.effect.transition() {
                        let named = TransitionRef::new(acted_on, &transition.name)
                            .expect("a lifecycle's transition name is one qualified-name segment");
                        self.edge(branch.clone(), DependencyRelation::Takes, named);
                    }
                    // Matched rather than read through `ResolvedInstance::event`: this module was
                    // written inside `ess-diff`, whose source scan bans every `EssIr` handle
                    // accessor by spelling, and the match is kept after the move because it is the
                    // one form that cannot pick up a handle by accident.
                    if let ResolvedInstance::Observed { event, .. } = &subject_of.instance {
                        self.edge(
                            branch.clone(),
                            DependencyRelation::Observes,
                            EventRef::from(event),
                        );
                    }
                }
            }
        }
    }

    /// An event rests on the types its payload reaches.
    fn walk_events(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.events {
            let subject: EssSemanticRef = EventRef::new(name.clone()).into();
            self.node(subject.clone());
            self.field_edges(&subject, &resolved.fields);
        }
    }

    /// A declared error rests on the types its payload reaches.
    fn walk_errors(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.errors {
            let subject: EssSemanticRef = ErrorRef::new(name.clone()).into();
            self.node(subject.clone());
            self.field_edges(&subject, &resolved.fields);
        }
    }

    /// A view rests on the entity it projects and on the types it exposes.
    fn walk_views(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.views {
            let subject: EssSemanticRef = ViewRef::new(name.clone()).into();
            self.node(subject.clone());
            self.edge(
                subject.clone(),
                DependencyRelation::Projects,
                EntityRef::from(&resolved.source),
            );
            self.field_edges(&subject, &resolved.fields);
        }
    }

    /// An actor rests on every command it may invoke.
    fn walk_actors(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.actors {
            let subject: EssSemanticRef = ActorRef::new(name.clone()).into();
            self.node(subject.clone());
            for handle in &resolved.may {
                self.edge(
                    subject.clone(),
                    DependencyRelation::MayInvoke,
                    CommandRef::from(handle),
                );
            }
        }
    }

    /// A binding rests on the event it reacts to, the command it invokes, the types it maps and the
    /// event it escalates into.
    fn walk_bindings(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.bindings {
            let subject: EssSemanticRef = BindingRef::new(name.clone()).into();
            self.node(subject.clone());
            self.edge(
                subject.clone(),
                DependencyRelation::ReactsTo,
                EventRef::from(&resolved.event),
            );
            self.edge(
                subject.clone(),
                DependencyRelation::Invokes,
                CommandRef::from(&resolved.command),
            );
            for mapping in &resolved.mapping {
                self.type_edges(&subject, DependencyRelation::Maps, &mapping.target_type);
            }
            if let Some(handle) = &resolved.escalation {
                self.edge(
                    subject.clone(),
                    DependencyRelation::EscalatesTo,
                    EventRef::from(handle),
                );
            }
        }
    }

    /// A component rests on what it owns, accepts and publishes.
    fn walk_components(&mut self, ir: &EssIr) {
        for (name, resolved) in &ir.components {
            let subject: EssSemanticRef = ComponentRef::new(name.clone()).into();
            self.node(subject.clone());
            for handle in &resolved.owns {
                self.edge(
                    subject.clone(),
                    DependencyRelation::Owns,
                    DomainRef::from(handle),
                );
            }
            for handle in &resolved.accepts {
                self.edge(
                    subject.clone(),
                    DependencyRelation::Accepts,
                    CommandRef::from(handle),
                );
            }
            for handle in &resolved.publishes {
                self.edge(
                    subject.clone(),
                    DependencyRelation::Publishes,
                    EventRef::from(handle),
                );
            }
        }
    }
}

/// How far a construct is from the change that reached it.
///
/// Design §23's vocabulary, restricted to the three this slice can produce. `Unaffected` is not
/// declared, and that is the wave's whole polarity in one omission: nothing here says a construct is
/// unaffected, because absence from a closure means the graph did not reach it and **not** that
/// gate G19 has been overruled. `PotentiallyImpacted` is not declared either — it would need a
/// comparison that can fail to decide, and this slice has none.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ImpactClass {
    /// The construct the change is about.
    DirectlyChanged,
    /// It references the changed construct itself.
    DirectlyDependent,
    /// It reaches the changed construct through at least one other.
    TransitivelyImpacted,
}

impl ImpactClass {
    /// The class a path of this length carries.
    #[must_use]
    pub const fn of(edges: usize) -> Self {
        match edges {
            0 => Self::DirectlyChanged,
            1 => Self::DirectlyDependent,
            _ => Self::TransitivelyImpacted,
        }
    }

    /// How it is written.
    pub const fn written(self) -> &'static str {
        match self {
            Self::DirectlyChanged => "directly-changed",
            Self::DirectlyDependent => "directly-dependent",
            Self::TransitivelyImpacted => "transitively-impacted",
        }
    }
}

impl fmt::Display for ImpactClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.written())
    }
}

/// Everything that rests on one construct, and the path to each.
///
/// The path is kept rather than recomputed on demand, because design §24's requirement is that an
/// impact is *explainable*, and a report that could produce the set but not the reason is the report
/// §24 was written against — `email-component risk = high` with nothing under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    /// The construct the walk started at.
    origin: EssSemanticRef,
    /// Every construct reached, with a shortest path from the origin to it.
    reached: BTreeMap<EssSemanticRef, Vec<DependencyEdge>>,
}

impl Reach {
    /// The construct the walk started at.
    pub fn origin(&self) -> &EssSemanticRef {
        &self.origin
    }

    /// Every construct reached, including the origin, in name order.
    pub fn constructs(&self) -> impl Iterator<Item = &EssSemanticRef> {
        self.reached.keys()
    }

    /// How many constructs were reached, the origin included.
    pub fn len(&self) -> usize {
        self.reached.len()
    }

    /// `true` when nothing at all was reached.
    ///
    /// It never is: the origin is always in the result. The method exists because clippy asks for it
    /// beside [`Self::len`], and answering it honestly is better than suppressing the lint.
    pub fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    /// `true` when `construct` rests on the origin, or is it.
    pub fn reaches(&self, construct: &EssSemanticRef) -> bool {
        self.reached.contains_key(construct)
    }

    /// Why `construct` was reached: the edges from the origin outward to it.
    ///
    /// Empty for the origin itself, which is the honest answer — it was not reached through
    /// anything, it *is* the thing that moved.
    pub fn path(&self, construct: &EssSemanticRef) -> Option<&[DependencyEdge]> {
        self.reached.get(construct).map(Vec::as_slice)
    }

    /// How far `construct` is from the origin.
    pub fn class(&self, construct: &EssSemanticRef) -> Option<ImpactClass> {
        self.path(construct).map(|path| ImpactClass::of(path.len()))
    }
}

#[cfg(test)]
mod tests {
    use ess_domain::name::QualifiedName;

    use super::*;

    /// A type name for a fixture.
    fn declared(name: &str) -> DeclaredTypeRef {
        DeclaredTypeRef::new(QualifiedName::new(name).expect("a valid qualified name"))
    }

    /// An event name for a fixture.
    fn event(name: &str) -> EventRef {
        EventRef::new(QualifiedName::new(name).expect("a valid qualified name"))
    }

    /// A three-node chain: an event has a field of `Money`, and `Money` has a field of `Currency`.
    fn chain() -> SemanticDependencyGraph {
        let mut graph = SemanticDependencyGraph {
            nodes: BTreeSet::new(),
            dependents: BTreeMap::new(),
        };
        graph.edge(
            event("a.b.Paid"),
            DependencyRelation::FieldType,
            declared("a.b.Money"),
        );
        graph.edge(
            declared("a.b.Money"),
            DependencyRelation::FieldType,
            declared("a.b.Currency"),
        );
        graph
    }

    #[test]
    fn a_closure_keeps_the_edges_that_explain_each_construct_it_reached() {
        // The fixture reaches the state the rule is load-bearing in: `Paid` does not reference
        // `Currency`, so the only way it can be reported is through `Money`, and the path is the
        // only thing that says so.
        let graph = chain();
        let currency: EssSemanticRef = declared("a.b.Currency").into();
        let reach = graph.closure(&currency);

        let paid: EssSemanticRef = event("a.b.Paid").into();
        assert_eq!(reach.class(&paid), Some(ImpactClass::TransitivelyImpacted));

        let path = reach.path(&paid).expect("the event was reached");
        assert_eq!(path.len(), 2, "two hops: {path:?}");
        assert_eq!(path[0].dependent, declared("a.b.Money").into());
        assert_eq!(path[0].dependency, currency);
        assert_eq!(path[1].dependent, paid);
        assert_eq!(path[1].dependency, declared("a.b.Money").into());
        assert_eq!(
            path[1].to_string(),
            "event a.b.Paid has a field of type type a.b.Money"
        );
    }

    #[test]
    fn the_construct_that_changed_is_in_its_own_closure_with_no_path() {
        let graph = chain();
        let currency: EssSemanticRef = declared("a.b.Currency").into();
        let reach = graph.closure(&currency);

        assert_eq!(reach.class(&currency), Some(ImpactClass::DirectlyChanged));
        assert_eq!(reach.path(&currency), Some(&[][..]));
    }

    #[test]
    fn a_closure_walks_the_edges_backwards_and_not_forwards() {
        // The direction is the one thing about this graph that can be wrong without failing to
        // compile, and getting it backwards produces a plausible, empty answer. `Paid` rests on
        // `Currency` and `Currency` rests on nothing, so a closure seeded at the *event* must reach
        // only the event.
        let graph = chain();
        let reach = graph.closure(&event("a.b.Paid").into());

        assert_eq!(
            reach.len(),
            1,
            "nothing in the fixture rests on the event, and the closure reported {} construct(s)",
            reach.len()
        );
        assert!(!reach.reaches(&declared("a.b.Money").into()));
    }

    #[test]
    fn a_slice_includes_its_seeds_each_with_no_path() {
        let graph = chain();
        let seeds: BTreeSet<EssSemanticRef> = [event("a.b.Paid").into()].into();

        let slice = graph.slice(&seeds);

        assert_eq!(slice.get(&event("a.b.Paid").into()), Some(&Vec::new()));
    }

    #[test]
    fn a_slice_reaches_what_a_seed_rests_on_transitively() {
        // The forward direction: the event rests on `Money`, which rests on `Currency`, so a change
        // to `Currency` is a change to what the event's artifact derives from — and the path is the
        // argument, hop by hop.
        let graph = chain();
        let seeds: BTreeSet<EssSemanticRef> = [event("a.b.Paid").into()].into();

        let slice = graph.slice(&seeds);

        let path = slice
            .get(&declared("a.b.Currency").into())
            .expect("the slice reaches Currency through Money");
        assert_eq!(path.len(), 2, "two hops: {path:?}");
        assert_eq!(path[0].dependent, event("a.b.Paid").into());
        assert_eq!(path[1].dependency, declared("a.b.Currency").into());
    }

    #[test]
    fn a_command_in_a_slice_brings_its_outcomes_and_what_they_name() {
        // The widening the membership rule exists for. The error is named only by the outcome, and
        // the outcome's edge points *at* the command — so a walk that only ran forward would omit
        // both, and an artifact derived from the command would claim to stand still past a change
        // to the error it documents.
        let mut graph = SemanticDependencyGraph {
            nodes: BTreeSet::new(),
            dependents: BTreeMap::new(),
        };
        let command =
            CommandRef::new(QualifiedName::new("a.b.Pay").expect("a valid qualified name"));
        let outcome = OutcomeRef::new(
            command.clone(),
            ess_domain::command::OutcomeName::new("rejected").expect("an outcome name"),
        );
        let error = ErrorRef::new(QualifiedName::new("a.b.Refused").expect("a valid name"));
        graph.edge(
            outcome.clone(),
            DependencyRelation::BranchOf,
            command.clone(),
        );
        graph.edge(
            outcome.clone(),
            DependencyRelation::RefusesWith,
            error.clone(),
        );

        let seeds: BTreeSet<EssSemanticRef> = [command.into()].into();
        let slice = graph.slice(&seeds);

        assert!(
            slice.contains_key(&outcome.clone().into()),
            "the outcome travels with its command: {slice:?}"
        );
        let path = slice
            .get(&error.into())
            .expect("the error the outcome refuses with is in the command's slice");
        assert_eq!(path.len(), 2, "through the outcome: {path:?}");
    }

    #[test]
    fn merging_two_graphs_can_only_ever_reach_more() {
        // The property the union of a `before` and an `after` graph rests on. The second graph
        // holds an edge the first does not, and the merge must not lose either.
        let mut second = SemanticDependencyGraph {
            nodes: BTreeSet::new(),
            dependents: BTreeMap::new(),
        };
        second.edge(
            event("a.b.Refunded"),
            DependencyRelation::FieldType,
            declared("a.b.Money"),
        );

        let merged = chain().merged(&second);
        let reach = merged.closure(&declared("a.b.Currency").into());

        assert!(
            reach.reaches(&event("a.b.Paid").into()),
            "the first graph's edge survived"
        );
        assert!(
            reach.reaches(&event("a.b.Refunded").into()),
            "the second graph's edge arrived"
        );
        assert_eq!(merged.len(), chain().len() + 1);
    }
}
