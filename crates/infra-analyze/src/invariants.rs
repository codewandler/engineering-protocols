//! Invariant candidates: the uniformity a cluster *almost* keeps, stated with its exceptions.
//!
//! A diagnosis finding says "this object is wrong". An invariant candidate says something
//! different: "all but *k* of the population do X" — a fact about uniformity, mined so IW3 can
//! offer it for promotion into a declared invariant. **A candidate with exceptions is not a
//! rule violation**: nobody declared the rule yet. The exceptions are evidence riding on the
//! candidate, rendered as "except: …", never as findings against the exceptional objects —
//! the object-level judgement, where one exists, is already a coded diagnosis
//! (`INFRA-DIAG-004`, `-016`).
//!
//! # When a candidate exists
//!
//! Each rule emits its candidate only when a **strict majority** of its population conforms:
//! "all images pull from registry R (except: 60 % of them)" is not an observation of
//! uniformity, it is noise wearing its clothes. The threshold is structural, not a flag —
//! two runs of one build cannot disagree about what "almost all" means. And a population that
//! was not observed emits nothing: the coverage candidate stays silent on a bundle that did
//! not scan disruption budgets, because unobserved is not uncovered.

use std::collections::BTreeMap;
use std::fmt;

use infra_compiler::InfraIr;
use serde::Serialize;

use crate::diagnose::pdb_covers;
use crate::properties::parse_image;

/// Declares every invariant-candidate code once — the `diag_codes!` idiom, fourth instance:
/// wire string and meaning bound to the variant on one line each.
macro_rules! prop_codes {
    ($( $(#[$attribute:meta])* $variant:ident => $wire:literal, $meaning:literal; )*) => {
        /// Stable machine-readable classification of an invariant candidate.
        ///
        /// Codes are part of the public interface: harnesses and tests match on them rather
        /// than on statement text.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(into = "String")]
        #[non_exhaustive]
        pub enum PropCode {
            $( $(#[$attribute])* $variant, )*
        }

        impl PropCode {
            /// Every code this build can produce, in declaration order.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// The code as it appears in output, such as `INFRA-PROP-001`.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )*
                }
            }

            /// One line saying what the candidate claims when it has no exceptions.
            pub const fn meaning(self) -> &'static str {
                match self {
                    $( Self::$variant => $meaning, )*
                }
            }
        }
    };
}

prop_codes! {
    /// Most images pull from one registry.
    UniformRegistry => "INFRA-PROP-001",
        "all images pull from one registry";

    /// Most multi-replica workloads are covered by a disruption budget.
    UniformPdbCoverage => "INFRA-PROP-002",
        "every multi-replica workload has a disruption budget";

    /// Most containers declare both resource requests and limits.
    UniformResourceBounds => "INFRA-PROP-003",
        "every container declares resource requests and limits";
}

impl fmt::Display for PropCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<PropCode> for String {
    fn from(code: PropCode) -> Self {
        code.as_str().to_owned()
    }
}

/// One member of the population that does not conform — evidence on the candidate, not a
/// finding against the member.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Exception {
    /// The nonconforming object, as an IR path: `workloads/shop/deployment/queue-redis`.
    pub subject: String,
    /// What it does instead, one line: ``image `redis:7-alpine` pulls from the default registry``.
    pub detail: String,
}

/// One invariant candidate: a uniformity most of a population keeps, with its exceptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvariantCandidate {
    /// The stable code a harness matches on.
    pub code: PropCode,
    /// The uniformity claim, concrete: ``all images pull from registry `localhost:31721` ``.
    pub statement: String,
    /// How many of the population conform.
    pub holds_for: u32,
    /// The population size the claim was judged over.
    pub population: u32,
    /// The named facts the candidate rests on, beyond the counts.
    pub evidence: BTreeMap<String, String>,
    /// Every nonconforming member, sorted.
    pub exceptions: Vec<Exception>,
}

/// Mines every invariant candidate out of one IR, in code order.
#[must_use]
pub fn candidates(ir: &InfraIr) -> Vec<InvariantCandidate> {
    let mut found = Vec::new();
    candidate_uniform_registry(ir, &mut found);
    candidate_uniform_pdb_coverage(ir, &mut found);
    candidate_uniform_resource_bounds(ir, &mut found);
    found
}

/// Whether conformance is a strict majority — the structural bar for calling it uniformity.
fn majority(holds_for: usize, population: usize) -> bool {
    holds_for * 2 > population
}

/// `INFRA-PROP-001` — most images pull from one registry.
///
/// The dominant registry is the one most images name; a tie breaks to the lexicographically
/// smallest so two runs cannot disagree. Images without a registry count as pulling from the
/// default registry, spelled `(default)` — a real place images come from, not a gap.
fn candidate_uniform_registry(ir: &InfraIr, found: &mut Vec<InvariantCandidate>) {
    let mut by_registry: BTreeMap<String, Vec<Exception>> = BTreeMap::new();
    for (key, workload) in &ir.model.workloads {
        for container in &workload.containers {
            let registry = parse_image(&container.image)
                .registry
                .unwrap_or_else(|| "(default)".to_owned());
            by_registry.entry(registry).or_default().push(Exception {
                subject: format!("workloads/{key}"),
                detail: format!("containers[{}] image `{}`", container.name, container.image),
            });
        }
    }
    let population: usize = by_registry.values().map(Vec::len).sum();
    // BTreeMap order makes the first maximum the lexicographically smallest.
    let Some((dominant, conforming)) = by_registry
        .iter()
        .max_by(|left, right| left.1.len().cmp(&right.1.len()).then(right.0.cmp(left.0)))
    else {
        return;
    };
    if !majority(conforming.len(), population) {
        return;
    }
    let mut exceptions: Vec<Exception> = by_registry
        .iter()
        .filter(|(registry, _)| *registry != dominant)
        .flat_map(|(_, members)| members.iter().cloned())
        .collect();
    exceptions.sort();
    found.push(InvariantCandidate {
        code: PropCode::UniformRegistry,
        statement: format!("all images pull from registry `{dominant}`"),
        holds_for: u32::try_from(conforming.len()).unwrap_or(u32::MAX),
        population: u32::try_from(population).unwrap_or(u32::MAX),
        evidence: BTreeMap::from([("registry".to_owned(), dominant.clone())]),
        exceptions,
    });
}

