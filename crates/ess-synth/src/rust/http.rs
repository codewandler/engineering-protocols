//! The server-crate emitter: the second transport this scope holds, and the one a caller reaches.
//!
//! # The transport is derived, not chosen
//!
//! A binding's `delivery: at_least_once` determines an in-process log — that is `system.rs`. A
//! component's `reached_by: network` determines something else: that the surface exists on a wire,
//! because the callers are not deployed with it. *Which* wire is not a preference either. This
//! repository projects exactly one contract for a component's command surface, the `OpenAPI`
//! document under `generated/openapi/`, and an `OpenAPI` document is an HTTP contract — so a server
//! that spoke anything else would contradict the document committed beside it. No framework, no
//! runtime, no second protocol, and no abstraction over transports that do not exist.
//!
//! # The routes are not this file's to invent
//!
//! Every path here comes from [`ess_gen::http::routes`], which is the same function the `OpenAPI`
//! projection builds its `paths` from, and every status from [`ess_gen::http::status`], which is
//! the same function that projection builds its responses from. A server and a contract that
//! computed these separately would agree on the day they were written and drift the first time a
//! wire name moved — invisibly, because a server answering a path no document declares looks
//! exactly like a server that works.
//!
//! # What it does not decide
//!
//! It chooses no realization: the emitted `serve_*` function takes the assembled system and hands
//! every command to the port, so a build with nothing implemented answers `501` naming the
//! obligation the plan owes (gap register D-2). It is not a deployment either — one connection at a
//! time, in accept order, `Connection: close` — and every concurrency decision it did not make is
//! one a deployment gets to make itself.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{EssIr, ResolvedComponent, ResolvedView};
use ess_domain::component::Reach;
use ess_domain::name::QualifiedName;
use ess_gen::http::{self, Method, Served};
use ess_gen::{Artifact, Provenance};

use crate::plan::{Capability, CapabilityKind, SynthesisPlan, REGENERATE};

use super::layout::Layout;
use super::wire::{self, Surface};
use super::{name, system, EDITION};

/// The label every startup line carries, so a reader can tell this record from any other JSON a
/// process writes.
const LOG_FORMAT: &str = "ess/1";

/// The transport the emitted server speaks, as the startup record names it.
const TRANSPORT: &str = "http/1.1";

/// Everything the server crate's renderers agree on, carried once.
pub(super) struct Server<'a> {
    /// The resolved model.
    ir: &'a EssIr,
    /// The plan, which is the gate on every codec below.
    plan: &'a SynthesisPlan,
    /// Where the Rust target put everything.
    layout: &'a Layout,
    /// The types crate, as a path spells it from inside this crate.
    types: String,
}

/// The wire emitter's seam. The server carries the whole generated wire rather than the slice one
/// route reaches, because every function it emits is `pub`: a codec nothing routes to is a codec a
/// caller of this crate can still use, and computing the reachable set would be a second answer to
/// "what crosses this boundary" that only this target would hold.
impl Surface for Server<'_> {
    fn ir(&self) -> &EssIr {
        self.ir
    }

    fn layout(&self) -> &Layout {
        self.layout
    }

    fn types(&self) -> &str {
        &self.types
    }

    fn presents_type(&self, declared: &QualifiedName) -> bool {
        self.plan
            .is_generated(CapabilityKind::DomainType, &declared.to_string())
    }

    fn presents_event(&self, declared: &QualifiedName) -> bool {
        self.plan
            .is_generated(CapabilityKind::EventType, &declared.to_string())
    }

    fn presents_error(&self, declared: &QualifiedName) -> bool {
        self.plan
            .is_generated(CapabilityKind::ErrorType, &declared.to_string())
    }

    fn presents_view(&self, declared: &QualifiedName) -> bool {
        self.plan
            .is_generated(CapabilityKind::ViewType, &declared.to_string())
    }

    fn presents_command(&self, declared: &QualifiedName) -> bool {
        self.plan
            .is_generated(CapabilityKind::CommandContract, &declared.to_string())
    }
}

/// Every component whose surface the specification says is reached from outside, in name order.
pub(crate) fn served(ir: &EssIr) -> Vec<&ResolvedComponent> {
    ir.components
        .values()
        .filter(|component| component.reached_by == Reach::Network)
        .collect()
}

