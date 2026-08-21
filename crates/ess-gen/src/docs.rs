//! Markdown and Mermaid: the first projection, and therefore the completeness check.
//!
//! Documentation is generated first because it is the cheapest way to find out what the model cannot
//! say. A construct with no rendering shows up here as a hole in a page a person reads, rather than
//! as a subtly wrong schema nobody validates — so the criterion this module is held to is *every
//! construct the IR carries appears on some page*, not *the pages look nice*.
//!
//! # Three ways a gap is made loud
//!
//! [`Generator::generate`] is infallible on purpose: a construct this crate cannot project is a gap
//! in this crate, not a fault in a specification that has already been resolved. So a gap cannot be
//! reported by failing — and must not be reported by crashing. A `panic!` here would turn "your
//! documentation is incomplete" into "the tool is broken", and it would destroy the very pages that
//! say what is missing, for a reader who cannot fix either. Instead:
//!
//! | the gap | how it becomes loud |
//! |---|---|
//! | a new variant of something this module renders | it stops compiling — no `match` on an enum here has a wildcard arm, so a new `Delivery`, `ResolvedBody`, `ResolvedCondition`, `ResolvedEffect`, `ResolvedFailure`, `ResolvedMappingValue`, `TestStrategy`, `Consistency` or `AssertionStyle` is a build failure in this file |
//! | a construct the IR holds that no page mentions | `tests/docs.rs` fails, asserted per construct |
//! | a construct the IR does not hold at all | [`Docs::known_gaps`], printed on the page where the reader went looking and counted in the index |
//!
//! The third ships nothing today: [`Docs::known_gaps`] is empty, because every construct
//! `ess-domain` parses now reaches [`EssIr`] and reaches a page — entities with their identity,
//! fields, invariants and lifecycle, views with their source, filter and consistency, actors with
//! their grants. The mechanism stays, and stays empty on purpose. It is an allowlist rather than a
//! discovery: a *new* gap is a failing test, and a *closed* one was a deleted entry that changed
//! the pages with it. A page that quietly omits an entity's lifecycle is indistinguishable from a
//! system that has none, which is the reading this table exists to prevent.
//!
//! # Determinism
//!
//! No clock, no RNG, no `HashMap`. Every list is a `BTreeMap`/`BTreeSet` iteration or a `Vec` in
//! declaration order, and Mermaid node identifiers are indices into those orders rather than hashes.
//! `tests/docs.rs` generates twice and compares bytes, because that is the only form in which this
//! paragraph is worth anything.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{
    Driver, EssIr, ResolvedActor, ResolvedBinding, ResolvedBody, ResolvedCommand,
    ResolvedComponent, ResolvedCondition, ResolvedConversion, ResolvedDomain, ResolvedEffect,
    ResolvedEntity, ResolvedError, ResolvedEvent, ResolvedFailure, ResolvedField, ResolvedMapping,
    ResolvedMappingValue, ResolvedSubject, ResolvedType, ResolvedView, ResolvedWorkload,
    TypeHandle,
};
use ess_domain::binding::Delivery;
use ess_domain::command::TestStrategy;
use ess_domain::entity::{Invariant, StateMachine, StateName};
use ess_domain::name::{Naming, QualifiedName};
use ess_domain::view::{AssertionStyle, Consistency};

use crate::artifact::{Artifact, Generator};
use crate::graph::{label, SystemGraph};
use ess_compiler::refs::{
    ActorRef, BindingRef, CommandRef, ComponentRef, DeclaredTypeRef, DomainRef, EntityRef,
    ErrorRef, EssSemanticRef, EventRef, ViewRef,
};

use crate::provenance::{Provenance, ProvenanceMint, SlicedProvenance};

/// Markdown and Mermaid: the cheapest check that every construct can be described.
pub struct Docs;

impl Generator for Docs {
    fn name(&self) -> &'static str {
        "docs"
    }

    fn describes(&self) -> &'static str {
        "Markdown and Mermaid: the cheapest check that every construct can be described"
    }

    fn directory(&self) -> &'static str {
        "docs"
    }

    /// Five kinds of page, and one per bounded context.
    ///
    /// The split follows what a reader arrives with a question about, not what the IR happens to
    /// store: a bounded context is the unit someone reads to learn a vocabulary, the interactions
    /// are the unit someone reads to learn how two contexts meet, and the crossings and the topology
    /// are each a single system-wide question — "what is this system willing to treat as what" and
    /// "what does it need in order to run" — that would be invisible if scattered per domain.
    fn generate(&self, ir: &EssIr, mint: &ProvenanceMint) -> Vec<Artifact> {
        // The four system-wide pages derive from the whole model, honestly: the index draws the
        // whole graph, the interactions page reads every binding, the crossings and topology pages
        // are each one system-wide question. A domain page derives from its own context — plus the
        // bindings and components, which reach across contexts by design — and says so.
        let mut out = vec![readme(ir, &mint.whole())];
        for domain in ir.domains.values() {
            out.push(domain_page(ir, domain, &domain_slice(ir, domain, mint)));
        }
        out.push(interactions_page(ir, &mint.whole()));
        out.push(crossings_page(ir, &mint.whole()));
        out.push(topology_page(ir, &mint.whole()));
        out
    }
}

impl Docs {
    /// Every construct this projection knows it cannot render, and what it would take to fix.
    ///
    /// Public because the honest count belongs to whoever is deciding whether the documentation is
    /// trustworthy, and because `tests/docs.rs` asserts the list is exactly this — so a construct
    /// that goes missing without an entry here fails the build instead of vanishing.
    pub const fn known_gaps() -> &'static [Gap] {
        GAPS
    }
}

/// A construct the specification language has and this projection cannot render.
///
/// Each entry is a hole in [`EssIr`], not in this module. Naming them individually is what stops
/// "the documentation never mentions a view" from reading the same as "the system has no views".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    /// What is missing, named as the specification's author would name it.
    pub construct: &'static str,
    /// What the source says about it that the IR drops.
    pub dropped: &'static str,
    /// Where a reader would have gone looking for it.
    pub page: &'static str,
    /// What would have to change for this projection to render it.
    pub needs: &'static str,
}

/// Nothing: every construct a specification declares reaches the IR, and reaches a page.
///
/// Empty rather than removed. The three entries that were here — entities, views and actors — each
/// named the change in `ess-compiler` that would close it, and each of those changes has since
/// happened: `ResolvedEntity`, `ResolvedView` and `ResolvedActor` are reachable from
/// [`ResolvedDomain`], so the constructs are rendered instead of listed. What is left is the
/// mechanism, which is the part worth keeping: an allowlist, so the next construct the IR drops is a
/// failing test rather than a page that quietly reads like a system without one.
const GAPS: &[Gap] = &[];

// ---- pages ------------------------------------------------------------------------------------

/// The index: what the system is, how it fits together, and where everything else is.
fn readme(ir: &EssIr, provenance: &SlicedProvenance) -> Artifact {
    let mut body = String::new();
    if let Some(summary) = &ir.summary {
        let _ = writeln!(body, "{summary}\n");
    }

    let _ = writeln!(body, "## The system as a graph\n");
    body.push_str(&system_graph(ir));
    let _ = writeln!(
        body,
        "\nA command is accepted by the component that owns its context, emits the events one of \
         its outcomes declares, and a dashed edge is a binding carrying an event into the next \
         command. Design §9 begins one step earlier, at the actor who invokes the first command, \
         and so does this graph: a solid edge out of an actor is a grant, and an actor drawn with \
         no edge at all may invoke nothing — which is something the model says, not an arrow \
         somebody forgot.\n"
    );

    let _ = writeln!(body, "## Bounded contexts\n");
    for domain in ir.domains.values() {
        let _ = writeln!(body, "{}", domain_index_entry(ir, domain));
    }

    let _ = writeln!(body, "\n## Components\n");
    let _ = writeln!(
        body,
        "A component is a unit of ownership, not a deployment. How many of each runs, and what each \
         needs, is [the topology](topology.md).\n"
    );
    for component in ir.components.values() {
        let _ = writeln!(body, "{}\n", component_prose(ir, component));
    }

    let _ = writeln!(body, "## The other pages\n");
    let _ = writeln!(body, "| page | what is on it |");
    let _ = writeln!(body, "|---|---|");
    for domain in ir.domains.values() {
        let _ = writeln!(
            body,
            "| [{}]({}) | the `{}` vocabulary: its types, entities, views, commands, events, \
             errors and actors |",
            display_of(&domain.naming, &domain.name),
            domain_path(&domain.name),
            domain.name
        );
    }
    let _ = writeln!(
        body,
        "| [Interactions](interactions.md) | every binding, with what it guarantees and what \
         happens when it fails |"
    );
    let _ = writeln!(
        body,
        "| [Type crossings](crossings.md) | every conversion this system permits, and the reason \
         someone gave for it |"
    );
    let _ = writeln!(
        body,
        "| [Topology](topology.md) | what each component needs in order to run |"
    );

    body.push('\n');
    body.push_str(&gap_table(
        "## What this projection cannot show",
        "These constructs are in the specification and not in the intermediate representation these \
         pages are generated from, so they cannot appear. They are listed rather than omitted: a \
         page that quietly leaves an entity out reads exactly like a system that has none.",
    ));

    page(
        "README.md".to_owned(),
        &format!("{} {}", ir.system, ir.version),
        &body,
        provenance,
    )
}

