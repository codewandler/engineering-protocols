// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package system is the `billing` system, v3: its components assembled, its bindings wired, and its one
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
	"example.invalid/billing/components/emailservice"
	"example.invalid/billing/components/invoiceservice"
	"example.invalid/billing/types/email"
	"example.invalid/billing/types/invoice"
	"example.invalid/billing/types/obligation"
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

// SystemEventDeliveryEscalated is `billing.email.DeliveryEscalated`.
type SystemEventDeliveryEscalated struct {
	// Event is what was published.
	Event email.DeliveryEscalated
}

func (SystemEventDeliveryEscalated) isSystemEvent() {}

// SystemEventEmailSent is `billing.email.EmailSent`.
type SystemEventEmailSent struct {
	// Event is what was published.
	Event email.EmailSent
}

func (SystemEventEmailSent) isSystemEvent() {}

// SystemEventInvoiceCancelled is `billing.invoice.InvoiceCancelled`.
type SystemEventInvoiceCancelled struct {
	// Event is what was published.
	Event invoice.InvoiceCancelled
}

func (SystemEventInvoiceCancelled) isSystemEvent() {}

// SystemEventInvoiceCreated is `billing.invoice.InvoiceCreated`.
type SystemEventInvoiceCreated struct {
	// Event is what was published.
	Event invoice.InvoiceCreated
}

func (SystemEventInvoiceCreated) isSystemEvent() {}

// SystemEventInvoiceIssued is `billing.invoice.InvoiceIssued`.
type SystemEventInvoiceIssued struct {
	// Event is what was published.
	Event invoice.InvoiceIssued
}

func (SystemEventInvoiceIssued) isSystemEvent() {}

// SystemEventInvoicePaid is `billing.invoice.InvoicePaid`.
type SystemEventInvoicePaid struct {
	// Event is what was published.
	Event invoice.InvoicePaid
}

func (SystemEventInvoicePaid) isSystemEvent() {}

// liftFromEmailService lifts one of `email-service`'s published events onto the system's log.
//
// `nil` where the value is a variant this module did not declare, which only Go's zero value
// can produce: the compiler cannot prove the switch below total, and the caller drops what
// it cannot place rather than logging a nil occurrence.
func liftFromEmailService(event emailservice.PublishedEvent) SystemEvent {
	switch value := event.(type) {
	case emailservice.PublishedEventDeliveryEscalated:
		return SystemEventDeliveryEscalated{Event: value.Event}
	case emailservice.PublishedEventEmailSent:
		return SystemEventEmailSent{Event: value.Event}
	}
	return nil
}

// liftFromInvoiceService lifts one of `invoice-service`'s published events onto the system's log.
//
// `nil` where the value is a variant this module did not declare, which only Go's zero value
// can produce: the compiler cannot prove the switch below total, and the caller drops what
// it cannot place rather than logging a nil occurrence.
func liftFromInvoiceService(event invoiceservice.PublishedEvent) SystemEvent {
	switch value := event.(type) {
	case invoiceservice.PublishedEventInvoiceCancelled:
		return SystemEventInvoiceCancelled{Event: value.Event}
	case invoiceservice.PublishedEventInvoiceCreated:
		return SystemEventInvoiceCreated{Event: value.Event}
	case invoiceservice.PublishedEventInvoiceIssued:
		return SystemEventInvoiceIssued{Event: value.Event}
	case invoiceservice.PublishedEventInvoicePaid:
		return SystemEventInvoicePaid{Event: value.Event}
	}
	return nil
}

// BindingInvocation is one command a binding invoked, and the input it passed — the transport's own
// record.
//
// Recorded by the pump at the moment of invocation, so what a binding actually passed is
// observable from outside — a conformance run holds a mapping to its words with exactly
// this — without instrumenting the component underneath.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type BindingInvocation interface {
	isBindingInvocation()
}

// BindingInvocationNotifyOnInvoiceCreated is `notify-on-invoice-created` invoking `billing.email.SendEmail`.
type BindingInvocationNotifyOnInvoiceCreated struct {
	// Input is what the binding passed.
	Input email.SendEmail
}

func (BindingInvocationNotifyOnInvoiceCreated) isBindingInvocation() {}

// NotifyOnInvoiceCreated reads a `billing.invoice.InvoiceCreated` as `billing.email.SendEmail` input — the binding `notify-on-invoice-created`.
//
// Fully determined by the specification: every input is filled from an event field —
// through the declared crossing where one is named — from a literal the target admits, or
// left absent where the input is optional and the binding says nothing.
func NotifyOnInvoiceCreated(event invoice.InvoiceCreated) email.SendEmail {
	return email.SendEmail{
		// Recipient is read from the event's `customer_email` through the declared crossing.
		Recipient: email.EmailAddressFromBillingInvoiceEmail(event.CustomerEmail),
		// Template is the literal `invoice-created` the binding wrote.
		Template: email.NewTemplateId("invoice-created"),
	}
}

// NotifyOnInvoiceCreatedEscalation is the escalation of `notify-on-invoice-created` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: the declared `billing.email.DeliveryEscalated`, recording that delivering `billing.email.SendEmail` for `notify-on-invoice-created` was given up on — the event is declared; how its fields are filled from the failed invocation is not.
type NotifyOnInvoiceCreatedEscalation interface {
	// NotifyOnInvoiceCreatedEscalation builds the declared `billing.email.DeliveryEscalated` from the invocation that was given up on.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	NotifyOnInvoiceCreatedEscalation(failed email.SendEmail) (email.DeliveryEscalated, *obligation.UnmetObligation)
}

