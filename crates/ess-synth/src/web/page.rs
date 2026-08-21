//! The page, and the glue between it and the module's linear memory.
//!
//! # The page holds no model
//!
//! Every command, field, outcome, event, view and lifecycle it shows is built at load time from
//! the catalogue the module carries. Nothing about `billing` — or about any other system — is
//! typed into the HTML, which is what makes the page correct after a specification changes rather
//! than the one artifact nobody regenerated. What varies with the model here is the title, the
//! provenance stamp and the summary; everything else is the same bytes for every specification,
//! and that is the point.
//!
//! # The glue is a file, not a copy
//!
//! `bridge.js` is thirty lines and it is the only place the boundary protocol is written down.
//! The page imports it; so does the boundary smoke test in `cargo xtask synth`, which is what
//! makes that test a test of the page's own glue rather than of a second implementation that
//! happens to agree.

use std::fmt::Write as _;

use crate::plan::REGENERATE;

use super::{Bridge, EXPORTS, GLUE, REALIZE};

/// The page a person opens.
pub(super) fn html(bridge: &Bridge<'_>) -> String {
    let ir = bridge.ir;
    let regenerate = format!("{REGENERATE} --target {}", super::TARGET);
    let mut out = bridge.plan.provenance.html_comment_for(&regenerate);
    let title = format!("{} {}", ir.naming.display_or(&ir.system), ir.version);
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta \
         name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title} — a \
         synthesised system</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n<header>\n<h1 \
         id=\"system\">{title}</h1>\n<p id=\"summary\"></p>\n<dl \
         id=\"provenance\"></dl>\n<p id=\"realization\" class=\"note\"></p>\n</header>\n<main>\n<\
         section id=\"commands-panel\">\n<h2>Commands</h2>\n<p class=\"note\">Every command this \
         specification declares, with the input it takes. Sending one runs the component's port \
         and then pumps the transport until quiescent.</p>\n<div \
         id=\"commands\"></div>\n</section>\n<section id=\"outcome-panel\">\n<h2>Last \
         outcome</h2>\n<div id=\"outcome\"><p class=\"note\">Nothing sent \
         yet.</p></div>\n</section>\n<section id=\"log-panel\">\n<h2>Event log</h2>\n<p \
         class=\"note\">The system's observable record: everything any component published, in \
         publication order. Redelivering an occurrence is the duplicate an <code>at least \
         once</code> guarantee explicitly permits — the occurrence is not published again, and \
         every binding that reacts to it runs again.</p>\n<div id=\"log\"></div>\n</section>\n<sec\
         tion id=\"invocations-panel\">\n<h2>Binding invocations</h2>\n<p class=\"note\">What each \
         binding invoked, and the input it filled — the transport's own record.</p>\n<div \
         id=\"invocations\"></div>\n</section>\n<section id=\"state-panel\">\n<h2>State</h2>\n<p \
         class=\"note\">Instances are observable exactly as far as the model publishes them: \
         through declared views. The lifecycle beside each entity is what the specification says \
         may happen to one.</p>\n<div id=\"views\"></div>\n<div \
         id=\"entities\"></div>\n</section>\n<section id=\"model-panel\">\n<h2>Model</h2>\n<div \
         id=\"model\"></div>\n</section>\n</main>\n<script type=\"module\">\n{}\n</script>\n</body\
         >\n</html>\n",
        page_script(bridge)
    );
    out
}

