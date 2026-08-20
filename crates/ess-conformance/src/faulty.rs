//! Implementations that are wrong in exactly one way, and where each one is caught.
//!
//! Design §25 and §26. A generated suite that passes against a correct implementation has shown one
//! thing only: that it asks for nothing a correct implementation cannot answer. It has shown nothing
//! at all about whether it would notice a wrong one, and a suite whose failures have never been
//! demonstrated is indistinguishable from a suite that checks nothing.
//!
//! So this module ships the other half of the evidence: a [`Fault`], a [`Faulty`] wrapper that
//! injects one, and — in `tests/faults.rs` — the matrix that asserts, per fault, **which named
//! scenario fails** and **how many unrelated ones still pass**. §25 is explicit that a generic panic
//! failing everything proves nothing, which is why both halves are asserted rather than the first.
//!
//! `aep_conformance::faulty` did this for AEP backends and is the precedent §25 names. What is
//! carried over unchanged is the shape: one `#[non_exhaustive]` enum, an `ALL` constant generated
//! from the same lines as the variants, one designated check per fault, and a blast-radius allowance
//! per fault that has to be updated *with a reason* rather than relaxed.
//!
//! # Three of these are not caught by anything, and that is the finding
//!
//! [`Caught::Nothing`] is not a gap in this module. A fault nobody catches is worth more than
//! another passing row: it says the specification cannot express the property, or that synthesis
//! does not ask for it. All three that turn up here have one root — the suite asserts *that* an
//! event was published and, for a refused transition, that one particular event was not; it never
//! asserts what an event **carried**, and it never asserts that nothing *else* was published.
//!
//! | fault | what a client would see | why nothing fails |
//! |---|---|---|
//! | [`WrongEventPayload`](Fault::WrongEventPayload) | every consumer of `InvoiceCreated` bills the wrong amount | every synthesised event assertion carries an empty payload |
//! | [`ExtraEvent`](Fault::ExtraEvent) | creating an invoice announces that it was cancelled | `ExpectNoEvent` is only synthesised for the one event a refused transition would have published |
//! | [`DropConsistencyToken`](Fault::DropConsistencyToken) | a `read_your_writes` view is never actually held to it | with no token the runner reads at `Current`, and §14's whole claim quietly lapses |
//!
//! They are in [`Fault::ALL`] and the matrix asserts they are *still* uncaught, so the day a later
//! slice closes one of these holes, the row fails and has to be rewritten rather than forgotten.
//!
//! # Boundary or implementation
//!
//! Seven of the ten are [`Injection::Boundary`]: [`Faulty`] perturbs what goes into the target and
//! what comes out of it, and never a broken internal, because that is the same position a real
//! client is in.
//!
//! Two cannot be. [`DropBinding`](Fault::DropBinding) and [`WrongMapping`](Fault::WrongMapping) are
//! defects *of a binding*, and the target interface deliberately does not attribute an observation
//! to the binding that caused it — an [`ObservedEvent`] carries no transport metadata (§41), so a
//! wrapper filtering `oracle.dispatch.HandedOff` out of `observe_events` would silence all three of
//! the oracle's bindings at once, not the one under test. Simulating them in the one observation
//! that *is* attributed — [`ObservedInvocation`] — would prove that the suite catches a **lie about**
//! a mapping while the system still mapped correctly, which is a different and much weaker claim. So
//! those two are injected in [`Oracle`], as
//! [`with_binding_dropped`](Oracle::with_binding_dropped) and
//! [`with_mapping_swapped`](Oracle::with_mapping_swapped), and [`Fault::injection`] says which of
//! the two mechanisms each fault uses so the split is a property of the type rather than a
//! paragraph.
//!
//! # Nothing here is a new source of variation
//!
//! §37 gives the runner the clock and the id source, and a faulty target that reached for either
//! would make the matrix flaky and therefore worthless. Every fault below is a pure function of what
//! it was given, plus — for [`StaleReadYourWrites`](Fault::StaleReadYourWrites) — the previous
//! answer to the same view in the same scenario, which is reset by
//! [`begin_scenario`](ConformanceTarget::begin_scenario).