/// One bounded context: everything declared inside it, in the order a reader needs it.
///
/// Each section is written in terms of the ones above it, and nothing links downwards: types before
/// entities, because an entity is made of them; entities before views, because a view projects one;
/// commands, events and errors next; and actors last, because a grant is a link *up* the page to the
/// command it names. A reader who meets `Money` first does not have to jump.
/// The slice a domain page derives from: the context and everything declared in it, plus every
/// binding and every component.
///
/// The members, not only the domain: membership edges point *at* the context (`type X is declared
/// in domain D`), so a slice seeded at the domain alone would close over nothing the page renders.
/// The bindings and components are included whole because they are the constructs that reach
/// across contexts by design — a binding into this domain's commands appears on this page, and
/// which bindings those are is itself a fact that changes. The cost of the width is a regeneration
/// when an unrelated binding moves; the alternative is a page claiming to stand still while its
/// own crossings section is stale, and those are not comparable errors.
fn domain_slice(ir: &EssIr, domain: &ResolvedDomain, mint: &ProvenanceMint) -> SlicedProvenance {
    let mut seeds: Vec<EssSemanticRef> = vec![DomainRef::new(domain.name.clone()).into()];
    seeds.extend(
        domain
            .types
            .iter()
            .map(|handle| DeclaredTypeRef::from(handle).into()),
    );
    seeds.extend(
        domain
            .entities
            .iter()
            .map(|handle| EntityRef::from(handle).into()),
    );
    seeds.extend(
        domain
            .commands
            .iter()
            .map(|handle| CommandRef::from(handle).into()),
    );
    seeds.extend(
        domain
            .events
            .iter()
            .map(|handle| EventRef::from(handle).into()),
    );
    seeds.extend(
        domain
            .errors
            .iter()
            .map(|handle| ErrorRef::from(handle).into()),
    );
    seeds.extend(
        domain
            .views
            .iter()
            .map(|handle| ViewRef::from(handle).into()),
    );
    seeds.extend(
        domain
            .actors
            .iter()
            .map(|handle| ActorRef::from(handle).into()),
    );
    seeds.extend(
        ir.bindings
            .keys()
            .map(|name| BindingRef::new(name.clone()).into()),
    );
    seeds.extend(
        ir.components
            .keys()
            .map(|name| ComponentRef::new(name.clone()).into()),
    );
    mint.of_seeds(seeds)
}

fn domain_page(ir: &EssIr, domain: &ResolvedDomain, provenance: &SlicedProvenance) -> Artifact {
    let mut body = String::new();
    if let Some(summary) = &domain.naming.summary {
        let _ = writeln!(body, "{summary}\n");
    }
    let _ = writeln!(
        body,
        "`{}` is one of {}'s bounded contexts. [Back to the index](../README.md).\n",
        domain.name, ir.system
    );

    types_section(ir, domain, &mut body);
    entities_section(ir, domain, &mut body);
    views_section(ir, domain, &mut body);
    commands_section(ir, domain, &mut body);
    events_section(ir, domain, &mut body);
    errors_section(ir, domain, &mut body);
    actors_section(ir, domain, &mut body);
    crossings_section(ir, domain, &mut body);

    body.push_str(&gap_table(
        "## What this page cannot show",
        "This context declares more than appears above. What is missing is missing from the \
         intermediate representation this page is generated from, not from the specification.",
    ));

    page(
        domain_path(&domain.name),
        display_of(&domain.naming, &domain.name),
        &body,
        provenance,
    )
}

/// Every binding: what it reacts to, what it invokes, and what it promises while doing so.
fn interactions_page(ir: &EssIr, provenance: &SlicedProvenance) -> Artifact {
    let mut body = String::from(
        "A binding is the only way an event in one context causes a command in another. Each one \
         states how many times the command may run and what happens when it does not, because a \
         binding that can fail quietly is the difference between specifying a system and specifying \
         a demo.\n\n[Back to the index](README.md).\n\n",
    );

    if ir.bindings.is_empty() {
        body.push_str("This system declares no bindings: nothing here reacts to anything.\n\n");
    }
    for binding in ir.bindings.values() {
        binding_section(ir, binding, &mut body);
    }

    let unread = unread_events(ir);
    if !unread.is_empty() {
        let _ = writeln!(body, "## Events nothing reacts to\n");
        let _ = writeln!(
            body,
            "Legal, and worth seeing. An event with no reader inside the system is either a \
             deliberate boundary — something outside consumes it — or a binding somebody forgot, \
             and only a person can tell which.\n"
        );
        for name in unread {
            let _ = writeln!(body, "- `{name}`");
        }
        body.push('\n');
    }

    page(
        "interactions.md".to_owned(),
        "Interactions",
        &body,
        provenance,
    )
}

/// Every declared conversion, with the reason attached to it.
///
/// A page of its own, and linked from the index by name, because this is where an audit lands. The
/// same reason is repeated at each point of use — on the binding that relies on it, and on the pages
/// of both contexts whose types it joins — so that a reader who never thought to ask "what may be
/// treated as what here" still meets the answer beside the type it concerns.
fn crossings_page(ir: &EssIr, provenance: &SlicedProvenance) -> Artifact {
    let mut body = String::from(
        "A conversion is this system's permission for a value of one type to be used as another. \
         Every one of them carries a reason, and the reason is required rather than optional \
         precisely so that this page can exist: someone asking why an invoice's email address is \
         allowed to become a mailbox address gets an answer written by the person who allowed it, \
         not a shrug.\n\nDeclaring a crossing is also the only way to make one. Two newtypes over \
         `String` do not convert because they are both strings; they convert because a line in the \
         specification says they may.\n\n",
    );

    if ir.conversions.is_empty() {
        body.push_str("This system declares no crossings. Every type is used only as itself.\n\n");
    }
    for conversion in &ir.conversions {
        let _ = writeln!(
            body,
            "## `{}` may be used as `{}`\n",
            conversion.from, conversion.to
        );
        let _ = writeln!(body, "{}\n", quote(&conversion.because));
        let users = crossing_users(ir, conversion);
        if users.is_empty() {
            let _ = writeln!(
                body,
                "Nothing uses this crossing yet. It is still part of what the system permits, \
                 which is why it is written down.\n"
            );
        } else {
            let _ = writeln!(body, "Relied on by:\n");
            for user in users {
                let _ = writeln!(body, "- {user}");
            }
            body.push('\n');
        }
    }

    body.push_str("[Back to the index](README.md).\n");
    page(
        "crossings.md".to_owned(),
        "Type crossings",
        &body,
        provenance,
    )
}

/// What each component needs in order to run, and what a replica floor is claiming.
fn topology_page(ir: &EssIr, provenance: &SlicedProvenance) -> Artifact {
    let mut body = String::from(
        "Runtime requirements, stated semantically. None of this is a deployment and nothing \
         generates a manifest from it: a replica floor of two is a claim that the system is not \
         correct with one instance, which is a fact about the design and survives every change of \
         hosting.\n\n",
    );

    for workload in ir.workloads.values() {
        let component = ir.component(&workload.component);
        let _ = writeln!(body, "## `{}`\n", component.name);
        if let Some(summary) = &component.naming.summary {
            let _ = writeln!(body, "{summary}\n");
        }
        let _ = writeln!(body, "{}\n", replicas_sentence(workload));
        let _ = writeln!(body, "{}\n", stateless_sentence(workload));
        if workload.requires.is_empty() {
            let _ = writeln!(body, "It requires nothing beyond itself.\n");
        } else {
            let _ = writeln!(body, "It requires:\n");
            for resource in &workload.requires {
                let _ = writeln!(body, "- `{}` — `{}`", resource.kind, resource.name);
            }
            body.push('\n');
        }
    }

    let idle: Vec<_> = ir
        .components
        .keys()
        .filter(|name| !ir.workloads.contains_key(*name))
        .collect();
    if !idle.is_empty() {
        let _ = writeln!(body, "## Components that run nowhere\n");
        let _ = writeln!(
            body,
            "Declared as a unit of ownership, with nothing in the topology running it. That is \
             legal — a context can be owned by a library — but it is the kind of legal worth \
             reading twice.\n"
        );
        for name in idle {
            let _ = writeln!(body, "- `{name}`");
        }
        body.push('\n');
    }

    body.push_str("[Back to the index](README.md).\n");
    page("topology.md".to_owned(), "Topology", &body, provenance)
}

// ---- sections ---------------------------------------------------------------------------------

/// The types an author declared in a context, one paragraph each.
///
/// An entity's state enum is left out because it is not an author's declaration: the compiler
/// synthesises it from a lifecycle, and it is rendered with that lifecycle, in the entity's own
/// section. Found by comparing handles with [`ResolvedEntity::state_type`] rather than by reading
/// `State` out of a name, because a name read for meaning is an identity used as a key.
fn types_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    let synthesised = state_types(ir, domain);
    let declared: Vec<_> = domain
        .types
        .iter()
        .filter(|handle| !synthesised.contains(*handle))
        .map(|handle| ir.named_type(handle))
        .collect();
    if declared.is_empty() {
        return;
    }
    let _ = writeln!(body, "## Types\n");
    for declared in &declared {
        let _ = writeln!(body, "### `{}`\n", relative(&declared.name, &domain.name));
        let _ = writeln!(body, "{}\n", type_prose(declared).trim_end());
    }
    orphan_note(ir, &declared, body);
}

/// The types nothing else in the IR mentions.
///
/// Worth a paragraph rather than a silent omission, and worth *only* a paragraph: nothing declares a
/// field of one of these, which makes it either vocabulary something outside this specification
/// uses or a leftover, and only a person can tell which. The one reading it must not invite is
/// "reached through a construct the projection dropped" — every construct that reaches a type
/// (entity, view, command, event, error, crossing) is counted below, so an orphan here is an orphan
/// in the model.
fn orphan_note(ir: &EssIr, declared: &[&ResolvedType], body: &mut String) {
    let referenced = referenced_types(ir);
    let orphans: Vec<_> = declared
        .iter()
        .filter(|it| !referenced.contains(&it.name))
        .map(|it| code(&it.name.to_string()))
        .collect();
    if orphans.is_empty() {
        return;
    }
    let _ = writeln!(
        body,
        "{} of the types above {} reached by nothing else in this system: {}. No entity, view, \
         command, event, error or crossing names {}, so it is either vocabulary something outside \
         this specification uses or a leftover — and only a person can tell which.\n",
        capitalise(&number(orphans.len())),
        if orphans.len() == 1 { "is" } else { "are" },
        list(&orphans),
        if orphans.len() == 1 { "it" } else { "them" }
    );
}

