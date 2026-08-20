# Generated projections

**Do not edit these files.** They are generated from [`examples/billing`](../examples/billing) by
`cargo xtask generate`, and CI fails if they differ from what the specification produces.

Every file here is a projection of one model, so two of them disagreeing is a bug in one of them —
and a file nothing generates any more is a contract this repository no longer stands behind.

Generated from billing v3 (model digest e19d384dac86219a).

| projection | files | describes |
| --- | --- | --- |
| `docs` | 6 | Markdown and Mermaid: the cheapest check that every construct can be described |
| `schema` | 25 | JSON Schema per command input and event payload |
| `openapi` | 2 | an OpenAPI document for every command a component accepts |
| `asyncapi` | 2 | an AsyncAPI 3.0 document per component, covering what it publishes and what it reacts to |

## `docs`

* [`docs/README.md`](docs/README.md)
* [`docs/crossings.md`](docs/crossings.md)
* [`docs/domains/billing.email.md`](docs/domains/billing.email.md)
* [`docs/domains/billing.invoice.md`](docs/domains/billing.invoice.md)
* [`docs/interactions.md`](docs/interactions.md)
* [`docs/topology.md`](docs/topology.md)

## `schema`

* [`schema/commands/billing.email.SendEmail.schema.json`](schema/commands/billing.email.SendEmail.schema.json)
* [`schema/commands/billing.invoice.CancelInvoice.schema.json`](schema/commands/billing.invoice.CancelInvoice.schema.json)
* [`schema/commands/billing.invoice.CreateInvoice.schema.json`](schema/commands/billing.invoice.CreateInvoice.schema.json)
* [`schema/commands/billing.invoice.IssueInvoice.schema.json`](schema/commands/billing.invoice.IssueInvoice.schema.json)
* [`schema/commands/billing.invoice.PayInvoice.schema.json`](schema/commands/billing.invoice.PayInvoice.schema.json)
* [`schema/errors/billing.email.Undeliverable.schema.json`](schema/errors/billing.email.Undeliverable.schema.json)
* [`schema/errors/billing.invoice.InvalidAmount.schema.json`](schema/errors/billing.invoice.InvalidAmount.schema.json)
* [`schema/errors/billing.invoice.InvoiceStateConflict.schema.json`](schema/errors/billing.invoice.InvoiceStateConflict.schema.json)
* [`schema/events/billing.email.DeliveryEscalated.schema.json`](schema/events/billing.email.DeliveryEscalated.schema.json)
* [`schema/events/billing.email.EmailSent.schema.json`](schema/events/billing.email.EmailSent.schema.json)
* [`schema/events/billing.invoice.InvoiceCancelled.schema.json`](schema/events/billing.invoice.InvoiceCancelled.schema.json)
* [`schema/events/billing.invoice.InvoiceCreated.schema.json`](schema/events/billing.invoice.InvoiceCreated.schema.json)
* [`schema/events/billing.invoice.InvoiceIssued.schema.json`](schema/events/billing.invoice.InvoiceIssued.schema.json)
* [`schema/events/billing.invoice.InvoicePaid.schema.json`](schema/events/billing.invoice.InvoicePaid.schema.json)
* [`schema/types/billing.email.EmailAddress.schema.json`](schema/types/billing.email.EmailAddress.schema.json)
* [`schema/types/billing.email.MessageId.schema.json`](schema/types/billing.email.MessageId.schema.json)
* [`schema/types/billing.email.TemplateId.schema.json`](schema/types/billing.email.TemplateId.schema.json)
* [`schema/types/billing.invoice.Channel.schema.json`](schema/types/billing.invoice.Channel.schema.json)
* [`schema/types/billing.invoice.CompanyRef.schema.json`](schema/types/billing.invoice.CompanyRef.schema.json)
* [`schema/types/billing.invoice.Email.schema.json`](schema/types/billing.invoice.Email.schema.json)
* [`schema/types/billing.invoice.Invoice.State.schema.json`](schema/types/billing.invoice.Invoice.State.schema.json)
* [`schema/types/billing.invoice.InvoiceId.schema.json`](schema/types/billing.invoice.InvoiceId.schema.json)
* [`schema/types/billing.invoice.LineItem.schema.json`](schema/types/billing.invoice.LineItem.schema.json)
* [`schema/types/billing.invoice.Money.schema.json`](schema/types/billing.invoice.Money.schema.json)
* [`schema/types/billing.invoice.Payee.schema.json`](schema/types/billing.invoice.Payee.schema.json)

## `openapi`

* [`openapi/email-service.yaml`](openapi/email-service.yaml)
* [`openapi/invoice-service.yaml`](openapi/invoice-service.yaml)

## `asyncapi`

* [`asyncapi/email-service.yaml`](asyncapi/email-service.yaml)
* [`asyncapi/invoice-service.yaml`](asyncapi/invoice-service.yaml)
