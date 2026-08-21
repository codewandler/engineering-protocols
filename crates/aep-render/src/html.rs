//! The scene as one self-contained page.
//!
//! # Self-contained means self-contained
//!
//! Decision 5 of the renderer plan, and it is stricter here than in `infra view`: that page loads
//! Mermaid from a CDN, so it needs a network the first time it is opened. This one embeds the SVG
//! the same crate just produced, its own stylesheet and its own script, and reaches nothing. A page
//! you can mail to somebody, commit, or open on a machine with no network is the point — the same
//! argument that keeps `task check` off the network, applied to the artefact rather than to the
//! gate.
//!
//! # The live block, and why it is inert in a file you double-click
//!
//! There is no server in this repository and none is coming from here — the no-daemon posture is a
//! decision, not an omission. What the page carries instead is preparation: a script that does
//! nothing at all under `file://`, and that under `http://` polls a sibling `run.json` every two
//! seconds and repaints the overlay. Nothing writes that file today, which is why the failure path
//! is *silence* rather than an error banner — a page that shouted about a missing file every two
//! seconds would be a worse static page than one with no script in it.
//!
//! The repaint swaps CSS classes on the groups [`crate::svg`] gives each state. It never rebuilds
//! the figure, because the figure's geometry is a function of the workflow and the workflow does
//! not change while a run is going.

use std::fmt::Write as _;

use crate::run::RunStatus;
use crate::scene::{EdgeAccent, NodeAccent, Scene};
use crate::svg::escape;
use crate::theme;

/// Writes the scene as one self-contained HTML page.
pub fn render(scene: &Scene) -> String {
    let mut out = String::with_capacity(8192);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(
        out,
        "<title>{} — {}</title>",
        escape(&scene.title),
        escape(&scene.reference)
    );
    out.push_str(&stylesheet());
    out.push_str("</head>\n<body>\n");

    let _ = writeln!(out, "<h1>{}</h1>", escape(&scene.title));
    let _ = writeln!(
        out,
        "<p class=\"ref\">{} · {} states · {} transitions</p>",
        escape(&scene.reference),
        scene.nodes.len(),
        scene.edges.len()
    );
    if let Some(summary) = &scene.summary {
        let _ = writeln!(out, "<p class=\"summary\">{}</p>", escape(summary));
    }
    let _ = writeln!(
        out,
        "<p id=\"run-line\" class=\"status {}\">{}</p>",
        status_class(scene.status()),
        escape(&scene.run_line().unwrap_or_else(|| "no run".to_owned()))
    );

    out.push_str("<figure>\n");
    out.push_str(&crate::svg::render(scene));
    out.push_str("</figure>\n");

    reasons(&mut out, scene);
    states(&mut out, scene);
    transitions(&mut out, scene);
    evidence(&mut out, scene);

    out.push_str(&live_script());
    out.push_str("</body>\n</html>\n");
    out
}

