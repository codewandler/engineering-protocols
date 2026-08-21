//! Turning a simulation's gaps into changes: what closes mechanically, what a human must decide,
//! and what this build will not represent at all.
//!
//! # Three dispositions, no fourth
//!
//! The synthesis discipline of `ess-synth`, applied to a cluster. There it is *generated*,
//! *obligation*, *refused* over the capabilities a specification determines; here it is the same
//! three words over the gaps a snapshot reports:
//!
//! | disposition | means | example |
//! |---|---|---|
//! | [`Generated`](Disposition::Generated) | a change closes it, derived from the gap and the observed state alone | `spec.replicas: 1 → 2` |
//! | [`Obligation`](Disposition::Obligation) | the shape of the answer is known; the value is a decision | which image replaces `:latest` |
//! | [`Refused`](Disposition::Refused) | this build will not represent it, rather than approximate it | a predicate states a condition, not a field |
//!
//! The line between the first two is **whether anything but the gap decides the value**. A
//! replica count outside `[2, 4]` has one nearest acceptable number and the range says which. An
//! image tagged `latest` has no mechanically-nearest replacement — picking one is an engineering
//! decision, and a generator that picked `1.4.2` would have taken it on somebody's behalf and
//! hidden it inside a patch file. So resources and probes sit on *both* sides: with a
//! [`Remedy`] in the specification the value came from a human and the patch
//! is written; without one it is an obligation naming the decision.
//!
//! # The projection closes what it opens
//!
//! Raising a workload from one replica to two makes it a multi-replica workload, and a
//! specification that also expects a disruption budget now gaps where it held. A projection that
//! emitted the replica patch and stopped would be handing somebody a tree that trades one gap for
//! another.
//!
//! So generation is a **fixed point**, not a pass. Each round simulates the specification against
//! the *working* model — the observed one with every change so far applied — and disposes the
//! gaps that round reports. A gap the changes opened is marked
//! [`Induced`](GapOrigin::Induced), so a reader can tell "your cluster is wrong about this" from
//! "this tree would make it wrong about this, and here is what also closes it".
//!
//! It terminates because a gap is disposed at most once, keyed by (expectation, subject), and
//! that set is finite: every round either disposes at least one new pair or ends the loop.
//!
//! # And it checks itself
//!
//! Two expectations can disagree — `[2, 4]` and `[5, 6]` over one workload cannot both be
//! satisfied — and a projection that wrote both patches would emit a file that closes one gap and
//! leaves the other open while claiming otherwise. After the fixed point, every
//! [`Generated`](Disposition::Generated) entry is checked against the final simulation, and one
//! whose gap is still open is re-read as [`Refused`](Disposition::Refused) with
//! [`Contradicted`](RefusalReason::Contradicted) — because picking a winner between two declared
//! expectations is not this crate's decision either.

use std::collections::{BTreeMap, BTreeSet};

use infra_compiler::ir::{InfraIr, InfraModel};
use infra_domain::observation::Identity;
use infra_domain::policy::PodDisruptionBudget;
use infra_domain::workload::{Probe, ProbeHandler, Probes, Resources};
use infra_spec::{
    simulate, Expectation, Gap, InfraSpec, Outcome, Port, ProbeHandlerRemedy, ProbeRemedy, Remedy,
    Simulation, Summary,
};
use serde::Serialize;
use serde_json::Value;

use crate::patch::{canonical_json, NewObject, ObjectPatch, ObjectRef, PatchDraft, PatchType};
use crate::render::{obligations_markdown, summary_markdown};

/// The format string a persisted projection document carries.
pub const PROJECTION_FORMAT: &str = "infra-projection/1";

/// The uid a generated object carries **inside the working model only**.
///
/// A bundle is an observation and every object in one has a uid the API server assigned, so the
/// hypothetical model needs something in that field to be a model at all. Nothing ever emits it:
/// a manifest is built from the change, not from the model, and a manifest carrying a uid would
/// be claiming an object identity that does not exist yet.
const PROJECTED_UID: &str = "projected-not-observed";

/// Where a projection's tree writes one object's patch.
fn patch_path(target: &ObjectRef, patch_type: PatchType) -> String {
    format!("patches/{}.{}.json", target.slug(), patch_type.as_str())
}

/// Where a projection's tree writes one generated object.
fn object_path(target: &ObjectRef) -> String {
    format!("objects/{}.json", target.slug())
}

/// Whether the snapshot reported a gap, or this projection's own changes opened one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapOrigin {
    /// The gap is in the simulation of the observed snapshot.
    Observed,
    /// The gap appears only after this projection's changes are applied.
    Induced,
}

impl GapOrigin {
    /// The origin as it reads in a table cell.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Induced => "induced",
        }
    }
}

/// What a generated change does, in one line a reviewer can read beside the patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeneratedChange {
    /// The artifact that carries it, relative to the projection root.
    pub artifact: String,
    /// The change itself: `spec.replicas: 1 -> 2`.
    pub change: String,
}

/// Why closing a gap is a decision rather than an edit.
///
/// Closed and typed, for the reason `INFRA-DIAG-*` findings carry evidence: an obligation a reader
/// cannot act on is an obligation nobody acts on. Each variant names the *shape* of the answer;
/// [`ProjectionObligation::decision`] names the answer in the cluster's own spellings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum ObligationReason {
    /// Choosing an image reference is an engineering decision: which build, which registry,
    /// which digest.
    ImageChoice,
    /// The object the expectation names was never deployed, and the expectation states only its
    /// name — not what it would run.
    ObjectUndefined,
    /// The reference names something nobody observed. Inventing the target, or rewriting the
    /// reference to point somewhere else, both guess at which of the two was meant.
    TargetUnknown,
    /// The expectation asks for a field and states no value to put in it.
    ValueUnstated {
        /// Which values nobody stated, in the specification's own spellings.
        fields: Vec<String>,
    },
    /// The field cannot be changed on a live object; closing the gap means replacing it.
    FieldImmutable {
        /// The field, in the manifest's spelling.
        field: String,
    },
    /// The object this projection would create already exists under that name, holding something
    /// else.
    NameTaken {
        /// The object in the way.
        object: String,
    },
}

