// Package realization is the hand-written half of the synthesised gatepass Go module.
//
// `generated/go/gatepass/` holds the types, the typestate lifecycle, the component port, the system
// and the HTTP surface — everything the specification determines. It holds no behaviour: every
// command's decision and every projection is an obligation the plan names, and a module built on
// the generated stubs compiles and answers each of them with a typed refusal.
//
// This module is the other half, and it is deliberately *not* a translation of
// `examples/gatepass-realization/`. Both were written from the same specification, in the language
// of the tree each links into, and the demonstration is that the two answer the same requests the
// same way: `cargo xtask synth --check` starts both and holds them to it.
//
// # No clock, no randomness
//
// Identifiers come from a per-store counter in the Uuid wire shape, for the reason the Rust half
// gives: two processes synthesised from one specification are started side by side and their
// answers compared, so an identifier from a random source would make the two disagree about a
// value neither of them chose.
package realization

import (
	"fmt"
	"sort"

	"example.invalid/gatepass/types/obligation"
	"example.invalid/gatepass/types/primitives"
	"example.invalid/gatepass/types/visit"
)

// Store is what one run's visits amount to: every snapshot, and the identifier mint.
//
// Keyed by the identifier's wire rendering and read back in sorted order, so the projections below
// answer in a stable order — the same order the Rust realization's BTreeMap gives, which is what
// lets two processes be compared row by row.
type Store struct {
	visits   map[string]visit.VisitSnapshot
	sequence int64
}

// NewStore is an empty store.
func NewStore() *Store {
	return &Store{visits: map[string]visit.VisitSnapshot{}}
}

// identifier is a fresh identity in the Uuid wire shape, from the counter rather than randomness.
func (s *Store) identifier() visit.VisitId {
	s.sequence++
	return visit.NewVisitId(primitives.NewUuid(fmt.Sprintf("00000000-0000-4000-8000-%012d", s.sequence)))
}

// keys is every stored identifier, in wire order.
func (s *Store) keys() []string {
	out := make([]string, 0, len(s.visits))
	for key := range s.visits {
		out = append(out, key)
	}
	sort.Strings(out)
	return out
}

// Realization is the honest implementation of every visit obligation, over one shared store.
//
// One type implements all five interfaces; the linker still resolves each obligation separately
// (D-2), and this is merely who answers.
type Realization struct {
	store *Store
}

// Over is the realization, answering over store.
func Over(store *Store) *Realization {
	return &Realization{store: store}
}

// unknownSubject is the one answer the generated seam cannot spell, refused loudly rather than
// guessed.
//
// A command naming a visit that was never registered has no declared outcome: `wrong-state`
// demands the VisitStateConflict state the visit is really in, and a visit that does not exist does
// not have one. Fabricating a state would be manufacturing an observation, so the honest total
// answer is the typed refusal — which the served surface reports as 501, naming the obligation.
// That it has to is a gap in the *model*: the specification language has no way to declare "no such
// subject", and the Rust half of this realization records the same finding.
func unknownSubject(source string) *obligation.UnmetObligation {
	return &obligation.UnmetObligation{Capability: "command behaviour", Source: source}
}

// RegisterVisit decides and enacts exactly one declared outcome of `gatepass.visit.RegisterVisit`.
func (r *Realization) RegisterVisit(input visit.RegisterVisit) (visit.RegisterVisitOutcome, *obligation.UnmetObligation) {
	// The declared guard, first and alone: `registered` when `expected_minutes > 0`, `refused`
	// otherwise. Nothing else about the input can refuse a registration.
	if input.ExpectedMinutes <= 0 {
		return visit.RegisterVisitOutcomeRefused{
			Error: visit.InvalidVisitLength{Submitted: input.ExpectedMinutes},
		}, nil
	}
	visitID := r.store.identifier()
	// What the command does not determine, the realization decides and says so: there is no badge
	// until one is printed at the desk, which is what AdmitVisitor carries.
	registered := visit.NewVisit(visit.VisitData{
		VisitId:         visitID,
		Visitor:         input.Visitor,
		Building:        input.Building,
		Host:            input.Host,
		ExpectedMinutes: input.ExpectedMinutes,
		ExpectedStay:    input.ExpectedStay,
		Deposit:         input.Deposit,
		Escorts:         input.Escorts,
		Notes:           input.Notes,
		Badge:           nil,
		OnWatchlist:     input.OnWatchlist,
	})
	r.store.visits[visitID.Value().Value()] = registered.Snapshot()
	return visit.RegisterVisitOutcomeRegistered{
		VisitRegistered: visit.VisitRegistered{
			VisitId:  visitID,
			Visitor:  input.Visitor,
			Building: input.Building,
		},
	}, nil
}

