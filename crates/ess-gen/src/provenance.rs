//! Where a generated artifact came from.
//!
//! Four facts, because each of them moves independently and each one alone is insufficient: the same
//! specification version compiled by two builds can differ, and the same compiler run through two
//! generator versions certainly does. Design §10.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ess_compiler::graph::SemanticDependencyGraph;
use ess_compiler::refs::EssSemanticRef;
use ess_compiler::EssIr;

/// What produced an artifact.
///
/// `Deserialize` as well as `Serialize`, and the two version fields are owned rather than
/// `&'static str`, because provenance is read back and not only written. A conformance suite is
/// committed and re-read by a later run, and a document that has been on disk cannot hold a
/// `&'static str` that came from someone else's `env!`. Costing two allocations per artifact to let
/// the same type serve both directions is a better trade than a second provenance type that drifts
/// from this one — which is what happened to the schema mapping this crate publishes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    /// The system the artifact describes.
    pub system: String,
    /// The version of that system's specification.
    pub specification_version: String,
    /// A digest of the resolved model.
    ///
    /// Of the IR rather than of the source files, deliberately: two source trees that differ only in
    /// comments and file layout mean the same system, and a digest that changes when a comment does
    /// makes every reader ignore it.
    pub source_digest: String,
    /// A digest of the model slice this artifact derives from — its contract with the model.
    ///
    /// `source_digest` answers "which resolution of the specification produced this";
    /// `contract_digest` answers the narrower question wave 7 asks: "did the part of the model this
    /// artifact is derived from move". For a system-level artifact the slice is legitimately the
    /// whole model and the two digests move together; for a per-construct artifact the slice is the
    /// construct plus everything it rests on ([`SemanticDependencyGraph::slice`]), so an unrelated
    /// change moves `source_digest` and leaves this one standing. The membership rule is
    /// conservative — sub-constructs travel with their parents, and the system header, naming,
    /// conversions and workloads are in every slice — because a too-big slice costs a regeneration
    /// and a too-small one costs a false "still current", and those are not comparable errors.
    pub contract_digest: String,
    /// The build that resolved it.
    pub compiler_version: String,
    /// The build that wrote the artifact.
    pub generator_version: String,
}

impl Provenance {
    /// This crate's version, as both the compiler and generator version.
    ///
    /// One number while the two ship together. When they stop shipping together this becomes two
    /// numbers, and the field names already say which is which.
    pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Derives provenance from the model an artifact is generated from.
    ///
    /// The contract digest is the **whole model's** — the honest default, and the fail-closed one:
    /// an artifact stamped this way is owed regeneration whenever anything moves. An artifact that
    /// derives from less than the whole model narrows through [`ProvenanceMint::of_seeds`].
    pub fn of(ir: &EssIr) -> Self {
        Self {
            system: ir.system.to_string(),
            specification_version: ir.version.to_string(),
            source_digest: digest(ir),
            contract_digest: slice_digest(ir, None),
            compiler_version: Self::VERSION.to_owned(),
            generator_version: Self::VERSION.to_owned(),
        }
    }

    /// The command that regenerates every projection this crate publishes.
    ///
    /// The default for [`Provenance::lines`] and its comment renderings. A stamped artifact from a
    /// *different* generator names its own command through the `*_for` forms below — `ess-synth`
    /// writes `protocol ess synthesize` — because a header that tells the reader to run the wrong
    /// verb is worse than no header at all.
    pub const REGENERATE: &'static str = "protocol ess generate";

    /// The provenance as comment lines, each already prefixed.
    ///
    /// Takes the prefix because every projection has a different one — `#` for YAML, `//` for Rust —
    /// and a generator that hand-assembles these lines gets one of them wrong.
    ///
    /// **Not for HTML or Markdown.** A per-line prefix cannot *close* an HTML comment, so
    /// `commented("<!--")` produces four unterminated openers and a renderer swallows the page.
    /// Use [`Provenance::html_comment`], which is the whole block. This method's own documentation
    /// used to recommend the broken form, which is how the docs generator found it.
    pub fn commented(&self, prefix: &str) -> String {
        self.commented_for(prefix, Self::REGENERATE)
    }

