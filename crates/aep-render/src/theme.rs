//! The house palette, in one place.
//!
//! The colours are not invented here: they are lifted from
//! `website/static/img/trace-evidence-gate.svg`, the figure this repository already publishes, so a
//! rendered workflow sits beside it without a second design. Naming them once is what keeps the
//! SVG, the HTML page and the terminal frame agreeing about what *blocked* looks like — three
//! emitters reading three literals is how a palette drifts.
//!
//! # Why the terminal colours are not the same values
//!
//! A terminal has no `#d29922`. The ANSI frame therefore carries its own constants, mapped to the
//! nearest of the sixteen colours a terminal is allowed to have, and they are declared here beside
//! the hex so the mapping is visible rather than scattered through the emitter.

/// The page behind everything.
pub const BACKGROUND: &str = "#0f1418";

/// The fill of a panel — a state box, a card, a table row.
pub const PANEL: &str = "#161d24";

/// Every hairline: a panel's border, an untaken edge, a rule under a heading.
pub const LINE: &str = "#2b3540";

/// Body text.
pub const TEXT: &str = "#d7dde3";

/// Secondary text: a phase list, a guard, a caption.
pub const MUTED: &str = "#8b98a5";

/// Something holds, or a run finished.
pub const GREEN: &str = "#3fb950";

/// Where the run is now.
pub const BLUE: &str = "#58a6ff";

/// A retreat: a taken back-edge, a budget that ran out, a person who owes an answer.
pub const AMBER: &str = "#d29922";

/// The protocol said no.
pub const RED: &str = "#f85149";

/// The type stack, matching the published figure exactly.
pub const MONO: &str = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

/// Back to the terminal's own colours.
pub const ANSI_RESET: &str = "\u{1b}[0m";

/// Dim: the terminal's stand-in for [`MUTED`].
pub const ANSI_MUTED: &str = "\u{1b}[2m";

/// Bold: what a title gets, since a frame has no second font.
pub const ANSI_BOLD: &str = "\u{1b}[1m";

/// Bright green, for [`GREEN`].
pub const ANSI_GREEN: &str = "\u{1b}[92m";

/// Bright blue, for [`BLUE`].
pub const ANSI_BLUE: &str = "\u{1b}[94m";

/// Bright yellow, for [`AMBER`].
pub const ANSI_AMBER: &str = "\u{1b}[93m";

/// Bright red, for [`RED`].
pub const ANSI_RED: &str = "\u{1b}[91m";
