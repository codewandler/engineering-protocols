//! The bridge crate's root module: the seam a realization is installed through, and the three
//! exports a browser calls.
//!
//! # The boundary is JSON over linear memory, and nothing else
//!
//! Three exports, no code generation on the JavaScript side, and no build-time tool: the page
//! reserves a buffer, writes UTF-8 into it, calls one function and reads the answer back out of
//! the module's own memory. `wasm-bindgen` would have made the glue shorter and cost a
//! cargo-installed binary pinned to a crate version — a build step this repository's gate cannot
//! take, because nothing in `task check` reaches the network.
//!
//! # Nothing traps
//!
//! Every refusal is a value: a malformed request, an unknown command, an input that does not match
//! its declared type, an obligation nobody has satisfied. A WebAssembly trap is invisible to the
//! page beyond "it failed", so the one thing this bridge must never do is panic — the honest
//! answer to every one of those is JSON naming what was wrong.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ess_compiler::ir::{EventHandle, ResolvedBinding, ResolvedFailure};
use ess_domain::component::ComponentName;

use crate::plan::{accepting_components, CapabilityKind, REGENERATE};
use crate::rust::layout::Layout as RustLayout;
use crate::rust::{event_variants, name, port, system as rust_system};

use super::{Bridge, EXPORTS, REALIZE};
use crate::rust::wire::ident;

/// One binding whose delivery the plan generates, and the component it lands on.
struct Delivery<'a> {
    /// The binding.
    binding: &'a ResolvedBinding,
    /// The one component that accepts its command.
    acceptor: &'a ComponentName,
}

/// The emitted `lib.rs`.
pub(super) fn module(bridge: &Bridge<'_>) -> String {
    let mut out = bridge
        .plan
        .provenance
        .commented_for("//", &format!("{REGENERATE} --target {}", super::TARGET));
    out.push('\n');
    header(&mut out, bridge);

    if bridge.system.is_none() {
        // A specification with no component and no binding has no system crate to drive, so there
        // is nothing to install and nothing to dispatch. The catalogue and the wire renderings
        // still stand on their own, and the page says so rather than offering a form that leads
        // nowhere.
        out.push_str(NO_SYSTEM);
        exports(&mut out);
        return out;
    }

    let deliveries = deliveries(bridge);
    error_type(&mut out, bridge);
    bound_trait(&mut out);
    bound_impl(&mut out, bridge, &deliveries);
    unrealized(&mut out, bridge);
    installation(&mut out, bridge);
    serving(&mut out);
    exports(&mut out);
    out
}

/// The module's own documentation and lints.
fn header(out: &mut String, bridge: &Bridge<'_>) {
    let ir = bridge.ir;
    let _ = writeln!(
        out,
        "//! The `{}` system, {}, behind a WebAssembly boundary.",
        ir.system, ir.version
    );
    if let Some(summary) = &ir.summary {
        let _ = writeln!(out, "//!\n//! {}", summary.trim());
    }
    let _ = write!(
        out,
        "//!\n//! Generated, not written: the specification is the source of truth, and the door \
         to changing\n//! anything here is `{REGENERATE} --target {}`. This crate emits no \
         behaviour — every\n//! command behaviour, view projection and escalation is an \
         obligation, listed with its contract\n//! in the `PLAN.md` beside this tree. With \
         nothing installed the page runs against the generated\n//! stubs and every command \
         answers with the typed refusal naming what is owed.\n//!\n//! # No \
         `forbid(unsafe_code)`, and why\n//!\n//! A WebAssembly export is a `#[no_mangle]` item, \
         which rustc's `unsafe_code` lint flags — so the\n//! lint every other generated crate \
         here forbids cannot be declared in this one. There is no\n//! `unsafe` block, no `unsafe \
         fn` and no raw-pointer dereference below; the buffer the page\n//! writes into is an \
         ordinary `Vec<u8>` this module allocated. `TARGET.md` states the\n//! weakening rather \
         than leaving it to be noticed.\n\n#![deny(missing_docs)]\n\n// The exports below pass \
         addresses in a 32-bit linear memory. Built for anything else, that\n// cast would \
         silently narrow, so this crate refuses rather than producing a module nobody\n// can \
         run.\n#[cfg(not(target_family = \"wasm\"))]\ncompile_error!(\n    \"this crate is a \
         browser realization: build it with `--target wasm32-unknown-unknown`\"\n);\n\npub mod \
         catalog;\npub mod json;\npub mod wire;\n\nuse std::cell::RefCell;\n",
        super::TARGET,
    );
}

