/**
 * The lab's material: a trimmed rendering of the compiled IR, and one canned run over it.
 *
 * NOTHING HERE IS EXECUTED. This file is a layout draft's stand-in for an engine — a hardcoded
 * array of steps, written out by hand so the three panels can be judged in motion before anything
 * drives them. Every *name* in it is real: the IR rows below are `protocol ess compile --path
 * examples/billing --format json` trimmed to the `billing.invoice` domain, and the outcomes,
 * errors, events and guards in the run are the ones `examples/billing/domains/invoice.yaml`
 * declares. The two *values* — an email address and two amounts — are invented, because a
 * specification declares types and not instances.
 *
 * The source line numbers are 1-based and refer to `_source.ts`, which is that file verbatim.
 */

/* ------------------------------------------------------------------ *
 * The intermediate layer
 * ------------------------------------------------------------------ */

/** What a row is, which is what colours its chip. */
export type IrKind =
  | 'system'
  | 'domain'
  | 'group'
  | 'type'
  | 'entity'
  | 'lifecycle'
  | 'command'
  | 'input'
  | 'outcome'
  | 'event'
  | 'error'
  | 'view';

export type IrRow = {
  /** Stable id, so a step can say which rows it is standing on. */
  id: string;
  depth: number;
  kind: IrKind;
  /** The declaration's own name, short — the domain prefix is the row above it. */
  label: string;
  /** What the compiler resolved it to. */
  detail?: string;
};

/**
 * `protocol ess compile --path examples/billing --format json`, trimmed.
 *
 * The counts on the group rows are the whole system's; the rows under them are the ones this run
 * touches. A real lab renders every one of them and lets the panel scroll.
 */
export const IR_ROWS: IrRow[] = [
  {id: 'sys', depth: 0, kind: 'system', label: 'billing', detail: 'v3 · 5 files · 2 domains'},
  {id: 'dom', depth: 1, kind: 'domain', label: 'billing.invoice', detail: 'wire invoices · display Invoicing'},

  {id: 'g.types', depth: 2, kind: 'group', label: 'types', detail: '11 declared'},
  {id: 'ty.money', depth: 3, kind: 'type', label: 'Money', detail: 'struct · amount: Decimal, currency: String · invariant amount >= 0'},
  {id: 'ty.email', depth: 3, kind: 'type', label: 'Email', detail: 'newtype of String'},
  {id: 'ty.id', depth: 3, kind: 'type', label: 'InvoiceId', detail: 'newtype of Uuid'},
  {id: 'ty.state', depth: 3, kind: 'type', label: 'Invoice.State', detail: 'enum · derived from the lifecycle'},

  {id: 'g.entities', depth: 2, kind: 'group', label: 'entities', detail: '1 declared'},
  {id: 'ent.invoice', depth: 3, kind: 'entity', label: 'Invoice', detail: 'identity invoice_id: InvoiceId · invariant total.amount >= 0'},
  {id: 'ent.invoice.lifecycle', depth: 4, kind: 'lifecycle', label: 'lifecycle', detail: 'initial Draft · issue → Issued · settle → Paid · cancel → Cancelled'},

  {id: 'g.commands', depth: 2, kind: 'group', label: 'commands', detail: '5 declared'},
  {id: 'cmd.create', depth: 3, kind: 'command', label: 'CreateInvoice', detail: 'wire create-invoice · actor Customer may invoke'},
  {id: 'cmd.create.input', depth: 4, kind: 'input', label: 'input', detail: 'customer_email: Email, amount: Money'},
  {id: 'cmd.create.accepted', depth: 4, kind: 'outcome', label: 'accepted', detail: 'when amount.amount > 0 · creates Invoice · emits InvoiceCreated · test construct_input'},
  {id: 'cmd.create.rejected', depth: 4, kind: 'outcome', label: 'rejected', detail: 'otherwise · reports InvalidAmount · test default_branch'},
  {id: 'cmd.issue', depth: 3, kind: 'command', label: 'IssueInvoice', detail: 'wire issue-invoice · moves Invoice.issue'},
  {id: 'cmd.pay', depth: 3, kind: 'command', label: 'PayInvoice', detail: 'wire pay-invoice · moves Invoice.settle'},
  {id: 'cmd.cancel', depth: 3, kind: 'command', label: 'CancelInvoice', detail: 'wire cancel-invoice · moves Invoice.cancel'},

  {id: 'g.events', depth: 2, kind: 'group', label: 'events', detail: '6 declared'},
  {id: 'evt.created', depth: 3, kind: 'event', label: 'InvoiceCreated', detail: 'invoice_id, customer_email, amount · triggers SendEmail through notify-on-invoice-created'},
  {id: 'evt.issued', depth: 3, kind: 'event', label: 'InvoiceIssued', detail: 'invoice_id'},
  {id: 'evt.paid', depth: 3, kind: 'event', label: 'InvoicePaid', detail: 'invoice_id, amount'},

  {id: 'g.errors', depth: 2, kind: 'group', label: 'errors', detail: '3 declared'},
  {id: 'err.invalid', depth: 3, kind: 'error', label: 'InvalidAmount', detail: 'submitted: Money — the amount is not positive'},
  {id: 'err.conflict', depth: 3, kind: 'error', label: 'InvoiceStateConflict', detail: 'state: Invoice.State — the command does not act from it'},

  {id: 'g.views', depth: 2, kind: 'group', label: 'views', detail: '2 declared'},
  {id: 'view.byid', depth: 3, kind: 'view', label: 'InvoiceById', detail: 'from Invoice · eventual · asserted with eventually'},
  {id: 'view.outstanding', depth: 3, kind: 'view', label: 'OutstandingInvoices', detail: 'from Invoice · read_your_writes · filter state == Issued'},
];

