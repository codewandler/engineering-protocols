// generated from billing v3
// model digest e19d384dac86219a
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! Invoicing — `billing.invoice`.
//!
//! Issuing invoices and tracking whether they are paid.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// Delivery channel — `billing.invoice.Channel`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// `Email`.
    Email,
    /// `Post`.
    Post,
    /// `Portal`.
    Portal,
}

/// CompanyRef — `billing.invoice.CompanyRef`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyRef(pub String);

/// Email — `billing.invoice.Email`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(pub String);

/// The states of `billing.invoice.Invoice`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Invoice<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceState {
    /// `Cancelled`.
    Cancelled,
    /// `Draft`.
    Draft,
    /// `Issued`.
    Issued,
    /// `Paid`.
    Paid,
}

/// InvoiceId — `billing.invoice.InvoiceId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceId(pub crate::primitives::Uuid);

/// LineItem — `billing.invoice.LineItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineItem {
    /// `description` — `String`.
    pub description: String,
    /// `quantity` — `Integer`.
    pub quantity: i64,
    /// `unit_price` — `billing.invoice.Money`.
    pub unit_price: Money,
}

/// Money — `billing.invoice.Money`.
///
/// Every value satisfies `amount >= 0` — declared here, enforced by whatever behaviour constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    /// `amount` — `Decimal`.
    pub amount: crate::primitives::Decimal,
    /// `currency` — `String`.
    pub currency: String,
}

/// Payee — `billing.invoice.Payee`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payee {
    /// Tagged `company` — `billing.invoice.CompanyRef`.
    Company(CompanyRef),
    /// Tagged `person` — `billing.invoice.Email`.
    Person(Email),
}

/// What Invoice — `billing.invoice.Invoice` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Invoice<S>`], and at a boundary by [`InvoiceSnapshot::state`].
///
/// Every value satisfies `total.amount >= 0` — declared here, enforced by whatever behaviour constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceData {
    /// The identity: `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
    /// `total` — `billing.invoice.Money`.
    pub total: Money,
    /// `payee` — `billing.invoice.Payee`.
    pub payee: Payee,
    /// `channel` — `billing.invoice.Channel`.
    pub channel: Channel,
    /// `lines` — `List<billing.invoice.LineItem>`.
    pub lines: Vec<LineItem>,
    /// `note` — `Optional<String>`.
    pub note: Option<String>,
    /// `metadata` — `Map<String, String>`.
    pub metadata: std::collections::BTreeMap<String, String>,
    /// `issued_at` — `Optional<Timestamp>`.
    pub issued_at: Option<crate::primitives::Timestamp>,
    /// `settlement_window` — `Duration`.
    pub settlement_window: crate::primitives::Duration,
    /// `is_recurring` — `Boolean`.
    pub is_recurring: bool,
    /// `signature` — `Bytes`.
    pub signature: Vec<u8>,
}

/// The states of `billing.invoice.Invoice`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](invoice_state::Marker), so [`Invoice<S>`](Invoice) can only ever rest in a real state.
pub mod invoice_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Cancelled {}
        impl Sealed for super::Draft {}
        impl Sealed for super::Issued {}
        impl Sealed for super::Paid {}
    }

    /// A declared state of `Invoice`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::InvoiceState;
    }

    /// `Cancelled`. Terminal: an instance may rest here forever.
    pub struct Cancelled;

    impl Marker for Cancelled {
        const STATE: super::InvoiceState = super::InvoiceState::Cancelled;
    }

    /// `Draft`. Where a new instance starts.
    pub struct Draft;

    impl Marker for Draft {
        const STATE: super::InvoiceState = super::InvoiceState::Draft;
    }

    /// `Issued`.
    pub struct Issued;

    impl Marker for Issued {
        const STATE: super::InvoiceState = super::InvoiceState::Issued;
    }

    /// `Paid`. Terminal: an instance may rest here forever.
    pub struct Paid;

    impl Marker for Paid {
        const STATE: super::InvoiceState = super::InvoiceState::Paid;
    }
}

/// Invoice — `billing.invoice.Invoice` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Draft`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`InvoiceSnapshot`]
/// and [`InvoiceSnapshot::refine`].
pub struct Invoice<S: invoice_state::Marker> {
    data: InvoiceData,
    state: core::marker::PhantomData<S>,
}

impl<S: invoice_state::Marker> Invoice<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> InvoiceState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &InvoiceData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> InvoiceData {
        self.data
    }
}

