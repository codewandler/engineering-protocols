//! The invoice obligations: four command behaviours and two projections, over one shared store.
//!
//! Written from `examples/billing/domains/invoice.yaml` and from the contracts the generated
//! `PLAN.md` carries. The lifecycle is not re-implemented: every move below goes through the
//! generated typestate — [`InvoiceSnapshot::refine`] into [`Invoice<S>`](billing_types::invoice::Invoice),
//! the declared transition method, back to a snapshot — so a move the specification does not
//! declare is not a branch here, it is a call that does not compile.
//!
//! # No clock, no randomness
//!
//! Invariant 9. Identifiers come from a per-store counter in the `Uuid` wire shape, so two runs of
//! one suite produce the same ids; `issued_at` stays `None` because nothing here owns a clock —
//! the conformance runner owns time, and no committed scenario asks for the field.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use billing_types::invoice::obligations::{
    CancelInvoiceBehavior, CreateInvoiceBehavior, InvoiceByIdQuery, IssueInvoiceBehavior,
    OutstandingInvoicesQuery, PayInvoiceBehavior,
};
use billing_types::invoice::{
    AnyInvoice, CancelInvoice, CancelInvoiceOutcome, Channel, CreateInvoice, CreateInvoiceOutcome,
    InvalidAmount, Invoice, InvoiceById, InvoiceCancelled, InvoiceCreated, InvoiceData, InvoiceId,
    InvoiceIssued, InvoicePaid, InvoiceSnapshot, InvoiceState, InvoiceStateConflict, IssueInvoice,
    IssueInvoiceOutcome, Money, OutstandingInvoices, PayInvoice, PayInvoiceOutcome, Payee,
};
use billing_types::obligation::UnmetObligation;
use billing_types::primitives::{Duration, Uuid};

/// `amount.amount > 0`, decided on the wire rendering and never on a float.
///
/// A `Decimal` carries its wire text — `10.50`, `0.00`, `-1` — and the guard needs only its sign:
/// a decimal is positive exactly when it carries no minus sign and some digit other than zero.
/// Parsing into a float to compare would round exactly the values money exists not to round.
pub fn positive(amount: &Money) -> bool {
    !amount.amount.0.starts_with('-')
        && amount
            .amount
            .0
            .chars()
            .any(|digit| digit.is_ascii_digit() && digit != '0')
}

/// What one scenario's invoices amount to: every snapshot, and the identifier mint.
#[derive(Debug, Default)]
struct Store {
    /// Every invoice ever created here, keyed by its id's wire rendering, in stable order.
    invoices: BTreeMap<String, InvoiceSnapshot>,
    /// The counter every generated identifier comes from.
    sequence: u64,
}

impl Store {
    /// A fresh identity in the `Uuid` wire shape, from the counter rather than from randomness.
    fn identifier(&mut self) -> InvoiceId {
        self.sequence += 1;
        InvoiceId(Uuid(format!(
            "00000000-0000-4000-8000-{:012}",
            self.sequence
        )))
    }
}

/// A cloneable handle on one scenario's invoices.
///
/// Shared, because the plan owes the invoice obligations one by one but they speak about the same
/// invoices — and because sharing is what lets the linker swap exactly one obligation for the
/// [`corrupted`](crate::corrupted) variant while the other implementations keep the same state.
#[derive(Clone, Debug, Default)]
pub struct SharedInvoices {
    store: Rc<RefCell<Store>>,
}

impl SharedInvoices {
    /// An empty store, for one scenario.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether an invoice with this identity was ever created here.
    ///
    /// For the adapter boundary: the generated behaviour seam cannot answer "no declared outcome"
    /// for a subject it has never seen — `unknown_subject` in this module argues it — so a
    /// conformance bridge asks this *before* routing a subject-bearing command in.
    pub fn knows(&self, id: &InvoiceId) -> bool {
        self.store.borrow().invoices.contains_key(&id.0 .0)
    }
}

/// The honest realization of every invoice obligation, over a shared store.
///
/// One struct implements all six traits; the linker still receives it once per obligation,
/// because resolution is per obligation (D-2) and the struct is merely who answers.
#[derive(Clone, Debug)]
pub struct InvoiceRealization {
    invoices: SharedInvoices,
}