/// What the module says when the specification declares no interaction layer at all.
const NO_SYSTEM: &str =
    "\n/// Serves one request, answering JSON whatever happens.\n///\n/// This \
                         specification declares no component and no binding, so there is no system \
                         to drive and\n/// no command to send: the catalogue is the whole \
                         surface.\npub fn serve(request: &str) -> String {\n    let mut out = \
                         String::new();\n    out.push('{');\n    match json::parse(request)\n     \
                            .ok()\n        .and_then(|request| \
                         request.member(\"request\").cloned())\n    {\n        \
                         Some(json::Value::Text(kind)) if kind == \"catalog\" => {\n            \
                         json::member(&mut out, \"ok\");\n            json::push_bool(&mut out, \
                         true);\n            json::member(&mut out, \"catalog\");\n            \
                         out.push_str(catalog::CATALOG);\n        }\n        _ => {\n            \
                         json::member(&mut out, \"ok\");\n            json::push_bool(&mut out, \
                         false);\n            json::member(&mut out, \"error\");\n            \
                         out.push('{');\n            json::member(&mut out, \"kind\");\n          \
                           json::push_text(&mut out, \"unknown-request\");\n            \
                         json::member(&mut out, \"detail\");\n            json::push_text(\n      \
                                   &mut out,\n                \"this specification declares no \
                         component and no binding: `catalog` is the only request\",\n            \
                         );\n            out.push('}');\n        }\n    }\n    \
                         out.push('}');\n    out\n}\n";

/// The bindings whose delivery the plan generates, with the port each lands on.
fn deliveries<'a>(bridge: &'a Bridge<'a>) -> Vec<Delivery<'a>> {
    let mut out = Vec::new();
    for binding in bridge.ir.bindings.values() {
        if !bridge
            .plan
            .is_generated(CapabilityKind::BindingDelivery, &binding.name.to_string())
        {
            continue;
        }
        let acceptors = accepting_components(bridge.ir, binding);
        if acceptors.len() == 1 {
            out.push(Delivery {
                binding,
                acceptor: &acceptors[0].name,
            });
        }
    }
    out
}

