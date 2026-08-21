// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! The `gatepass` system, v1: its components assembled, its bindings wired, and its one transport.
//!
//! The transport is derived from the specification, not chosen: `at_least_once` is the only
//! delivery guarantee the model declares, so published events land on an append-only log and a
//! pump delivers each to every binding that reacts to it. The log is the system's observable
//! record, and so is the record of what each binding invoked. What no specification determines
//! — how an escalation event is filled, behaviour behind the ports — stays an obligation; see
//! the `PLAN.md` beside this workspace.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// An event on the system's log: everything any component publishes, and everything a binding
/// escalates into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemEvent {
    /// `gatepass.visit.VisitRegistered`.
    VisitRegistered(gatepass_types::visit::VisitRegistered),
    /// `gatepass.visit.VisitorAdmitted`.
    VisitorAdmitted(gatepass_types::visit::VisitorAdmitted),
    /// `gatepass.visit.VisitorDeparted`.
    VisitorDeparted(gatepass_types::visit::VisitorDeparted),
}

impl From<pass_service::PublishedEvent> for SystemEvent {
    fn from(event: pass_service::PublishedEvent) -> Self {
        match event {
            pass_service::PublishedEvent::VisitRegistered(event) => Self::VisitRegistered(event),
            pass_service::PublishedEvent::VisitorAdmitted(event) => Self::VisitorAdmitted(event),
            pass_service::PublishedEvent::VisitorDeparted(event) => Self::VisitorDeparted(event),
        }
    }
}

/// The `gatepass` system: every component behind its port, and the transport between them.
///
/// The component fields are public because commands enter the system through a component's own
/// port; the log and its delivery cursor are not, because publishing happens by pumping, not by
/// writing history directly.
pub struct System<PassServiceBehaviors> {
    /// The `pass-service` component.
    pub pass_service: pass_service::PassService<PassServiceBehaviors>,
    published: Vec<SystemEvent>,
    cursor: usize,
}

impl<PassServiceBehaviors> System<PassServiceBehaviors> {
    /// Assembles the system from its components.
    pub fn new(pass_service: pass_service::PassService<PassServiceBehaviors>) -> Self {
        Self {
            pass_service,
            published: Vec::new(),
            cursor: 0,
        }
    }

    /// Everything published so far, in publication order — the system's observable record.
    pub fn published(&self) -> &[SystemEvent] {
        &self.published
    }
}

impl<PassServiceBehaviors> System<PassServiceBehaviors>
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    /// Delivers until quiescent: collects every component's outbox onto the log, then delivers
    /// each logged event to every binding that reacts to it — at least once each, which is the
    /// guarantee the specification declares.
    ///
    /// `Err` carries the first unmet obligation that delivery could not route around; the log
    /// keeps everything already published. A specification whose bindings feed each other
    /// without end will not quiesce, and this pump will not pretend otherwise.
    pub fn pump(&mut self) -> Result<(), gatepass_types::obligation::UnmetObligation> {
        loop {
            self.collect();
            if self.cursor == self.published.len() {
                return Ok(());
            }
            self.cursor += 1;
        }
    }

    /// Moves every component's outbox onto the log, in component order.
    fn collect(&mut self) {
        for event in self.pass_service.drain_outbox() {
            self.published.push(SystemEvent::from(event));
        }
    }
}
