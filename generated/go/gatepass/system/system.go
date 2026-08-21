// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package system is the `gatepass` system, v1: its components assembled, its bindings wired, and its one
// transport.
//
// The transport is derived from the specification, not chosen: `at_least_once` is the only
// delivery guarantee the model declares, so published events land on an append-only log and
// a pump delivers each to every binding that reacts to it. The log is the system's observable
// record, and so is the record of what each binding invoked. What no specification
// determines — how an escalation event is filled, behaviour behind the ports — stays an
// obligation; see the PLAN.md beside this module.
package system

import (
	"example.invalid/gatepass/components/passservice"
	"example.invalid/gatepass/types/obligation"
	"example.invalid/gatepass/types/visit"
)

// SystemEvent is an event on the system's log: everything any component publishes, and everything
// a binding escalates into.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type SystemEvent interface {
	isSystemEvent()
}

// SystemEventVisitRegistered is `gatepass.visit.VisitRegistered`.
type SystemEventVisitRegistered struct {
	// Event is what was published.
	Event visit.VisitRegistered
}

func (SystemEventVisitRegistered) isSystemEvent() {}

// SystemEventVisitorAdmitted is `gatepass.visit.VisitorAdmitted`.
type SystemEventVisitorAdmitted struct {
	// Event is what was published.
	Event visit.VisitorAdmitted
}

func (SystemEventVisitorAdmitted) isSystemEvent() {}

// SystemEventVisitorDeparted is `gatepass.visit.VisitorDeparted`.
type SystemEventVisitorDeparted struct {
	// Event is what was published.
	Event visit.VisitorDeparted
}

func (SystemEventVisitorDeparted) isSystemEvent() {}

// liftFromPassService lifts one of `pass-service`'s published events onto the system's log.
//
// `nil` where the value is a variant this module did not declare, which only Go's zero value
// can produce: the compiler cannot prove the switch below total, and the caller drops what
// it cannot place rather than logging a nil occurrence.
func liftFromPassService(event passservice.PublishedEvent) SystemEvent {
	switch value := event.(type) {
	case passservice.PublishedEventVisitRegistered:
		return SystemEventVisitRegistered{Event: value.Event}
	case passservice.PublishedEventVisitorAdmitted:
		return SystemEventVisitorAdmitted{Event: value.Event}
	case passservice.PublishedEventVisitorDeparted:
		return SystemEventVisitorDeparted{Event: value.Event}
	}
	return nil
}

// System is the `gatepass` system: every component behind its port, and the transport between them.
//
// The component fields are exported because commands enter the system through a component's
// own port; the log and its delivery cursor are not, because publishing happens by pumping,
// not by writing history directly.
type System struct {
	// PassService is the `pass-service` component.
	PassService *passservice.PassService
	// published is the log, in publication order.
	published []SystemEvent
	// cursor is how far the pump has delivered.
	cursor int
}

// NewSystem assembles the system from its components.
func NewSystem(passService *passservice.PassService) *System {
	return &System{
		// PassService is the `pass-service` component's port.
		PassService: passService,
	}
}

// Published is everything published so far, in publication order — the system's observable
// record.
func (s *System) Published() []SystemEvent {
	return s.published
}

// Pump delivers until quiescent: collects every component's outbox onto the log, then delivers
// each logged event to every binding that reacts to it — at least once each, which is the
// guarantee the specification declares.
//
// The result carries the first unmet obligation that delivery could not route around; the log
// keeps everything already published. A specification whose bindings feed each other without
// end will not quiesce, and this pump will not pretend otherwise.
func (s *System) Pump() *obligation.UnmetObligation {
	for {
		s.collect()
		if s.cursor == len(s.published) {
			return nil
		}
		s.cursor++
	}
}

// collect moves every component's outbox onto the log, in component order.
func (s *System) collect() {
	for _, published := range s.PassService.DrainOutbox() {
		if lifted := liftFromPassService(published); lifted != nil {
			s.published = append(s.published, lifted)
		}
	}
}
