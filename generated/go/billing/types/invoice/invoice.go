// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

// Package invoice is Invoicing — `billing.invoice`.
//
// Issuing invoices and tracking whether they are paid.
//
// Everything this bounded context declares that the synthesis plan marks generated and this
// target can represent. What it cannot is in the TARGET.md beside this module, never absent.
package invoice

import (
	"example.invalid/billing/types/obligation"
	"example.invalid/billing/types/primitives"
)

// Channel is Delivery channel — `billing.invoice.Channel`: one of a closed set of names.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type Channel interface {
	isChannel()
}

// ChannelEmail is `Email`.
type ChannelEmail struct{}

func (ChannelEmail) isChannel() {}

// ChannelPost is `Post`.
type ChannelPost struct{}

func (ChannelPost) isChannel() {}

// ChannelPortal is `Portal`.
type ChannelPortal struct{}

func (ChannelPortal) isChannel() {}

// CompanyRef is CompanyRef — `billing.invoice.CompanyRef`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewCompanyRef] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// CompanyRef, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type CompanyRef struct {
	value string
}

// NewCompanyRef wraps a `String` as CompanyRef.
func NewCompanyRef(value string) CompanyRef {
	return CompanyRef{value: value}
}

// Value is the wrapped `String`.
func (v CompanyRef) Value() string {
	return v.value
}

// Email is Email — `billing.invoice.Email`: a distinct wrapper around `String`.
//
// The field is unexported, so the only way to make one carrying a value is [NewEmail] —
// a defined type over `string` would have let an untyped constant be assigned straight to
// Email, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type Email struct {
	value string
}

// NewEmail wraps a `String` as Email.
func NewEmail(value string) Email {
	return Email{value: value}
}

// Value is the wrapped `String`.
func (v Email) Value() string {
	return v.value
}

// InvoiceState is the states of `billing.invoice.Invoice`, as runtime values.
//
// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is
// not carried here — it is carried by one type per state, where an undeclared move is a
// method that does not exist.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type InvoiceState interface {
	isInvoiceState()
}

// InvoiceStateCancelled is `Cancelled`.
type InvoiceStateCancelled struct{}

func (InvoiceStateCancelled) isInvoiceState() {}

// InvoiceStateDraft is `Draft`.
type InvoiceStateDraft struct{}

func (InvoiceStateDraft) isInvoiceState() {}

// InvoiceStateIssued is `Issued`.
type InvoiceStateIssued struct{}

func (InvoiceStateIssued) isInvoiceState() {}

// InvoiceStatePaid is `Paid`.
type InvoiceStatePaid struct{}

func (InvoiceStatePaid) isInvoiceState() {}

// InvoiceId is InvoiceId — `billing.invoice.InvoiceId`: a distinct wrapper around `Uuid`.
//
// The field is unexported, so the only way to make one carrying a value is [NewInvoiceId] —
// a defined type over `primitives.Uuid` would have let an untyped constant be assigned straight to
// InvoiceId, which is the distinctness this declaration exists for. Go's zero value still
// needs no constructor (see TARGET.md).
type InvoiceId struct {
	value primitives.Uuid
}

// NewInvoiceId wraps a `Uuid` as InvoiceId.
func NewInvoiceId(value primitives.Uuid) InvoiceId {
	return InvoiceId{value: value}
}

// Value is the wrapped `Uuid`.
func (v InvoiceId) Value() primitives.Uuid {
	return v.value
}

// LineItem is LineItem — `billing.invoice.LineItem`.
type LineItem struct {
	// Description is `description` — `String`.
	Description string
	// Quantity is `quantity` — `Integer`.
	Quantity int64
	// UnitPrice is `unit_price` — `billing.invoice.Money`.
	UnitPrice Money
}

// Money is Money — `billing.invoice.Money`.
//
// Every value satisfies `amount >= 0` — declared here, enforced by whatever behaviour constructs one.
type Money struct {
	// Amount is `amount` — `Decimal`.
	Amount primitives.Decimal
	// Currency is `currency` — `String`.
	Currency string
}

// Payee is Payee — `billing.invoice.Payee`: one of a fixed set of shapes, tagged on the wire by `kind`.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type Payee interface {
	isPayee()
}