/// The server crate, when any component's surface is served — and nothing at all when none is.
///
/// A specification that says nothing about reach gets no crate, no manifest member and no route
/// table, which is what keeps the normative example's tree the tree it was before this word
/// existed.
pub(super) fn server_crate(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    covered: &mut BTreeSet<Capability>,
) -> Vec<Artifact> {
    let components = served(ir);
    if components.is_empty() {
        return Vec::new();
    }
    let server = Server {
        ir,
        plan,
        layout,
        types: Layout::crate_ident(layout.package()),
    };
    let package = layout.server_package();
    let provenance = &plan.provenance;

    let mut artifacts = vec![
        manifest(ir, layout, provenance),
        lib_module(ir, layout, &components, provenance),
        Artifact::new(
            format!("crates/{package}/src/json.rs"),
            format!(
                "{}{}",
                provenance.commented_for("//", REGENERATE),
                super::json::JSON
            ),
        ),
        Artifact::new(
            format!("crates/{package}/src/wire.rs"),
            format!(
                "{}{}",
                provenance.commented_for("//", REGENERATE),
                wire::module(&server)
            ),
        ),
        Artifact::new(
            format!("crates/{package}/src/http.rs"),
            format!("{}{}", provenance.commented_for("//", REGENERATE), HTTP),
        ),
    ];

    for component in &components {
        covered.insert(Capability {
            kind: CapabilityKind::ComponentTransport,
            source: component.name.to_string(),
        });
        artifacts.push(surface_module(&server, component, &components));
        artifacts.push(Artifact::new(
            format!("crates/{package}/src/{}.openapi.json", component.name),
            ess_gen::openapi::json(ir, component),
        ));
        artifacts.push(Artifact::new(
            format!("crates/{package}/src/{}.docs.md", component.name),
            ess_gen::docs::served(ir, component),
        ));
    }
    artifacts
}

/// The server crate's manifest: the types crate, every component crate and the system crate, by
/// path — the workspace stays self-contained and zero third-party dependencies.
fn manifest(ir: &EssIr, layout: &Layout, provenance: &Provenance) -> Artifact {
    let package = layout.server_package();
    let mut out = provenance.commented_for("#", REGENERATE);
    let _ = write!(
        out,
        "\n[package]\nname = \"{package}\"\ndescription = \"The HTTP surface of the `{}` \
         specification, {}: the routes its components' declarations determine, generated.\"\n\
         version = \"{}.0.0\"\nedition = \"{EDITION}\"\n\n[dependencies]\n",
        ir.system,
        ir.version,
        ir.version.get(),
    );
    let mut dependencies = vec![layout.package().to_owned()];
    for component in ir.components.keys() {
        dependencies.push(layout.component_package(component).to_owned());
    }
    dependencies.push(layout.system_package().to_owned());
    dependencies.sort();
    for dependency in dependencies {
        let _ = writeln!(out, "{dependency} = {{ path = \"../{dependency}\" }}");
    }
    Artifact::new(format!("crates/{package}/Cargo.toml"), out)
}

/// The crate root: what it is, what it refuses to be, and its module list.
fn lib_module(
    ir: &EssIr,
    layout: &Layout,
    components: &[&ResolvedComponent],
    provenance: &Provenance,
) -> Artifact {
    let package = layout.server_package();
    let mut out = provenance.commented_for("//", REGENERATE);
    out.push('\n');
    let _ = writeln!(
        out,
        "//! The HTTP surface of `{}` {}, synthesised.",
        ir.system, ir.version
    );
    out.push_str(
        "//!\n//! One module per component the specification declares is reached over a network, \
         each holding\n//! that component's route table, its listener and the two documents it \
         publishes about itself.\n//! The routes are the ones the committed `OpenAPI` document \
         declares, from the same mapping, so a\n//! path served here and a path published there \
         cannot be two different \
         answers.\n//!\n//! Generated, not written: the specification is the source of truth, and \
         the door to changing\n//! anything here is `",
    );
    out.push_str(REGENERATE);
    out.push_str(
        "`. What is deliberately absent is absent by\n//! decision — no framework, no runtime, no \
         second protocol, no concurrency, no authentication —\n//! and each absence is argued in \
         the `TARGET.md` beside this \
         workspace.\n\n#![forbid(unsafe_code)]\n#![deny(missing_docs)]\n\npub mod http;\npub mod \
         json;\npub mod wire;\n",
    );
    for component in components {
        let _ = writeln!(out, "pub mod {};", module_ident(component));
    }
    Artifact::new(format!("crates/{package}/src/lib.rs"), out)
}

/// The module identifier one component's surface lands in.
fn module_ident(component: &ResolvedComponent) -> String {
    name::value_ident(&component.name.to_string())
}

