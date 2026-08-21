//! The scene as SVG.
//!
//! # The same figure as the ones already published
//!
//! The palette, the type stack and the panel geometry come from
//! `website/static/img/trace-evidence-gate.svg`, so a rendered workflow can sit next to the
//! repository's other diagrams without looking like it came from a different project. The classes
//! are declared once in a `<style>` block and every shape names one, which is also what makes the
//! HTML page's live overlay possible: repainting a run means swapping a class, not rewriting a
//! path.
//!
//! # Byte-identical twice
//!
//! Nothing here reads a clock, a locale or an environment variable, every number is an `i32`
//! computed by [`crate::scene`], and every collection was ordered before it arrived. Rendering the
//! same [`Scene`] twice therefore produces the same bytes, which `tests/rendering.rs` asserts
//! rather than assumes — a figure that is committed and re-rendered must not turn up in a diff for
//! having chosen a different iteration order.

use std::fmt::Write as _;

use crate::run::RunStatus;
use crate::scene::{units, Edge, EdgeAccent, Node, NodeAccent, Scene};
use crate::theme;

/// Writes the scene as a standalone SVG document.
pub fn render(scene: &Scene) -> String {
    let mut out = String::with_capacity(4096);
    let _ = writeln!(
        out,
        "<svg viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" \
         xmlns=\"http://www.w3.org/2000/svg\" font-family=\"{font}\" role=\"img\" \
         aria-label=\"{label}\">",
        width = scene.width,
        height = scene.height,
        font = theme::MONO,
        label = escape(&aria_label(scene)),
    );
    out.push_str(&style());
    out.push_str(&markers());
    let _ = writeln!(
        out,
        "  <rect class=\"bg\" width=\"{}\" height=\"{}\"/>",
        scene.width, scene.height
    );
    header(&mut out, scene);
    // Edges first, so a box is never drawn under an arrow.
    for edge in &scene.edges {
        write_edge(&mut out, edge);
    }
    for node in &scene.nodes {
        write_node(&mut out, node);
    }
    footer(&mut out, scene);
    out.push_str("</svg>\n");
    out
}

/// One sentence describing the picture, for a screen reader.
///
/// A diagram with no text alternative is a diagram that is not there for some readers, and the
/// figures this repository already publishes carry one — so this one does too, and it says what the
/// run is doing rather than that a diagram exists.
fn aria_label(scene: &Scene) -> String {
    let mut label = format!(
        "Workflow {}: {} states and {} transitions",
        scene.reference,
        scene.nodes.len(),
        scene.edges.len()
    );
    if let Some(run) = &scene.run {
        if let Some(current) = &run.current {
            let _ = write!(label, ". A run is in `{current}` and is {}", run.status);
        }
        if !run.reasons.is_empty() {
            let _ = write!(label, ", for {} stated reason(s)", run.reasons.len());
        }
    }
    label.push('.');
    label
}

