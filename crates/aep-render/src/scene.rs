//! One laid-out picture, and the four emitters all draw *this*.
//!
//! # Why there is a scene at all, and not three renderers
//!
//! Decision 2 of the renderer plan. A [`Scene`] is a workflow plus an optional [`RunView`], already
//! resolved into boxes with coordinates, arrows with polylines and text that has been decided on —
//! so [`crate::svg`], [`crate::html`] and [`crate::ansi`] each hold one question, *how do I write
//! this out*, and none of them holds *what does blocked mean*. Live mode falls out of that shape:
//! watching a run advance is rebuilding the `RunView`, rebuilding the scene and emitting again.
//! There is no per-backend state to invalidate because there is no per-backend state.
//!
//! # Coordinates are integers
//!
//! Every position in this module is an `i32`, not an `f32`. A workflow diagram needs no sub-pixel
//! precision, and byte-identical output twice is an acceptance criterion — so the arithmetic that
//! produces the numbers in the SVG is arithmetic with one answer, rather than floating-point
//! formatting that has to be trusted to round the same way.
//!
//! # The layout in one paragraph
//!
//! States run **down** the page, one layer per row, because a guard reads as a sentence and a
//! sentence wants horizontal room: `(tests.unit.failed == 0 and tests.contract.failed == 0 and
//! static_analysis.errors == 0)` sits beside its arrow instead of being wrapped into a matchbox.
//! Retreats — the edges that run back up — are routed through a gutter on the **left**, and forward
//! edges that skip a layer through a gutter on the **right**, so neither crosses a box.

use aep_domain::ids::StateId;
use aep_domain::predicate::Predicate;
use aep_domain::requirement::RequirementSet;
use aep_domain::workflow::{Transition, Workflow};

use crate::layout::Layout;
use crate::run::{RunStatus, RunView};

/// Space between the drawing and the edge of the canvas.
const MARGIN: i32 = 28;

/// How tall the header block is: title, reference, and the run line when there is a run.
const HEADER: i32 = 74;

/// One extra line of header, for the run status.
const HEADER_RUN: i32 = 22;

/// How wide a state box is.
const NODE_W: i32 = 260;

/// How tall a state box is.
const NODE_H: i32 = 56;

/// Horizontal space between two boxes in the same layer.
const COL_GAP: i32 = 40;

/// Vertical space between one layer's boxes and the next layer's.
const ROW_GAP: i32 = 52;

/// How far apart two routed edges run in a gutter.
const LANE_GAP: i32 = 14;

/// How much clear space a gutter keeps between its outermost lane and the boxes.
const GUTTER_PAD: i32 = 18;

/// One line of text in the footer.
const LINE_H: i32 = 18;

/// The advance of one monospace character at the body size.
///
/// An estimate, and only ever used to decide how wide the canvas has to be — never to position
/// anything. A canvas 20 pixels too wide is invisible; a guard clipped at the right edge is a lie
/// about what gates a transition, so the estimate rounds up.
const CHAR_W: i32 = 7;

/// The smallest canvas this crate emits, matching the published house figures.
const MIN_WIDTH: i32 = 960;

/// A point on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    /// Distance from the left edge.
    pub x: i32,
    /// Distance from the top edge.
    pub y: i32,
}

/// A box on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl Rect {
    /// The horizontal centre.
    pub fn centre_x(self) -> i32 {
        self.x + self.w / 2
    }

    /// The vertical centre.
    pub fn centre_y(self) -> i32 {
        self.y + self.h / 2
    }

    /// The bottom edge.
    pub fn bottom(self) -> i32 {
        self.y + self.h
    }

    /// The right edge.
    pub fn right(self) -> i32 {
        self.x + self.w
    }
}