/// The glue: the boundary protocol, written once.
pub(super) fn glue(bridge: &Bridge<'_>) -> String {
    let regenerate = format!("{REGENERATE} --target {}", super::TARGET);
    let mut out = bridge.plan.provenance.commented_for("//", &regenerate);
    let _ = write!(
        out,
        "{GLUE_BODY}\nexport const EXPORTS = [{}];\n\nexport const REALIZE = {REALIZE:?};\n",
        EXPORTS
            .iter()
            .map(|export| format!("{export:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    out
}

/// The page's own script: it loads the module, asks for the catalogue, and renders from it.
fn page_script(bridge: &Bridge<'_>) -> String {
    let module = crate::rust::layout::Layout::crate_ident(bridge.layout.package());
    format!(
        "import {{ open }} from \"./{GLUE}\";\n\n// Where `cargo build --release --target \
         wasm32-unknown-unknown` leaves the module, and where a host\n// that links a realization \
         leaves its own. Both answer this page: the exports travel from the\n// bridge's `rlib` \
         into the host's `cdylib`.\nconst CANDIDATES = \
         [\n  \"target/wasm32-unknown-unknown/release/{module}.wasm\",\n  \
         \"target/wasm32-unknown-unknown/debug/{module}.wasm\",\n  \
         \"{module}.wasm\",\n];\n{SCRIPT}"
    )
}

/// The fixed part of the page's script: everything that renders the catalogue.
const SCRIPT: &str = r##"
let system = null;
let catalog = null;

/** Loads the first module that answers, so a debug build and a release build both work. */
async function load() {
  const failures = [];
  for (const candidate of CANDIDATES) {
    try {
      const response = await fetch(candidate);
      if (!response.ok) { failures.push(`${candidate}: ${response.status}`); continue; }
      return { system: await open(await response.arrayBuffer()), from: candidate };
    } catch (error) {
      failures.push(`${candidate}: ${error.message}`);
    }
  }
  throw new Error(`no module answered.\n${failures.join("\n")}`);
}

/** One element, with attributes and children, because a page of `innerHTML` is a page of holes. */
function element(tag, attributes = {}, children = []) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attributes)) {
    if (key === "class") node.className = value;
    else if (key === "text") node.textContent = value;
    else node.setAttribute(key, value);
  }
  for (const child of [].concat(children)) {
    node.appendChild(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

/** A table with a header row and one row per record. */
function table(columns, rows) {
  const head = element("tr", {}, columns.map((column) => element("th", { text: column })));
  const body = rows.map((cells) => element("tr", {}, cells.map((cell) =>
    element("td", {}, typeof cell === "string" ? [cell] : [cell]))));
  return element("table", {}, [element("thead", {}, [head]), element("tbody", {}, body)]);
}

/** A value as compact JSON, which is what it is. */
function json(value) {
  return element("code", { class: "value", text: JSON.stringify(value) });
}

// ---- input controls, built from the declared type ------------------------------------------------

/** A control for one declared type reference: `{ node, read }`. `read()` may answer undefined. */
function control(type) {
  switch (type.kind) {
    case "primitive": return primitive(type.name);
    case "declared": return declared(type.name);
    case "optional": return optional(type.of);
    case "list": return list(type.of);
    case "map": return map(type);
    default: return primitive("string");
  }
}

function primitive(name) {
  if (name === "boolean") {
    const node = element("input", { type: "checkbox" });
    return { node, read: () => node.checked };
  }
  if (name === "integer") {
    const node = element("input", { type: "number", step: "1", value: "0" });
    return { node, read: () => Number.parseInt(node.value === "" ? "0" : node.value, 10) };
  }
  const hints = {
    decimal: "10.50",
    timestamp: "2026-01-01T00:00:00Z",
    duration: "P30D",
    uuid: "00000000-0000-4000-8000-000000000000",
    bytes: "base64",
  };
  const node = element("input", { type: "text", placeholder: hints[name] ?? "" });
  return { node, read: () => node.value };
}

function declared(name) {
  const declaration = catalog.types[name];
  if (!declaration) return primitive("string");
  if (declaration.kind === "newtype") return control(declaration.of);
  if (declaration.kind === "enum") {
    const node = element("select", {}, declaration.variants.map((variant) =>
      element("option", { value: variant, text: variant })));
    return { node, read: () => node.value };
  }
  if (declaration.kind === "struct") {
    return record(declaration.fields);
  }
  const labels = Object.keys(declaration.variants);
  const choice = element("select", {}, labels.map((label) =>
    element("option", { value: label, text: label })));
  const holder = element("div", { class: "nested" });
  let held = null;
  const show = () => {
    holder.replaceChildren();
    held = control(declaration.variants[choice.value].type);
    holder.appendChild(held.node);
  };
  choice.addEventListener("change", show);
  show();
  const node = element("div", { class: "union" }, [choice, holder]);
  return {
    node,
    read: () => {
      const value = { [declaration.tag]: choice.value };
      const payload = held.read();
      if (payload !== undefined) value[declaration.content] = payload;
      return value;
    },
  };
}

/** A group of declared fields, each labelled with its own name and type. */
function record(fields) {
  const parts = fields.map((field) => {
    const held = control(field.type);
    const label = element("label", {}, [
      element("span", { class: "field", text: field.display ?? field.name }),
      element("code", { class: "spelling", text: field.spelling }),
      held.node,
    ]);
    if (field.summary) label.appendChild(element("small", { text: field.summary }));
    return { field, held, label };
  });
  const node = element("div", { class: "record" }, parts.map((part) => part.label));
  return {
    node,
    read: () => {
      const value = {};
      for (const part of parts) {
        const held = part.held.read();
        if (held !== undefined) value[part.field.wire] = held;
      }
      return value;
    },
  };
}

function optional(of) {
  const present = element("input", { type: "checkbox" });
  const holder = element("div", { class: "nested" });
  const held = control(of);
  holder.appendChild(held.node);
  const sync = () => { holder.hidden = !present.checked; };
  present.addEventListener("change", sync);
  sync();
  const node = element("div", { class: "optional" }, [
    element("label", { class: "inline" }, [present, element("span", { text: "present" })]),
    holder,
  ]);
  return { node, read: () => (present.checked ? held.read() : undefined) };
}

function list(of) {
  const rows = [];
  const holder = element("div", { class: "nested" });
  const add = element("button", { type: "button", text: "add" });
  add.addEventListener("click", () => {
    const held = control(of);
    const remove = element("button", { type: "button", text: "remove" });
    const row = element("div", { class: "row" }, [held.node, remove]);
    remove.addEventListener("click", () => {
      holder.removeChild(row);
      rows.splice(rows.indexOf(entry), 1);
    });
    const entry = { held, row };
    rows.push(entry);
    holder.appendChild(row);
  });
  const node = element("div", { class: "list" }, [holder, add]);
  return { node, read: () => rows.map((entry) => entry.held.read()) };
}

function map(type) {
  const rows = [];
  const holder = element("div", { class: "nested" });
  const add = element("button", { type: "button", text: "add" });
  add.addEventListener("click", () => {
    const key = element("input", { type: "text", placeholder: type.key });
    const held = control(type.value);
    const remove = element("button", { type: "button", text: "remove" });
    const row = element("div", { class: "row" }, [key, held.node, remove]);
    remove.addEventListener("click", () => {
      holder.removeChild(row);
      rows.splice(rows.indexOf(entry), 1);
    });
    const entry = { key, held, row };
    rows.push(entry);
    holder.appendChild(row);
  });
  const node = element("div", { class: "map" }, [holder, add]);
  return {
    node,
    read: () => {
      const value = {};
      for (const entry of rows) value[entry.key.value] = entry.held.read();
      return value;
    },
  };
}

// ---- rendering ------------------------------------------------------------------------------------

function renderHeader(from, realized) {
  document.getElementById("system").textContent = `${catalog.display} ${catalog.version}`;
  document.getElementById("summary").textContent = catalog.summary ?? "";
  const provenance = document.getElementById("provenance");
  provenance.replaceChildren();
  const facts = [
    ["system", catalog.system],
    ["specification", catalog.provenance.specification_version],
    ["model digest", catalog.provenance.source_digest],
    ["contract digest", catalog.provenance.contract_digest],
    ["compiler", catalog.provenance.compiler_version],
    ["generator", catalog.provenance.generator_version],
    ["plan", `${catalog.plan.generated} generated · ${catalog.plan.obligations} obligations · ${catalog.plan.refused} refused`],
    ["module", from],
  ];
  for (const [term, definition] of facts) {
    provenance.appendChild(element("dt", { text: term }));
    provenance.appendChild(element("dd", { text: definition }));
  }
  document.getElementById("realization").textContent = realized
    ? "A realization is installed: the obligations below are implemented."
    : "No realization is installed. Every command will answer with the typed refusal naming what the plan owes — that is this tree's honest empty state.";
}

function renderCommands() {
  const holder = document.getElementById("commands");
  holder.replaceChildren();
  for (const command of catalog.commands) {
    const card = element("article", { class: "card" });
    card.appendChild(element("h3", { text: command.display }));
    card.appendChild(element("code", { class: "name", text: command.name }));
    if (command.summary) card.appendChild(element("p", { text: command.summary }));
    card.appendChild(element("p", { class: "note", text: command.component
      ? `accepted by ${command.component}`
      : `not dispatchable: ${command.refusal}` }));
    if (command.behavior && command.behavior.disposition === "obligation") {
      card.appendChild(element("details", {}, [
        element("summary", { text: "the behaviour is an obligation" }),
        element("p", { class: "note", text: command.behavior.why }),
        element("p", { class: "contract", text: command.behavior.contract }),
      ]));
    }
    const outcomes = command.outcomes.map((outcome) => [
      element("code", { text: outcome.name }),
      outcome.condition,
      outcome.publishes.length ? outcome.publishes.join(", ") : "—",
      outcome.error ?? "—",
    ]);
    card.appendChild(element("details", {}, [
      element("summary", { text: `${command.outcomes.length} declared outcome(s)` }),
      table(["outcome", "when", "publishes", "refuses with"], outcomes),
    ]));
    if (command.dispatchable) {
      const held = record(command.input);
      const send = element("button", { type: "button", text: `send ${command.display}` });
      send.addEventListener("click", () => dispatch(command.name, held.read()));
      card.appendChild(held.node);
      card.appendChild(send);
    }
    holder.appendChild(card);
  }
}

function renderOutcome(answer) {
  const holder = document.getElementById("outcome");
  holder.replaceChildren();
  if (!answer) { holder.appendChild(element("p", { class: "note", text: "Nothing sent yet." })); return; }
  if (answer.ok === false) {
    holder.appendChild(element("p", { class: "refused", text: `refused — ${answer.error.kind}` }));
    holder.appendChild(json(answer.error));
    return;
  }
  const outcome = answer.outcome;
  if (!outcome) { holder.appendChild(element("p", { class: "note", text: "Delivered again." })); return; }
  holder.appendChild(element("p", { class: "accepted" }, [
    element("code", { text: answer.command }),
    ` took the declared outcome `,
    element("strong", { text: outcome.outcome }),
  ]));
  if (outcome.refusal) {
    holder.appendChild(element("p", { class: "refused", text: `refused with ${outcome.refusal.error}` }));
    holder.appendChild(json(outcome.refusal.payload));
  }
  if (outcome.published && outcome.published.length) {
    holder.appendChild(table(["published", "payload"],
      outcome.published.map((entry) => [element("code", { text: entry.event }), json(entry.payload)])));
  }
}

function renderLog(log) {
  const holder = document.getElementById("log");
  holder.replaceChildren();
  if (!log.length) { holder.appendChild(element("p", { class: "note", text: "Nothing published yet." })); return; }
  holder.appendChild(table(["#", "event", "payload", ""], log.map((entry) => {
    const again = element("button", { type: "button", text: "redeliver" });
    again.addEventListener("click", () => redeliver(entry.occurrence));
    return [String(entry.occurrence), element("code", { text: entry.event }), json(entry.payload), again];
  })));
}

function renderInvocations(invocations) {
  const holder = document.getElementById("invocations");
  holder.replaceChildren();
  if (!invocations.length) { holder.appendChild(element("p", { class: "note", text: "No binding has invoked anything yet." })); return; }
  holder.appendChild(table(["binding", "reacting to", "invoked", "input"], invocations.map((entry) => [
    element("code", { text: entry.binding }),
    element("code", { text: entry.event }),
    element("code", { text: entry.command }),
    json(entry.input),
  ])));
}

function renderViews(views) {
  const holder = document.getElementById("views");
  holder.replaceChildren();
  for (const view of catalog.views) {
    const card = element("article", { class: "card" });
    card.appendChild(element("h3", { text: view.display }));
    card.appendChild(element("code", { class: "name", text: view.name }));
    card.appendChild(element("p", { class: "note", text:
      `projects ${view.entity} at ${view.consistency} consistency` + (view.filter ? `, where ${view.filter}` : "") }));
    const served = views[view.name];
    if (served && served.unmet) {
      card.appendChild(element("p", { class: "refused", text:
        `unmet obligation — ${served.unmet.capability} “${served.unmet.source}”. Nothing has implemented this projection.` }));
    } else if (served && served.rows.length) {
      card.appendChild(table(view.fields.map((field) => field.display ?? field.name),
        served.rows.map((row) => view.fields.map((field) => json(row[field.wire])))));
    } else {
      card.appendChild(element("p", { class: "note", text: "no rows" }));
    }
    holder.appendChild(card);
  }
}

function renderEntities() {
  const holder = document.getElementById("entities");
  holder.replaceChildren();
  for (const entity of catalog.entities) {
    const card = element("article", { class: "card" });
    card.appendChild(element("h3", { text: `${entity.display} — lifecycle` }));
    card.appendChild(element("code", { class: "name", text: entity.name }));
    card.appendChild(element("p", { class: "note", text:
      `starts in ${entity.initial}` + (entity.terminal.length ? `; may rest in ${entity.terminal.join(", ")}` : "") }));
    card.appendChild(table(["move", "from", "to"],
      entity.transitions.map((move) => [element("code", { text: move.name }), move.from.join(", "), move.to])));
    if (entity.views.length) {
      card.appendChild(element("p", { class: "note", text: `observable through ${entity.views.join(", ")}` }));
    } else {
      card.appendChild(element("p", { class: "note", text: "no declared view projects this entity, so its instances are not observable from here" }));
    }
    holder.appendChild(card);
  }
}

function renderModel() {
  const holder = document.getElementById("model");
  holder.replaceChildren();
  holder.appendChild(element("details", {}, [
    element("summary", { text: `${catalog.components.length} component(s)` }),
    table(["component", "accepts", "publishes"], catalog.components.map((component) => [
      element("code", { text: component.name }), component.accepts.join(", "), component.publishes.join(", "),
    ])),
  ]));
  holder.appendChild(element("details", {}, [
    element("summary", { text: `${catalog.bindings.length} binding(s)` }),
    table(["binding", "on", "invokes", "delivery", "on failure"], catalog.bindings.map((binding) => [
      element("code", { text: binding.name }),
      element("code", { text: binding.event }),
      element("code", { text: binding.command }),
      binding.delivery,
      binding.on_failure + (binding.escalation ? ` into ${binding.escalation}` : ""),
    ])),
  ]));
  holder.appendChild(element("details", {}, [
    element("summary", { text: `${catalog.conversions.length} declared crossing(s)` }),
    table(["from", "to", "because"], catalog.conversions.map((conversion) => [
      element("code", { text: conversion.from }), element("code", { text: conversion.to }), conversion.because,
    ])),
  ]));
  holder.appendChild(element("details", {}, [
    element("summary", { text: `${catalog.events.length} event(s)` }),
    table(["event", "carries"], catalog.events.map((event) => [
      element("code", { text: event.name }),
      event.fields.map((field) => `${field.name}: ${field.spelling}`).join(", ") || "—",
    ])),
  ]));
}

// ---- talking to the system ---------------------------------------------------------------------

function observe(answer) {
  renderOutcome(answer);
  if (answer.log) renderLog(answer.log);
  if (answer.invocations) renderInvocations(answer.invocations);
  if (answer.views) renderViews(answer.views);
}

function dispatch(command, input) {
  observe(system.request({ request: "command", command, input }));
}

function redeliver(occurrence) {
  observe(system.request({ request: "redeliver", occurrence }));
}

async function main() {
  let opened;
  try {
    opened = await load();
  } catch (error) {
    document.getElementById("realization").textContent = String(error.message);
    return;
  }
  system = opened.system;
  const answer = system.request({ request: "catalog" });
  catalog = answer.catalog;
  renderHeader(opened.from, system.realized);
  renderCommands();
  renderEntities();
  renderModel();
  observe(system.request({ request: "observe" }));
}

main();
"##;

/// The glue's fixed body: the whole boundary protocol.
const GLUE_BODY: &str = r#"
// The boundary, written once. The page imports it, and so does the smoke test that loads the
// module outside a browser — so what the test exercises is the page's own glue and not a second
// implementation that happens to agree with it.
//
// Three exports and nothing else. `memory.buffer` is read afresh on every access because
// allocating inside the module may grow the memory, which detaches every view taken before it.

/**
 * Instantiates a module and answers a driver over it.
 *
 * `source` is anything `WebAssembly.instantiate` accepts: the bytes of a `.wasm`, or a compiled
 * `Module`. If the module exports the optional realization hook, it is called once, before any
 * request — that is how a host that linked implementations of the plan's obligations reaches a
 * page neither of them was written against.
 */
export async function open(source) {
  const { instance } = await WebAssembly.instantiate(source, {});
  const exports = instance.exports;
  for (const name of EXPORTS) {
    if (typeof exports[name] !== "function") {
      throw new Error(`the module does not export ${name}; the page and the module disagree`);
    }
  }
  const realized = typeof exports[REALIZE] === "function";
  if (realized) exports[REALIZE]();

  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  function request(body) {
    const bytes = encoder.encode(JSON.stringify(body));
    const address = exports.ess_input_reserve(bytes.length);
    new Uint8Array(exports.memory.buffer, address, bytes.length).set(bytes);
    const at = exports.ess_dispatch();
    const length = exports.ess_output_len();
    const text = decoder.decode(new Uint8Array(exports.memory.buffer, at, length));
    return JSON.parse(text);
  }

  return { request, realized, exports };
}
"#;

/// The page's stylesheet.
///
/// Plain CSS, no framework and no download: a page whose first act is to fetch a stylesheet from
/// somebody else's server is a page that stops working offline, and this one is opened from a
/// directory.
const STYLE: &str = r#":root { color-scheme: light dark; --edge: color-mix(in srgb, currentColor 18%, transparent); }
* { box-sizing: border-box; }
body { margin: 0 auto; padding: 2rem 1.5rem 6rem; max-width: 62rem; font: 15px/1.55 ui-sans-serif, system-ui, sans-serif; }
header { border-bottom: 1px solid var(--edge); padding-bottom: 1.5rem; margin-bottom: 1rem; }
h1 { font-size: 1.6rem; margin: 0 0 .25rem; }
h2 { font-size: 1.1rem; margin: 2.5rem 0 .5rem; letter-spacing: .04em; text-transform: uppercase; opacity: .75; }
h3 { font-size: 1rem; margin: 0 0 .2rem; }
p { margin: .4rem 0; }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .85em; }
code.name { opacity: .6; }
code.spelling { opacity: .55; margin-left: .4rem; }
code.value { white-space: pre-wrap; word-break: break-word; }
.note { opacity: .7; font-size: .9em; }
.contract { border-left: 2px solid var(--edge); padding-left: .75rem; opacity: .85; }
.card { border: 1px solid var(--edge); border-radius: 8px; padding: 1rem 1.1rem; margin: .75rem 0; }
.record { display: grid; gap: .6rem; margin: .75rem 0; }
.record label { display: grid; gap: .15rem; }
.record label.inline, label.inline { display: flex; align-items: center; gap: .4rem; }
.field { font-weight: 600; }
.nested { border-left: 2px solid var(--edge); padding-left: .75rem; margin-top: .3rem; }
.row { display: flex; gap: .4rem; align-items: start; margin-bottom: .3rem; }
input[type="text"], input[type="number"], select { padding: .35rem .5rem; border: 1px solid var(--edge); border-radius: 5px; background: transparent; color: inherit; font: inherit; width: 100%; }
.row input { width: auto; flex: 1; }
button { padding: .4rem .8rem; border: 1px solid var(--edge); border-radius: 5px; background: transparent; color: inherit; font: inherit; cursor: pointer; }
button:hover { border-color: currentColor; }
table { border-collapse: collapse; width: 100%; margin: .5rem 0; }
th, td { text-align: left; vertical-align: top; padding: .35rem .6rem .35rem 0; border-bottom: 1px solid var(--edge); }
th { font-size: .78em; letter-spacing: .05em; text-transform: uppercase; opacity: .6; font-weight: 600; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: .1rem .9rem; margin: .75rem 0 0; font-size: .85em; }
dt { opacity: .6; }
dd { margin: 0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; word-break: break-all; }
details { margin: .5rem 0; }
summary { cursor: pointer; opacity: .8; }
.accepted strong { color: color-mix(in srgb, currentColor 40%, green); }
.refused { color: color-mix(in srgb, currentColor 35%, crimson); }
"#;
