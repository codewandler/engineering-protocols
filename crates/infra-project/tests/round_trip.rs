//! The claim this whole crate rests on, checked the long way round.
//!
//! A projection says: *apply these files and every gap I marked generated holds, every gap I owed
//! stays exactly as it is, and nothing else moves.* The library reaches that answer by mutating a
//! copy of the IR — fast, and one edit away from being a claim about itself. This test reaches it
//! through the bytes instead: the emitted patch files are applied to the **observation bundle**,
//! the bundle is recompiled from scratch, and the specification is simulated against the result.
//!
//! Two independent routes to one post-state. If the patch a reviewer would apply and the model the
//! projection verified itself against ever disagree, the disagreement is here.
//!
//! # Blast radius is the point
//!
//! "Every gap it claimed closes" is the easy half. The half that matters is **every other outcome
//! unchanged**: a patch that satisfies one expectation by breaking another passes the first check
//! and fails this one. So the comparison is over the whole outcome map, keyed by expectation and
//! subject, and a subject that was undecidable and is now a gap — or held and is now undecidable —
//! is a failure with its own sentence.

mod apply;
mod support;

use std::collections::{BTreeMap, BTreeSet};

use infra_compiler::InfraIr;
use infra_project::{Disposition, Projection};
use infra_spec::{InfraSpec, Outcome};
use serde_json::Value;

/// The bundle key each patched or generated kind lives under.
fn bundle_kind(kind: &str) -> &'static str {
    match kind {
        "Deployment" => "deployments",
        "StatefulSet" => "statefulsets",
        "DaemonSet" => "daemonsets",
        "PodDisruptionBudget" => "poddisruptionbudgets",
        other => panic!("this projection does not write a {other}"),
    }
}

/// The uid a generated object is given **to enter a bundle**, and only for that.
///
/// A bundle is an *observation*: `INFRA-OBJECT-002` requires an identity on every object in one,
/// because everything downstream keys on it. The emitted manifest carries no uid — a manifest must
/// not claim an identity the API server has not assigned — so this test supplies the one the
/// cluster would have. It is the single honest seam between "a file somebody commits" and "an
/// object somebody observed", and it is here rather than in the library so the library cannot use
/// it.
const APPLIED_UID: &str = "uid-applied-by-round-trip";

/// Applies a whole projection to a bundle document, in place.
fn apply_projection(bundle: &mut Value, projection: &Projection, files: &BTreeMap<String, String>) {
    for patch in &projection.patches {
        let document: Value = serde_json::from_str(
            files
                .get(&patch.path)
                .unwrap_or_else(|| panic!("{} is in the tree", patch.path)),
        )
        .expect("an emitted patch is JSON");
        let keyed: &[(&str, &str)] = match patch.patch_type {
            infra_project::PatchType::Merge => &[],
            infra_project::PatchType::Strategic => apply::KEYED_LISTS,
        };
        let items = bundle["kinds"][bundle_kind(&patch.target.kind)]["items"]
            .as_array_mut()
            .expect("the bundle scanned this kind");
        let object = items
            .iter_mut()
            .find(|item| {
                item["metadata"]["name"] == Value::String(patch.target.name.clone())
                    && item["metadata"]["namespace"]
                        == Value::String(patch.target.namespace.clone())
            })
            .unwrap_or_else(|| panic!("{} is in the bundle", patch.target));
        apply::apply(object, &document, keyed);
    }

    for object in &projection.objects {
        let mut manifest: Value = serde_json::from_str(
            files
                .get(&object.path)
                .unwrap_or_else(|| panic!("{} is in the tree", object.path)),
        )
        .expect("an emitted manifest is JSON");
        assert!(
            manifest["metadata"].get("uid").is_none(),
            "a committed manifest must not claim a uid: {manifest}"
        );
        manifest["metadata"]["uid"] = Value::String(format!("{APPLIED_UID}-{}", object.path));
        bundle["kinds"][bundle_kind(&object.target.kind)]["items"]
            .as_array_mut()
            .expect("the bundle scanned this kind")
            .push(manifest);
    }
}

/// Every subject that did not simply hold, keyed by expectation and subject.
fn outcomes(spec: &InfraSpec, ir: &InfraIr) -> BTreeMap<(String, String), Outcome> {
    infra_spec::simulate(spec, ir)
        .reports
        .into_iter()
        .flat_map(|report| {
            report
                .outcomes
                .into_iter()
                .map(move |outcome| ((report.id.clone(), outcome.subject), outcome.outcome))
        })
        .collect()
}