    /// [`Provenance::commented`], naming the command that actually regenerates the artifact.
    pub fn commented_for(&self, prefix: &str, regenerate: &str) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        for line in self.lines_for(regenerate) {
            let _ = writeln!(out, "{prefix} {line}");
        }
        out
    }

    /// The provenance as one HTML comment block, for Markdown and HTML.
    ///
    /// One `<!--` and one `-->` around the whole thing, because a comment that is opened per line
    /// and never closed makes the rest of the document disappear — silently, in a renderer, which is
    /// the worst place to find out.
    pub fn html_comment(&self) -> String {
        self.html_comment_for(Self::REGENERATE)
    }

    /// [`Provenance::html_comment`], naming the command that actually regenerates the artifact.
    pub fn html_comment_for(&self, regenerate: &str) -> String {
        use std::fmt::Write as _;

        let mut out = String::from("<!--\n");
        for line in self.lines_for(regenerate) {
            let _ = writeln!(out, "  {line}");
        }
        out.push_str("-->\n");
        out
    }

    /// The provenance as plain lines.
    pub fn lines(&self) -> Vec<String> {
        self.lines_for(Self::REGENERATE)
    }

    /// [`Provenance::lines`], naming the command that actually regenerates the artifact.
    pub fn lines_for(&self, regenerate: &str) -> Vec<String> {
        vec![
            format!(
                "generated from {} {}",
                self.system, self.specification_version
            ),
            format!("model digest {}", self.source_digest),
            format!("contract digest {}", self.contract_digest),
            format!(
                "compiler {} · generator {}",
                self.compiler_version, self.generator_version
            ),
            format!("do not edit: regenerate with `{regenerate}`"),
        ]
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({})",
            self.system, self.specification_version, self.source_digest
        )
    }
}

/// A digest of the resolved model.
///
/// Over the IR's canonical JSON, so it is stable under anything that does not change what the system
/// means. The full SHA-256, all 64 hex characters, and the width is load-bearing: this began as a
/// 16-character truncation "for telling two models apart in a comment header, not for resisting an
/// adversary" — and then gate G19 made completion decisions rest on it and wave 5 made suite
/// acceptance rest on it (`ess impact` refuses a suite whose digest mismatches). A 64-bit digest is
/// fine against drift and weak against construction, so once the digest became an acceptance
/// criterion the truncation had to go (gap register D-4).
fn digest(ir: &EssIr) -> String {
    use sha2::{Digest, Sha256};

    use std::fmt::Write as _;

    let json = serde_json::to_vec(ir).unwrap_or_default();
    let hash = Sha256::digest(&json);
    let mut out = String::with_capacity(64);
    for byte in &hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The model slice one artifact derives from, as the artifact records it.
///
/// Two cases and no third, and the default everywhere is [`WholeModel`](Self::WholeModel) — the
/// fail-closed direction, because a whole-model slice makes the artifact owed whenever anything
/// moves. Narrowing to a seed set is something a generator does explicitly, per artifact, by
/// naming the constructs the artifact is about.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "of", rename_all = "kebab-case")]
pub enum ModelSlice {
    /// The artifact derives from the whole model.
    WholeModel,
    /// The artifact derives from these constructs, closed over everything they rest on.
    ///
    /// The seeds, not the closure: the closure is a function of the model, and recording it would
    /// commit a copy that drifts the moment the model moves. Whoever asks "did this slice move"
    /// closes the seeds against the model in hand — [`SemanticDependencyGraph::slice`], the same
    /// walk `ess impact` narrows by.
    Constructs {
        /// The constructs the artifact is about.
        seeds: BTreeSet<EssSemanticRef>,
    },
}

/// A provenance stamped for one artifact, still attached to the slice it was stamped for.
///
/// The pairing is the point: a generator that computed a digest for one slice and recorded another
/// beside it would be making a false claim about derivation, so the two travel as one value and
/// [`crate::artifact::run`] checks the stamped digest against the recorded slice for every artifact
/// it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicedProvenance {
    /// The provenance to stamp into the artifact.
    pub provenance: Provenance,
    /// The slice the contract digest is of.
    pub slice: ModelSlice,
}

/// Mints per-artifact provenance for one model: whole, or sliced to what an artifact derives from.
///
/// Built once per generator run — it carries the dependency graph, and the graph is a function of
/// the model — and handed to every [`Generator`](crate::artifact::Generator), which is what makes
/// "each artifact carries the digest of its own slice" one mechanism rather than four generators'
/// four conventions.
pub struct ProvenanceMint<'a> {
    /// The model everything is minted from.
    ir: &'a EssIr,
    /// The dependency graph of that model — the same walk `ess impact` runs.
    graph: SemanticDependencyGraph,
    /// The whole-model provenance, computed once.
    whole: Provenance,
}

