//! The scene as a terminal frame.
//!
//! # No `ratatui`, and the refusal is recorded here
//!
//! Decision 4 of the renderer plan. `ratatui` is a good crate and this is one screen: a title, a
//! column of states, the guards between them and a footer. Taking it would add a terminal backend,
//! an event loop and a widget tree — plus `crossterm` and its platform code — to a workspace whose
//! dependency rule is *prefer no dependency, and record the refusal*. What it would buy is layout
//! for a list, and this file is that layout in two hundred lines of `String`.
//!
//! # A frame is a value, not a screen
//!
//! [`frame`] returns a `String` and touches nothing. It reads no clock, opens no terminal, clears
//! nothing and knows nothing about how often it is called. `--watch` therefore lives in the CLI,
//! where the poll interval and the ANSI clear sequence belong: a crate that owned the loop would be
//! a crate that could not be tested without one, and the determinism scan over this crate would
//! have to make an exception for a timer.
//!
//! # Colour, and the escape-stripped snapshot
//!
//! The colours are the sixteen a terminal is allowed to have, mapped from the house palette in
//! [`crate::theme`]. [`strip`] removes them, which is how the frame gets a snapshot test that reads
//! as text — asserting on a string full of `\x1b[92m` tells a reviewer nothing about what the frame
//! says.

use std::fmt::Write as _;

use crate::run::RunStatus;
use crate::scene::{EdgeAccent, Node, NodeAccent, Scene};
use crate::theme;

/// The scene as a terminal frame, with ANSI colour.
///
/// No trailing clear, no cursor movement and no clock: the caller decides when to draw it and what
/// to do with the screen first.
pub fn frame(scene: &Scene) -> String {
    let id_width = scene
        .nodes
        .iter()
        .map(|node| node.id.as_ref().chars().count())
        .max()
        .unwrap_or(0)
        .max(8);
    let title_width = scene
        .nodes
        .iter()
        .map(|node| node.title.chars().count())
        .max()
        .unwrap_or(0)
        .max(8);

    let mut out = String::with_capacity(2048);
    let _ = writeln!(
        out,
        "{bold}{title}{reset}",
        bold = theme::ANSI_BOLD,
        title = scene.title,
        reset = theme::ANSI_RESET
    );
    let _ = writeln!(
        out,
        "{dim}{} · {} states · {} transitions{reset}",
        scene.reference,
        scene.nodes.len(),
        scene.edges.len(),
        dim = theme::ANSI_MUTED,
        reset = theme::ANSI_RESET
    );
    if let Some(line) = scene.run_line() {
        let _ = writeln!(
            out,
            "{}{line}{}",
            status_colour(scene.status()),
            theme::ANSI_RESET
        );
    }
    out.push('\n');

    for (index, node) in scene.nodes.iter().enumerate() {
        write_node(&mut out, node, id_width, title_width);
        let next = scene.nodes.get(index + 1).map(|node| &node.id);
        for edge in scene.edges.iter().filter(|edge| edge.from == node.id) {
            let colour = match edge.accent {
                EdgeAccent::Idle => theme::ANSI_MUTED,
                EdgeAccent::Taken => theme::ANSI_GREEN,
                EdgeAccent::Retreat => theme::ANSI_AMBER,
            };
            let guard = edge.guard.as_deref().unwrap_or("");
            let taken = if edge.taken > 0 {
                format!(" (taken {}×)", edge.taken)
            } else {
                String::new()
            };
            let stem = if next == Some(&edge.to) {
                "  │".to_owned()
            } else if edge.back {
                format!("  ╰─◀ {}", edge.to)
            } else {
                format!("  ├─▶ {}", edge.to)
            };
            let mut line = stem;
            if !taken.is_empty() {
                line.push_str(&taken);
            }
            if !guard.is_empty() {
                let _ = write!(line, "  {guard}");
            }
            let _ = writeln!(out, "{colour}{line}{}", theme::ANSI_RESET);
        }
    }

    if !scene.evidence.is_empty() {
        let summary = scene
            .evidence
            .iter()
            .map(|(kind, count)| format!("{kind} ×{count}"))
            .collect::<Vec<_>>()
            .join(" · ");
        let _ = writeln!(
            out,
            "\n{}evidence  {summary}{}",
            theme::ANSI_MUTED,
            theme::ANSI_RESET
        );
    }

    if !scene.reasons.is_empty() {
        let colour = status_colour(scene.status());
        let _ = writeln!(out, "\n{colour}{}:{}", scene.status(), theme::ANSI_RESET);
        // Verbatim, in the engine's own words and order. See `RunView::reasons`.
        for reason in &scene.reasons {
            let _ = writeln!(out, "  {colour}{reason}{}", theme::ANSI_RESET);
        }
    }

    out
}