/// The page's own stylesheet, in the house palette.
fn stylesheet() -> String {
    format!(
        "<style>\n\
         :root {{ color-scheme: dark; }}\n\
         body {{ margin: 2rem auto; max-width: 78rem; padding: 0 1.5rem;\n\
         \x20 background: {bg}; color: {text};\n\
         \x20 font: 14px/1.6 {mono}; }}\n\
         h1 {{ font-size: 1.3rem; font-weight: 600; margin: 0 0 .2rem; }}\n\
         h2 {{ font-size: 1rem; margin: 2.4rem 0 .6rem; color: {muted};\n\
         \x20 letter-spacing: .06em; text-transform: uppercase; }}\n\
         p.ref, p.summary {{ color: {muted}; margin: .2rem 0; }}\n\
         p.status {{ margin: .8rem 0 0; }}\n\
         figure {{ margin: 1.4rem 0 0; padding: 0; overflow-x: auto; }}\n\
         table {{ border-collapse: collapse; width: 100%; }}\n\
         th, td {{ border: 1px solid {line}; padding: .3rem .6rem; text-align: left;\n\
         \x20 vertical-align: top; }}\n\
         th {{ color: {muted}; font-weight: 500; background: {panel}; }}\n\
         td.wrap {{ white-space: normal; }}\n\
         code {{ background: {panel}; padding: 0 .3rem; border-radius: 3px; }}\n\
         ul.reasons {{ margin: .4rem 0 0; padding-left: 1.2rem; }}\n\
         .ok {{ color: {green}; }} .blue {{ color: {blue}; }}\n\
         .amber {{ color: {amber}; }} .red {{ color: {red}; }} .dim {{ color: {muted}; }}\n\
         .bar {{ border-left: 4px solid {red}; padding-left: .8rem; }}\n\
         .bar-amber {{ border-left-color: {amber}; }}\n\
         </style>\n",
        bg = theme::BACKGROUND,
        panel = theme::PANEL,
        line = theme::LINE,
        text = theme::TEXT,
        muted = theme::MUTED,
        green = theme::GREEN,
        blue = theme::BLUE,
        amber = theme::AMBER,
        red = theme::RED,
        mono = theme::MONO,
    )
}

/// Why the run stopped, verbatim, directly under the diagram.
fn reasons(out: &mut String, scene: &Scene) {
    if scene.reasons.is_empty() {
        let _ = writeln!(out, "<ul id=\"reasons\" class=\"reasons\" hidden></ul>");
        return;
    }
    let bar = if scene.status().is_stopped() {
        "bar"
    } else {
        "bar bar-amber"
    };
    let _ = writeln!(
        out,
        "<div class=\"{bar}\"><p class=\"{}\">{}</p>",
        status_class(scene.status()),
        escape(&format!("{}:", scene.status()))
    );
    out.push_str("<ul id=\"reasons\" class=\"reasons\">\n");
    // Verbatim. See `RunView::reasons`: these are the engine's sentences, not this crate's.
    for reason in &scene.reasons {
        let _ = writeln!(out, "<li>{}</li>", escape(reason));
    }
    out.push_str("</ul>\n</div>\n");
}

/// The states table.
fn states(out: &mut String, scene: &Scene) {
    out.push_str(
        "<h2>States</h2>\n<table>\n<tr><th>state</th><th>title</th><th>phases</th>\
                  <th>requires</th><th>run</th></tr>\n",
    );
    for node in &scene.nodes {
        let mut marks = Vec::new();
        if node.terminal {
            marks.push("terminal".to_owned());
        }
        if node.irreversible {
            marks.push("irreversible".to_owned());
        }
        if let Some(visits) = node.visits {
            marks.push(format!("entered ×{visits}"));
        }
        let mut run = accent_word(node.accent).to_owned();
        if !marks.is_empty() {
            let _ = write!(run, " · {}", marks.join(" · "));
        }
        let _ = writeln!(
            out,
            "<tr><td><code>{id}</code></td><td>{title}</td><td class=\"dim\">{phases}</td>\
             <td class=\"wrap dim\">{requires}</td><td class=\"{class}\">{run}</td></tr>",
            id = escape(node.id.as_ref()),
            title = escape(&node.title),
            phases = escape(&node.phases.join(", ")),
            requires = escape(&node.requires.join("; ")),
            class = accent_class(node.accent),
            run = escape(&run),
        );
    }
    out.push_str("</table>\n");
}