// PayeeCompany is the shape tagged `company` — `billing.invoice.CompanyRef`.
type PayeeCompany struct {
	// Value is what this shape carries.
	Value CompanyRef
}

func (PayeeCompany) isPayee() {}

// PayeePerson is the shape tagged `person` — `billing.invoice.Email`.
type PayeePerson struct {
	// Value is what this shape carries.
	Value Email
}

func (PayeePerson) isPayee() {}

// InvoiceData is what Invoice — `billing.invoice.Invoice` — holds, apart from where it is in its lifecycle.
//
// The identity and every declared field. The state is deliberately not one: inside the domain
// it is carried by the type ([InvoiceDraft] and its siblings), and at a boundary by [InvoiceSnapshot].
//
// Every value satisfies `total.amount >= 0` — declared here, enforced by whatever behaviour constructs one.
type InvoiceData struct {
	// InvoiceId is the identity: `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
	// Total is `total` — `billing.invoice.Money`.
	Total Money
	// Payee is `payee` — `billing.invoice.Payee`.
	Payee Payee
	// Channel is `channel` — `billing.invoice.Channel`.
	Channel Channel
	// Lines is `lines` — `List<billing.invoice.LineItem>`.
	Lines []LineItem
	// Note is `note` — `Optional<String>`.
	Note *string
	// Metadata is `metadata` — `Map<String, String>`.
	Metadata map[string]string
	// IssuedAt is `issued_at` — `Optional<Timestamp>`.
	IssuedAt *primitives.Timestamp
	// SettlementWindow is `settlement_window` — `Duration`.
	SettlementWindow primitives.Duration
	// IsRecurring is `is_recurring` — `Boolean`.
	IsRecurring bool
	// Signature is `signature` — `Bytes`.
	Signature []byte
}

// InvoiceInCancelled is `billing.invoice.Invoice` resting in `Cancelled`. Terminal: an instance may rest here forever.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type InvoiceInCancelled struct {
	data InvoiceData
}

// State is the state this instance rests in, as the runtime value.
func (InvoiceInCancelled) State() InvoiceState {
	return InvoiceStateCancelled{}
}

// Data is what it holds.
func (v InvoiceInCancelled) Data() InvoiceData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v InvoiceInCancelled) Snapshot() InvoiceSnapshot {
	return InvoiceSnapshot{State: InvoiceStateCancelled{}, Data: v.data}
}

func (InvoiceInCancelled) isAnyInvoice() {}

// InvoiceInDraft is `billing.invoice.Invoice` resting in `Draft`. Where a new instance starts.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type InvoiceInDraft struct {
	data InvoiceData
}

// NewInvoice starts a new `billing.invoice.Invoice` in `Draft` — the only state the lifecycle starts one in.
func NewInvoice(data InvoiceData) InvoiceInDraft {
	return InvoiceInDraft{data: data}
}

// State is the state this instance rests in, as the runtime value.
func (InvoiceInDraft) State() InvoiceState {
	return InvoiceStateDraft{}
}

// Data is what it holds.
func (v InvoiceInDraft) Data() InvoiceData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v InvoiceInDraft) Snapshot() InvoiceSnapshot {
	return InvoiceSnapshot{State: InvoiceStateDraft{}, Data: v.data}
}

func (InvoiceInDraft) isAnyInvoice() {}

// Issue takes `issue` — `Draft` → `Issued`. Taken by the `issued` outcome of `billing.invoice.IssueInvoice`.
func (v InvoiceInDraft) Issue() InvoiceInIssued {
	return InvoiceInIssued{data: v.data}
}

// Cancel takes `cancel` — `Draft` → `Cancelled`. Taken by the `cancelled` outcome of `billing.invoice.CancelInvoice`.
func (v InvoiceInDraft) Cancel() InvoiceInCancelled {
	return InvoiceInCancelled{data: v.data}
}

// InvoiceInIssued is `billing.invoice.Invoice` resting in `Issued`.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type InvoiceInIssued struct {
	data InvoiceData
}

// State is the state this instance rests in, as the runtime value.
func (InvoiceInIssued) State() InvoiceState {
	return InvoiceStateIssued{}
}

// Data is what it holds.
func (v InvoiceInIssued) Data() InvoiceData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v InvoiceInIssued) Snapshot() InvoiceSnapshot {
	return InvoiceSnapshot{State: InvoiceStateIssued{}, Data: v.data}
}