impl<'a> ProvenanceMint<'a> {
    /// Prepares to mint provenance for artifacts of this model.
    #[must_use]
    pub fn new(ir: &'a EssIr) -> Self {
        Self {
            ir,
            graph: SemanticDependencyGraph::of(ir),
            whole: Provenance::of(ir),
        }
    }

    /// Provenance for an artifact that derives from the whole model.
    #[must_use]
    pub fn whole(&self) -> SlicedProvenance {
        SlicedProvenance {
            provenance: self.whole.clone(),
            slice: ModelSlice::WholeModel,
        }
    }

    /// Provenance for an artifact that derives from these constructs and what they rest on.
    #[must_use]
    pub fn of_seeds(&self, seeds: impl IntoIterator<Item = EssSemanticRef>) -> SlicedProvenance {
        let slice = ModelSlice::Constructs {
            seeds: seeds.into_iter().collect(),
        };
        let provenance = Provenance {
            contract_digest: self.digest_of(&slice),
            ..self.whole.clone()
        };
        SlicedProvenance { provenance, slice }
    }

    /// The contract digest one slice computes against this model.
    ///
    /// Public within the crate's surface because [`crate::artifact::run`] re-derives every
    /// artifact's digest from its recorded slice and refuses a stamp that disagrees.
    #[must_use]
    pub fn digest_of(&self, slice: &ModelSlice) -> String {
        match slice {
            ModelSlice::WholeModel => slice_digest(self.ir, None),
            ModelSlice::Constructs { seeds } => {
                let members = self.graph.slice(seeds);
                slice_digest(self.ir, Some(&members))
            }
        }
    }
}

