//! The system as one graph, rendered more than one way.
//!
//! Design §9's picture — an actor invokes a command, a command emits an event, a binding carries an
//! event into the next command, and a component is the box the whole thing sits in — is the single
//! most reproduced artifact this repository has. The documentation page opens with it, and
//! `protocol ess graph` prints it for someone who wants it in a pull request or piped into
//! `dot -Tsvg`.
//!
//! # Why the model is here and not in the caller
//!
//! Because it was in two callers, and they disagreed. `protocol ess graph` built its own graph and
//! rendered it as DOT; [`crate::docs`] walked the IR again and rendered a Mermaid `flowchart`. The
//! two readings were not two renderings of one graph — they were two graphs:
//!
//! | question | what the CLI answered | what the page answered |
//! |---|---|---|
//! | is an actor in the graph? | no | yes, in a group of its own, with its grants as edges |
//! | which component holds a command? | the component that `owns:` its bounded context | the component that `accepts:` it |
//! | which component holds an event? | the owner of its context | every component that `publishes:` it |
//!
//! `ess-domain`'s `component` module makes those different on purpose: a component may accept a
//! command from a domain nobody owns, and may publish an event from a domain it does not own. So
//! the CLI drew a decomposition the specification did not state, and left out the actor entirely —
//! and nothing compared them, which is the same hole `tests/agreement.rs` was written for after
//! three projections carried three copies of one type mapping.
//!
//! The reading kept is the page's, because it is the one a *reader* checks a decomposition against:
//! `accepts:` and `publishes:` are what a component says about itself, and a box drawn from
//! `owns:` claims a surface the document never declared. So [`SystemGraph::of`] is that reading,
//! once, and both renderings project from the value it returns.
//!
//! # What each rendering is allowed to differ in
//!
//! Presentation, and nothing else. [`SystemGraph::mermaid`] writes node identifiers as indices
//! because Mermaid identifiers may not hold a dot; [`SystemGraph::dot`] writes qualified names
//! because DOT quotes them happily and a person reads the DOT. [`SystemGraph::dot`] also puts the
//! delivery guarantee and the failure policy on a binding's edge, which the page states in prose
//! beside its diagram instead. Neither is allowed to differ in *which nodes exist, which edges
//! exist, or which group holds which node* — those come from the value, and
//! `crates/protocol-cli/tests/graph.rs` compares the two published renderings to keep it that way.
//!
//! # Determinism
//!
//! No clock, no RNG, no `HashMap`. Every list here is a `BTreeMap`/`BTreeSet` iteration or a `Vec`
//! in declaration order, so node identifiers are indices into a stable order rather than into
//! iteration chance, and two runs over one specification produce identical bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ess_compiler::ir::EssIr;
use ess_domain::binding::{Delivery, Failure};
use ess_domain::name::QualifiedName;

/// What the group of actors is called on the diagram.
const WHO_MAY_ASK: &str = "who may ask";

/// What the group of commands and events no component claims is called.
const OWNED_BY_NO_COMPONENT: &str = "owned by no component";

/// What a grant is labelled with.
const MAY_INVOKE: &str = "may invoke";

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// Someone or something a command is invoked on behalf of.
    Actor,
    /// A requested state change.
    Command,
    /// A fact that happened.
    Event,
}

impl NodeKind {
    /// The prefix a Mermaid identifier of this kind carries.
    ///
    /// Three prefixes rather than one counter, so a node identifier read out of a rendered diagram
    /// says which kind it is without looking it up.
    fn prefix(self) -> &'static str {
        match self {
            Self::Actor => "who",
            Self::Command => "cmd",
            Self::Event => "evt",
        }
    }

    /// The DOT shape a node of this kind is drawn with.
    ///
    /// A command is a box and an event is an ellipse, so every arrow's direction is readable
    /// without following the labels: a box makes ellipses, an ellipse re-enters a box. An actor is
    /// neither — it is outside the system asking — so it gets a shape of its own rather than
    /// borrowing one.
    fn dot_shape(self) -> &'static str {
        match self {
            Self::Actor => "octagon",
            Self::Command => "box",
            Self::Event => "ellipse",
        }
    }
}

