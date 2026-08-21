// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package passservice is pass-service — the `pass-service` component of `gatepass` v1.
//
// Holds every expected, present and departed visit for one site.
//
// The component's outer surface exactly as the specification declares it: accepted commands as
// methods, declared views as queries, published events as a typed outbox. The behaviour behind
// every handler is an implementation obligation — see the PLAN.md beside this module — and
// until one is satisfied, its stub answers with a typed refusal naming what is owed.
package passservice

import (
	"example.invalid/gatepass/types/obligation"
	"example.invalid/gatepass/types/visit"
)

// Behaviors bundles every behaviour and query this component owes.
//
// Constructing the port over each bounded context's `Unimplemented` yields a component
// that compiles and refuses, in the type system, everything not yet implemented.
type Behaviors interface {
	visit.AdmitVisitorBehavior
	visit.RegisterVisitBehavior
	visit.SignOutVisitorBehavior
	visit.ExpectedVisitsQuery
	visit.VisitByIdQuery
}

// PublishedEvent is an event this component declares it publishes, on its way to the system's
// transport.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type PublishedEvent interface {
	isPublishedEvent()
}

// PublishedEventVisitRegistered is `gatepass.visit.VisitRegistered`.
type PublishedEventVisitRegistered struct {
	// Event is what was published.
	Event visit.VisitRegistered
}

func (PublishedEventVisitRegistered) isPublishedEvent() {}

// PublishedEventVisitorAdmitted is `gatepass.visit.VisitorAdmitted`.
type PublishedEventVisitorAdmitted struct {
	// Event is what was published.
	Event visit.VisitorAdmitted
}

func (PublishedEventVisitorAdmitted) isPublishedEvent() {}

// PublishedEventVisitorDeparted is `gatepass.visit.VisitorDeparted`.
type PublishedEventVisitorDeparted struct {
	// Event is what was published.
	Event visit.VisitorDeparted
}

func (PublishedEventVisitorDeparted) isPublishedEvent() {}

// PassService is pass-service — the port over the component's obligations.
//
// The behaviours and the outbox are unexported: commands enter through the methods below,
// and the system's transport is the only thing that drains what they published.
type PassService struct {
	// behaviors is everything this component owes.
	behaviors Behaviors
	// outbox holds what has been published since the last drain.
	outbox []PublishedEvent
}

// New builds a port over the given obligation implementations.
func New(behaviors Behaviors) *PassService {
	return &PassService{behaviors: behaviors}
}

// DrainOutbox hands over everything published since the last drain, in publication order.
//
// The system's transport calls this; anything else reading it is taking events the
// transport will then never deliver.
func (c *PassService) DrainOutbox() []PublishedEvent {
	drained := c.outbox
	c.outbox = nil
	return drained
}

// AdmitVisitor accepts `gatepass.visit.AdmitVisitor`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *PassService) AdmitVisitor(input visit.AdmitVisitor) (visit.AdmitVisitorOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.AdmitVisitor(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case visit.AdmitVisitorOutcomeAdmitted:
		c.outbox = append(c.outbox, PublishedEventVisitorAdmitted{Event: value.VisitorAdmitted})
	case visit.AdmitVisitorOutcomeWrongState:
	}
	return outcome, nil
}

// RegisterVisit accepts `gatepass.visit.RegisterVisit`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *PassService) RegisterVisit(input visit.RegisterVisit) (visit.RegisterVisitOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.RegisterVisit(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case visit.RegisterVisitOutcomeRegistered:
		c.outbox = append(c.outbox, PublishedEventVisitRegistered{Event: value.VisitRegistered})
	case visit.RegisterVisitOutcomeRefused:
	}
	return outcome, nil
}

// SignOutVisitor accepts `gatepass.visit.SignOutVisitor`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *PassService) SignOutVisitor(input visit.SignOutVisitor) (visit.SignOutVisitorOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.SignOutVisitor(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case visit.SignOutVisitorOutcomeSignedOut:
		c.outbox = append(c.outbox, PublishedEventVisitorDeparted{Event: value.VisitorDeparted})
	case visit.SignOutVisitorOutcomeWrongState:
	}
	return outcome, nil
}

// ExpectedVisits serves `gatepass.visit.ExpectedVisits` at `read_your_writes` consistency, from the owed projection.
func (c *PassService) ExpectedVisits() ([]visit.ExpectedVisits, *obligation.UnmetObligation) {
	return c.behaviors.ExpectedVisits()
}

// VisitById serves `gatepass.visit.VisitById` at `eventual` consistency, from the owed projection.
func (c *PassService) VisitById() ([]visit.VisitById, *obligation.UnmetObligation) {
	return c.behaviors.VisitById()
}