impl InvoiceRealization {
    /// The realization, answering over `invoices`.
    pub fn over(invoices: SharedInvoices) -> Self {
        Self { invoices }
    }

    /// The acceptance half of `CreateInvoice`, with the guard already decided.
    ///
    /// `pub(crate)` so the [`corrupted`](crate::corrupted) variant can reuse the honest acceptance
    /// while dropping exactly the guard — a fault that replaces the whole method would be a
    /// different implementation, not a corruption of this one.
    pub(crate) fn accept(&mut self, input: CreateInvoice) -> CreateInvoiceOutcome {
        let mut store = self.invoices.store.borrow_mut();
        let invoice_id = store.identifier();
        // What the command does not determine, the realization decides, and says so: no line
        // items yet, the default settlement window the business runs on, delivery over email
        // because the payee is reached by one, and no signature until an issuer signs.
        let created = Invoice::new(InvoiceData {
            invoice_id: invoice_id.clone(),
            total: input.amount.clone(),
            payee: Payee::Person(input.customer_email.clone()),
            channel: Channel::Email,
            lines: Vec::new(),
            note: None,
            metadata: BTreeMap::new(),
            issued_at: None,
            settlement_window: Duration("P30D".to_owned()),
            is_recurring: false,
            signature: Vec::new(),
        });
        store.invoices.insert(
            invoice_id.0 .0.clone(),
            AnyInvoice::Draft(created).snapshot(),
        );
        CreateInvoiceOutcome::Accepted {
            invoice_created: InvoiceCreated {
                invoice_id,
                customer_email: input.customer_email,
                amount: input.amount,
            },
        }
    }
}

/// The one answer the generated seam cannot spell, refused loudly rather than guessed.
///
/// The conformance target interface answers a command against a subject it has never seen with
/// *no declared outcome* — the refusal the specification does not model. The generated behaviour
/// trait cannot: its `Ok` is the outcome enum, and the `wrong-state` variant demands the
/// `InvoiceStateConflict` state the invoice is really in, which an invoice that does not exist
/// does not have. Fabricating one would be manufactured evidence, so the honest total answer is
/// the typed refusal — loud, and never a state nobody observed. The conformance bridge checks
/// [`SharedInvoices::knows`] first and never routes an unknown subject here; that the seam needs
/// this workaround at all is a recorded W6.3 finding about the generator.
fn unknown_subject(source: &'static str) -> UnmetObligation {
    UnmetObligation {
        capability: "command behaviour",
        source,
    }
}

impl CreateInvoiceBehavior for InvoiceRealization {
    fn create_invoice(
        &mut self,
        input: CreateInvoice,
    ) -> Result<CreateInvoiceOutcome, UnmetObligation> {
        // The declared guard, first and alone: `accepted` when `amount.amount > 0`, `rejected`
        // otherwise. Nothing else about the input can refuse a creation.
        if !positive(&input.amount) {
            return Ok(CreateInvoiceOutcome::Rejected {
                error: InvalidAmount {
                    submitted: input.amount,
                },
            });
        }
        Ok(self.accept(input))
    }
}

impl IssueInvoiceBehavior for InvoiceRealization {
    fn issue_invoice(
        &mut self,
        input: IssueInvoice,
    ) -> Result<IssueInvoiceOutcome, UnmetObligation> {
        let key = input.invoice_id.0 .0.clone();
        let mut store = self.invoices.store.borrow_mut();
        let Some(snapshot) = store.invoices.get(&key).cloned() else {
            return Err(unknown_subject("billing.invoice.IssueInvoice"));
        };
        // `issue` runs from `Draft` and from nowhere else — the typed lifecycle carries that, so
        // the legal move is a method call and every other state is the declared `wrong-state`.
        match snapshot.refine() {
            AnyInvoice::Draft(invoice) => {
                store
                    .invoices
                    .insert(key, AnyInvoice::Issued(invoice.issue()).snapshot());
                Ok(IssueInvoiceOutcome::Issued {
                    invoice_issued: InvoiceIssued {
                        invoice_id: input.invoice_id,
                    },
                })
            }
            resting => Ok(IssueInvoiceOutcome::WrongState {
                error: InvoiceStateConflict {
                    state: resting.state(),
                },
            }),
        }
    }
}