func (InvoiceInIssued) isAnyInvoice() {}

// Settle takes `settle` — `Issued` → `Paid`. Taken by the `settled` outcome of `billing.invoice.PayInvoice`.
func (v InvoiceInIssued) Settle() InvoiceInPaid {
	return InvoiceInPaid{data: v.data}
}

// Cancel takes `cancel` — `Issued` → `Cancelled`. Taken by the `cancelled` outcome of `billing.invoice.CancelInvoice`.
func (v InvoiceInIssued) Cancel() InvoiceInCancelled {
	return InvoiceInCancelled{data: v.data}
}

// InvoiceInPaid is `billing.invoice.Invoice` resting in `Paid`. Terminal: an instance may rest here forever.
//
// One type per declared state: a transition is a method on exactly the states the
// specification declares it starts from, so an undeclared move is a method that does not
// exist. The field is unexported — the only way to reach a state is the constructor or a
// declared move (see TARGET.md for what Go's zero value still permits).
type InvoiceInPaid struct {
	data InvoiceData
}

// State is the state this instance rests in, as the runtime value.
func (InvoiceInPaid) State() InvoiceState {
	return InvoiceStatePaid{}
}

// Data is what it holds.
func (v InvoiceInPaid) Data() InvoiceData {
	return v.data
}

// Snapshot is this instance at a boundary: the state as a value beside the data.
func (v InvoiceInPaid) Snapshot() InvoiceSnapshot {
	return InvoiceSnapshot{State: InvoiceStatePaid{}, Data: v.data}
}

func (InvoiceInPaid) isAnyInvoice() {}

// AnyInvoice is an instance of `billing.invoice.Invoice` in whichever declared state it was found.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type AnyInvoice interface {
	isAnyInvoice()

	// State is the state this instance rests in.
	State() InvoiceState

	// Snapshot is this instance at a boundary.
	Snapshot() InvoiceSnapshot
}

// InvoiceSnapshot is `billing.invoice.Invoice` as it crosses a boundary: the state as a value beside the data.
//
// Wire and storage know states only at runtime; [InvoiceSnapshot.Refine] is the one door back into
// the typed lifecycle.
type InvoiceSnapshot struct {
	// State is where the instance is in its lifecycle.
	State InvoiceState
	// Data is what it holds.
	Data InvoiceData
}

// Refine refines the runtime state into the typed one.
//
// Rust's is total, and this one cannot be: the state is a sealed interface, whose zero
// value is nil and names no declared state, so a snapshot nothing constructed reaches here.
// `ok` is false for exactly that snapshot and for no other — every declared state has an
// arm (see TARGET.md).
func (v InvoiceSnapshot) Refine() (AnyInvoice, bool) {
	switch v.State.(type) {
	case InvoiceStateCancelled:
		return InvoiceInCancelled{data: v.Data}, true
	case InvoiceStateDraft:
		return InvoiceInDraft{data: v.Data}, true
	case InvoiceStateIssued:
		return InvoiceInIssued{data: v.Data}, true
	case InvoiceStatePaid:
		return InvoiceInPaid{data: v.Data}, true
	}
	return nil, false
}

// CancelInvoice is Cancel invoice — the input of `billing.invoice.CancelInvoice`.
//
// Everything it can result in is [CancelInvoiceOutcome].
type CancelInvoice struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
}

// CancelInvoiceOutcome is everything `billing.invoice.CancelInvoice` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type CancelInvoiceOutcome interface {
	isCancelInvoiceOutcome()
}

// CancelInvoiceOutcomeCancelled is `cancelled` — otherwise.
//
// The invoice is cancelled, from Draft or from Issued.
type CancelInvoiceOutcomeCancelled struct {
	// InvoiceCancelled is the `billing.invoice.InvoiceCancelled` this outcome publishes.
	InvoiceCancelled InvoiceCancelled
}

func (CancelInvoiceOutcomeCancelled) isCancelInvoiceOutcome() {}

// CancelInvoiceOutcomeWrongState is `wrong-state` — from a state no declared move starts in.
//
// The invoice is already Paid or already Cancelled, so nothing was cancelled.
type CancelInvoiceOutcomeWrongState struct {
	// Error is why it was refused: `billing.invoice.InvoiceStateConflict`.
	Error InvoiceStateConflict
}

