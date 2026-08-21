//! Text renderings of the two documents, for a person reading a terminal.
//!
//! Canonical like the JSON is: same input, same bytes, no clock and no width detection. A
//! rendering that depends on the terminal is a rendering two runs of one build can disagree
//! about, and `tests/determinism.rs` renders twice and compares.

use std::fmt::Write as _;

use aep_domain::predicate::Truth;

use crate::drift::{InfraChange, InfraDrift};
use crate::simulate::{Outcome, Simulation};

/// The marker a verdict prints as: three characters, one per truth value.
fn marker(verdict: Truth) -> &'static str {
    match verdict {
        Truth::True => "ok ",
        Truth::False => "gap",
        Truth::Unknown => "unk",
    }
}

/// One line per expectation, its subjects beneath when it did not simply hold, then the counts.
#[must_use]
pub fn simulation_to_text(simulation: &Simulation) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "{} against {} ({})",
        simulation.specification,
        simulation.snapshot.context,
        &simulation.snapshot.digest[..12]
    );
    let _ = writeln!(rendered);
    for report in &simulation.reports {
        let _ = writeln!(
            rendered,
            "{} {} — {} [{}, {} subject{}]",
            marker(report.verdict),
            report.id,
            report.claim,
            report.scope,
            report.subjects.len(),
            if report.subjects.len() == 1 { "" } else { "s" }
        );
        if let Some(statement) = &report.statement {
            let _ = writeln!(rendered, "      {statement}");
        }
        for outcome in &report.outcomes {
            let detail = match &outcome.outcome {
                Outcome::Holds => continue,
                Outcome::Gap(gap) => describe_gap(gap),
                Outcome::Undecidable(reason) => reason.to_string(),
            };
            let _ = writeln!(rendered, "      {} — {detail}", outcome.subject);
        }
    }
    let summary = &simulation.summary;
    let _ = writeln!(rendered);
    let _ = writeln!(
        rendered,
        "{} expectations: {} hold, {} gaps, {} undecidable",
        summary.expectations, summary.holds, summary.gaps, summary.undecidable
    );
    rendered
}