impl CancelInvoiceBehavior for InvoiceRealization {
    fn cancel_invoice(
        &mut self,
        input: CancelInvoice,
    ) -> Result<CancelInvoiceOutcome, UnmetObligation> {
        let key = input.invoice_id.0 .0.clone();
        let mut store = self.invoices.store.borrow_mut();
        let Some(snapshot) = store.invoices.get(&key).cloned() else {
            return Err(unknown_subject("billing.invoice.CancelInvoice"));
        };
        // `cancel` runs from `Draft` and from `Issued`; `Paid` and `Cancelled` are terminal for it.
        let cancelled = match snapshot.refine() {
            AnyInvoice::Draft(invoice) => invoice.cancel(),
            AnyInvoice::Issued(invoice) => invoice.cancel(),
            resting => {
                return Ok(CancelInvoiceOutcome::WrongState {
                    error: InvoiceStateConflict {
                        state: resting.state(),
                    },
                })
            }
        };
        store
            .invoices
            .insert(key, AnyInvoice::Cancelled(cancelled).snapshot());
        Ok(CancelInvoiceOutcome::Cancelled {
            invoice_cancelled: InvoiceCancelled {
                invoice_id: input.invoice_id,
            },
        })
    }
}

impl PayInvoiceBehavior for InvoiceRealization {
    fn pay_invoice(&mut self, input: PayInvoice) -> Result<PayInvoiceOutcome, UnmetObligation> {
        // The guard decides before the subject does — `settled` is guarded by
        // `amount.amount > 0` and `rejected` is the unguarded branch — so a non-positive payment
        // is refused whatever the invoice is, including one that does not exist.
        if !positive(&input.amount) {
            return Ok(PayInvoiceOutcome::Rejected {
                error: InvalidAmount {
                    submitted: input.amount,
                },
            });
        }
        let key = input.invoice_id.0 .0.clone();
        let mut store = self.invoices.store.borrow_mut();
        let Some(snapshot) = store.invoices.get(&key).cloned() else {
            return Err(unknown_subject("billing.invoice.PayInvoice"));
        };
        match snapshot.refine() {
            AnyInvoice::Issued(invoice) => {
                store
                    .invoices
                    .insert(key, AnyInvoice::Paid(invoice.settle()).snapshot());
                Ok(PayInvoiceOutcome::Settled {
                    invoice_paid: InvoicePaid {
                        invoice_id: input.invoice_id,
                        amount: input.amount,
                    },
                })
            }
            resting => Ok(PayInvoiceOutcome::WrongState {
                error: InvoiceStateConflict {
                    state: resting.state(),
                },
            }),
        }
    }
}

impl InvoiceByIdQuery for InvoiceRealization {
    /// Every invoice, projected to its declared row.
    ///
    /// Served current rather than lagging: `eventual` is an upper bound on staleness, and a
    /// projection read straight off the store satisfies it. Making a projection *really* lag so
    /// the runner's bounded waiting is exercised is the reference implementation's job, and wave 4
    /// already discharged it.
    fn invoice_by_id(&self) -> Result<Vec<InvoiceById>, UnmetObligation> {
        Ok(self
            .invoices
            .store
            .borrow()
            .invoices
            .values()
            .map(|snapshot| InvoiceById {
                invoice_id: snapshot.data.invoice_id.clone(),
                total: snapshot.data.total.clone(),
            })
            .collect())
    }
}