impl ObligationReason {
    /// The reason as it reads in a sentence.
    pub fn describes(&self) -> String {
        match self {
            Self::ImageChoice => {
                "choosing an image is an engineering decision, not a substitution".to_owned()
            }
            Self::ObjectUndefined => {
                "the expectation names the object and not what it would run".to_owned()
            }
            Self::TargetUnknown => "nobody observed what this points at".to_owned(),
            Self::ValueUnstated { fields } => {
                format!("no value is stated for {}", fields.join(" and "))
            }
            Self::FieldImmutable { field } => {
                format!("`{field}` cannot be patched on a live object")
            }
            Self::NameTaken { object } => format!("`{object}` already exists"),
        }
    }
}

/// A gap somebody has to close by deciding something, and what the decision is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionObligation {
    /// Why it cannot be generated.
    pub reason: ObligationReason,
    /// What satisfying it looks like, naming the cluster's own objects and fields.
    pub decision: String,
}

/// Why this build will not represent a gap as a change at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum RefusalReason {
    /// A predicate states a *condition*, not a field to change. `workload.ready_pods >= 1` is
    /// false for a dozen different reasons and names none of them, so there is no edit it
    /// determines — and an edit chosen from the condition's left-hand side would be this crate
    /// guessing which fact the author meant to move.
    NotAField,
    /// Another expectation's change puts the subject outside this one. Two declared expectations
    /// disagree, and a projection that picked a winner would be deciding which of them the author
    /// meant.
    Contradicted {
        /// The expectation whose change contradicts this one.
        by: String,
    },
}

impl RefusalReason {
    /// The reason as it reads in a sentence.
    pub fn describes(&self) -> String {
        match self {
            Self::NotAField => "a predicate states a condition, not a field to change".to_owned(),
            Self::Contradicted { by } => {
                format!("`{by}` moves this subject outside this expectation")
            }
        }
    }
}

/// A gap this projection declines to represent, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionRefusal {
    /// The reason class.
    pub reason: RefusalReason,
    /// The refusal in full, naming the subject's own facts.
    pub detail: String,
}

/// What this projection decided about one gap. Three cases, no fourth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum Disposition {
    /// A change this projection wrote closes it.
    Generated(GeneratedChange),
    /// Closing it takes a decision this projection must not take.
    Obligation(ProjectionObligation),
    /// This projection does not represent it, and says so rather than approximating.
    Refused(ProjectionRefusal),
}

impl Disposition {
    /// The wire discriminant, for a summary that counts rather than reads.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generated(_) => "generated",
            Self::Obligation(_) => "obligation",
            Self::Refused(_) => "refused",
        }
    }
}

/// One gap, and what this projection decided about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionEntry {
    /// The expectation the gap is against.
    pub expectation: String,
    /// The expectation kind's wire name.
    pub kind: String,
    /// The subject's IR key.
    pub subject: String,
    /// Whether the snapshot reported the gap or this projection's changes opened it.
    pub origin: GapOrigin,
    /// The gap itself, exactly as the simulation reported it.
    pub gap: Gap,
    /// One sentence saying what the subject has and what the expectation wanted — the same
    /// sentence `protocol infra simulate` prints, from the same function.
    pub reads: String,
    /// What this projection decided.
    #[serde(flatten)]
    pub disposition: Disposition,
}

/// The inputs a projection is a function of.
///
/// Both digests, deliberately. A tree derived from a specification and a snapshot is only
/// reviewable if a reader can tell *which* specification and *which* snapshot — and a name is not
/// an identity: two revisions of `expected.yaml` are the same name and different documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionProvenance {
    /// The kubeconfig context the scan targeted.
    pub context: String,
    /// The snapshot's content digest.
    pub snapshot_digest: String,
    /// The specification's content digest.
    pub specification_digest: String,
}

/// What a projection did, in counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProjectionSummary {
    /// Subject gaps in the simulation of the observed snapshot.
    pub gaps_observed: usize,
    /// Subject gaps this projection's own changes opened.
    pub gaps_induced: usize,
    /// Entries a generated change closes.
    pub generated: usize,
    /// Entries that need a decision.
    pub obligations: usize,
    /// Entries this build does not represent.
    pub refusals: usize,
    /// Patch files written.
    pub patches: usize,
    /// New-object manifests written.
    pub objects: usize,
    /// The expectation-level verdicts before any change.
    pub verdicts_before: Summary,
    /// The expectation-level verdicts after every change in this tree is applied.
    pub verdicts_after: Summary,
}

/// The whole projection: what would have to change, as files somebody can review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Projection {
    /// The format claim, `infra-projection/1`.
    pub format: &'static str,
    /// The specification's name, as it declares itself.
    pub specification: String,
    /// What it was computed from.
    pub provenance: ProjectionProvenance,
    /// One entry per gap, in the specification's own order, subjects sorted within an
    /// expectation.
    pub entries: Vec<ProjectionEntry>,
    /// The patches, in path order.
    pub patches: Vec<ObjectPatch>,
    /// The generated objects, in path order.
    pub objects: Vec<NewObject>,
    /// The counts.
    pub summary: ProjectionSummary,
}

/// The persisted document: the projection, plus the tree it writes.
///
/// The artifacts are *in* the document for the reason `protocol ess generate --format json`
/// carries its own: `cargo xtask` commits the tree by reading what the command printed, so the
/// bytes it commits are the bytes the command produces. A second path from the library to a file
/// would be a second answer to "what does this projection write".
#[derive(Debug, Clone, Serialize)]
pub struct ProjectionDocument<'a> {
    /// The format claim.
    pub format: &'static str,
    /// The specification's name.
    pub specification: &'a str,
    /// What it was computed from.
    pub provenance: &'a ProjectionProvenance,
    /// One entry per gap.
    pub entries: &'a [ProjectionEntry],
    /// The counts.
    pub summary: &'a ProjectionSummary,
    /// Every file the tree holds, in path order.
    pub artifacts: Vec<Artifact>,
}

/// One file of a projection tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Artifact {
    /// The path, relative to the projection root.
    pub path: String,
    /// The file's whole contents.
    pub contents: String,
}

