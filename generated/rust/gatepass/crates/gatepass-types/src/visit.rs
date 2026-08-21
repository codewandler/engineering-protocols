// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! Visits — `gatepass.visit`.
//!
//! Expecting a visitor, letting them in, and letting them out again.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// Badge — `gatepass.visit.Badge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    /// `serial` — `String`.
    pub serial: String,
    /// `printed_at` — `Optional<Timestamp>`.
    pub printed_at: Option<crate::primitives::Timestamp>,
    /// `signature` — `Bytes`.
    pub signature: Vec<u8>,
}

/// Building — `gatepass.visit.Building`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Building {
    /// `North`.
    North,
    /// `South`.
    South,
    /// `Annex`.
    Annex,
}

/// Deposit — `gatepass.visit.Deposit`.
///
/// Every value satisfies `amount >= 0` — declared here, enforced by whatever behaviour constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deposit {
    /// `amount` — `Decimal`.
    pub amount: crate::primitives::Decimal,
    /// `currency` — `String`.
    pub currency: String,
}

/// EmployeeId — `gatepass.visit.EmployeeId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeId(pub String);

/// Host — `gatepass.visit.Host`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    /// Tagged `contractor` — `gatepass.visit.VendorRef`.
    Contractor(VendorRef),
    /// Tagged `employee` — `gatepass.visit.EmployeeId`.
    Employee(EmployeeId),
}

/// VendorRef — `gatepass.visit.VendorRef`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorRef(pub String);

/// The states of `gatepass.visit.Visit`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Visit<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitState {
    /// `Departed`.
    Departed,
    /// `Expected`.
    Expected,
    /// `OnSite`.
    OnSite,
}

/// VisitId — `gatepass.visit.VisitId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitId(pub crate::primitives::Uuid);

/// VisitorName — `gatepass.visit.VisitorName`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitorName(pub String);

/// What Visit — `gatepass.visit.Visit` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Visit<S>`], and at a boundary by [`VisitSnapshot::state`].
///
/// Every value satisfies `deposit.amount >= 0` — declared here, enforced by whatever behaviour constructs one.
/// Every value satisfies `expected_minutes > 0` — declared here, enforced by whatever behaviour constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitData {
    /// The identity: `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
    /// `visitor` — `gatepass.visit.VisitorName`.
    pub visitor: VisitorName,
    /// `building` — `gatepass.visit.Building`.
    pub building: Building,
    /// `host` — `gatepass.visit.Host`.
    pub host: Host,
    /// `expected_minutes` — `Integer`.
    pub expected_minutes: i64,
    /// `expected_stay` — `Duration`.
    pub expected_stay: crate::primitives::Duration,
    /// `deposit` — `gatepass.visit.Deposit`.
    pub deposit: Deposit,
    /// `escorts` — `List<gatepass.visit.VisitorName>`.
    pub escorts: Vec<VisitorName>,
    /// `notes` — `Map<String, String>`.
    pub notes: std::collections::BTreeMap<String, String>,
    /// `badge` — `Optional<gatepass.visit.Badge>`.
    pub badge: Option<Badge>,
    /// `on_watchlist` — `Boolean`.
    pub on_watchlist: bool,
}

/// The states of `gatepass.visit.Visit`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](visit_state::Marker), so [`Visit<S>`](Visit) can only ever rest in a real state.
pub mod visit_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Departed {}
        impl Sealed for super::Expected {}
        impl Sealed for super::OnSite {}
    }

    /// A declared state of `Visit`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::VisitState;
    }

    /// `Departed`. Terminal: an instance may rest here forever.
    pub struct Departed;

    impl Marker for Departed {
        const STATE: super::VisitState = super::VisitState::Departed;
    }

    /// `Expected`. Where a new instance starts.
    pub struct Expected;

    impl Marker for Expected {
        const STATE: super::VisitState = super::VisitState::Expected;
    }

    /// `OnSite`.
    pub struct OnSite;

    impl Marker for OnSite {
        const STATE: super::VisitState = super::VisitState::OnSite;
    }
}