/// The class definitions the whole document shares.
fn style() -> String {
    format!(
        "  <style>\n\
         \x20   .bg {{ fill: {bg}; }}\n\
         \x20   .box {{ fill: {panel}; stroke: {line}; stroke-width: 1; }}\n\
         \x20   .box-visited {{ fill: {panel}; stroke: {line}; stroke-width: 1; opacity: .55; }}\n\
         \x20   .box-current {{ fill: {panel}; stroke: {blue}; stroke-width: 2; }}\n\
         \x20   .box-stopped {{ fill: {panel}; stroke: {red}; stroke-width: 2; }}\n\
         \x20   .box-done {{ fill: {panel}; stroke: {green}; stroke-width: 2; }}\n\
         \x20   .visited {{ opacity: .55; }}\n\
         \x20   .h1 {{ fill: {text}; font-size: 15px; letter-spacing: .04em; }}\n\
         \x20   .h2 {{ fill: {muted}; font-size: 11px; letter-spacing: .06em; }}\n\
         \x20   .t {{ fill: {text}; font-size: 13px; }}\n\
         \x20   .dim {{ fill: {muted}; font-size: 10px; }}\n\
         \x20   .guard {{ fill: {muted}; font-size: 11px; }}\n\
         \x20   .ok {{ fill: {green}; font-size: 11px; }}\n\
         \x20   .amber {{ fill: {amber}; font-size: 11px; }}\n\
         \x20   .red {{ fill: {red}; font-size: 11px; }}\n\
         \x20   .blue {{ fill: {blue}; font-size: 11px; }}\n\
         \x20   .edge {{ fill: none; stroke: {line}; stroke-width: 1.5; }}\n\
         \x20   .edge-back {{ stroke-dasharray: 4 4; }}\n\
         \x20   .edge-taken {{ fill: none; stroke: {green}; stroke-width: 2; }}\n\
         \x20   .edge-retreat {{ fill: none; stroke: {amber}; stroke-width: 2; }}\n\
         \x20 </style>\n",
        bg = theme::BACKGROUND,
        panel = theme::PANEL,
        line = theme::LINE,
        text = theme::TEXT,
        muted = theme::MUTED,
        green = theme::GREEN,
        blue = theme::BLUE,
        amber = theme::AMBER,
        red = theme::RED,
    )
}

/// One arrowhead per edge colour.
///
/// Three markers rather than one recoloured marker, because SVG 1.1 gives a marker no way to
/// inherit its referrer's stroke — `context-stroke` is SVG 2, and `rsvg-convert` is the rasteriser
/// this repository shells out to.
fn markers() -> String {
    let mut out = String::from("  <defs>\n");
    for (id, colour) in [
        ("arrow-idle", theme::LINE),
        ("arrow-taken", theme::GREEN),
        ("arrow-retreat", theme::AMBER),
    ] {
        let _ = writeln!(
            out,
            "    <marker id=\"{id}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
             markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\n\
             \x20     <path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"{colour}\"/>\n\
             \x20   </marker>"
        );
    }
    out.push_str("  </defs>\n");
    out
}

/// The title block.
fn header(out: &mut String, scene: &Scene) {
    let _ = writeln!(
        out,
        "  <text class=\"h1\" x=\"{}\" y=\"32\">{}</text>",
        28,
        escape(&scene.title)
    );
    let _ = writeln!(
        out,
        "  <text class=\"h2\" x=\"28\" y=\"50\">{} · {} states · {} transitions</text>",
        escape(&scene.reference),
        scene.nodes.len(),
        scene.edges.len()
    );
    if let Some(line) = scene.run_line() {
        let _ = writeln!(
            out,
            "  <text class=\"{}\" x=\"28\" y=\"70\">{}</text>",
            status_class(scene.status()),
            escape(&line)
        );
    }
}

/// One state box, its labels and whatever the overlay has to say about it.
fn write_node(out: &mut String, node: &Node) {
    let frame = node.frame;
    let class = match node.accent {
        NodeAccent::Idle => "box",
        NodeAccent::Visited => "box-visited",
        NodeAccent::Current => "box-current",
        NodeAccent::Stopped => "box-stopped",
        NodeAccent::Done => "box-done",
    };
    let group = if node.accent == NodeAccent::Visited {
        " class=\"visited\""
    } else {
        ""
    };
    // Addressable by state id, which is what lets the HTML page's live block repaint an overlay by
    // swapping a class instead of re-fetching the whole figure.
    let _ = writeln!(
        out,
        "  <g id=\"state-{}\"{group}>",
        escape(node.id.as_ref())
    );
    let _ = writeln!(
        out,
        "    <rect class=\"{class}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/>",
        frame.x, frame.y, frame.w, frame.h
    );
    let _ = writeln!(
        out,
        "    <text class=\"t\" x=\"{}\" y=\"{}\">{}</text>",
        frame.x + 14,
        frame.y + 24,
        escape(&node.title)
    );
    let mut caption = node.id.as_ref().to_owned();
    if !node.phases.is_empty() {
        let _ = write!(caption, " · {}", node.phases.join(", "));
    }
    if node.terminal {
        caption.push_str(" · terminal");
    }
    if node.irreversible {
        caption.push_str(" · irreversible");
    }
    let _ = writeln!(
        out,
        "    <text class=\"dim\" x=\"{}\" y=\"{}\">{}</text>",
        frame.x + 14,
        frame.y + 42,
        escape(&caption)
    );
    // Only a repeat visit earns a badge: `×1` on every box is nine pieces of ink saying nothing.
    if let Some(visits) = node.visits.filter(|count| *count > 1) {
        let _ = writeln!(
            out,
            "    <text class=\"amber\" x=\"{}\" y=\"{}\" text-anchor=\"end\">×{visits}</text>",
            frame.right() - 12,
            frame.y + 24
        );
    }
    for (index, line) in node.requires.iter().enumerate() {
        let _ = writeln!(
            out,
            "    <text class=\"dim\" x=\"{}\" y=\"{}\">requires {}</text>",
            frame.right() + 14,
            frame.y + 24 + units(index) * 14,
            escape(line)
        );
    }
    out.push_str("  </g>\n");
}

