//! The entity renderer: data, typed lifecycle states, and the boundary between runtime and typed
//! state.
//!
//! This is the slice's load-bearing piece: **the compiler refuses the transition the specification
//! refuses**. The mechanism is ordinary typestate — one sealed marker type per declared state, the
//! entity generic over it, one method per declared transition on exactly the states it starts
//! from, and one constructor, on the initial state. An illegal move is then not an error case but
//! a method that does not exist, which is the same shape `ess-compiler` gives an unresolved
//! reference: unrepresentable rather than checked.
//!
//! Runtime state uses the hybrid the design's §7 sketches, because wire and storage know states
//! only as values: a snapshot type carries the state enum beside the data, and `refine` is the one
//! door from value-state back into type-state. Both directions are total — every declared state
//! has an arm — and no other door exists, because the marker types are sealed and the typed
//! entity's fields are private.

use std::fmt::Write as _;

use ess_compiler::ir::ResolvedEntity;
use ess_domain::entity::StateName;

use super::{items, name, Emit};

/// Everything one entity contributes to its module, in reading order: data, markers, the typed
/// entity, its transitions, then the runtime boundary.
pub(super) fn lifecycle(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity) {
    let context = Entity::of(emit, entity);
    data_struct(out, emit, entity, &context);
    state_module(out, entity, &context);
    typed_entity(out, entity, &context);
    transitions(out, emit, entity, &context);
    boundary(out, entity, &context);
}

/// The derived names one entity's items share.
struct Entity {
    /// The entity's own type name — `Invoice`.
    type_name: String,
    /// The runtime state enum — `InvoiceState`.
    state_enum: String,
    /// The marker module — `invoice_state`.
    state_module: String,
}

impl Entity {
    fn of(emit: &Emit<'_>, entity: &ResolvedEntity) -> Self {
        let type_name = emit.layout.type_name(&entity.name);
        Self {
            state_enum: emit.layout.type_name(entity.state_type.name()),
            state_module: format!("{}_state", name::value_ident(&type_name)),
            type_name,
        }
    }
}

/// The identity and every declared field — everything except where the instance is in its
/// lifecycle, which is carried by the type or by the snapshot, never duplicated as a field.
fn data_struct(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity, context: &Entity) {
    let _ = writeln!(
        out,
        "\n/// What {} — `{}` — holds, apart from where it is in its lifecycle.\n///\n/// The \
         identity and every declared field. The state is deliberately not one: inside the domain \
         it\n/// is carried by the type parameter of [`{}<S>`], and at a boundary by \
         [`{}Snapshot::state`].",
        entity.naming.display_or(&entity.name),
        entity.name,
        context.type_name,
        context.type_name
    );
    items::invariant_doc(out, &entity.invariants);
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct {}Data {{",
        context.type_name
    );
    let _ = writeln!(
        out,
        "    /// The identity: `{}` — `{}`.\n    pub {}: {},",
        entity.identity.name,
        entity.identity.type_ref,
        name::value_ident(&entity.identity.name),
        emit.rust_type(&entity.identity.type_ref)
    );
    for field in &entity.fields {
        let _ = writeln!(
            out,
            "    /// `{}` — `{}`.\n    pub {}: {},",
            field.name,
            field.type_ref,
            name::value_ident(&field.name),
            emit.rust_type(&field.type_ref)
        );
    }
    out.push_str("}\n");
}

/// The marker module: one sealed type per declared state, each knowing its runtime value.
fn state_module(out: &mut String, entity: &ResolvedEntity, context: &Entity) {
    let _ = writeln!(
        out,
        "\n/// The states of `{}`, at the type level.\n///\n/// One marker type per declared \
         state, sealed: a state the lifecycle does not declare cannot\n/// implement \
         [`Marker`]({}::Marker), so [`{}<S>`]({}) can only ever rest in a real state.\npub mod {} \
         {{",
        entity.name,
        context.state_module,
        context.type_name,
        context.type_name,
        context.state_module
    );

    out.push_str("    /// Closes [`Marker`] over the declared states.\n    mod sealed {\n        /// Implemented only by the marker types beside this module.\n        pub trait Sealed {}\n");
    for state in &entity.lifecycle.states {
        let _ = writeln!(out, "        impl Sealed for super::{state} {{}}");
    }
    out.push_str("    }\n\n");

    let _ = writeln!(
        out,
        "    /// A declared state of `{}`, as a type.\n    pub trait Marker: sealed::Sealed \
         {{\n        /// The same state, as the runtime value.\n        const STATE: \
         super::{};\n    }}",
        context.type_name, context.state_enum
    );

    for state in &entity.lifecycle.states {
        let _ = writeln!(out, "\n    /// `{state}`.{}", state_note(entity, state));
        let _ = writeln!(
            out,
            "    pub struct {state};\n\n    impl Marker for {state} {{\n        const STATE: \
             super::{} = super::{}::{state};\n    }}",
            context.state_enum, context.state_enum
        );
    }
    out.push_str("}\n");
}

/// What the lifecycle says about one state, for its marker's doc line.
fn state_note(entity: &ResolvedEntity, state: &StateName) -> &'static str {
    if *state == entity.lifecycle.initial {
        " Where a new instance starts."
    } else if entity.lifecycle.terminal.contains(state) {
        " Terminal: an instance may rest here forever."
    } else {
        ""
    }
}