/// The refusal type: every way a request can fail, each a value the page can render.
fn error_type(out: &mut String, bridge: &Bridge<'_>) {
    let types = &bridge.types;
    let _ = write!(
        out,
        "\n/// Why a request could not be served.\n///\n/// Every variant is a *value*: a \
         WebAssembly trap tells a page nothing beyond \"it failed\", so\n/// nothing below panics \
         and nothing below unwraps a caller's input.\n#[derive(Debug, Clone, PartialEq, \
         Eq)]\npub enum BridgeError {{\n    /// The request was not well-formed \
         JSON.\n    Malformed(json::ParseError),\n    /// The request named no kind this bridge \
         serves.\n    UnknownRequest(String),\n    /// The request named a command this system \
         does not declare, or this target cannot \
         dispatch.\n    UnknownCommand(String),\n    /// The input did not match the type the \
         command declares.\n    Undecodable(json::DecodeError),\n    /// An obligation nothing has \
         satisfied was reached — a fact about the realization, never\n    /// about the \
         request.\n    Unmet {{\n        /// The capability kind, as the plan spells \
         it.\n        capability: String,\n        /// The construct that requires it, in the \
         specification's own spelling.\n        source: String,\n    }},\n    /// A redelivery \
         named an occurrence the log does not hold.\n    NoSuchOccurrence(usize),\n}}\n\nimpl \
         BridgeError {{\n    /// Writes the refusal as JSON, naming the kind so a page can react \
         to it rather than\n    /// display it.\n    pub fn encode(&self, out: &mut String) \
         {{\n        out.push('{{');\n        match self {{\n            \
         Self::Malformed(error) => {{\n                json::member(out, \
         \"kind\");\n                json::push_text(out, \"malformed\");\n                \
         json::member(out, \"detail\");\n                json::push_text(out, \
         &error.to_string());\n            }}\n            Self::UnknownRequest(kind) => \
         {{\n                json::member(out, \"kind\");\n                json::push_text(out, \
         \"unknown-request\");\n                json::member(out, \"request\");\n                \
         json::push_text(out, kind);\n            }}\n            Self::UnknownCommand(command) \
         => {{\n                json::member(out, \"kind\");\n                \
         json::push_text(out, \"unknown-command\");\n                json::member(out, \
         \"command\");\n                json::push_text(out, command);\n            }}\n            \
         Self::Undecodable(error) => {{\n                json::member(out, \
         \"kind\");\n                json::push_text(out, \"undecodable\");\n                \
         json::member(out, \"at\");\n                json::push_text(out, \
         &error.at);\n                json::member(out, \"expected\");\n                \
         json::push_text(out, &error.expected);\n                json::member(out, \
         \"found\");\n                json::push_text(out, &error.found);\n            }}\n            \
         Self::Unmet {{ capability, source }} => {{\n                json::member(out, \
         \"kind\");\n                json::push_text(out, \
         \"unmet-obligation\");\n                json::member(out, \
         \"capability\");\n                json::push_text(out, capability);\n                \
         json::member(out, \"source\");\n                json::push_text(out, \
         source);\n            }}\n            Self::NoSuchOccurrence(occurrence) => \
         {{\n                json::member(out, \"kind\");\n                json::push_text(out, \
         \"no-such-occurrence\");\n                json::member(out, \
         \"occurrence\");\n                json::push_integer(out, *occurrence as \
         i64);\n            }}\n        }}\n        out.push('}}');\n    }}\n}}\n\nimpl \
         From<json::DecodeError> for BridgeError {{\n    fn from(error: json::DecodeError) -> Self \
         {{\n        Self::Undecodable(error)\n    }}\n}}\n\nimpl \
         From<{types}::obligation::UnmetObligation> for BridgeError {{\n    fn from(unmet: \
         {types}::obligation::UnmetObligation) -> Self {{\n        Self::Unmet {{\n            \
         capability: unmet.capability.to_owned(),\n            source: \
         unmet.source.to_owned(),\n        }}\n    }}\n}}\n"
    );
}

/// The seam: what the bridge needs of a running system, and nothing about how one was assembled.
fn bound_trait(out: &mut String) {
    out.push_str(
        "\n/// The running system, behind a boundary that erases which realization assembled \
         it.\n///\n/// Implemented once below, generically, over the generated `System` — so a \
         host links its own\n/// implementations of every obligation, hands the assembled system \
         to [`install`], and this\n/// bridge never chooses one. Zero implementations for an \
         obligation is an unsatisfied\n/// obligation and two is an ambiguity; neither is a \
         decision for the machinery (gap register\n/// D-2), so there is no registry and no \
         default beyond the generated stubs that \
         refuse.\npub trait Bound {\n    /// Runs one declared command from its JSON input, then \
         pumps the transport until quiescent.\n    ///\n    /// # Errors\n    ///\n    /// \
         [`BridgeError`] for a command this system does not accept, an input that does not \
         match\n    /// the declared type, or an obligation nothing has satisfied. A *declared* \
         refusal is not an\n    /// error: it comes back as the outcome it is.\n    fn run(&mut \
         self, command: &str, input: &json::Value) -> Result<String, BridgeError>;\n\n    /// \
         Delivers one already-published occurrence again — the duplicate `at_least_once` \
         permits.\n    ///\n    /// # Errors\n    ///\n    /// [`BridgeError::NoSuchOccurrence`] \
         for an index the log does not hold, and\n    /// [`BridgeError::Unmet`] for an \
         obligation redelivery reached.\n    fn replay(&mut self, occurrence: usize) -> \
         Result<(), BridgeError>;\n\n    /// Everything published so far, in publication order — \
         the system's observable record.\n    fn log(&self) -> String;\n\n    /// Every command a \
         binding invoked, with the input it passed.\n    fn invoked(&self) -> String;\n\n    /// \
         Every declared view's rows, or the refusal serving it answered with.\n    fn \
         projected(&self) -> String;\n}\n",
    );
}

