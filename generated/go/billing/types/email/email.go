// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package email is email — `billing.email`.
//
// Sending the notifications other contexts ask for.
//
// Everything this bounded context declares that the synthesis plan marks generated and this
// target can represent. What it cannot is in the TARGET.md beside this module, never absent.
package email

import (
	"example.invalid/billing/types/invoice"
	"example.invalid/billing/types/obligation"
	"example.invalid/billing/types/primitives"
)

// EmailAddress is EmailAddress — `billing.email.EmailAddress`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewEmailAddress] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// EmailAddress, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type EmailAddress struct {
	value string
}

// NewEmailAddress wraps a `String` as EmailAddress.
func NewEmailAddress(value string) EmailAddress {
	return EmailAddress{value: value}
}

// Value is the wrapped `String`.
func (v EmailAddress) Value() string {
	return v.value
}

// MessageId is MessageId — `billing.email.MessageId`: a distinct wrapper around `Uuid`.
//
// The field is unexported, so the only way to make one carrying a value is [NewMessageId] —
// a defined type over `primitives.Uuid` would have let an untyped constant be assigned straight to
// MessageId, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type MessageId struct {
	value primitives.Uuid
}

// NewMessageId wraps a `Uuid` as MessageId.
func NewMessageId(value primitives.Uuid) MessageId {
	return MessageId{value: value}
}

// Value is the wrapped `Uuid`.
func (v MessageId) Value() primitives.Uuid {
	return v.value
}

// TemplateId is TemplateId — `billing.email.TemplateId`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewTemplateId] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// TemplateId, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type TemplateId struct {
	value string
}

// NewTemplateId wraps a `String` as TemplateId.
func NewTemplateId(value string) TemplateId {
	return TemplateId{value: value}
}

// Value is the wrapped `String`.
func (v TemplateId) Value() string {
	return v.value
}

// SendEmail is SendEmail — the input of `billing.email.SendEmail`.
//
// Everything it can result in is [SendEmailOutcome].
type SendEmail struct {
	// Recipient is `recipient` — `billing.email.EmailAddress`.
	Recipient EmailAddress
	// Template is `template` — `billing.email.TemplateId`.
	Template TemplateId
}

// SendEmailOutcome is everything `billing.email.SendEmail` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type SendEmailOutcome interface {
	isSendEmailOutcome()
}

// SendEmailOutcomeSent is `sent` — otherwise.
type SendEmailOutcomeSent struct {
	// EmailSent is the `billing.email.EmailSent` this outcome publishes.
	EmailSent EmailSent
}

func (SendEmailOutcomeSent) isSendEmailOutcome() {}

// SendEmailOutcomeFailed is `failed` — externally decided (the provider rejects the recipient address).
type SendEmailOutcomeFailed struct {
	// Error is why it was refused: `billing.email.Undeliverable`.
	Error Undeliverable
}

func (SendEmailOutcomeFailed) isSendEmailOutcome() {}

// DeliveryEscalated is Delivery escalated — the event `billing.email.DeliveryEscalated`.
//
// Sending was given up on and handed to a person.
type DeliveryEscalated struct {
	// Recipient is `recipient` — `billing.email.EmailAddress`.
	Recipient EmailAddress
	// Template is `template` — `billing.email.TemplateId`.
	Template TemplateId
}

// EmailSent is EmailSent — the event `billing.email.EmailSent`.
type EmailSent struct {
	// MessageId is `message_id` — `billing.email.MessageId`.
	MessageId MessageId
	// Recipient is `recipient` — `billing.email.EmailAddress`.
	Recipient EmailAddress
}

// Undeliverable is the declared error `billing.email.Undeliverable`.
//
// The address was rejected by the provider.
type Undeliverable struct{}

// EmailAddressFromBillingInvoiceEmail is the declared crossing `billing.invoice.Email` → `billing.email.EmailAddress`.
//
// Permitted by the specification because: An invoice's customer email is a deliverable address; the email context validates it again on the way out, so the invoice context does not have to know how.
func EmailAddressFromBillingInvoiceEmail(value invoice.Email) EmailAddress {
	return NewEmailAddress(value.Value())
}

// SendEmailBehavior is the behaviour `billing.email.SendEmail` — an implementation obligation.
//
// Why it is not generated: decided outside the system: the provider rejects the recipient address.
//
// Contract: given `billing.email.SendEmail` input, decide and enact exactly one outcome — `sent` otherwise, emits `billing.email.EmailSent`; `failed` externally decided (the provider rejects the recipient address), error `billing.email.Undeliverable`.
type SendEmailBehavior interface {
	// SendEmail decides and enacts exactly one declared outcome of `billing.email.SendEmail`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	SendEmail(input SendEmail) (SendEmailOutcome, *obligation.UnmetObligation)
}

// Unimplemented satisfies every obligation of this bounded context by refusing in the type
// system.
//
// Each method returns the typed refusal naming what is owed — never a panic, never a
// guessed value — so a module built on this stub compiles and reports its own gaps.
type Unimplemented struct{}

// SendEmail refuses: the behaviour `billing.email.SendEmail` — an implementation obligation.
func (Unimplemented) SendEmail(input SendEmail) (SendEmailOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "billing.email.SendEmail"}
}