/// What the overlay says about one state.
///
/// The order is the order of precedence when more than one could apply: a terminal state a
/// completed run is sitting in is [`NodeAccent::Done`], not [`NodeAccent::Current`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeAccent {
    /// The run has not been here. Also every box when there is no run at all.
    Idle,
    /// The run has been here and moved on.
    Visited,
    /// Where the run is.
    Current,
    /// Where the run is, and nothing can move it — blocked, out of budget, or reading a broken
    /// document. Drawn in red, with the reasons under the diagram.
    Stopped,
    /// Where the run is, and the workflow is over.
    Done,
}

/// What the overlay says about one transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeAccent {
    /// Never taken by this run — or there is no run.
    Idle,
    /// The run moved along this edge.
    Taken,
    /// The run moved *back* along this edge. Amber, because a retreat is a fact about the work
    /// worth seeing at a glance: verification failed and the change went back to be redone.
    Retreat,
}

/// One state, laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Which state this is.
    pub id: StateId,
    /// Its human title.
    pub title: String,
    /// What happens here, when the document says.
    pub summary: Option<String>,
    /// The phases it declares, in id order.
    pub phases: Vec<String>,
    /// What must hold to enter it, one line per requirement.
    pub requires: Vec<String>,
    /// Whether the workflow ends here.
    pub terminal: bool,
    /// Whether work done here cannot be undone.
    pub irreversible: bool,
    /// What the overlay says about it.
    pub accent: NodeAccent,
    /// How many times the run entered it, when the caller knew.
    pub visits: Option<u32>,
    /// Its row.
    pub layer: usize,
    /// Its column inside the row.
    pub column: usize,
    /// Where its box is.
    pub frame: Rect,
}

/// One transition, laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Where it starts.
    pub from: StateId,
    /// Where it goes.
    pub to: StateId,
    /// Its guard in one line, or `None` when it is unconditional.
    ///
    /// `None` rather than the word `always`: an arrow with no label already says *nothing gates
    /// this*, and writing it out would give an unguarded edge more ink than a guarded one.
    pub guard: Option<String>,
    /// What the document says the transition means.
    pub description: Option<String>,
    /// Structured requirements on the transition, one line each.
    pub requires: Vec<String>,
    /// Whether it runs back up the page.
    pub back: bool,
    /// What the overlay says about it.
    pub accent: EdgeAccent,
    /// How many times the run took it.
    pub taken: usize,
    /// The polyline to draw, source end first.
    pub points: Vec<Point>,
    /// Where the guard label goes.
    pub label: Point,
    /// What to write there, or `None` when there is nothing to say.
    ///
    /// Normally the guard. For an edge routed through a gutter it is the guard **prefixed with the
    /// pair it joins**, because a label sitting in a column beside a lane on the far side of the
    /// diagram is a label nobody can attribute to an arrow: `verify → implement · (…)` says which
    /// retreat it gates and the bare predicate does not.
    pub label_text: Option<String>,
}

/// A workflow, and a run over it, laid out and ready to write out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    /// The workflow's human title.
    pub title: String,
    /// Its reference, as `<id>/<major>`.
    pub reference: String,
    /// What it is for, when the document says.
    pub summary: Option<String>,
    /// Its states, in layer then column order.
    pub nodes: Vec<Node>,
    /// Its transitions, in declaration order.
    pub edges: Vec<Edge>,
    /// The run being drawn over it, when there is one.
    pub run: Option<RunView>,
    /// Evidence the run produced, by kind, in kind order.
    pub evidence: Vec<(String, u32)>,
    /// Why the run stopped — the engine's own sentences, unedited.
    pub reasons: Vec<String>,
    /// How wide the canvas is.
    pub width: i32,
    /// How tall it is.
    pub height: i32,
    /// Where the diagram ends and the footer begins.
    pub footer_y: i32,
}