// Unimplemented satisfies every obligation of the system by refusing in the type system.
//
// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
// value — so a system built on this stub compiles and reports its own gaps.
type Unimplemented struct{}

// NotifyOnInvoiceCreatedEscalation refuses: the escalation of `notify-on-invoice-created` — an implementation obligation.
func (Unimplemented) NotifyOnInvoiceCreatedEscalation(failed email.SendEmail) (email.DeliveryEscalated, *obligation.UnmetObligation) {
	return email.DeliveryEscalated{}, &obligation.UnmetObligation{Capability: "binding escalation", Source: "notify-on-invoice-created"}
}

// Obligations is what the system itself owes its implementor: exactly the seams the pump
// calls, bundled.
type Obligations interface {
	NotifyOnInvoiceCreatedEscalation
}

// System is the `billing` system: every component behind its port, and the transport between them.
//
// The component fields are exported because commands enter the system through a component's
// own port; the log and its delivery cursor are not, because publishing happens by pumping,
// not by writing history directly.
type System struct {
	// EmailService is the `email-service` component.
	EmailService *emailservice.EmailService
	// InvoiceService is the `invoice-service` component.
	InvoiceService *invoiceservice.InvoiceService
	// obligations is what nothing in this module can determine.
	obligations Obligations
	// invocations records every command a binding invoked, with what it passed.
	invocations []BindingInvocation
	// published is the log, in publication order.
	published []SystemEvent
	// cursor is how far the pump has delivered.
	cursor int
}

// NewSystem assembles the system from its components and the owed obligations.
func NewSystem(emailService *emailservice.EmailService, invoiceService *invoiceservice.InvoiceService, obligations Obligations) *System {
	return &System{
		// EmailService is the `email-service` component's port.
		EmailService: emailService,
		// InvoiceService is the `invoice-service` component's port.
		InvoiceService: invoiceService,
		// obligations is what the pump calls where the specification determines nothing.
		obligations: obligations,
	}
}

// Published is everything published so far, in publication order — the system's observable
// record.
func (s *System) Published() []SystemEvent {
	return s.published
}

// Invocations is every command a binding invoked so far, in invocation order, with what it
// passed.
func (s *System) Invocations() []BindingInvocation {
	return s.invocations
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
		event := s.published[s.cursor]
		s.cursor++
		if unmet := s.deliver(event); unmet != nil {
			return unmet
		}
	}
}

// Redeliver delivers one already-published occurrence to every binding that reacts to it,
// again, then pumps until quiescent.
//
// The duplicate a delivery guarantee of at least once explicitly permits: the occurrence is
// not published a second time — a second occurrence would be a different claim — but every
// reacting binding runs again, and what that causes lands on the log as usual.
func (s *System) Redeliver(event SystemEvent) *obligation.UnmetObligation {
	if unmet := s.deliver(event); unmet != nil {
		return unmet
	}
	return s.Pump()
}

// collect moves every component's outbox onto the log, in component order.
func (s *System) collect() {
	for _, published := range s.EmailService.DrainOutbox() {
		if lifted := liftFromEmailService(published); lifted != nil {
			s.published = append(s.published, lifted)
		}
	}
	for _, published := range s.InvoiceService.DrainOutbox() {
		if lifted := liftFromInvoiceService(published); lifted != nil {
			s.published = append(s.published, lifted)
		}
	}
}

// deliver delivers one logged event to every binding that reacts to it.
func (s *System) deliver(event SystemEvent) *obligation.UnmetObligation {
	switch value := event.(type) {
	case SystemEventDeliveryEscalated:
	case SystemEventEmailSent:
	case SystemEventInvoiceCancelled:
	case SystemEventInvoiceCreated:
		if unmet := s.deliverNotifyOnInvoiceCreated(value.Event); unmet != nil {
			return unmet
		}
	case SystemEventInvoiceIssued:
	case SystemEventInvoicePaid:
	}
	return nil
}

// deliverNotifyOnInvoiceCreated delivers one `billing.invoice.InvoiceCreated` to `notify-on-invoice-created`: transform, record the invocation, invoke the
// acceptor's port, and answer a declared refusal with the declared policy (at_least_once, on failure
// escalate).
//
// An unmet obligation is deliberately not routed into the policy: a port refusing because
// its behaviour is owed is a fact about the module being unfinished, not about a delivery.
func (s *System) deliverNotifyOnInvoiceCreated(event invoice.InvoiceCreated) *obligation.UnmetObligation {
	input := NotifyOnInvoiceCreated(event)
	s.invocations = append(s.invocations, BindingInvocationNotifyOnInvoiceCreated{Input: input})
	outcome, refused := s.EmailService.SendEmail(input)
	if refused != nil {
		return refused
	}
	switch outcome.(type) {
	case email.SendEmailOutcomeSent:
	case email.SendEmailOutcomeFailed:
		// The declared refusal is the failure the policy names: escalate.
		escalation, owed := s.obligations.NotifyOnInvoiceCreatedEscalation(input)
		if owed != nil {
			return owed
		}
		s.published = append(s.published, SystemEventDeliveryEscalated{Event: escalation})
	}
	return nil
}