/// Every named type reached from a field, an input, a payload, a union variant or a crossing.
///
/// An entity's fields and a view's projected fields are in here, because they are the reason most
/// types exist: leaving them out would report `LineItem` as reached by nothing while the page above
/// draws it inside `Invoice`. An entity's state enum is deliberately not counted as a reference —
/// nothing *names* it, the compiler makes it.
fn referenced_types(ir: &EssIr) -> BTreeSet<QualifiedName> {
    let mut out = BTreeSet::new();
    let mut note = |reference: &ess_compiler::ir::ResolvedTypeRef| {
        for handle in reference.named_leaves() {
            out.insert(handle.name().clone());
        }
    };
    for declared in ir.types.values() {
        match &declared.body {
            ResolvedBody::Newtype { of, .. } => note(of),
            ResolvedBody::Struct { fields, .. } => {
                for field in fields {
                    note(&field.type_ref);
                }
            }
            ResolvedBody::Enum { .. } => {}
            ResolvedBody::Union { variants, .. } => {
                for variant in variants.values() {
                    note(variant);
                }
            }
        }
    }
    for entity in ir.entities.values() {
        note(&entity.identity.type_ref);
        for field in &entity.fields {
            note(&field.type_ref);
        }
    }
    for view in ir.views.values() {
        for field in &view.fields {
            note(&field.type_ref);
        }
    }
    for command in ir.commands.values() {
        for field in &command.input {
            note(&field.type_ref);
        }
    }
    for event in ir.events.values() {
        for field in &event.fields {
            note(&field.type_ref);
        }
    }
    for error in ir.errors.values() {
        for field in &error.fields {
            note(&field.type_ref);
        }
    }
    for conversion in &ir.conversions {
        note(&conversion.from);
        note(&conversion.to);
    }
    out
}

/// Every entity: what identifies it, what it holds, what stays true, and where it may move.
///
/// The lifecycle is the part that cannot be a table. `Paid` not becoming `Cancelled` is expressed by
/// the *absence* of a transition, so the section carries three things a list of states cannot: the
/// diagram, the initial and terminal states, and — because absence does not draw — the pairs no move
/// connects.
fn entities_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    if domain.entities.is_empty() {
        return;
    }
    let projections = ir.projections();
    let drivers = ir.drivers();
    let _ = writeln!(body, "## Entities\n");
    let _ = writeln!(
        body,
        "An entity is what this context is about: something with an identity that outlives any one \
         request, a shape, and a lifecycle. The lifecycle is exhaustive — a move that is not drawn \
         below is a move this specification does not permit, and that is the only way it says so. \
         Every move is labelled with the command that takes it, because a move nothing can trigger \
         is refused rather than drawn.\n"
    );
    for handle in &domain.entities {
        let entity = ir.entity(handle);
        let _ = writeln!(body, "### `{}`\n", relative(&entity.name, &domain.name));
        let _ = writeln!(body, "{}\n", naming_sentence(&entity.naming, &entity.name));
        let _ = writeln!(body, "{}\n", identity_sentence(entity));
        if entity.fields.is_empty() {
            let _ = writeln!(
                body,
                "It holds nothing beyond its identity and its state.\n"
            );
        } else {
            let _ = writeln!(body, "It holds:\n");
            for field in &entity.fields {
                let _ = writeln!(body, "{}", field_bullet(field));
            }
            body.push('\n');
        }
        let _ = writeln!(body, "{}\n", entity_invariants_sentence(entity));
        let _ = writeln!(body, "{}\n", state_type_sentence(entity));
        let _ = writeln!(body, "{}\n", resting_sentence(&entity.lifecycle));
        let driven = drivers.get(handle).map_or(&[][..], Vec::as_slice);
        body.push_str(&state_diagram(&entity.lifecycle, driven));
        body.push('\n');
        let _ = writeln!(body, "{}\n", driven_sentence(&entity.lifecycle, driven));
        let _ = writeln!(body, "{}", legality_note(&entity.lifecycle).trim_end());
        body.push('\n');
        let _ = writeln!(
            body,
            "{}\n",
            observed_by_sentence(ir, domain, projections.get(handle))
        );
    }
}

/// Every view: what it reads, which instances it holds, and how soon it holds them.
fn views_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    if domain.views.is_empty() {
        return;
    }
    let _ = writeln!(body, "## Views\n");
    let _ = writeln!(
        body,
        "A view is what the outside world is promised it can observe. Each one says which instances \
         it contains and how soon it reflects a command that has already returned, because \"you \
         can read this\" without \"how soon\" is the promise every flaky suite is built on.\n"
    );
    for handle in &domain.views {
        let view = ir.view(handle);
        let source = ir.entity(&view.source);
        let _ = writeln!(body, "### `{}`\n", relative(&view.name, &domain.name));
        let _ = writeln!(body, "{}\n", naming_sentence(&view.naming, &view.name));
        let _ = writeln!(
            body,
            "It reads {}.\n",
            section_link(ir, domain, &source.name, &source.domain)
        );
        let _ = writeln!(body, "{}\n", filter_sentence(view));
        if view.fields.is_empty() {
            let _ = writeln!(
                body,
                "It exposes no fields, so it answers \"does an instance match\" and nothing about \
                 the instance.\n"
            );
        } else {
            let _ = writeln!(body, "It exposes:\n");
            for field in &view.fields {
                let _ = writeln!(body, "{}", field_bullet(field));
            }
            body.push('\n');
        }
        let _ = writeln!(body, "{}\n", consistency_sentence(view.consistency));
        let _ = writeln!(body, "{}\n", assertion_sentence(view.assertion_style));
    }
}

/// Every actor, and the commands each of them may invoke.
fn actors_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    if domain.actors.is_empty() {
        return;
    }
    let _ = writeln!(body, "## Actors\n");
    let _ = writeln!(
        body,
        "An actor is who may ask this context for something. Every grant below points at a command \
         this specification declares — a grant is a resolved reference, so \"may invoke\" something \
         nobody wrote is not a permission this model can express, and an authorisation that \
         authorises nothing cannot ship quietly.\n"
    );
    for handle in &domain.actors {
        let actor = ir.actor(handle);
        let _ = writeln!(body, "### `{}`\n", relative(&actor.name, &domain.name));
        let _ = writeln!(body, "{}\n", naming_sentence(&actor.naming, &actor.name));
        let _ = writeln!(body, "{}\n", grants_sentence(ir, domain, actor));
    }
}

/// Every command, with its input and — the part that matters — every outcome.
fn commands_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    if domain.commands.is_empty() {
        return;
    }
    let _ = writeln!(body, "## Commands\n");
    for handle in &domain.commands {
        let command = ir.command(handle);
        let _ = writeln!(body, "### `{}`\n", relative(&command.name, &domain.name));
        let _ = writeln!(
            body,
            "{}\n",
            naming_sentence(&command.naming, &command.name)
        );
        if command.input.is_empty() {
            let _ = writeln!(body, "It takes no input.\n");
        } else {
            let _ = writeln!(body, "It takes:\n");
            for field in &command.input {
                let _ = writeln!(body, "{}", field_bullet(field));
            }
            body.push('\n');
        }
        let _ = writeln!(
            body,
            "{}\n",
            outcome_count_sentence(command.outcomes.len(), &command.name)
        );
        for outcome in &command.outcomes {
            let _ = writeln!(body, "{}\n", outcome_prose(ir, command, outcome));
        }
    }
}

/// Every event, what it carries, and who causes and reads it.
fn events_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    if domain.events.is_empty() {
        return;
    }
    let reactions = ir.reactions();
    let _ = writeln!(body, "## Events\n");
    for handle in &domain.events {
        let event = ir.event(handle);
        let _ = writeln!(body, "### `{}`\n", relative(&event.name, &domain.name));
        let _ = writeln!(body, "{}\n", naming_sentence(&event.naming, &event.name));
        if event.fields.is_empty() {
            let _ = writeln!(
                body,
                "It carries nothing: the fact that it happened is the whole payload.\n"
            );
        } else {
            let _ = writeln!(body, "It carries:\n");
            for field in &event.fields {
                let _ = writeln!(body, "{}", field_bullet(field));
            }
            body.push('\n');
        }
        for sentence in emitters(ir, event) {
            let _ = writeln!(body, "{sentence}\n");
        }
        match reactions.get(handle) {
            None => {
                let _ = writeln!(body, "Nothing in this system reacts to it.\n");
            }
            Some(bindings) => {
                let names = list(&bindings.iter().map(|it| code(it.name.as_str())).collect());
                let _ = writeln!(
                    body,
                    "{names} reacts to it — see [Interactions](../interactions.md).\n"
                );
            }
        }
    }
}

/// Every error, what it carries, and which branch reports it.
fn errors_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    if domain.errors.is_empty() {
        return;
    }
    let _ = writeln!(body, "## Errors\n");
    for handle in &domain.errors {
        let error = ir.error(handle);
        let _ = writeln!(body, "### `{}`\n", relative(&error.name, &domain.name));
        if let Some(summary) = &error.summary {
            let _ = writeln!(body, "{summary}\n");
        }
        if error.fields.is_empty() {
            let _ = writeln!(
                body,
                "It carries nothing beyond its name, so a caller can tell what went wrong and not \
                 which value caused it.\n"
            );
        } else {
            let _ = writeln!(body, "It carries:\n");
            for field in &error.fields {
                let _ = writeln!(body, "{}", field_bullet(field));
            }
            body.push('\n');
        }
        for sentence in reporters(ir, error) {
            let _ = writeln!(body, "{sentence}\n");
        }
    }
}

/// The crossings with an end in this context, repeated here on purpose.
///
/// This is the answer to "where does a conversion's reason go so that someone finds it without
/// knowing to look": beside the type. A reader on this page is reading about `Email`; that is where
/// the sentence saying `Email` may become somebody else's address has to be.
fn crossings_section(ir: &EssIr, domain: &ResolvedDomain, body: &mut String) {
    let relevant: Vec<_> = ir
        .conversions
        .iter()
        .filter(|conversion| {
            touches(&conversion.from, &domain.name) || touches(&conversion.to, &domain.name)
        })
        .collect();
    if relevant.is_empty() {
        return;
    }
    let _ = writeln!(body, "## Type crossings\n");
    let _ = writeln!(
        body,
        "Types in this context that the specification permits to be used as another type, or the \
         other way round. Nothing else crosses: two newtypes over the same primitive stay distinct \
         until a line in the specification says otherwise.\n"
    );
    for conversion in relevant {
        let _ = writeln!(
            body,
            "**`{}` may be used as `{}`**, because:\n",
            conversion.from, conversion.to
        );
        let _ = writeln!(body, "{}\n", quote(&conversion.because));
    }
    let _ = writeln!(
        body,
        "Every crossing in the system is on one page: [Type crossings](../crossings.md).\n"
    );
}

