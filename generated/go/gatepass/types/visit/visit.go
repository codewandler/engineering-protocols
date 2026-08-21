// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package visit is Visits — `gatepass.visit`.
//
// Expecting a visitor, letting them in, and letting them out again.
//
// Everything this bounded context declares that the synthesis plan marks generated and this
// target can represent. What it cannot is in the TARGET.md beside this module, never absent.
package visit

import (
	"example.invalid/gatepass/types/obligation"
	"example.invalid/gatepass/types/primitives"
)

// Badge is Badge — `gatepass.visit.Badge`.
type Badge struct {
	// Serial is `serial` — `String`.
	Serial string
	// PrintedAt is `printed_at` — `Optional<Timestamp>`.
	PrintedAt *primitives.Timestamp
	// Signature is `signature` — `Bytes`.
	Signature []byte
}

// Building is Building — `gatepass.visit.Building`: one of a closed set of names.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type Building interface {
	isBuilding()
}

// BuildingNorth is `North`.
type BuildingNorth struct{}

func (BuildingNorth) isBuilding() {}

// BuildingSouth is `South`.
type BuildingSouth struct{}

func (BuildingSouth) isBuilding() {}

// BuildingAnnex is `Annex`.
type BuildingAnnex struct{}

func (BuildingAnnex) isBuilding() {}

// Deposit is Deposit — `gatepass.visit.Deposit`.
//
// Every value satisfies `amount >= 0` — declared here, enforced by whatever behaviour constructs one.
type Deposit struct {
	// Amount is `amount` — `Decimal`.
	Amount primitives.Decimal
	// Currency is `currency` — `String`.
	Currency string
}

// EmployeeId is EmployeeId — `gatepass.visit.EmployeeId`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewEmployeeId] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// EmployeeId, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type EmployeeId struct {
	value string
}

// NewEmployeeId wraps a `String` as EmployeeId.
func NewEmployeeId(value string) EmployeeId {
	return EmployeeId{value: value}
}

// Value is the wrapped `String`.
func (v EmployeeId) Value() string {
	return v.value
}

// Host is Host — `gatepass.visit.Host`: one of a fixed set of shapes, tagged on the wire by `kind`.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type Host interface {
	isHost()
}

// HostContractor is the shape tagged `contractor` — `gatepass.visit.VendorRef`.
type HostContractor struct {
	// Value is what this shape carries.
	Value VendorRef
}

func (HostContractor) isHost() {}

// HostEmployee is the shape tagged `employee` — `gatepass.visit.EmployeeId`.
type HostEmployee struct {
	// Value is what this shape carries.
	Value EmployeeId
}

func (HostEmployee) isHost() {}

// VendorRef is VendorRef — `gatepass.visit.VendorRef`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewVendorRef] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// VendorRef, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type VendorRef struct {
	value string
}

// NewVendorRef wraps a `String` as VendorRef.
func NewVendorRef(value string) VendorRef {
	return VendorRef{value: value}
}

// Value is the wrapped `String`.
func (v VendorRef) Value() string {
	return v.value
}

// VisitState is the states of `gatepass.visit.Visit`, as runtime values.
//
// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is
// not carried here — it is carried by one type per state, where an undeclared move is a
// method that does not exist.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type VisitState interface {
	isVisitState()
}

// VisitStateDeparted is `Departed`.
type VisitStateDeparted struct{}

func (VisitStateDeparted) isVisitState() {}

// VisitStateExpected is `Expected`.
type VisitStateExpected struct{}

func (VisitStateExpected) isVisitState() {}

// VisitStateOnSite is `OnSite`.
type VisitStateOnSite struct{}

func (VisitStateOnSite) isVisitState() {}

// VisitId is VisitId — `gatepass.visit.VisitId`: a distinct wrapper around `Uuid`.
//
// The field is unexported, so the only way to make one carrying a value is [NewVisitId] —
// a defined type over `primitives.Uuid` would have let an untyped constant be assigned straight to
// VisitId, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type VisitId struct {
	value primitives.Uuid
}

// NewVisitId wraps a `Uuid` as VisitId.
func NewVisitId(value primitives.Uuid) VisitId {
	return VisitId{value: value}
}

// Value is the wrapped `Uuid`.
func (v VisitId) Value() primitives.Uuid {
	return v.value
}

// VisitorName is VisitorName — `gatepass.visit.VisitorName`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewVisitorName] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// VisitorName, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type VisitorName struct {
	value string
}

