// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! pass-service — the `pass-service` component of `gatepass` v1.
//!
//! Holds every expected, present and departed visit for one site.
//!
//! The component's outer surface exactly as the specification declares it: accepted commands as
//! handlers, declared views as queries, published events as a typed outbox. The behaviour behind
//! every handler is an implementation obligation — see the `PLAN.md` beside this workspace — and
//! until one is satisfied, its stub answers with a typed refusal naming what is owed.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// An event this component declares it publishes, on its way to the system's transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedEvent {
    /// `gatepass.visit.VisitRegistered`.
    VisitRegistered(gatepass_types::visit::VisitRegistered),
    /// `gatepass.visit.VisitorAdmitted`.
    VisitorAdmitted(gatepass_types::visit::VisitorAdmitted),
    /// `gatepass.visit.VisitorDeparted`.
    VisitorDeparted(gatepass_types::visit::VisitorDeparted),
}

/// pass-service — the port over the component's obligations.
///
/// `B` bundles every behaviour and query this component owes; constructing it over the domain's
/// `obligations::Unimplemented` yields a component that compiles and refuses, in the type system,
/// everything not yet implemented.
pub struct PassService<B> {
    behaviors: B,
    outbox: Vec<PublishedEvent>,
}

impl<B> PassService<B> {
    /// A new port over the given obligation implementations.
    pub fn new(behaviors: B) -> Self {
        Self {
            behaviors,
            outbox: Vec::new(),
        }
    }

    /// Hands over everything published since the last drain, in publication order.
    ///
    /// The system's transport calls this; anything else reading it is taking events the transport
    /// will then never deliver.
    pub fn drain_outbox(&mut self) -> Vec<PublishedEvent> {
        core::mem::take(&mut self.outbox)
    }
}

impl<B> PassService<B>
where
    B: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    /// Accepts `gatepass.visit.AdmitVisitor`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn admit_visitor(&mut self, input: gatepass_types::visit::AdmitVisitor) -> Result<gatepass_types::visit::AdmitVisitorOutcome, gatepass_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.admit_visitor(input)?;
        match &outcome {
            gatepass_types::visit::AdmitVisitorOutcome::Admitted { visitor_admitted, .. } => {
                self.outbox.push(PublishedEvent::VisitorAdmitted(visitor_admitted.clone()));
            }
            gatepass_types::visit::AdmitVisitorOutcome::WrongState { .. } => {}
        }
        Ok(outcome)
    }

    /// Accepts `gatepass.visit.RegisterVisit`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn register_visit(&mut self, input: gatepass_types::visit::RegisterVisit) -> Result<gatepass_types::visit::RegisterVisitOutcome, gatepass_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.register_visit(input)?;
        match &outcome {
            gatepass_types::visit::RegisterVisitOutcome::Registered { visit_registered, .. } => {
                self.outbox.push(PublishedEvent::VisitRegistered(visit_registered.clone()));
            }
            gatepass_types::visit::RegisterVisitOutcome::Refused { .. } => {}
        }
        Ok(outcome)
    }

    /// Accepts `gatepass.visit.SignOutVisitor`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn sign_out_visitor(&mut self, input: gatepass_types::visit::SignOutVisitor) -> Result<gatepass_types::visit::SignOutVisitorOutcome, gatepass_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.sign_out_visitor(input)?;
        match &outcome {
            gatepass_types::visit::SignOutVisitorOutcome::SignedOut { visitor_departed, .. } => {
                self.outbox.push(PublishedEvent::VisitorDeparted(visitor_departed.clone()));
            }
            gatepass_types::visit::SignOutVisitorOutcome::WrongState { .. } => {}
        }
        Ok(outcome)
    }

    /// Serves `gatepass.visit.ExpectedVisits` at `read_your_writes` consistency, from the owed projection.
    pub fn expected_visits(&self) -> Result<Vec<gatepass_types::visit::ExpectedVisits>, gatepass_types::obligation::UnmetObligation> {
        self.behaviors.expected_visits()
    }

    /// Serves `gatepass.visit.VisitById` at `eventual` consistency, from the owed projection.
    pub fn visit_by_id(&self) -> Result<Vec<gatepass_types::visit::VisitById>, gatepass_types::obligation::UnmetObligation> {
        self.behaviors.visit_by_id()
    }
}