/// One binding, its guarantees in prose, its mapping, and the flow a table cannot show.
fn binding_section(ir: &EssIr, binding: &ResolvedBinding, body: &mut String) {
    let event = ir.event(&binding.event);
    let command = ir.command(&binding.command);
    let _ = writeln!(body, "## `{}`\n", binding.name);
    if let Some(summary) = &binding.naming.summary {
        let _ = writeln!(body, "{summary}\n");
    }
    let _ = writeln!(
        body,
        "`{}` causes [`{}`]({}#{}).\n",
        event.name,
        command.name,
        domain_path(&ir.domain(&command.domain).name),
        slug(&relative(&command.name, &ir.domain(&command.domain).name))
    );

    body.push_str(&binding_flow(ir, binding));
    body.push('\n');

    let _ = writeln!(body, "{}\n", delivery_sentence(binding.delivery, command));
    let _ = writeln!(body, "{}\n", failure_sentence(ir, binding));

    if binding.mapping.is_empty() {
        let _ = writeln!(
            body,
            "It fills none of the command's input: every value the command needs has to come from \
             somewhere else.\n"
        );
    } else {
        let _ = writeln!(body, "It fills the command's input like this:\n");
        for mapping in &binding.mapping {
            let _ = writeln!(body, "{}", mapping_bullet(mapping));
        }
        body.push('\n');
    }
}

// ---- prose ------------------------------------------------------------------------------------

/// A named type as a sentence, because its shape is one fact and a table of one fact is furniture.
fn type_prose(declared: &ResolvedType) -> String {
    let name = code(&declared.name.to_string());
    let mut out = match &declared.body {
        ResolvedBody::Newtype { of, invariants } => {
            let mut text = format!(
                "{name} wraps `{of}` and is not interchangeable with one: the whole value of naming \
                 it separately is the crossings the model then refuses."
            );
            text.push_str(&invariants_clause(invariants));
            text
        }
        ResolvedBody::Struct { fields, invariants } => {
            let mut text = format!(
                "{name} is a record of {}:\n\n",
                plural(fields.len(), "field")
            );
            for field in fields {
                let _ = writeln!(text, "{}", field_bullet(field));
            }
            let clause = invariants_clause(invariants);
            if !clause.is_empty() {
                let _ = write!(text, "\n{}", clause.trim_start());
            }
            text
        }
        ResolvedBody::Enum { variants } => format!(
            "{name} is one of {}.",
            list(&variants.iter().map(|it| code(it)).collect())
        ),
        ResolvedBody::Union { tag, variants } => {
            let mut text = format!(
                "{name} is one of {}, told apart by a `{tag}` field — tagged, so a decoder never \
                 has to guess which branch it is reading:\n\n",
                plural(variants.len(), "shape")
            );
            for (variant, type_ref) in variants {
                let _ = writeln!(text, "- `{variant}` — `{type_ref}`");
            }
            text
        }
    };
    if let Some(display) = &declared.naming.display {
        let _ = write!(out, "\n\nShown to a person as \"{display}\".");
    }
    out
}

/// One outcome, including the two things a name alone loses: what decides it, and what it costs.
fn outcome_prose(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ess_compiler::ir::ResolvedOutcome,
) -> String {
    let mut out = format!("**`{}`** — ", outcome.name);
    if let Some(summary) = &outcome.summary {
        let _ = write!(out, "{summary} ");
    }
    let _ = write!(
        out,
        "{}",
        condition_sentence(ir, command, &outcome.condition)
    );
    let _ = write!(out, " {}", effect_sentence(ir, outcome.subject.as_ref()));
    if let Some(error) = &outcome.error {
        let reported = ir.error(error);
        let _ = write!(out, " It reports `{}`", reported.name);
        if reported.fields.is_empty() {
            out.push('.');
        } else {
            let carried = list(&reported.fields.iter().map(|it| code(&it.name)).collect());
            let _ = write!(out, ", carrying {carried}.");
        }
    }
    match outcome.emits.as_slice() {
        [] => out.push_str(" It emits nothing."),
        emitted => {
            let names = list(&emitted.iter().map(|it| code(&it.to_string())).collect());
            let _ = write!(out, " It emits {names}.");
        }
    }
    let _ = write!(out, " {}", strategy_sentence(outcome.test_strategy));
    out
}

/// What this branch does to an entity, including the case where it does nothing.
///
/// Written for every outcome and not only for the ones with a subject, because silence is the one
/// answer a reader cannot interpret: "this branch changes no entity" and "the projection dropped the
/// field" look identical on a page, and the first is a fact about the system.
fn effect_sentence(ir: &EssIr, subject: Option<&ResolvedSubject>) -> String {
    let Some(subject) = subject else {
        return "No entity in this specification changes.".to_owned();
    };
    let entity = ir.entity(&subject.entity);
    let effect = match &subject.effect {
        ResolvedEffect::Creates => format!(
            "It creates a `{}`, which starts in `{}`.",
            entity.name, entity.lifecycle.initial
        ),
        ResolvedEffect::Moves { transition } => format!(
            "It moves a `{}` from {} to `{}`, along the declared move `{}`.",
            entity.name,
            list(
                &transition
                    .from
                    .iter()
                    .map(|state| code(&state.to_string()))
                    .collect()
            ),
            transition.to,
            transition.name
        ),
        ResolvedEffect::Updates => format!(
            "It changes a `{}` without moving it along its lifecycle.",
            entity.name
        ),
    };
    format!("{effect} {}", instance_sentence(ir, subject))
}

/// Which instance the branch acts on, and where a reader finds its identity.
///
/// A page that says an invoice moved and not *which* invoice describes a system nobody can call.
/// The two sentences differ because the two surfaces do: an existing instance is named by the caller
/// in the request, and a new one is announced by the event the branch emits, because it did not
/// exist when the request was made.
fn instance_sentence(ir: &EssIr, subject: &ResolvedSubject) -> String {
    let field = subject.instance.field();
    match subject.instance.event() {
        None => format!(
            "The instance is the one named by the input field {}.",
            code(&field.name)
        ),
        Some(event) => format!(
            "The new instance's identity is published as {} on `{}`.",
            code(&field.name),
            ir.event(event).name
        ),
    }
}

/// What decides that an outcome is the one taken.
///
/// [`ResolvedCondition::WrongState`] is the one case that reads the rest of the specification, and
/// deliberately so: the document does not say which states the branch answers in, because the
/// transitions already do. A page that printed only "the subject is in the wrong state" would leave
/// a reader to do that subtraction by hand across a lifecycle and a command, which is exactly the
/// work [`EssIr::wrong_states`] exists to have already done.
fn condition_sentence(
    ir: &EssIr,
    command: &ResolvedCommand,
    condition: &ResolvedCondition,
) -> String {
    match condition {
        ResolvedCondition::When { predicate } => {
            format!("Taken when `{predicate}` holds of the input.")
        }
        ResolvedCondition::Otherwise => {
            "The default branch, taken when no other outcome's condition matched.".to_owned()
        }
        ResolvedCondition::External { cause } => format!(
            "Decided outside the input: {cause}. No predicate over the input reaches this branch, \
             and saying `when: false` instead would have claimed it is unreachable, which is a \
             different and false statement."
        ),
        ResolvedCondition::WrongState => {
            let mut text = "Taken when the subject is resting in a state none of this command's \
                            moves start from"
                .to_owned();
            for (handle, states) in ir.wrong_states(command) {
                let _ = write!(
                    text,
                    " — a `{}` in {}",
                    ir.entity(handle).name,
                    list(
                        &states
                            .iter()
                            .map(|state| code(&state.to_string()))
                            .collect()
                    )
                );
            }
            text.push_str(
                ", which is what is left of the lifecycle once this command's own moves are taken \
                 away. The document lists none of it.",
            );
            text
        }
    }
}

/// How a generated test is meant to reach a branch.
///
/// On the page because the specification computes it once, on the model, so that no two projections
/// can disagree about whether a branch can be reached by constructing an input.
fn strategy_sentence(strategy: TestStrategy) -> &'static str {
    match strategy {
        TestStrategy::ConstructInput => {
            "A test reaches it by constructing an input that satisfies that condition."
        }
        TestStrategy::DefaultBranch => {
            "A test reaches it by constructing an input that satisfies no other outcome's condition."
        }
        TestStrategy::InjectFault => {
            "A test reaches it by injecting the declared fault, because no input can."
        }
        TestStrategy::ArrangeState => {
            "A test reaches it by driving an instance into one of those states and then issuing the \
             command, because no input selects this branch."
        }
    }
}

/// How many times the command may run, and what that obliges the command to be.
fn delivery_sentence(delivery: Delivery, command: &ResolvedCommand) -> String {
    match delivery {
        Delivery::AtLeastOnce => format!(
            "Delivered **at least once**, so `{}` must be idempotent: the same event arriving twice \
             must not do the work twice. \"Exactly once\" is what everyone believes they have until \
             a retry proves otherwise, which is why this is written down rather than assumed.",
            command.name
        ),
    }
}

