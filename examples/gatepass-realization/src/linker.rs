//! The linker: generated components and hand implementations, assembled — and never chosen among.
//!
//! Gap register D-2, taken as written, and the same discipline
//! [`billing_realization`](../../billing-realization/src/linker.rs) holds: the linker does not
//! choose. Zero implementations offered for an obligation is an **unsatisfied obligation**; two is
//! an **ambiguity error naming both**. Selection among alternatives is `Realization` material and
//! stays proposed with it, so there is deliberately no priority, no default and no "first wins".
//!
//! Errors accumulate (invariant 3): a linker with three empty slots reports three unsatisfied
//! obligations, not the first one it happened to walk.
//!
//! The obligation list here is [`OBLIGATIONS`], spelled as the generated stubs spell it, and a test
//! below holds it equal to what `generated/rust/gatepass/plan.json` owes — so this module cannot
//! quietly keep linking a plan that has moved.

use std::fmt;

use gatepass_types::obligation::UnmetObligation;
use gatepass_types::visit::obligations::{
    AdmitVisitorBehavior, ExpectedVisitsQuery, RegisterVisitBehavior, SignOutVisitorBehavior,
    VisitByIdQuery,
};
use gatepass_types::visit::{
    AdmitVisitor, AdmitVisitorOutcome, ExpectedVisits, RegisterVisit, RegisterVisitOutcome,
    SignOutVisitor, SignOutVisitorOutcome, VisitById,
};

use crate::visit::{SharedVisits, VisitRealization};

/// Every obligation the gatepass plan owes, as `(capability, source)` in the stubs' own spelling.
///
/// Held equal to `generated/rust/gatepass/plan.json` by
/// `the_linkers_obligation_list_is_exactly_the_plans`, so a specification change that moves an
/// obligation fails here instead of leaving the linker resolving a list that no longer exists.
pub const OBLIGATIONS: &[(&str, &str)] = &[
    ("command behaviour", "gatepass.visit.AdmitVisitor"),
    ("command behaviour", "gatepass.visit.RegisterVisit"),
    ("command behaviour", "gatepass.visit.SignOutVisitor"),
    ("view query", "gatepass.visit.ExpectedVisits"),
    ("view query", "gatepass.visit.VisitById"),
];

/// How the honest offers name themselves in an ambiguity error.
pub const HONEST: &str = "gatepass-realization/honest";

/// Why one obligation could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Nothing was offered for an obligation the plan owes.
    Unsatisfied {
        /// The capability kind, as the plan spells it.
        capability: &'static str,
        /// The construct that requires it, in the specification's own spelling.
        source: &'static str,
    },
    /// More than one implementation claims one obligation — named in full, chosen among never.
    Ambiguous {
        /// The capability kind, as the plan spells it.
        capability: &'static str,
        /// The construct that requires it, in the specification's own spelling.
        source: &'static str,
        /// Every claimant, in the order offered.
        offered: Vec<&'static str>,
    },
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsatisfied { capability, source } => write!(
                f,
                "nothing implements the {capability} `{source}`, which the plan owes"
            ),
            Self::Ambiguous {
                capability,
                source,
                offered,
            } => write!(
                f,
                "{} implementations claim the {capability} `{source}` — {} — and this linker does \
                 not choose between them",
                offered.len(),
                offered.join(", ")
            ),
        }
    }
}

/// Every refusal one linking produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkErrors(Vec<LinkError>);

impl LinkErrors {
    /// The refusals, in the order the obligations are listed.
    pub fn as_slice(&self) -> &[LinkError] {
        &self.0
    }
}

impl fmt::Display for LinkErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, error) in self.0.iter().enumerate() {
            if position > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

/// One offer: an implementation, and who is offering it.
///
/// The name is not decoration. It is what an ambiguity error carries, and an error that says "two
/// implementations" without saying which two is an error nobody can act on.
struct Offer {
    /// Who offered it.
    by: &'static str,
}

/// What is on offer for each obligation, before anything is resolved.
#[derive(Default)]
struct Offers {
    /// Per obligation, in [`OBLIGATIONS`] order, who claims it.
    claims: Vec<Vec<Offer>>,
}

impl Offers {
    /// An empty offer sheet, one slot per obligation the plan owes.
    fn new() -> Self {
        Self {
            claims: OBLIGATIONS.iter().map(|_| Vec::new()).collect(),
        }
    }

    /// Records one claim.
    fn offer(&mut self, capability: &str, source: &str, by: &'static str) {
        let position = OBLIGATIONS
            .iter()
            .position(|(kind, named)| *kind == capability && *named == source)
            .unwrap_or_else(|| {
                panic!(
                    "`{capability}` `{source}` is not an obligation this plan owes; the linker's \
                     list and the plan have diverged"
                )
            });
        self.claims[position].push(Offer { by });
    }