// NewVisitorName wraps a `String` as VisitorName.
func NewVisitorName(value string) VisitorName {
	return VisitorName{value: value}
}

// Value is the wrapped `String`.
func (v VisitorName) Value() string {
	return v.value
}

// VisitData is what Visit — `gatepass.visit.Visit` — holds, apart from where it is in its lifecycle.
//
// The identity and every declared field. The state is deliberately not one: inside the domain
// it is carried by the type ([VisitExpected] and its siblings), and at a boundary by [VisitSnapshot].
//
// Every value satisfies `deposit.amount >= 0` — declared here, enforced by whatever behaviour constructs one.
// Every value satisfies `expected_minutes > 0` — declared here, enforced by whatever behaviour constructs one.
type VisitData struct {
	// VisitId is the identity: `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
	// Visitor is `visitor` — `gatepass.visit.VisitorName`.
	Visitor VisitorName
	// Building is `building` — `gatepass.visit.Building`.
	Building Building
	// Host is `host` — `gatepass.visit.Host`.
	Host Host
	// ExpectedMinutes is `expected_minutes` — `Integer`.
	ExpectedMinutes int64
	// ExpectedStay is `expected_stay` — `Duration`.
	ExpectedStay primitives.Duration
	// Deposit is `deposit` — `gatepass.visit.Deposit`.
	Deposit Deposit
	// Escorts is `escorts` — `List<gatepass.visit.VisitorName>`.
	Escorts []VisitorName
	// Notes is `notes` — `Map<String, String>`.
	Notes map[string]string
	// Badge is `badge` — `Optional<gatepass.visit.Badge>`.
	Badge *Badge
	// OnWatchlist is `on_watchlist` — `Boolean`.
	OnWatchlist bool
}

// VisitInDeparted is `gatepass.visit.Visit` resting in `Departed`. Terminal: an instance may rest here forever.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type VisitInDeparted struct {
	data VisitData
}

// State is the state this instance rests in, as the runtime value.
func (VisitInDeparted) State() VisitState {
	return VisitStateDeparted{}
}

// Data is what it holds.
func (v VisitInDeparted) Data() VisitData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v VisitInDeparted) Snapshot() VisitSnapshot {
	return VisitSnapshot{State: VisitStateDeparted{}, Data: v.data}
}

func (VisitInDeparted) isAnyVisit() {}

// VisitInExpected is `gatepass.visit.Visit` resting in `Expected`. Where a new instance starts.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type VisitInExpected struct {
	data VisitData
}

// NewVisit starts a new `gatepass.visit.Visit` in `Expected` — the only state the lifecycle starts one in.
func NewVisit(data VisitData) VisitInExpected {
	return VisitInExpected{data: data}
}

// State is the state this instance rests in, as the runtime value.
func (VisitInExpected) State() VisitState {
	return VisitStateExpected{}
}

// Data is what it holds.
func (v VisitInExpected) Data() VisitData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v VisitInExpected) Snapshot() VisitSnapshot {
	return VisitSnapshot{State: VisitStateExpected{}, Data: v.data}
}

func (VisitInExpected) isAnyVisit() {}

// Arrive takes `arrive` — `Expected` → `OnSite`. Taken by the `admitted` outcome of `gatepass.visit.AdmitVisitor`.
func (v VisitInExpected) Arrive() VisitInOnSite {
	return VisitInOnSite{data: v.data}
}

// VisitInOnSite is `gatepass.visit.Visit` resting in `OnSite`.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type VisitInOnSite struct {
	data VisitData
}

// State is the state this instance rests in, as the runtime value.
func (VisitInOnSite) State() VisitState {
	return VisitStateOnSite{}
}

// Data is what it holds.
func (v VisitInOnSite) Data() VisitData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v VisitInOnSite) Snapshot() VisitSnapshot {
	return VisitSnapshot{State: VisitStateOnSite{}, Data: v.data}
}

func (VisitInOnSite) isAnyVisit() {}

// Depart takes `depart` — `OnSite` → `Departed`. Taken by the `signed-out` outcome of `gatepass.visit.SignOutVisitor`.
func (v VisitInOnSite) Depart() VisitInDeparted {
	return VisitInDeparted{data: v.data}
}