/// `INFRA-PROP-002` — most multi-replica workloads are covered by a disruption budget.
///
/// Silent when the bundle did not scan budgets or holds no multi-replica workload: a candidate
/// about a population nobody observed would be manufactured uniformity.
fn candidate_uniform_pdb_coverage(ir: &InfraIr, found: &mut Vec<InvariantCandidate>) {
    let Some(budgets) = &ir.model.pod_disruption_budgets else {
        return;
    };
    let mut holds_for = 0u32;
    let mut population = 0u32;
    let mut exceptions = Vec::new();
    for (key, workload) in &ir.model.workloads {
        let Some(replicas) = workload.replicas else {
            continue;
        };
        if replicas < 2 {
            continue;
        }
        population += 1;
        let covered = budgets.values().any(|budget| {
            budget.identity.namespace == workload.identity.namespace
                && pdb_covers(&budget.selector, &workload.template_labels)
        });
        if covered {
            holds_for += 1;
        } else {
            exceptions.push(Exception {
                subject: format!("workloads/{key}"),
                detail: format!("{replicas} replicas, no covering budget"),
            });
        }
    }
    if population == 0 || !majority(holds_for as usize, population as usize) {
        return;
    }
    exceptions.sort();
    found.push(InvariantCandidate {
        code: PropCode::UniformPdbCoverage,
        statement: "every multi-replica workload has a disruption budget".to_owned(),
        holds_for,
        population,
        evidence: BTreeMap::new(),
        exceptions,
    });
}

/// `INFRA-PROP-003` — most containers declare both requests and limits.
fn candidate_uniform_resource_bounds(ir: &InfraIr, found: &mut Vec<InvariantCandidate>) {
    let mut holds_for = 0u32;
    let mut population = 0u32;
    let mut exceptions = Vec::new();
    for (key, workload) in &ir.model.workloads {
        for container in &workload.containers {
            population += 1;
            let mut missing = Vec::new();
            if container.resources.requests.is_empty() {
                missing.push("requests");
            }
            if container.resources.limits.is_empty() {
                missing.push("limits");
            }
            if missing.is_empty() {
                holds_for += 1;
            } else {
                exceptions.push(Exception {
                    subject: format!("workloads/{key}"),
                    detail: format!(
                        "containers[{}] declares no {}",
                        container.name,
                        missing.join(" and ")
                    ),
                });
            }
        }
    }
    if population == 0 || !majority(holds_for as usize, population as usize) {
        return;
    }
    exceptions.sort();
    found.push(InvariantCandidate {
        code: PropCode::UniformResourceBounds,
        statement: "every container declares resource requests and limits".to_owned(),
        holds_for,
        population,
        evidence: BTreeMap::new(),
        exceptions,
    });
}

/// The candidates as pretty JSON with a trailing newline — the canonical rendering, byte-equal
/// across runs.
#[must_use]
pub fn candidates_to_json(candidates: &[InvariantCandidate]) -> String {
    let mut rendered = serde_json::to_string_pretty(candidates)
        .expect("an invariant candidate has no non-serializable state");
    rendered.push('\n');
    rendered
}

/// The candidates as text, one block per candidate: the statement with its counts, then one
/// indented line per exception.
#[must_use]
pub fn candidates_to_text(candidates: &[InvariantCandidate]) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    for candidate in candidates {
        let _ = writeln!(
            out,
            "{} {} — holds for {} of {}{}",
            candidate.code,
            candidate.statement,
            candidate.holds_for,
            candidate.population,
            if candidate.exceptions.is_empty() {
                ""
            } else {
                "; except:"
            }
        );
        for exception in &candidate.exceptions {
            let _ = writeln!(out, "  {} — {}", exception.subject, exception.detail);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_renders_in_the_prop_namespace_and_wire_strings_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for code in PropCode::ALL {
            assert!(
                code.as_str().starts_with("INFRA-PROP-"),
                "{code:?} renders as {} — outside the INFRA-PROP- namespace",
                code.as_str()
            );
            assert!(
                seen.insert(code.as_str()),
                "{} is declared for two variants",
                code.as_str()
            );
        }
        assert_eq!(PropCode::ALL.len(), 3, "the catalogue is three candidates");
    }

    #[test]
    fn a_minority_is_not_a_majority_and_a_bare_half_is_not_either() {
        assert!(majority(2, 3));
        assert!(!majority(1, 2), "half is not a strict majority");
        assert!(!majority(0, 0), "an empty population holds nothing");
    }
}