impl Scene {
    /// Lays a workflow out, with an optional run drawn over it.
    ///
    /// Passing `None` is the bare topology: every box idle, every arrow dim. That is what
    /// `protocol workflow render` without `--run` or `--state` emits, and it is a document about
    /// the workflow rather than about any work.
    pub fn build(workflow: &Workflow, run: Option<&RunView>) -> Self {
        let layout = Layout::of(workflow);
        let gutters = Gutters::of(workflow, &layout);
        let grid = Grid::of(&layout, &gutters, run.is_some());

        let nodes = laid_out_nodes(workflow, &layout, &grid, run);
        let edges = laid_out_edges(workflow, &layout, &gutters, &grid, run);

        let evidence: Vec<(String, u32)> = run
            .map(|view| {
                view.evidence
                    .iter()
                    .map(|(kind, count)| (kind.clone(), *count))
                    .collect()
            })
            .unwrap_or_default();
        let reasons: Vec<String> = run.map(|view| view.reasons.clone()).unwrap_or_default();

        let diagram_bottom =
            grid.header + units(layout.depth().max(1)) * (NODE_H + ROW_GAP) - ROW_GAP;
        let footer_y = diagram_bottom + 34;
        let mut height = footer_y;
        if !evidence.is_empty() {
            height += LINE_H;
        }
        if !reasons.is_empty() {
            height += LINE_H + units(reasons.len()) * LINE_H;
        }
        height += MARGIN;

        Self {
            title: workflow.title.clone(),
            reference: format!("{}/{}", workflow.id, workflow.version.get()),
            summary: workflow.summary.clone(),
            width: canvas_width(&grid, &gutters, &nodes, &edges, &reasons),
            height,
            footer_y,
            nodes,
            edges,
            run: run.cloned(),
            evidence,
            reasons,
        }
    }

    /// The node for `state`, if the workflow declares it.
    pub fn node(&self, state: &StateId) -> Option<&Node> {
        self.nodes.iter().find(|node| &node.id == state)
    }

    /// The run's status, or [`RunStatus::Unknown`] when nothing is drawn over the workflow.
    pub fn status(&self) -> RunStatus {
        self.run
            .as_ref()
            .map_or(RunStatus::Unknown, |run| run.status)
    }

    /// A one-line description of the run, for a header or a footer.
    ///
    /// `None` when there is no run: a bare workflow diagram has nothing to say about work.
    pub fn run_line(&self) -> Option<String> {
        use std::fmt::Write as _;

        let run = self.run.as_ref()?;
        let mut line = match (&run.run, &run.task) {
            (Some(id), _) => format!("run {id}"),
            (None, Some(task)) => format!("task {task}"),
            (None, None) => "run".to_owned(),
        };
        if let Some(current) = &run.current {
            let _ = write!(line, " · in {current}");
        }
        let _ = write!(line, " · {}", run.status);
        if let Some(iterations) = run.iterations {
            let _ = write!(line, " · {iterations} iteration(s)");
        }
        Some(line)
    }
}

/// A count as a coordinate unit.
///
/// Saturating rather than `as`. A workflow with more than two billion states is not a diagram, but
/// `as` would silently wrap that case into a *negative* coordinate and draw something absurd rather
/// than something large — and saying which of the two happens costs one line.
pub(crate) fn units(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// The edges that cannot be drawn as a straight arrow to the next row, and where each one runs.
///
/// Two kinds go into a gutter: a **retreat**, which runs back up the page, and a **skip**, which
/// runs forwards past a layer. Retreats take the left gutter and skips the right, so the two never
/// share a lane and a reader can tell one from the other by which side of the diagram it is on.
/// Longest span outermost inside each gutter, so a long edge never crosses a short one.
struct Gutters<'a> {
    /// Retreats, outermost first.
    left: Vec<(&'a StateId, &'a StateId)>,
    /// Forward edges that skip a layer, outermost first.
    right: Vec<(&'a StateId, &'a StateId)>,
}

