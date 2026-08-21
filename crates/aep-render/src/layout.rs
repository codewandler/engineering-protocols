//! Where each state goes: layers down the page, columns across it.
//!
//! # Hand-rolled, and the refusal is the point
//!
//! Decision 3 of the renderer plan: **no `graphviz`, and no layout crate.** A workflow in this
//! repository has nine states and ten transitions; the four committed ones are chains with a single
//! retreat. Shelling out to `dot` would put a system package between `protocol workflow render` and
//! a picture — the same argument that keeps the gate off the network — and a layered-graph crate
//! would buy an ordering heuristic for a graph small enough to have no crossings to remove. What
//! this module is instead is sixty lines of longest-path layering, and it is enough because the
//! input is small by construction: a workflow nobody can read is a workflow nobody should have
//! written.
//!
//! # Determinism is the acceptance criterion, not a nice property
//!
//! `protocol workflow render --format svg` has to produce the same bytes twice, because a figure
//! that is committed and then re-rendered must not show up in a diff for having chosen a different
//! iteration order. Every map here is a [`BTreeMap`], every set a [`BTreeSet`], and every tie is
//! broken by an explicit rule — discovery order first, then the state id — rather than by whatever
//! order a traversal happened to produce. `tests/determinism.rs` scans this crate for the tokens
//! that would break it.
//!
//! # The algorithm
//!
//! 1. Depth-first from the initial state, visiting successors in id order, to classify the edges
//!    that close a cycle. Those are the retreats — `verify → implement` — and layering has to
//!    ignore them or there is no topological order at all.
//! 2. Longest-path layering over what is left: a state sits one layer below its deepest
//!    predecessor. Longest path rather than shortest, so `implement` lands after
//!    `establish_verifiers` even though nothing forces the reader's eye there.
//! 3. Columns within a layer, in discovery order, so a branch that opens first is drawn leftmost.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use aep_domain::ids::StateId;
use aep_domain::workflow::Workflow;

/// Which layer and column every state of a workflow occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The layer each state sits in, counted from the initial state's zero.
    layers: BTreeMap<StateId, usize>,
    /// The column each state occupies inside its layer, counted from zero on the left.
    columns: BTreeMap<StateId, usize>,
    /// How many layers there are.
    depth: usize,
    /// How many columns the widest layer needs.
    width: usize,
}

impl Layout {
    /// Lays a workflow out.
    pub fn of(workflow: &Workflow) -> Self {
        let successors = successors_of(workflow);
        let (order, cycle_closing) = classify(workflow, &successors);
        let layers = layer(workflow, &successors, &cycle_closing);
        let columns = columns(&layers, &order);
        let depth = layers.values().copied().max().map_or(0, |top| top + 1);
        let width = columns.values().copied().max().map_or(0, |last| last + 1);
        Self {
            layers,
            columns,
            depth,
            width,
        }
    }

    /// Which layer `state` is in, or `None` when the workflow does not declare it.
    pub fn layer_of(&self, state: &StateId) -> Option<usize> {
        self.layers.get(state).copied()
    }

    /// Which column `state` occupies inside its layer.
    pub fn column_of(&self, state: &StateId) -> Option<usize> {
        self.columns.get(state).copied()
    }

    /// How many layers the workflow has.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// How many columns the widest layer needs.
    pub fn width(&self) -> usize {
        self.width
    }

    /// `true` when a transition from `from` to `to` runs back up the page.
    ///
    /// Computed from the finished layering rather than from the traversal, so a self-loop and a
    /// sideways edge inside one layer both count — a drawing has to route either of them around the
    /// boxes, and neither can be drawn as a straight arrow to the next row.
    pub fn is_back_edge(&self, from: &StateId, to: &StateId) -> bool {
        match (self.layer_of(from), self.layer_of(to)) {
            (Some(source), Some(target)) => target <= source,
            _ => false,
        }
    }

    /// The states of one layer, left to right.
    pub fn row(&self, layer: usize) -> Vec<&StateId> {
        let mut row: Vec<&StateId> = self
            .layers
            .iter()
            .filter(|(_, at)| **at == layer)
            .map(|(state, _)| state)
            .collect();
        row.sort_by_key(|state| self.column_of(state).unwrap_or(0));
        row
    }
}

