//! The entity renderer: data, typed lifecycle states, and the boundary between runtime and typed
//! state.
//!
//! **This is where the second target had the most to prove.** Rust carries the lifecycle in a type
//! parameter — `Invoice<S>` with a sealed marker per state — and Go cannot: a method may not be
//! declared for one instantiation of a generic type, so `Invoice[Draft].Issue()` has no spelling.
//!
//! What Go *can* do is the same guarantee by a different construction: **one distinct type per
//! declared state**, each holding the data in an unexported field, with a transition emitted as a
//! method on exactly the states the specification declares it starts from, returning the type of
//! the state it ends in. An undeclared move is then not an error case — it is a method that does
//! not exist, which is precisely what the Rust emitter achieves and precisely what the model
//! means by "a move nobody declared is a move nobody may make".
//!
//! Two differences remain, and both are recorded as target weakenings rather than smoothed over:
//!
//! * Go's zero value needs no constructor, so `InvoiceIssued{}` is spellable even though nothing
//!   issued it. The unexported field keeps such a value empty; nothing keeps it from existing.
//! * Refinement from a runtime state cannot be total, because the snapshot's state field is a
//!   sealed interface whose zero value is nil and names no declared state. `Refine` therefore
//!   answers `(value, ok)`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ess_compiler::ir::ResolvedEntity;
use ess_domain::entity::StateName;

use super::{field_line, items, name, Emit, EXHAUSTIVENESS_NOTE};

/// Everything one entity contributes to its package, in reading order: data, one type per state
/// with its transitions, then the runtime boundary.
pub(super) fn lifecycle(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity) {
    data_struct(out, emit, entity);
    states(out, emit, entity);
    boundary(out, emit, entity);
}

/// The identity and every declared field — everything except where the instance is in its
/// lifecycle, which is carried by the type or by the snapshot, never duplicated as a field.
fn data_struct(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity) {
    let data = emit.layout.data(&entity.name);
    let type_name = emit.layout.declared(&entity.name);
    let _ = writeln!(
        out,
        "\n// {data} is what {} — `{}` — holds, apart from where it is in its \
         lifecycle.\n//\n// The identity and every declared field. The state is deliberately not \
         one: inside the domain\n// it is carried by the type ([{type_name}{}] and its siblings), \
         and at a boundary by [{}].",
        entity.naming.display_or(&entity.name),
        entity.name,
        entity.lifecycle.initial,
        emit.layout.snapshot(&entity.name)
    );
    items::invariant_doc(out, &entity.invariants);
    let _ = writeln!(out, "type {data} struct {{");
    let mut taken = BTreeMap::new();
    let identity = items::field_ident(&mut taken, &entity.identity.name);
    let _ = writeln!(
        out,
        "\t// {identity} is the identity: `{}` — `{}`.\n\t{identity} {}",
        entity.identity.name,
        entity.identity.type_ref,
        emit.go_type(&entity.identity.type_ref)
    );
    for field in &entity.fields {
        field_line(out, emit, &mut taken, field);
    }
    out.push_str("}\n");
}