/// The blanket implementation over the generated system.
fn bound_impl(out: &mut String, bridge: &Bridge<'_>, deliveries: &[Delivery<'_>]) {
    let ir = bridge.ir;
    let system = bridge.system();
    let types = &bridge.types;
    let layout = bridge.layout.rust();

    let mut generics = rust_system::components_generics(ir);
    let mut bounds: Vec<String> = Vec::new();
    for (component_name, generic) in ir
        .components
        .keys()
        .zip(rust_system::components_generics(ir))
    {
        let component = &ir.components[component_name];
        let list = port::bound_list(ir, layout, component, types);
        if !list.is_empty() {
            bounds.push(format!("    {generic}: {},", list.join(" + ")));
        }
    }
    let mut owed: Vec<String> = Vec::new();
    for delivery in deliveries {
        let pascal = name::pascal(&delivery.binding.name.to_string());
        if !bridge.plan.is_generated(
            CapabilityKind::BindingTransformation,
            &delivery.binding.name.to_string(),
        ) {
            owed.push(format!("{system}::obligations::{pascal}Transformation"));
        }
        if let ResolvedFailure::Escalate { .. } = delivery.binding.on_failure() {
            owed.push(format!("{system}::obligations::{pascal}Escalation"));
        }
    }
    if !owed.is_empty() {
        generics.push("Obligations".to_owned());
        bounds.push(format!("    Obligations: {},", owed.join(" + ")));
    }
    let angled = if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    };
    let where_clause = if bounds.is_empty() {
        String::new()
    } else {
        format!("\nwhere\n{}", bounds.join("\n"))
    };

    let _ = write!(
        out,
        "\nimpl{angled} Bound for {system}::System{angled}{where_clause}\n{{\n"
    );
    run_method(out, bridge);
    replay_method(out, bridge);
    log_method(out, bridge);
    invoked_method(out, bridge, deliveries);
    projected_method(out, bridge);
    out.push_str("}\n");
}

/// The dispatch arm per accepted command.
fn run_method(out: &mut String, bridge: &Bridge<'_>) {
    let ir = bridge.ir;
    out.push_str(
        "    fn run(&mut self, command: &str, input: &json::Value) -> Result<String, BridgeError> \
         {\n        let mut out = String::new();\n        match command {\n",
    );
    for command in ir.commands.values() {
        let Some(component) = bridge.acceptors.get(&command.name) else {
            continue;
        };
        if !bridge
            .plan
            .is_generated(CapabilityKind::CommandContract, &command.name.to_string())
        {
            continue;
        }
        let field = name::value_ident(&component.to_string());
        let method = name::value_ident(&bridge.layout.rust().type_name(&command.name));
        let ident = ident(&command.name);
        let _ = write!(
            out,
            "            {:?} => {{\n                let input = \
             wire::decode_command_{ident}(input, \"input\")?;\n                let outcome = \
             self.{field}.{method}(input)?;\n                \
             wire::encode_outcome_{ident}(&outcome, &mut out);\n            }}\n",
            command.name.to_string()
        );
    }
    out.push_str(
        "            other => return Err(BridgeError::UnknownCommand(other.to_owned())),\n        \
         }\n        self.pump()?;\n        Ok(out)\n    }\n",
    );
}

/// Redelivery, by index into the log the page is already showing.
fn replay_method(out: &mut String, bridge: &Bridge<'_>) {
    let system = bridge.system();
    let _ = write!(
        out,
        "\n    fn replay(&mut self, occurrence: usize) -> Result<(), BridgeError> {{\n        let \
         Some(event) = {system}::System::published(self).get(occurrence).cloned() else \
         {{\n            return \
         Err(BridgeError::NoSuchOccurrence(occurrence));\n        }};\n        \
         self.redeliver(&event)?;\n        Ok(())\n    }}\n"
    );
}