impl<'a> Gutters<'a> {
    /// Works out which of a workflow's transitions have to be routed.
    fn of(workflow: &'a Workflow, layout: &Layout) -> Self {
        let mut left: Vec<(&'a StateId, &'a StateId)> = Vec::new();
        let mut right: Vec<(&'a StateId, &'a StateId)> = Vec::new();
        let mut seen: Vec<(&StateId, &StateId)> = Vec::new();
        for transition in &workflow.transitions {
            let pair = (&transition.from, &transition.to);
            // Two transitions between one pair are one lane's worth of routing and two arrows'
            // worth of drawing.
            if seen.contains(&pair) {
                continue;
            }
            seen.push(pair);
            if layout.is_back_edge(&transition.from, &transition.to) {
                left.push(pair);
            } else if span(layout, &transition.from, &transition.to) > 1 {
                right.push(pair);
            }
        }
        let by_span = |lanes: &mut Vec<(&'a StateId, &'a StateId)>| {
            lanes.sort_by_key(|(from, to)| {
                (
                    std::cmp::Reverse(span(layout, from, to)),
                    (*from).clone(),
                    (*to).clone(),
                )
            });
        };
        by_span(&mut left);
        by_span(&mut right);
        Self { left, right }
    }

    /// Which lane a transition runs in, and whether that lane is in the left gutter.
    fn lane_of(&self, from: &StateId, to: &StateId) -> Option<(usize, bool)> {
        let at = |lanes: &[(&StateId, &StateId)]| {
            lanes
                .iter()
                .position(|(source, target)| *source == from && *target == to)
        };
        at(&self.left)
            .map(|index| (index, true))
            .or_else(|| at(&self.right).map(|index| (index, false)))
    }

    /// How wide the left gutter is.
    fn left_width(&self) -> i32 {
        gutter(self.left.len())
    }

    /// How wide the right gutter is.
    fn right_width(&self) -> i32 {
        gutter(self.right.len())
    }
}

/// How wide a gutter holding `lanes` routed edges has to be.
fn gutter(lanes: usize) -> i32 {
    if lanes == 0 {
        0
    } else {
        GUTTER_PAD + units(lanes) * LANE_GAP
    }
}

/// Where the boxes sit on the canvas, once the gutters have claimed their space.
struct Grid {
    /// The left edge of the first column of boxes.
    nodes_left: i32,
    /// The right edge of the last column of boxes.
    nodes_right: i32,
    /// The top edge of the first row of boxes — everything above it is the title block.
    header: i32,
    /// Where a routed edge's label goes: clear of the boxes and of the right gutter.
    lane_label_x: i32,
}

impl Grid {
    /// Works the geometry out from the layering and the gutters.
    fn of(layout: &Layout, gutters: &Gutters<'_>, with_run: bool) -> Self {
        let nodes_left = MARGIN + gutters.left_width();
        let columns = units(layout.width().max(1));
        let nodes_right = nodes_left + columns * NODE_W + (columns - 1) * COL_GAP;
        Self {
            nodes_left,
            nodes_right,
            header: HEADER + if with_run { HEADER_RUN } else { 0 },
            lane_label_x: nodes_right + gutters.right_width() + 14,
        }
    }

    /// Where one state's box goes.
    fn frame(&self, layout: &Layout, state: &StateId) -> Rect {
        let layer = units(layout.layer_of(state).unwrap_or(0));
        let column = units(layout.column_of(state).unwrap_or(0));
        Rect {
            x: self.nodes_left + column * (NODE_W + COL_GAP),
            y: self.header + layer * (NODE_H + ROW_GAP),
            w: NODE_W,
            h: NODE_H,
        }
    }
}

/// Every state as a laid-out node, in layer then column order.
fn laid_out_nodes(
    workflow: &Workflow,
    layout: &Layout,
    grid: &Grid,
    run: Option<&RunView>,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(workflow.states.len());
    for layer in 0..layout.depth() {
        for state in layout.row(layer) {
            let Some(declared) = workflow.state(state) else {
                continue;
            };
            nodes.push(Node {
                id: state.clone(),
                title: declared.title.clone(),
                summary: declared.summary.clone(),
                phases: declared.phases.iter().map(ToString::to_string).collect(),
                requires: requirement_lines(&declared.requires),
                terminal: declared.is_terminal(),
                irreversible: declared.irreversible,
                accent: accent_of(state, declared.is_terminal(), run),
                visits: run.and_then(|view| view.visits_of(state)),
                layer,
                column: layout.column_of(state).unwrap_or(0),
                frame: grid.frame(layout, state),
            });
        }
    }
    nodes
}