/// One sentence saying what a subject has and what the expectation wanted.
fn describe_gap(gap: &crate::simulate::Gap) -> String {
    use crate::simulate::Gap;
    match gap {
        Gap::WorkloadAbsent {
            namespace,
            kind,
            name,
        } => format!("no {} named {name} in {namespace}", kind.as_str()),
        Gap::ReplicasOutsideRange {
            have,
            want_min,
            want_max,
        } => format!("declares {have} replicas, wanted [{want_min}, {want_max}]"),
        Gap::ResourcesAbsent {
            container,
            requests_missing,
            limits_missing,
        } => format!(
            "container {container} declares no {}",
            missing_pair(*requests_missing, *limits_missing, "requests", "limits")
        ),
        Gap::ProbeAbsent {
            container,
            liveness_missing,
            readiness_missing,
        } => format!(
            "container {container} declares no {} probe",
            missing_pair(
                *liveness_missing,
                *readiness_missing,
                "liveness",
                "readiness"
            )
        ),
        Gap::ImageRegistryNotAllowed {
            container,
            image,
            have,
            allowed,
        } => format!(
            "container {container} pulls {image} from {have}, wanted one of [{}]",
            allowed.join(", ")
        ),
        Gap::ImageTagIsLatest {
            container,
            image,
            tag,
        } => match tag {
            Some(tag) => format!("container {container} pulls {image}, tagged `{tag}`"),
            None => format!("container {container} pulls {image}, untagged — which is `latest`"),
        },
        Gap::ImageNotPinned { container, image } => {
            format!("container {container} pulls {image}, not pinned by digest")
        }
        Gap::DisruptionBudgetAbsent { replicas } => {
            format!("declares {replicas} replicas and no disruption budget covers it")
        }
        Gap::SelectorMatchesNoPod { selector } => format!(
            "selector {} matches no observed pod",
            selector
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Gap::ReferenceUnresolved { site, target } => {
            format!("{site} requires {target}, which was not observed")
        }
        Gap::NamespaceNotAllowed { have, allowed } => {
            format!("sits in {have}, wanted one of [{}]", allowed.join(", "))
        }
        Gap::PredicateFalse { predicate, facts } => format!(
            "`{predicate}` is false at {}",
            facts
                .iter()
                .map(|(path, value)| format!("{path}={value}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `a`, `b`, or `a and b` — whichever of the pair is missing.
fn missing_pair(first: bool, second: bool, first_word: &str, second_word: &str) -> String {
    match (first, second) {
        (true, true) => format!("{first_word} and {second_word}"),
        (true, false) => first_word.to_owned(),
        (false, true) => second_word.to_owned(),
        // Unreachable from a gap, which exists only when at least one is missing; rendering the
        // pair anyway beats a panic in a formatter.
        (false, false) => format!("{first_word} or {second_word}"),
    }
}

/// One line per change, then the count.
#[must_use]
pub fn drift_to_text(report: &InfraDrift) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "{}: {} -> {}",
        report.from.context,
        &report.from.digest[..12],
        &report.to.digest[..12]
    );
    let _ = writeln!(rendered);
    if report.changes.is_empty() {
        let _ = writeln!(
            rendered,
            "no change: the two snapshots are the same cluster state"
        );
        return rendered;
    }
    for change in &report.changes {
        let _ = writeln!(rendered, "  {change}");
    }
    let _ = writeln!(rendered);
    let _ = writeln!(
        rendered,
        "{} change{}",
        report.changes.len(),
        if report.changes.len() == 1 { "" } else { "s" }
    );
    rendered
}

/// How many changes of each variant a report holds, for a summary line a harness can read.
#[must_use]
pub fn drift_counts(report: &InfraDrift) -> std::collections::BTreeMap<&'static str, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for change in &report.changes {
        *counts.entry(change_name(change)).or_insert(0) += 1;
    }
    counts
}

/// The wire discriminant of a change, for counting.
fn change_name(change: &InfraChange) -> &'static str {
    match change {
        InfraChange::Added { .. } => "added",
        InfraChange::Removed { .. } => "removed",
        InfraChange::ReplicasChanged { .. } => "replicas_changed",
        InfraChange::ContainerAdded { .. } => "container_added",
        InfraChange::ContainerRemoved { .. } => "container_removed",
        InfraChange::ImageChanged { .. } => "image_changed",
        InfraChange::ResourcesChanged { .. } => "resources_changed",
        InfraChange::ProbesChanged { .. } => "probes_changed",
        InfraChange::EnvironmentChanged { .. } => "environment_changed",
        InfraChange::WorkloadFieldChanged { .. } => "workload_field_changed",
        InfraChange::ServiceFieldChanged { .. } => "service_field_changed",
        InfraChange::IngressRoutingChanged { .. } => "ingress_routing_changed",
        InfraChange::ConfigContentChanged { .. } => "config_content_changed",
        InfraChange::ClaimPhaseChanged { .. } => "claim_phase_changed",
        InfraChange::ReferenceBroke { .. } => "reference_broke",
        InfraChange::ReferenceHealed { .. } => "reference_healed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_marker_is_three_characters_so_the_columns_line_up() {
        for verdict in [Truth::True, Truth::False, Truth::Unknown] {
            assert_eq!(marker(verdict).len(), 3, "{verdict} renders off-column");
        }
    }

    #[test]
    fn the_missing_pair_names_only_what_is_missing() {
        assert_eq!(
            missing_pair(true, true, "requests", "limits"),
            "requests and limits"
        );
        assert_eq!(missing_pair(true, false, "requests", "limits"), "requests");
        assert_eq!(missing_pair(false, true, "requests", "limits"), "limits");
    }
}