/// Every event the system's log can carry, and the variant each arrives as.
fn logged<'a>(bridge: &'a Bridge<'a>) -> BTreeMap<&'a EventHandle, String> {
    let mut events: BTreeSet<&EventHandle> = bridge
        .ir
        .components
        .values()
        .flat_map(|component| component.publishes.iter())
        .collect();
    for delivery in deliveries(bridge) {
        events.insert(&delivery.binding.event);
        if let ResolvedFailure::Escalate { emits } = delivery.binding.on_failure() {
            events.insert(emits);
        }
    }
    event_variants(bridge.ir, bridge.layout.rust(), &events)
}

/// The log, with each occurrence's index — which is also the handle redelivery takes.
fn log_method(out: &mut String, bridge: &Bridge<'_>) {
    let system = bridge.system();
    let _ = write!(
        out,
        "\n    fn log(&self) -> String {{\n        let mut out = String::new();\n        \
         out.push('[');\n        for (occurrence, event) in \
         {system}::System::published(self).iter().enumerate() {{\n            if occurrence > 0 \
         {{\n                out.push(',');\n            }}\n            \
         out.push('{{');\n            json::member(&mut out, \"occurrence\");\n            \
         json::push_integer(&mut out, occurrence as i64);\n            json::member(&mut out, \
         \"event\");\n            match event {{\n"
    );
    for (event, variant) in logged(bridge) {
        let name = event.name().to_string();
        let _ = write!(
            out,
            "                {system}::SystemEvent::{variant}(payload) => {{\n                    \
             json::push_text(&mut out, {name:?});\n                    json::member(&mut out, \
             \"payload\");\n"
        );
        if bridge.presents_event(event.name()) {
            let _ = writeln!(
                out,
                "                    wire::encode_event_{}(payload, &mut out);",
                ident(event.name())
            );
        } else {
            // The plan does not mark this event generated, so no rendering of it exists to write.
            // Named on the log all the same: an occurrence the page silently dropped would be a
            // system that looks quieter than it is.
            out.push_str(
                "                    let _ = payload;\n                    \
                 out.push_str(\"null\");\n",
            );
        }
        out.push_str("                }\n");
    }
    out.push_str(
        "            }\n            out.push('}');\n        }\n        out.push(']');\n        \
         out\n    }\n",
    );
}

/// The transport's record of what each binding invoked.
fn invoked_method(out: &mut String, bridge: &Bridge<'_>, deliveries: &[Delivery<'_>]) {
    let system = bridge.system();
    if deliveries.is_empty() {
        out.push_str(
            "\n    fn invoked(&self) -> String {\n        // No binding of this specification has \
             a generated delivery, so nothing ever invokes a\n        // command on this system's \
             behalf and the record is empty by \
             construction.\n        \"[]\".to_owned()\n    }\n",
        );
        return;
    }
    let _ = write!(
        out,
        "\n    fn invoked(&self) -> String {{\n        let mut out = String::new();\n        \
         out.push('[');\n        for (position, invocation) in \
         {system}::System::invocations(self).iter().enumerate() {{\n            if position > 0 \
         {{\n                out.push(',');\n            }}\n            \
         out.push('{{');\n            match invocation {{\n"
    );
    for delivery in deliveries {
        let variant = name::pascal(&delivery.binding.name.to_string());
        let command = delivery.binding.command.name().to_string();
        let _ = write!(
            out,
            "                {system}::BindingInvocation::{variant}(input) => \
             {{\n                    json::member(&mut out, \"binding\");\n                    \
             json::push_text(&mut out, {:?});\n                    json::member(&mut out, \
             \"event\");\n                    json::push_text(&mut out, {:?});\n                    \
             json::member(&mut out, \"command\");\n                    json::push_text(&mut \
             out, {command:?});\n                    json::member(&mut out, \
             \"input\");\n                    wire::encode_command_{}(input, &mut \
             out);\n                }}\n",
            delivery.binding.name.to_string(),
            delivery.binding.event.name().to_string(),
            ident(delivery.binding.command.name()),
        );
        let _ = delivery.acceptor;
    }
    out.push_str(
        "            }\n            out.push('}');\n        }\n        out.push(']');\n        \
         out\n    }\n",
    );
}