// AnyVisit is an instance of `gatepass.visit.Visit` in whichever declared state it was found.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type AnyVisit interface {
	isAnyVisit()

	// State is the state this instance rests in.
	State() VisitState

	// Snapshot is this instance at a boundary.
	Snapshot() VisitSnapshot
}

// VisitSnapshot is `gatepass.visit.Visit` as it crosses a boundary: the state as a value beside the data.
//
// Wire and storage know states only at runtime; [VisitSnapshot.Refine] is the one door back into
// the typed lifecycle.
type VisitSnapshot struct {
	// State is where the instance is in its lifecycle.
	State VisitState
	// Data is what it holds.
	Data VisitData
}

// Refine refines the runtime state into the typed one.
//
// Rust's is total, and this one cannot be: the state is a sealed interface, whose zero
// value is nil and names no declared state, so a snapshot nothing constructed reaches here.
// `ok` is false for exactly that snapshot and for no other — every declared state has an
// arm (see TARGET.md).
func (v VisitSnapshot) Refine() (AnyVisit, bool) {
	switch v.State.(type) {
	case VisitStateDeparted:
		return VisitInDeparted{data: v.Data}, true
	case VisitStateExpected:
		return VisitInExpected{data: v.Data}, true
	case VisitStateOnSite:
		return VisitInOnSite{data: v.Data}, true
	}
	return nil, false
}

// AdmitVisitor is Admit the visitor — the input of `gatepass.visit.AdmitVisitor`.
//
// Everything it can result in is [AdmitVisitorOutcome].
type AdmitVisitor struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
	// Badge is `badge` — `gatepass.visit.Badge`.
	Badge Badge
}

// AdmitVisitorOutcome is everything `gatepass.visit.AdmitVisitor` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type AdmitVisitorOutcome interface {
	isAdmitVisitorOutcome()
}

// AdmitVisitorOutcomeAdmitted is `admitted` — otherwise.
//
// The visitor is on site, holding the badge that was printed.
type AdmitVisitorOutcomeAdmitted struct {
	// VisitorAdmitted is the `gatepass.visit.VisitorAdmitted` this outcome publishes.
	VisitorAdmitted VisitorAdmitted
}

func (AdmitVisitorOutcomeAdmitted) isAdmitVisitorOutcome() {}

// AdmitVisitorOutcomeWrongState is `wrong-state` — from a state no declared move starts in.
//
// The visit is not Expected, so nobody was admitted.
type AdmitVisitorOutcomeWrongState struct {
	// Error is why it was refused: `gatepass.visit.VisitStateConflict`.
	Error VisitStateConflict
}

func (AdmitVisitorOutcomeWrongState) isAdmitVisitorOutcome() {}

// RegisterVisit is Register a visit — the input of `gatepass.visit.RegisterVisit`.
//
// Everything it can result in is [RegisterVisitOutcome].
type RegisterVisit struct {
	// Visitor is `visitor` — `gatepass.visit.VisitorName`.
	Visitor VisitorName
	// Building is `building` — `gatepass.visit.Building`.
	Building Building
	// Host is `host` — `gatepass.visit.Host`.
	Host Host
	// ExpectedMinutes is `expected_minutes` — `Integer`.
	ExpectedMinutes int64
	// ExpectedStay is `expected_stay` — `Duration`.
	ExpectedStay primitives.Duration
	// Deposit is `deposit` — `gatepass.visit.Deposit`.
	Deposit Deposit
	// Escorts is `escorts` — `List<gatepass.visit.VisitorName>`.
	Escorts []VisitorName
	// Notes is `notes` — `Map<String, String>`.
	Notes map[string]string
	// OnWatchlist is `on_watchlist` — `Boolean`.
	OnWatchlist bool
}

// RegisterVisitOutcome is everything `gatepass.visit.RegisterVisit` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type RegisterVisitOutcome interface {
	isRegisterVisitOutcome()
}

// RegisterVisitOutcomeRegistered is `registered` — when `expected_minutes > 0`.
//
// The visit is recorded, and the visitor is Expected.
type RegisterVisitOutcomeRegistered struct {
	// VisitRegistered is the `gatepass.visit.VisitRegistered` this outcome publishes.
	VisitRegistered VisitRegistered
}

func (RegisterVisitOutcomeRegistered) isRegisterVisitOutcome() {}

// RegisterVisitOutcomeRefused is `refused` — otherwise.
//
// The expected length was not positive, and nothing was recorded.
type RegisterVisitOutcomeRefused struct {
	// Error is why it was refused: `gatepass.visit.InvalidVisitLength`.
	Error InvalidVisitLength
}