/// What a group of nodes stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    /// The actors, outside every component.
    Actors,
    /// One component's declared surface.
    Component,
    /// What no component claims.
    Unowned,
}

/// What an edge stands for.
///
/// Three kinds, because a reader has to be able to tell a permission from a consequence: a grant is
/// what an actor is allowed to ask for, an emission is what a command produces, and a binding is
/// what carries the second into the next command across a queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// An actor may invoke a command.
    Grant,
    /// A command's outcome emits an event.
    Emission,
    /// A binding carries an event into a command.
    Binding,
}

/// A command, an event or an actor.
///
/// Keyed by [`name`](Self::name) alone, and that is safe rather than lucky: `DomainSpec::validate`
/// refuses a qualified name declared as two kinds, and `DomainSpec::validate_all` refuses one
/// declared by two domains, so a name identifies at most one node in a resolved model.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphNode<'a> {
    /// Which of the three it is.
    pub kind: NodeKind,
    /// Its qualified name.
    pub name: &'a QualifiedName,
    /// The bounded context that declares it.
    pub domain: &'a QualifiedName,
}

/// A box some nodes are drawn inside.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphGroup<'a> {
    /// Which of the three it is.
    pub kind: GroupKind,
    /// What the box is labelled with: a component's name, or the phrase the other two carry.
    pub label: String,
    /// The nodes inside it, in the order the specification declares them.
    ///
    /// A name may appear in two groups — §6 lets a component publish an event another component
    /// publishes too — and that is the model saying two components claim one fact, not a defect in
    /// this list.
    pub members: Vec<&'a QualifiedName>,
}

/// An arrow between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphEdge<'a> {
    /// Which of the three it is.
    pub kind: EdgeKind,
    /// Where it starts.
    pub from: &'a QualifiedName,
    /// Where it ends.
    pub to: &'a QualifiedName,
    /// The word the model puts on it: the outcome that emits, the binding that carries, or
    /// `may invoke`.
    pub label: String,
    /// How many times the command may run. Only a binding has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Delivery>,
    /// What happens when it does not. Only a binding has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<Failure>,
}

/// The graph the interaction layer is about.
///
/// Read once from an [`EssIr`] by [`SystemGraph::of`], and rendered by
/// [`mermaid`](Self::mermaid) and [`dot`](Self::dot). Two decisions worth stating.
/// **Components are groups rather than nodes**: a component is who has to ship the answer, and a
/// graph that made it a node would put "who owns this" on the same footing as "what causes this".
/// **Errors are not nodes**: nothing reacts to one, so an error would be a leaf that cannot
/// participate in the causality this graph exists to show — `protocol ess inspect` is where a
/// command's errors are.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SystemGraph<'a> {
    /// The system this is a graph of.
    pub system: &'a QualifiedName,
    /// The version of the specification it was read from.
    pub version: String,
    /// The boxes, in the order they are drawn.
    pub groups: Vec<GraphGroup<'a>>,
    /// Every node, actors first, then commands, then events, each in name order.
    pub nodes: Vec<GraphNode<'a>>,
    /// Every edge: grants, then emissions, then bindings.
    pub edges: Vec<GraphEdge<'a>>,
}

impl<'a> SystemGraph<'a> {
    /// Reads the graph out of the IR.
    ///
    /// Every iteration here is over a `BTreeMap`, a `BTreeSet` or a declaration-order `Vec`, which
    /// is what makes two runs byte-identical rather than usually byte-identical.
    pub fn of(ir: &'a EssIr) -> Self {
        Self {
            system: &ir.system,
            version: ir.version.to_string(),
            groups: groups(ir),
            nodes: nodes(ir),
            edges: edges(ir),
        }
    }