/// Visit — `gatepass.visit.Visit` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Expected`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`VisitSnapshot`]
/// and [`VisitSnapshot::refine`].
pub struct Visit<S: visit_state::Marker> {
    data: VisitData,
    state: core::marker::PhantomData<S>,
}

impl<S: visit_state::Marker> Visit<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> VisitState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &VisitData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> VisitData {
        self.data
    }
}

impl Visit<visit_state::Expected> {
    /// A new instance, resting in `Expected` — the only state the lifecycle starts one in.
    pub fn new(data: VisitData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

impl Visit<visit_state::Expected> {
    /// `arrive` — `Expected` → `OnSite`. Taken by the `admitted` outcome of `gatepass.visit.AdmitVisitor`.
    pub fn arrive(self) -> Visit<visit_state::OnSite> {
        Visit {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl Visit<visit_state::OnSite> {
    /// `depart` — `OnSite` → `Departed`. Taken by the `signed-out` outcome of `gatepass.visit.SignOutVisitor`.
    pub fn depart(self) -> Visit<visit_state::Departed> {
        Visit {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

/// `gatepass.visit.Visit` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`VisitSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: VisitState,
    /// What it holds.
    pub data: VisitData,
}

/// An `Visit` in whichever declared state it was found.
pub enum AnyVisit {
    /// Resting in `Departed`.
    Departed(Visit<visit_state::Departed>),
    /// Resting in `Expected`.
    Expected(Visit<visit_state::Expected>),
    /// Resting in `OnSite`.
    OnSite(Visit<visit_state::OnSite>),
}

impl VisitSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `VisitState` cannot spell one.
    pub fn refine(self) -> AnyVisit {
        match self.state {
            VisitState::Departed => AnyVisit::Departed(Visit {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            VisitState::Expected => AnyVisit::Expected(Visit {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            VisitState::OnSite => AnyVisit::OnSite(Visit {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyVisit {
    /// The state, as the runtime value.
    pub fn state(&self) -> VisitState {
        match self {
            Self::Departed(_) => VisitState::Departed,
            Self::Expected(_) => VisitState::Expected,
            Self::OnSite(_) => VisitState::OnSite,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> VisitSnapshot {
        match self {
            Self::Departed(instance) => VisitSnapshot {
                state: VisitState::Departed,
                data: instance.into_data(),
            },
            Self::Expected(instance) => VisitSnapshot {
                state: VisitState::Expected,
                data: instance.into_data(),
            },
            Self::OnSite(instance) => VisitSnapshot {
                state: VisitState::OnSite,
                data: instance.into_data(),
            },
        }
    }
}

/// Admit the visitor — the input of `gatepass.visit.AdmitVisitor`.
///
/// Everything it can result in is [`AdmitVisitorOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitVisitor {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
    /// `badge` — `gatepass.visit.Badge`.
    pub badge: Badge,
}

/// Everything `gatepass.visit.AdmitVisitor` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitVisitorOutcome {
    /// `admitted` — otherwise.
    ///
    /// The visitor is on site, holding the badge that was printed.
    Admitted {
        /// The `gatepass.visit.VisitorAdmitted` this outcome publishes.
        visitor_admitted: VisitorAdmitted,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The visit is not Expected, so nobody was admitted.
    WrongState {
        /// Why it was refused: `gatepass.visit.VisitStateConflict`.
        error: VisitStateConflict,
    },
}

/// Register a visit — the input of `gatepass.visit.RegisterVisit`.
///
/// Everything it can result in is [`RegisterVisitOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterVisit {
    /// `visitor` — `gatepass.visit.VisitorName`.
    pub visitor: VisitorName,
    /// `building` — `gatepass.visit.Building`.
    pub building: Building,
    /// `host` — `gatepass.visit.Host`.
    pub host: Host,
    /// `expected_minutes` — `Integer`.
    pub expected_minutes: i64,
    /// `expected_stay` — `Duration`.
    pub expected_stay: crate::primitives::Duration,
    /// `deposit` — `gatepass.visit.Deposit`.
    pub deposit: Deposit,
    /// `escorts` — `List<gatepass.visit.VisitorName>`.
    pub escorts: Vec<VisitorName>,
    /// `notes` — `Map<String, String>`.
    pub notes: std::collections::BTreeMap<String, String>,
    /// `on_watchlist` — `Boolean`.
    pub on_watchlist: bool,
}

/// Everything `gatepass.visit.RegisterVisit` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterVisitOutcome {
    /// `registered` — when `expected_minutes > 0`.
    ///
    /// The visit is recorded, and the visitor is Expected.
    Registered {
        /// The `gatepass.visit.VisitRegistered` this outcome publishes.
        visit_registered: VisitRegistered,
    },
    /// `refused` — otherwise.
    ///
    /// The expected length was not positive, and nothing was recorded.
    Refused {
        /// Why it was refused: `gatepass.visit.InvalidVisitLength`.
        error: InvalidVisitLength,
    },
}

/// Sign the visitor out — the input of `gatepass.visit.SignOutVisitor`.
///
/// Everything it can result in is [`SignOutVisitorOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignOutVisitor {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
}

/// Everything `gatepass.visit.SignOutVisitor` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignOutVisitorOutcome {
    /// `signed-out` — otherwise.
    ///
    /// The visitor has left the building.
    SignedOut {
        /// The `gatepass.visit.VisitorDeparted` this outcome publishes.
        visitor_departed: VisitorDeparted,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The visit is not OnSite, so nobody was signed out.
    WrongState {
        /// Why it was refused: `gatepass.visit.VisitStateConflict`.
        error: VisitStateConflict,
    },
}

/// VisitRegistered — the event `gatepass.visit.VisitRegistered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitRegistered {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
    /// `visitor` — `gatepass.visit.VisitorName`.
    pub visitor: VisitorName,
    /// `building` — `gatepass.visit.Building`.
    pub building: Building,
}

/// VisitorAdmitted — the event `gatepass.visit.VisitorAdmitted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitorAdmitted {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
    /// `badge` — `gatepass.visit.Badge`.
    pub badge: Badge,
}

/// VisitorDeparted — the event `gatepass.visit.VisitorDeparted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitorDeparted {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
}

/// The declared error `gatepass.visit.InvalidVisitLength`.
///
/// The expected length of the visit is not a positive number of minutes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidVisitLength {
    /// `submitted` — `Integer`.
    pub submitted: i64,
}

/// The declared error `gatepass.visit.VisitStateConflict`.
///
/// The visit is not in a state this command acts from, so nothing moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitStateConflict {
    /// `state` — `gatepass.visit.Visit.State`.
    pub state: VisitState,
}

/// Expected visits — one row of the view `gatepass.visit.ExpectedVisits`.
///
/// Projects `gatepass.visit.Visit` at `read_your_writes` consistency, containing instances where `state == Expected`.
/// Serving it is an implementation obligation — see the plan — because how a projection is kept
/// current is a storage decision the specification does not take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedVisits {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
    /// `visitor` — `gatepass.visit.VisitorName`.
    pub visitor: VisitorName,
    /// `building` — `gatepass.visit.Building`.
    pub building: Building,
    /// `deposit` — `gatepass.visit.Deposit`.
    pub deposit: Deposit,
}

/// Visit by id — one row of the view `gatepass.visit.VisitById`.
///
/// Projects `gatepass.visit.Visit` at `eventual` consistency.
/// Serving it is an implementation obligation — see the plan — because how a projection is kept
/// current is a storage decision the specification does not take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitById {
    /// `visit_id` — `gatepass.visit.VisitId`.
    pub visit_id: VisitId,
    /// `visitor` — `gatepass.visit.VisitorName`.
    pub visitor: VisitorName,
    /// `host` — `gatepass.visit.Host`.
    pub host: Host,
    /// `escorts` — `List<gatepass.visit.VisitorName>`.
    pub escorts: Vec<VisitorName>,
    /// `notes` — `Map<String, String>`.
    pub notes: std::collections::BTreeMap<String, String>,
    /// `badge` — `Optional<gatepass.visit.Badge>`.
    pub badge: Option<Badge>,
}

/// What this bounded context owes its implementor, as typed seams.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every trait by refusing in the type system, so the workspace builds —
/// and says exactly what it cannot yet do — before a line is hand-written.
pub mod obligations {
    /// The behaviour `gatepass.visit.AdmitVisitor` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `gatepass.visit.AdmitVisitor` input, decide and enact exactly one outcome — `admitted` otherwise, takes `arrive` of `gatepass.visit.Visit`, emits `gatepass.visit.VisitorAdmitted`; `wrong-state` from a state no declared move starts in, error `gatepass.visit.VisitStateConflict`.
    pub trait AdmitVisitorBehavior {
        /// Decides and enacts exactly one declared outcome of `gatepass.visit.AdmitVisitor`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn admit_visitor(&mut self, input: super::AdmitVisitor) -> Result<super::AdmitVisitorOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `gatepass.visit.RegisterVisit` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `gatepass.visit.RegisterVisit` input, decide and enact exactly one outcome — `registered` when `expected_minutes > 0`, creates `gatepass.visit.Visit`, emits `gatepass.visit.VisitRegistered`; `refused` otherwise, error `gatepass.visit.InvalidVisitLength`.
    pub trait RegisterVisitBehavior {
        /// Decides and enacts exactly one declared outcome of `gatepass.visit.RegisterVisit`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn register_visit(&mut self, input: super::RegisterVisit) -> Result<super::RegisterVisitOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `gatepass.visit.SignOutVisitor` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: given `gatepass.visit.SignOutVisitor` input, decide and enact exactly one outcome — `signed-out` otherwise, takes `depart` of `gatepass.visit.Visit`, emits `gatepass.visit.VisitorDeparted`; `wrong-state` from a state no declared move starts in, error `gatepass.visit.VisitStateConflict`.
    pub trait SignOutVisitorBehavior {
        /// Decides and enacts exactly one declared outcome of `gatepass.visit.SignOutVisitor`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn sign_out_visitor(&mut self, input: super::SignOutVisitor) -> Result<super::SignOutVisitorOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `gatepass.visit.ExpectedVisits` — an implementation obligation.
    ///
    /// Why it is not generated: how the projection is kept current is a storage decision.
    ///
    /// Contract: a query answering `gatepass.visit.ExpectedVisits` with rows projected from `gatepass.visit.Visit` at `read_your_writes` consistency, containing instances where `state == Expected`.
    pub trait ExpectedVisitsQuery {
        /// Serves `gatepass.visit.ExpectedVisits` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn expected_visits(&self) -> Result<Vec<super::ExpectedVisits>, crate::obligation::UnmetObligation>;
    }

    /// The query `gatepass.visit.VisitById` — an implementation obligation.
    ///
    /// Why it is not generated: how the projection is kept current is a storage decision.
    ///
    /// Contract: a query answering `gatepass.visit.VisitById` with rows projected from `gatepass.visit.Visit` at `eventual` consistency.
    pub trait VisitByIdQuery {
        /// Serves `gatepass.visit.VisitById` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn visit_by_id(&self) -> Result<Vec<super::VisitById>, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl AdmitVisitorBehavior for Unimplemented {
        fn admit_visitor(&mut self, _input: super::AdmitVisitor) -> Result<super::AdmitVisitorOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "gatepass.visit.AdmitVisitor" })
        }
    }

    impl RegisterVisitBehavior for Unimplemented {
        fn register_visit(&mut self, _input: super::RegisterVisit) -> Result<super::RegisterVisitOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "gatepass.visit.RegisterVisit" })
        }
    }

    impl SignOutVisitorBehavior for Unimplemented {
        fn sign_out_visitor(&mut self, _input: super::SignOutVisitor) -> Result<super::SignOutVisitorOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "gatepass.visit.SignOutVisitor" })
        }
    }

    impl ExpectedVisitsQuery for Unimplemented {
        fn expected_visits(&self) -> Result<Vec<super::ExpectedVisits>, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "view query", source: "gatepass.visit.ExpectedVisits" })
        }
    }

    impl VisitByIdQuery for Unimplemented {
        fn visit_by_id(&self) -> Result<Vec<super::VisitById>, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "view query", source: "gatepass.visit.VisitById" })
        }
    }
}
