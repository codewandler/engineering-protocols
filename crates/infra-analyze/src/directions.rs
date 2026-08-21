//! Directions: the deduplicated, severity-ranked "what next" a diagnosis adds up to.
//!
//! A diagnosis of a real cluster is long — hundreds of findings, many of them one cause wearing
//! many subjects. A direction is the aggregation an operator acts on: one entry per (code,
//! shared root cause), highest severity first, every affected subject listed once. **Derived
//! deterministically from findings and candidates, prescribing nothing beyond what they
//! state**: the action line is the code's registered meaning plus the shared evidence, never an
//! invented remediation.
//!
//! # Grouping by root evidence
//!
//! Where a code carries one evidence key that names the *cause* rather than the subject — the
//! missing reference's name, the waiting reason, the autoscaler's absent target — findings
//! sharing that value collapse into one direction ("9 workloads share one cause: secret
//! `agent-credentials` was not observed"). Codes without such a key group whole: forty
//! containers without probes are one direction with forty subjects, not forty lines.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::code::{DiagCode, Severity};
use crate::diagnose::Diagnosis;
use crate::invariants::InvariantCandidate;

/// One next-action entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Direction {
    /// The severity of the findings behind it (candidates rank as info).
    pub severity: Severity,
    /// The code behind it — `INFRA-DIAG-*` for findings, `INFRA-PROP-*` for a candidate whose
    /// exceptions are worth a look.
    pub code: String,
    /// What the findings state, plus the shared cause when one groups them.
    pub action: String,
    /// Every affected subject, sorted and deduplicated.
    pub subjects: Vec<String>,
}

/// The evidence key that names a finding's *cause* (not its subject), where a code has one.
///
/// The grouping table is part of the code's semantics: two findings of such a code with equal
/// values are one problem in two places.
const fn root_evidence_key(code: DiagCode) -> Option<&'static str> {
    match code {
        DiagCode::MissingReference | DiagCode::MissingOptionalReference => Some("name"),
        DiagCode::PodStuckWaiting => Some("reason"),
        DiagCode::HpaTargetMissing => Some("target_name"),
        _ => None,
    }
}

/// Distills a diagnosis and the invariant candidates into directions: severity-ranked,
/// deduplicated, grouped by root evidence where several findings share it.
#[must_use]
pub fn directions(diagnosis: &Diagnosis, candidates: &[InvariantCandidate]) -> Vec<Direction> {
    // Group key: (code, Some(root value) | None). BTreeMap for canonical order within a code.
    let mut groups: BTreeMap<(DiagCode, Option<String>), Vec<String>> = BTreeMap::new();
    for finding in &diagnosis.findings {
        let root = root_evidence_key(finding.code)
            .and_then(|key| finding.evidence.get(key))
            .cloned();
        groups
            .entry((finding.code, root))
            .or_default()
            .push(finding.subject.clone());
    }

    let mut directions = Vec::new();
    for ((code, root), mut subjects) in groups {
        subjects.sort();
        subjects.dedup();
        let action = match &root {
            Some(value) if subjects.len() > 1 => format!(
                "{} — {} subjects share one cause: `{value}`",
                code.meaning(),
                subjects.len()
            ),
            Some(value) => format!("{} (`{value}`)", code.meaning()),
            None => code.meaning().to_owned(),
        };
        directions.push(Direction {
            severity: code.severity(),
            code: code.as_str().to_owned(),
            action,
            subjects,
        });
    }

    // A candidate with exceptions is a direction to look at the exceptions — stated as the
    // uniformity fact it is, not as a violation.
    for candidate in candidates {
        if candidate.exceptions.is_empty() {
            continue;
        }
        let mut subjects: Vec<String> = candidate
            .exceptions
            .iter()
            .map(|exception| exception.subject.clone())
            .collect();
        subjects.sort();
        subjects.dedup();
        directions.push(Direction {
            severity: Severity::Info,
            code: candidate.code.as_str().to_owned(),
            action: format!(
                "{} holds for {} of {}; {} exception(s) break the uniformity",
                candidate.statement,
                candidate.holds_for,
                candidate.population,
                candidate.exceptions.len()
            ),
            subjects,
        });
    }

    // Highest severity first; within a severity, code order, then action — total, so two runs
    // render byte-identically.
    directions.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.action.cmp(&right.action))
    });
    directions
}