/* ------------------------------------------------------------------ *
 * The run
 * ------------------------------------------------------------------ */

/** The glyph and colour an entry is written in. */
export type EffectKind =
  | 'call'
  | 'guard'
  | 'accepted'
  | 'rejected'
  | 'entity'
  | 'event'
  | 'view'
  | 'nothing'
  | 'note';

export type Effect = {
  kind: EffectKind;
  text: string;
  detail?: string;
};

/** An inclusive, 1-based range of lines in `source.ts`. */
export type LineRange = [number, number];

export type Step = {
  id: string;
  /** What the toolbar calls this step. */
  label: string;
  /** One line: what the step is doing, in the specification's own words where there are any. */
  note: string;
  source: LineRange[];
  ir: string[];
  effects: Effect[];
};

const AMOUNT = '{amount: "120.00", currency: "EUR"}';
const ZERO = '{amount: "0.00", currency: "EUR"}';

/**
 * Eleven steps: one accepted exchange through `CreateInvoice`, then one refused.
 *
 * The refused amount is `0.00` rather than a negative one, and that is not a softening: `Money`
 * declares the invariant `amount >= 0` (lines 27-29), so a negative amount is not a `Money` at all
 * and could never reach the guard. `0.00` is the refusal this specification can actually express —
 * `when: amount.amount > 0` is false, and `InvalidAmount.submitted` can still carry the value.
 */