/// One transition: its polyline, its arrowhead and its guard.
fn write_edge(out: &mut String, edge: &Edge) {
    let (class, marker) = match edge.accent {
        EdgeAccent::Idle if edge.back => ("edge edge-back", "arrow-idle"),
        EdgeAccent::Idle => ("edge", "arrow-idle"),
        EdgeAccent::Taken => ("edge-taken", "arrow-taken"),
        EdgeAccent::Retreat => ("edge-retreat", "arrow-retreat"),
    };
    let points = edge
        .points
        .iter()
        .map(|point| format!("{},{}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = writeln!(
        out,
        "  <polyline class=\"{class}\" points=\"{points}\" marker-end=\"url(#{marker})\"/>"
    );
    if let Some(text) = &edge.label_text {
        let class = match edge.accent {
            EdgeAccent::Retreat => "amber",
            EdgeAccent::Taken => "ok",
            EdgeAccent::Idle => "guard",
        };
        let _ = writeln!(
            out,
            "  <text class=\"{class}\" x=\"{}\" y=\"{}\">{}</text>",
            edge.label.x,
            edge.label.y,
            escape(text)
        );
    }
}

/// Evidence counts, and the reasons a run stopped.
fn footer(out: &mut String, scene: &Scene) {
    let mut y = scene.footer_y;
    if !scene.evidence.is_empty() {
        let summary = scene
            .evidence
            .iter()
            .map(|(kind, count)| format!("{kind} ×{count}"))
            .collect::<Vec<_>>()
            .join(" · ");
        let _ = writeln!(
            out,
            "  <text class=\"dim\" x=\"28\" y=\"{y}\">evidence  {}</text>",
            escape(&summary)
        );
        y += 18;
    }
    if scene.reasons.is_empty() {
        return;
    }
    let colour = if scene.status().is_stopped() {
        theme::RED
    } else {
        theme::AMBER
    };
    let bar_height = 4 + units(scene.reasons.len()) * 18;
    let _ = writeln!(
        out,
        "  <rect x=\"28\" y=\"{}\" width=\"4\" height=\"{bar_height}\" fill=\"{colour}\" rx=\"2\"/>",
        y - 12
    );
    let _ = writeln!(
        out,
        "  <text class=\"{}\" x=\"42\" y=\"{y}\">{}</text>",
        status_class(scene.status()),
        escape(&format!("{}:", scene.status()))
    );
    y += 18;
    // Verbatim, in the order the engine produced them. See `RunView::reasons`.
    for reason in &scene.reasons {
        let _ = writeln!(
            out,
            "  <text class=\"dim\" x=\"42\" y=\"{y}\">{}</text>",
            escape(reason)
        );
        y += 18;
    }
}

/// The text class that says how a run stands.
fn status_class(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Completed => "ok",
        RunStatus::Running => "blue",
        RunStatus::Blocked | RunStatus::Broken => "red",
        RunStatus::Waiting | RunStatus::Exhausted => "amber",
        RunStatus::Unknown => "dim",
    }
}