use std::cell::RefCell;
use std::collections::BTreeMap;

use aep_domain::facts::Number;
use aep_domain::node::Node;

use crate::reference::{
    Billing, Oracle, CANCEL_INVOICE, CREATE_INVOICE, INVOICE_CANCELLED, INVOICE_CREATED,
    INVOICE_ISSUED,
};
use crate::scenario::{EventRef, OutcomeRef, ViewRef};
use crate::target::{
    ConformanceTarget, EventObservationRequest, ExternalOutcomeControl, ImplementationIdentity,
    InvocationObservationRequest, ObservedEvent, ObservedInvocation, RedeliveryRequest,
    ScenarioContext, SemanticCommandRequest, SemanticCommandResult, SemanticViewRequest,
    SemanticViewResult, TargetError,
};

// ---- the vocabulary --------------------------------------------------------------------------

/// Which specification a fault is injected into, and therefore which suite catches it.
///
/// Two, because one of them cannot make §26's second claim. `examples/billing/` declares a single
/// binding, so a binding that stops running fails every binding scenario there is; the oracle
/// fixture declares three, on three events, and its `README.md` says in as many words that this is
/// what it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System {
    /// `examples/billing/`, the normative example.
    Billing,
    /// `examples/oracle-fixture/`, the fixture built for the checks billing cannot make fail.
    Oracle,
}

impl System {
    /// The directory under `examples/` that holds it.
    pub fn directory(self) -> &'static str {
        match self {
            Self::Billing => "billing",
            Self::Oracle => "oracle-fixture",
        }
    }
}

/// Where a fault is injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Injection {
    /// In [`Faulty`], which perturbs only what goes in and what comes out.
    Boundary,
    /// In the reference implementation, because the boundary cannot express it.
    ///
    /// See the [module documentation](self): the two that need this are defects of a *binding*, and
    /// the target interface does not attribute an observation to the binding that produced it.
    Implementation,
}

/// What a suite catches a fault with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caught {
    /// The one scenario that exists to catch it, by id.
    By(&'static str),
    /// Nothing catches it, and why — see the [module documentation](self).
    Nothing(&'static str),
}

impl Caught {
    /// The scenario that catches it, where one does.
    pub fn scenario(self) -> Option<&'static str> {
        match self {
            Self::By(scenario) => Some(scenario),
            Self::Nothing(_) => None,
        }
    }
}

/// Declares one fault per line, and generates the list a matrix walks from the same lines.
///
/// Hand-maintaining `ALL` beside the enum is what `aep_domain`'s `validation_codes!` macro exists to
/// stop, after five codes had fallen out of such a list. The same argument applies with more force
/// here: a fault missing from `ALL` is a row the matrix silently does not run, which is exactly the
/// silent omission this whole slice is about.
macro_rules! faults {
    ($(
        $(#[$attribute:meta])*
        $variant:ident => $written:literal, $system:expr, $injection:expr, $caught:expr, $describe:literal;
    )*) => {
        /// One way an implementation of a specification can be wrong.
        ///
        /// Seven are design §25's fault table. Three more are faults this slice went looking for and
        /// found that **nothing catches** — see the [module documentation](self).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum Fault {
            $( $(#[$attribute])* $variant, )*
        }

        impl Fault {
            /// Every fault the matrix injects, in declaration order.
            ///
            /// Generated, so it cannot fall behind the enum — which is what makes the matrix a
            /// matrix rather than a list somebody has to remember to extend (§25).
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// How it is written, in a report and in the identity a faulty target answers with.
            pub fn written(self) -> &'static str {
                match self { $( Self::$variant => $written, )* }
            }

            /// Which specification it is injected into.
            pub fn system(self) -> System {
                match self { $( Self::$variant => $system, )* }
            }

            /// Whether it is injected at the boundary or in the implementation, and it is the
            /// [module documentation](self) that argues why for the two that are not at the boundary.
            pub fn injection(self) -> Injection {
                match self { $( Self::$variant => $injection, )* }
            }

            /// The scenario that exists to catch it, or why nothing does.
            pub fn caught(self) -> Caught {
                match self { $( Self::$variant => $caught, )* }
            }

            /// What goes wrong, in one line, for a report.
            pub fn describe(self) -> &'static str {
                match self { $( Self::$variant => $describe, )* }
            }
        }
    };
}