/// One served component: its route table, its handlers, its startup record and its listener.
fn surface_module(
    server: &Server<'_>,
    component: &ResolvedComponent,
    siblings: &[&ResolvedComponent],
) -> Artifact {
    let ir = server.ir;
    let layout = server.layout;
    let package = layout.server_package();
    let routes = http::routes(ir, component);

    let mut out = server.plan.provenance.commented_for("//", REGENERATE);
    out.push('\n');
    let _ = writeln!(
        out,
        "//! The `{}` component of `{}` {}, on the wire.",
        component.name, ir.system, ir.version
    );
    out.push_str(
        "//!\n//! The specification says this component's callers are not deployed with it, so its \
         surface\n//! exists on a wire. Which wire is derived rather than chosen: the one contract \
         this model\n//! projects for a command surface is the `OpenAPI` document, and an \
         `OpenAPI` document is an\n//! HTTP contract. The document is beside this file, served \
         verbatim at `/openapi.json`.\n\nuse crate::{http, json, wire};\n",
    );

    documents(&mut out, component);
    route_table(&mut out, &routes, ir);
    startup(&mut out, server, component, &routes, siblings);
    serve_function(&mut out, server, component);
    dispatch(&mut out, server, component, &routes);
    handlers(&mut out, server, component, &routes);

    Artifact::new(
        format!("crates/{package}/src/{}.rs", module_ident(component)),
        out,
    )
}

/// The two documents this surface publishes about itself, embedded from the files beside it.
fn documents(out: &mut String, component: &ResolvedComponent) {
    let _ = write!(
        out,
        "\n/// The contract this surface answers, byte for byte as `generated/` commits it.\n///\n\
         /// Embedded rather than rebuilt at run time: a server that regenerated its own contract \
         could\n/// publish one the repository never reviewed.\npub const OPENAPI: &str = \
         include_str!(\"{0}.openapi.json\");\n\n/// The prose the same model produced, byte for \
         byte as the documentation projection wrote it.\npub const DOCS: &str = \
         include_str!(\"{0}.docs.md\");\n",
        component.name
    );
}

/// Every route, as a table the log line and the reader both read.
fn route_table(out: &mut String, routes: &[http::Route<'_>], ir: &EssIr) {
    out.push_str(
        "\n/// Every route this surface answers, in path order.\n///\n/// The same set the \
         `OpenAPI` document declares, plus the two documents about the surface\n/// itself, which \
         no specification construct names and nothing can therefore derive. A path\n/// absent \
         from this table is answered with `404`, including one the document declares and \
         this\n/// table forgot — which is the failure a table computed twice would \
         hide.\npub const ROUTES: &[(&str, &str)] = &[\n",
    );
    for (method, path, _, _) in table(routes, ir) {
        let _ = writeln!(out, "    ({method:?}, {path:?}),");
    }
    out.push_str("];\n");
}

/// The whole surface as rows of `(method, path, what it serves, the construct's name)`.
///
/// The two documents are rows too, so `ROUTES`, the startup record and the dispatcher are one list
/// in three renderings rather than three lists that have to be kept level.
fn table<'a>(
    routes: &'a [http::Route<'a>],
    ir: &'a EssIr,
) -> Vec<(&'static str, String, &'static str, String)> {
    let mut rows: Vec<(&'static str, String, &'static str, String)> = vec![
        (
            Method::Get.as_str(),
            http::DOCS.to_owned(),
            "documentation",
            "docs".to_owned(),
        ),
        (
            Method::Get.as_str(),
            http::OPENAPI.to_owned(),
            "contract",
            "openapi".to_owned(),
        ),
    ];
    for route in routes {
        rows.push(match route.serves {
            Served::Command(handle) => (
                route.method.as_str(),
                route.path.clone(),
                "command",
                ir.command(handle).name.to_string(),
            ),
            Served::View(handle) => (
                route.method.as_str(),
                route.path.clone(),
                "view",
                ir.view(handle).name.to_string(),
            ),
        });
    }
    rows.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(right.0)));
    rows
}