/// What happens when the command does not run, and how a reader could tell that it did.
///
/// The escalation's event is named rather than left as "surfaced to a person somehow", because the
/// page is what a conformance target is written against: a sentence that names no observable
/// describes a requirement nobody can be asked to prove.
fn failure_sentence(ir: &EssIr, binding: &ResolvedBinding) -> String {
    match binding.on_failure() {
        ResolvedFailure::Retry => {
            "When it fails it is **retried**, on whatever schedule the transport provides. Nothing \
             here says how many times, so nothing here says when it stops. A retry publishes \
             nothing of its own, because it is already observable: it is another invocation of the \
             command."
                .to_owned()
        }
        ResolvedFailure::Escalate { emits } => format!(
            "When it fails it is **escalated** — surfaced to a person, who decides what happens \
             next — and the system publishes `{}` to say so. Surfacing something to a person \
             happens outside the system, so that event is the only way a reader, a test or a \
             conformance target can tell that the escalation happened at all.",
            ir.event(emits).name
        ),
        ResolvedFailure::Drop => {
            "When it fails the work is **dropped**. The system loses it, silently, and that is a \
             decision someone made deliberately: `drop` is never a default, so this word was \
             typed. Nothing is published, on purpose — an event here would make this a \
             notification, which is a different decision."
                .to_owned()
        }
    }
}

/// One filled command input, and the reason its types were allowed to meet.
fn mapping_bullet(mapping: &ResolvedMapping) -> String {
    let mut out = format!("- `{}` (`{}`) ← ", mapping.target, mapping.target_type);
    match &mapping.value {
        ResolvedMappingValue::EventField { field, type_ref } => {
            let _ = write!(out, "the event's `{field}` (`{type_ref}`)");
            if let Some(because) = &mapping.conversion {
                let _ = write!(
                    out,
                    ". The two types differ, and the crossing is declared: \"{}.\"",
                    because.trim().trim_end_matches('.')
                );
            } else {
                out.push('.');
            }
        }
        ResolvedMappingValue::Literal { value } => {
            let _ = write!(
                out,
                "the literal `{value}`. Nothing in the model says how to read that as a `{}`, so \
                 the compiler took it on trust rather than checking it.",
                mapping.target_type
            );
        }
    }
    out
}

/// A component's ownership, which is the only claim it makes.
fn component_prose(ir: &EssIr, component: &ResolvedComponent) -> String {
    let mut out = format!("**`{}`**", component.name);
    if let Some(display) = &component.naming.display {
        let _ = write!(out, " (shown as \"{display}\")");
    }
    if let Some(summary) = &component.naming.summary {
        let _ = write!(out, " — {summary}");
    } else {
        out.push('.');
    }
    let _ = write!(out, " It owns {}.", owned_list(ir, component));
    if component.accepts.is_empty() {
        out.push_str(" It accepts no commands.");
    } else {
        let names = list(
            &component
                .accepts
                .iter()
                .map(|it| code(&it.to_string()))
                .collect(),
        );
        let _ = write!(out, " It accepts {names}.");
    }
    if component.publishes.is_empty() {
        out.push_str(" It publishes no events.");
    } else {
        let names = list(
            &component
                .publishes
                .iter()
                .map(|it| code(&it.to_string()))
                .collect(),
        );
        let _ = write!(out, " It publishes {names}.");
    }
    out
}

/// The contexts a component owns, or the fact that it owns none.
fn owned_list(ir: &EssIr, component: &ResolvedComponent) -> String {
    if component.owns.is_empty() {
        return "no bounded context — it is a unit of ownership that owns nothing, which is worth a \
                second look"
            .to_owned();
    }
    list(
        &component
            .owns
            .iter()
            .map(|handle| {
                let domain = ir.domain(handle);
                format!("[`{}`]({})", domain.name, domain_path(&domain.name))
            })
            .collect(),
    )
}

/// One line in the index for a bounded context, with the numbers rather than an adjective.
fn domain_index_entry(ir: &EssIr, domain: &ResolvedDomain) -> String {
    let mut out = format!(
        "- **[{}]({})** (`{}`)",
        display_of(&domain.naming, &domain.name),
        domain_path(&domain.name),
        domain.name
    );
    if let Some(summary) = &domain.naming.summary {
        let _ = write!(out, " — {summary}");
    }
    let _ = write!(out, " {}.", capitalise(&list(&member_counts(ir, domain))));
    out
}

/// What a context holds, counted in the order its page renders it.
///
/// The type count excludes the enum each entity's lifecycle forms, because the page's `Types`
/// section excludes it too: a count that does not match the list under it is a count a reader stops
/// trusting.
fn member_counts(ir: &EssIr, domain: &ResolvedDomain) -> Vec<String> {
    let authored_types = domain.types.len() - state_types(ir, domain).len();
    vec![
        plural(authored_types, "type"),
        plural(domain.entities.len(), "entity"),
        plural(domain.views.len(), "view"),
        plural(domain.commands.len(), "command"),
        plural(domain.events.len(), "event"),
        plural(domain.errors.len(), "error"),
        plural(domain.actors.len(), "actor"),
    ]
}

/// What a replica floor claims, which is not a capacity plan.
fn replicas_sentence(workload: &ResolvedWorkload) -> String {
    let floor = match workload.replicas.min {
        0 => {
            "No replica floor is declared, so the specification does not say that running this is \
              necessary."
                .to_owned()
        }
        1 => "One instance is enough: nothing about the design needs a second.".to_owned(),
        min => format!(
            "At least {min} instances. That is a statement about correctness, not about load — the \
             specification says this system is not correct with fewer."
        ),
    };
    match workload.replicas.max {
        None => format!("{floor} No ceiling is declared."),
        Some(max) => format!("{floor} At most {max}."),
    }
}

/// Whether an instance holds anything that outlives a request.
fn stateless_sentence(workload: &ResolvedWorkload) -> &'static str {
    if workload.stateless {
        "Stateless: an instance holds nothing that outlives a request, so instances are \
         interchangeable."
    } else {
        "Stateful: an instance holds state that outlives a request, so instances are not \
         interchangeable."
    }
}

/// What a construct is called, on the wire and to a person.
fn naming_sentence(naming: &Naming, name: &QualifiedName) -> String {
    let mut parts = Vec::new();
    if let Some(display) = &naming.display {
        parts.push(format!("shown to a person as \"{display}\""));
    }
    if let Some(wire) = &naming.wire {
        parts.push(format!("called `{wire}` on the wire"));
    }
    if parts.is_empty() {
        format!("`{name}`.")
    } else {
        format!("`{name}`, {}.", list(&parts))
    }
}

/// How an instance is identified, and why the field's *name* is on this page at all.
fn identity_sentence(entity: &ResolvedEntity) -> String {
    let mut out = format!(
        "An instance is identified by `{}`, a `{}`",
        entity.identity.name, entity.identity.type_ref
    );
    if let Some(wire) = &entity.identity.naming.wire {
        if wire != &entity.identity.name {
            let _ = write!(out, ", called `{wire}` on the wire");
        }
    }
    if let Some(display) = &entity.identity.naming.display {
        let _ = write!(out, ", shown as \"{display}\"");
    }
    out.push_str(
        ". The name is part of the model and not a convention: a view projects the identity under \
         that name, so a projection inventing its own would disagree with the view.",
    );
    out
}

/// What must hold of an instance at rest, or the fact that nothing does.
fn entity_invariants_sentence(entity: &ResolvedEntity) -> String {
    if entity.invariants.is_empty() {
        return "No invariant is declared, so nothing here constrains an instance at rest."
            .to_owned();
    }
    format!(
        "Every instance satisfies {} — a predicate over this entity's own fields, checked against \
         them rather than stored as a sentence, so an invariant reading something the entity does \
         not have is refused instead of documented.",
        list(
            &entity
                .invariants
                .iter()
                .map(|it| code(&it.statement))
                .collect()
        )
    )
}

/// The states an instance can be in, and where that enum comes from.
///
/// The states are read from the lifecycle rather than from the enum's variants: both say the same
/// thing, and taking them from the declaration that owns the rule means this page cannot show a
/// state the diagram below does not.
fn state_type_sentence(entity: &ResolvedEntity) -> String {
    format!(
        "Its state is a `{}`, one of {}. That enum is synthesised from the lifecycle rather than \
         declared beside it, so the states a view's filter compares and the states drawn below \
         cannot disagree.",
        entity.state_type,
        list(
            &entity
                .lifecycle
                .states
                .iter()
                .map(|state| code(state.as_str()))
                .collect()
        )
    )
}

/// Where an instance starts, and where it is allowed to stop.
fn resting_sentence(lifecycle: &StateMachine) -> String {
    let mut out = format!("An instance is created in `{}`.", lifecycle.initial);
    if lifecycle.terminal.is_empty() {
        out.push_str(
            " No state is terminal: nothing in this lifecycle says an instance may stop moving.",
        );
        return out;
    }
    let _ = write!(
        out,
        " {} {} terminal, so an instance may rest there forever. That is declared rather than \
         inferred from having no way out: an entity that cannot leave a state is either finished or \
         stuck, and only its author knows which.",
        capitalise(&list(
            &lifecycle
                .terminal
                .iter()
                .map(|state| code(state.as_str()))
                .collect()
        )),
        if lifecycle.terminal.len() == 1 {
            "is"
        } else {
            "are"
        }
    );
    out
}

/// Which views expose an entity, so a reader learns what of it leaves the context.
fn observed_by_sentence(
    ir: &EssIr,
    domain: &ResolvedDomain,
    views: Option<&Vec<&ResolvedView>>,
) -> String {
    let Some(views) = views else {
        return "No view projects it, so nothing outside this context is promised a way to observe \
                one."
            .to_owned();
    };
    let links: Vec<String> = views
        .iter()
        .map(|view| section_link(ir, domain, &view.name, &view.domain))
        .collect();
    format!(
        "{} {} it: {}.",
        capitalise(&plural(links.len(), "view")),
        if links.len() == 1 {
            "projects"
        } else {
            "project"
        },
        list(&links)
    )
}

/// Which instances a view holds, including the case where it holds all of them.
fn filter_sentence(view: &ResolvedView) -> String {
    match &view.filter {
        None => "It contains every instance of that entity: no filter narrows it, which is a \
                 decision somebody made and not a line somebody omitted."
            .to_owned(),
        Some(filter) => format!(
            "It contains the instances where `{filter}` holds, and only those — so an instance a \
             caller cannot find in here has been filtered out rather than lost."
        ),
    }
}