    /// Resolves every obligation, accumulating what could not be resolved.
    fn resolve(self) -> Result<(), LinkErrors> {
        let mut errors = Vec::new();
        for (position, (capability, source)) in OBLIGATIONS.iter().enumerate() {
            match self.claims[position].len() {
                1 => {}
                0 => errors.push(LinkError::Unsatisfied { capability, source }),
                _ => errors.push(LinkError::Ambiguous {
                    capability,
                    source,
                    offered: self.claims[position].iter().map(|offer| offer.by).collect(),
                }),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(LinkErrors(errors))
        }
    }
}

/// Everything one linking produced: the system, and the store it answers over.
pub struct Assembled {
    /// The system every command enters through.
    pub system: gatepass_system::System<Behaviors>,
    /// The store the realization answers over, for a caller that wants to look.
    pub visits: SharedVisits,
}

/// The bundle the generated port takes: one value satisfying every obligation the component owes.
///
/// A struct rather than the realization itself, because what the port asks for is the *set* of
/// obligations and what the linker resolves is each one separately. Every method below delegates,
/// and delegation is the whole body — a decision taken here would be a decision the linker made.
#[derive(Clone, Debug)]
pub struct Behaviors {
    visit: VisitRealization,
}

impl RegisterVisitBehavior for Behaviors {
    fn register_visit(
        &mut self,
        input: RegisterVisit,
    ) -> Result<RegisterVisitOutcome, UnmetObligation> {
        self.visit.register_visit(input)
    }
}

impl AdmitVisitorBehavior for Behaviors {
    fn admit_visitor(
        &mut self,
        input: AdmitVisitor,
    ) -> Result<AdmitVisitorOutcome, UnmetObligation> {
        self.visit.admit_visitor(input)
    }
}

impl SignOutVisitorBehavior for Behaviors {
    fn sign_out_visitor(
        &mut self,
        input: SignOutVisitor,
    ) -> Result<SignOutVisitorOutcome, UnmetObligation> {
        self.visit.sign_out_visitor(input)
    }
}

impl ExpectedVisitsQuery for Behaviors {
    fn expected_visits(&self) -> Result<Vec<ExpectedVisits>, UnmetObligation> {
        self.visit.expected_visits()
    }
}

impl VisitByIdQuery for Behaviors {
    fn visit_by_id(&self) -> Result<Vec<VisitById>, UnmetObligation> {
        self.visit.visit_by_id()
    }
}

/// The honest linkage: exactly one implementation per obligation, resolved rather than assumed.
///
/// # Errors
///
/// [`LinkErrors`] holding one entry per obligation that was not offered exactly once. It cannot
/// happen for the offers this function makes — which is the point of resolving them anyway: the
/// resolution is the mechanism, and a mechanism only exercised when it fails is a mechanism nobody
/// has run.
pub fn link() -> Result<Assembled, LinkErrors> {
    let visits = SharedVisits::new();
    let visit = VisitRealization::over(visits.clone());

    let mut offers = Offers::new();
    for (capability, source) in OBLIGATIONS {
        offers.offer(capability, source, HONEST);
    }
    offers.resolve()?;

    let behaviors = Behaviors { visit };
    Ok(Assembled {
        system: gatepass_system::System::new(pass_service::PassService::new(behaviors)),
        visits,
    })
}

/// The honest linkage, or a panic naming everything unresolved.
///
/// # Panics
///
/// If any obligation is not offered exactly once.
pub fn honest() -> Assembled {
    link().unwrap_or_else(|errors| panic!("the gatepass realization does not link:\n{errors}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_linkers_obligation_list_is_exactly_the_plans() {
        let plan = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../generated/rust/gatepass/plan.json"
        ))
        .expect("the committed plan is beside the workspace it describes");
        let plan: serde_json::Value = serde_json::from_str(&plan).expect("the plan is JSON");
        let mut owed: Vec<(String, String)> = plan["capabilities"]
            .as_array()
            .expect("the plan lists capabilities")
            .iter()
            .filter(|planned| planned["disposition"]["disposition"] == "obligation")
            .map(|planned| {
                (
                    planned["kind"]
                        .as_str()
                        .expect("a capability kind")
                        .replace('_', " "),
                    planned["source"].as_str().expect("a source").to_owned(),
                )
            })
            .collect();
        owed.sort();

        let mut linked: Vec<(String, String)> = OBLIGATIONS
            .iter()
            .map(|(capability, source)| {
                (
                    (*capability).replace("behaviour", "behavior"),
                    (*source).to_owned(),
                )
            })
            .collect();
        linked.sort();

        // The plan spells the kind in snake case and with the American spelling its Rust enum
        // carries; the stubs spell it the way `CapabilityKind::describes` renders it. Normalising
        // both is what keeps this a comparison of the *set* rather than of two spellings.
        let owed: Vec<(String, String)> = owed
            .into_iter()
            .map(|(kind, source)| (kind.replace("behaviour", "behavior"), source))
            .collect();
        assert_eq!(
            linked, owed,
            "the linker resolves a different set of obligations than the committed plan owes"
        );
    }

    #[test]
    fn an_obligation_nobody_offers_is_unsatisfied_rather_than_silently_missing() {
        let mut offers = Offers::new();
        // Every obligation but the first, so the fixture reaches the state the rule is about:
        // exactly one slot is empty and the rest resolve.
        for (capability, source) in OBLIGATIONS.iter().skip(1) {
            offers.offer(capability, source, HONEST);
        }
        let errors = offers.resolve().expect_err("one obligation is unoffered");
        assert_eq!(
            errors.as_slice(),
            &[LinkError::Unsatisfied {
                capability: OBLIGATIONS[0].0,
                source: OBLIGATIONS[0].1,
            }]
        );
    }

    #[test]
    fn two_implementations_of_one_obligation_are_named_rather_than_chosen_between() {
        let mut offers = Offers::new();
        for (capability, source) in OBLIGATIONS {
            offers.offer(capability, source, HONEST);
        }
        offers.offer(OBLIGATIONS[0].0, OBLIGATIONS[0].1, "a-second-realization");
        let errors = offers
            .resolve()
            .expect_err("one obligation has two claimants");
        assert_eq!(
            errors.as_slice(),
            &[LinkError::Ambiguous {
                capability: OBLIGATIONS[0].0,
                source: OBLIGATIONS[0].1,
                offered: vec![HONEST, "a-second-realization"],
            }],
            "gap register D-2: the machinery names both and picks neither"
        );
    }

    #[test]
    fn the_honest_linkage_assembles_a_system() {
        let assembled = link().expect("every obligation is offered exactly once");
        let _ = assembled.system;
        let _ = assembled.visits;
    }
}