impl Projection {
    /// Every file this projection writes, keyed by path relative to the output root.
    ///
    /// The one producer. `--out` writes exactly this map, `--format json` prints exactly this
    /// map, and `cargo xtask infra --check` compares the committed tree against exactly this map.
    #[must_use]
    pub fn artifacts(&self) -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        for patch in &self.patches {
            files.insert(patch.path.clone(), canonical_json(&patch.patch));
        }
        for object in &self.objects {
            files.insert(object.path.clone(), canonical_json(&object.manifest));
        }
        files.insert("SUMMARY.md".to_owned(), summary_markdown(self));
        files.insert("OBLIGATIONS.md".to_owned(), obligations_markdown(self));
        files
    }

    /// The persistable document, artifacts included.
    #[must_use]
    pub fn document(&self) -> ProjectionDocument<'_> {
        ProjectionDocument {
            format: self.format,
            specification: &self.specification,
            provenance: &self.provenance,
            entries: &self.entries,
            summary: &self.summary,
            artifacts: self
                .artifacts()
                .into_iter()
                .map(|(path, contents)| Artifact { path, contents })
                .collect(),
        }
    }

    /// The canonical JSON document, key-sorted, with a trailing newline.
    #[must_use]
    pub fn to_json(&self) -> String {
        let value = serde_json::to_value(self.document())
            .expect("a projection has no unserializable state");
        canonical_json(&value)
    }

    /// The entries whose disposition is not [`Generated`](Disposition::Generated) — what
    /// `OBLIGATIONS.md` is a rendering of.
    pub fn owed(&self) -> impl Iterator<Item = &ProjectionEntry> {
        self.entries
            .iter()
            .filter(|entry| !matches!(entry.disposition, Disposition::Generated(_)))
    }
}

/// A change this projection would make to the cluster.
///
/// Applied to a working copy of the model to reach the next round's simulation, and rendered into
/// a patch or a manifest for the tree. One value, two readings — so the model a projection
/// verifies itself against and the file it hands a reviewer cannot describe different changes.
#[derive(Debug, Clone)]
enum Change {
    /// Set a workload's declared replica count.
    Replicas {
        /// The workload's IR key.
        workload: String,
        /// What it declares now.
        from: u32,
        /// What the patch sets it to.
        to: u32,
    },
    /// Write a container's resource envelope, only the halves the snapshot is missing.
    Resources {
        /// The workload's IR key.
        workload: String,
        /// The container's name.
        container: String,
        /// Requested quantities, when requests are what is missing.
        requests: Option<BTreeMap<String, String>>,
        /// Limit quantities, when limits are what is missing.
        limits: Option<BTreeMap<String, String>>,
    },
    /// Write a container's probes, only the ones the snapshot is missing.
    Probes {
        /// The workload's IR key.
        workload: String,
        /// The container's name.
        container: String,
        /// The liveness probe, when that is the missing one.
        liveness: Option<ProbeRemedy>,
        /// The readiness probe, when that is the missing one.
        readiness: Option<ProbeRemedy>,
    },
    /// Create a disruption budget covering one workload.
    DisruptionBudget {
        /// The namespace it goes in.
        namespace: String,
        /// Its name, which is the workload's.
        name: String,
        /// The selector, which is the workload's own pod selector.
        selector: BTreeMap<String, String>,
    },
}

impl Change {
    /// The object this change is against.
    fn target(&self, model: &InfraModel) -> ObjectRef {
        match self {
            Self::Replicas { workload, .. }
            | Self::Resources { workload, .. }
            | Self::Probes { workload, .. } => {
                let resolved = &model.workloads[workload];
                ObjectRef::workload(
                    resolved.kind,
                    resolved.identity.namespace.as_deref().unwrap_or_default(),
                    &resolved.identity.name,
                )
            }
            Self::DisruptionBudget {
                namespace, name, ..
            } => ObjectRef::disruption_budget(namespace, name),
        }
    }

    /// The one line a reviewer reads beside the patch.
    fn describes(&self) -> String {
        match self {
            Self::Replicas { from, to, .. } => format!("spec.replicas: {from} -> {to}"),
            Self::Resources {
                container,
                requests,
                limits,
                ..
            } => {
                let mut halves = Vec::new();
                if let Some(requests) = requests {
                    halves.push(format!("requests {}", render_quantities(requests)));
                }
                if let Some(limits) = limits {
                    halves.push(format!("limits {}", render_quantities(limits)));
                }
                format!("containers[{container}].resources: {}", halves.join(", "))
            }
            Self::Probes {
                container,
                liveness,
                readiness,
                ..
            } => {
                let mut written = Vec::new();
                if liveness.is_some() {
                    written.push("livenessProbe");
                }
                if readiness.is_some() {
                    written.push("readinessProbe");
                }
                format!("containers[{container}]: {} written", written.join(" and "))
            }
            Self::DisruptionBudget {
                namespace,
                name,
                selector,
            } => format!(
                "poddisruptionbudget {namespace}/{name}: maxUnavailable 1 over {}",
                render_labels(selector)
            ),
        }
    }
}

