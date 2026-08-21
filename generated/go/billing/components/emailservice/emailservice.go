// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package emailservice is email-service — the `email-service` component of `billing` v3.
//
// Sends what other contexts ask it to send.
//
// The component's outer surface exactly as the specification declares it: accepted commands as
// methods, declared views as queries, published events as a typed outbox. The behaviour behind
// every handler is an implementation obligation — see the PLAN.md beside this module — and
// until one is satisfied, its stub answers with a typed refusal naming what is owed.
package emailservice

import (
	"example.invalid/billing/types/email"
	"example.invalid/billing/types/obligation"
)

// Behaviors bundles every behaviour and query this component owes.
//
// Constructing the port over each bounded context's `Unimplemented` yields a component
// that compiles and refuses, in the type system, everything not yet implemented.
type Behaviors interface {
	email.SendEmailBehavior
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

// PublishedEventDeliveryEscalated is `billing.email.DeliveryEscalated`.
type PublishedEventDeliveryEscalated struct {
	// Event is what was published.
	Event email.DeliveryEscalated
}

func (PublishedEventDeliveryEscalated) isPublishedEvent() {}

// PublishedEventEmailSent is `billing.email.EmailSent`.
type PublishedEventEmailSent struct {
	// Event is what was published.
	Event email.EmailSent
}

func (PublishedEventEmailSent) isPublishedEvent() {}

// EmailService is email-service — the port over the component's obligations.
//
// The behaviours and the outbox are unexported: commands enter through the methods below,
// and the system's transport is the only thing that drains what they published.
type EmailService struct {
	// behaviors is everything this component owes.
	behaviors Behaviors
	// outbox holds what has been published since the last drain.
	outbox []PublishedEvent
}

// New builds a port over the given obligation implementations.
func New(behaviors Behaviors) *EmailService {
	return &EmailService{behaviors: behaviors}
}

// DrainOutbox hands over everything published since the last drain, in publication order.
//
// The system's transport calls this; anything else reading it is taking events the
// transport will then never deliver.
func (c *EmailService) DrainOutbox() []PublishedEvent {
	drained := c.outbox
	c.outbox = nil
	return drained
}

// SendEmail accepts `billing.email.SendEmail`: runs the behaviour obligation, then publishes the declared events
// the outcome carries.
//
// The second result is the typed refusal of an unmet obligation — never a domain outcome,
// which always arrives as a variant of the outcome interface, refusals included.
func (c *EmailService) SendEmail(input email.SendEmail) (email.SendEmailOutcome, *obligation.UnmetObligation) {
	outcome, unmet := c.behaviors.SendEmail(input)
	if unmet != nil {
		return nil, unmet
	}
	switch value := outcome.(type) {
	case email.SendEmailOutcomeSent:
		c.outbox = append(c.outbox, PublishedEventEmailSent{Event: value.EmailSent})
	case email.SendEmailOutcomeFailed:
	}
	return outcome, nil
}