faults! {
    /// `F-WRONG-EVENT`: a created invoice is announced as an issued one.
    WrongEvent => "wrong-event", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.CreateInvoice/outcome/accepted"),
        "a branch reports an event it does not declare it emits in place of the one it does";

    /// `F-REJECTION`: an amount the guard refuses is accepted anyway.
    AcceptInvalidAmount => "accept-invalid-amount", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.CreateInvoice/outcome/rejected"),
        "an input that satisfies no branch's guard is accepted by the guarded one";

    /// `F-ILLEGAL-TRANSITION`: a move the lifecycle does not declare is honoured.
    AllowIllegalTransition => "allow-illegal-transition", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.Invoice/state/Paid/refuses/billing.invoice.CancelInvoice"),
        "a command moves an entity from a state its transition does not run from";

    /// `F-DROPPED-BINDING`: an event reaches nothing.
    DropBinding => "drop-binding", System::Oracle, Injection::Implementation,
        Caught::By("handoff-on-placed/binding/flow"),
        "a declared binding never runs, so the consequence it promises never happens";

    /// `F-WRONG-MAPPING`: a binding fills an input from the wrong field of the event.
    WrongMapping => "wrong-mapping", System::Oracle, Injection::Implementation,
        Caught::By("handoff-on-placed/binding/mapping"),
        "a binding maps `alternate_contact` where the specification writes `contact`";

    /// `F-VIEW-RACE`: a read that demands the write it was told about is answered from before it.
    StaleReadYourWrites => "stale-read-your-writes", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.IssueInvoice/outcome/issued"),
        "a read_your_writes view answers from before the command that just returned";

    /// `F-EXTERNAL-OUTCOME`: a forced failure is reported as a success.
    IgnoreExternalOutcome => "ignore-external-outcome", System::Billing, Injection::Boundary,
        Caught::By("billing.email.SendEmail/outcome/failed"),
        "an externally decided failure is ignored and the command reports success";

    /// A published event carries a value the command was never given.
    ///
    /// Not one of §25's seven. It is here because nothing catches it: every event assertion
    /// synthesis writes carries an empty payload, so `InvoiceCreated` may announce any amount at all.
    WrongEventPayload => "wrong-event-payload", System::Billing, Injection::Boundary,
        Caught::Nothing(
            "every synthesised event assertion carries an empty payload, so no scenario compares \
             what an event carried against the input that caused it",
        ),
        "a published event carries an amount the command was never given";

    /// A branch publishes an event it does not declare it emits, beside the ones it does.
    ///
    /// Not one of §25's seven, and uncaught: `ESS-CF-NO-EVENT` names the rule "a branch publishes no
    /// event it does not declare it emits", and synthesis only ever asks it of the single event a
    /// refused transition would have published.
    ExtraEvent => "extra-event", System::Billing, Injection::Boundary,
        Caught::Nothing(
            "`ExpectNoEvent` is synthesised only for the one event a refused transition or a \
             refusing branch would have published, never for the events a branch does not declare",
        ),
        "creating an invoice also announces that it was cancelled";

    /// A command answers without the token a later read would demand.
    ///
    /// Not one of §25's seven, and uncaught: the runner falls back to `Current` when no token is in
    /// hand, so a target opts out of every `read_your_writes` check by returning nothing.
    DropConsistencyToken => "drop-consistency-token", System::Billing, Injection::Boundary,
        Caught::Nothing(
            "with no token the runner reads at `Current`, so §14's read-your-writes demand is never \
             made and the check silently becomes a weaker one",
        ),
        "a command returns no consistency token, so no read is ever held to it";
}

