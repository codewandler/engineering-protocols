//! The dependency graph, read off the specifications this repository actually ships.
//!
//! Two claims are worth a test here and the rest is arithmetic. The first is that **every relation
//! in the vocabulary is minted by a walk over a real model** — a relation nothing produces is an
//! edge nobody can be reached by, which is the same defect class as a refusal that cannot fire, and
//! it is invisible from inside a unit test built to produce one. The second is that the walk records
//! the edges an author actually wrote, in the direction the author wrote them, which is the one
//! thing about a graph that can be wrong without failing to compile.

mod support;

use std::collections::BTreeSet;

use ess_conformance::scenario::{
    ActorRef, CommandRef, ComponentRef, DeclaredTypeRef, EntityRef, EssSemanticRef, EventRef,
};
use ess_diff::graph::{DependencyRelation, SemanticDependencyGraph};
use ess_domain::component::ComponentName;
use ess_domain::name::QualifiedName;
use support::compiled;

/// A qualified name for an assertion.
fn name(value: &str) -> QualifiedName {
    QualifiedName::new(value).expect("a valid qualified name")
}

/// The graph of the normative example.
fn billing() -> SemanticDependencyGraph {
    SemanticDependencyGraph::of(&compiled("examples/billing"))
}

#[test]
fn every_relation_in_the_vocabulary_is_minted_by_a_specification_this_repository_ships() {
    // The check the vocabulary is worth having. `DependencyRelation` is a closed set, so a variant
    // no walk produces is an edge that can never explain anything — and adding one is exactly the
    // kind of change that compiles, reads well and does nothing.
    //
    // Two specifications, because neither alone carries every construct: `billing` has the views,
    // bindings, unions and escalation, and `revision-pair` is the smaller one the delta tests use.
    let mut minted: BTreeSet<DependencyRelation> = BTreeSet::new();
    for example in [
        "examples/billing",
        "examples/oracle-fixture",
        "examples/revision-pair/before",
    ] {
        let graph = SemanticDependencyGraph::of(&compiled(example));
        minted.extend(graph.edges().map(|edge| edge.relation));
    }

    let missing: Vec<DependencyRelation> = DependencyRelation::ALL
        .into_iter()
        .filter(|relation| !minted.contains(relation))
        .collect();
    assert!(
        missing.is_empty(),
        "no example specification produces {missing:?} — either a walk is missing, or the relation \
         names something the model cannot express and should not be declared"
    );
}

#[test]
fn the_graph_records_the_reference_an_author_wrote_and_not_its_reverse() {
    // The direction is the one property of this graph that is wrong silently: reversed, every
    // closure still runs, still terminates and reports a plausible, empty answer. So the assertion
    // is made on a pair where the two directions differ visibly — an actor references a command,
    // and a command references no actor.
    let graph = billing();
    let customer: EssSemanticRef = ActorRef::new(name("billing.invoice.Customer")).into();
    let create: EssSemanticRef = CommandRef::new(name("billing.invoice.CreateInvoice")).into();

    let grants: Vec<_> = graph
        .dependents_of(&create)
        .filter(|edge| edge.dependent == customer)
        .collect();
    assert_eq!(
        grants.len(),
        1,
        "the actor depends on the command it may invoke: {:?}",
        graph.dependents_of(&create).collect::<Vec<_>>()
    );
    assert_eq!(grants[0].relation, DependencyRelation::MayInvoke);

    assert_eq!(
        graph.dependents_of(&customer).count(),
        0,
        "nothing in a specification depends on an actor, and an edge here would mean the walk \
         recorded the reference backwards"
    );
}

#[test]
fn a_type_is_reached_through_the_declarations_that_hold_it_and_not_by_name() {
    // The transitive claim the whole wave rests on, on the model a person can audit. `Channel` is
    // held by the `Invoice` entity and by nothing else, so a scenario that never mentions `Channel`
    // is still reached — through the entity — and the path is what says why.
    let graph = billing();
    let channel: EssSemanticRef = DeclaredTypeRef::new(name("billing.invoice.Channel")).into();
    let invoice: EssSemanticRef = EntityRef::new(name("billing.invoice.Invoice")).into();

    let reach = graph.closure(&channel);

    assert!(
        reach.reaches(&invoice),
        "the entity holds a `Channel` field"
    );
    let path = reach.path(&invoice).expect("the entity was reached");
    assert_eq!(path.len(), 1, "one hop, and it is the field: {path:?}");
    assert_eq!(path[0].relation, DependencyRelation::FieldType);
    assert_eq!(path[0].dependency, channel);

    // And the view over that entity, which mentions no type at all in its own declaration of the
    // channel — two hops, which is the answer a text search for `Channel` could not have given.
    let views: Vec<_> = reach
        .constructs()
        .filter(|construct| matches!(construct, EssSemanticRef::View { .. }))
        .collect();
    assert!(
        !views.is_empty(),
        "a view projecting the invoice must be reachable from a type the invoice holds"
    );
}

#[test]
fn a_component_is_reached_through_what_it_accepts_and_publishes() {
    // Design §24's worked example, on this repository's own model: the point of an impact report is
    // that it can name the deployable unit a change lands in, and it must get there by a semantic
    // path rather than by a risk score.
    let graph = billing();
    let created: EssSemanticRef = EventRef::new(name("billing.invoice.InvoiceCreated")).into();
    let reach = graph.closure(&created);

    let components: Vec<&EssSemanticRef> = reach
        .constructs()
        .filter(|construct| matches!(construct, EssSemanticRef::Component { .. }))
        .collect();
    assert!(
        !components.is_empty(),
        "some component publishes or reacts to the event: {:?}",
        reach.constructs().collect::<Vec<_>>()
    );

    let invoice_service: EssSemanticRef =
        ComponentRef::new(ComponentName::new("invoice-service").expect("a component name")).into();
    let path = reach
        .path(&invoice_service)
        .expect("the component that publishes the event is reached");
    assert_eq!(
        path.last().expect("a non-empty path").relation,
        DependencyRelation::Publishes
    );
}

#[test]
fn building_the_same_graph_twice_produces_the_same_edges_in_the_same_order() {
    // Two independent compilations and two independent walks. An unordered map anywhere in the
    // build would show up here as a different edge order rather than as a rumour, which is the same
    // check `tests/canonical.rs` makes of the delta's bytes.
    let first: Vec<String> = billing().edges().map(ToString::to_string).collect();
    let second: Vec<String> = billing().edges().map(ToString::to_string).collect();

    assert_eq!(first, second);
    assert!(
        first.len() > 100,
        "the billing graph is not a toy: {} edge(s)",
        first.len()
    );
}

#[test]
fn a_closure_over_the_whole_model_terminates_and_stays_inside_it() {
    // A graph with a cycle in it — and the model permits one, because a binding's command can emit
    // the event another binding reacts to — would hang a naive walk. This runs the closure from
    // every node there is, so the fixture reaches the state the guard is load-bearing in rather than
    // relying on one hand-picked start.
    let graph = billing();
    let nodes = graph.nodes().clone();

    for node in &nodes {
        let reach = graph.closure(node);
        for reached in reach.constructs() {
            assert!(
                nodes.contains(reached),
                "the closure from {node} reported {reached}, which is not a node of the graph"
            );
        }
        assert!(reach.reaches(node), "every construct is in its own closure");
    }
}