/// Runs the whole round trip and answers with every way it did not hold.
///
/// A `Vec` of sentences rather than a panic, so the mutation test below can assert that a
/// corrupted patch is *named* — a guard that fails without saying which expectation regressed
/// costs the next reader the hour this crate exists to save.
fn round_trip(
    spec: &InfraSpec,
    bundle_text: &str,
    corrupt: impl Fn(&mut BTreeMap<String, String>),
) -> Vec<String> {
    let ir = support::compile(bundle_text);
    let projection = infra_project::project(spec, &ir);
    let mut files = projection.artifacts();
    corrupt(&mut files);

    let mut bundle: Value = serde_json::from_str(bundle_text).expect("the bundle is JSON");
    apply_projection(&mut bundle, &projection, &files);
    let patched = support::compile(&bundle.to_string());

    let before = outcomes(spec, &ir);
    let after = outcomes(spec, &patched);

    let mut failures = Vec::new();
    let mut accounted: BTreeSet<(String, String)> = BTreeSet::new();
    for entry in &projection.entries {
        let key = (entry.expectation.clone(), entry.subject.clone());
        accounted.insert(key.clone());
        match &entry.disposition {
            Disposition::Generated(change) => match after.get(&key) {
                None => {}
                Some(outcome) => failures.push(format!(
                    "`{}` on `{}` was generated ({}) and did not close: after applying the tree \
                     it is {outcome:?}",
                    entry.expectation, entry.subject, change.change
                )),
            },
            Disposition::Obligation(_) | Disposition::Refused(_) => match after.get(&key) {
                Some(Outcome::Gap(gap)) if gap == &entry.gap => {}
                other => failures.push(format!(
                    "`{}` on `{}` was owed, so applying the tree must leave it exactly as it \
                     was; it is now {other:?}",
                    entry.expectation, entry.subject
                )),
            },
        }
    }

    for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
        if accounted.contains(key) {
            continue;
        }
        if before.get(key) != after.get(key) {
            failures.push(format!(
                "`{}` on `{}` moved and no entry accounts for it: {:?} -> {:?}. A patch that \
                 fixes one expectation by moving another is the blast radius this test exists \
                 for",
                key.0,
                key.1,
                before.get(key),
                after.get(key)
            ));
        }
    }

    let predicted = projection.summary.verdicts_after;
    let actual = infra_spec::simulate(spec, &patched).summary;
    if predicted != actual {
        failures.push(format!(
            "the projection predicted {predicted:?} and applying its own files produced {actual:?}"
        ));
    }
    failures
}

#[test]
fn applying_the_emitted_tree_closes_every_gap_it_claims_and_moves_nothing_else() {
    let spec = support::example_spec();
    let bundle = support::read("examples/k3d-dev-cluster/observation.json");
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    // The fixture has to reach the state this test is about, or it proves nothing: something must
    // be generated, and something must be left owed.
    assert!(
        projection.summary.generated > 0 && projection.summary.obligations > 0,
        "the committed fixture is meant to produce both kinds of disposition: {:?}",
        projection.summary
    );
    assert!(
        projection.summary.gaps_induced > 0,
        "the committed fixture is meant to reach the case where the projection's own changes open \
         a gap it then closes: {:?}",
        projection.summary
    );

    let failures = round_trip(&spec, &bundle, |_| {});
    assert!(
        failures.is_empty(),
        "the emitted tree does not do what it says:\n{}",
        failures.join("\n")
    );
}

#[test]
fn a_corrupted_patch_value_is_caught_and_the_regressed_expectation_is_named() {
    // The mutation this test exists for, applied to the *emitted bytes* rather than to the code:
    // somebody edits a patch file down to a value that no longer satisfies the expectation it was
    // written for. Nothing about the projection document changes, so only the round trip can see
    // it — and it has to say which expectation regressed, not that "a check failed".
    let spec = support::example_spec();
    let bundle = support::read("examples/k3d-dev-cluster/observation.json");
    let failures = round_trip(&spec, &bundle, |files| {
        let corrupted = "patches/shop.deployment.flaky-agent.strategic.json";
        let mut document: Value =
            serde_json::from_str(&files[corrupted]).expect("the emitted patch is JSON");
        document["spec"]["replicas"] = Value::from(1);
        files.insert(corrupted.to_owned(), document.to_string());
    });
    assert!(
        failures
            .iter()
            .any(|failure| failure.contains("shop-replicas")
                && failure.contains("workloads/shop/deployment/flaky-agent")),
        "a patch corrupted back to the observed value must name the expectation that no longer \
         closes:\n{}",
        failures.join("\n")
    );
}

#[test]
fn a_container_patch_emitted_as_a_plain_merge_would_delete_the_containers_it_does_not_name() {
    // Why the type on the filename is load-bearing rather than decorative. The same emitted
    // document, applied under the two rules, gives two different clusters — and the wrong one
    // does not fail loudly at apply time, it silently drops containers.
    let spec = support::example_spec();
    let bundle_text = support::read("examples/k3d-dev-cluster/observation.json");
    let ir = support::compile(&bundle_text);
    let projection = infra_project::project(&spec, &ir);
    let files = projection.artifacts();
    let patch = projection
        .patches
        .iter()
        .find(|patch| patch.patch_type == infra_project::PatchType::Strategic)
        .expect("the fixture produces a strategic patch");
    let document: Value = serde_json::from_str(&files[&patch.path]).expect("JSON");

    let mut bundle: Value = serde_json::from_str(&bundle_text).expect("JSON");
    let items = bundle["kinds"]["deployments"]["items"]
        .as_array_mut()
        .expect("scanned");
    let object = items
        .iter_mut()
        .find(|item| item["metadata"]["name"] == Value::String(patch.target.name.clone()))
        .expect("the workload is in the bundle");
    let before = object["spec"]["template"]["spec"]["containers"]
        .as_array()
        .expect("a list")
        .clone();
    apply::apply(object, &document, &[]);
    let after = object["spec"]["template"]["spec"]["containers"]
        .as_array()
        .expect("a list")
        .clone();

    assert_eq!(before.len(), 1, "the fixture's workload has one container");
    assert!(
        after[0].get("image").is_none(),
        "applied as a plain merge patch the container list is replaced, so the image the cluster \
         was running is gone: {after:?}"
    );
}