// ---- the wrapper -----------------------------------------------------------------------------

/// A target wrapped so that exactly one property fails to hold.
///
/// It perturbs what goes in and what comes out, and never an internal — see the
/// [module documentation](self) for why, and for the two faults that cannot be injected this way.
#[derive(Debug)]
pub struct Faulty<T> {
    inner: T,
    fault: Fault,
    memory: RefCell<Vec<(ViewRef, SemanticViewResult)>>,
}

impl<T> Faulty<T> {
    /// Wraps `inner` so that `fault` is injected.
    pub fn new(inner: T, fault: Fault) -> Self {
        Self {
            inner,
            fault,
            memory: RefCell::new(Vec::new()),
        }
    }

    /// Which fault this target carries.
    pub fn fault(&self) -> Fault {
        self.fault
    }

    /// The target underneath.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Records the fresh answer to `view` and returns the one before it.
    ///
    /// A projection exactly one read behind, which is what "stale" means here: not empty forever,
    /// but always answering the question before the one that was asked. Deterministic, and reset per
    /// scenario, so two runs of the matrix agree.
    fn one_read_behind(&self, view: &ViewRef, fresh: SemanticViewResult) -> SemanticViewResult {
        let mut memory = self.memory.borrow_mut();
        if let Some((_, previous)) = memory.iter_mut().find(|(known, _)| known == view) {
            return std::mem::replace(previous, fresh);
        }
        // The first read of a scenario has nothing behind it, and an empty view is what a projection
        // that has not caught up holds.
        memory.push((view.clone(), fresh));
        SemanticViewResult::default()
    }
}

/// The billing reference, wrong in exactly one way.
///
/// # Panics
///
/// If `fault` is not one of `System::Billing`'s, which is a defect in the caller rather than a
/// condition: a fault names the specification it belongs to, and injecting an oracle fault into
/// billing would produce a green run that proves nothing.
pub fn billing(fault: Fault) -> Faulty<Billing> {
    assert_eq!(
        fault.system(),
        System::Billing,
        "{fault:?} is a fault of `{}`, not of billing",
        fault.system().directory()
    );
    Faulty::new(Billing::new(), fault)
}

/// The oracle reference, wrong in exactly one way — including the two the boundary cannot express.
///
/// # Panics
///
/// If `fault` is not one of `System::Oracle`'s. See [`billing`].
pub fn oracle(fault: Fault) -> Faulty<Oracle> {
    assert_eq!(
        fault.system(),
        System::Oracle,
        "{fault:?} is a fault of `{}`, not of the oracle fixture",
        fault.system().directory()
    );
    let reference = match fault {
        Fault::DropBinding => Oracle::new().with_binding_dropped(Oracle::HANDOFF_ON_PLACED),
        Fault::WrongMapping => Oracle::new().with_mapping_swapped(Oracle::HANDOFF_ON_PLACED),
        _ => Oracle::new(),
    };
    Faulty::new(reference, fault)
}