/// Renders a quantity map the way a manifest reads.
fn render_quantities(quantities: &BTreeMap<String, String>) -> String {
    format!(
        "{{{}}}",
        quantities
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Renders a label map the way a `matchLabels` block reads.
fn render_labels(labels: &BTreeMap<String, String>) -> String {
    labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Projects `spec`'s gaps against `ir` into the changes that would close them.
///
/// Total: there is no failure mode. A validated specification and a compiled IR are both
/// well-formed by construction, so everything left to say is a disposition.
#[must_use]
pub fn project(spec: &InfraSpec, ir: &InfraIr) -> Projection {
    let observed = simulate(spec, ir);
    let mut bench = Workbench::new(ir);

    bench.reach_fixed_point(spec, &observed);
    // What the tree would leave behind. Obligations are read off *this* simulation and not the
    // observed one, so a gap the patches happened to close never arrives as an obligation nobody
    // owes any more.
    let settled = simulate(spec, &bench.working);
    let still_open = bench.owe_the_rest(spec, &observed, &settled);
    bench.account_for_closures(&observed, &still_open);
    bench.recheck_generated(&still_open);

    let (entries, patches, objects) = bench.finish();
    let summary = ProjectionSummary {
        gaps_observed: gap_count(&observed),
        gaps_induced: entries
            .iter()
            .filter(|entry| entry.origin == GapOrigin::Induced)
            .count(),
        generated: count(&entries, "generated"),
        obligations: count(&entries, "obligation"),
        refusals: count(&entries, "refused"),
        patches: patches.len(),
        objects: objects.len(),
        verdicts_before: observed.summary,
        verdicts_after: settled.summary,
    };

    Projection {
        format: PROJECTION_FORMAT,
        specification: spec.name.clone(),
        provenance: ProjectionProvenance {
            context: ir.provenance.context.clone(),
            snapshot_digest: ir.digest(),
            specification_digest: spec.digest(),
        },
        entries,
        patches,
        objects,
        summary,
    }
}

/// The mutable state of one projection while it is being computed.
///
/// A struct rather than eight locals threaded through four passes, because the passes have to see
/// each other's work: the fixed point writes into the drafts the settle pass reads, and the
/// self-check re-reads what the fixed point decided.
struct Workbench {
    /// The observed cluster with every change so far applied — the hypothetical the next round
    /// simulates against.
    working: InfraIr,
    /// Per-object patch drafts, keyed by the target's filename stem.
    drafts: BTreeMap<String, (ObjectRef, PatchDraft)>,
    /// Generated objects, keyed by `namespace/name`.
    created: BTreeMap<String, (ObjectRef, Value)>,
    /// One entry per gap, keyed by the expectation's *index* and the subject. The index is also
    /// the report order, so the entry list sorts into the order a reader reads the simulation in.
    disposed: BTreeMap<(usize, String), ProjectionEntry>,
    /// Where each generated change landed, before the tree's filenames are known.
    slots: BTreeMap<(usize, String), ArtifactSlot>,
    /// Which expectation each generated change was written for.
    generated_by: BTreeMap<(usize, String), String>,
}

impl Workbench {
    /// A bench holding the observed cluster and nothing decided yet.
    fn new(ir: &InfraIr) -> Self {
        Self {
            working: ir.clone(),
            drafts: BTreeMap::new(),
            created: BTreeMap::new(),
            disposed: BTreeMap::new(),
            slots: BTreeMap::new(),
            generated_by: BTreeMap::new(),
        }
    }

    /// Generates every change that can be generated, including for the gaps those changes open.
    ///
    /// Each round disposes what that round's simulation reports and applies it; the next round
    /// sees the world those changes made. It ends when a round generates nothing, which it must,
    /// because `disposed` only grows and (expectation, subject) is finite.
    fn reach_fixed_point(&mut self, spec: &InfraSpec, observed: &Simulation) {
        loop {
            let current = simulate(spec, &self.working);
            let mut progressed = false;
            for (index, report) in current.reports.iter().enumerate() {
                let expectation = &spec.expectations[index];
                for outcome in &report.outcomes {
                    let Outcome::Gap(gap) = &outcome.outcome else {
                        continue;
                    };
                    let key = (index, outcome.subject.clone());
                    if self.disposed.contains_key(&key) {
                        continue;
                    }
                    let Some(change) = generate(
                        expectation,
                        gap,
                        &outcome.subject,
                        &self.working,
                        &self.created,
                    ) else {
                        continue;
                    };
                    let target = change.target(&self.working.model);
                    let slot = record(&change, &target, &mut self.drafts, &mut self.created);
                    let describes = change.describes();
                    apply(&mut self.working.model, &change);
                    self.generated_by.insert(key.clone(), report.id.clone());
                    self.slots.insert(key.clone(), slot);
                    self.disposed.insert(
                        key,
                        entry(
                            report,
                            &outcome.subject,
                            gap,
                            origin_of(observed, index, &outcome.subject),
                            Disposition::Generated(GeneratedChange {
                                // Filled in once every draft is complete; see `ArtifactSlot`.
                                artifact: String::new(),
                                change: describes,
                            }),
                        ),
                    );
                    progressed = true;
                }
            }
            if !progressed {
                return;
            }
        }
    }

    /// Disposes every gap the tree does not close, and answers with the set of them.
    fn owe_the_rest(
        &mut self,
        spec: &InfraSpec,
        observed: &Simulation,
        settled: &Simulation,
    ) -> BTreeSet<(usize, String)> {
        let mut still_open = BTreeSet::new();
        for (index, report) in settled.reports.iter().enumerate() {
            for outcome in &report.outcomes {
                let Outcome::Gap(gap) = &outcome.outcome else {
                    continue;
                };
                let key = (index, outcome.subject.clone());
                still_open.insert(key.clone());
                if self.disposed.contains_key(&key) {
                    continue;
                }
                let expectation = &spec.expectations[index];
                let disposition = owe(
                    expectation,
                    gap,
                    &outcome.subject,
                    &self.working,
                    &self.created,
                );
                self.disposed.insert(
                    key,
                    entry(
                        report,
                        &outcome.subject,
                        gap,
                        origin_of(observed, index, &outcome.subject),
                        disposition,
                    ),
                );
            }
        }
        still_open
    }

    /// Accounts for the gaps that closed on the way.
    ///
    /// A change written for one expectation can satisfy another — raising a workload to two
    /// replicas closes `replicas_within [2, 4]` and `workload.replicas >= 2` at once — and a gap
    /// that simply vanished from the report would be one this projection never accounted for. So
    /// every observed gap that is neither disposed nor still open is recorded as closed, naming
    /// what closed it.
    fn account_for_closures(
        &mut self,
        observed: &Simulation,
        still_open: &BTreeSet<(usize, String)>,
    ) {
        for (index, report) in observed.reports.iter().enumerate() {
            for outcome in &report.outcomes {
                let Outcome::Gap(gap) = &outcome.outcome else {
                    continue;
                };
                let key = (index, outcome.subject.clone());
                if self.disposed.contains_key(&key) || still_open.contains(&key) {
                    continue;
                }
                let by = self
                    .disposed
                    .iter()
                    .find(|((_, subject), entry)| {
                        subject == &outcome.subject
                            && matches!(entry.disposition, Disposition::Generated(_))
                    })
                    .map(|(_, entry)| entry);
                let change = match by {
                    Some(entry) => {
                        let Disposition::Generated(change) = &entry.disposition else {
                            unreachable!("the search filtered on the variant")
                        };
                        GeneratedChange {
                            artifact: change.artifact.clone(),
                            change: format!("closed by {} — {}", entry.expectation, change.change),
                        }
                    }
                    // A change against one subject can close a gap against another — a generated
                    // budget covers whatever its selector matches. Naming a file that does not
                    // mention this subject would be a false attribution, so the summary, which
                    // lists every change in the tree, is the honest pointer.
                    None => GeneratedChange {
                        artifact: "SUMMARY.md".to_owned(),
                        change: "closed by other changes in this tree".to_owned(),
                    },
                };
                let slot = by.and_then(|entry| {
                    self.slots
                        .iter()
                        .find(|((_, subject), _)| subject == &entry.subject)
                        .map(|(_, slot)| slot.clone())
                });
                if let Some(slot) = slot {
                    self.slots.insert(key.clone(), slot);
                }
                self.disposed.insert(
                    key,
                    entry(
                        report,
                        &outcome.subject,
                        gap,
                        GapOrigin::Observed,
                        Disposition::Generated(change),
                    ),
                );
            }
        }
    }

    /// The self-check: a generated entry whose gap survived every change in this tree is a claim
    /// the tree does not support, and the only way to reach one is two expectations that disagree.
    fn recheck_generated(&mut self, still_open: &BTreeSet<(usize, String)>) {
        let generated_by = self.generated_by.clone();
        for (key, entry) in &mut self.disposed {
            if !still_open.contains(key) {
                continue;
            }
            let Disposition::Generated(change) = &entry.disposition else {
                continue;
            };
            let by = generated_by
                .iter()
                .find(|(other, _)| other.1 == key.1 && other.0 != key.0)
                .map_or_else(|| "another expectation".to_owned(), |(_, id)| id.clone());
            entry.disposition = Disposition::Refused(ProjectionRefusal {
                reason: RefusalReason::Contradicted { by: by.clone() },
                detail: format!(
                    "`{}` would close this, and `{by}` writes a different value into the same \
                     field; two declared expectations disagree and picking a winner is not this \
                     projection's decision",
                    change.change
                ),
            });
        }
    }

    /// Assembles the tree: the filenames settle here, and every generated entry names the file it
    /// landed in.
    fn finish(mut self) -> (Vec<ProjectionEntry>, Vec<ObjectPatch>, Vec<NewObject>) {
        let patches: Vec<ObjectPatch> = self
            .drafts
            .into_values()
            .filter(|(_, draft)| !draft.is_empty())
            .map(|(target, draft)| {
                let patch_type = draft.patch_type();
                ObjectPatch {
                    path: patch_path(&target, patch_type),
                    target,
                    patch_type,
                    patch: draft.document(),
                }
            })
            .collect();
        let objects: Vec<NewObject> = self
            .created
            .into_values()
            .map(|(target, manifest)| NewObject {
                path: object_path(&target),
                target,
                manifest,
            })
            .collect();

        let by_slug: BTreeMap<String, &str> = patches
            .iter()
            .map(|patch| (patch.target.slug(), patch.path.as_str()))
            .collect();
        for (key, entry) in &mut self.disposed {
            let Disposition::Generated(change) = &mut entry.disposition else {
                continue;
            };
            change.artifact = match self.slots.get(key) {
                Some(ArtifactSlot::Object(path)) => path.clone(),
                Some(ArtifactSlot::Patch(slug)) => by_slug
                    .get(slug)
                    .map(|path| (*path).to_owned())
                    .unwrap_or_default(),
                // An entry whose closure is not attributable to one file already carries its own
                // pointer; leaving it is not the same as having nothing to say.
                None => change.artifact.clone(),
            };
        }

        (self.disposed.into_values().collect(), patches, objects)
    }
}

/// How many subject gaps a simulation reports.
fn gap_count(simulation: &Simulation) -> usize {
    simulation
        .reports
        .iter()
        .flat_map(|report| &report.outcomes)
        .filter(|outcome| matches!(outcome.outcome, Outcome::Gap(_)))
        .count()
}

/// How many entries carry one disposition.
fn count(entries: &[ProjectionEntry], disposition: &str) -> usize {
    entries
        .iter()
        .filter(|entry| entry.disposition.as_str() == disposition)
        .count()
}

/// Whether the observed simulation already reported this gap.
fn origin_of(observed: &Simulation, index: usize, subject: &str) -> GapOrigin {
    let reported = observed.reports.get(index).is_some_and(|report| {
        report
            .outcomes
            .iter()
            .any(|outcome| outcome.subject == subject && matches!(outcome.outcome, Outcome::Gap(_)))
    });
    if reported {
        GapOrigin::Observed
    } else {
        GapOrigin::Induced
    }
}

/// Assembles one entry.
fn entry(
    report: &infra_spec::ExpectationReport,
    subject: &str,
    gap: &Gap,
    origin: GapOrigin,
    disposition: Disposition,
) -> ProjectionEntry {
    ProjectionEntry {
        expectation: report.id.clone(),
        kind: report.kind.clone(),
        subject: subject.to_owned(),
        origin,
        gap: gap.clone(),
        reads: infra_spec::describe_gap(gap),
        disposition,
    }
}

/// The change that closes a gap, or `None` when closing it is a decision.
///
/// Reads the gap and the working model, and nothing else. There is no configuration, no default
/// table and no heuristic here on purpose: every value written into a patch either comes out of
/// the gap itself or out of a [`Remedy`] a human wrote.
fn generate(
    expectation: &Expectation,
    gap: &Gap,
    subject: &str,
    ir: &InfraIr,
    created: &BTreeMap<String, (ObjectRef, Value)>,
) -> Option<Change> {
    let workload = subject.strip_prefix("workloads/").map(ToOwned::to_owned);
    match gap {
        Gap::ReplicasOutsideRange {
            have,
            want_min,
            want_max,
        } => Some(Change::Replicas {
            workload: workload?,
            from: *have,
            // The nearest acceptable count, which the range decides and nothing else does. Below
            // the floor means raise to the floor; above the ceiling means lower to the ceiling.
            to: if have < want_min {
                *want_min
            } else {
                *want_max
            },
        }),
        Gap::ResourcesAbsent {
            container,
            requests_missing,
            limits_missing,
        } => {
            let remedy = expectation.remedy.as_ref()?;
            let (requests, limits) = remedy.resource_quantities();
            // A remedy that states only one of two missing halves closes nothing — the
            // expectation wants both — so the whole gap is owed rather than half-patched.
            if (*requests_missing && requests.is_empty()) || (*limits_missing && limits.is_empty())
            {
                return None;
            }
            Some(Change::Resources {
                workload: workload?,
                container: container.clone(),
                // A half the snapshot already has is not written: a container declaring requests
                // and no limits keeps the requests it has.
                requests: requests_missing.then(|| requests.clone()),
                limits: limits_missing.then(|| limits.clone()),
            })
        }
        Gap::ProbeAbsent {
            container,
            liveness_missing,
            readiness_missing,
        } => {
            let Some(Remedy::Probes {
                liveness,
                readiness,
            }) = expectation.remedy.as_ref()
            else {
                return None;
            };
            if (*liveness_missing && liveness.is_none())
                || (*readiness_missing && readiness.is_none())
            {
                return None;
            }
            Some(Change::Probes {
                workload: workload?,
                container: container.clone(),
                liveness: if *liveness_missing {
                    liveness.clone()
                } else {
                    None
                },
                readiness: if *readiness_missing {
                    readiness.clone()
                } else {
                    None
                },
            })
        }
        Gap::DisruptionBudgetAbsent { .. } => {
            let key = workload?;
            let resolved = ir.model.workloads.get(&key)?;
            let namespace = resolved.identity.namespace.clone()?;
            // An empty selector would make a budget over the whole namespace, which is a
            // different statement from "cover this workload" and not one the gap asked for.
            if resolved.selector.is_empty() {
                return None;
            }
            // A budget whose selector does not match the pods the workload makes covers nothing,
            // and the gap would still be there after applying it.
            if !infra_analyze::pdb_covers(&resolved.selector, &resolved.template_labels) {
                return None;
            }
            let name = resolved.identity.name.clone();
            let budget_key = format!("{namespace}/{name}");
            if created.contains_key(&budget_key) {
                return None;
            }
            if let Some(budgets) = &ir.model.pod_disruption_budgets {
                if budgets.contains_key(&budget_key) {
                    return None;
                }
            }
            Some(Change::DisruptionBudget {
                namespace,
                name,
                selector: resolved.selector.clone(),
            })
        }
        Gap::WorkloadAbsent { .. }
        | Gap::ImageRegistryNotAllowed { .. }
        | Gap::ImageTagIsLatest { .. }
        | Gap::ImageNotPinned { .. }
        | Gap::SelectorMatchesNoPod { .. }
        | Gap::ReferenceUnresolved { .. }
        | Gap::NamespaceNotAllowed { .. }
        | Gap::PredicateFalse { .. } => None,
    }
}

/// The obligation or refusal a gap nobody can patch arrives as.
///
/// One arm per gap kind, and the arm *is* the sentence somebody acts on — so the twelve answers
/// stay in one place a reader can compare, rather than in twelve helpers they have to chase.
#[allow(clippy::too_many_lines)]
fn owe(
    expectation: &Expectation,
    gap: &Gap,
    subject: &str,
    ir: &InfraIr,
    created: &BTreeMap<String, (ObjectRef, Value)>,
) -> Disposition {
    let obligation = |reason: ObligationReason, decision: String| {
        Disposition::Obligation(ProjectionObligation { reason, decision })
    };
    match gap {
        Gap::WorkloadAbsent {
            namespace,
            kind,
            name,
        } => obligation(
            ObligationReason::ObjectUndefined,
            format!(
                "write a {} manifest for {namespace}/{name}: this specification says it should \
                 exist and says nothing about what it would run",
                kind.as_str()
            ),
        ),
        Gap::ResourcesAbsent {
            container,
            requests_missing,
            limits_missing,
        } => {
            let mut fields = Vec::new();
            if *requests_missing {
                fields.push("resources.requests".to_owned());
            }
            if *limits_missing {
                fields.push("resources.limits".to_owned());
            }
            let unstated: Vec<String> = unstated_resource_halves(expectation, &fields);
            obligation(
                ObligationReason::ValueUnstated {
                    fields: unstated.clone(),
                },
                format!(
                    "choose the {} for container `{container}`, or state them once as a \
                     `remedy: {{resources: …}}` on expectation `{}` and this projection writes \
                     them",
                    unstated.join(" and "),
                    expectation.id
                ),
            )
        }
        Gap::ProbeAbsent {
            container,
            liveness_missing,
            readiness_missing,
        } => {
            let mut fields = Vec::new();
            if *liveness_missing {
                fields.push("probes.liveness".to_owned());
            }
            if *readiness_missing {
                fields.push("probes.readiness".to_owned());
            }
            let unstated = unstated_probes(expectation, &fields);
            obligation(
                ObligationReason::ValueUnstated {
                    fields: unstated.clone(),
                },
                format!(
                    "choose what makes container `{container}` healthy — a path and a port, or a \
                     port to connect to — and state it as a `remedy: {{probes: …}}` on \
                     expectation `{}`; {} unstated",
                    expectation.id,
                    unstated.join(" and ")
                ),
            )
        }
        Gap::ImageRegistryNotAllowed {
            container,
            image,
            allowed,
            ..
        } => obligation(
            ObligationReason::ImageChoice,
            format!(
                "choose the build of `{image}` that container `{container}` should run from one \
                 of [{}]; a rewritten registry prefix is a different image, not the same one \
                 somewhere else",
                allowed.join(", ")
            ),
        ),
        Gap::ImageTagIsLatest {
            container, image, ..
        }
        | Gap::ImageNotPinned { container, image } => obligation(
            ObligationReason::ImageChoice,
            format!(
                "choose the version of `{image}` that container `{container}` should run and \
                     write it as a tag or a `sha256:` digest; nothing in the snapshot says which \
                     build `{image}` is today"
            ),
        ),
        Gap::DisruptionBudgetAbsent { replicas } => {
            disruption_budget_obligation(subject, *replicas, ir, created)
        }
        Gap::SelectorMatchesNoPod { selector } => obligation(
            ObligationReason::TargetUnknown,
            format!(
                "decide which is true: the workload that should carry {} was never deployed, or \
                 this selector names labels nothing carries. Deploying one and rewriting the \
                 other are different changes and the snapshot does not say which was meant",
                render_labels(selector)
            ),
        ),
        Gap::ReferenceUnresolved { site, target } => obligation(
            ObligationReason::TargetUnknown,
            format!(
                "create the {target} this cluster expects at {site}, or change the reference. \
                 Its contents are not in the snapshot — a secret is only ever there as a digest \
                 — so nothing here can write one"
            ),
        ),
        Gap::NamespaceNotAllowed { have, allowed } => obligation(
            ObligationReason::FieldImmutable {
                field: "metadata.namespace".to_owned(),
            },
            format!(
                "recreate this workload in one of [{}]: a live object's namespace cannot be \
                 patched, so moving it out of `{have}` is a delete and a create, which is not a \
                 patch and not this projection's to write",
                allowed.join(", ")
            ),
        ),
        Gap::ReplicasOutsideRange {
            have,
            want_min,
            want_max,
        } => obligation(
            ObligationReason::TargetUnknown,
            format!(
                "set the replica count of `{subject}` to within [{want_min}, {want_max}]; it \
                 declares {have}. This projection writes that patch itself wherever the subject \
                 is a workload the snapshot holds, so reaching this sentence means it is not one"
            ),
        ),
        Gap::PredicateFalse { predicate, facts } => Disposition::Refused(ProjectionRefusal {
            reason: RefusalReason::NotAField,
            detail: format!(
                "`{predicate}` is false at {}. A predicate states a condition, and a condition \
                 does not name the field that would satisfy it — the same predicate is false for \
                 a workload that needs another replica and for one whose pods will not start",
                facts
                    .iter()
                    .map(|(path, value)| format!("{path}={value}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }),
    }
}

/// Which halves of a resources gap nobody stated a value for.
fn unstated_resource_halves(expectation: &Expectation, missing: &[String]) -> Vec<String> {
    let Some(remedy) = expectation.remedy.as_ref() else {
        return missing.to_vec();
    };
    let (requests, limits) = remedy.resource_quantities();
    missing
        .iter()
        .filter(|field| match field.as_str() {
            "resources.requests" => requests.is_empty(),
            "resources.limits" => limits.is_empty(),
            _ => true,
        })
        .cloned()
        .collect()
}

/// Which probes nobody stated a value for.
fn unstated_probes(expectation: &Expectation, missing: &[String]) -> Vec<String> {
    let Some(Remedy::Probes {
        liveness,
        readiness,
    }) = expectation.remedy.as_ref()
    else {
        return missing.to_vec();
    };
    missing
        .iter()
        .filter(|field| match field.as_str() {
            "probes.liveness" => liveness.is_none(),
            "probes.readiness" => readiness.is_none(),
            _ => true,
        })
        .cloned()
        .collect()
}

/// The three reasons a missing budget is a decision rather than a manifest.
fn disruption_budget_obligation(
    subject: &str,
    replicas: u32,
    ir: &InfraIr,
    created: &BTreeMap<String, (ObjectRef, Value)>,
) -> Disposition {
    let obligation = |reason: ObligationReason, decision: String| {
        Disposition::Obligation(ProjectionObligation { reason, decision })
    };
    let Some(key) = subject.strip_prefix("workloads/") else {
        return obligation(
            ObligationReason::TargetUnknown,
            "this gap names no workload to write a budget for".to_owned(),
        );
    };
    let Some(workload) = ir.model.workloads.get(key) else {
        return obligation(
            ObligationReason::TargetUnknown,
            format!("`{key}` is not a workload this snapshot holds"),
        );
    };
    let namespace = workload.identity.namespace.clone().unwrap_or_default();
    let name = workload.identity.name.clone();
    let budget_key = format!("{namespace}/{name}");
    let taken = created.contains_key(&budget_key)
        || ir
            .model
            .pod_disruption_budgets
            .as_ref()
            .is_some_and(|budgets| budgets.contains_key(&budget_key));
    if taken {
        return obligation(
            ObligationReason::NameTaken {
                object: format!("poddisruptionbudget {budget_key}"),
            },
            format!(
                "a disruption budget named `{name}` is already in `{namespace}` and does not \
                 cover this workload; choose another name, or widen the budget that is there"
            ),
        );
    }
    if workload.selector.is_empty() {
        return obligation(
            ObligationReason::ValueUnstated {
                fields: vec!["spec.selector.matchLabels".to_owned()],
            },
            format!(
                "this workload declares no pod selector, so there is nothing to write into a \
                 budget's `matchLabels`; a budget with an empty selector guards every pod in \
                 `{namespace}`, which is a different statement from covering {replicas} replicas \
                 of `{name}`"
            ),
        );
    }
    obligation(
        ObligationReason::ValueUnstated {
            fields: vec!["spec.selector.matchLabels".to_owned()],
        },
        format!(
            "this workload's own selector {} does not match its pod template's labels {}, so a \
             budget built from it would cover nothing; fix the workload before a budget can \
             cover it",
            render_labels(&workload.selector),
            render_labels(&workload.template_labels)
        ),
    )
}

/// Where a generated change ended up, before the tree's filenames are known.
///
/// A patch file's name carries its type, and its type is the join over every change in it — so a
/// replica change filed before a container change would name a file that does not exist by the
/// time the tree is written. The slot defers the name until every draft is complete.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ArtifactSlot {
    /// A patch, by the target object's filename stem.
    Patch(String),
    /// A generated object, whose path is settled the moment it is written.
    Object(String),
}

/// Files a change into the tree it belongs to, and answers with where it went.
fn record(
    change: &Change,
    target: &ObjectRef,
    drafts: &mut BTreeMap<String, (ObjectRef, PatchDraft)>,
    created: &mut BTreeMap<String, (ObjectRef, Value)>,
) -> ArtifactSlot {
    if let Change::DisruptionBudget {
        namespace,
        name,
        selector,
    } = change
    {
        let manifest = serde_json::json!({
            "apiVersion": target.api_version,
            "kind": target.kind,
            "metadata": {"name": name, "namespace": namespace},
            "spec": {
                // One pod at a time, which is the only bound the gap itself determines: the gap
                // says "more than one replica and nothing covers it", and `maxUnavailable: 1` is
                // the weakest budget that answers it. `minAvailable: <replicas>` would block
                // every voluntary eviction, and any other number is a decision about how much of
                // this workload may be down — which nobody wrote down.
                "maxUnavailable": 1,
                "selector": {"matchLabels": selector},
            },
        });
        created.insert(format!("{namespace}/{name}"), (target.clone(), manifest));
        return ArtifactSlot::Object(object_path(target));
    }

    let (_, draft) = drafts
        .entry(target.slug())
        .or_insert_with(|| (target.clone(), PatchDraft::default()));
    match change {
        Change::Replicas { to, .. } => draft.set_spec("replicas", Value::from(*to)),
        Change::Resources {
            container,
            requests,
            limits,
            ..
        } => {
            let mut resources = serde_json::Map::new();
            if let Some(requests) = requests {
                resources.insert("requests".to_owned(), quantities_json(requests));
            }
            if let Some(limits) = limits {
                resources.insert("limits".to_owned(), quantities_json(limits));
            }
            draft.set_container(container, "resources", Value::Object(resources));
        }
        Change::Probes {
            container,
            liveness,
            readiness,
            ..
        } => {
            if let Some(probe) = liveness {
                draft.set_container(container, "livenessProbe", probe_json(probe));
            }
            if let Some(probe) = readiness {
                draft.set_container(container, "readinessProbe", probe_json(probe));
            }
        }
        // Returned above; a manifest is not a patch and never reaches a draft.
        Change::DisruptionBudget { .. } => {}
    }
    ArtifactSlot::Patch(target.slug())
}

/// A quantity map as a manifest holds it.
fn quantities_json(quantities: &BTreeMap<String, String>) -> Value {
    Value::Object(
        quantities
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect(),
    )
}

/// A probe as a manifest holds it — the API's camel-case spellings, not the specification's.
fn probe_json(probe: &ProbeRemedy) -> Value {
    let mut rendered = serde_json::Map::new();
    match &probe.handler {
        ProbeHandlerRemedy::HttpGet { path, port } => {
            let mut handler = serde_json::Map::new();
            if let Some(path) = path {
                handler.insert("path".to_owned(), Value::String(path.clone()));
            }
            handler.insert("port".to_owned(), port_json(port));
            rendered.insert("httpGet".to_owned(), Value::Object(handler));
        }
        ProbeHandlerRemedy::TcpSocket { port } => {
            rendered.insert(
                "tcpSocket".to_owned(),
                serde_json::json!({"port": port_json(port)}),
            );
        }
    }
    for (field, value) in [
        ("initialDelaySeconds", probe.initial_delay_seconds),
        ("periodSeconds", probe.period_seconds),
        ("timeoutSeconds", probe.timeout_seconds),
        ("failureThreshold", probe.failure_threshold),
    ] {
        if let Some(value) = value {
            rendered.insert(field.to_owned(), Value::from(value));
        }
    }
    Value::Object(rendered)
}

/// A port as a manifest holds it: a number stays a number, a name stays a string.
fn port_json(port: &Port) -> Value {
    match port {
        Port::Number(number) => Value::from(*number),
        Port::Name(name) => Value::String(name.clone()),
    }
}

/// The observed shape of a stated probe, for the working model.
fn probe_observed(probe: &ProbeRemedy) -> Probe {
    Probe {
        handler: match &probe.handler {
            ProbeHandlerRemedy::HttpGet { path, port } => ProbeHandler::HttpGet {
                path: path.clone(),
                port: Some(port.to_string()),
            },
            ProbeHandlerRemedy::TcpSocket { port } => ProbeHandler::Tcp {
                port: Some(port.to_string()),
            },
        },
        initial_delay_seconds: probe.initial_delay_seconds,
        period_seconds: probe.period_seconds,
        timeout_seconds: probe.timeout_seconds,
        failure_threshold: probe.failure_threshold,
    }
}

/// Applies a change to the working model — the hypothetical cluster the next round simulates.
///
/// The mutation and the patch are built from **one** [`Change`], which is what makes the
/// verification honest: if these two ever described different edits, the projection would verify
/// a cluster nobody would get by applying its files. `tests/round_trip.rs` closes the loop from
/// the other side, by applying the emitted patches to the bundle and recompiling.
fn apply(model: &mut InfraModel, change: &Change) {
    match change {
        Change::Replicas { workload, to, .. } => {
            if let Some(resolved) = model.workloads.get_mut(workload) {
                resolved.replicas = Some(*to);
            }
        }
        Change::Resources {
            workload,
            container,
            requests,
            limits,
        } => {
            if let Some(found) = container_mut(model, workload, container) {
                found.resources = Resources {
                    requests: requests
                        .clone()
                        .unwrap_or_else(|| found.resources.requests.clone()),
                    limits: limits
                        .clone()
                        .unwrap_or_else(|| found.resources.limits.clone()),
                };
            }
        }
        Change::Probes {
            workload,
            container,
            liveness,
            readiness,
        } => {
            if let Some(found) = container_mut(model, workload, container) {
                let probes = Probes {
                    liveness: liveness
                        .as_ref()
                        .map(probe_observed)
                        .or_else(|| found.probes.liveness.clone()),
                    readiness: readiness
                        .as_ref()
                        .map(probe_observed)
                        .or_else(|| found.probes.readiness.clone()),
                    startup: found.probes.startup.clone(),
                };
                found.probes = probes;
            }
        }
        Change::DisruptionBudget {
            namespace,
            name,
            selector,
        } => {
            let budgets = model
                .pod_disruption_budgets
                .get_or_insert_with(BTreeMap::new);
            budgets.insert(
                format!("{namespace}/{name}"),
                PodDisruptionBudget {
                    identity: Identity {
                        namespace: Some(namespace.clone()),
                        name: name.clone(),
                        uid: PROJECTED_UID.to_owned(),
                    },
                    labels: BTreeMap::new(),
                    selector: selector.clone(),
                    min_available: None,
                    max_unavailable: Some("1".to_owned()),
                },
            );
        }
    }
}

/// One container of one workload in the working model, by name.
fn container_mut<'a>(
    model: &'a mut InfraModel,
    workload: &str,
    container: &str,
) -> Option<&'a mut infra_compiler::ir::ResolvedContainer> {
    model
        .workloads
        .get_mut(workload)?
        .containers
        .iter_mut()
        .find(|found| found.name == container)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_bound_of_a_range_is_the_bound_the_count_is_outside_of() {
        // The one value this crate derives without a human stating it, so it is worth pinning:
        // below the floor raises to the floor, above the ceiling lowers to the ceiling, and
        // neither ever lands in the middle of somebody's range.
        for (have, want_min, want_max, expected) in [(1, 2, 4, 2), (9, 2, 4, 4), (0, 3, 3, 3)] {
            let gap = Gap::ReplicasOutsideRange {
                have,
                want_min,
                want_max,
            };
            let Gap::ReplicasOutsideRange {
                have,
                want_min,
                want_max,
            } = gap
            else {
                unreachable!()
            };
            let to = if have < want_min { want_min } else { want_max };
            assert_eq!(to, expected, "{have} against [{want_min}, {want_max}]");
        }
    }

    #[test]
    fn a_manifest_port_keeps_the_type_it_was_written_as() {
        assert_eq!(port_json(&Port::Number(8080)), Value::from(8080));
        assert_eq!(
            port_json(&Port::Name("http".to_owned())),
            Value::String("http".to_owned()),
            "a named port is a string; a number in quotes would name a port called `8080`"
        );
    }

    #[test]
    fn a_generated_budget_names_no_uid_because_nothing_has_assigned_one() {
        let target = ObjectRef::disruption_budget("shop", "storefront-server");
        let mut drafts = BTreeMap::new();
        let mut created = BTreeMap::new();
        record(
            &Change::DisruptionBudget {
                namespace: "shop".to_owned(),
                name: "storefront-server".to_owned(),
                selector: BTreeMap::from([("app".to_owned(), "shop".to_owned())]),
            },
            &target,
            &mut drafts,
            &mut created,
        );
        let (_, manifest) = &created["shop/storefront-server"];
        assert!(
            manifest["metadata"].get("uid").is_none(),
            "a manifest is what a person commits; the API server assigns the uid: {manifest}"
        );
        assert_eq!(manifest["spec"]["maxUnavailable"], Value::from(1));
    }
}