impl OutstandingInvoicesQuery for InvoiceRealization {
    /// The declared filter, `state == Issued`, at `read_your_writes` — which a projection read
    /// straight off the store gives without waiting.
    fn outstanding_invoices(&self) -> Result<Vec<OutstandingInvoices>, UnmetObligation> {
        Ok(self
            .invoices
            .store
            .borrow()
            .invoices
            .values()
            .filter(|snapshot| snapshot.state == InvoiceState::Issued)
            .map(|snapshot| OutstandingInvoices {
                invoice_id: snapshot.data.invoice_id.clone(),
                total: snapshot.data.total.clone(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store the tests drive commands against.
    fn realization() -> (SharedInvoices, InvoiceRealization) {
        let invoices = SharedInvoices::new();
        let realization = InvoiceRealization::over(invoices.clone());
        (invoices, realization)
    }

    fn money(rendering: &str) -> Money {
        Money {
            amount: billing_types::primitives::Decimal(rendering.to_owned()),
            currency: "EUR".to_owned(),
        }
    }

    fn create(realization: &mut InvoiceRealization, rendering: &str) -> CreateInvoiceOutcome {
        realization
            .create_invoice(CreateInvoice {
                customer_email: billing_types::invoice::Email("a@example.com".to_owned()),
                amount: money(rendering),
            })
            .expect("the honest realization satisfies the obligation")
    }

    #[test]
    fn a_zero_amount_is_rejected_with_the_declared_error_carrying_what_was_submitted() {
        let (_, mut realization) = realization();
        let outcome = create(&mut realization, "0.00");
        let CreateInvoiceOutcome::Rejected { error } = outcome else {
            panic!("`0.00` satisfies no branch's guard but `rejected`, got {outcome:?}");
        };
        assert_eq!(
            error.submitted,
            money("0.00"),
            "the declared error carries the refused amount, so the caller learns what was refused"
        );
    }

    #[test]
    fn a_negative_amount_with_nonzero_digits_is_still_rejected() {
        // The sign test is the half of `positive` a digit scan alone would miss: `-1` has a
        // nonzero digit and must still be refused.
        let (_, mut realization) = realization();
        let outcome = create(&mut realization, "-1");
        assert!(
            matches!(outcome, CreateInvoiceOutcome::Rejected { .. }),
            "`-1` must take `rejected`, got {outcome:?}"
        );
    }

    #[test]
    fn paying_a_draft_invoice_reports_the_state_it_is_really_in() {
        let (_, mut realization) = realization();
        let CreateInvoiceOutcome::Accepted { invoice_created } = create(&mut realization, "10.50")
        else {
            panic!("a positive amount is accepted");
        };
        let outcome = realization
            .pay_invoice(PayInvoice {
                invoice_id: invoice_created.invoice_id,
                amount: money("10.50"),
            })
            .expect("the honest realization satisfies the obligation");
        let PayInvoiceOutcome::WrongState { error } = outcome else {
            panic!("`settle` does not run from `Draft`, got {outcome:?}");
        };
        assert_eq!(
            error.state,
            InvoiceState::Draft,
            "the conflict names the state the invoice is really in, not merely 'not payable'"
        );
    }

    #[test]
    fn the_outstanding_projection_holds_exactly_the_issued() {
        let (_, mut realization) = realization();
        let CreateInvoiceOutcome::Accepted { invoice_created } = create(&mut realization, "1")
        else {
            panic!("a positive amount is accepted");
        };
        let kept = invoice_created.invoice_id.clone();
        create(&mut realization, "2");

        realization
            .issue_invoice(IssueInvoice {
                invoice_id: kept.clone(),
            })
            .expect("the obligation is satisfied");
        let rows = realization
            .outstanding_invoices()
            .expect("the obligation is satisfied");
        assert_eq!(
            rows.len(),
            1,
            "only the issued invoice is outstanding; the draft must not appear"
        );
        assert_eq!(rows[0].invoice_id, kept, "the row names the issued invoice");
    }

    #[test]
    fn an_unknown_subject_is_refused_loudly_not_answered_with_a_fabricated_state() {
        // The recorded seam gap: no declared outcome can carry "never seen", and inventing a
        // state would be manufactured evidence. The typed refusal names the command it is about.
        let (invoices, mut realization) = realization();
        let stranger = InvoiceId(Uuid("00000000-0000-4000-8000-999999999999".to_owned()));
        assert!(
            !invoices.knows(&stranger),
            "the fixture must reach the state where the rule is load-bearing: an id nothing created"
        );
        let refusal = realization
            .issue_invoice(IssueInvoice {
                invoice_id: stranger,
            })
            .expect_err("an unknown subject has no honest declared outcome");
        assert_eq!(refusal.source, "billing.invoice.IssueInvoice");
        assert_eq!(refusal.capability, "command behaviour");
    }
}