/// How soon a view reflects a command that has already returned.
fn consistency_sentence(consistency: Consistency) -> &'static str {
    match consistency {
        Consistency::ReadYourWrites => {
            "**Read-your-writes**: it is current the moment the command that changed it returns. A \
             caller that has just created an invoice and cannot see it in here has been told a lie \
             about what it did."
        }
        Consistency::Eventual => {
            "**Eventual**: it catches up some time after the command returns, so a caller that \
             reads it immediately may legitimately not see its own write yet. Nothing here says how \
             long that takes, so nothing here lets a caller wait a fixed time and call it correct."
        }
    }
}

/// What that consistency obliges a generated test to do, which is where it stops being a word.
fn assertion_sentence(style: AssertionStyle) -> &'static str {
    match style {
        AssertionStyle::Expect => {
            "A generated scenario asserts it once, immediately after the command: a view promising \
             this and not keeping the promise has to fail the suite rather than be retried until it \
             passes."
        }
        AssertionStyle::Eventually => {
            "A generated scenario therefore retries the assertion until the projection catches up, \
             rather than asserting once and racing it. The repair everyone reaches for instead is a \
             sleep, which turns the suite into a test of the machine it runs on."
        }
    }
}

/// The commands an actor may invoke, as links to where each one is written.
fn grants_sentence(ir: &EssIr, domain: &ResolvedDomain, actor: &ResolvedActor) -> String {
    if actor.may.is_empty() {
        return "It may invoke nothing: it observes. \"Who is in this picture\" is part of what a \
                specification describes, so an actor with no grant is a statement rather than an \
                unfinished line."
            .to_owned();
    }
    let links: Vec<String> = actor
        .may
        .iter()
        .map(|handle| {
            let command = ir.command(handle);
            section_link(ir, domain, &command.name, &command.domain)
        })
        .collect();
    format!("It may invoke {}.", list(&links))
}

/// The invariants a type's values satisfy, as a clause rather than a heading.
fn invariants_clause(invariants: &[Invariant]) -> String {
    if invariants.is_empty() {
        return String::new();
    }
    format!(
        " Every value satisfies {}.",
        list(&invariants.iter().map(|it| code(&it.statement)).collect())
    )
}

/// One field, with the two things its type does not say.
fn field_bullet(field: &ResolvedField) -> String {
    let mut out = format!("- `{}` — `{}`", field.name, field.type_ref);
    if field.type_ref.is_optional() {
        out.push_str(", which may be absent");
    }
    if let Some(wire) = &field.naming.wire {
        if wire != &field.name {
            let _ = write!(out, ", called `{wire}` on the wire");
        }
    }
    if let Some(display) = &field.naming.display {
        let _ = write!(out, ", shown as \"{display}\"");
    }
    out
}

/// Which command and branch — or which binding's escalation — causes an event.
///
/// A binding is the second way an event happens. Leaving it out would print "no command in this
/// system emits it, so something outside the specification does" on the page of an event this
/// specification is the only possible source of, which is the reverse of the truth.
fn emitters(ir: &EssIr, event: &ResolvedEvent) -> Vec<String> {
    let mut out = Vec::new();
    for binding in ir.bindings.values() {
        if let ResolvedFailure::Escalate { emits } = binding.on_failure() {
            if emits.name() == &event.name {
                out.push(format!(
                    "Emitted when binding `{}` escalates: `{}` failed and a person was told.",
                    binding.name,
                    ir.command(&binding.command).name
                ));
            }
        }
    }
    for command in ir.commands.values() {
        let branches: Vec<_> = command
            .outcomes
            .iter()
            .filter(|outcome| outcome.emits.iter().any(|it| it.name() == &event.name))
            .map(|outcome| code(outcome.name.as_str()))
            .collect();
        if !branches.is_empty() {
            out.push(format!(
                "Emitted by `{}` on its {} {}.",
                command.name,
                list(&branches),
                plural_bare(branches.len(), "outcome")
            ));
        }
    }
    if out.is_empty() {
        out.push(
            "No command in this system emits it, so something outside the specification does."
                .to_owned(),
        );
    }
    out
}

/// Which command and branch reports an error.
fn reporters(ir: &EssIr, error: &ResolvedError) -> Vec<String> {
    let mut out = Vec::new();
    for command in ir.commands.values() {
        let branches: Vec<_> = command
            .outcomes
            .iter()
            .filter(|outcome| {
                outcome
                    .error
                    .as_ref()
                    .is_some_and(|it| it.name() == &error.name)
            })
            .map(|outcome| code(outcome.name.as_str()))
            .collect();
        if !branches.is_empty() {
            out.push(format!(
                "Reported by `{}` on its {} {}.",
                command.name,
                list(&branches),
                plural_bare(branches.len(), "outcome")
            ));
        }
    }
    if out.is_empty() {
        out.push(
            "No outcome in this system reports it: it is declared and unreachable.".to_owned(),
        );
    }
    out
}

/// How many outcomes a command has, said as a person would say it.
fn outcome_count_sentence(count: usize, name: &QualifiedName) -> String {
    match count {
        0 => format!(
            "`{name}` declares no outcomes, so nothing here says what it does or when it refuses."
        ),
        1 => "It has one outcome.".to_owned(),
        _ => format!("It has {} outcomes.", number(count)),
    }
}

// ---- Mermaid ----------------------------------------------------------------------------------

