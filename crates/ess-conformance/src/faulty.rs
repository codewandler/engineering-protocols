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
//! # A fault caught by nothing is the finding, and every one recorded here has since been closed
//!
//! [`Caught::Nothing`] is not a gap in this module. A fault nobody catches is worth more than
//! another passing row: it says the specification cannot express the property, or that synthesis
//! does not ask for it. Three rows sat here; all three have since been closed and moved to the
//! other side of the matrix rather than being deleted — two by teaching synthesis to ask for more,
//! and the last by changing the **model**:
//!
//! | fault | what a client would see | what closed it |
//! |---|---|---|
//! | [`ExtraEvent`](Fault::ExtraEvent) | a cancelled invoice is announced as paid | every event the specification declares and this branch does not emit is now asserted absent |
//! | [`DropConsistencyToken`](Fault::DropConsistencyToken) | a `read_your_writes` view is never actually held to it | a read with no token to demand is `unsupported`, not a weaker read at `Current` |
//! | [`WrongEventPayload`](Fault::WrongEventPayload) | every consumer of `InvoicePaid` records an amount nobody paid | an outcome's `payload:` now says which input determines which event field, so the value stopped being a guess and became a reading |
//!
//! The last of the three sat here longest because it needed the **model** to change rather than
//! the synthesizer: `999` is a well-formed `Money` in a field the event declares, and until wave
//! 6.5 nothing in the model said where a payload field's *value* comes from — asserting
//! `InvoicePaid.amount == PayInvoice.amount` would have been a match on a shared field name, the
//! inference this crate refuses everywhere else. The `payload:` declaration on a command outcome
//! is the construct that licenses it, and [`PartialEventPayload`](Fault::PartialEventPayload)
//! remains the other half of the same coin: a *type* was always declared, so a declared field left
//! out was already caught before any value could be.
//!
//! An event field with **no** declared source stays undetermined — `InvoiceCreated.invoice_id` is
//! the implementation's to mint — and the suite shows that by asserting its presence and type and
//! never a value. A fault perturbing only an undetermined field would still be caught by nothing,
//! and that is the specification's decision rather than a gap here.
//!
//! # Boundary or implementation
//!
//! Eleven of the thirteen are [`Injection::Boundary`]: [`Faulty`] perturbs what goes into the
//! target and what comes out of it, and never a broken internal, because that is the same position
//! a real client is in.
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
    Billing, Oracle, CANCEL_INVOICE, CREATE_INVOICE, INVALID_AMOUNT, INVOICE_CANCELLED,
    INVOICE_CREATED, INVOICE_ISSUED, INVOICE_PAID, ISSUE_INVOICE, OUTSTANDING, PAY_INVOICE,
};
use crate::scenario::{ErrorRef, EventRef, OutcomeRef, ViewRef};
use crate::target::{
    ConformanceTarget, DeclaredErrorValue, EventObservationRequest, ExternalOutcomeControl,
    ImplementationIdentity, InvocationObservationRequest, ObservedEvent, ObservedInvocation,
    RedeliveryRequest, ScenarioContext, SemanticCommandRequest, SemanticCommandResult,
    SemanticViewRequest, SemanticViewResult, TargetError,
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
        /// Seven are design §25's fault table. Five more were gone looking for, three of which
        /// nothing caught when they were written down — see the [module documentation](self) for
        /// which of those are closed and which one still needs the model to change.
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

    /// A wrong-state refusal reports the wrong declared error.
    ///
    /// Not one of §25's seven, and it was uncatchable until the model could say what a command
    /// answers in a state its moves do not start from: nothing moved, nothing was published, and the
    /// only thing wrong is the name of the error — so every assertion the suite could previously
    /// make passed. It is the fault that says the `wrong_state:` branch bought a real check rather
    /// than a tidier document.
    WrongRefusalError => "wrong-refusal-error", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.Invoice/state/Paid/refuses/billing.invoice.IssueInvoice"),
        "a command refused for the right reason names the wrong declared error";

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
    /// Not one of §25's seven, and the row that was caught by nothing the longest — until wave 6.5
    /// gave the model a `payload:` declaration on a command outcome. Every field
    /// `billing.invoice.InvoicePaid` declares is present and every one is of its declared type —
    /// `999` is as well-formed a `Money` as the amount that was submitted — so the only check that
    /// can see it is `InvoicePaid.amount == PayInvoice.amount`, and that check is licensed exactly
    /// where the specification declares `amount: input.amount`. `examples/billing/` declares it on
    /// `settled`, so both scenarios that exercise the branch now assert the value; the matrix
    /// designates the *transition* scenario because §10's outcome scenario already designates
    /// [`PartialEventPayload`](Fault::PartialEventPayload), and one scenario names one fault.
    WrongEventPayload => "wrong-event-payload", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled"),
        "a published event carries an amount the command was never given";

    /// A published event leaves out a field its declaration says it carries.
    ///
    /// The other half of [`WrongEventPayload`](Fault::WrongEventPayload), and the half the model
    /// does license: `billing.invoice.InvoicePaid` declares `invoice_id` and `amount`, so an
    /// occurrence carrying only the first contradicts the specification without any claim about
    /// where a value comes from. It is here so that "presence and type are asserted" is a row in the
    /// matrix rather than a sentence in a doc comment.
    PartialEventPayload => "partial-event-payload", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.PayInvoice/outcome/settled"),
        "a published event leaves out a field its declaration says it carries";

    /// A branch publishes an event it does not declare it emits, beside the ones it does.
    ///
    /// Not one of §25's seven. It was uncaught while `ExpectNoEvent` was synthesised only for the
    /// events a *sibling* branch emits; the rule `ESS-CF-NO-EVENT` names is wider than that, and
    /// synthesis now asks it of every event the specification declares and this branch does not.
    ExtraEvent => "extra-event", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.CancelInvoice/outcome/cancelled"),
        "cancelling an invoice also announces that it was paid";

    /// A command answers without the token a later read would demand.
    ///
    /// Not one of §25's seven. It was uncaught while the runner fell back to `Current` with no token
    /// in hand — a weaker read that passes, which is the "skip that looks like a pass". A
    /// read-your-writes read that cannot be demanded is now `unsupported`, which fails the run.
    DropConsistencyToken => "drop-consistency-token", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.Invoice/transition/issue/by/billing.invoice.IssueInvoice/issued"),
        "a command returns no consistency token, so no read is ever held to it";

    /// A projection publishes a value the type it holds forbids.
    ///
    /// The wave 6.5 value-object family's row. `OutstandingInvoices.total` is a `Money` and `Money`
    /// declares `amount >= 0` of every value; a projection that corrupts what it publishes breaks
    /// that claim with **no command having done anything wrong** — the write side is untouched, so
    /// every outcome, transition and refusal scenario passes, and the only checks positioned to see
    /// it are the two that read a value object's own invariants off the field positions that hold
    /// one. The designated scenario is the position the fault corrupts; the check at
    /// `InvoiceById.total` stays green, which is exactly why two positions are two scenarios.
    NegativeProjectedTotal => "negative-projected-total", System::Billing, Injection::Boundary,
        Caught::By("billing.invoice.Money/invariant/at/billing.invoice.OutstandingInvoices/total"),
        "the outstanding list reports a total below zero, which no Money admits";
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
            Fault::WrongEventPayload if command == PAY_INVOICE => {
                for occurrence in &mut result.direct_events {
                    if occurrence.event == event(INVOICE_PAID) {
                        occurrence.payload.insert("amount".to_owned(), money(999.0));
                    }
                }
            }
            Fault::PartialEventPayload if command == PAY_INVOICE => {
                for occurrence in &mut result.direct_events {
                    if occurrence.event == event(INVOICE_PAID) {
                        occurrence.payload.remove("amount");
                    }
                }
            }
            Fault::ExtraEvent if command == CANCEL_INVOICE => {
                // Named after the invoice that was just cancelled, so it is the plausible version of
                // this defect rather than an obviously stray message — and it is the dangerous one:
                // a consumer that hears `InvoicePaid` stops chasing an invoice nobody paid.
                let identity = input.get("invoice_id").cloned().unwrap_or_default();
                result
                    .direct_events
                    .push(ObservedEvent::new(event(INVOICE_PAID)).with("invoice_id", identity));
            }
            Fault::AllowIllegalTransition
                if command == CANCEL_INVOICE && refused_for_state(&result, CANCEL_INVOICE) =>
            {
                // The reference answers the declared `wrong-state` branch for a move its lifecycle
                // does not run; this reports that refusal as the success it was not, drops the error
                // that named it, and publishes what the move would have published.
                result.outcome = Some(cancelled());
                result.error = None;
                result
                    .direct_events
                    .push(ObservedEvent::new(event(INVOICE_CANCELLED)).with(
                        "invoice_id",
                        input.get("invoice_id").cloned().unwrap_or_default(),
                    ));
            }
            Fault::WrongRefusalError
                if command == ISSUE_INVOICE && refused_for_state(&result, ISSUE_INVOICE) =>
            {
                // The branch is right and the error is not. Nothing observable changed — no event
                // was published and no state moved — so the only thing that distinguishes this from
                // a correct implementation is the error the refusal names, which is exactly what
                // `wrong_state:` put into the model.
                result.error = Some(DeclaredErrorValue::new(declared_error(INVALID_AMOUNT)));
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
        let mut fresh = self.inner.query_view(request)?;
        if self.fault == Fault::StaleReadYourWrites && demanded {
            return Ok(self.one_read_behind(&view, fresh));
        }
        if self.fault == Fault::NegativeProjectedTotal && view.to_string() == OUTSTANDING {
            // One view, every row, one field: the projection's own corruption, not the write's.
            // `InvoiceById` answers untouched, which is what pins the two positions apart.
            for row in &mut fresh.rows {
                if let Some(Node::Map(fields)) = row.get_mut("total") {
                    fields.insert(
                        "amount".to_owned(),
                        Node::Number(
                            Number::new(-1.0)
                                .unwrap_or_else(|error| panic!("-1 is finite: {error}")),
                        ),
                    );
                }
            }
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
    branch_of(CANCEL_INVOICE, "cancelled")
}

/// `true` when the reference answered the `wrong_state:` branch of `command_name`.
///
/// Read off the branch the result names rather than off "no outcome and no events": a target that
/// declares the branch reports it, and a fault that keyed on the absence of an outcome would stop
/// firing the moment a specification adopted the construct.
fn refused_for_state(result: &SemanticCommandResult, command_name: &str) -> bool {
    result
        .outcome
        .as_ref()
        .is_some_and(|taken| taken == &branch_of(command_name, WRONG_STATE))
}

/// The name every `wrong_state:` branch in `examples/billing/` is declared under.
const WRONG_STATE: &str = "wrong-state";

/// One declared error, by name.
///
/// # Panics
///
/// It does not, for the reason [`event`] does not.
fn declared_error(value: &str) -> ErrorRef {
    value
        .parse()
        .unwrap_or_else(|error| panic!("`{value}` is a well-formed error: {error}"))
}

/// One declared branch of one command.
///
/// # Panics
///
/// It does not, for the reason [`event`] does not.
fn branch_of(command_name: &str, name: &str) -> OutcomeRef {
    OutcomeRef::new(
        command_name
            .parse()
            .unwrap_or_else(|error| panic!("`{command_name}` is a well-formed command: {error}")),
        name.parse()
            .unwrap_or_else(|error| panic!("`{name}` is a well-formed outcome name: {error}")),
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
        assert_eq!(Fault::ALL.len(), 13);
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