export const STEPS: Step[] = [
  {
    id: 'compile',
    label: 'compile',
    note: 'The specification is read and every reference in it resolved.',
    source: [[1, 6]],
    ir: ['sys', 'dom'],
    effects: [
      {
        kind: 'note',
        text: 'billing v3 compiled',
        detail: '5 files · 2 domains, 11 types, 5 commands, 6 events, 3 errors, 1 binding, 2 components',
      },
    ],
  },
  {
    id: 'command',
    label: 'resolve CreateInvoice',
    note: 'The command, its wire name and the two fields it takes.',
    source: [[145, 155]],
    ir: ['cmd.create', 'cmd.create.input'],
    effects: [
      {
        kind: 'call',
        text: 'CreateInvoice',
        detail: `{customer_email: "ada@example.com", amount: ${AMOUNT}}`,
      },
    ],
  },
  {
    id: 'guard',
    label: 'evaluate the guard',
    note: 'Which outcome this input lands in is a declared predicate, not a branch someone wrote.',
    source: [
      [164, 166],
      [188, 188],
    ],
    ir: ['cmd.create.accepted', 'cmd.create.rejected'],
    effects: [{kind: 'guard', text: 'when amount.amount > 0', detail: 'true — the accepted branch'}],
  },
  {
    id: 'accepted',
    label: 'outcome accepted',
    note: 'The subject hangs off the outcome, not off the command.',
    source: [
      [165, 167],
      [173, 173],
      [186, 186],
    ],
    ir: ['cmd.create.accepted'],
    effects: [{kind: 'accepted', text: 'outcome: accepted', detail: 'The invoice is created in Draft.'}],
  },
  {
    id: 'entity',
    label: 'create the invoice',
    note: 'creates: is not a transition — a new instance has no state to move out of.',
    source: [
      [61, 68],
      [101, 103],
    ],
    ir: ['ent.invoice', 'ent.invoice.lifecycle'],
    effects: [
      {
        kind: 'entity',
        text: 'entity billing.invoice.Invoice #inv-1',
        detail: "state Draft — the lifecycle's initial, reached without a transition",
      },
    ],
  },
  {
    id: 'event',
    label: 'announce InvoiceCreated',
    note: 'payload: says where the announced fact’s values come from; invoice_id has no line, on purpose.',
    source: [
      [174, 175],
      [182, 185],
      [307, 314],
    ],
    ir: ['evt.created'],
    effects: [
      {
        kind: 'event',
        text: 'event InvoiceCreated',
        detail: `{invoice_id: "inv-1", customer_email: "ada@example.com", amount: ${AMOUNT}}`,
      },
      {
        kind: 'note',
        text: 'binding notify-on-invoice-created',
        detail: 'at_least_once / escalate → billing.email.SendEmail (declared in system.yaml, not shown)',
      },
    ],
  },
  {
    id: 'views',
    label: 'project the views',
    note: 'Two views over one entity, and the consistency each is asserted with.',
    source: [[333, 351]],
    ir: ['view.byid', 'view.outstanding'],
    effects: [
      {
        kind: 'view',
        text: 'view InvoiceById — eventually',
        detail: `{invoice_id: "inv-1", total: ${AMOUNT}}`,
      },
      {
        kind: 'note',
        text: 'view OutstandingInvoices unchanged',
        detail: 'filter state == Issued, and this invoice is Draft',
      },
    ],
  },
  {
    id: 'command-2',
    label: 'resolve CreateInvoice, again',
    note: 'The same command, a second input — the one the specification refuses.',
    source: [[145, 155]],
    ir: ['cmd.create', 'cmd.create.input'],
    effects: [
      {
        kind: 'call',
        text: 'CreateInvoice',
        detail: `{customer_email: "ada@example.com", amount: ${ZERO}}`,
      },
    ],
  },
  {
    id: 'guard-2',
    label: 'evaluate the guard',
    note: 'Money declares amount >= 0, so 0.00 is the refusal this model can express — a negative one is not a Money at all.',
    source: [
      [27, 29],
      [164, 166],
      [188, 188],
    ],
    ir: ['ty.money', 'cmd.create.accepted', 'cmd.create.rejected'],
    effects: [{kind: 'guard', text: 'when amount.amount > 0', detail: 'false — the branch left is rejected'}],
  },
  {
    id: 'rejected',
    label: 'outcome rejected',
    note: 'An error carries what a caller needs in order to react, not just a name.',
    source: [
      [188, 190],
      [128, 133],
    ],
    ir: ['cmd.create.rejected', 'err.invalid'],
    effects: [
      {
        kind: 'rejected',
        text: 'outcome: rejected — billing.invoice.InvalidAmount',
        detail: `{submitted: ${ZERO}}`,
      },
    ],
  },
  {
    id: 'nothing',
    label: 'nothing moved',
    note: 'A refusal declares no subject; declaring one is refused as refusal_mutated_state.',
    source: [[189, 190]],
    ir: ['cmd.create.rejected'],
    effects: [
      {
        kind: 'nothing',
        text: 'nothing created — the money did not move',
        detail: 'no entity, no event, no view change',
      },
    ],
  },
];

/** The glyph each ledger entry opens with. */
export const EFFECT_GLYPH: Record<EffectKind, string> = {
  call: '→',
  guard: '?',
  accepted: '✓',
  rejected: '✗',
  entity: '+',
  event: '⚡',
  view: '~',
  nothing: '∅',
  note: '·',
};