/// The three startup lines, everything about them that the specification determines.
///
/// Each constant is a JSON object **without its closing brace**, because the one member every line
/// still needs is the one the specification does not determine: `runtime`, which carries what is
/// true of this process rather than of this model. That split is the whole comparison — two
/// binaries synthesised from one specification must agree on every byte outside `runtime`, and a
/// field that moved out of `runtime` to make a comparison pass would be a field that stopped being
/// checked.
fn startup(
    out: &mut String,
    server: &Server<'_>,
    component: &ResolvedComponent,
    routes: &[http::Route<'_>],
    siblings: &[&ResolvedComponent],
) {
    let lines = startup_lines(server, component, routes, siblings);
    out.push_str(
        "\n/// What this process says about itself as it starts, before it answers anything.\n///\n\
         /// Three lines of JSON on standard output, in this order, every member of them derived \
         from the\n/// specification — except `runtime`, which is appended by the emitted code \
         below and holds what\n/// is true of *this process*: the language it was synthesised \
         into, and the address it bound.\n/// Everything outside `runtime` is the same in every \
         language this plan is emitted into, and\n/// `cargo xtask synth --check` starts both and \
         compares them.\npub const STARTUP: &[&str] = &[\n",
    );
    for line in &lines {
        let _ = writeln!(out, "    {line:?},");
    }
    out.push_str("];\n");
    out.push_str(
        "\n/// Writes the startup record, with this process's own facts closing each line.\nfn \
         announce(address: &std::net::SocketAddr) {\n    for facts in STARTUP {\n        let mut \
         line = String::from(*facts);\n        line.push_str(\",\\\"runtime\\\":{\\\"address\\\":\
         \");\n        json::push_text(&mut line, &address.to_string());\n        \
         line.push_str(\",\\\"language\\\":\\\"rust\\\",\\\"port\\\":\");\n        \
         json::push_integer(&mut line, i64::from(address.port()));\n        \
         line.push_str(\"}}\");\n        println!(\"{line}\");\n    }\n}\n",
    );
}

/// The three lines, as JSON text without the closing brace of each.
fn startup_lines(
    server: &Server<'_>,
    component: &ResolvedComponent,
    routes: &[http::Route<'_>],
    siblings: &[&ResolvedComponent],
) -> Vec<String> {
    startup_facts(
        server.ir,
        server.plan,
        component,
        &table(routes, server.ir),
        siblings.len(),
        LOG_FORMAT,
        TRANSPORT,
    )
}

/// The specification-derived half of the startup record, shared by every target that serves.
///
/// One function, because the whole point of the record is that two synthesised applications write
/// the same one: a second implementation of it in the second emitter would be a second answer to
/// "what does this system say about itself", and the comparison that is supposed to catch drift
/// would be comparing two copies of the same mistake. What each target appends is its `runtime`,
/// which is where a language belongs.
pub(crate) fn startup_facts(
    ir: &EssIr,
    plan: &SynthesisPlan,
    component: &ResolvedComponent,
    rows: &[(&'static str, String, &'static str, String)],
    surfaces: usize,
    log_format: &str,
    transport: &str,
) -> Vec<String> {
    let provenance = &plan.provenance;
    let counts = plan.counts();

    let mut starting = String::from("{");
    member(&mut starting, "log", log_format);
    member(&mut starting, "event", "system.starting");
    member(&mut starting, "system", &ir.system.to_string());
    member(&mut starting, "version", &ir.version.to_string());
    member(&mut starting, "model_digest", &provenance.source_digest);
    member(
        &mut starting,
        "contract_digest",
        &provenance.contract_digest,
    );
    let _ = write!(
        starting,
        ",\"components\":[{}]",
        ir.components
            .keys()
            .map(|name| text(&name.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    );
    let _ = write!(
        starting,
        ",\"capabilities\":{{\"generated\":{},\"obligations\":{},\"refused\":{}}}",
        counts.generated, counts.obligations, counts.refused
    );

    let mut serving = String::from("{");
    member(&mut serving, "log", log_format);
    member(&mut serving, "event", "surface.serving");
    member(&mut serving, "component", &component.name.to_string());
    member(&mut serving, "reached_by", component.reached_by.as_str());
    member(&mut serving, "transport", transport);
    let _ = write!(serving, ",\"routes\":{}", rows.len());
    serving.push_str(",\"paths\":[");
    for (position, (method, path, serves, named)) in rows.iter().enumerate() {
        if position > 0 {
            serving.push(',');
        }
        serving.push('{');
        member(&mut serving, "method", method);
        member(&mut serving, "path", path);
        member(&mut serving, "serves", serves);
        member(&mut serving, "name", named);
        serving.push('}');
    }
    serving.push(']');

    let mut ready = String::from("{");
    member(&mut ready, "log", log_format);
    member(&mut ready, "event", "system.ready");
    member(&mut ready, "system", &ir.system.to_string());
    let _ = write!(ready, ",\"surfaces\":{surfaces}");

    vec![starting, serving, ready]
}

/// One string member of an object being built, with the separator it needs.
fn member(out: &mut String, key: &str, value: &str) {
    if !out.ends_with('{') {
        out.push(',');
    }
    out.push_str(&text(key));
    out.push(':');
    out.push_str(&text(value));
}

/// One JSON string, escaped the way the emitted writer escapes one.
fn text(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|error| panic!("a string serialises: {error}"))
}

/// The listener: bind, announce, then answer one connection at a time until something breaks.
fn serve_function(out: &mut String, server: &Server<'_>, component: &ResolvedComponent) {
    let ir = server.ir;
    let layout = server.layout;
    let system_crate = Layout::crate_ident(layout.system_package());
    let generics = system::components_generics(ir);
    let angled = generic_list(server);
    let bounds = super::port::bound_list(
        ir,
        layout,
        component,
        &Layout::crate_ident(layout.package()),
    );

    let _ = write!(
        out,
        "\n/// Serves `{}` at `address`, and does not return while it can answer.\n///\n\
         /// `address` may name port `0`, which binds an ephemeral port; the startup record says \
         which one\n/// was taken, because a caller that cannot learn the port cannot make a \
         request.\n///\n/// It chooses no realization. Every command reaches the port, and a port \
         over unimplemented\n/// obligations answers the typed refusal this surface reports as \
         `501` — the honest empty\n/// state rather than a server that pretends.\n///\n\
         /// # Errors\n///\n/// Anything the listener refuses: the address is taken, the port is \
         privileged, the socket\n/// died.\npub fn serve{angled}(system: &mut \
         {system_crate}::System{angled}, address: &str) -> std::io::Result<()>\n",
        component.name
    );
    if !generics.is_empty() {
        out.push_str("where\n");
        for generic in &generics {
            if bounds.is_empty() {
                let _ = writeln!(out, "    {generic}: Sized,");
            } else {
                let _ = writeln!(out, "    {generic}: {},", bounds.join(" + "));
            }
        }
    }
    out.push_str(SERVE_BODY);
}

/// The listener's body, which no specification changes.
const SERVE_BODY: &str = r"{
    let listener = std::net::TcpListener::bind(address)?;
    announce(&listener.local_addr()?);
    for connection in listener.incoming() {
        let mut reader = std::io::BufReader::new(connection?);
        let answer = match http::read(&mut reader) {
            Ok(request) => dispatch(system, &request),
            Err(refusal) => refusal,
        };
        let mut stream = reader.into_inner();
        http::write(&mut stream, &answer)?;
    }
    Ok(())
}
";

/// The generic parameter list `System` carries, as this crate has to spell it.
fn generic_list(server: &Server<'_>) -> String {
    let mut generics = system::components_generics(server.ir);
    if system::has_obligations(server.ir, server.plan) {
        generics.push("Obligations".to_owned());
    }
    if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    }
}

/// The route match: one arm per path, and one arm for everything else.
fn dispatch(
    out: &mut String,
    server: &Server<'_>,
    component: &ResolvedComponent,
    routes: &[http::Route<'_>],
) {
    let ir = server.ir;
    let system_crate = Layout::crate_ident(server.layout.system_package());
    let angled = generic_list(server);
    let bounds = super::port::bound_list(
        ir,
        server.layout,
        component,
        &Layout::crate_ident(server.layout.package()),
    );
    let generics = system::components_generics(ir);

    let _ = write!(
        out,
        "\n/// Answers one request.\n///\n/// A path this table does not hold is a `404` naming \
         where the whole table is published; a\n/// path it holds under a different method is a \
         `405` naming the one it answers. Neither is a\n/// status the contract declares, and \
         neither should be: both are facts about a transport rather\n/// than about any \
         command.\nfn dispatch{angled}(system: &mut {system_crate}::System{angled}, request: \
         &http::Request) -> http::Response\n"
    );
    if !generics.is_empty() {
        out.push_str("where\n");
        for generic in &generics {
            if bounds.is_empty() {
                let _ = writeln!(out, "    {generic}: Sized,");
            } else {
                let _ = writeln!(out, "    {generic}: {},", bounds.join(" + "));
            }
        }
    }
    out.push_str("{\n    match request.path.as_str() {\n");
    for (method, path, _, _) in table(routes, ir) {
        let _ = writeln!(
            out,
            "        {path:?} => {{\n            if request.method != {method:?} {{\n                return http::method_not_allowed({method:?});\n            }}"
        );
        match path.as_str() {
            path if path == http::OPENAPI => out
                .push_str("            http::Response::new(200, http::JSON, OPENAPI)\n        }\n"),
            path if path == http::DOCS => out.push_str(
                "            http::Response::new(200, http::MARKDOWN, DOCS)\n        }\n",
            ),
            _ => {
                let route = routes
                    .iter()
                    .find(|route| route.path == path)
                    .expect("every non-document row of the table is a route");
                let call = match route.serves {
                    Served::Command(handle) => format!(
                        "            {}(system, &request.body)\n        }}\n",
                        handler_ident(&ir.command(handle).name)
                    ),
                    Served::View(handle) => format!(
                        "            {}(system)\n        }}\n",
                        handler_ident(&ir.view(handle).name)
                    ),
                };
                out.push_str(&call);
            }
        }
    }
    out.push_str(
        "        other => http::Response::refusal(\n            404,\n            \
         &format!(\"`{other}` is not a path this surface declares; `GET /openapi.json` publishes \
         every one that is\"),\n        ),\n    }\n}\n",
    );
}

/// The handler function name for one construct: its whole qualified name, snake-cased.
fn handler_ident(declared: &QualifiedName) -> String {
    format!("serve_{}", wire::ident(declared))
}

/// One handler per route: decode, call the port, render what the contract declares.
fn handlers(
    out: &mut String,
    server: &Server<'_>,
    component: &ResolvedComponent,
    routes: &[http::Route<'_>],
) {
    let ir = server.ir;
    for route in routes {
        match route.serves {
            Served::Command(handle) => command_handler(out, server, component, ir.command(handle)),
            Served::View(handle) => view_handler(out, server, component, ir.view(handle)),
        }
    }
}

/// One accepted command: body in, declared outcome out, at the status the contract publishes.
fn command_handler(
    out: &mut String,
    server: &Server<'_>,
    component: &ResolvedComponent,
    command: &ess_compiler::ir::ResolvedCommand,
) {
    let ir = server.ir;
    let layout = server.layout;
    let system_crate = Layout::crate_ident(layout.system_package());
    let angled = generic_list(server);
    let generics = system::components_generics(ir);
    let bounds = super::port::bound_list(
        ir,
        layout,
        component,
        &Layout::crate_ident(layout.package()),
    );
    let ident = wire::ident(&command.name);
    let field = name::value_ident(&component.name.to_string());
    let method = name::value_ident(&layout.type_name(&command.name));
    let outcome_type = format!(
        "{}Outcome",
        wire::path(
            layout,
            &Layout::crate_ident(layout.package()),
            &command.name
        )
    );

    let _ = write!(
        out,
        "\n/// `POST` `{}`: reads the declared input, runs the port, answers the declared \
         outcome.\nfn serve_{ident}{angled}(system: &mut {system_crate}::System{angled}, body: \
         &[u8]) -> http::Response\n",
        command.name
    );
    if !generics.is_empty() {
        out.push_str("where\n");
        for generic in &generics {
            if bounds.is_empty() {
                let _ = writeln!(out, "    {generic}: Sized,");
            } else {
                let _ = writeln!(out, "    {generic}: {},", bounds.join(" + "));
            }
        }
    }
    out.push_str(READ_BODY);
    let _ = write!(
        out,
        "    let input = match wire::decode_command_{ident}(&value, \"body\") {{
        Ok(input) => input,
        Err(error) => {{
            // `400` and not `422`: this is a body the schema decides, which is the difference
            // between fixing a value and fixing a serialiser.
            return http::Response::refusal(400, &format!(\"{{error}}\"));
        }}
    }};
    match system.{field}.{method}(input) {{
        Ok(outcome) => answer_{ident}(&outcome),
        Err(unmet) => http::Response::refusal(501, &format!(\"{{unmet}}\")),
    }}
}}
"
    );

    // The outcome renderer, whose statuses and body shape are the contract's own.
    let _ = write!(
        out,
        "\n/// One declared outcome of `{}`, as the contract publishes it: the branch that was \
         taken,\n/// the declared error where there is one, and that error's own \
         payload.\nfn answer_{ident}(outcome: &{outcome_type}) -> http::Response {{\n    let mut \
         body = String::from(\"{{\");\n    let status = match outcome {{\n",
        command.name
    );
    for outcome in &command.outcomes {
        let variant = name::pascal(outcome.name.as_str());
        let pattern = match &outcome.error {
            Some(_) => format!("{outcome_type}::{variant} {{ error, .. }}"),
            None if carries(server, outcome) => format!("{outcome_type}::{variant} {{ .. }}"),
            None => format!("{outcome_type}::{variant}"),
        };
        let _ = writeln!(out, "        {pattern} => {{");
        let _ = writeln!(
            out,
            "            json::member(&mut body, \"outcome\");\n            \
             json::push_text(&mut body, {:?});",
            outcome.name.as_str()
        );
        if let Some(handle) = &outcome.error {
            let declared = ir.error(handle);
            let _ = writeln!(
                out,
                "            json::member(&mut body, \"error\");\n            \
                 json::push_text(&mut body, {:?});",
                declared.name.to_string()
            );
            if !declared.fields.is_empty() {
                let _ = writeln!(
                    out,
                    "            json::member(&mut body, \"payload\");\n            \
                     wire::encode_error_{}(error, &mut body);",
                    wire::ident(&declared.name)
                );
            }
        }
        let _ = writeln!(out, "            {}\n        }}", http::status(outcome));
    }
    out.push_str(
        "    };\n    body.push('}');\n    http::Response::new(status, http::JSON, body)\n}\n",
    );
}

/// `true` when an outcome's variant carries anything at all, so its pattern needs `{ .. }`.
fn carries(server: &Server<'_>, outcome: &ess_compiler::ir::ResolvedOutcome) -> bool {
    let _ = server;
    !outcome.emits.is_empty()
}

/// One declared view: the rows the projection holds, under the key the contract declares.
fn view_handler(
    out: &mut String,
    server: &Server<'_>,
    component: &ResolvedComponent,
    view: &ResolvedView,
) {
    let ir = server.ir;
    let layout = server.layout;
    let system_crate = Layout::crate_ident(layout.system_package());
    let angled = generic_list(server);
    let generics = system::components_generics(ir);
    let bounds = super::port::bound_list(
        ir,
        layout,
        component,
        &Layout::crate_ident(layout.package()),
    );
    let ident = wire::ident(&view.name);
    let field = name::value_ident(&component.name.to_string());
    let method = name::value_ident(&layout.type_name(&view.name));

    let _ = write!(
        out,
        "\n/// `GET` `{}` at `{}` consistency: every row the owed projection holds.\nfn \
         serve_{ident}{angled}(system: &{system_crate}::System{angled}) -> http::Response\n",
        view.name,
        view.consistency.as_str()
    );
    if !generics.is_empty() {
        out.push_str("where\n");
        for generic in &generics {
            if bounds.is_empty() {
                let _ = writeln!(out, "    {generic}: Sized,");
            } else {
                let _ = writeln!(out, "    {generic}: {},", bounds.join(" + "));
            }
        }
    }
    let _ = write!(
        out,
        "{{
    match system.{field}.{method}() {{
        Ok(rows) => {{
            let mut body = String::from(\"{{\");
            json::member(&mut body, \"rows\");
            body.push('[');
            for (position, row) in rows.iter().enumerate() {{
                if position > 0 {{
                    body.push(',');
                }}
                wire::encode_view_{ident}(row, &mut body);
            }}
            body.push(']');
            body.push('}}');
            http::Response::new(200, http::JSON, body)
        }}
        Err(unmet) => http::Response::refusal(501, &format!(\"{{unmet}}\")),
    }}
}}
"
    );
}

/// A command handler's opening: the body as text, then as a JSON value, or the refusal that says
/// why it is neither. The same lines whatever the command, so they are written once.
const READ_BODY: &str = r#"{
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not UTF-8: {error}"));
        }
    };
    let value = match json::parse(text) {
        Ok(value) => value,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not JSON: {error}"));
        }
    };
"#;

/// The body of the emitted `http` module: HTTP/1.1, as much of it as this surface needs.
const HTTP: &str = r#"
//! HTTP/1.1, as much of it as a synthesised surface needs and no more.
//!
//! Not a framework and not a deployment. One connection at a time, in accept order: read the
//! request line, read the headers, read exactly `Content-Length` bytes, answer, close. There is no
//! keep-alive, no pipelining, no compression, no TLS and no thread pool, and every one of those is
//! a decision a deployment gets to make rather than one a generator makes for it. What this file
//! *does* guarantee is the part the specification determines: the status codes and the bodies.
//!
//! Written here rather than taken from a crate for the reason the JSON reader beside it is: the
//! emitted tree builds with zero third-party crates, inside a gate that reaches no network.

use std::io::{BufRead, Read, Write};

/// The largest body this surface reads, in bytes.
///
/// A caller can claim any length, and a server that allocated whatever it was told to is a server
/// anyone can stop by saying a large number. A megabyte is far past any command input this model
/// can describe.
pub const MAX_BODY: usize = 1_048_576;

/// The media type every answer derived from the model carries.
pub const JSON: &str = "application/json";

/// The media type the prose answer carries.
///
/// The bytes served are the committed Markdown, unrendered: rendering it to HTML here would be a
/// second rendering of the documentation, and the two would differ the first time either moved.
pub const MARKDOWN: &str = "text/markdown; charset=utf-8";

/// One request, as much of it as this surface reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The method, verbatim.
    pub method: String,
    /// The target, with any query string removed.
    ///
    /// The model declares no parameter, so a query string names nothing on this surface. It is
    /// dropped rather than refused, because a caller that appends one has not made a different
    /// request.
    pub path: String,
    /// The body: exactly the `Content-Length` bytes the caller announced.
    pub body: Vec<u8>,
}

/// One answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The media type of the body.
    pub content_type: &'static str,
    /// The body.
    pub body: String,
}