// AdmitVisitor decides and enacts exactly one declared outcome of `gatepass.visit.AdmitVisitor`.
func (r *Realization) AdmitVisitor(input visit.AdmitVisitor) (visit.AdmitVisitorOutcome, *obligation.UnmetObligation) {
	key := input.VisitId.Value().Value()
	snapshot, held := r.store.visits[key]
	if !held {
		return nil, unknownSubject("gatepass.visit.AdmitVisitor")
	}
	resting, ok := snapshot.Refine()
	if !ok {
		return nil, unknownSubject("gatepass.visit.AdmitVisitor")
	}
	// `arrive` runs from `Expected` and from nowhere else — the typed lifecycle carries that, so
	// the legal move is a method call and every other state is the declared `wrong-state`.
	expected, ok := resting.(visit.VisitInExpected)
	if !ok {
		return visit.AdmitVisitorOutcomeWrongState{
			Error: visit.VisitStateConflict{State: resting.State()},
		}, nil
	}
	data := expected.Arrive().Data()
	badge := input.Badge
	data.Badge = &badge
	r.store.visits[key] = visit.VisitSnapshot{State: visit.VisitStateOnSite{}, Data: data}
	return visit.AdmitVisitorOutcomeAdmitted{
		VisitorAdmitted: visit.VisitorAdmitted{VisitId: input.VisitId, Badge: input.Badge},
	}, nil
}

// SignOutVisitor decides and enacts exactly one declared outcome of
// `gatepass.visit.SignOutVisitor`.
func (r *Realization) SignOutVisitor(input visit.SignOutVisitor) (visit.SignOutVisitorOutcome, *obligation.UnmetObligation) {
	key := input.VisitId.Value().Value()
	snapshot, held := r.store.visits[key]
	if !held {
		return nil, unknownSubject("gatepass.visit.SignOutVisitor")
	}
	resting, ok := snapshot.Refine()
	if !ok {
		return nil, unknownSubject("gatepass.visit.SignOutVisitor")
	}
	onSite, ok := resting.(visit.VisitInOnSite)
	if !ok {
		return visit.SignOutVisitorOutcomeWrongState{
			Error: visit.VisitStateConflict{State: resting.State()},
		}, nil
	}
	r.store.visits[key] = onSite.Depart().Snapshot()
	return visit.SignOutVisitorOutcomeSignedOut{
		VisitorDeparted: visit.VisitorDeparted{VisitId: input.VisitId},
	}, nil
}

// ExpectedVisits serves the declared filter, applied: `state == Expected` and nothing else.
//
// Read straight off the store, which is what read_your_writes obliges — a receptionist who has
// just registered a visitor and cannot see them here has been told a lie about what they did.
func (r *Realization) ExpectedVisits() ([]visit.ExpectedVisits, *obligation.UnmetObligation) {
	rows := make([]visit.ExpectedVisits, 0, len(r.store.visits))
	for _, key := range r.store.keys() {
		snapshot := r.store.visits[key]
		if _, expected := snapshot.State.(visit.VisitStateExpected); !expected {
			continue
		}
		rows = append(rows, visit.ExpectedVisits{
			VisitId:  snapshot.Data.VisitId,
			Visitor:  snapshot.Data.Visitor,
			Building: snapshot.Data.Building,
			Deposit:  snapshot.Data.Deposit,
		})
	}
	return rows, nil
}

// VisitById serves every visit, projected to its declared row.
//
// Served current rather than lagging: `eventual` is an upper bound on staleness, and a projection
// read straight off the store satisfies it.
func (r *Realization) VisitById() ([]visit.VisitById, *obligation.UnmetObligation) {
	rows := make([]visit.VisitById, 0, len(r.store.visits))
	for _, key := range r.store.keys() {
		snapshot := r.store.visits[key]
		rows = append(rows, visit.VisitById{
			VisitId: snapshot.Data.VisitId,
			Visitor: snapshot.Data.Visitor,
			Host:    snapshot.Data.Host,
			Escorts: snapshot.Data.Escorts,
			Notes:   snapshot.Data.Notes,
			Badge:   snapshot.Data.Badge,
		})
	}
	return rows, nil
}