func (RegisterVisitOutcomeRefused) isRegisterVisitOutcome() {}

// SignOutVisitor is Sign the visitor out — the input of `gatepass.visit.SignOutVisitor`.
//
// Everything it can result in is [SignOutVisitorOutcome].
type SignOutVisitor struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
}

// SignOutVisitorOutcome is everything `gatepass.visit.SignOutVisitor` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type SignOutVisitorOutcome interface {
	isSignOutVisitorOutcome()
}

// SignOutVisitorOutcomeSignedOut is `signed-out` — otherwise.
//
// The visitor has left the building.
type SignOutVisitorOutcomeSignedOut struct {
	// VisitorDeparted is the `gatepass.visit.VisitorDeparted` this outcome publishes.
	VisitorDeparted VisitorDeparted
}

func (SignOutVisitorOutcomeSignedOut) isSignOutVisitorOutcome() {}

// SignOutVisitorOutcomeWrongState is `wrong-state` — from a state no declared move starts in.
//
// The visit is not OnSite, so nobody was signed out.
type SignOutVisitorOutcomeWrongState struct {
	// Error is why it was refused: `gatepass.visit.VisitStateConflict`.
	Error VisitStateConflict
}

func (SignOutVisitorOutcomeWrongState) isSignOutVisitorOutcome() {}

// VisitRegistered is VisitRegistered — the event `gatepass.visit.VisitRegistered`.
type VisitRegistered struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
	// Visitor is `visitor` — `gatepass.visit.VisitorName`.
	Visitor VisitorName
	// Building is `building` — `gatepass.visit.Building`.
	Building Building
}

// VisitorAdmitted is VisitorAdmitted — the event `gatepass.visit.VisitorAdmitted`.
type VisitorAdmitted struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
	// Badge is `badge` — `gatepass.visit.Badge`.
	Badge Badge
}

// VisitorDeparted is VisitorDeparted — the event `gatepass.visit.VisitorDeparted`.
type VisitorDeparted struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
}

// InvalidVisitLength is the declared error `gatepass.visit.InvalidVisitLength`.
//
// The expected length of the visit is not a positive number of minutes.
type InvalidVisitLength struct {
	// Submitted is `submitted` — `Integer`.
	Submitted int64
}

// VisitStateConflict is the declared error `gatepass.visit.VisitStateConflict`.
//
// The visit is not in a state this command acts from, so nothing moved.
type VisitStateConflict struct {
	// State is `state` — `gatepass.visit.Visit.State`.
	State VisitState
}

// ExpectedVisits is Expected visits — one row of the view `gatepass.visit.ExpectedVisits`.
//
// Projects `gatepass.visit.Visit` at `read_your_writes` consistency, containing instances where `state == Expected`.
// Serving it is an implementation obligation — see the plan — because how a projection is
// kept current is a storage decision the specification does not take.
type ExpectedVisits struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
	// Visitor is `visitor` — `gatepass.visit.VisitorName`.
	Visitor VisitorName
	// Building is `building` — `gatepass.visit.Building`.
	Building Building
	// Deposit is `deposit` — `gatepass.visit.Deposit`.
	Deposit Deposit
}

// VisitById is Visit by id — one row of the view `gatepass.visit.VisitById`.
//
// Projects `gatepass.visit.Visit` at `eventual` consistency.
// Serving it is an implementation obligation — see the plan — because how a projection is
// kept current is a storage decision the specification does not take.
type VisitById struct {
	// VisitId is `visit_id` — `gatepass.visit.VisitId`.
	VisitId VisitId
	// Visitor is `visitor` — `gatepass.visit.VisitorName`.
	Visitor VisitorName
	// Host is `host` — `gatepass.visit.Host`.
	Host Host
	// Escorts is `escorts` — `List<gatepass.visit.VisitorName>`.
	Escorts []VisitorName
	// Notes is `notes` — `Map<String, String>`.
	Notes map[string]string
	// Badge is `badge` — `Optional<gatepass.visit.Badge>`.
	Badge *Badge
}

