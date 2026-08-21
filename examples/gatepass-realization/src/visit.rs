//! The visit obligations: three command behaviours and two projections, over one shared store.
//!
//! Written from `examples/gatepass/domains/visit.yaml` and from the contracts the generated
//! `PLAN.md` carries. The lifecycle is not re-implemented: every move below goes through the
//! generated typestate — [`VisitSnapshot::refine`] into `Visit<S>`, the declared transition
//! method, back to a snapshot — so a move the specification does not declare is not a branch here,
//! it is a call that does not compile.
//!
//! # No clock, no randomness
//!
//! Invariant 9, and here it is load-bearing twice over: two processes synthesised from one
//! specification are started side by side and their answers compared, so an identifier from a
//! random source would make the two disagree about a value neither of them chose. Identifiers come
//! from a per-store counter in the `Uuid` wire shape; nothing reads a clock, and `printed_at` is
//! whatever the caller sent.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gatepass_types::obligation::UnmetObligation;
use gatepass_types::primitives::Uuid;
use gatepass_types::visit::obligations::{
    AdmitVisitorBehavior, ExpectedVisitsQuery, RegisterVisitBehavior, SignOutVisitorBehavior,
    VisitByIdQuery,
};
use gatepass_types::visit::{
    AdmitVisitor, AdmitVisitorOutcome, AnyVisit, ExpectedVisits, InvalidVisitLength, RegisterVisit,
    RegisterVisitOutcome, SignOutVisitor, SignOutVisitorOutcome, Visit, VisitById, VisitData,
    VisitId, VisitRegistered, VisitSnapshot, VisitState, VisitStateConflict, VisitorAdmitted,
    VisitorDeparted,
};

/// What one run's visits amount to: every snapshot, and the identifier mint.
#[derive(Debug, Default)]
struct Store {
    /// Every visit ever registered here, keyed by its id's wire rendering, in stable order.
    visits: BTreeMap<String, VisitSnapshot>,
    /// The counter every generated identifier comes from.
    sequence: u64,
}

impl Store {
    /// A fresh identity in the `Uuid` wire shape, from the counter rather than from randomness.
    fn identifier(&mut self) -> VisitId {
        self.sequence += 1;
        VisitId(Uuid(format!(
            "00000000-0000-4000-8000-{:012}",
            self.sequence
        )))
    }
}

/// A cloneable handle on one run's visits.
///
/// Shared, because the plan owes the five obligations one by one and they all speak about the same
/// visits. The linker still resolves each obligation separately (D-2); this is merely who answers.
#[derive(Clone, Debug, Default)]
pub struct SharedVisits {
    store: Rc<RefCell<Store>>,
}