/// The directions as pretty JSON with a trailing newline — canonical, byte-equal across runs.
#[must_use]
pub fn directions_to_json(directions: &[Direction]) -> String {
    let mut rendered = serde_json::to_string_pretty(directions)
        .expect("a direction has no non-serializable state");
    rendered.push('\n');
    rendered
}

/// The directions as text: one headline per direction, subjects indented under it.
#[must_use]
pub fn directions_to_text(directions: &[Direction]) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    for direction in directions {
        let _ = writeln!(
            out,
            "{} {} {}",
            direction.severity, direction.code, direction.action
        );
        for subject in &direction.subjects {
            let _ = writeln!(out, "  -> {subject}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnose::Finding;

    fn finding(code: DiagCode, subject: &str, evidence: &[(&str, &str)]) -> Finding {
        Finding {
            code,
            severity: code.severity(),
            subject: subject.to_owned(),
            site: None,
            message: String::new(),
            evidence: evidence
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn findings_sharing_a_root_evidence_value_collapse_into_one_direction() {
        let diagnosis = Diagnosis {
            findings: vec![
                finding(
                    DiagCode::MissingReference,
                    "workloads/app/deployment/a",
                    &[("name", "creds"), ("kind", "secret")],
                ),
                finding(
                    DiagCode::MissingReference,
                    "workloads/app/deployment/b",
                    &[("name", "creds"), ("kind", "secret")],
                ),
                finding(
                    DiagCode::MissingReference,
                    "workloads/app/deployment/c",
                    &[("name", "other"), ("kind", "secret")],
                ),
            ],
        };
        let produced = directions(&diagnosis, &[]);
        assert_eq!(produced.len(), 2, "two causes, two directions");
        let shared = produced
            .iter()
            .find(|direction| direction.action.contains("`creds`"))
            .expect("the shared cause has a direction");
        assert!(
            shared
                .action
                .contains("2 subjects share one cause: `creds`"),
            "the shared cause is named: {}",
            shared.action
        );
        assert_eq!(shared.subjects.len(), 2);
        assert!(
            produced
                .iter()
                .any(|direction| direction.action.contains("`other`")),
            "the lone cause keeps its own direction"
        );
    }

    #[test]
    fn directions_rank_errors_above_warnings_above_info() {
        let diagnosis = Diagnosis {
            findings: vec![
                finding(DiagCode::SingleReplica, "workloads/app/deployment/a", &[]),
                finding(DiagCode::NoProbes, "workloads/app/deployment/a", &[]),
                finding(
                    DiagCode::PodStuckWaiting,
                    "pods/app/a-1",
                    &[("reason", "ImagePullBackOff")],
                ),
            ],
        };
        let ranked = directions(&diagnosis, &[]);
        let severities: Vec<Severity> = ranked.iter().map(|entry| entry.severity).collect();
        assert_eq!(
            severities,
            vec![Severity::Error, Severity::Warning, Severity::Info],
            "highest first"
        );
    }

    #[test]
    fn a_clean_candidate_produces_no_direction_and_an_excepted_one_states_its_counts() {
        use crate::invariants::{Exception, InvariantCandidate, PropCode};

        let clean = InvariantCandidate {
            code: PropCode::UniformRegistry,
            statement: "all images pull from registry `r`".to_owned(),
            holds_for: 3,
            population: 3,
            evidence: BTreeMap::new(),
            exceptions: Vec::new(),
        };
        let excepted = InvariantCandidate {
            code: PropCode::UniformResourceBounds,
            statement: "every container declares resource requests and limits".to_owned(),
            holds_for: 2,
            population: 3,
            evidence: BTreeMap::new(),
            exceptions: vec![Exception {
                subject: "workloads/app/deployment/a".to_owned(),
                detail: "containers[main] declares no limits".to_owned(),
            }],
        };
        let empty = Diagnosis {
            findings: Vec::new(),
        };
        assert!(
            directions(&empty, &[clean]).is_empty(),
            "a uniformity nothing breaks needs no action"
        );
        let produced = directions(&empty, &[excepted]);
        assert_eq!(produced.len(), 1);
        assert!(
            produced[0].action.contains("holds for 2 of 3"),
            "the direction restates the fact, prescribing nothing: {}",
            produced[0].action
        );
    }
}