/// XML-escapes text going into an element or an attribute.
///
/// All five, including the apostrophe: a guard is authored text and a summary is prose, and
/// `won't` inside a single-quoted attribute would end the attribute. There is no separate element
/// and attribute escaper because one that is safe in both places is one nobody can pick wrongly.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fixture_workflow, mid_run, snapshot};

    #[test]
    fn the_bare_workflow_renders_as_the_committed_figure() {
        snapshot(
            "adp-default.svg",
            &render(&Scene::build(&fixture_workflow(), None)),
        );
    }

    #[test]
    fn the_mid_run_overlay_renders_as_the_committed_figure() {
        snapshot(
            "adp-default-mid-run.svg",
            &render(&Scene::build(&fixture_workflow(), Some(&mid_run()))),
        );
    }

    #[test]
    fn the_same_scene_renders_to_the_same_bytes_twice() {
        let workflow = fixture_workflow();
        let run = mid_run();
        let scene = Scene::build(&workflow, Some(&run));
        assert_eq!(render(&scene), render(&scene));
        // And a rebuilt scene, which is what live mode does every 500 ms.
        let rebuilt = Scene::build(&workflow, Some(&mid_run()));
        assert_eq!(
            render(&scene),
            render(&rebuilt),
            "a re-render of the same run must not produce a diff"
        );
    }

    #[test]
    fn a_blocked_run_draws_a_red_current_box_and_prints_its_reasons_unedited() {
        let workflow = fixture_workflow();
        let run = mid_run();
        let svg = render(&Scene::build(&workflow, Some(&run)));
        assert!(
            svg.contains("class=\"box-stopped\""),
            "the current box of a blocked run is the red one"
        );
        assert!(svg.contains(theme::RED), "the red bar names the house red");
        for reason in &run.reasons {
            assert!(
                svg.contains(&escape(reason)),
                "the reason `{reason}` must appear verbatim"
            );
        }
    }

    #[test]
    fn the_taken_retreat_is_amber_and_an_untaken_edge_is_not() {
        let workflow = fixture_workflow();
        let taken = render(&Scene::build(&workflow, Some(&mid_run())));
        assert!(
            taken.contains("class=\"edge-retreat\""),
            "the run went back, so the retreat is amber"
        );
        // Past the stylesheet, which declares every class whether or not a shape uses one.
        let bare = render(&Scene::build(&workflow, None));
        let drawn = bare.rsplit("</style>").next().expect("a body");
        assert!(
            !drawn.contains("class=\"edge-retreat\""),
            "with no run, nothing has been taken and nothing is amber"
        );
        assert!(
            drawn.contains("class=\"edge edge-back\""),
            "an untaken retreat is still drawn, dashed"
        );
    }

    #[test]
    fn authored_text_is_escaped_rather_than_written_into_the_markup() {
        assert_eq!(escape("a < b && c"), "a &lt; b &amp;&amp; c");
        assert_eq!(escape("won't \"quote\""), "won&#39;t &quot;quote&quot;");
        let workflow = fixture_workflow();
        let svg = render(&Scene::build(&workflow, None));
        // The guards contain `>` and `<`; none of them may reach the document raw.
        let body = svg
            .rsplit("</style>")
            .next()
            .expect("the document has a body");
        assert!(
            !body.contains("failed > 0"),
            "a guard must be escaped before it is written"
        );
        assert!(body.contains("failed &gt; 0"));
    }

    #[test]
    fn the_document_declares_its_size_and_carries_a_text_alternative() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, Some(&mid_run()));
        let svg = render(&scene);
        assert!(svg.starts_with(&format!(
            "<svg viewBox=\"0 0 {} {}\"",
            scene.width, scene.height
        )));
        assert!(svg.contains("aria-label=\"Workflow adp/default/1"));
        assert!(
            svg.contains("is blocked"),
            "the alternative text says what the run is doing"
        );
        assert!(svg.trim_end().ends_with("</svg>"));
    }
}
