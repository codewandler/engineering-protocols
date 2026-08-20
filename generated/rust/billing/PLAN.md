<!--
  generated from billing v3
  model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
  compiler 0.1.0 · generator 0.1.0
  do not edit: regenerate with `protocol ess synthesize`
-->
# Synthesis plan — billing v3

Scope: `component-skeletons`, planner `ess-synth 0.1.0`. Regenerate with `protocol ess synthesize`.

45 capabilities: **33 generated**, **8 obligations**, **4 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `billing.email.EmailAddress` |
| domain type | `billing.email.MessageId` |
| domain type | `billing.email.TemplateId` |
| domain type | `billing.invoice.Channel` |
| domain type | `billing.invoice.CompanyRef` |
| domain type | `billing.invoice.Email` |
| domain type | `billing.invoice.Invoice.State` |
| domain type | `billing.invoice.InvoiceId` |
| domain type | `billing.invoice.LineItem` |
| domain type | `billing.invoice.Money` |
| domain type | `billing.invoice.Payee` |
| entity lifecycle | `billing.invoice.Invoice` |
| command contract | `billing.email.SendEmail` |
| command contract | `billing.invoice.CancelInvoice` |
| command contract | `billing.invoice.CreateInvoice` |
| command contract | `billing.invoice.IssueInvoice` |
| command contract | `billing.invoice.PayInvoice` |
| event type | `billing.email.DeliveryEscalated` |
| event type | `billing.email.EmailSent` |
| event type | `billing.invoice.InvoiceCancelled` |
| event type | `billing.invoice.InvoiceCreated` |
| event type | `billing.invoice.InvoiceIssued` |
| event type | `billing.invoice.InvoicePaid` |
| error type | `billing.email.Undeliverable` |
| error type | `billing.invoice.InvalidAmount` |
| error type | `billing.invoice.InvoiceStateConflict` |
| view type | `billing.invoice.InvoiceById` |
| view type | `billing.invoice.OutstandingInvoices` |
| conversion | `billing.invoice.Email -> billing.email.EmailAddress` |
| binding transformation | `notify-on-invoice-created` |
| binding delivery | `notify-on-invoice-created` |
| component port | `email-service` |
| component port | `invoice-service` |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `billing.email.SendEmail` | decided outside the system: the provider rejects the recipient address | given `billing.email.SendEmail` input, decide and enact exactly one outcome — `sent` otherwise, emits `billing.email.EmailSent`; `failed` externally decided (the provider rejects the recipient address), error `billing.email.Undeliverable` |
| command behaviour | `billing.invoice.CancelInvoice` | the contract is declared; the algorithm is not | given `billing.invoice.CancelInvoice` input, decide and enact exactly one outcome — `cancelled` otherwise, takes `cancel` of `billing.invoice.Invoice`, emits `billing.invoice.InvoiceCancelled`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict` |
| command behaviour | `billing.invoice.CreateInvoice` | the contract is declared; the algorithm is not | given `billing.invoice.CreateInvoice` input, decide and enact exactly one outcome — `accepted` when `amount.amount > 0`, creates `billing.invoice.Invoice`, emits `billing.invoice.InvoiceCreated`; `rejected` otherwise, error `billing.invoice.InvalidAmount` |
| command behaviour | `billing.invoice.IssueInvoice` | the contract is declared; the algorithm is not | given `billing.invoice.IssueInvoice` input, decide and enact exactly one outcome — `issued` otherwise, takes `issue` of `billing.invoice.Invoice`, emits `billing.invoice.InvoiceIssued`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict` |
| command behaviour | `billing.invoice.PayInvoice` | the contract is declared; the algorithm is not | given `billing.invoice.PayInvoice` input, decide and enact exactly one outcome — `settled` when `amount.amount > 0`, takes `settle` of `billing.invoice.Invoice`, emits `billing.invoice.InvoicePaid`; `rejected` otherwise, error `billing.invoice.InvalidAmount`; `wrong-state` from a state no declared move starts in, error `billing.invoice.InvoiceStateConflict` |
| view query | `billing.invoice.InvoiceById` | how the projection is kept current is a storage decision | a query answering `billing.invoice.InvoiceById` with rows projected from `billing.invoice.Invoice` at `eventual` consistency |
| view query | `billing.invoice.OutstandingInvoices` | how the projection is kept current is a storage decision | a query answering `billing.invoice.OutstandingInvoices` with rows projected from `billing.invoice.Invoice` at `read_your_writes` consistency, containing instances where `state == Issued` |
| binding escalation | `notify-on-invoice-created` | the contract is declared; the algorithm is not | the declared `billing.email.DeliveryEscalated`, recording that delivering `billing.email.SendEmail` for `notify-on-invoice-created` was given up on — the event is declared; how its fields are filled from the failed invocation is not |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
| actor grants | `billing.invoice.Auditor` | planning | observes only; it may invoke no command; a grant is checked against a caller identity, which types do not carry, and enforcement belongs to the layer that knows who is calling |
| actor grants | `billing.invoice.Customer` | planning | may invoke `billing.invoice.CreateInvoice`; a grant is checked against a caller identity, which types do not carry, and enforcement belongs to the layer that knows who is calling |
| workload | `email-service` | planning | requires at least 2 replica(s); topology synthesis is deferred with its design |
| workload | `invoice-service` | planning | requires at least 2 replica(s); topology synthesis is deferred with its design |