/// The typed entity, its state accessors, and the one constructor — on the initial state.
fn typed_entity(out: &mut String, entity: &ResolvedEntity, context: &Entity) {
    let Entity {
        type_name,
        state_enum,
        state_module,
    } = context;
    let _ = writeln!(
        out,
        "\n/// {} — `{}` — with its lifecycle state carried by the type.\n///\n/// The one \
         constructor rests in `{}`, and the only way to change `S` is a method generated from\n/// \
         a declared transition. A move the specification does not declare is therefore not an \
         error\n/// case: it does not compile. Where the state is data — wire, storage — use \
         [`{}Snapshot`]\n/// and [`{}Snapshot::refine`].",
        entity.naming.display_or(&entity.name),
        entity.name,
        entity.lifecycle.initial,
        type_name,
        type_name
    );
    let _ = writeln!(
        out,
        "pub struct {type_name}<S: {state_module}::Marker> {{\n    data: \
         {type_name}Data,\n    state: core::marker::PhantomData<S>,\n}}"
    );

    let _ = writeln!(
        out,
        "\nimpl<S: {state_module}::Marker> {type_name}<S> {{\n    /// The state this instance \
         rests in, as the runtime value.\n    pub fn state(&self) -> {state_enum} {{\n        \
         S::STATE\n    }}\n\n    /// What it holds.\n    pub fn data(&self) -> &{type_name}Data \
         {{\n        &self.data\n    }}\n\n    /// Hands the data back, giving up the typed \
         state.\n    pub fn into_data(self) -> {type_name}Data {{\n        self.data\n    }}\n}}"
    );

    let _ = writeln!(
        out,
        "\nimpl {type_name}<{state_module}::{}> {{\n    /// A new instance, resting in `{}` — the \
         only state the lifecycle starts one in.\n    pub fn new(data: {type_name}Data) -> Self \
         {{\n        Self {{\n            data,\n            state: \
         core::marker::PhantomData,\n        }}\n    }}\n}}",
        entity.lifecycle.initial, entity.lifecycle.initial
    );
}

