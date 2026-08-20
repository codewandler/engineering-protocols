<!--
  generated from billing v3
  model digest e19d384dac86219a
  compiler 0.1.0 · generator 0.1.0
  do not edit: regenerate with `protocol ess synthesize`
-->
# Synthesis plan — billing v3

Scope: `semantic-types`, planner `ess-synth 0.1.0`. Regenerate with `protocol ess synthesize`.

43 capabilities: **29 generated**, **7 obligations**, **7 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

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

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
| actor grants | `billing.invoice.Auditor` | planning | observes only; it may invoke no command; a grant is checked against a caller identity, which types do not carry, and enforcement belongs to the layer that knows who is calling |
| actor grants | `billing.invoice.Customer` | planning | may invoke `billing.invoice.CreateInvoice`; a grant is checked against a caller identity, which types do not carry, and enforcement belongs to the layer that knows who is calling |
| binding | `notify-on-invoice-created` | planning | reacts to `billing.invoice.InvoiceCreated` by invoking `billing.email.SendEmail` (at_least_once, on failure escalate); both the transformation and the delivery guarantee need the interaction layer, which the semantic-types scope does not hold |
| component port | `email-service` | planning | accepts 1 command(s) and publishes 2 event(s); a port surface needs the interaction layer, which the semantic-types scope does not hold |
| component port | `invoice-service` | planning | accepts 4 command(s) and publishes 4 event(s); a port surface needs the interaction layer, which the semantic-types scope does not hold |
| workload | `email-service` | planning | requires at least 2 replica(s); topology synthesis is deferred with its design |
| workload | `invoice-service` | planning | requires at least 2 replica(s); topology synthesis is deferred with its design |