func (CancelInvoiceOutcomeWrongState) isCancelInvoiceOutcome() {}

// CreateInvoice is Create invoice — the input of `billing.invoice.CreateInvoice`.
//
// Everything it can result in is [CreateInvoiceOutcome].
type CreateInvoice struct {
	// CustomerEmail is `customer_email` — `billing.invoice.Email`.
	CustomerEmail Email
	// Amount is `amount` — `billing.invoice.Money`.
	Amount Money
}

// CreateInvoiceOutcome is everything `billing.invoice.CreateInvoice` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type CreateInvoiceOutcome interface {
	isCreateInvoiceOutcome()
}

// CreateInvoiceOutcomeAccepted is `accepted` — when `amount.amount > 0`.
//
// The invoice is created in Draft.
type CreateInvoiceOutcomeAccepted struct {
	// InvoiceCreated is the `billing.invoice.InvoiceCreated` this outcome publishes.
	InvoiceCreated InvoiceCreated
}

func (CreateInvoiceOutcomeAccepted) isCreateInvoiceOutcome() {}

// CreateInvoiceOutcomeRejected is `rejected` — otherwise.
//
// The amount was not positive, and nothing was created.
type CreateInvoiceOutcomeRejected struct {
	// Error is why it was refused: `billing.invoice.InvalidAmount`.
	Error InvalidAmount
}

func (CreateInvoiceOutcomeRejected) isCreateInvoiceOutcome() {}

// IssueInvoice is Issue invoice — the input of `billing.invoice.IssueInvoice`.
//
// Everything it can result in is [IssueInvoiceOutcome].
type IssueInvoice struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
}

// IssueInvoiceOutcome is everything `billing.invoice.IssueInvoice` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type IssueInvoiceOutcome interface {
	isIssueInvoiceOutcome()
}

// IssueInvoiceOutcomeIssued is `issued` — otherwise.
//
// The invoice leaves Draft and is now Issued.
type IssueInvoiceOutcomeIssued struct {
	// InvoiceIssued is the `billing.invoice.InvoiceIssued` this outcome publishes.
	InvoiceIssued InvoiceIssued
}

func (IssueInvoiceOutcomeIssued) isIssueInvoiceOutcome() {}

// IssueInvoiceOutcomeWrongState is `wrong-state` — from a state no declared move starts in.
//
// The invoice is not in Draft, so it was not issued.
type IssueInvoiceOutcomeWrongState struct {
	// Error is why it was refused: `billing.invoice.InvoiceStateConflict`.
	Error InvoiceStateConflict
}

func (IssueInvoiceOutcomeWrongState) isIssueInvoiceOutcome() {}

// PayInvoice is Pay invoice — the input of `billing.invoice.PayInvoice`.
//
// Everything it can result in is [PayInvoiceOutcome].
type PayInvoice struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
	// Amount is `amount` — `billing.invoice.Money`.
	Amount Money
}

// PayInvoiceOutcome is everything `billing.invoice.PayInvoice` can result in — one variant per declared outcome.
//
// An infrastructure failure is deliberately not in here: a refusal is a fact about the
// domain, a transport fault is a fact about the run, and conflating the two is what the
// declared outcomes exist to prevent.
//
// A closed set: the marker method below is unexported, so no type outside this package can
// join it. Go cannot check that a `switch` over it handles every case — that is a target-stage
// weakening of what the specification declares, recorded in TARGET.md, not a gap in the model.
type PayInvoiceOutcome interface {
	isPayInvoiceOutcome()
}

// PayInvoiceOutcomeSettled is `settled` — when `amount.amount > 0`.
//
// The payment is accepted and the invoice becomes Paid.
type PayInvoiceOutcomeSettled struct {
	// InvoicePaid is the `billing.invoice.InvoicePaid` this outcome publishes.
	InvoicePaid InvoicePaid
}

func (PayInvoiceOutcomeSettled) isPayInvoiceOutcome() {}

// PayInvoiceOutcomeRejected is `rejected` — otherwise.
//
// The payment was not positive, so the invoice did not move.
type PayInvoiceOutcomeRejected struct {
	// Error is why it was refused: `billing.invoice.InvalidAmount`.
	Error InvalidAmount
}