/// Every state's successors, deduplicated and in id order.
///
/// Two transitions between the same pair — one guarded, one not — are one arrow's worth of layout
/// and two arrows' worth of drawing, so the layout collapses them and [`crate::scene`] does not.
fn successors_of(workflow: &Workflow) -> BTreeMap<StateId, BTreeSet<StateId>> {
    let mut successors: BTreeMap<StateId, BTreeSet<StateId>> = workflow
        .states
        .keys()
        .map(|state| (state.clone(), BTreeSet::new()))
        .collect();
    for transition in &workflow.transitions {
        if let Some(targets) = successors.get_mut(&transition.from) {
            targets.insert(transition.to.clone());
        }
    }
    successors
}

/// Depth-first from the initial state: the order states are discovered, and the edges that close a
/// cycle.
///
/// Iterative rather than recursive, because a recursive walk over an authored document is a stack
/// depth an author controls. States the initial one cannot reach are appended afterwards in id
/// order: validation refuses those, so reaching this path means somebody built a `Workflow` by hand
/// — and drawing it badly is better than drawing nothing.
fn classify(
    workflow: &Workflow,
    successors: &BTreeMap<StateId, BTreeSet<StateId>>,
) -> (BTreeMap<StateId, usize>, BTreeSet<(StateId, StateId)>) {
    let mut order: BTreeMap<StateId, usize> = BTreeMap::new();
    let mut cycle_closing: BTreeSet<(StateId, StateId)> = BTreeSet::new();
    let mut next = 0_usize;

    let mut roots = vec![workflow.initial.clone()];
    roots.extend(workflow.states.keys().cloned());

    // `open` is the DFS stack as a set: an edge into one of these is an edge that closes a cycle.
    let mut open: BTreeSet<StateId> = BTreeSet::new();
    let mut stack: Vec<(StateId, Vec<StateId>)> = Vec::new();

    for root in roots {
        if order.contains_key(&root) || !workflow.states.contains_key(&root) {
            continue;
        }
        order.insert(root.clone(), next);
        next += 1;
        open.insert(root.clone());
        stack.push((
            root.clone(),
            successors
                .get(&root)
                .map(|targets| targets.iter().rev().cloned().collect())
                .unwrap_or_default(),
        ));

        while let Some((state, pending)) = stack.last_mut() {
            let Some(target) = pending.pop() else {
                let finished = state.clone();
                open.remove(&finished);
                stack.pop();
                continue;
            };
            let from = state.clone();
            if open.contains(&target) {
                cycle_closing.insert((from, target));
                continue;
            }
            if order.contains_key(&target) {
                continue;
            }
            order.insert(target.clone(), next);
            next += 1;
            open.insert(target.clone());
            let children = successors
                .get(&target)
                .map(|targets| targets.iter().rev().cloned().collect())
                .unwrap_or_default();
            stack.push((target, children));
        }
    }

    (order, cycle_closing)
}

/// Longest-path layering over the edges that do not close a cycle.
///
/// Kahn's algorithm with a queue seeded in id order, so the relaxation visits predecessors before
/// successors and no state is placed twice. A state with no forward predecessor — the initial one,
/// and anything a hand-built workflow left dangling — sits at layer zero.
fn layer(
    workflow: &Workflow,
    successors: &BTreeMap<StateId, BTreeSet<StateId>>,
    cycle_closing: &BTreeSet<(StateId, StateId)>,
) -> BTreeMap<StateId, usize> {
    let mut incoming: BTreeMap<StateId, usize> = workflow
        .states
        .keys()
        .map(|state| (state.clone(), 0))
        .collect();
    for (from, targets) in successors {
        for to in targets {
            if cycle_closing.contains(&(from.clone(), to.clone())) {
                continue;
            }
            if let Some(count) = incoming.get_mut(to) {
                *count += 1;
            }
        }
    }

    let mut layers: BTreeMap<StateId, usize> = workflow
        .states
        .keys()
        .map(|state| (state.clone(), 0))
        .collect();
    let mut queue: VecDeque<StateId> = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(state, _)| state.clone())
        .collect();

    while let Some(state) = queue.pop_front() {
        let here = layers.get(&state).copied().unwrap_or(0);
        let Some(targets) = successors.get(&state) else {
            continue;
        };
        for target in targets {
            if cycle_closing.contains(&(state.clone(), target.clone())) {
                continue;
            }
            if let Some(depth) = layers.get_mut(target) {
                *depth = (*depth).max(here + 1);
            }
            if let Some(count) = incoming.get_mut(target) {
                *count -= 1;
                if *count == 0 {
                    queue.push_back(target.clone());
                }
            }
        }
    }

    layers
}