/// The transitions table.
fn transitions(out: &mut String, scene: &Scene) {
    out.push_str(
        "<h2>Transitions</h2>\n<table>\n<tr><th>from</th><th>to</th><th>guard</th>\
         <th>requires</th><th>means</th><th>run</th></tr>\n",
    );
    for edge in &scene.edges {
        let run = match edge.accent {
            EdgeAccent::Idle if edge.back => "not taken (retreat)".to_owned(),
            EdgeAccent::Idle => "not taken".to_owned(),
            EdgeAccent::Taken => format!("taken ×{}", edge.taken),
            EdgeAccent::Retreat => format!("went back ×{}", edge.taken),
        };
        let class = match edge.accent {
            EdgeAccent::Idle => "dim",
            EdgeAccent::Taken => "ok",
            EdgeAccent::Retreat => "amber",
        };
        let _ = writeln!(
            out,
            "<tr><td><code>{from}</code></td><td><code>{to}</code></td>\
             <td class=\"wrap\"><code>{guard}</code></td><td class=\"wrap dim\">{requires}</td>\
             <td class=\"wrap dim\">{means}</td><td class=\"{class}\">{run}</td></tr>",
            from = escape(edge.from.as_ref()),
            to = escape(edge.to.as_ref()),
            guard = escape(edge.guard.as_deref().unwrap_or("—")),
            requires = escape(&edge.requires.join("; ")),
            means = escape(edge.description.as_deref().unwrap_or("")),
            run = escape(&run),
        );
    }
    out.push_str("</table>\n");
}

/// The evidence table, when a run produced any.
fn evidence(out: &mut String, scene: &Scene) {
    if scene.evidence.is_empty() {
        return;
    }
    out.push_str("<h2>Evidence</h2>\n<table>\n<tr><th>kind</th><th>records</th></tr>\n");
    for (kind, count) in &scene.evidence {
        let _ = writeln!(
            out,
            "<tr><td><code>{}</code></td><td>{count}</td></tr>",
            escape(kind)
        );
    }
    out.push_str("</table>\n");
}

/// The overlay-repainting block. Inert under `file://`; see the module documentation.
fn live_script() -> String {
    format!(
        "<script>\n\
         (function () {{\n\
         \x20 if (window.location.protocol === 'file:') return;\n\
         \x20 var BOX = {{ idle: 'box', visited: 'box-visited', current: 'box-current',\n\
         \x20   stopped: 'box-stopped', done: 'box-done' }};\n\
         \x20 var STOPPED = ['blocked', 'budget-exhausted', 'store-broken'];\n\
         \x20 function paint(run) {{\n\
         \x20   var path = run.path || [];\n\
         \x20   var stopped = STOPPED.indexOf(run.status) !== -1;\n\
         \x20   var groups = document.querySelectorAll('g[id^=\"state-\"]');\n\
         \x20   for (var i = 0; i !== groups.length; i++) {{\n\
         \x20     var group = groups[i];\n\
         \x20     var id = group.id.slice('state-'.length);\n\
         \x20     var rect = group.querySelector('rect');\n\
         \x20     if (!rect) continue;\n\
         \x20     var kind = 'idle';\n\
         \x20     if (path.indexOf(id) !== -1) kind = 'visited';\n\
         \x20     if (run.current === id) {{\n\
         \x20       kind = stopped ? 'stopped' : (run.status === 'completed' ? 'done' : 'current');\n\
         \x20     }}\n\
         \x20     rect.setAttribute('class', BOX[kind]);\n\
         \x20     group.setAttribute('class', kind === 'visited' ? 'visited' : '');\n\
         \x20   }}\n\
         \x20   var line = document.getElementById('run-line');\n\
         \x20   if (line && run.line) line.textContent = run.line;\n\
         \x20   var reasons = document.getElementById('reasons');\n\
         \x20   if (reasons && run.reasons) {{\n\
         \x20     reasons.textContent = '';\n\
         \x20     reasons.hidden = run.reasons.length === 0;\n\
         \x20     for (var r = 0; r !== run.reasons.length; r++) {{\n\
         \x20       var item = document.createElement('li');\n\
         \x20       item.textContent = run.reasons[r];\n\
         \x20       reasons.appendChild(item);\n\
         \x20     }}\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 function poll() {{\n\
         \x20   fetch('{LIVE_FILE}', {{ cache: 'no-store' }})\n\
         \x20     .then(function (response) {{ return response.ok ? response.json() : null; }})\n\
         \x20     .then(function (run) {{ if (run) paint(run); }})\n\
         \x20     .catch(function () {{ /* no sibling run.json: stay the static page */ }});\n\
         \x20 }}\n\
         \x20 poll();\n\
         \x20 window.setInterval(poll, {LIVE_INTERVAL_MS});\n\
         }})();\n\
         </script>\n"
    )
}