/// Every declared view, served through the port of the component that owns it.
fn projected_method(out: &mut String, bridge: &Bridge<'_>) {
    out.push_str(
        "\n    fn projected(&self) -> String {\n        let mut out = String::new();\n        \
         out.push('{');\n",
    );
    for view in bridge.ir.views.values() {
        let Some(component) = bridge.view_components.get(&view.name) else {
            continue;
        };
        if !bridge.presents_view(&view.name) {
            continue;
        }
        let field = name::value_ident(&component.to_string());
        let method = name::value_ident(&bridge.layout.rust().type_name(&view.name));
        let _ = write!(
            out,
            "        json::member(&mut out, {:?});\n        match self.{field}.{method}() \
             {{\n            Ok(rows) => {{\n                out.push('{{');\n                \
             json::member(&mut out, \"rows\");\n                out.push('[');\n                \
             for (position, row) in rows.iter().enumerate() {{\n                    if position > \
             0 {{\n                        out.push(',');\n                    }}\n                    \
             wire::encode_view_{}(row, &mut out);\n                }}\n                \
             out.push(']');\n                out.push('}}');\n            }}\n            \
             Err(unmet) => {{\n                out.push('{{');\n                json::member(&mut \
             out, \"unmet\");\n                out.push('{{');\n                json::member(&mut \
             out, \"capability\");\n                json::push_text(&mut out, \
             unmet.capability);\n                json::member(&mut out, \
             \"source\");\n                json::push_text(&mut out, \
             unmet.source);\n                out.push('}}');\n                \
             out.push('}}');\n            }}\n        }}\n",
            view.name.to_string(),
            ident(&view.name),
        );
    }
    out.push_str("        out.push('}');\n        out\n    }\n");
}

/// The type that satisfies every obligation by refusing, so the module runs before anyone links.
fn unrealized(out: &mut String, bridge: &Bridge<'_>) {
    let ir = bridge.ir;
    let types = &bridge.types;
    let layout = bridge.layout.rust();
    out.push_str(
        "\n/// Every obligation of this system, refused in the type system.\n///\n/// Not a second \
         copy of the generated stubs: each method below delegates to the one the\n/// Rust target \
         emitted, so the refusal a page shows is the plan entry that target names. It\n/// exists \
         because a component may accept commands from more than one bounded context, and\n/// no \
         single generated `Unimplemented` covers two.\npub struct Unrealized;\n",
    );

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for component in ir.components.values() {
        for accepted in &component.accepts {
            let declared = accepted.name();
            let module = layout.module(layout.owner(declared));
            let type_name = layout.type_name(declared);
            let method = name::value_ident(&type_name);
            let trait_path = format!("{types}::{module}::obligations::{type_name}Behavior");
            if !seen.insert(trait_path.clone()) {
                continue;
            }
            let input = port::types_path(layout, types, declared);
            let _ = write!(
                out,
                "\nimpl {trait_path} for Unrealized {{\n    fn {method}(&mut self, input: \
                 {input}) -> Result<{input}Outcome, {types}::obligation::UnmetObligation> \
                 {{\n        {types}::{module}::obligations::Unimplemented.{method}(input)\n    \
                 }}\n}}\n"
            );
        }
        for domain in &component.owns {
            for view in &ir.domain(domain).views {
                let declared = view.name();
                let module = layout.module(layout.owner(declared));
                let type_name = layout.type_name(declared);
                let method = name::value_ident(&type_name);
                let trait_path = format!("{types}::{module}::obligations::{type_name}Query");
                if !seen.insert(trait_path.clone()) {
                    continue;
                }
                let row = port::types_path(layout, types, declared);
                let _ = write!(
                    out,
                    "\nimpl {trait_path} for Unrealized {{\n    fn {method}(&self) -> \
                     Result<Vec<{row}>, {types}::obligation::UnmetObligation> {{\n        \
                     {types}::{module}::obligations::Unimplemented.{method}()\n    }}\n}}\n"
                );
            }
        }
    }
}