/// A lifecycle as a Mermaid state diagram, from `ess-domain`'s own [`StateMachine`].
///
/// Rendered from the domain type directly rather than from a mirror of it: the machine holds no
/// reference that points outside itself, so a copy would only be a second place for the states to
/// disagree with the transitions.
///
/// Every state appears whether or not a transition touches it: a state with no arrows is a fact
/// about the model, and dropping it would hide exactly the sort of dead end the compiler refuses.
///
/// Each arrow carries the command that takes it, which is the whole of gate G14 as a reader meets
/// it: a lifecycle whose moves have no verbs is a diagram of what may happen with no way to make any
/// of it happen. The commands come from [`EssIr::drivers`] rather than from a name that looks like
/// the transition's — the spelling of a move says nothing about who performs it.
fn state_diagram(lifecycle: &StateMachine, drivers: &[Driver<'_>]) -> String {
    let mut out = String::from("```mermaid\nstateDiagram-v2\n");
    let _ = writeln!(out, "    [*] --> {}", lifecycle.initial);
    for transition in &lifecycle.transitions {
        let label = match takers(drivers, &transition.name).as_slice() {
            // Unreachable for a validated specification — `missing_causation` refuses it — so this
            // draws the arrow rather than hiding it, exactly as an untouched state is drawn.
            [] => transition.name.clone(),
            takers => format!("{} ({})", transition.name, takers.join(", ")),
        };
        for from in &transition.from {
            let _ = writeln!(out, "    {from} --> {}: {label}", transition.to);
        }
    }
    for state in &lifecycle.states {
        if lifecycle.is_terminal(state) {
            let _ = writeln!(out, "    {state} --> [*]");
        } else if !touched(lifecycle, state) {
            // Mermaid draws a bare identifier as an unconnected state, which is precisely what this
            // is: declared, and reached by no move. `StateMachine::validate` refuses one, so this
            // arm only fires for a machine nothing validated — and it draws it rather than hide it.
            let _ = writeln!(out, "    {state}");
        }
    }
    out.push_str("```\n");
    out
}

/// The local names of the commands that take one transition, in the order the IR holds them.
///
/// Local rather than qualified because this goes inside a Mermaid arrow label, where the context is
/// already the entity's own page and a fully qualified name would push the label past the arrow.
fn takers(drivers: &[Driver<'_>], transition: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for driver in drivers {
        if driver.takes(transition) {
            let local = driver.command.name.local().to_owned();
            if !out.contains(&local) {
                out.push(local);
            }
        }
    }
    out
}

/// Which command and branch takes each declared move, spelt out beneath the diagram.
///
/// The diagram's labels say *which command*; this says which of its branches, which is the unit a
/// generated scenario is built per. It also states the rule that makes the list exhaustive, so a
/// reader knows an unlisted move is impossible rather than merely undocumented.
fn driven_sentence(lifecycle: &StateMachine, drivers: &[Driver<'_>]) -> String {
    if lifecycle.transitions.is_empty() {
        return "It declares no moves, so nothing changes its state once it exists.".to_owned();
    }
    let mut out = String::from(
        "Each move is taken by a declared command outcome, and a move nothing takes is refused as \
         `missing_causation` rather than left as a state change nobody can trigger:\n\n",
    );
    for transition in &lifecycle.transitions {
        let taken: Vec<String> = drivers
            .iter()
            .filter(|driver| driver.takes(&transition.name))
            .map(|driver| {
                format!(
                    "`{}` on its `{}` outcome",
                    driver.command.name, driver.outcome.name
                )
            })
            .collect();
        let by = if taken.is_empty() {
            "nothing in this specification".to_owned()
        } else {
            list(&taken)
        };
        let _ = writeln!(out, "- `{}` — taken by {by}", transition.name);
    }
    let creators: Vec<String> = drivers
        .iter()
        .filter(|driver| matches!(driver.effect, ResolvedEffect::Creates))
        .map(|driver| {
            format!(
                "`{}` on its `{}` outcome",
                driver.command.name, driver.outcome.name
            )
        })
        .collect();
    let _ = write!(
        out,
        "\n{}",
        if creators.is_empty() {
            "No command here creates one, so an instance arrives from outside this specification."
                .to_owned()
        } else {
            format!(
                "An instance is brought into existence by {}.",
                list(&creators)
            )
        }
    );
    out
}

/// `true` when any transition or the initial state mentions this state.
fn touched(lifecycle: &StateMachine, state: &StateName) -> bool {
    &lifecycle.initial == state
        || lifecycle.transitions.iter().any(|transition| {
            &transition.to == state || transition.from.iter().any(|from| from == state)
        })
}

/// What the diagram above cannot say, and the enumeration that repairs it.
///
/// The model expresses "a paid invoice may not be cancelled" as the *absence* of a transition, and
/// absence does not draw: a missing arrow looks like an arrow nobody has added yet. So the pairs no
/// move connects are listed, derived from the same transitions the diagram is drawn from, which is
/// why the two cannot come apart.
fn legality_note(lifecycle: &StateMachine) -> String {
    if lifecycle.states.len() < 2 {
        return "It has one state, so there is no move to permit or to forbid.\n".to_owned();
    }
    let unconnected = forbidden(lifecycle);
    if unconnected.is_empty() {
        return "Every ordered pair of these states is connected by some move, so this lifecycle \
                forbids nothing.\n"
            .to_owned();
    }
    let mut out = String::from(
        "Illegal transitions are illegal by absence: no rule forbids them, there is simply no \
         arrow, because a rule would be a second place for the same truth to live. A diagram cannot \
         show an absence, so the pairs it does not connect are listed here, derived from the same \
         transitions — anything named below is a move this specification does not permit.\n\n",
    );
    for (from, to) in unconnected {
        let _ = writeln!(out, "- `{from}` may not become `{to}`");
    }
    out.push('\n');
    out
}

/// Every ordered pair of distinct states with no transition between them.
fn forbidden(lifecycle: &StateMachine) -> Vec<(&StateName, &StateName)> {
    let mut out = Vec::new();
    for from in &lifecycle.states {
        for to in &lifecycle.states {
            if from != to && !lifecycle.can_move(from, to) {
                out.push((from, to));
            }
        }
    }
    out
}

/// One binding as a flow: the event, the command, each branch, and where a failure goes.
///
/// A diagram rather than a table because the failure path is the part a table flattens: `escalate`
/// means there is an edge out of this system to a person, and that edge is the whole reason the word
/// is required.
fn binding_flow(ir: &EssIr, binding: &ResolvedBinding) -> String {
    let event = ir.event(&binding.event);
    let command = ir.command(&binding.command);
    let mut out = String::from("```mermaid\nflowchart LR\n");
    let _ = writeln!(out, "    event[\"{}\"]", label(&event.name.to_string()));
    let _ = writeln!(out, "    command[\"{}\"]", label(&command.name.to_string()));
    let _ = writeln!(
        out,
        "    event -->|\"{}\"| command",
        label(binding.name.as_str())
    );
    let mut reached_failure = false;
    for (index, outcome) in command.outcomes.iter().enumerate() {
        let _ = writeln!(
            out,
            "    outcome{index}[\"{}\"]",
            label(outcome.name.as_str())
        );
        let _ = writeln!(out, "    command --> outcome{index}");
        for (emitted, handle) in outcome.emits.iter().enumerate() {
            let _ = writeln!(
                out,
                "    emit{index}_{emitted}[\"{}\"]",
                label(&handle.to_string())
            );
            let _ = writeln!(out, "    outcome{index} --> emit{index}_{emitted}");
        }
        if let Some(handle) = &outcome.error {
            let _ = writeln!(out, "    error{index}[\"{}\"]", label(&handle.to_string()));
            let _ = writeln!(out, "    outcome{index} --> error{index}");
            let _ = writeln!(
                out,
                "    error{index} --> failure[\"{}\"]",
                label(&failure_label(ir, binding))
            );
            reached_failure = true;
        }
    }
    // The edge the whole diagram is for. `escalate` is a hand-off out of this system, and the event
    // is the only mark it leaves inside it — so a reader who cannot see the event on the page cannot
    // tell an escalation from nothing happening.
    if let (true, ResolvedFailure::Escalate { emits }) = (reached_failure, binding.on_failure()) {
        let _ = writeln!(
            out,
            "    escalation[\"{}\"]",
            label(&ir.event(emits).name.to_string())
        );
        let _ = writeln!(out, "    failure --> escalation");
    }
    out.push_str("```\n");
    out
}

/// Where a failed binding's work goes, in a few words for a diagram node.
fn failure_label(ir: &EssIr, binding: &ResolvedBinding) -> String {
    match binding.on_failure() {
        ResolvedFailure::Retry => "retried by the transport".to_owned(),
        ResolvedFailure::Escalate { emits } => {
            format!("escalated to a person, emitting {}", ir.event(emits).name)
        }
        ResolvedFailure::Drop => "dropped: the work is lost".to_owned(),
    }
}

/// The whole system: actors, and the commands and events each component declares.
///
/// The diagram is [`SystemGraph`]'s, fenced. The graph itself is not read here: `protocol ess
/// graph` publishes the same picture, and a second reading of the IR in this file is how the two
/// came to be different graphs wearing one name — see [`crate::graph`] for what they disagreed
/// about. The fence is this page's furniture and the only thing added.
fn system_graph(ir: &EssIr) -> String {
    format!("```mermaid\n{}```\n", SystemGraph::of(ir).mermaid())
}

// ---- plumbing ---------------------------------------------------------------------------------

/// A page, with the provenance a reader can see and the provenance a tool can read.
fn page(path: String, title: &str, body: &str, sliced: &SlicedProvenance) -> Artifact {
    let provenance = &sliced.provenance;
    let mut contents = provenance_comment(provenance);
    let _ = writeln!(contents, "\n# {title}\n");
    contents.push_str(body);
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&provenance_footer(provenance));
    Artifact::sliced(path, contents, sliced.slice.clone())
}

/// The provenance block every artifact opens with, as one HTML comment.
///
/// Not `Provenance::commented("<!--")`, whose own doc comment offers exactly that prefix for
/// Markdown: a per-line prefix cannot close an HTML comment, so four lines each opening one and none
/// closing it leaves a renderer swallowing the rest of the page. `Provenance::lines` is the part of
/// that API usable here, and the block form is the valid one.
fn provenance_comment(provenance: &Provenance) -> String {
    let mut out = String::from("<!--\n");
    for line in provenance.lines() {
        // `--` is what ends an HTML comment early. Nothing that reaches these lines can contain
        // one — a qualified name has no hyphens, a version is `v` and digits, a digest is hex — and
        // `tests/docs.rs` asserts it, which is cheaper than an escape no reader could decode.
        let _ = writeln!(out, "{line}");
    }
    out.push_str("-->\n");
    out
}

/// The same four facts, visible.
///
/// Duplicated on purpose: the comment above is for a tool and a diff, and it is invisible to exactly
/// the person who is about to edit a generated file by hand and lose the work.
fn provenance_footer(provenance: &Provenance) -> String {
    format!(
        "\n---\n\nGenerated from {} {} · model digest `{}` · compiler {} · generator {}. Do not \
         edit this file; change the specification and regenerate it with `protocol ess generate`.\n",
        provenance.system,
        provenance.specification_version,
        provenance.source_digest,
        provenance.compiler_version,
        provenance.generator_version,
    )
}

/// The known gaps as a table, under a heading the page chooses, or nothing when there are none.
///
/// Nothing, rather than an empty table under its heading: a section that says "what this cannot
/// show" and then shows an empty table teaches a reader to skip it, and the day it has a row in it
/// is the day that habit costs something.
fn gap_table(heading: &str, preamble: &str) -> String {
    if Docs::known_gaps().is_empty() {
        return String::new();
    }
    let mut out = format!("{heading}\n\n{preamble}\n\n");
    let _ = writeln!(
        out,
        "| construct | what is dropped | where it would go | what it needs |"
    );
    let _ = writeln!(out, "|---|---|---|---|");
    for gap in Docs::known_gaps() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            gap.construct,
            cell(gap.dropped),
            gap.page,
            cell(gap.needs)
        );
    }
    out
}

/// The path of a bounded context's page, relative to the projection's own directory.
fn domain_path(name: &QualifiedName) -> String {
    format!("domains/{}", domain_file(name))
}

/// The file name of a bounded context's page, which is also the link between two of them.
fn domain_file(name: &QualifiedName) -> String {
    format!("{name}.md")
}

/// The enum each of a context's entities forms from its lifecycle.
///
/// Structural: a handle equal to some [`ResolvedEntity::state_type`], not a name with `State` read
/// out of it. Intersected with the context's own types, so the count and the section that skips
/// these cannot disagree.
fn state_types<'a>(ir: &'a EssIr, domain: &'a ResolvedDomain) -> BTreeSet<&'a TypeHandle> {
    domain
        .entities
        .iter()
        .map(|handle| &ir.entity(handle).state_type)
        .filter(|state| domain.types.contains(*state))
        .collect()
}

/// A link from a bounded context's page to a construct's own section.
///
/// Its own page means a bare fragment, and another context's page means the sibling file: writing
/// `domains/billing.invoice.md#invoice` from inside `domains/billing.invoice.md` would be a second
/// spelling of one place, and the second spelling is the one that rots.
fn section_link(
    ir: &EssIr,
    from: &ResolvedDomain,
    name: &QualifiedName,
    owner: &ess_compiler::ir::DomainHandle,
) -> String {
    let owner = ir.domain(owner);
    let anchor = slug(&relative(name, &owner.name));
    if owner.name == from.name {
        format!("[`{}`](#{anchor})", relative(name, &from.name))
    } else {
        format!("[`{name}`]({}#{anchor})", domain_file(&owner.name))
    }
}

/// A name with its context's prefix removed, so a page does not repeat its own title on every line.
fn relative(name: &QualifiedName, domain: &QualifiedName) -> String {
    name.segments()
        .strip_prefix(domain.segments())
        .map_or_else(|| name.to_string(), |rest| rest.join("."))
}

/// `true` when either end of a reference is declared inside a context.
fn touches(reference: &ess_compiler::ir::ResolvedTypeRef, domain: &QualifiedName) -> bool {
    reference
        .named_leaves()
        .iter()
        .any(|handle| handle.name().is_within(domain))
}

/// The bindings that rely on a crossing, and the input each of them fills with it.
fn crossing_users(ir: &EssIr, conversion: &ResolvedConversion) -> Vec<String> {
    let mut out = Vec::new();
    for binding in ir.bindings.values() {
        for mapping in &binding.mapping {
            let crossed = matches!(
                &mapping.value,
                ResolvedMappingValue::EventField { type_ref, .. }
                    if type_ref == &conversion.from && mapping.target_type == conversion.to
            );
            if crossed {
                out.push(format!(
                    "[`{}`](interactions.md#{}), filling `{}`",
                    binding.name,
                    slug(binding.name.as_str()),
                    mapping.target
                ));
            }
        }
    }
    out
}