/// Columns inside each layer, in discovery order and then by id.
fn columns(
    layers: &BTreeMap<StateId, usize>,
    order: &BTreeMap<StateId, usize>,
) -> BTreeMap<StateId, usize> {
    let mut rows: BTreeMap<usize, Vec<&StateId>> = BTreeMap::new();
    for (state, layer) in layers {
        rows.entry(*layer).or_default().push(state);
    }
    let mut columns = BTreeMap::new();
    for row in rows.values_mut() {
        row.sort_by_key(|state| (order.get(*state).copied().unwrap_or(usize::MAX), *state));
        for (column, state) in row.iter().enumerate() {
            columns.insert((*state).clone(), column);
        }
    }
    columns
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fixture_workflow, state};

    #[test]
    fn the_back_edge_does_not_push_implement_below_verify() {
        let workflow = fixture_workflow();
        let layout = Layout::of(&workflow);
        let implement = layout.layer_of(&state("implement")).expect("laid out");
        let verify = layout.layer_of(&state("verify")).expect("laid out");
        // The fixture has to contain the retreat, or this test proves nothing about it.
        assert!(
            workflow
                .transitions
                .iter()
                .any(|it| it.from == state("verify") && it.to == state("implement")),
            "the fixture must carry the verify -> implement retreat"
        );
        assert_eq!(
            verify,
            implement + 1,
            "verify sits one below implement; the retreat is not a layering edge"
        );
        assert!(layout.is_back_edge(&state("verify"), &state("implement")));
        assert!(!layout.is_back_edge(&state("implement"), &state("verify")));
    }

    #[test]
    fn a_chain_lays_out_one_state_per_layer_in_declaration_order() {
        let workflow = fixture_workflow();
        let layout = Layout::of(&workflow);
        assert_eq!(layout.width(), 1, "the fixture is a chain, so one column");
        assert_eq!(
            layout.depth(),
            workflow.states.len(),
            "a chain of n states makes n layers"
        );
        assert_eq!(layout.layer_of(&workflow.initial), Some(0));
        for layer in 0..layout.depth() {
            assert_eq!(layout.row(layer).len(), 1, "one state in layer {layer}");
        }
    }

    #[test]
    fn longest_path_beats_shortest_when_a_state_can_be_reached_two_ways() {
        // `a -> b -> c` and `a -> c`. Shortest path would put `c` beside `b`; longest path puts it
        // below, which is what keeps an arrow from pointing sideways into its own row.
        let workflow = crate::testing::workflow_with(
            &["a", "b", "c"],
            &[("a", "b"), ("b", "c"), ("a", "c")],
            "c",
        );
        let layout = Layout::of(&workflow);
        assert_eq!(layout.layer_of(&state("a")), Some(0));
        assert_eq!(layout.layer_of(&state("b")), Some(1));
        assert_eq!(
            layout.layer_of(&state("c")),
            Some(2),
            "c goes below its deepest predecessor, not its shallowest"
        );
        assert!(!layout.is_back_edge(&state("a"), &state("c")));
    }

    #[test]
    fn two_states_that_can_only_be_reached_in_parallel_share_a_layer() {
        let workflow = crate::testing::workflow_with(
            &["a", "left", "right", "join"],
            &[
                ("a", "left"),
                ("a", "right"),
                ("left", "join"),
                ("right", "join"),
            ],
            "join",
        );
        let layout = Layout::of(&workflow);
        assert_eq!(
            layout.layer_of(&state("left")),
            layout.layer_of(&state("right"))
        );
        assert_eq!(layout.width(), 2, "the branch needs two columns");
        assert_eq!(
            layout.row(1),
            vec![&state("left"), &state("right")],
            "discovery order puts `left` first because successors are walked in id order"
        );
        assert_eq!(layout.layer_of(&state("join")), Some(2));
    }
}
