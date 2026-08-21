//! Where the browser realization lands, and how it reaches the Rust target's crates.
//!
//! One decision taken once, for the same reason [`crate::rust::layout`] exists: the manifest
//! writes a path, the module writes a `use`, the page writes a filename and a test greps for one.
//! A convention re-derived per renderer is a convention three renderers spell three ways.

use ess_compiler::ir::EssIr;

use crate::rust::layout::Layout as RustLayout;

/// The shape of the emitted browser realization: one crate, one page, one glue file.
///
/// One crate rather than one per component, deliberately, and the opposite call from the Rust
/// target's: a component is the specification's unit of ownership, but a *browser tab* is not —
/// a page that loaded one WebAssembly module per component would hold several disconnected
/// systems, and the transport between them is exactly what the system crate is.
pub(crate) struct Layout {
    /// The Rust target's layout, because this target does not re-derive its names: the bridge
    /// imports the crates that target emitted, and a second answer to "what is the types crate
    /// called" is a second answer that drifts.
    rust: RustLayout,
    /// The package name of the emitted bridge crate — `billing-web`.
    package: String,
}

impl Layout {
    /// Derives the layout of a resolved specification.
    pub fn of(ir: &EssIr) -> Self {
        let rust = RustLayout::of(ir);
        let mut package = format!("{}-web", ir.system.segments().join("-"));
        // Repaired the way the Rust target repairs a component package, and for the same reason:
        // a system whose own name makes `{system}-web` collide with a crate that already exists
        // must still emit, and a deterministic rename that can itself collide is a collision with
        // extra steps.
        while package == rust.package()
            || package == rust.system_package()
            || ir
                .components
                .keys()
                .any(|component| rust.component_package(component) == package)
        {
            package.push_str("-bridge");
        }
        Self { rust, package }
    }

    /// The Rust target's layout, whose names this one imports rather than invents.
    pub fn rust(&self) -> &RustLayout {
        &self.rust
    }

    /// The package name of the emitted bridge crate.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// One source file of the bridge crate, by module name.
    pub fn source(&self, module: &str) -> String {
        format!("crates/{}/src/{module}.rs", self.package)
    }

    /// The bridge crate's manifest.
    pub fn manifest(&self) -> String {
        format!("crates/{}/Cargo.toml", self.package)
    }

    /// How a crate of the Rust target's tree is reached from the bridge crate's manifest.
    ///
    /// The one place this target is *not* standalone, and it is a stated weakening rather than an
    /// accident: the bridge is a front end over the crates the Rust target emitted, so its
    /// manifest names them by path. `crates/<bridge>/` is four levels under `generated/`, and the
    /// Rust tree sits at `generated/rust/<system>/` — the committed layout `cargo xtask synth`
    /// writes, which is therefore part of this target's contract.
    pub fn rust_crate_path(package: &str, system: &str) -> String {
        format!("../../../../rust/{system}/crates/{package}")
    }
}
