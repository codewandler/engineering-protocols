//! Deterministic projections of a specification.
//!
//! Everything here reads an [`EssIr`](ess_compiler::EssIr) and writes text. Nothing reads a
//! `Specification`, and that is the point: a generator holding names rather than resolved handles has
//! to either re-check every reference or trust that something else did, and the second one is how a
//! generator emits an `OpenAPI` schema for a type that does not exist.
//!
//! # Every projection is one trait
//!
//! Review F9 counted eleven crates in the original design and asked for three. So there is one
//! [`Generator`], and each projection is an implementation of it rather than a crate of its own. The
//! shape is the same every time — IR in, named artifacts out — because the interesting differences
//! between `OpenAPI` and Markdown are in the body, not in how they are invoked.
//!
//! # An artifact says where it came from
//!
//! Design §10 asks for provenance on generated output: specification version, source digest,
//! compiler version, generator version. [`Provenance`] carries all four, and every generator emits it,
//! because an artifact that cannot say which specification produced it is an artifact nobody can
//! audit — and the moment there are two checkouts, that is the only question anyone asks about it.
//!
//! # Determinism
//!
//! Same IR in, byte-identical bytes out. No clock, no RNG, `BTreeMap`/`BTreeSet` only, and a test per
//! generator that generates twice and compares. Review F8's point was that the test is what makes the
//! property true; a comment claiming determinism is a comment.

pub mod artifact;
pub mod asyncapi;
pub mod docs;
pub mod openapi;
pub mod provenance;
pub mod schema;

pub use artifact::{Artifact, Generator};
pub use provenance::Provenance;

/// Every projection this build publishes, in the order they were built.
///
/// Documentation first, deliberately: it is the cheapest check on model completeness, because a
/// construct with no rendering shows up as a hole in a page a person reads rather than as a subtly
/// wrong schema nobody validates. The contracts come after, once the model has survived being
/// described.
pub fn generators() -> Vec<Box<dyn Generator>> {
    vec![
        Box::new(docs::Docs),
        Box::new(schema::JsonSchema),
        Box::new(openapi::OpenApi),
        Box::new(asyncapi::AsyncApi),
    ]
}

/// The projection with this name, if there is one.
pub fn generator(name: &str) -> Option<Box<dyn Generator>> {
    generators()
        .into_iter()
        .find(|generator| generator.name() == name)
}

/// Runs every projection and returns every artifact, keyed by path.
///
/// Keyed across *all* generators, not per generator: two projections claiming one path means the
/// second silently overwrites the first, and the output tree looks complete while missing a file.
pub fn generate_all(
    ir: &ess_compiler::EssIr,
) -> Result<std::collections::BTreeMap<String, Artifact>, artifact::DuplicatePath> {
    let mut out = std::collections::BTreeMap::new();
    for generator in generators() {
        for (path, artifact) in artifact::run(generator.as_ref(), ir)? {
            if out.contains_key(&path) {
                return Err(artifact::DuplicatePath {
                    generator: generator.name(),
                    path,
                });
            }
            out.insert(path, artifact);
        }
    }
    Ok(out)
}
