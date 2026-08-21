// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package invoiceservice is invoice-service — the `invoice-service` component of `billing` v3.
//
// Issues invoices and tracks payment.
//
// The component's outer surface exactly as the specification declares it: accepted commands as
// methods, declared views as queries, published events as a typed outbox. The behaviour behind
// every handler is an implementation obligation — see the PLAN.md beside this module — and
// until one is satisfied, its stub answers with a typed refusal naming what is owed.
package invoiceservice

import (
	"example.invalid/billing/types/invoice"
	"example.invalid/billing/types/obligation"
)

// Behaviors bundles every behaviour and query this component owes.
//
// Constructing the port over each bounded context's `Unimplemented` yields a component
// that compiles and refuses, in the type system, everything not yet implemented.
type Behaviors interface {
	invoice.CancelInvoiceBehavior
	invoice.CreateInvoiceBehavior
	invoice.IssueInvoiceBehavior
	invoice.PayInvoiceBehavior
	invoice.InvoiceByIdQuery
	invoice.OutstandingInvoicesQuery
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

// PublishedEventInvoiceCancelled is `billing.invoice.InvoiceCancelled`.
type PublishedEventInvoiceCancelled struct {
	// Event is what was published.
	Event invoice.InvoiceCancelled
}

func (PublishedEventInvoiceCancelled) isPublishedEvent() {}

// PublishedEventInvoiceCreated is `billing.invoice.InvoiceCreated`.
type PublishedEventInvoiceCreated struct {
	// Event is what was published.
	Event invoice.InvoiceCreated
}

func (PublishedEventInvoiceCreated) isPublishedEvent() {}

// PublishedEventInvoiceIssued is `billing.invoice.InvoiceIssued`.
type PublishedEventInvoiceIssued struct {
	// Event is what was published.
	Event invoice.InvoiceIssued
}

func (PublishedEventInvoiceIssued) isPublishedEvent() {}

// PublishedEventInvoicePaid is `billing.invoice.InvoicePaid`.
type PublishedEventInvoicePaid struct {
	// Event is what was published.
	Event invoice.InvoicePaid
}

func (PublishedEventInvoicePaid) isPublishedEvent() {}

// InvoiceService is invoice-service — the port over the component's obligations.
//
// The behaviours and the outbox are unexported: commands enter through the methods below,
// and the system's transport is the only thing that drains what they published.
type InvoiceService struct {
	// behaviors is everything this component owes.
	behaviors Behaviors
	// outbox holds what has been published since the last drain.
	outbox []PublishedEvent
}

// New builds a port over the given obligation implementations.
func New(behaviors Behaviors) *InvoiceService {
	return &InvoiceService{behaviors: behaviors}
}

// DrainOutbox hands over everything published since the last drain, in publication order.
//
// The system's transport calls this; anything else reading it is taking events the
// transport will then never deliver.
func (c *InvoiceService) DrainOutbox() []PublishedEvent {
	drained := c.outbox
	c.outbox = nil
	return drained
}

// CancelInvoice accepts `billing.invoice.CancelInvoice`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *InvoiceService) CancelInvoice(input invoice.CancelInvoice) (invoice.CancelInvoiceOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.CancelInvoice(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case invoice.CancelInvoiceOutcomeCancelled:
		c.outbox = append(c.outbox, PublishedEventInvoiceCancelled{Event: value.InvoiceCancelled})
	case invoice.CancelInvoiceOutcomeWrongState:
	}
	return outcome, nil
}

// CreateInvoice accepts `billing.invoice.CreateInvoice`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *InvoiceService) CreateInvoice(input invoice.CreateInvoice) (invoice.CreateInvoiceOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.CreateInvoice(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case invoice.CreateInvoiceOutcomeAccepted:
		c.outbox = append(c.outbox, PublishedEventInvoiceCreated{Event: value.InvoiceCreated})
	case invoice.CreateInvoiceOutcomeRejected:
	}
	return outcome, nil
}

// IssueInvoice accepts `billing.invoice.IssueInvoice`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *InvoiceService) IssueInvoice(input invoice.IssueInvoice) (invoice.IssueInvoiceOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.IssueInvoice(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case invoice.IssueInvoiceOutcomeIssued:
		c.outbox = append(c.outbox, PublishedEventInvoiceIssued{Event: value.InvoiceIssued})
	case invoice.IssueInvoiceOutcomeWrongState:
	}
	return outcome, nil
}

// PayInvoice accepts `billing.invoice.PayInvoice`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *InvoiceService) PayInvoice(input invoice.PayInvoice) (invoice.PayInvoiceOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.PayInvoice(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case invoice.PayInvoiceOutcomeSettled:
		c.outbox = append(c.outbox, PublishedEventInvoicePaid{Event: value.InvoicePaid})
	case invoice.PayInvoiceOutcomeRejected:
	case invoice.PayInvoiceOutcomeWrongState:
	}
	return outcome, nil
}

// InvoiceById serves `billing.invoice.InvoiceById` at `eventual` consistency, from the owed projection.
func (c *InvoiceService) InvoiceById() ([]invoice.InvoiceById, *obligation.UnmetObligation) {
	return c.behaviors.InvoiceById()
}

// OutstandingInvoices serves `billing.invoice.OutstandingInvoices` at `read_your_writes` consistency, from the owed projection.
func (c *InvoiceService) OutstandingInvoices() ([]invoice.OutstandingInvoices, *obligation.UnmetObligation) {
	return c.behaviors.OutstandingInvoices()
}
