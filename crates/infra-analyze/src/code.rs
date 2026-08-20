//! The diagnosis codes: stable, registered, each with its severity.
//!
//! The `validation_codes!`/`infra_codes!` idiom a third time, with one addition: a finding's
//! severity is part of the code's registration, not of the call site that produces it. A code
//! that could arrive as `error` from one rule and `info` from another would make
//! `--min-severity` mean different things on different lines, so severity is a function of the
//! code — where one class of fact genuinely splits by seriousness (a required reference that is
//! absent versus an optional one), the split is two codes.

use std::fmt;

/// How serious a finding is.
///
/// Ordered so `Info < Warning < Error`, which is what makes a `--min-severity` threshold a
/// single comparison instead of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Worth knowing, not wrong: a single replica, an object nothing references.
    Info,
    /// Wrong in a way that degrades or will degrade: no probes, no resource bounds.
    Warning,
    /// Broken now: a required reference that is absent, a container that cannot start.
    Error,
}

impl Severity {
    /// Every severity, lowest first.
    pub const ALL: [Self; 3] = [Self::Info, Self::Warning, Self::Error];

    /// The lowercase word output uses.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Declares every diagnosis code once: wire string, severity and meaning from the same line as
/// the variant, so none of the three can drift from the others.
macro_rules! diag_codes {
    ($( $(#[$attribute:meta])* $variant:ident => $wire:literal, $severity:ident, $meaning:literal; )*) => {
        /// Stable machine-readable classification of a diagnosis finding.
        ///
        /// Codes are part of the public interface: harnesses and tests match on them rather
        /// than on message text.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(into = "String")]
        #[non_exhaustive]
        pub enum DiagCode {
            $( $(#[$attribute])* $variant, )*
        }

        impl DiagCode {
            /// Every code this build can produce, in declaration order.
            ///
            /// Generated, so it cannot fall behind the enum — and `tests/diagnosis.rs` insists
            /// every entry fires on the committed example observation, so a rule cannot be
            /// registered without being load-bearing somewhere.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// The code as it appears in output, such as `INFRA-DIAG-004`.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )*
                }
            }

            /// The severity every finding under this code carries.
            pub const fn severity(self) -> Severity {
                match self {
                    $( Self::$variant => Severity::$severity, )*
                }
            }

            /// One line saying what a finding under this code means.
            pub const fn meaning(self) -> &'static str {
                match self {
                    $( Self::$variant => $meaning, )*
                }
            }
        }
    };
}

diag_codes! {
    /// A service's selector matches no observed workload's pod template.
    DanglingSelector => "INFRA-DIAG-001", Warning,
        "a service selector matches nothing; traffic to the service goes nowhere";

    /// A required reference — a non-optional configmap/secret ref, a service account, a claim,
    /// a service, a node, a namespace — names something the cluster does not hold.
    MissingReference => "INFRA-DIAG-002", Error,
        "a required reference names something that was not observed";

    /// An optional configmap/secret reference names something absent — legal, and worth
    /// knowing, because an optional ref that never resolves is usually a leftover.
    MissingOptionalReference => "INFRA-DIAG-003", Info,
        "an optional reference names something that was not observed";

    /// A container declares no resource requests, no limits, or neither.
    NoResourceBounds => "INFRA-DIAG-004", Warning,
        "a container runs without resource requests or limits; the scheduler is flying blind";

    /// A container declares no liveness probe, no readiness probe, or neither.
    NoProbes => "INFRA-DIAG-005", Warning,
        "a container has no liveness or readiness probe; the cluster cannot tell healthy from wedged";

    /// A container image is `:latest`, untagged, and not pinned by digest.
    UnpinnedImage => "INFRA-DIAG-006", Warning,
        "an image without a fixed tag or digest; two pods of one workload can run different code";

    /// A workload wants exactly one replica.
    SingleReplica => "INFRA-DIAG-007", Info,
        "one replica: any disruption is an outage";

    /// A container is stuck waiting — `CrashLoopBackOff`, `ImagePullBackOff` and their kind.
    PodStuckWaiting => "INFRA-DIAG-008", Error,
        "a container waits with a non-transient reason; it is not going to start by itself";

    /// A container has restarted at least five times.
    HighRestartCount => "INFRA-DIAG-009", Warning,
        "a container restarts repeatedly; something inside it keeps dying";

    /// A pod its workload expects ready is not ready.
    PodNotReady => "INFRA-DIAG-010", Warning,
        "a controller-managed pod is not ready and not done";

    /// A configmap or secret no modelled site references.
    OrphanedConfig => "INFRA-DIAG-011", Info,
        "a configmap or secret referenced by no env, envFrom or volume site";

    /// A persistent volume claim whose observed phase is `Pending` or `Lost`.
    UnboundClaim => "INFRA-DIAG-012", Warning,
        "a claim that is not bound to a volume; pods mounting it cannot start";

    /// A persistent volume claim no modelled workload volume references.
    OrphanedClaim => "INFRA-DIAG-013", Info,
        "a claim referenced by no workload volume";

    /// Two or more services select exactly the same set of workloads.
    DuplicateSelectors => "INFRA-DIAG-014", Info,
        "two services target the same workload set; often deliberate, always worth knowing";
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<DiagCode> for String {
    fn from(code: DiagCode) -> Self {
        code.as_str().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_renders_in_the_diag_namespace_and_the_generated_list_holds_them_all() {
        assert_eq!(DiagCode::ALL.len(), 14, "the catalogue is fourteen codes");
        for code in DiagCode::ALL {
            assert!(
                code.as_str().starts_with("INFRA-DIAG-"),
                "{code:?} renders as {} — outside the INFRA-DIAG- namespace",
                code.as_str()
            );
        }
    }

    #[test]
    fn wire_strings_are_unique_because_two_rules_sharing_one_code_are_indistinguishable_downstream()
    {
        let mut seen = std::collections::BTreeSet::new();
        for code in DiagCode::ALL {
            assert!(
                seen.insert(code.as_str()),
                "{} appears twice in the catalogue",
                code.as_str()
            );
        }
    }

    #[test]
    fn severity_orders_info_below_warning_below_error() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn the_required_and_optional_reference_codes_disagree_in_severity_by_design() {
        // The distinction the taxonomy exists for: an absent required ref breaks a pod, an
        // absent optional ref is a fact.
        assert_eq!(DiagCode::MissingReference.severity(), Severity::Error);
        assert_eq!(
            DiagCode::MissingOptionalReference.severity(),
            Severity::Info
        );
    }
}