/// Every transition as a laid-out edge, in declaration order.
fn laid_out_edges(
    workflow: &Workflow,
    layout: &Layout,
    gutters: &Gutters<'_>,
    grid: &Grid,
    run: Option<&RunView>,
) -> Vec<Edge> {
    let mut edges = Vec::with_capacity(workflow.transitions.len());
    for transition in &workflow.transitions {
        let from = grid.frame(layout, &transition.from);
        let to = grid.frame(layout, &transition.to);
        let back = layout.is_back_edge(&transition.from, &transition.to);
        let lane = gutters.lane_of(&transition.from, &transition.to);
        let (points, label) = route(from, to, lane, gutters, grid);
        let taken = run.map_or(0, |view| view.times_taken(&transition.from, &transition.to));
        let guard = guard_of(transition);
        let label_text = match (lane.is_some(), &guard) {
            (true, Some(text)) => Some(format!("{} → {} · {text}", transition.from, transition.to)),
            (true, None) => Some(format!("{} → {}", transition.from, transition.to)),
            (false, other) => other.clone(),
        };
        edges.push(Edge {
            from: transition.from.clone(),
            to: transition.to.clone(),
            guard,
            description: transition.description.clone(),
            requires: requirement_lines(&transition.requires),
            back,
            accent: match (taken, back) {
                (0, _) => EdgeAccent::Idle,
                (_, true) => EdgeAccent::Retreat,
                (_, false) => EdgeAccent::Taken,
            },
            taken,
            points,
            label,
            label_text,
        });
    }
    edges
}

/// The polyline one edge runs along, and where its label sits.
///
/// Three shapes, and no others: straight down a column, an elbow across to another column, and out
/// into a gutter and back. Curves were considered and refused — a workflow of nine states needs no
/// bezier, and a straight line is a line whose endpoints a reader can find.
fn route(
    from: Rect,
    to: Rect,
    lane: Option<(usize, bool)>,
    gutters: &Gutters<'_>,
    grid: &Grid,
) -> (Vec<Point>, Point) {
    if let Some((index, on_left)) = lane {
        let x = if on_left {
            MARGIN + 8 + units(index) * LANE_GAP
        } else {
            grid.nodes_right
                + GUTTER_PAD
                + (units(gutters.right.len()) - 1 - units(index)) * LANE_GAP
        };
        let exit = if on_left { from.x } else { from.right() };
        let entry = if on_left { to.x } else { to.right() };
        return (
            vec![
                Point {
                    x: exit,
                    y: from.centre_y(),
                },
                Point {
                    x,
                    y: from.centre_y(),
                },
                Point {
                    x,
                    y: to.centre_y(),
                },
                Point {
                    x: entry,
                    y: to.centre_y(),
                },
            ],
            Point {
                x: grid.lane_label_x,
                y: (from.centre_y() + to.centre_y()) / 2 + 4,
            },
        );
    }
    let mid = (from.bottom() + to.y) / 2;
    if from.centre_x() == to.centre_x() {
        return (
            vec![
                Point {
                    x: from.centre_x(),
                    y: from.bottom(),
                },
                Point {
                    x: to.centre_x(),
                    y: to.y,
                },
            ],
            Point {
                x: from.centre_x() + 12,
                y: mid + 4,
            },
        );
    }
    (
        vec![
            Point {
                x: from.centre_x(),
                y: from.bottom(),
            },
            Point {
                x: from.centre_x(),
                y: mid,
            },
            Point {
                x: to.centre_x(),
                y: mid,
            },
            Point {
                x: to.centre_x(),
                y: to.y,
            },
        ],
        Point {
            x: from.centre_x().max(to.centre_x()) + 12,
            y: mid - 6,
        },
    )
}