/// The digest of one model slice: the named constructs' resolved content, canonically serialized,
/// hashed whole.
///
/// `members: None` means the whole model — every construct of every family. Either way the
/// serialization carries the system header (name, version, naming, summary) and the two construct
/// families that have no [`EssSemanticRef`] to be sliced by, conversions and workloads, in **every**
/// slice: a change there cannot be attributed to a construct, so the only digest that does not lie
/// about it is one that moves for every artifact. That is the same fail-closed answer `ess impact`
/// gives for the families its delta does not compare.
///
/// A sub-construct reference — an outcome, a transition — contributes no entry of its own: its
/// content is part of its parent's, and [`SemanticDependencyGraph::slice`] guarantees the parent is
/// in any slice that holds the child.
fn slice_digest(
    ir: &EssIr,
    members: Option<&BTreeMap<EssSemanticRef, Vec<ess_compiler::graph::DependencyEdge>>>,
) -> String {
    use std::fmt::Write as _;

    use sha2::{Digest, Sha256};

    let constructs: BTreeMap<String, serde_json::Value> = match members {
        Some(members) => members
            .keys()
            .filter_map(|member| {
                construct_content(ir, member).map(|body| (member.to_string(), body))
            })
            .collect(),
        None => ir
            .domains
            .keys()
            .map(|name| EssSemanticRef::from(ess_compiler::refs::DomainRef::new(name.clone())))
            .chain(
                ir.types
                    .keys()
                    .map(|name| ess_compiler::refs::DeclaredTypeRef::new(name.clone()).into()),
            )
            .chain(
                ir.entities
                    .keys()
                    .map(|name| ess_compiler::refs::EntityRef::new(name.clone()).into()),
            )
            .chain(
                ir.commands
                    .keys()
                    .map(|name| ess_compiler::refs::CommandRef::new(name.clone()).into()),
            )
            .chain(
                ir.events
                    .keys()
                    .map(|name| ess_compiler::refs::EventRef::new(name.clone()).into()),
            )
            .chain(
                ir.errors
                    .keys()
                    .map(|name| ess_compiler::refs::ErrorRef::new(name.clone()).into()),
            )
            .chain(
                ir.views
                    .keys()
                    .map(|name| ess_compiler::refs::ViewRef::new(name.clone()).into()),
            )
            .chain(
                ir.actors
                    .keys()
                    .map(|name| ess_compiler::refs::ActorRef::new(name.clone()).into()),
            )
            .chain(
                ir.bindings
                    .keys()
                    .map(|name| ess_compiler::refs::BindingRef::new(name.clone()).into()),
            )
            .chain(
                ir.components
                    .keys()
                    .map(|name| ess_compiler::refs::ComponentRef::new(name.clone()).into()),
            )
            .filter_map(|member: EssSemanticRef| {
                construct_content(ir, &member).map(|body| (member.to_string(), body))
            })
            .collect(),
    };

    let document = serde_json::json!({
        "constructs": constructs,
        "conversions": ir.conversions,

        "naming": ir.naming,
        "summary": ir.summary,
        "system": ir.system,
        "version": ir.version,
        "workloads": ir.workloads,
    });

    let json = serde_json::to_vec(&document).unwrap_or_else(|error| {
        panic!("a slice of the IR serialises, as the whole IR already does: {error}")
    });
    let hash = Sha256::digest(&json);
    let mut out = String::with_capacity(64);
    for byte in &hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The resolved content one reference names, or `None` for a sub-construct whose content is its
/// parent's.
///
/// A reference the model has no construct for serialises as `null` rather than being dropped: it
/// cannot happen when slices are minted from the model itself, and if it ever does, a digest that
/// visibly holds a hole is better than one that silently shrank.
fn construct_content(ir: &EssIr, member: &EssSemanticRef) -> Option<serde_json::Value> {
    fn body<T: serde::Serialize>(found: Option<&T>) -> serde_json::Value {
        match found {
            Some(value) => serde_json::to_value(value)
                .unwrap_or_else(|error| panic!("the IR serialises: {error}")),
            None => serde_json::Value::Null,
        }
    }
    match member {
        EssSemanticRef::Domain { name } => Some(body(ir.domains.get(name.name()))),
        EssSemanticRef::Type { name } => Some(body(ir.types.get(name.name()))),
        EssSemanticRef::Entity { name } => Some(body(ir.entities.get(name.name()))),
        EssSemanticRef::Command { name } => Some(body(ir.commands.get(name.name()))),
        EssSemanticRef::Event { name } => Some(body(ir.events.get(name.name()))),
        EssSemanticRef::Error { name } => Some(body(ir.errors.get(name.name()))),
        EssSemanticRef::View { name } => Some(body(ir.views.get(name.name()))),
        EssSemanticRef::Actor { name } => Some(body(ir.actors.get(name.name()))),
        EssSemanticRef::Binding { name } => Some(body(ir.bindings.get(name.name()))),
        EssSemanticRef::Component { name } => Some(body(ir.components.get(name.name()))),
        EssSemanticRef::Outcome { .. } | EssSemanticRef::Transition { .. } => None,
    }
}

/// The two digests read back off a committed artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDigests {
    /// The model the artifact claims it was generated from.
    pub source_digest: String,
    /// The slice the artifact claims it derives from.
    pub contract_digest: String,
}

impl Provenance {
    /// Reads the two digests back off an artifact's text, whatever form the stamp took.
    ///
    /// The inverse of this type's own emissions and of nothing else: the comment-line forms
    /// (`model digest …` / `contract digest …`, behind `#`, `//` or inside an HTML comment) and the
    /// serialized-field forms (`"source_digest": "…"`, `"spec_digest": "…"`,
    /// `"contract_digest": "…"`). One reader beside the writer, so a committed artifact's claim of
    /// derivation is checked against exactly what was stamped — a second parser somewhere else is
    /// how the check and the stamp drift apart.
    ///
    /// `None` when either digest is missing or is not 64 lower-case hex characters. That is the
    /// fail-closed answer on purpose: an artifact whose provenance cannot be read is an artifact
    /// whose claims cannot be checked, and every caller treats it as owed.
    #[must_use]
    pub fn read_digests(text: &str) -> Option<ArtifactDigests> {
        let source = digest_after(
            text,
            &[
                "model digest ",
                "\"source_digest\": \"",
                "\"spec_digest\": \"",
            ],
        )?;
        let contract = digest_after(text, &["contract digest ", "\"contract_digest\": \""])?;
        Some(ArtifactDigests {
            source_digest: source,
            contract_digest: contract,
        })
    }
}

/// The first well-formed digest following any of these markers, or `None`.
fn digest_after(text: &str, markers: &[&str]) -> Option<String> {
    for marker in markers {
        let Some(at) = text.find(marker) else {
            continue;
        };
        let candidate: String = text[at + marker.len()..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            .collect();
        if candidate.len() == 64 {
            return Some(candidate);
        }
    }
    None
}