func (PayInvoiceOutcomeRejected) isPayInvoiceOutcome() {}

// PayInvoiceOutcomeWrongState is `wrong-state` — from a state no declared move starts in.
//
// The invoice is not Issued, so the payment did not settle it.
type PayInvoiceOutcomeWrongState struct {
	// Error is why it was refused: `billing.invoice.InvoiceStateConflict`.
	Error InvoiceStateConflict
}

func (PayInvoiceOutcomeWrongState) isPayInvoiceOutcome() {}

// InvoiceCancelled is InvoiceCancelled — the event `billing.invoice.InvoiceCancelled`.
type InvoiceCancelled struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
}

// InvoiceCreated is InvoiceCreated — the event `billing.invoice.InvoiceCreated`.
type InvoiceCreated struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
	// CustomerEmail is `customer_email` — `billing.invoice.Email`.
	CustomerEmail Email
	// Amount is `amount` — `billing.invoice.Money`.
	Amount Money
}

// InvoiceIssued is InvoiceIssued — the event `billing.invoice.InvoiceIssued`.
type InvoiceIssued struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
}

// InvoicePaid is InvoicePaid — the event `billing.invoice.InvoicePaid`.
type InvoicePaid struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
	// Amount is `amount` — `billing.invoice.Money`.
	Amount Money
}

// InvalidAmount is the declared error `billing.invoice.InvalidAmount`.
//
// The requested amount is not positive.
type InvalidAmount struct {
	// Submitted is `submitted` — `billing.invoice.Money`.
	Submitted Money
}

// InvoiceStateConflict is the declared error `billing.invoice.InvoiceStateConflict`.
//
// The invoice is not in a state this command acts from, so nothing moved.
type InvoiceStateConflict struct {
	// State is `state` — `billing.invoice.Invoice.State`.
	State InvoiceState
}

// InvoiceById is InvoiceById — one row of the view `billing.invoice.InvoiceById`.
//
// Projects `billing.invoice.Invoice` at `eventual` consistency.
// Serving it is an implementation obligation — see the plan — because how a projection is
// kept current is a storage decision the specification does not take.
type InvoiceById struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
	// Total is `total` — `billing.invoice.Money`.
	Total Money
}

// OutstandingInvoices is Outstanding invoices — one row of the view `billing.invoice.OutstandingInvoices`.
//
// Projects `billing.invoice.Invoice` at `read_your_writes` consistency, containing instances where `state == Issued`.
// Serving it is an implementation obligation — see the plan — because how a projection is
// kept current is a storage decision the specification does not take.
type OutstandingInvoices struct {
	// InvoiceId is `invoice_id` — `billing.invoice.InvoiceId`.
	InvoiceId InvoiceId
	// Total is `total` — `billing.invoice.Money`.
	Total Money
}

// CancelInvoiceBehavior is the behaviour `billing.invoice.CancelInvoice` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `billing.invoice.CancelInvoice` input, decide and enact exactly one outcome — `cancelled` otherwise, takes `cancel` of `billing.invoice.Invoice`, emits `billing.invoice.InvoiceCancelled`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict`.
type CancelInvoiceBehavior interface {
	// CancelInvoice decides and enacts exactly one declared outcome of `billing.invoice.CancelInvoice`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	CancelInvoice(input CancelInvoice) (CancelInvoiceOutcome, *obligation.UnmetObligation)
}

// CreateInvoiceBehavior is the behaviour `billing.invoice.CreateInvoice` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `billing.invoice.CreateInvoice` input, decide and enact exactly one outcome — `accepted` when `amount.amount > 0`, creates `billing.invoice.Invoice`, emits `billing.invoice.InvoiceCreated`; `rejected` otherwise, error `billing.invoice.InvalidAmount`.
type CreateInvoiceBehavior interface {
	// CreateInvoice decides and enacts exactly one declared outcome of `billing.invoice.CreateInvoice`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	CreateInvoice(input CreateInvoice) (CreateInvoiceOutcome, *obligation.UnmetObligation)
}