/// How wide the canvas has to be for nothing to be clipped.
///
/// Text is measured with [`CHAR_W`], an over-estimate: a canvas twenty pixels too wide is
/// invisible, and a guard cut off at the right edge is a lie about what gates a transition.
fn canvas_width(
    grid: &Grid,
    gutters: &Gutters<'_>,
    nodes: &[Node],
    edges: &[Edge],
    reasons: &[String],
) -> i32 {
    let mut width = grid.nodes_right + gutters.right_width() + MARGIN;
    for node in nodes {
        for line in &node.requires {
            // The word `requires ` plus the line, written to the right of the box.
            let text = units(9 + line.chars().count());
            width = width.max(node.frame.right() + 14 + text * CHAR_W + MARGIN);
        }
    }
    for edge in edges {
        let Some(text) = &edge.label_text else {
            continue;
        };
        width = width.max(edge.label.x + units(text.chars().count()) * CHAR_W + MARGIN);
    }
    for reason in reasons {
        width = width.max(MARGIN + 16 + units(reason.chars().count()) * CHAR_W + MARGIN);
    }
    width.max(MIN_WIDTH)
}

/// How many layers a transition crosses.
fn span(layout: &Layout, from: &StateId, to: &StateId) -> usize {
    let source = layout.layer_of(from).unwrap_or(0);
    let target = layout.layer_of(to).unwrap_or(0);
    target.abs_diff(source)
}

/// The guard of a transition as one line, or `None` when nothing gates it.
fn guard_of(transition: &Transition) -> Option<String> {
    match &transition.when {
        Predicate::Always => None,
        other => Some(other.to_string()),
    }
}

/// A requirement set as one line per requirement, in a fixed order.
///
/// The order is the declaration order of the fields, not any evaluated ordering: this is a
/// description of the document, and a renderer that sorted requirements by whether they hold would
/// be evaluating a workflow it has no engine to evaluate with.
fn requirement_lines(requires: &RequirementSet) -> Vec<String> {
    let mut lines = Vec::new();
    for predicate in &requires.predicates {
        lines.push(predicate.to_string());
    }
    for evidence in &requires.evidence {
        lines.push(evidence.to_string());
    }
    for artifact in &requires.artifacts {
        lines.push(artifact.to_string());
    }
    for review in &requires.reviews {
        lines.push(review.to_string());
    }
    for approval in &requires.approvals {
        lines.push(approval.to_string());
    }
    for conditional in &requires.conditional {
        lines.push(format!("if {} then …", conditional.when));
    }
    lines
}