impl Invoice<invoice_state::Draft> {
    /// A new instance, resting in `Draft` — the only state the lifecycle starts one in.
    pub fn new(data: InvoiceData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

impl Invoice<invoice_state::Draft> {
    /// `issue` — `Draft` → `Issued`. Taken by the `issued` outcome of `billing.invoice.IssueInvoice`.
    pub fn issue(self) -> Invoice<invoice_state::Issued> {
        Invoice {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `cancel` — `Draft` → `Cancelled`. Taken by the `cancelled` outcome of `billing.invoice.CancelInvoice`.
    pub fn cancel(self) -> Invoice<invoice_state::Cancelled> {
        Invoice {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl Invoice<invoice_state::Issued> {
    /// `settle` — `Issued` → `Paid`. Taken by the `settled` outcome of `billing.invoice.PayInvoice`.
    pub fn settle(self) -> Invoice<invoice_state::Paid> {
        Invoice {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `cancel` — `Issued` → `Cancelled`. Taken by the `cancelled` outcome of `billing.invoice.CancelInvoice`.
    pub fn cancel(self) -> Invoice<invoice_state::Cancelled> {
        Invoice {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

/// `billing.invoice.Invoice` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`InvoiceSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: InvoiceState,
    /// What it holds.
    pub data: InvoiceData,
}

/// An `Invoice` in whichever declared state it was found.
pub enum AnyInvoice {
    /// Resting in `Cancelled`.
    Cancelled(Invoice<invoice_state::Cancelled>),
    /// Resting in `Draft`.
    Draft(Invoice<invoice_state::Draft>),
    /// Resting in `Issued`.
    Issued(Invoice<invoice_state::Issued>),
    /// Resting in `Paid`.
    Paid(Invoice<invoice_state::Paid>),
}

impl InvoiceSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `InvoiceState` cannot spell one.
    pub fn refine(self) -> AnyInvoice {
        match self.state {
            InvoiceState::Cancelled => AnyInvoice::Cancelled(Invoice {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            InvoiceState::Draft => AnyInvoice::Draft(Invoice {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            InvoiceState::Issued => AnyInvoice::Issued(Invoice {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            InvoiceState::Paid => AnyInvoice::Paid(Invoice {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyInvoice {
    /// The state, as the runtime value.
    pub fn state(&self) -> InvoiceState {
        match self {
            Self::Cancelled(_) => InvoiceState::Cancelled,
            Self::Draft(_) => InvoiceState::Draft,
            Self::Issued(_) => InvoiceState::Issued,
            Self::Paid(_) => InvoiceState::Paid,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> InvoiceSnapshot {
        match self {
            Self::Cancelled(instance) => InvoiceSnapshot {
                state: InvoiceState::Cancelled,
                data: instance.into_data(),
            },
            Self::Draft(instance) => InvoiceSnapshot {
                state: InvoiceState::Draft,
                data: instance.into_data(),
            },
            Self::Issued(instance) => InvoiceSnapshot {
                state: InvoiceState::Issued,
                data: instance.into_data(),
            },
            Self::Paid(instance) => InvoiceSnapshot {
                state: InvoiceState::Paid,
                data: instance.into_data(),
            },
        }
    }
}

/// Cancel invoice — the input of `billing.invoice.CancelInvoice`.
///
/// Everything it can result in is [`CancelInvoiceOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelInvoice {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
}

/// Everything `billing.invoice.CancelInvoice` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelInvoiceOutcome {
    /// `cancelled` — otherwise.
    ///
    /// The invoice is cancelled, from Draft or from Issued.
    Cancelled {
        /// The `billing.invoice.InvoiceCancelled` this outcome publishes.
        invoice_cancelled: InvoiceCancelled,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The invoice is already Paid or already Cancelled, so nothing was cancelled.
    WrongState {
        /// Why it was refused: `billing.invoice.InvoiceStateConflict`.
        error: InvoiceStateConflict,
    },
}

/// Create invoice — the input of `billing.invoice.CreateInvoice`.
///
/// Everything it can result in is [`CreateInvoiceOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateInvoice {
    /// `customer_email` — `billing.invoice.Email`.
    pub customer_email: Email,
    /// `amount` — `billing.invoice.Money`.
    pub amount: Money,
}

/// Everything `billing.invoice.CreateInvoice` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateInvoiceOutcome {
    /// `accepted` — when `amount.amount > 0`.
    ///
    /// The invoice is created in Draft.
    Accepted {
        /// The `billing.invoice.InvoiceCreated` this outcome publishes.
        invoice_created: InvoiceCreated,
    },
    /// `rejected` — otherwise.
    ///
    /// The amount was not positive, and nothing was created.
    Rejected {
        /// Why it was refused: `billing.invoice.InvalidAmount`.
        error: InvalidAmount,
    },
}

/// Issue invoice — the input of `billing.invoice.IssueInvoice`.
///
/// Everything it can result in is [`IssueInvoiceOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueInvoice {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
}

/// Everything `billing.invoice.IssueInvoice` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueInvoiceOutcome {
    /// `issued` — otherwise.
    ///
    /// The invoice leaves Draft and is now Issued.
    Issued {
        /// The `billing.invoice.InvoiceIssued` this outcome publishes.
        invoice_issued: InvoiceIssued,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The invoice is not in Draft, so it was not issued.
    WrongState {
        /// Why it was refused: `billing.invoice.InvoiceStateConflict`.
        error: InvoiceStateConflict,
    },
}

/// Pay invoice — the input of `billing.invoice.PayInvoice`.
///
/// Everything it can result in is [`PayInvoiceOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayInvoice {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
    /// `amount` — `billing.invoice.Money`.
    pub amount: Money,
}

/// Everything `billing.invoice.PayInvoice` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayInvoiceOutcome {
    /// `settled` — when `amount.amount > 0`.
    ///
    /// The payment is accepted and the invoice becomes Paid.
    Settled {
        /// The `billing.invoice.InvoicePaid` this outcome publishes.
        invoice_paid: InvoicePaid,
    },
    /// `rejected` — otherwise.
    ///
    /// The payment was not positive, so the invoice did not move.
    Rejected {
        /// Why it was refused: `billing.invoice.InvalidAmount`.
        error: InvalidAmount,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The invoice is not Issued, so the payment did not settle it.
    WrongState {
        /// Why it was refused: `billing.invoice.InvoiceStateConflict`.
        error: InvoiceStateConflict,
    },
}

/// InvoiceCancelled — the event `billing.invoice.InvoiceCancelled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceCancelled {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
}

/// InvoiceCreated — the event `billing.invoice.InvoiceCreated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceCreated {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
    /// `customer_email` — `billing.invoice.Email`.
    pub customer_email: Email,
    /// `amount` — `billing.invoice.Money`.
    pub amount: Money,
}

/// InvoiceIssued — the event `billing.invoice.InvoiceIssued`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceIssued {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
}

/// InvoicePaid — the event `billing.invoice.InvoicePaid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoicePaid {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
    /// `amount` — `billing.invoice.Money`.
    pub amount: Money,
}

/// The declared error `billing.invoice.InvalidAmount`.
///
/// The requested amount is not positive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAmount {
    /// `submitted` — `billing.invoice.Money`.
    pub submitted: Money,
}

/// The declared error `billing.invoice.InvoiceStateConflict`.
///
/// The invoice is not in a state this command acts from, so nothing moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceStateConflict {
    /// `state` — `billing.invoice.Invoice.State`.
    pub state: InvoiceState,
}

/// InvoiceById — one row of the view `billing.invoice.InvoiceById`.
///
/// Projects `billing.invoice.Invoice` at `eventual` consistency.
/// Serving it is an implementation obligation — see the plan — because how a projection is kept
/// current is a storage decision the specification does not take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceById {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
    /// `total` — `billing.invoice.Money`.
    pub total: Money,
}

/// Outstanding invoices — one row of the view `billing.invoice.OutstandingInvoices`.
///
/// Projects `billing.invoice.Invoice` at `read_your_writes` consistency, containing instances where `state == Issued`.
/// Serving it is an implementation obligation — see the plan — because how a projection is kept
/// current is a storage decision the specification does not take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutstandingInvoices {
    /// `invoice_id` — `billing.invoice.InvoiceId`.
    pub invoice_id: InvoiceId,
    /// `total` — `billing.invoice.Money`.
    pub total: Money,
}

/// What this bounded context owes its implementor, as typed seams.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every trait by refusing in the type system, so the workspace builds —
/// and says exactly what it cannot yet do — before a line is hand-written.
pub mod obligations {
    /// The behaviour `billing.invoice.CancelInvoice` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `billing.invoice.CancelInvoice` input, decide and enact exactly one outcome — `cancelled` otherwise, takes `cancel` of `billing.invoice.Invoice`, emits `billing.invoice.InvoiceCancelled`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict`.
    pub trait CancelInvoiceBehavior {
        /// Decides and enacts exactly one declared outcome of `billing.invoice.CancelInvoice`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn cancel_invoice(&mut self, input: super::CancelInvoice) -> Result<super::CancelInvoiceOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `billing.invoice.CreateInvoice` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `billing.invoice.CreateInvoice` input, decide and enact exactly one outcome — `accepted` when `amount.amount > 0`, creates `billing.invoice.Invoice`, emits `billing.invoice.InvoiceCreated`; `rejected` otherwise, error `billing.invoice.InvalidAmount`.
    pub trait CreateInvoiceBehavior {
        /// Decides and enacts exactly one declared outcome of `billing.invoice.CreateInvoice`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn create_invoice(&mut self, input: super::CreateInvoice) -> Result<super::CreateInvoiceOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `billing.invoice.IssueInvoice` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `billing.invoice.IssueInvoice` input, decide and enact exactly one outcome — `issued` otherwise, takes `issue` of `billing.invoice.Invoice`, emits `billing.invoice.InvoiceIssued`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict`.
    pub trait IssueInvoiceBehavior {
        /// Decides and enacts exactly one declared outcome of `billing.invoice.IssueInvoice`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn issue_invoice(&mut self, input: super::IssueInvoice) -> Result<super::IssueInvoiceOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `billing.invoice.PayInvoice` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `billing.invoice.PayInvoice` input, decide and enact exactly one outcome — `settled` when `amount.amount > 0`, takes `settle` of `billing.invoice.Invoice`, emits `billing.invoice.InvoicePaid`; `rejected` otherwise, error `billing.invoice.InvalidAmount`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict`.
    pub trait PayInvoiceBehavior {
        /// Decides and enacts exactly one declared outcome of `billing.invoice.PayInvoice`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn pay_invoice(&mut self, input: super::PayInvoice) -> Result<super::PayInvoiceOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `billing.invoice.InvoiceById` — an implementation obligation.
    ///
    /// Why it is not generated: how the projection is kept current is a storage decision.
    ///
    /// Contract: a query answering `billing.invoice.InvoiceById` with rows projected from `billing.invoice.Invoice` at `eventual` consistency.
    pub trait InvoiceByIdQuery {
        /// Serves `billing.invoice.InvoiceById` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn invoice_by_id(&self) -> Result<Vec<super::InvoiceById>, crate::obligation::UnmetObligation>;
    }

    /// The query `billing.invoice.OutstandingInvoices` — an implementation obligation.
    ///
    /// Why it is not generated: how the projection is kept current is a storage decision.
    ///
    /// Contract: a query answering `billing.invoice.OutstandingInvoices` with rows projected from `billing.invoice.Invoice` at `read_your_writes` consistency, containing instances where `state == Issued`.
    pub trait OutstandingInvoicesQuery {
        /// Serves `billing.invoice.OutstandingInvoices` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn outstanding_invoices(&self) -> Result<Vec<super::OutstandingInvoices>, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl CancelInvoiceBehavior for Unimplemented {
        fn cancel_invoice(&mut self, _input: super::CancelInvoice) -> Result<super::CancelInvoiceOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "billing.invoice.CancelInvoice" })
        }
    }

    impl CreateInvoiceBehavior for Unimplemented {
        fn create_invoice(&mut self, _input: super::CreateInvoice) -> Result<super::CreateInvoiceOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "billing.invoice.CreateInvoice" })
        }
    }

    impl IssueInvoiceBehavior for Unimplemented {
        fn issue_invoice(&mut self, _input: super::IssueInvoice) -> Result<super::IssueInvoiceOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "billing.invoice.IssueInvoice" })
        }
    }

    impl PayInvoiceBehavior for Unimplemented {
        fn pay_invoice(&mut self, _input: super::PayInvoice) -> Result<super::PayInvoiceOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "billing.invoice.PayInvoice" })
        }
    }

    impl InvoiceByIdQuery for Unimplemented {
        fn invoice_by_id(&self) -> Result<Vec<super::InvoiceById>, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "view query", source: "billing.invoice.InvoiceById" })
        }
    }

    impl OutstandingInvoicesQuery for Unimplemented {
        fn outstanding_invoices(&self) -> Result<Vec<super::OutstandingInvoices>, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "view query", source: "billing.invoice.OutstandingInvoices" })
        }
    }
}