/// The installed system, the seam that replaces it, and the one nobody has realized.
fn installation(out: &mut String, bridge: &Bridge<'_>) {
    let ir = bridge.ir;
    let system = bridge.system();
    let layout = bridge.layout.rust();

    let mut arguments: Vec<String> = ir
        .components
        .keys()
        .map(|component| {
            let package = RustLayout::crate_ident(layout.component_package(component));
            let port = name::pascal(&component.to_string());
            format!("{package}::{port}::new(Unrealized)")
        })
        .collect();
    // The system's own obligations, where it has any: the generated stub covers all of them, so
    // there is nothing to compose here.
    if bridge.has_system_obligations() {
        arguments.push(format!("{system}::obligations::Unimplemented"));
    }

    let _ = write!(
        out,
        "\nthread_local! {{\n    /// The one system this module drives, or nothing until \
         something installs one.\n    static SYSTEM: RefCell<Option<Box<dyn Bound>>> = const {{ \
         RefCell::new(None) }};\n    /// The buffer the page writes a request \
         into.\n    static INPUT: RefCell<Vec<u8>> = const {{ RefCell::new(Vec::new()) }};\n    \
         /// The response the last dispatch produced, held so its address stays \
         valid.\n    static OUTPUT: RefCell<String> = const {{ RefCell::new(String::new()) \
         }};\n}}\n\n/// Installs the assembled system this module drives.\n///\n/// Called by a \
         host that has linked one implementation per obligation — never by this crate,\n/// which \
         has none to offer. Installing twice replaces: the last system installed is the \
         one\n/// serving, and there is no merge, because merging two realizations would be \
         choosing between\n/// them.\npub fn install(system: Box<dyn Bound>) {{\n    \
         SYSTEM.with(|held| {{\n        *held.borrow_mut() = Some(system);\n    \
         }});\n}}\n\n/// The system nobody has realized: generated ports over stubs that \
         refuse.\n///\n/// The honest empty state. Every command answers with the typed refusal \
         naming what is owed,\n/// which is exactly what `PLAN.md` says is \
         owed.\nfn unrealized() -> Box<dyn Bound> {{\n    \
         Box::new({system}::System::new({}))\n}}\n\n/// Runs one action against the installed \
         system, installing the unrealized one if none is.\nfn with_system<T>(action: impl \
         FnOnce(&mut dyn Bound) -> T) -> T {{\n    SYSTEM.with(|held| {{\n        let mut held = \
         held.borrow_mut();\n        let system = held.get_or_insert_with(unrealized);\n        \
         action(system.as_mut())\n    }})\n}}\n",
        arguments.join(", ")
    );
}

/// The request loop: one function, every refusal a value.
fn serving(out: &mut String) {
    out.push_str(SERVE);
}

/// The fixed body of the request loop.
///
/// Fixed because the *protocol* is fixed: four requests, whatever the specification. What varies
/// with the model is everything they reach — the commands, the log, the views — and that is
/// generated above.
const SERVE: &str = r#"
/// Serves one request, answering JSON whatever happens.
///
/// Four requests, and the protocol is the same for every specification:
///
/// | request | what comes back |
/// | --- | --- |
/// | `{"request":"catalog"}` | the model this page renders itself from |
/// | `{"request":"observe"}` | the log, the binding invocations, and every view's rows |
/// | `{"request":"command","command":…,"input":{…}}` | the outcome, then the same observation |
/// | `{"request":"redeliver","occurrence":n}` | the occurrence delivered again, then the observation |
///
/// A refusal comes back as `{"ok":false,"error":{…}}` with a `kind` a page can react to, never as
/// a trap.
pub fn serve(request: &str) -> String {
    let mut out = String::new();
    match answer(request, &mut out) {
        Ok(()) => out,
        Err(error) => {
            let mut refusal = String::new();
            refusal.push('{');
            json::member(&mut refusal, "ok");
            json::push_bool(&mut refusal, false);
            json::member(&mut refusal, "error");
            error.encode(&mut refusal);
            refusal.push('}');
            refusal
        }
    }
}