impl<T: ConformanceTarget> ConformanceTarget for Faulty<T> {
    /// The implementation underneath, named for the defect it carries.
    ///
    /// A report that said `billing-reference` for a build that is deliberately wrong would attest
    /// the opposite of what happened (§30).
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        let inner = self.inner.identity()?;
        Ok(ImplementationIdentity::new(
            format!("{}-{}", inner.name, self.fault.written()),
            inner.version,
        ))
    }

    fn begin_scenario(&self, scenario: &ScenarioContext) -> Result<(), TargetError> {
        // Isolation covers the fault's own memory too, or a stale answer from the previous scenario
        // would be a second, undeclared defect (§8).
        self.memory.borrow_mut().clear();
        self.inner.begin_scenario(scenario)
    }

    fn execute_command(
        &self,
        mut request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError> {
        let command = request.command.to_string();
        if self.fault == Fault::AcceptInvalidAmount && command == CREATE_INVOICE {
            // On the way in, as `aep_conformance`'s `ReplayApplies` rewrites an idempotency key: the
            // client submitted an amount the specification refuses and got an invoice.
            if let Some(amount) = request.input.get_mut("amount") {
                *amount = money(1.0);
            }
        }
        let input = request.input.clone();
        let mut result = self.inner.execute_command(request)?;

        match self.fault {
            Fault::WrongEvent if command == CREATE_INVOICE => {
                for occurrence in &mut result.direct_events {
                    if occurrence.event == event(INVOICE_CREATED) {
                        occurrence.event = event(INVOICE_ISSUED);
                    }
                }
            }
            Fault::WrongEventPayload if command == CREATE_INVOICE => {
                for occurrence in &mut result.direct_events {
                    if occurrence.event == event(INVOICE_CREATED) {
                        occurrence.payload.insert("amount".to_owned(), money(999.0));
                    }
                }
            }
            Fault::ExtraEvent if command == CREATE_INVOICE => {
                // Named after the invoice that was just created, so it is the plausible version of
                // this defect rather than an obviously stray message.
                let identity = result
                    .direct_events
                    .iter()
                    .find(|occurrence| occurrence.event == event(INVOICE_CREATED))
                    .and_then(|occurrence| occurrence.payload.get("invoice_id"))
                    .cloned()
                    .unwrap_or_default();
                result.direct_events.push(
                    ObservedEvent::new(event(INVOICE_CANCELLED)).with("invoice_id", identity),
                );
            }
            Fault::AllowIllegalTransition
                if command == CANCEL_INVOICE && result.outcome.is_none() =>
            {
                // The reference answers `undeclared` for a move its lifecycle does not run; this
                // reports that refusal as the success it was not, and publishes what the move would
                // have published.
                result.outcome = Some(cancelled());
                result
                    .direct_events
                    .push(ObservedEvent::new(event(INVOICE_CANCELLED)).with(
                        "invoice_id",
                        input.get("invoice_id").cloned().unwrap_or_default(),
                    ));
            }
            Fault::DropConsistencyToken => result.consistency = None,
            _ => {}
        }
        Ok(result)
    }

    fn query_view(&self, request: SemanticViewRequest) -> Result<SemanticViewResult, TargetError> {
        let view = request.view.clone();
        // Only a read that *demanded* the write is perturbed: an eventual read asks at `Current` and
        // is allowed to be behind, so answering it late would be conformant rather than faulty.
        let demanded = request.consistency.token().is_some();
        let fresh = self.inner.query_view(request)?;
        if self.fault == Fault::StaleReadYourWrites && demanded {
            return Ok(self.one_read_behind(&view, fresh));
        }
        Ok(fresh)
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        self.inner.observe_events(request)
    }

    fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError> {
        if self.fault == Fault::IgnoreExternalOutcome {
            // Accepted and ignored, which is the defect: a target that *refused* the control would
            // be reported as `error` and would be telling the truth about itself.
            return Ok(());
        }
        self.inner.configure_external_outcome(request)
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        self.inner.redeliver_event(request)
    }

    fn observe_invocations(
        &self,
        request: InvocationObservationRequest,
    ) -> Result<Vec<ObservedInvocation>, TargetError> {
        self.inner.observe_invocations(request)
    }

    fn end_scenario(&self, scenario: &ScenarioContext) -> Result<(), TargetError> {
        self.inner.end_scenario(scenario)
    }
}

// ---- the names this module writes --------------------------------------------------------------

/// Parses an event name this module names as a literal.
///
/// # Panics
///
/// It does not: the names come from [`crate::reference`], which checks them against
/// `examples/billing/`.
fn event(value: &str) -> EventRef {
    value
        .parse()
        .unwrap_or_else(|error| panic!("`{value}` is a well-formed event: {error}"))
}