/// What the overlay says about one state.
fn accent_of(state: &StateId, terminal: bool, run: Option<&RunView>) -> NodeAccent {
    let Some(view) = run else {
        return NodeAccent::Idle;
    };
    if view.current.as_ref() == Some(state) {
        if view.status == RunStatus::Completed && terminal {
            return NodeAccent::Done;
        }
        if view.status.is_stopped() {
            return NodeAccent::Stopped;
        }
        return NodeAccent::Current;
    }
    if view.has_visited(state) {
        NodeAccent::Visited
    } else {
        NodeAccent::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fixture_workflow, mid_run, state};

    #[test]
    fn a_bare_workflow_marks_nothing_and_takes_no_edge() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, None);
        assert_eq!(scene.nodes.len(), workflow.states.len());
        assert!(
            scene
                .nodes
                .iter()
                .all(|node| node.accent == NodeAccent::Idle),
            "with no run, nothing is current and nothing is visited"
        );
        assert!(scene
            .edges
            .iter()
            .all(|edge| edge.accent == EdgeAccent::Idle));
        assert!(scene.reasons.is_empty());
        assert_eq!(scene.run_line(), None);
    }

    #[test]
    fn the_mid_run_overlay_marks_current_visited_and_the_retreat() {
        let workflow = fixture_workflow();
        let run = mid_run();
        // The fixture has to have gone back, or the amber rule is untested.
        assert_eq!(
            run.times_taken(&state("verify"), &state("implement")),
            1,
            "the fixture must have taken the retreat"
        );
        let scene = Scene::build(&workflow, Some(&run));

        assert_eq!(
            scene.node(&state("implement")).expect("laid out").accent,
            NodeAccent::Stopped,
            "the run is blocked in `implement`, so its box is the red one"
        );
        assert_eq!(
            scene.node(&state("verify")).expect("laid out").accent,
            NodeAccent::Visited,
            "verify was entered and left behind"
        );
        assert_eq!(
            scene.node(&state("review")).expect("laid out").accent,
            NodeAccent::Idle,
            "review was never reached"
        );

        let retreat = scene
            .edges
            .iter()
            .find(|edge| edge.from == state("verify") && edge.to == state("implement"))
            .expect("the retreat is an edge of the scene");
        assert_eq!(retreat.accent, EdgeAccent::Retreat);
        assert!(retreat.back, "it runs back up the page");
        assert_eq!(retreat.taken, 1);

        let forward = scene
            .edges
            .iter()
            .find(|edge| edge.from == state("implement") && edge.to == state("verify"))
            .expect("the forward edge is an edge of the scene");
        assert_eq!(
            forward.accent,
            EdgeAccent::Taken,
            "a taken forward edge is not a retreat"
        );

        assert_eq!(scene.reasons, run.reasons, "reasons travel verbatim");
        assert_eq!(scene.evidence.len(), 2);
    }

    #[test]
    fn a_completed_run_in_a_terminal_state_is_done_not_current() {
        let workflow = fixture_workflow();
        let mut run = mid_run();
        run.status = RunStatus::Completed;
        run.current = Some(state("complete"));
        run.path.push(state("complete"));
        let scene = Scene::build(&workflow, Some(&run));
        assert!(
            workflow
                .state(&state("complete"))
                .expect("declared")
                .is_terminal(),
            "the fixture's `complete` must actually be terminal"
        );
        assert_eq!(
            scene.node(&state("complete")).expect("laid out").accent,
            NodeAccent::Done
        );
    }

    #[test]
    fn an_unconditional_transition_carries_no_guard_label() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, None);
        let intake = scene
            .edges
            .iter()
            .find(|edge| edge.from == state("receive") && edge.to == state("specify"))
            .expect("the intake edge exists");
        assert_eq!(
            intake.guard, None,
            "an unguarded arrow says `nothing gates this` by having no label"
        );
        let gated = scene
            .edges
            .iter()
            .find(|edge| edge.from == state("specify") && edge.to == state("decompose"))
            .expect("the specify edge exists");
        assert_eq!(
            gated.guard.as_deref(),
            Some("artifact.specification.exists")
        );
    }

    #[test]
    fn the_canvas_grows_to_fit_the_longest_guard_rather_than_clipping_it() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, None);
        for edge in &scene.edges {
            let Some(text) = &edge.label_text else {
                continue;
            };
            let right = edge.label.x + units(text.chars().count()) * CHAR_W;
            assert!(
                right <= scene.width,
                "`{text}` would be clipped at x={right} on a {}px canvas",
                scene.width
            );
        }
        assert!(scene.width >= MIN_WIDTH);
    }

    #[test]
    fn a_state_that_requires_independent_evidence_says_so_on_its_box() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, None);
        let adversarial = scene.node(&state("adversarial_verify")).expect("laid out");
        assert!(
            adversarial
                .requires
                .iter()
                .any(|line| line.contains("(independent)")),
            "independence is the point of that state and has to be visible: {:?}",
            adversarial.requires
        );
    }
}