impl Response {
    /// An answer carrying a body.
    pub fn new(status: u16, content_type: &'static str, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type,
            body: body.into(),
        }
    }

    /// A refusal this surface makes rather than the specification.
    ///
    /// A malformed request, a path nothing declares, a method a path does not answer, an
    /// obligation nothing has satisfied. None of these is a declared outcome, and none is
    /// published in the contract, because each is a fact about a transport rather than about a
    /// command. The body is JSON with one member: a caller that has just failed to satisfy a
    /// contract should not have to parse a second format to read why.
    pub fn refusal(status: u16, detail: &str) -> Self {
        let mut body = String::from("{");
        crate::json::member(&mut body, "refused");
        crate::json::push_text(&mut body, detail);
        body.push('}');
        Self::new(status, JSON, body)
    }
}

/// The answer for a path this surface holds under a different method.
pub fn method_not_allowed(allowed: &str) -> Response {
    Response::refusal(
        405,
        &format!("this path answers `{allowed}`, and the contract declares no other method for it"),
    )
}

/// Reads one request, or the refusal that says why it could not be read.
///
/// # Errors
///
/// Never as an `Err` of the outer kind: everything that can go wrong with a request is an answer
/// the caller should receive, so the failure arm is the [`Response`] to send back.
pub fn read(reader: &mut std::io::BufReader<std::net::TcpStream>) -> Result<Request, Response> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => {
            return Err(Response::refusal(
                400,
                "the connection closed before a request line arrived",
            ))
        }
        Ok(_) => {}
        Err(error) => {
            return Err(Response::refusal(
                400,
                &format!("the request line could not be read: {error}"),
            ))
        }
    }
    let mut parts = line.trim_end().split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let version = parts.next().unwrap_or_default().to_owned();
    if method.is_empty() || target.is_empty() || !version.starts_with("HTTP/1.") {
        return Err(Response::refusal(
            400,
            "the request line is not `METHOD TARGET HTTP/1.1`",
        ));
    }
    let path = target
        .split('?')
        .next()
        .unwrap_or(target.as_str())
        .to_owned();

    let mut length = 0_usize;
    let mut chunked = false;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => {
                return Err(Response::refusal(
                    400,
                    "the connection closed inside the headers",
                ))
            }
            Ok(_) => {}
            Err(error) => {
                return Err(Response::refusal(
                    400,
                    &format!("a header could not be read: {error}"),
                ))
            }
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(Response::refusal(400, "a header line has no `:`"));
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "content-length" {
            match value.parse::<usize>() {
                Ok(parsed) => length = parsed,
                Err(_) => {
                    return Err(Response::refusal(
                        400,
                        "`Content-Length` is not a number of bytes",
                    ))
                }
            }
        } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
            chunked = true;
        }
    }
    if chunked {
        return Err(Response::refusal(
            411,
            "this surface reads a body announced by `Content-Length`; chunked transfer is not read",
        ));
    }
    if length > MAX_BODY {
        return Err(Response::refusal(
            413,
            &format!("the body is {length} bytes and this surface reads at most {MAX_BODY}"),
        ));
    }
    let mut body = vec![0_u8; length];
    if let Err(error) = reader.read_exact(&mut body) {
        return Err(Response::refusal(
            400,
            &format!("the body was shorter than `Content-Length` announced: {error}"),
        ));
    }
    Ok(Request { method, path, body })
}

/// Writes one answer, and lets the connection close behind it.
///
/// # Errors
///
/// Whatever the socket refuses.
pub fn write(stream: &mut std::net::TcpStream, answer: &Response) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        answer.status,
        reason(answer.status),
        answer.content_type,
        answer.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(answer.body.as_bytes())?;
    stream.flush()
}

/// The reason phrase for every status this surface can answer with.
///
/// Every one of them is either a status the contract declares for a branch, or one of the four this
/// surface answers about the request itself. A status not in this list is one nothing emits.
pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        _ => "Unknown",
    }
}
"#;