    /// The graph as a Mermaid `flowchart`, without the Markdown fence around it.
    ///
    /// Without the fence because the two callers need different furniture: a Markdown page wraps it
    /// in ` ```mermaid `, and a CLI writing to a pipe must not, or the first thing anyone does with
    /// the output is delete three characters off each end. The fence is the only difference between
    /// the two, which is what `crates/protocol-cli/tests/graph.rs` normalises and all it normalises.
    pub fn mermaid(&self) -> String {
        let ids = self.mermaid_ids();
        // `TB` rather than `LR`: the page is read in a column, and a wide graph is a horizontal
        // scrollbar on every documentation site there is.
        let mut out = String::from("flowchart TB\n");
        let mut units = 0usize;
        for group in &self.groups {
            let id = match group.kind {
                GroupKind::Actors => "who".to_owned(),
                GroupKind::Component => {
                    let id = format!("unit{units}");
                    units += 1;
                    id
                }
                GroupKind::Unowned => "unowned".to_owned(),
            };
            let _ = writeln!(out, "    subgraph {id}[\"{}\"]", label(&group.label));
            for member in &group.members {
                let _ = writeln!(
                    out,
                    "        {}[\"{}\"]",
                    ids[member],
                    label(&member.to_string())
                );
            }
            out.push_str("    end\n");
        }
        for edge in &self.edges {
            // A binding crosses contexts asynchronously; a grant and an emission do not. Dashing
            // the first is the one visual difference a reader needs in order to see where a queue
            // is.
            let arrow = if edge.kind == EdgeKind::Binding {
                "-.->"
            } else {
                "-->"
            };
            let _ = writeln!(
                out,
                "    {} {arrow}|\"{}\"| {}",
                ids[edge.from],
                label(&edge.label),
                ids[edge.to]
            );
        }
        out
    }

    /// The graph as Graphviz DOT, ready for `dot -Tsvg`.
    pub fn dot(&self) -> String {
        let index = self.by_name();
        let mut out = String::new();
        let _ = writeln!(
            out,
            "// {} {} — {} node(s), {} edge(s)",
            self.system,
            self.version,
            self.nodes.len(),
            self.edges.len()
        );
        let _ = writeln!(out, "digraph {} {{", dot_string(&self.system.to_string()));
        out.push_str("  rankdir=LR;\n");
        out.push_str("  node [fontname=\"sans-serif\"];\n");
        out.push_str("  edge [fontname=\"sans-serif\"];\n");

        for group in &self.groups {
            let _ = writeln!(out, "  subgraph {} {{", dot_string(&group.cluster()));
            let _ = writeln!(out, "    label={};", dot_string(&group.label));
            out.push_str("    style=rounded;\n");
            for member in &group.members {
                let node = index[member];
                let _ = writeln!(
                    out,
                    "    {} [label={}, shape={}];",
                    dot_string(&node.name.to_string()),
                    dot_label(&[node.name.local(), &node.domain.to_string()]),
                    node.kind.dot_shape()
                );
            }
            out.push_str("  }\n");
        }

        for edge in &self.edges {
            let label = match (edge.kind, edge.delivery, edge.on_failure) {
                (EdgeKind::Binding, Some(delivery), Some(failure)) => dot_label(&[
                    &edge.label,
                    &format!("{} / {}", delivery_word(delivery), failure_word(failure)),
                ]),
                (EdgeKind::Emission, _, _) => dot_label(&["emits", &edge.label]),
                _ => dot_label(&[&edge.label]),
            };
            let style = if edge.kind == EdgeKind::Binding {
                ", style=dashed"
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "  {} -> {} [label={label}{style}];",
                dot_string(&edge.from.to_string()),
                dot_string(&edge.to.to_string())
            );
        }

        out.push_str("}\n");
        out
    }