/// One type per declared state, its accessors, its transitions, and — on the initial state only —
/// the constructor.
fn states(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity) {
    let data = emit.layout.data(&entity.name);
    let state_enum = emit.layout.declared(entity.state_type.name());
    let any = emit.layout.any(&entity.name);
    let snapshot = emit.layout.snapshot(&entity.name);

    for state in &entity.lifecycle.states {
        let resting = emit.layout.state(&entity.name, state.as_str());
        let value = emit
            .layout
            .variant(entity.state_type.name(), state.as_str());
        let _ = writeln!(
            out,
            "\n// {resting} is `{}` resting in `{state}`.{}\n//\n// One type per declared state: a \
             transition is a method on exactly the states the\n// specification declares it starts \
             from, so an undeclared move is a method that does not\n// exist. The field is \
             unexported — the only way to reach a state is the constructor or a\n// declared move \
             (see TARGET.md for what Go's zero value still permits).\ntype {resting} struct \
             {{\n\tdata {data}\n}}",
            entity.name,
            state_note(entity, state)
        );

        if *state == entity.lifecycle.initial {
            let ctor = emit.layout.ctor(&entity.name);
            let _ = writeln!(
                out,
                "\n// {ctor} starts a new `{}` in `{state}` — the only state the lifecycle starts \
                 one in.\nfunc {ctor}(data {data}) {resting} {{\n\treturn {resting}{{data: \
                 data}}\n}}",
                entity.name
            );
        }

        let _ = writeln!(
            out,
            "\n// State is the state this instance rests in, as the runtime value.\nfunc \
             ({resting}) State() {state_enum} {{\n\treturn {value}{{}}\n}}\n\n// Data is what it \
             holds.\nfunc (v {resting}) Data() {data} {{\n\treturn v.data\n}}\n\n// Snapshot is \
             this instance at a boundary: the state as a value beside the data.\nfunc (v \
             {resting}) Snapshot() {snapshot} {{\n\treturn {snapshot}{{State: {value}{{}}, Data: \
             v.data}}\n}}\n\nfunc ({resting}) {}() {{}}",
            name::marker(any)
        );

        for transition in entity.lifecycle.outgoing(state) {
            let method = name::exported(&transition.name);
            let arrives = emit.layout.state(&entity.name, transition.to.as_str());
            let _ = writeln!(
                out,
                "\n// {method} takes `{}` — `{state}` → `{}`.{}\nfunc (v {resting}) {method}() \
                 {arrives} {{\n\treturn {arrives}{{data: v.data}}\n}}",
                transition.name,
                transition.to,
                drivers_note(emit, entity, &transition.name)
            );
        }
    }
}

/// What the lifecycle says about one state, for its type's doc line.
fn state_note(entity: &ResolvedEntity, state: &StateName) -> &'static str {
    if *state == entity.lifecycle.initial {
        " Where a new instance starts."
    } else if entity.lifecycle.terminal.contains(state) {
        " Terminal: an instance may rest here forever."
    } else {
        ""
    }
}

/// Which command outcomes take a transition, for its method's doc line — so the generated API says
/// who calls it, instead of leaving the reader to grep the specification.
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

/// The runtime boundary: the any-state interface, the snapshot, and the refinement between them.
fn boundary(out: &mut String, emit: &Emit<'_>, entity: &ResolvedEntity) {
    let data = emit.layout.data(&entity.name);
    let state_enum = emit.layout.declared(entity.state_type.name());
    let any = emit.layout.any(&entity.name);
    let snapshot = emit.layout.snapshot(&entity.name);

    let _ = writeln!(
        out,
        "\n// {any} is an instance of `{}` in whichever declared state it was found.\n//",
        entity.name
    );
    out.push_str(EXHAUSTIVENESS_NOTE);
    let _ = writeln!(
        out,
        "type {any} interface {{\n\t{}()\n\n\t// State is the state this instance rests in.\n\t\
         State() {state_enum}\n\n\t// Snapshot is this instance at a boundary.\n\tSnapshot() \
         {snapshot}\n}}",
        name::marker(any)
    );

    let _ = writeln!(
        out,
        "\n// {snapshot} is `{}` as it crosses a boundary: the state as a value beside the \
         data.\n//\n// Wire and storage know states only at runtime; [{snapshot}.Refine] is the \
         one door back into\n// the typed lifecycle.\ntype {snapshot} struct {{\n\t// State is \
         where the instance is in its lifecycle.\n\tState {state_enum}\n\t// Data is what it \
         holds.\n\tData {data}\n}}",
        entity.name
    );

    let _ = writeln!(
        out,
        "\n// Refine refines the runtime state into the typed one.\n//\n// Rust's is total, and \
         this one cannot be: the state is a sealed interface, whose zero\n// value is nil and \
         names no declared state, so a snapshot nothing constructed reaches here.\n// `ok` is \
         false for exactly that snapshot and for no other — every declared state has an\n// arm \
         (see TARGET.md).\nfunc (v {snapshot}) Refine() ({any}, bool) {{\n\tswitch \
         v.State.(type) {{"
    );
    for state in &entity.lifecycle.states {
        let value = emit
            .layout
            .variant(entity.state_type.name(), state.as_str());
        let resting = emit.layout.state(&entity.name, state.as_str());
        let _ = writeln!(
            out,
            "\tcase {value}:\n\t\treturn {resting}{{data: v.Data}}, true"
        );
    }
    out.push_str("\t}\n\treturn nil, false\n}\n");
}