/// The sibling document the live block polls for.
pub const LIVE_FILE: &str = "run.json";

/// How often it polls, in milliseconds.
pub const LIVE_INTERVAL_MS: u32 = 2000;

/// The word a table cell uses for an overlay accent.
fn accent_word(accent: NodeAccent) -> &'static str {
    match accent {
        NodeAccent::Idle => "not reached",
        NodeAccent::Visited => "visited",
        NodeAccent::Current => "current",
        NodeAccent::Stopped => "current, stopped",
        NodeAccent::Done => "current, complete",
    }
}

/// The CSS class for an overlay accent.
fn accent_class(accent: NodeAccent) -> &'static str {
    match accent {
        NodeAccent::Idle => "dim",
        // Green for both: a state the run finished with and a workflow the run finished are the
        // same colour on purpose — nothing is owed there.
        NodeAccent::Visited | NodeAccent::Done => "ok",
        NodeAccent::Current => "blue",
        NodeAccent::Stopped => "red",
    }
}

/// The CSS class for a run status.
fn status_class(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Completed => "ok",
        RunStatus::Running => "blue",
        RunStatus::Blocked | RunStatus::Broken => "red",
        RunStatus::Waiting | RunStatus::Exhausted => "amber",
        RunStatus::Unknown => "dim",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fixture_workflow, mid_run, snapshot};

    #[test]
    fn the_page_reaches_nothing_outside_itself() {
        let workflow = fixture_workflow();
        let page = render(&Scene::build(&workflow, Some(&mid_run())));
        // `xmlns="http://www.w3.org/2000/svg"` is deliberately not on this list: an XML namespace
        // is an identifier and is never dereferenced. Everything that *would* be fetched is.
        for forbidden in [
            "<link",
            "<img",
            "<iframe",
            "@import",
            "src=\"http",
            "href=\"http",
            "url(http",
        ] {
            assert!(
                !page.contains(forbidden),
                "a self-contained page must not carry `{forbidden}`"
            );
        }
        assert!(
            page.contains("fetch('run.json'"),
            "the only fetch is the sibling document, which is a relative path"
        );
    }

    #[test]
    fn the_live_block_does_nothing_in_a_file_you_double_click() {
        let page = render(&Scene::build(&fixture_workflow(), None));
        assert!(
            page.contains("if (window.location.protocol === 'file:') return;"),
            "the guard is the first statement of the block"
        );
        assert!(page.contains("window.setInterval(poll, 2000)"));
    }

    #[test]
    fn the_figure_is_embedded_rather_than_linked() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, None);
        let page = render(&scene);
        assert!(
            page.contains(&crate::svg::render(&scene)),
            "the SVG is inline"
        );
        assert!(!page.contains("<img"), "nothing is loaded from elsewhere");
    }

    #[test]
    fn a_blocked_run_lists_its_reasons_verbatim_and_marks_the_state() {
        let workflow = fixture_workflow();
        let run = mid_run();
        let page = render(&Scene::build(&workflow, Some(&run)));
        for reason in &run.reasons {
            assert!(
                page.contains(&format!("<li>{}</li>", escape(reason))),
                "the reason `{reason}` must be one list item, unedited"
            );
        }
        assert!(page.contains("current, stopped"));
        assert!(page.contains("went back ×1"), "the retreat is in the table");
    }

    #[test]
    fn the_mid_run_page_renders_as_the_committed_document() {
        snapshot(
            "adp-default-mid-run.html",
            &render(&Scene::build(&fixture_workflow(), Some(&mid_run()))),
        );
    }

    #[test]
    fn the_same_scene_renders_to_the_same_page_twice() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, Some(&mid_run()));
        assert_eq!(render(&scene), render(&scene));
    }
}