// IssueInvoiceBehavior is the behaviour `billing.invoice.IssueInvoice` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `billing.invoice.IssueInvoice` input, decide and enact exactly one outcome — `issued` otherwise, takes `issue` of `billing.invoice.Invoice`, emits `billing.invoice.InvoiceIssued`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict`.
type IssueInvoiceBehavior interface {
	// IssueInvoice decides and enacts exactly one declared outcome of `billing.invoice.IssueInvoice`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	IssueInvoice(input IssueInvoice) (IssueInvoiceOutcome, *obligation.UnmetObligation)
}

// PayInvoiceBehavior is the behaviour `billing.invoice.PayInvoice` — an implementation obligation.
//
// Why it is not generated: the contract is declared; the algorithm is not.
//
// Contract: given `billing.invoice.PayInvoice` input, decide and enact exactly one outcome — `settled` when `amount.amount > 0`, takes `settle` of `billing.invoice.Invoice`, emits `billing.invoice.InvoicePaid`; `rejected` otherwise, error `billing.invoice.InvalidAmount`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict`.
type PayInvoiceBehavior interface {
	// PayInvoice decides and enacts exactly one declared outcome of `billing.invoice.PayInvoice`.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	PayInvoice(input PayInvoice) (PayInvoiceOutcome, *obligation.UnmetObligation)
}

// InvoiceByIdQuery is the query `billing.invoice.InvoiceById` — an implementation obligation.
//
// Why it is not generated: how the projection is kept current is a storage decision.
//
// Contract: a query answering `billing.invoice.InvoiceById` with rows projected from `billing.invoice.Invoice` at `eventual` consistency.
type InvoiceByIdQuery interface {
	// InvoiceById serves `billing.invoice.InvoiceById` rows at the view's declared consistency.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	InvoiceById() ([]InvoiceById, *obligation.UnmetObligation)
}

// OutstandingInvoicesQuery is the query `billing.invoice.OutstandingInvoices` — an implementation obligation.
//
// Why it is not generated: how the projection is kept current is a storage decision.
//
// Contract: a query answering `billing.invoice.OutstandingInvoices` with rows projected from `billing.invoice.Invoice` at `read_your_writes` consistency, containing instances where `state == Issued`.
type OutstandingInvoicesQuery interface {
	// OutstandingInvoices serves `billing.invoice.OutstandingInvoices` rows at the view's declared consistency.
	//
	// The second result is the typed refusal of an obligation nothing has satisfied; a
	// satisfying implementation never returns one.
	OutstandingInvoices() ([]OutstandingInvoices, *obligation.UnmetObligation)
}

// Unimplemented satisfies every obligation of this bounded context by refusing in the type
// system.
//
// Each method returns the typed refusal naming what is owed — never a panic, never a
// guessed value — so a module built on this stub compiles and reports its own gaps.
type Unimplemented struct{}

// CancelInvoice refuses: the behaviour `billing.invoice.CancelInvoice` — an implementation obligation.
func (Unimplemented) CancelInvoice(input CancelInvoice) (CancelInvoiceOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "billing.invoice.CancelInvoice"}
}

// CreateInvoice refuses: the behaviour `billing.invoice.CreateInvoice` — an implementation obligation.
func (Unimplemented) CreateInvoice(input CreateInvoice) (CreateInvoiceOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "billing.invoice.CreateInvoice"}
}

// IssueInvoice refuses: the behaviour `billing.invoice.IssueInvoice` — an implementation obligation.
func (Unimplemented) IssueInvoice(input IssueInvoice) (IssueInvoiceOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "billing.invoice.IssueInvoice"}
}

// PayInvoice refuses: the behaviour `billing.invoice.PayInvoice` — an implementation obligation.
func (Unimplemented) PayInvoice(input PayInvoice) (PayInvoiceOutcome, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "command behaviour", Source: "billing.invoice.PayInvoice"}
}

// InvoiceById refuses: the query `billing.invoice.InvoiceById` — an implementation obligation.
func (Unimplemented) InvoiceById() ([]InvoiceById, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "view query", Source: "billing.invoice.InvoiceById"}
}

// OutstandingInvoices refuses: the query `billing.invoice.OutstandingInvoices` — an implementation obligation.
func (Unimplemented) OutstandingInvoices() ([]OutstandingInvoices, *obligation.UnmetObligation) {
	return nil, &obligation.UnmetObligation{Capability: "view query", Source: "billing.invoice.OutstandingInvoices"}
}