/// One `impl` block per state that has outgoing moves, holding exactly the declared transitions
/// out of it. This is where the specification's refusals become the compiler's: a state with no
/// declared move out has no block, and no block holds a method its state does not start.
fn transitions(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity, context: &Entity) {
    let Entity {
        type_name,
        state_module,
        ..
    } = context;
    for state in &entity.lifecycle.states {
        let outgoing = entity.lifecycle.outgoing(state);
        if outgoing.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\nimpl {type_name}<{state_module}::{state}> {{");
        for (position, transition) in outgoing.iter().enumerate() {
            if position > 0 {
                out.push('\n');
            }
            let _ = writeln!(
                out,
                "    /// `{}` — `{state}` → `{}`.{}\n    pub fn {}(self) -> \
                 {type_name}<{state_module}::{}> {{\n        {type_name} {{\n            data: \
                 self.data,\n            state: core::marker::PhantomData,\n        }}\n    }}",
                transition.name,
                transition.to,
                drivers_note(emit, entity, &transition.name),
                name::value_ident(&transition.name),
                transition.to
            );
        }
        out.push_str("}\n");
    }
}

/// Which command outcomes take a transition, for its method's doc line — so the generated API
/// says who calls it, instead of leaving the reader to grep the specification.
fn drivers_note(emit: &Emit<'_>, entity: &ResolvedEntity, transition: &str) -> String {
    let drivers = emit.ir.drivers();
    let Some((_, of_entity)) = drivers
        .iter()
        .find(|(handle, _)| *handle.name() == entity.name)
    else {
        return String::new();
    };
    let takers: Vec<String> = of_entity
        .iter()
        .filter(|driver| driver.takes(transition))
        .map(|driver| {
            format!(
                "the `{}` outcome of `{}`",
                driver.outcome.name, driver.command.name
            )
        })
        .collect();
    if takers.is_empty() {
        return String::new();
    }
    format!(" Taken by {}.", takers.join(", "))
}

/// The runtime boundary: the snapshot, the any-state enum, and the total refinement between them.
fn boundary(out: &mut String, entity: &ResolvedEntity, context: &Entity) {
    let Entity {
        type_name,
        state_enum,
        state_module,
    } = context;

    let _ = writeln!(
        out,
        "\n/// `{}` as it crosses a boundary: the state as a value beside the data.\n///\n/// Wire \
         and storage know states only at runtime; [`{type_name}Snapshot::refine`] is the one door \
         back\n/// into the typed lifecycle.\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct \
         {type_name}Snapshot {{\n    /// Where the instance is in its lifecycle.\n    pub state: \
         {state_enum},\n    /// What it holds.\n    pub data: {type_name}Data,\n}}",
        entity.name
    );

    let _ = writeln!(
        out,
        "\n/// An `{type_name}` in whichever declared state it was found.\npub enum \
         Any{type_name} {{"
    );
    for state in &entity.lifecycle.states {
        let _ = writeln!(
            out,
            "    /// Resting in `{state}`.\n    {state}({type_name}<{state_module}::{state}>),"
        );
    }
    out.push_str("}\n");

    let _ = writeln!(
        out,
        "\nimpl {type_name}Snapshot {{\n    /// Refines the runtime state into the typed one.\n    \
         ///\n    /// Total: every declared state has an arm, and an undeclared state cannot reach \
         here because\n    /// `{state_enum}` cannot spell one.\n    pub fn refine(self) -> \
         Any{type_name} {{\n        match self.state {{"
    );
    for state in &entity.lifecycle.states {
        let _ = writeln!(
            out,
            "            {state_enum}::{state} => Any{type_name}::{state}({type_name} {{"
        );
        out.push_str(
            "                data: self.data,\n                state: \
             core::marker::PhantomData,\n            }),\n",
        );
    }
    out.push_str("        }\n    }\n}\n");

    let _ = writeln!(
        out,
        "\nimpl Any{type_name} {{\n    /// The state, as the runtime value.\n    pub fn \
         state(&self) -> {state_enum} {{\n        match self {{"
    );
    for state in &entity.lifecycle.states {
        let _ = writeln!(
            out,
            "            Self::{state}(_) => {state_enum}::{state},"
        );
    }
    out.push_str("        }\n    }\n\n    /// Back to the boundary shape.\n");
    let _ = writeln!(
        out,
        "    pub fn snapshot(self) -> {type_name}Snapshot {{\n        match self {{"
    );
    for state in &entity.lifecycle.states {
        let _ = writeln!(
            out,
            "            Self::{state}(instance) => {type_name}Snapshot {{\n                \
             state: {state_enum}::{state},\n                data: instance.into_data(),"
        );
        out.push_str("            },\n");
    }
    out.push_str("        }\n    }\n}\n");
}
