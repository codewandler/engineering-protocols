//! Draws a workflow, and a run over it.
//!
//! A workflow is a state machine with guarded transitions, and until now this repository could only
//! print one as YAML. This crate turns one into a picture: the states laid out down the page, the
//! guards beside the arrows, and — when a run is handed in — where that run is, where it has been,
//! what it has produced and why it stopped.
//!
//! ```no_run
//! use aep_render::{ansi, html, scene::Scene, svg, RunView};
//! # fn example(workflow: &aep_domain::workflow::Workflow, run: &RunView) {
//! let scene = Scene::build(workflow, Some(run));
//! let figure = svg::render(&scene);   // a standalone SVG document
//! let page = html::render(&scene);    // one self-contained HTML page
//! let frame = ansi::frame(&scene);    // one terminal frame, as a String
//! # let _ = (figure, page, frame);
//! # }
//! ```
//!
//! # One scene, four renderings
//!
//! [`scene::Scene`] is the whole design. Building one resolves the layout, the overlay and every
//! piece of text exactly once; [`svg`], [`html`] and [`ansi`] then answer only *how do I write this
//! out*. PNG is the fourth and it is not here — it is the SVG handed to `rsvg-convert` by the
//! caller, because rasterising means running another program and this crate runs nothing.
//!
//! What that shape buys is **live mode for free**: watching a driver run advance is rebuilding the
//! [`RunView`], rebuilding the scene and emitting again. There is no per-backend state to
//! invalidate, so there is nothing to get out of step.
//!
//! # What this crate does not do
//!
//! * **It does not evaluate anything.** It never decides whether a guard holds, whether evidence is
//!   sufficient or whether a transition is permitted — the engine does that, and a renderer that
//!   answered the same questions would be a second protocol implementation with no conformance
//!   suites behind it. Everything the overlay shows was decided elsewhere and handed in as a
//!   [`RunView`].
//! * **It does not read a clock, a terminal or a file.** `--watch` polls, and that poll lives in
//!   `protocol-cli` where a clock is allowed. `tests/determinism.rs` scans these sources for the
//!   tokens that would break that.
//! * **It does not depend on `aep-engine` or `aep-driver`.** See [`run`] for why the overlay
//!   arrives as a plain struct instead.
//!
//! # Three dependencies that were considered and refused
//!
//! The workspace rule is *prefer no dependency, and record the refusal*. Three were weighed:
//!
//! | candidate | what it would have bought | why not |
//! |---|---|---|
//! | `graphviz` / `dot` | graph layout | A system binary between `protocol workflow render` and a picture, for a graph of nine nodes. [`layout`] is sixty lines instead. |
//! | `ratatui` | a terminal UI | A backend, an event loop and a widget tree — plus `crossterm` — for one screen that is a list. [`ansi`] is that list. |
//! | `resvg` / `usvg` | PNG without a system tool | A rasteriser, a font stack and a colour pipeline compiled into the CLI for a format nothing in the gate reads. The CLI shells out to `rsvg-convert` and says so by name when it is absent. |
//!
//! # Determinism
//!
//! Invariant 9 applies here as an acceptance criterion rather than as a general principle: the same
//! workflow and the same [`RunView`] produce **byte-identical** output, so a figure that is
//! generated, committed and regenerated does not turn up in a diff. Every collection is ordered,
//! every coordinate is an `i32`, and nothing reads ambient state.

pub mod ansi;
pub mod html;
pub mod layout;
pub mod run;
pub mod scene;
pub mod svg;
pub mod theme;

#[cfg(test)]
mod testing;

pub use layout::Layout;
pub use run::{RunStatus, RunView};
pub use scene::{Edge, EdgeAccent, Node, NodeAccent, Scene};