impl SharedVisits {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

/// The honest realization of every visit obligation, over a shared store.
#[derive(Clone, Debug)]
pub struct VisitRealization {
    visits: SharedVisits,
}

impl VisitRealization {
    /// The realization, answering over `visits`.
    pub fn over(visits: SharedVisits) -> Self {
        Self { visits }
    }
}

/// The one answer the generated seam cannot spell, refused loudly rather than guessed.
///
/// A command naming a visit that was never registered has no declared outcome: `wrong-state`
/// demands the `VisitStateConflict` state the visit is really in, and a visit that does not exist
/// does not have one. Fabricating a state would be manufacturing an observation, so the honest
/// total answer is the typed refusal — which the served surface reports as `501`, naming the
/// obligation. That it has to is a gap in the *model*, not in this file: the specification language
/// has no way to declare "no such subject", and the same finding is recorded against the billing
/// realization.
fn unknown_subject(source: &'static str) -> UnmetObligation {
    UnmetObligation {
        capability: "command behaviour",
        source,
    }
}

impl RegisterVisitBehavior for VisitRealization {
    fn register_visit(
        &mut self,
        input: RegisterVisit,
    ) -> Result<RegisterVisitOutcome, UnmetObligation> {
        // The declared guard, first and alone: `registered` when `expected_minutes > 0`, `refused`
        // otherwise. Nothing else about the input can refuse a registration.
        if input.expected_minutes <= 0 {
            return Ok(RegisterVisitOutcome::Refused {
                error: InvalidVisitLength {
                    submitted: input.expected_minutes,
                },
            });
        }
        let mut store = self.visits.store.borrow_mut();
        let visit_id = store.identifier();
        // What the command does not determine, the realization decides and says so: nobody is on
        // the watch list until somebody puts them there, and there is no badge until one is
        // printed at the desk — which is what `AdmitVisitor` carries.
        let registered = Visit::new(VisitData {
            visit_id: visit_id.clone(),
            visitor: input.visitor.clone(),
            building: input.building,
            host: input.host,
            expected_minutes: input.expected_minutes,
            expected_stay: input.expected_stay,
            deposit: input.deposit,
            escorts: input.escorts,
            notes: input.notes,
            badge: None,
            on_watchlist: input.on_watchlist,
        });
        store.visits.insert(
            visit_id.0 .0.clone(),
            AnyVisit::Expected(registered).snapshot(),
        );
        Ok(RegisterVisitOutcome::Registered {
            visit_registered: VisitRegistered {
                visit_id,
                visitor: input.visitor,
                building: input.building,
            },
        })
    }
}

impl AdmitVisitorBehavior for VisitRealization {
    fn admit_visitor(
        &mut self,
        input: AdmitVisitor,
    ) -> Result<AdmitVisitorOutcome, UnmetObligation> {
        let key = input.visit_id.0 .0.clone();
        let mut store = self.visits.store.borrow_mut();
        let Some(snapshot) = store.visits.get(&key).cloned() else {
            return Err(unknown_subject("gatepass.visit.AdmitVisitor"));
        };
        // `arrive` runs from `Expected` and from nowhere else — the typed lifecycle carries that,
        // so the legal move is a method call and every other state is the declared `wrong-state`.
        match snapshot.refine() {
            AnyVisit::Expected(visit) => {
                let mut data = visit.arrive().into_data();
                data.badge = Some(input.badge.clone());
                store.visits.insert(
                    key,
                    VisitSnapshot {
                        state: VisitState::OnSite,
                        data,
                    },
                );
                Ok(AdmitVisitorOutcome::Admitted {
                    visitor_admitted: VisitorAdmitted {
                        visit_id: input.visit_id,
                        badge: input.badge,
                    },
                })
            }
            resting => Ok(AdmitVisitorOutcome::WrongState {
                error: VisitStateConflict {
                    state: resting.state(),
                },
            }),
        }
    }
}

impl SignOutVisitorBehavior for VisitRealization {
    fn sign_out_visitor(
        &mut self,
        input: SignOutVisitor,
    ) -> Result<SignOutVisitorOutcome, UnmetObligation> {
        let key = input.visit_id.0 .0.clone();
        let mut store = self.visits.store.borrow_mut();
        let Some(snapshot) = store.visits.get(&key).cloned() else {
            return Err(unknown_subject("gatepass.visit.SignOutVisitor"));
        };
        match snapshot.refine() {
            AnyVisit::OnSite(visit) => {
                store
                    .visits
                    .insert(key, AnyVisit::Departed(visit.depart()).snapshot());
                Ok(SignOutVisitorOutcome::SignedOut {
                    visitor_departed: VisitorDeparted {
                        visit_id: input.visit_id,
                    },
                })
            }
            resting => Ok(SignOutVisitorOutcome::WrongState {
                error: VisitStateConflict {
                    state: resting.state(),
                },
            }),
        }
    }
}

impl ExpectedVisitsQuery for VisitRealization {
    /// The declared filter, applied: `state == Expected` and nothing else.
    ///
    /// Read straight off the store, which is what `read_your_writes` obliges — a receptionist who
    /// has just registered a visitor and cannot see them here has been told a lie about what they
    /// did.
    fn expected_visits(&self) -> Result<Vec<ExpectedVisits>, UnmetObligation> {
        Ok(self
            .visits
            .store
            .borrow()
            .visits
            .values()
            .filter(|snapshot| snapshot.state == VisitState::Expected)
            .map(|snapshot| ExpectedVisits {
                visit_id: snapshot.data.visit_id.clone(),
                visitor: snapshot.data.visitor.clone(),
                building: snapshot.data.building,
                deposit: snapshot.data.deposit.clone(),
            })
            .collect())
    }
}

impl VisitByIdQuery for VisitRealization {
    /// Every visit, projected to its declared row.
    ///
    /// Served current rather than lagging: `eventual` is an upper bound on staleness, and a
    /// projection read straight off the store satisfies it.
    fn visit_by_id(&self) -> Result<Vec<VisitById>, UnmetObligation> {
        Ok(self
            .visits
            .store
            .borrow()
            .visits
            .values()
            .map(|snapshot| VisitById {
                visit_id: snapshot.data.visit_id.clone(),
                visitor: snapshot.data.visitor.clone(),
                host: snapshot.data.host.clone(),
                escorts: snapshot.data.escorts.clone(),
                notes: snapshot.data.notes.clone(),
                badge: snapshot.data.badge.clone(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gatepass_types::primitives::{Decimal, Duration};
    use gatepass_types::visit::{Badge, Building, Deposit, EmployeeId, Host, VisitorName};

    fn registration(minutes: i64) -> RegisterVisit {
        RegisterVisit {
            visitor: VisitorName("Ada Lovelace".to_owned()),
            building: Building::North,
            host: Host::Employee(EmployeeId("e-42".to_owned())),
            expected_minutes: minutes,
            expected_stay: Duration("PT90M".to_owned()),
            deposit: Deposit {
                amount: Decimal("25.00".to_owned()),
                currency: "EUR".to_owned(),
            },
            escorts: Vec::new(),
            notes: BTreeMap::new(),
            on_watchlist: false,
        }
    }

    fn badge() -> Badge {
        Badge {
            serial: "N-0007".to_owned(),
            printed_at: None,
            signature: vec![1, 2, 3],
        }
    }

    #[test]
    fn a_visit_of_no_length_is_refused_with_the_number_that_was_sent() {
        let mut realization = VisitRealization::over(SharedVisits::new());
        let outcome = realization
            .register_visit(registration(0))
            .expect("the obligation is implemented");
        assert_eq!(
            outcome,
            RegisterVisitOutcome::Refused {
                error: InvalidVisitLength { submitted: 0 }
            },
            "the declared guard is `expected_minutes > 0`, and the error carries what was sent"
        );
    }

    #[test]
    fn a_visitor_signed_out_cannot_be_signed_out_again_and_is_told_which_state_they_are_in() {
        let mut realization = VisitRealization::over(SharedVisits::new());
        let RegisterVisitOutcome::Registered { visit_registered } = realization
            .register_visit(registration(90))
            .expect("the obligation is implemented")
        else {
            panic!("a positive length is registered");
        };
        let visit_id = visit_registered.visit_id;
        realization
            .admit_visitor(AdmitVisitor {
                visit_id: visit_id.clone(),
                badge: badge(),
            })
            .expect("the obligation is implemented");
        realization
            .sign_out_visitor(SignOutVisitor {
                visit_id: visit_id.clone(),
            })
            .expect("the obligation is implemented");

        // The fixture has reached the state the rule is about: the visit is Departed, which is a
        // state `depart` does not run from, so the second sign-out is the declared refusal rather
        // than a repeat of the first.
        let again = realization
            .sign_out_visitor(SignOutVisitor { visit_id })
            .expect("the obligation is implemented");
        assert_eq!(
            again,
            SignOutVisitorOutcome::WrongState {
                error: VisitStateConflict {
                    state: VisitState::Departed
                }
            },
            "`no` without `they already left` sends the desk back to guess"
        );
    }

    #[test]
    fn the_expected_list_holds_only_visitors_who_have_not_arrived() {
        let mut realization = VisitRealization::over(SharedVisits::new());
        let RegisterVisitOutcome::Registered { visit_registered } = realization
            .register_visit(registration(30))
            .expect("the obligation is implemented")
        else {
            panic!("a positive length is registered");
        };
        realization
            .register_visit(registration(45))
            .expect("the obligation is implemented");
        assert_eq!(
            realization
                .expected_visits()
                .expect("the obligation is implemented")
                .len(),
            2,
            "both are expected before either arrives"
        );

        realization
            .admit_visitor(AdmitVisitor {
                visit_id: visit_registered.visit_id.clone(),
                badge: badge(),
            })
            .expect("the obligation is implemented");
        let expected = realization
            .expected_visits()
            .expect("the obligation is implemented");
        assert_eq!(
            expected.len(),
            1,
            "the declared filter is `state == Expected`"
        );
        assert_ne!(
            expected[0].visit_id, visit_registered.visit_id,
            "and the one that arrived is the one that left the list"
        );
        assert_eq!(
            realization
                .visit_by_id()
                .expect("the obligation is implemented")
                .len(),
            2,
            "the unfiltered projection still holds both"
        );
    }
}