/// One request, served into `out`.
fn answer(request: &str, out: &mut String) -> Result<(), BridgeError> {
    let request = json::parse(request).map_err(BridgeError::Malformed)?;
    let kind = json::text_at(
        json::member_at(&request, "", "request")?,
        "request",
        "a request kind",
    )?
    .to_owned();
    out.push('{');
    json::member(out, "ok");
    json::push_bool(out, true);
    match kind.as_str() {
        "catalog" => {
            json::member(out, "catalog");
            out.push_str(catalog::CATALOG);
        }
        "observe" => observe(out),
        "command" => {
            let command = json::text_at(
                json::member_at(&request, "", "command")?,
                "command",
                "a command name",
            )?
            .to_owned();
            let input = request
                .member("input")
                .cloned()
                .unwrap_or(json::Value::Object(Vec::new()));
            let outcome = with_system(|system| system.run(&command, &input))?;
            json::member(out, "command");
            json::push_text(out, &command);
            json::member(out, "outcome");
            out.push_str(&outcome);
            observe(out);
        }
        "redeliver" => {
            let occurrence = json::integer_at(
                json::member_at(&request, "", "occurrence")?,
                "occurrence",
                "an occurrence index",
            )?;
            let occurrence = usize::try_from(occurrence)
                .map_err(|_| BridgeError::NoSuchOccurrence(usize::MAX))?;
            with_system(|system| system.replay(occurrence))?;
            observe(out);
        }
        other => return Err(BridgeError::UnknownRequest(other.to_owned())),
    }
    out.push('}');
    Ok(())
}

/// The whole observable surface, written into an answer already in progress.
fn observe(out: &mut String) {
    let (log, invoked, projected) =
        with_system(|system| (system.log(), system.invoked(), system.projected()));
    json::member(out, "log");
    out.push_str(&log);
    json::member(out, "invocations");
    out.push_str(&invoked);
    json::member(out, "views");
    out.push_str(&projected);
}
"#;

/// The three exports, and the module documentation that says what a caller does with them.
fn exports(out: &mut String) {
    out.push_str(EXPORTS_SOURCE);
    debug_assert_eq!(
        EXPORTS,
        ["ess_input_reserve", "ess_dispatch", "ess_output_len"],
        "the exported symbols and the list `cargo xtask synth` checks the module against have \
         drifted apart"
    );
    let _ = REALIZE;
}

/// The fixed body of the boundary: three exports, no glue tool, no dependency.
const EXPORTS_SOURCE: &str = r#"
// ---- the boundary --------------------------------------------------------------------------------
//
// Three exports and no code generation on either side. A caller reserves a buffer of the request's
// byte length, writes UTF-8 into the module's memory at the address it gets back, calls
// `ess_dispatch`, and reads `ess_output_len` bytes from the address that returns. The buffers are
// ordinary `Vec<u8>` and `String` this module owns; nothing here dereferences a raw pointer.

/// Reserves a buffer of `length` bytes for the next request and answers its address.
///
/// The buffer is zeroed and owned by this module until the next reservation, so a caller may write
/// into it and then call [`ess_dispatch`]. Reserving again discards whatever was there.
#[no_mangle]
pub extern "C" fn ess_input_reserve(length: u32) -> u32 {
    INPUT.with(|held| {
        let mut held = held.borrow_mut();
        *held = vec![0; length as usize];
        held.as_ptr() as usize as u32
    })
}

/// Serves the request in the reserved buffer and answers the address of the JSON response.
///
/// Its length is [`ess_output_len`]. The response stays valid until the next dispatch.
#[no_mangle]
pub extern "C" fn ess_dispatch() -> u32 {
    let request = INPUT.with(|held| String::from_utf8_lossy(&held.borrow()).into_owned());
    let response = serve(&request);
    OUTPUT.with(|held| {
        let mut held = held.borrow_mut();
        *held = response;
        held.as_ptr() as usize as u32
    })
}

/// The length in bytes of the response [`ess_dispatch`] last produced.
#[no_mangle]
pub extern "C" fn ess_output_len() -> u32 {
    OUTPUT.with(|held| held.borrow().len() as u32)
}
"#;