/// Events no binding reacts to.
fn unread_events(ir: &EssIr) -> Vec<&QualifiedName> {
    let reactions = ir.reactions();
    ir.events
        .keys()
        .filter(|name| !reactions.keys().any(|handle| handle.name() == *name))
        .collect()
}

/// The display name, or the last segment when nothing overrides it.
fn display_of<'a>(naming: &'a Naming, name: &'a QualifiedName) -> &'a str {
    naming.display_or(name)
}

/// A phrase in backticks.
fn code(text: &str) -> String {
    format!("`{text}`")
}

/// A blockquote, so a quoted reason reads as somebody's words rather than as this page's.
fn quote(text: &str) -> String {
    text.lines()
        .map(|line| format!("> {}", line.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Text safe inside a Markdown table cell.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// An English list: `a`, `a and b`, `a, b and c`.
fn list(items: &Vec<String>) -> String {
    match items.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [head @ .., last] => format!("{} and {last}", head.join(", ")),
    }
}

/// A count and its noun, agreeing.
fn plural(count: usize, noun: &str) -> String {
    format!("{} {}", number(count), plural_bare(count, noun))
}

/// A noun, agreeing with a count that is printed elsewhere.
fn plural_bare(count: usize, noun: &str) -> String {
    if count == 1 {
        return noun.to_owned();
    }
    match noun.strip_suffix('y') {
        // `entity` becomes `entities`, by the general rule for a consonant before the `y` — because
        // "two entitys" in an index is the sort of detail that makes a reader distrust the numbers
        // beside it.
        Some(stem) if stem.ends_with(|it: char| !"aeiou".contains(it)) => format!("{stem}ies"),
        _ => format!("{noun}s"),
    }
}

/// A phrase that has become the start of a sentence.
fn capitalise(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(characters).collect(),
    }
}

/// A small number as a word, because a sentence with a digit in it reads like a form.
fn number(count: usize) -> String {
    match count {
        0 => "no".to_owned(),
        1 => "one".to_owned(),
        2 => "two".to_owned(),
        3 => "three".to_owned(),
        4 => "four".to_owned(),
        5 => "five".to_owned(),
        6 => "six".to_owned(),
        7 => "seven".to_owned(),
        8 => "eight".to_owned(),
        9 => "nine".to_owned(),
        other => other.to_string(),
    }
}

/// The anchor a Markdown renderer derives from a heading.
///
/// Lowercased, spaces hyphenated, everything else dropped — the rule GitHub applies. Computed rather
/// than guessed because a link to `#createinvoice` that should have been `#create-invoice` fails
/// silently: the page opens, at the wrong place.
fn slug(heading: &str) -> String {
    let mut out = String::with_capacity(heading.len());
    for character in heading.chars() {
        if character.is_ascii_alphanumeric() {
            out.extend(character.to_lowercase());
        } else if character == '-' || character == '_' {
            out.push(character);
        } else if character == ' ' {
            out.push('-');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use ess_domain::entity::Transition;

    /// A state name, or a panic naming the spelling that is not one.
    fn state(name: &str) -> StateName {
        StateName::new(name).unwrap_or_else(|error| panic!("`{name}` is a state name: {error}"))
    }

    /// A machine of `ess-domain`'s own type, so the renderer is exercised against what the IR hands
    /// it rather than against a fixture shaped to suit it.
    fn machine(
        initial: &str,
        states: &[&str],
        terminal: &[&str],
        transitions: Vec<Transition>,
    ) -> StateMachine {
        StateMachine {
            states: states.iter().map(|it| state(it)).collect(),
            initial: state(initial),
            terminal: terminal.iter().map(|it| state(it)).collect(),
            transitions,
        }
    }

    /// One move, or a panic naming the transition that is not one.
    fn moves(name: &str, from: &[&str], to: &str) -> Transition {
        Transition::new(name, from.iter().map(|it| state(it)), state(to))
            .unwrap_or_else(|error| panic!("`{name}` is a transition name: {error}"))
    }

    /// The billing example's lifecycle, as `examples/billing/domains/invoice.yaml` declares it.
    ///
    /// The same shape `ResolvedEntity::lifecycle` carries, built here so the diagram's expected
    /// output is asserted without compiling a specification — `tests/docs.rs` does that over the
    /// example itself.
    fn invoice_lifecycle() -> StateMachine {
        machine(
            "Draft",
            &["Draft", "Issued", "Paid", "Cancelled"],
            &["Paid", "Cancelled"],
            vec![
                moves("issue", &["Draft"], "Issued"),
                moves("settle", &["Issued"], "Paid"),
                moves("cancel", &["Draft", "Issued"], "Cancelled"),
            ],
        )
    }

    #[test]
    fn a_lifecycle_renders_as_a_state_diagram_with_its_initial_and_terminal_states_marked() {
        let diagram = state_diagram(&invoice_lifecycle(), &[]);

        assert!(
            diagram.starts_with("```mermaid\nstateDiagram-v2\n"),
            "{diagram}"
        );
        assert!(diagram.contains("    [*] --> Draft\n"), "{diagram}");
        assert!(
            diagram.contains("    Draft --> Issued: issue\n"),
            "{diagram}"
        );
        assert!(
            diagram.contains("    Issued --> Paid: settle\n"),
            "{diagram}"
        );
        assert!(
            diagram.contains("    Draft --> Cancelled: cancel\n"),
            "{diagram}"
        );
        assert!(
            diagram.contains("    Issued --> Cancelled: cancel\n"),
            "{diagram}"
        );
        assert!(diagram.contains("    Paid --> [*]\n"), "{diagram}");
        assert!(diagram.contains("    Cancelled --> [*]\n"), "{diagram}");
    }

    #[test]
    fn a_transition_from_two_states_draws_one_arrow_from_each() {
        let diagram = state_diagram(&invoice_lifecycle(), &[]);

        assert_eq!(
            diagram.matches("cancel").count(),
            2,
            "`cancel` leaves both Draft and Issued: {diagram}"
        );
    }

    #[test]
    fn a_state_no_transition_touches_is_still_drawn() {
        // `StateMachine::validate` refuses an unreachable state, so this machine cannot come out of
        // a compiled specification. It is rendered anyway: a projection that silently dropped the
        // state would hide exactly the dead end the compiler exists to refuse, and the diagram is
        // the artifact somebody looks at when asking why the refusal happened.
        let stranded = machine("Draft", &["Draft", "Void"], &[], Vec::new());

        let diagram = state_diagram(&stranded, &[]);

        assert!(diagram.contains("    [*] --> Draft\n"), "{diagram}");
        assert!(diagram.contains("    Void\n"), "{diagram}");
    }

    #[test]
    fn the_page_names_every_transition_the_specification_does_not_permit() {
        let note = legality_note(&invoice_lifecycle());

        // The example's own headline case: a paid invoice may not be cancelled, and the model says
        // so by not saying anything.
        assert!(note.contains("`Paid` may not become `Cancelled`"), "{note}");
        assert!(note.contains("`Cancelled` may not become `Paid`"), "{note}");
        assert!(note.contains("`Draft` may not become `Paid`"), "{note}");
        assert!(
            !note.contains("`Draft` may not become `Issued`"),
            "that transition exists: {note}"
        );
    }

    #[test]
    fn a_lifecycle_with_one_state_forbids_nothing_rather_than_forbidding_everything() {
        // A single state is the only zero-transition machine `StateMachine::validate` accepts, and
        // the complement of nothing over one state is nothing. Listing "may not become" pairs here
        // would be inventing a prohibition out of an empty set.
        let single = machine("Draft", &["Draft"], &["Draft"], Vec::new());

        let note = legality_note(&single);

        assert!(!note.contains("may not become"), "{note}");
        assert!(note.contains("one state"), "{note}");
    }

    #[test]
    fn a_lifecycle_that_connects_every_pair_says_it_forbids_nothing() {
        let open = machine(
            "Draft",
            &["Draft", "Paid"],
            &[],
            vec![
                moves("settle", &["Draft"], "Paid"),
                moves("reopen", &["Paid"], "Draft"),
            ],
        );

        let note = legality_note(&open);

        // The distinction the page has to keep: "nothing is forbidden" and "nothing was carried"
        // read the same to a reader and are opposite statements about the model.
        assert!(!note.contains("may not become"), "{note}");
        assert!(note.contains("forbids nothing"), "{note}");
    }

    #[test]
    fn a_heading_and_its_anchor_agree() {
        assert_eq!(slug("`CreateInvoice`"), "createinvoice");
        assert_eq!(slug("Create invoice"), "create-invoice");
        assert_eq!(
            slug("notify-on-invoice-created"),
            "notify-on-invoice-created"
        );
        assert_eq!(slug("Invoice.State"), "invoicestate");
    }

    #[test]
    fn a_list_of_three_reads_as_a_person_would_write_it() {
        assert_eq!(list(&vec![]), "");
        assert_eq!(list(&vec!["a".to_owned()]), "a");
        assert_eq!(list(&vec!["a".to_owned(), "b".to_owned()]), "a and b");
        assert_eq!(
            list(&vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]),
            "a, b and c"
        );
    }

    #[test]
    fn a_gap_that_ships_says_which_crate_closes_it() {
        // The allowlist is empty: every construct a specification declares reaches the IR and
        // reaches a page. The rule each future entry has to satisfy is asserted anyway, because the
        // day someone adds one is the day nobody is reading this file.
        for gap in Docs::known_gaps() {
            assert!(
                gap.needs.contains("ess-compiler"),
                "a gap says which crate closes it, or nobody closes it: {gap:?}"
            );
            assert!(
                !gap.page.is_empty(),
                "a gap nobody is told where to look for is a gap nobody looks for: {gap:?}"
            );
        }
    }

    #[test]
    fn a_plural_of_entity_is_entities() {
        assert_eq!(plural(1, "entity"), "one entity");
        assert_eq!(plural(2, "entity"), "two entities");
        assert_eq!(plural(0, "view"), "no views");
    }
}