    /// One Mermaid identifier per node, numbered inside its kind.
    ///
    /// An index rather than the qualified name, because a Mermaid identifier may not hold a dot; an
    /// index into [`nodes`](Self::nodes), whose order is fixed by name, rather than into whatever
    /// order a caller happened to walk.
    fn mermaid_ids(&self) -> BTreeMap<&'a QualifiedName, String> {
        let mut counts: BTreeMap<NodeKind, usize> = BTreeMap::new();
        let mut ids = BTreeMap::new();
        for node in &self.nodes {
            let index = counts.entry(node.kind).or_default();
            ids.insert(node.name, format!("{}{index}", node.kind.prefix()));
            *index += 1;
        }
        ids
    }

    /// Every node, by the name that identifies it.
    fn by_name(&self) -> BTreeMap<&'a QualifiedName, &GraphNode<'a>> {
        self.nodes.iter().map(|node| (node.name, node)).collect()
    }
}

impl GraphGroup<'_> {
    /// The DOT cluster name this group is drawn as.
    ///
    /// `cluster_` is not decoration: Graphviz draws a subgraph as a box only when its name starts
    /// with that word.
    fn cluster(&self) -> String {
        match self.kind {
            GroupKind::Actors => "cluster_actors".to_owned(),
            GroupKind::Component => format!("cluster_{}", self.label),
            GroupKind::Unowned => "cluster_unowned".to_owned(),
        }
    }
}

/// Every node the graph holds: actors, then commands, then events.
///
/// That order fixes the Mermaid identifiers, so it is part of the output rather than a detail.
fn nodes(ir: &EssIr) -> Vec<GraphNode<'_>> {
    let mut out = Vec::new();
    for actor in ir.actors.values() {
        out.push(GraphNode {
            kind: NodeKind::Actor,
            name: &actor.name,
            domain: actor.domain.name(),
        });
    }
    for command in ir.commands.values() {
        out.push(GraphNode {
            kind: NodeKind::Command,
            name: &command.name,
            domain: command.domain.name(),
        });
    }
    for event in ir.events.values() {
        out.push(GraphNode {
            kind: NodeKind::Event,
            name: &event.name,
            domain: event.domain.name(),
        });
    }
    out
}

/// The boxes: the actors, then one per component, then whatever no component claimed.
///
/// The actors are drawn outside every component on purpose. An actor is who *asks*, not something a
/// component holds, and drawing one inside `invoice-service` would claim the service contains it.
/// Drawn first because that is where design §9's graph starts, and an actor with no outgoing edge
/// stays in the picture: "may invoke nothing" is a grant list, not a missing arrow.
///
/// A component with an empty surface still gets its box. A component that accepts nothing and
/// publishes nothing is a decomposition someone has started and not finished, and it should look
/// like an empty box rather than like a component nobody declared.
fn groups(ir: &EssIr) -> Vec<GraphGroup<'_>> {
    let mut out = Vec::new();
    if !ir.actors.is_empty() {
        out.push(GraphGroup {
            kind: GroupKind::Actors,
            label: WHO_MAY_ASK.to_owned(),
            members: ir.actors.keys().collect(),
        });
    }

    let mut placed: BTreeSet<&QualifiedName> = BTreeSet::new();
    for component in ir.components.values() {
        let mut members = Vec::new();
        for handle in &component.accepts {
            placed.insert(handle.name());
            members.push(handle.name());
        }
        for handle in &component.publishes {
            placed.insert(handle.name());
            members.push(handle.name());
        }
        out.push(GraphGroup {
            kind: GroupKind::Component,
            label: component.name.as_str().to_owned(),
            members,
        });
    }

    // Their own box rather than silently dropped: a command no component accepts is a hole in the
    // decomposition, and it should look like one.
    let loose: Vec<&QualifiedName> = ir
        .commands
        .keys()
        .chain(ir.events.keys())
        .filter(|name| !placed.contains(*name))
        .collect();
    if !loose.is_empty() {
        out.push(GraphGroup {
            kind: GroupKind::Unowned,
            label: OWNED_BY_NO_COMPONENT.to_owned(),
            members: loose,
        });
    }

    out
}