/// One state as a line of the frame.
fn write_node(out: &mut String, node: &Node, id_width: usize, title_width: usize) {
    let (marker, colour) = match node.accent {
        NodeAccent::Idle => ("·", theme::ANSI_MUTED),
        NodeAccent::Visited => ("✓", theme::ANSI_GREEN),
        NodeAccent::Current => ("▶", theme::ANSI_BLUE),
        NodeAccent::Stopped => ("■", theme::ANSI_RED),
        NodeAccent::Done => ("★", theme::ANSI_GREEN),
    };
    let body = match node.accent {
        NodeAccent::Current | NodeAccent::Stopped | NodeAccent::Done => theme::ANSI_BOLD,
        NodeAccent::Idle | NodeAccent::Visited => theme::ANSI_MUTED,
    };
    let visits = match node.visits {
        Some(count) if count > 1 => format!("×{count}"),
        _ => String::new(),
    };
    let mut tail = node.phases.join(", ");
    if node.terminal {
        if tail.is_empty() {
            tail.push_str("terminal");
        } else {
            tail.push_str(" · terminal");
        }
    }
    let _ = writeln!(
        out,
        "  {colour}{marker}{reset} {body}{id:<id_width$}{reset}  {body}{title:<title_width$}{reset}  \
         {amber}{visits:<4}{reset}{dim}{tail}{reset}",
        id = node.id.as_ref(),
        title = node.title,
        amber = if visits.is_empty() {
            ""
        } else {
            theme::ANSI_AMBER
        },
        dim = theme::ANSI_MUTED,
        reset = theme::ANSI_RESET,
    );
}

/// The colour a status is written in.
fn status_colour(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Completed => theme::ANSI_GREEN,
        RunStatus::Running => theme::ANSI_BLUE,
        RunStatus::Blocked | RunStatus::Broken => theme::ANSI_RED,
        RunStatus::Waiting | RunStatus::Exhausted => theme::ANSI_AMBER,
        RunStatus::Unknown => theme::ANSI_MUTED,
    }
}

/// Every ANSI escape sequence removed, leaving the text the frame says.
///
/// What it is for: a snapshot test that a person can read, and a `--out` file that is not full of
/// control characters. It handles the sequences this module emits — `ESC [ … m` — and passes
/// anything else through, because a renderer that silently ate an unrecognised escape would hide
/// the bug that produced it.
pub fn strip(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' || characters.peek() != Some(&'[') {
            out.push(character);
            continue;
        }
        characters.next();
        for inside in characters.by_ref() {
            if inside.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fixture_workflow, mid_run, snapshot};

    #[test]
    fn the_mid_run_frame_reads_as_the_committed_text() {
        // Escape-stripped: a snapshot full of `\x1b[92m` is a snapshot nobody reviews.
        snapshot(
            "adp-default-mid-run.txt",
            &strip(&frame(&Scene::build(&fixture_workflow(), Some(&mid_run())))),
        );
    }

    #[test]
    fn the_frame_is_the_same_string_twice_and_reads_no_clock() {
        let workflow = fixture_workflow();
        let scene = Scene::build(&workflow, Some(&mid_run()));
        assert_eq!(frame(&scene), frame(&scene));
    }

    #[test]
    fn a_blocked_run_marks_its_state_and_names_every_reason() {
        let workflow = fixture_workflow();
        let run = mid_run();
        let text = strip(&frame(&Scene::build(&workflow, Some(&run))));
        assert!(
            text.contains("■ implement"),
            "the blocked state carries the stopped marker:\n{text}"
        );
        assert!(text.contains("✓ verify"), "verify was visited");
        assert!(text.contains("· review"), "review was not reached");
        assert!(text.contains("×2"), "implement was entered twice");
        assert!(text.contains("blocked:"));
        for reason in &run.reasons {
            assert!(text.contains(reason), "the reason `{reason}` is verbatim");
        }
    }

    #[test]
    fn the_retreat_is_drawn_as_a_back_arrow_in_amber_with_its_count() {
        let workflow = fixture_workflow();
        let coloured = frame(&Scene::build(&workflow, Some(&mid_run())));
        assert!(
            strip(&coloured).contains("╰─◀ implement (taken 1×)"),
            "the retreat is drawn where it leaves `verify`:\n{}",
            strip(&coloured)
        );
        // The glyph comes from the geometry and the colour from the overlay, so the glyph alone
        // would pass on a run that never went back. Amber is the claim being tested.
        assert!(
            coloured.contains(&format!("{}  ╰─◀ implement", theme::ANSI_AMBER)),
            "a taken retreat is amber, not the colour of a forward move"
        );
        let bare = frame(&Scene::build(&workflow, None));
        assert!(
            !bare.contains(&format!("{}  ╰─◀", theme::ANSI_AMBER)),
            "an untaken retreat is not amber: nothing has gone back"
        );
    }

    #[test]
    fn a_bare_workflow_marks_nothing_and_still_shows_every_guard() {
        let workflow = fixture_workflow();
        let text = strip(&frame(&Scene::build(&workflow, None)));
        assert!(!text.contains('▶'), "no run means no current state");
        assert!(!text.contains('■'));
        assert!(text.contains("test.exists"), "the guards are still there");
        assert!(
            !text.contains("blocked"),
            "a workflow without a run says nothing about a run"
        );
    }

    #[test]
    fn stripping_removes_the_colours_and_leaves_everything_else() {
        assert_eq!(strip("\u{1b}[92mgreen\u{1b}[0m"), "green");
        assert_eq!(strip("plain"), "plain");
        assert_eq!(strip("a\u{1b}[1;31mb\u{1b}[0mc"), "abc");
        let workflow = fixture_workflow();
        let coloured = frame(&Scene::build(&workflow, Some(&mid_run())));
        assert!(coloured.contains('\u{1b}'), "the frame is coloured");
        assert!(
            !strip(&coloured).contains('\u{1b}'),
            "and the stripped frame is not"
        );
    }
}