/// `billing.invoice.CancelInvoice/cancelled`, the branch the illegal move reports.
///
/// # Panics
///
/// It does not, for the reason [`event`] does not.
fn cancelled() -> OutcomeRef {
    OutcomeRef::new(
        CANCEL_INVOICE
            .parse()
            .unwrap_or_else(|error| panic!("`{CANCEL_INVOICE}` is a well-formed command: {error}")),
        "cancelled"
            .parse()
            .unwrap_or_else(|error| panic!("`cancelled` is a well-formed outcome name: {error}")),
    )
}

/// A `billing.invoice.Money` of `amount`.
///
/// # Panics
///
/// It does not: the literals here are finite.
fn money(amount: f64) -> Node {
    let mut fields = BTreeMap::new();
    fields.insert(
        "amount".to_owned(),
        Node::Number(
            Number::new(amount).unwrap_or_else(|error| panic!("{amount} is finite: {error}")),
        ),
    );
    fields.insert(
        "currency".to_owned(),
        Node::Text("amount.currency".to_owned()),
    );
    Node::Map(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_faults_claim_the_same_scenario() {
        // Two faults caught by one scenario makes it impossible to say which property that scenario
        // actually protects — `aep_conformance`'s argument, one level down: there the unit is a
        // suite, here it is a single named check.
        let mut claimed: Vec<&str> = Fault::ALL
            .iter()
            .filter_map(|fault| fault.caught().scenario())
            .collect();
        claimed.sort_unstable();
        let mut unique = claimed.clone();
        unique.dedup();
        assert_eq!(
            claimed, unique,
            "two faults designating one scenario means neither of them shows what that scenario \
             protects"
        );
    }

    #[test]
    fn every_fault_says_what_it_is_and_where_it_goes() {
        // The list is generated from the same lines as the variants, so what is left to check is
        // that no row was declared empty — a fault with no description is a row in a matrix nobody
        // can read.
        assert_eq!(Fault::ALL.len(), 10);
        for fault in Fault::ALL {
            assert!(!fault.written().is_empty(), "{fault:?} has no written form");
            assert!(!fault.describe().is_empty(), "{fault:?} describes nothing");
            match fault.caught() {
                Caught::By(scenario) => assert!(
                    scenario.contains('/'),
                    "{fault:?} names `{scenario}`, which is not a scenario id"
                ),
                Caught::Nothing(why) => assert!(
                    why.len() > 20,
                    "{fault:?} is uncaught and says only `{why}`; an uncaught fault is a finding \
                     about the model or the synthesizer, and the finding is the reason"
                ),
            }
        }
    }

    #[test]
    fn only_the_two_faults_the_boundary_cannot_express_are_injected_in_the_implementation() {
        // The split is a property worth pinning rather than a habit: every fault that *can* be a
        // perturbation of what goes in and what comes out must be one, because that is the position
        // a real client is in. The two exceptions are argued in the module documentation.
        let implementation: Vec<Fault> = Fault::ALL
            .iter()
            .copied()
            .filter(|fault| fault.injection() == Injection::Implementation)
            .collect();
        assert_eq!(
            implementation,
            vec![Fault::DropBinding, Fault::WrongMapping],
            "a fault injected in the implementation needs the argument in this module's \
             documentation, not just a line in the table"
        );
    }

    #[test]
    fn a_fault_is_injected_into_the_system_that_declares_what_it_breaks() {
        // `oracle` and `billing` refuse a fault from the other system rather than running it, which
        // would otherwise be a green row in the matrix that proved nothing at all.
        for fault in Fault::ALL {
            match fault.system() {
                System::Billing => {
                    let _ = billing(*fault);
                }
                System::Oracle => {
                    let _ = oracle(*fault);
                }
            }
        }
        assert_eq!(System::Billing.directory(), "billing");
        assert_eq!(System::Oracle.directory(), "oracle-fixture");
    }
}