/// The arrows: grants, then emissions, then bindings.
///
/// Bindings come from `ir.bindings` rather than from `EssIr::reactions`, which groups the same
/// bindings by event. Both hold the same set; this one is ordered by the binding identifier, which
/// is the order an author reads their own document in.
fn edges(ir: &EssIr) -> Vec<GraphEdge<'_>> {
    let mut out = Vec::new();
    for actor in ir.actors.values() {
        for handle in &actor.may {
            out.push(GraphEdge {
                kind: EdgeKind::Grant,
                from: &actor.name,
                to: handle.name(),
                label: MAY_INVOKE.to_owned(),
                delivery: None,
                on_failure: None,
            });
        }
    }
    for command in ir.commands.values() {
        for outcome in &command.outcomes {
            for handle in &outcome.emits {
                out.push(GraphEdge {
                    kind: EdgeKind::Emission,
                    from: &command.name,
                    to: handle.name(),
                    // Which branch emits it — an event emitted only on refusal is not the same edge
                    // as one emitted on success, and a graph that merged them would say the happy
                    // path always produces both.
                    label: outcome.name.as_str().to_owned(),
                    delivery: None,
                    on_failure: None,
                });
            }
        }
    }
    for binding in ir.bindings.values() {
        out.push(GraphEdge {
            kind: EdgeKind::Binding,
            from: binding.event.name(),
            to: binding.command.name(),
            label: binding.name.as_str().to_owned(),
            delivery: Some(binding.delivery),
            on_failure: Some(binding.failure),
        });
    }
    out
}

/// The word a document used for a delivery guarantee.
///
/// Spelled as the author typed it, because that is what they will search their sources for. Public
/// so that a diagram's edge and `protocol ess inspect` cannot come to spell one guarantee two ways.
pub fn delivery_word(delivery: Delivery) -> &'static str {
    match delivery {
        Delivery::AtLeastOnce => "at_least_once",
    }
}

/// The word a document used for what happens when a binding fails.
pub fn failure_word(failure: Failure) -> &'static str {
    match failure {
        Failure::Retry => "retry",
        Failure::Escalate => "escalate",
        Failure::Drop => "drop",
    }
}

/// Text safe inside a Mermaid quoted label.
pub(crate) fn label(text: &str) -> String {
    text.replace('"', "#quot;").replace('\n', " ")
}

/// Escapes what DOT treats specially inside a quoted string.
///
/// Nothing in the model can hold a quote or a backslash today. The escaping is here anyway, because
/// the alternative is a rendering whose correctness depends on a regular expression in another
/// crate.
fn dot_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A DOT identifier or attribute value.
fn dot_string(value: &str) -> String {
    format!("\"{}\"", dot_escape(value))
}

/// A DOT label whose parts are shown on separate lines.
fn dot_label(lines: &[&str]) -> String {
    let escaped: Vec<String> = lines.iter().map(|line| dot_escape(line)).collect();
    format!("\"{}\"", escaped.join("\\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mermaid_label_cannot_close_the_quoted_string_it_sits_in() {
        assert_eq!(
            label("a \"quoted\" name"),
            "a #quot;quoted#quot; name",
            "a bare quote would end the label and leave Mermaid parsing the rest as syntax"
        );
    }

    #[test]
    fn a_dot_label_keeps_its_parts_on_separate_lines() {
        assert_eq!(
            dot_label(&["SendEmail", "billing.email"]),
            "\"SendEmail\\nbilling.email\"",
            "the local name and its context are two lines of one label, not two labels"
        );
    }

    #[test]
    fn a_component_group_is_a_dot_cluster_and_graphviz_only_boxes_clusters() {
        let group = GraphGroup {
            kind: GroupKind::Component,
            label: "email-service".to_owned(),
            members: Vec::new(),
        };
        assert_eq!(
            group.cluster(),
            "cluster_email-service",
            "without the `cluster_` prefix Graphviz draws no box at all"
        );
    }
}