// AdmitVisitorBehavior is the behaviour `gatepass.visit.AdmitVisitor` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `gatepass.visit.AdmitVisitor` input, decide and enact exactly one outcome — `admitted` otherwise, takes `arrive` of `gatepass.visit.Visit`, emits `gatepass.visit.VisitorAdmitted`; `wrong-state` from a state no declared move starts in, error `gatepass.visit.VisitStateConflict`.
type AdmitVisitorBehavior interface {
	// AdmitVisitor decides and enacts exactly one declared outcome of `gatepass.visit.AdmitVisitor`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	AdmitVisitor(input AdmitVisitor) (AdmitVisitorOutcome, *obligation.UnmetObligation)
}

// RegisterVisitBehavior is the behaviour `gatepass.visit.RegisterVisit` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `gatepass.visit.RegisterVisit` input, decide and enact exactly one outcome — `registered` when `expected_minutes > 0`, creates `gatepass.visit.Visit`, emits `gatepass.visit.VisitRegistered`; `refused` otherwise, error `gatepass.visit.InvalidVisitLength`.
type RegisterVisitBehavior interface {
	// RegisterVisit decides and enacts exactly one declared outcome of `gatepass.visit.RegisterVisit`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	RegisterVisit(input RegisterVisit) (RegisterVisitOutcome, *obligation.UnmetObligation)
}

// SignOutVisitorBehavior is the behaviour `gatepass.visit.SignOutVisitor` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `gatepass.visit.SignOutVisitor` input, decide and enact exactly one outcome — `signed-out` otherwise, takes `depart` of `gatepass.visit.Visit`, emits `gatepass.visit.VisitorDeparted`; `wrong-state` from a state no declared move starts in, error `gatepass.visit.VisitStateConflict`.
type SignOutVisitorBehavior interface {
	// SignOutVisitor decides and enacts exactly one declared outcome of `gatepass.visit.SignOutVisitor`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	SignOutVisitor(input SignOutVisitor) (SignOutVisitorOutcome, *obligation.UnmetObligation)
}

// ExpectedVisitsQuery is the query `gatepass.visit.ExpectedVisits` — an implementation obligation.
//
// Why it is not generated: how the projection is kept current is a storage decision.
//
// Contract: a query answering `gatepass.visit.ExpectedVisits` with rows projected from `gatepass.visit.Visit` at `read_your_writes` consistency, containing instances where `state == Expected`.
type ExpectedVisitsQuery interface {
	// ExpectedVisits serves `gatepass.visit.ExpectedVisits` rows at the view's declared consistency.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	ExpectedVisits() ([]ExpectedVisits, *obligation.UnmetObligation)
}

// VisitByIdQuery is the query `gatepass.visit.VisitById` — an implementation obligation.
//
// Why it is not generated: how the projection is kept current is a storage decision.
//
// Contract: a query answering `gatepass.visit.VisitById` with rows projected from `gatepass.visit.Visit` at `eventual` consistency.
type VisitByIdQuery interface {
	// VisitById serves `gatepass.visit.VisitById` rows at the view's declared consistency.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	VisitById() ([]VisitById, *obligation.UnmetObligation)
}

// Unimplemented satisfies every obligation of this bounded context by refusing in the type
// system.
//
// Each method returns the typed refusal naming what is owed — never a panic, never a
// guessed value — so a module built on this stub compiles and reports its own gaps.
type Unimplemented struct{}

// AdmitVisitor refuses: the behaviour `gatepass.visit.AdmitVisitor` — an implementation obligation.
func (Unimplemented) AdmitVisitor(input AdmitVisitor) (AdmitVisitorOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "gatepass.visit.AdmitVisitor"}
}

// RegisterVisit refuses: the behaviour `gatepass.visit.RegisterVisit` — an implementation obligation.
func (Unimplemented) RegisterVisit(input RegisterVisit) (RegisterVisitOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "gatepass.visit.RegisterVisit"}
}

// SignOutVisitor refuses: the behaviour `gatepass.visit.SignOutVisitor` — an implementation obligation.
func (Unimplemented) SignOutVisitor(input SignOutVisitor) (SignOutVisitorOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "gatepass.visit.SignOutVisitor"}
}

// ExpectedVisits refuses: the query `gatepass.visit.ExpectedVisits` — an implementation obligation.
func (Unimplemented) ExpectedVisits() ([]ExpectedVisits, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "view query", Source: "gatepass.visit.ExpectedVisits"}
}

// VisitById refuses: the query `gatepass.visit.VisitById` — an implementation obligation.
func (Unimplemented) VisitById() ([]VisitById, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "view query", Source: "gatepass.visit.VisitById"}
}
