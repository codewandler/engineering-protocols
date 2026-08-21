/**
 * The lab's engine: a real system, running, and the three panels read off what it answers.
 *
 * NOTHING HERE IS INVENTED, and that is the whole change from the layout draft this replaces. The
 * draft shipped a hardcoded array of eleven steps with real names in it; this file has no steps in
 * it at all. It sends a fixed script of commands to `billing_web_realized.wasm` — the browser
 * realization the repository already synthesises, `generated/web/billing/` linked with
 * `examples/billing-realization`'s hand-written behaviour — and turns what comes back into the
 * steps the panels render.
 *
 * So each panel now stands on something:
 *
 * | panel | where its content comes from |
 * | --- | --- |
 * | the source | `_source.ts`, `invoice.yaml` verbatim |
 * | the intermediate layer | `{"request":"catalog"}` — the compiler's own model, out of the module |
 * | the run | the outcome, the log, the binding invocations and the view rows of each command |
 * | the highlight | `_spans.ts`, which finds each declaration's lines in the file itself |
 *
 * # Determinism
 *
 * The same module and the same script produce a byte-identical stream of steps, every load. There
 * is no `Date.now` and no `Math.random` on this path, and there is none inside the module either:
 * identifiers come from a per-store counter in the `Uuid` wire shape (invariant 9), which is why
 * the invoice below is always `…0001`. `_run.test.mjs` holds the stream to that.
 *
 * # The three invented values
 *
 * An email address and two amounts. A specification declares types and not instances, so somebody
 * has to choose an input — and the choice is confined to [`SCRIPT`], where it can be read at a
 * glance. Everything the run then says about those values came back from the module.
 */

import {open} from './_bridge.mjs';
import {head, PRINCIPAL_DOMAIN, ranges, span} from './_spans.ts';
import type {LineRange} from './_spans.ts';

export type {LineRange};

/* ------------------------------------------------------------------ *
 * What the module answers with
 * ------------------------------------------------------------------ */

/** A type as the compiler resolved it — enough of it to spell the reference back. */
type TypeRef = {kind: string; name?: string; of?: TypeRef; key?: string; value?: TypeRef};

/** A declared field, with the spelling the specification used for its type. */
type Field = {name: string; optional: boolean; spelling: string; wire: string; type: TypeRef};

/** One declared type: a newtype, a struct, an enum or a tagged union. */
type DeclaredType = {
  display: string;
  kind: string;
  spelling?: string;
  of?: TypeRef;
  fields?: Field[];
  invariants?: string[];
  variants?: string[] | Record<string, {spelling: string; type: TypeRef}>;
  tag?: string;
};

/** What a command's outcome does when it is the one taken. */
type Moves = {
  entity: string;
  effect: {kind: string; transition?: string; from?: string[]; to?: string};
};

/** One declared outcome of a command. */
type Outcome = {
  name: string;
  condition: string;
  error: string | null;
  moves: Moves | null;
  publishes: string[];
  summary: string | null;
};

/** One declared command, with the component that accepts it. */
type Command = {
  name: string;
  display: string;
  domain: string;
  component: string;
  dispatchable: boolean;
  input: Field[];
  outcomes: Outcome[];
  behavior: {contract: string; disposition: string; why: string};
};

/** One declared entity, its identity, its fields and its lifecycle. */
type Entity = {
  name: string;
  display: string;
  identity: Field;
  fields: Field[];
  invariants: string[];
  initial: string;
  states: string[];
  terminal: string[];
  transitions: {name: string; from: string[]; to: string}[];
  views: string[];
};

/** One declared event or error. */
type Announcement = {name: string; display?: string; fields: Field[]; summary: string | null};

/** One declared view, with the consistency a generated scenario asserts it at. */
type View = {
  name: string;
  display: string;
  entity: string;
  consistency: string;
  filter: string | null;
  fields: Field[];
};

/** One declared binding: the reaction the transport carries. */
type Binding = {
  name: string;
  event: string;
  command: string;
  delivery: string;
  on_failure: string;
  escalation: string | null;
};

/** The model a page renders itself from, as `{"request":"catalog"}` answers it. */
type Catalog = {
  system: string;
  display: string;
  version: string;
  summary: string | null;
  types: Record<string, DeclaredType>;
  entities: Entity[];
  commands: Command[];
  events: Announcement[];
  errors: Announcement[];
  views: View[];
  bindings: Binding[];
  plan: {generated: number; obligations: number; refused: number};
  provenance: Record<string, string>;
};

/** A value that crossed the boundary as JSON. */
type Json = string | number | boolean | null | Json[] | {[key: string]: Json};

/** One occurrence in the system's published record. */
type LogEntry = {occurrence: number; event: string; payload: Record<string, Json>};

/** One command a binding invoked, with the input it filled. */
type Invocation = {
  binding: string;
  event: string;
  command: string;
  input: Record<string, Json>;
};

/** A declared view's rows, or the refusal serving it answered with. */
type ViewRows = {rows?: Record<string, Json>[]; unmet?: {capability: string; source: string}};

/** What one command answered: the outcome it took, and the whole observable surface after it. */
type CommandAnswer = {
  ok: boolean;
  command?: string;
  outcome?: {
    outcome: string;
    published: {event: string; payload: Record<string, Json>}[];
    refusal?: {error: string; payload: Record<string, Json>};
  };
  log?: LogEntry[];
  invocations?: Invocation[];
  views?: Record<string, ViewRows>;
  catalog?: Catalog;
  error?: {kind: string; [key: string]: Json};
};

/** The one function the boundary offers: JSON in, JSON out. */
export type Request = (body: unknown) => CommandAnswer;

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
  | 'view'
  | 'binding';

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

export type Step = {
  id: string;
  /** What the run-state widget calls this step. */
  label: string;
  /** One line: what the step is doing, in the specification's own words where there are any. */
  note: string;
  source: LineRange[];
  ir: string[];
  effects: Effect[];
};

/** Everything the lab renders, all of it read out of one module. */
export type Run = {
  ir: IrRow[];
  steps: Step[];
  /** What the toolbar calls this run. */
  title: string;
  /** Whether the module carried a realization, or answers every command with what it is owed. */
  realized: boolean;
};

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

/* ------------------------------------------------------------------ *
 * The script
 * ------------------------------------------------------------------ */

/** The amount the specification accepts, and the one it refuses. */
const AMOUNT = {amount: '120.00', currency: 'EUR'};

/**
 * Zero rather than a negative amount, and that is not a softening.
 *
 * `Money` declares the invariant `amount >= 0`, so a negative amount is not a `Money` at all and
 * could never reach the guard. `0.00` is the refusal this specification can actually express.
 */
const ZERO = {amount: '0.00', currency: 'EUR'};

/** The one address in the script. */
const CUSTOMER = 'ada@example.com';

/** What earlier exchanges left behind for later ones to name. */
type Held = {invoice: string};

/** One command sent to the module, and why it is in the script. */
type Exchange = {
  command: string;
  input: (held: Held) => Record<string, Json>;
  why: string;
};

/**
 * Five commands: one accepted, two moves, one move the lifecycle does not have, one refusal.
 *
 * Every declared outcome kind of this specification appears — an acceptance, a transition, a
 * refusal decided by a guard, and a refusal decided by the state the instance is in — so a reader
 * who steps to the end has seen the model answer in all four ways.
 */
const SCRIPT: Exchange[] = [
  {
    command: 'billing.invoice.CreateInvoice',
    input: () => ({customer_email: CUSTOMER, amount: AMOUNT}),
    why: 'One accepted command, and everything the system does because of it.',
  },
  {
    command: 'billing.invoice.IssueInvoice',
    input: (held) => ({invoice_id: held.invoice}),
    why: 'The first declared move: the invoice the previous command created leaves Draft.',
  },
  {
    command: 'billing.invoice.PayInvoice',
    input: (held) => ({invoice_id: held.invoice, amount: AMOUNT}),
    why: 'Issued → Paid, the transition the design uses as its worked example.',
  },
  {
    command: 'billing.invoice.CancelInvoice',
    input: (held) => ({invoice_id: held.invoice}),
    why: 'Cancelling a Paid invoice — a move the lifecycle does not have, asked for anyway.',
  },
  {
    command: 'billing.invoice.CreateInvoice',
    input: () => ({customer_email: CUSTOMER, amount: ZERO}),
    why: 'The refusal this specification can express: the guard is false and nothing is created.',
  },
];

/* ------------------------------------------------------------------ *
 * Rendering values the way the specification spells them
 * ------------------------------------------------------------------ */

/** The last segment of a qualified name — the domain prefix is the row above it. */
function short(name: string): string {
  const at = name.lastIndexOf('.');
  return at < 0 ? name : name.slice(at + 1);
}

/** A condition as the catalogue words it, without the backticks a Markdown reader wanted. */
function plain(text: string): string {
  return text.replace(/`/g, '');
}

/**
 * A JSON value in the specification's own shape: unquoted keys, quoted strings, no line breaks.
 *
 * Deterministic by construction — object members come back in the order the module wrote them,
 * and nothing here sorts or re-formats.
 */
function brief(value: Json | undefined): string {
  if (value === undefined) {
    return '';
  }
  if (typeof value === 'string') {
    return `"${value}"`;
  }
  if (value === null || typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(brief).join(', ')}]`;
  }
  return `{${Object.entries(value)
    .map(([key, member]) => `${key}: ${brief(member)}`)
    .join(', ')}}`;
}

/** The declaration the module named, or a failure that says which one it was. */
function must<T>(value: T | undefined, what: string): T {
  if (value === undefined) {
    throw new Error(`the module answered with ${what}, which its own catalogue does not declare`);
  }
  return value;
}

/* ------------------------------------------------------------------ *
 * The intermediate layer, built from the catalogue
 * ------------------------------------------------------------------ */

/** How one declared type reads on a single row. */
function typeDetail(name: string, declared: DeclaredType, entities: Entity[]): string {
  const parts: string[] = [];
  switch (declared.kind) {
    case 'newtype':
      parts.push(`newtype of ${declared.spelling ?? declared.of?.name ?? '?'}`);
      break;
    case 'struct':
      parts.push('struct');
      parts.push((declared.fields ?? []).map((field) => `${field.name}: ${field.spelling}`).join(', '));
      break;
    case 'enum':
      parts.push('enum');
      parts.push((Array.isArray(declared.variants) ? declared.variants : []).join(' | '));
      break;
    case 'union':
      parts.push(`union tagged ${declared.tag ?? '?'}`);
      parts.push(
        Object.entries(
          Array.isArray(declared.variants) ? {} : (declared.variants ?? {}),
        )
          .map(([variant, of]) => `${variant}: ${of.spelling}`)
          .join(', '),
      );
      break;
    default:
      parts.push(declared.kind);
  }
  for (const invariant of declared.invariants ?? []) {
    parts.push(`invariant ${invariant}`);
  }
  // The state enum is not declared anywhere in the file: the compiler derives it from the
  // lifecycle, and a row that did not say so would look like a type somebody forgot to write.
  if (entities.some((entity) => `${entity.name}.State` === name)) {
    parts.push('derived from the lifecycle');
  }
  return parts.filter((part) => part !== '').join(' · ');
}

/**
 * What an outcome does, without the condition that reached it.
 *
 * The condition already has its own step — the guard — so repeating it under the outcome would
 * spend a line of the ledger saying what the line above it said.
 */
function subjectAndEmissions(outcome: Outcome): string {
  const parts: string[] = [];
  if (outcome.moves) {
    const effect = outcome.moves.effect;
    parts.push(
      effect.kind === 'creates'
        ? `creates ${short(outcome.moves.entity)}`
        : `moves ${short(outcome.moves.entity)}.${effect.transition ?? '?'} → ${effect.to ?? '?'}`,
    );
  }
  for (const event of outcome.publishes) {
    parts.push(`emits ${short(event)}`);
  }
  if (outcome.error) {
    parts.push(`reports ${short(outcome.error)}`);
  }
  return parts.join(' · ');
}

/** How one outcome reads on a single row: the condition that reaches it, then what it does. */
function outcomeDetail(outcome: Outcome): string {
  return [plain(outcome.condition), subjectAndEmissions(outcome)]
    .filter((part) => part !== '')
    .join(' · ');
}

/** The whole model, as rows, in the order the catalogue holds it. */
function buildIr(catalog: Catalog): IrRow[] {
  const rows: IrRow[] = [];
  const counts = [
    `${Object.keys(catalog.types).length} types`,
    `${catalog.entities.length} ${catalog.entities.length === 1 ? 'entity' : 'entities'}`,
    `${catalog.commands.length} commands`,
    `${catalog.events.length} events`,
    `${catalog.errors.length} errors`,
    `${catalog.views.length} views`,
  ].join(' · ');
  rows.push({
    id: 'sys',
    depth: 0,
    kind: 'system',
    label: catalog.system,
    detail: `${catalog.version} · ${counts}`,
  });

  // The domains, with the one this file declares first: the panel beside it shows that file, so
  // a reader looking for what they are reading should not have to scroll past another context.
  const domains = [...new Set(catalog.commands.map((command) => command.domain))].sort((left, right) => {
    if (left === PRINCIPAL_DOMAIN) return -1;
    if (right === PRINCIPAL_DOMAIN) return 1;
    return left.localeCompare(right);
  });
  const owner = (name: string): string =>
    domains.find((domain) => name.startsWith(`${domain}.`)) ?? domains[0];

  for (const domain of domains) {
    const types = Object.entries(catalog.types).filter(([name]) => owner(name) === domain);
    const entities = catalog.entities.filter((entity) => owner(entity.name) === domain);
    const commands = catalog.commands.filter((command) => command.domain === domain);
    const events = catalog.events.filter((event) => owner(event.name) === domain);
    const errors = catalog.errors.filter((error) => owner(error.name) === domain);
    const views = catalog.views.filter((view) => owner(view.name) === domain);

    rows.push({
      id: `dom.${domain}`,
      depth: 1,
      kind: 'domain',
      label: domain,
      detail: [...new Set(commands.map((command) => command.component))].join(', '),
    });

    if (types.length > 0) {
      rows.push({id: `g.${domain}.types`, depth: 2, kind: 'group', label: 'types', detail: `${types.length} declared`});
      for (const [name, declared] of types) {
        rows.push({
          id: `ty.${name}`,
          depth: 3,
          kind: 'type',
          label: short(name),
          detail: typeDetail(name, declared, catalog.entities),
        });
      }
    }

    if (entities.length > 0) {
      rows.push({
        id: `g.${domain}.entities`,
        depth: 2,
        kind: 'group',
        label: 'entities',
        detail: `${entities.length} declared`,
      });
    }
    for (const entity of entities) {
      rows.push({
        id: `ent.${entity.name}`,
        depth: 3,
        kind: 'entity',
        label: short(entity.name),
        detail: [
          `identity ${entity.identity.name}: ${entity.identity.spelling}`,
          `${entity.fields.length} fields`,
          ...entity.invariants.map((invariant) => `invariant ${invariant}`),
        ].join(' · '),
      });
      rows.push({
        id: `lc.${entity.name}`,
        depth: 4,
        kind: 'lifecycle',
        label: 'lifecycle',
        detail: [
          `initial ${entity.initial}`,
          ...entity.transitions.map(
            (transition) => `${transition.name}: ${transition.from.join(' | ')} → ${transition.to}`,
          ),
          `terminal ${entity.terminal.join(', ')}`,
        ].join(' · '),
      });
    }

    if (commands.length > 0) {
      rows.push({
        id: `g.${domain}.commands`,
        depth: 2,
        kind: 'group',
        label: 'commands',
        detail: `${commands.length} declared`,
      });
      for (const command of commands) {
        rows.push({
          id: `cmd.${command.name}`,
          depth: 3,
          kind: 'command',
          label: short(command.name),
          detail: `${command.component} · behaviour is an ${command.behavior.disposition}`,
        });
        rows.push({
          id: `in.${command.name}`,
          depth: 4,
          kind: 'input',
          label: 'input',
          detail: command.input.map((field) => `${field.name}: ${field.spelling}`).join(', '),
        });
        for (const outcome of command.outcomes) {
          rows.push({
            id: `out.${command.name}.${outcome.name}`,
            depth: 4,
            kind: 'outcome',
            label: outcome.name,
            detail: outcomeDetail(outcome),
          });
        }
      }
    }

    if (events.length > 0) {
      rows.push({id: `g.${domain}.events`, depth: 2, kind: 'group', label: 'events', detail: `${events.length} declared`});
      for (const event of events) {
        rows.push({
          id: `evt.${event.name}`,
          depth: 3,
          kind: 'event',
          label: short(event.name),
          detail: event.fields.map((field) => `${field.name}: ${field.spelling}`).join(', '),
        });
      }
    }

    if (errors.length > 0) {
      rows.push({id: `g.${domain}.errors`, depth: 2, kind: 'group', label: 'errors', detail: `${errors.length} declared`});
      for (const error of errors) {
        const carried = error.fields.map((field) => `${field.name}: ${field.spelling}`).join(', ');
        rows.push({
          id: `err.${error.name}`,
          depth: 3,
          kind: 'error',
          label: short(error.name),
          detail: [carried, error.summary ?? ''].filter((part) => part !== '').join(' — '),
        });
      }
    }

    if (views.length > 0) {
      rows.push({id: `g.${domain}.views`, depth: 2, kind: 'group', label: 'views', detail: `${views.length} declared`});
      for (const view of views) {
        rows.push({
          id: `view.${view.name}`,
          depth: 3,
          kind: 'view',
          label: short(view.name),
          detail: [
            `from ${short(view.entity)}`,
            view.consistency,
            view.filter ? `filter ${view.filter}` : '',
          ]
            .filter((part) => part !== '')
            .join(' · '),
        });
      }
    }
  }

  if (catalog.bindings.length > 0) {
    rows.push({
      id: 'g.bindings',
      depth: 1,
      kind: 'group',
      label: 'bindings',
      detail: `${catalog.bindings.length} declared — they are what crosses a domain`,
    });
    for (const binding of catalog.bindings) {
      rows.push({
        id: `bind.${binding.name}`,
        depth: 2,
        kind: 'binding',
        label: binding.name,
        detail: [
          `${short(binding.event)} → ${short(binding.command)}`,
          binding.delivery,
          `on failure ${binding.on_failure}`,
        ].join(' · '),
      });
    }
  }

  return rows;
}

/* ------------------------------------------------------------------ *
 * The run, step by step, from what each command answered
 * ------------------------------------------------------------------ */

/** What the system looked like before an exchange, so the step can say what the exchange changed. */
type Before = {
  log: LogEntry[];
  invocations: Invocation[];
  views: Record<string, ViewRows>;
};

/** The observable surface, as one value. */
function surfaceOf(answer: CommandAnswer): Before {
  return {
    log: answer.log ?? [],
    invocations: answer.invocations ?? [],
    views: answer.views ?? {},
  };
}

/** A view's rows as one comparable string — the module writes them in a fixed order. */
function rowsOf(view: ViewRows | undefined): string {
  return JSON.stringify(view?.rows ?? view?.unmet ?? null);
}

/** The first step: what was loaded, and whether anything is behind it. */
function loadStep(catalog: Catalog, realized: boolean): Step {
  return {
    id: 'load',
    label: 'load the module',
    note: `${catalog.system} ${catalog.version} — the panels below are read out of this module, not written beside it.`,
    source: ranges(span('domain'), span('summary'), span('naming')),
    ir: ['sys', `dom.${PRINCIPAL_DOMAIN}`],
    effects: [
      {
        kind: 'note',
        text: `${catalog.system} ${catalog.version} compiled`,
        detail: `${Object.keys(catalog.types).length} types · ${catalog.commands.length} commands · ${catalog.events.length} events · ${catalog.errors.length} errors · ${catalog.views.length} views · ${catalog.bindings.length} binding`,
      },
      {
        kind: 'note',
        text: `plan: ${catalog.plan.generated} generated · ${catalog.plan.obligations} obligations · ${catalog.plan.refused} refused`,
        detail: `model digest ${(catalog.provenance.source_digest ?? '').slice(0, 16)}…`,
      },
      realized
        ? {
            kind: 'accepted',
            text: 'a realization is installed',
            detail: 'every command below runs the hand-written behaviour the plan owes',
          }
        : {
            kind: 'rejected',
            text: 'no realization is installed',
            detail: 'every command below answers with the obligation nothing has satisfied',
          },
    ],
  };
}

/**
 * One exchange, as the steps a reader walks through.
 *
 * The shape of the walk is fixed — call, guard, outcome, subject, announcement, transport,
 * projection — but every step in it is emitted only if the module's answer contains it. A command
 * with one outcome gets no guard step; a refusal gets no subject; an exchange nothing reacted to
 * gets no transport step.
 */
function exchangeSteps(
  catalog: Catalog,
  exchange: Exchange,
  input: Record<string, Json>,
  answer: CommandAnswer,
  before: Before,
  at: number,
): Step[] {
  const steps: Step[] = [];
  const name = exchange.command;
  const command = must(
    catalog.commands.find((candidate) => candidate.name === name),
    `the command ${name}`,
  );
  const result = must(answer.outcome, `no outcome for ${name}`);
  const taken = must(
    command.outcomes.find((candidate) => candidate.name === result.outcome),
    `the outcome ${name}.${result.outcome}`,
  );
  const after = surfaceOf(answer);

  steps.push({
    id: `${at}.call`,
    label: `call ${short(name)}`,
    note: exchange.why,
    source: ranges(head('commands', name), span('commands', name, 'naming'), span('commands', name, 'input')),
    ir: [`cmd.${name}`, `in.${name}`],
    effects: [{kind: 'call', text: short(name), detail: brief(input)}],
  });

  if (command.outcomes.length > 1) {
    steps.push({
      id: `${at}.guard`,
      label: 'decide the outcome',
      note: `${command.outcomes.length} declared outcomes, and which one this input lands in is a declaration rather than a branch someone wrote.`,
      source: ranges(
        ...command.outcomes.map((outcome) => head('commands', name, 'outcomes', outcome.name)),
      ),
      ir: command.outcomes.map((outcome) => `out.${name}.${outcome.name}`),
      effects: [
        {
          kind: 'guard',
          text: plain(taken.condition),
          detail: `${taken.name} — the branch this input takes`,
        },
      ],
    });
  }

  const refusal = result.refusal;
  steps.push({
    id: `${at}.outcome`,
    label: `outcome ${taken.name}`,
    note: taken.summary ?? `The command took its ${taken.name} outcome.`,
    source: ranges(
      span('commands', name, 'outcomes', taken.name),
      refusal ? span('errors', refusal.error) : undefined,
    ),
    ir: [`out.${name}.${taken.name}`, ...(refusal ? [`err.${refusal.error}`] : [])],
    effects: [
      refusal
        ? {
            kind: 'rejected',
            text: `outcome: ${taken.name} — ${refusal.error}`,
            detail: brief(refusal.payload),
          }
        : {
            kind: 'accepted',
            text: `outcome: ${taken.name}`,
            detail: subjectAndEmissions(taken),
          },
    ],
  });

  const moves = taken.moves;
  if (moves) {
    const entity = must(
      catalog.entities.find((candidate) => candidate.name === moves.entity),
      `the entity ${moves.entity}`,
    );
    const effect = moves.effect;
    const identity =
      result.published
        .map((published) => published.payload[entity.identity.name])
        .find((value): value is string => typeof value === 'string') ??
      (typeof input[entity.identity.name] === 'string' ? (input[entity.identity.name] as string) : '?');
    const creates = effect.kind === 'creates';
    steps.push({
      id: `${at}.subject`,
      label: creates ? `create the ${short(entity.name).toLowerCase()}` : `move ${effect.transition ?? ''}`,
      note: creates
        ? `creates: is not a transition — a new instance has no state to move out of, so it starts at the lifecycle's initial, ${entity.initial}.`
        : `The move is declared in the entity's lifecycle and the command names it; nothing infers one from the other's spelling.`,
      source: creates
        ? ranges(head('entities', entity.name), span('entities', entity.name, 'identity'), span('entities', entity.name, 'lifecycle', 'initial'))
        : ranges(span('entities', entity.name, 'lifecycle', 'transitions', effect.transition ?? '')),
      ir: [`ent.${entity.name}`, `lc.${entity.name}`],
      effects: [
        {
          kind: 'entity',
          text: `entity ${short(entity.name)} ${identity}`,
          detail: creates
            ? `state ${entity.initial} — the lifecycle's initial, reached without a transition`
            : `${(effect.from ?? []).join(' | ')} → ${effect.to ?? '?'} via ${effect.transition ?? '?'}`,
        },
      ],
    });
  }

  result.published.forEach((published, index) => {
    const declared = catalog.events.find((event) => event.name === published.event);
    steps.push({
      id: `${at}.event.${index}`,
      label: `announce ${short(published.event)}`,
      note: declared
        ? `The announcement carries ${declared.fields.map((field) => field.name).join(', ')} — declared fields, with values the outcome's payload block says where to find.`
        : 'The command announced a fact.',
      source: ranges(
        span('events', published.event),
        span('commands', name, 'outcomes', taken.name, 'payload'),
      ),
      ir: [`evt.${published.event}`],
      effects: [
        {kind: 'event', text: `event ${short(published.event)}`, detail: brief(published.payload)},
      ],
    });
  });

  // What the transport did with the announcement. Everything in the log beyond the command's own
  // announcements got there because a binding reacted, which is the one part of a run no single
  // declaration in this file describes.
  const reacted = after.invocations.slice(before.invocations.length);
  if (reacted.length > 0) {
    const carried = after.log.slice(before.log.length + result.published.length);
    const bindings = reacted
      .map((invocation) => catalog.bindings.find((binding) => binding.name === invocation.binding))
      .filter((binding): binding is Binding => binding !== undefined);
    steps.push({
      id: `${at}.transport`,
      label: 'deliver to the binding',
      note: `A binding is a declared reaction, and it is declared in system.yaml — the file beside this one, which this panel does not show.`,
      source: [],
      ir: [
        ...bindings.map((binding) => `bind.${binding.name}`),
        ...reacted.map((invocation) => `cmd.${invocation.command}`),
      ],
      effects: [
        ...reacted.map((invocation, index): Effect => {
          const binding = bindings[index];
          return {
            kind: 'note',
            text: `binding ${invocation.binding}`,
            detail: `${binding ? `${binding.delivery} / on failure ${binding.on_failure} · ` : ''}${short(invocation.command)} ${brief(invocation.input)}`,
          };
        }),
        ...carried.map(
          (entry): Effect => ({
            kind: 'event',
            text: `event ${short(entry.event)}`,
            detail: brief(entry.payload),
          }),
        ),
      ],
    });
  }

  const moved = catalog.views.filter(
    (view) => rowsOf(before.views[view.name]) !== rowsOf(after.views[view.name]),
  );
  if (moved.length > 0) {
    steps.push({
      id: `${at}.views`,
      label: 'project the views',
      note: `A view's consistency is what a generated scenario asserts it with: ${moved
        .map((view) => `${short(view.name)} ${view.consistency}`)
        .join(', ')}.`,
      source: ranges(...moved.map((view) => span('views', view.name))),
      ir: moved.map((view) => `view.${view.name}`),
      effects: catalog.views.map((view): Effect => {
        const rows = after.views[view.name]?.rows ?? [];
        if (!moved.includes(view)) {
          return {
            kind: 'note',
            text: `view ${short(view.name)} unchanged`,
            detail: view.filter ? `filter ${view.filter}` : `${rows.length} row(s), none of them touched`,
          };
        }
        return {
          kind: 'view',
          text: `view ${short(view.name)} — ${view.consistency}`,
          detail: rows.length > 0 ? rows.map((row) => brief(row)).join(' · ') : 'no rows',
        };
      }),
    });
  }

  // The claim a refusal makes, checked rather than asserted: it is only written when the log, the
  // invocations and every view came back identical to what they were before the command.
  const untouched =
    refusal !== undefined &&
    result.published.length === 0 &&
    after.log.length === before.log.length &&
    after.invocations.length === before.invocations.length &&
    moved.length === 0;
  if (untouched) {
    steps.push({
      id: `${at}.nothing`,
      label: 'nothing moved',
      note: 'A refusal declares no subject, and declaring one is refused as refusal_mutated_state: a refused command changes nothing.',
      source: ranges(head('commands', name, 'outcomes', taken.name)),
      ir: [`out.${name}.${taken.name}`],
      effects: [
        {
          kind: 'nothing',
          text: 'nothing moved',
          detail: `the record still holds ${after.log.length} occurrence(s), and no view changed`,
        },
      ],
    });
  }

  return steps;
}

/**
 * Runs the script against one module and answers everything the lab renders.
 *
 * `request` is the boundary — `open()`'s driver in a browser, and the same function over the same
 * module in the test. Nothing else is injected, because nothing else is a decision: the script is
 * fixed and the module is deterministic, so this returns the same value every time it is called
 * against the same `.wasm`.
 *
 * @throws if the module answers something its own catalogue does not declare, which is a fault in
 * the module rather than in the page, and is shown as one.
 */
export function buildRun(request: Request, realized: boolean): Run {
  const answered = request({request: 'catalog'});
  const catalog = must(answered.catalog, 'no catalogue');

  const steps: Step[] = [loadStep(catalog, realized)];
  let before = surfaceOf(request({request: 'observe'}));
  const held: Held = {invoice: ''};

  SCRIPT.forEach((exchange, at) => {
    const input = exchange.input(held);
    const answer = request({request: 'command', command: exchange.command, input});
    if (!answer.ok) {
      const error = answer.error;
      throw new Error(
        `the module refused ${exchange.command}: ${error ? JSON.stringify(error) : 'for no stated reason'}`,
      );
    }
    steps.push(...exchangeSteps(catalog, exchange, input, answer, before, at));

    // What the next exchange names. The identity is the implementation's to assign, so the only
    // place it can be read is an announcement the run has already seen.
    const entity = catalog.entities[0];
    if (entity) {
      for (const published of answer.outcome?.published ?? []) {
        const identity = published.payload[entity.identity.name];
        if (typeof identity === 'string' && held.invoice === '') {
          held.invoice = identity;
        }
      }
    }
    before = surfaceOf(answer);
  });

  return {
    ir: buildIr(catalog),
    steps,
    title: `${catalog.system} ${catalog.version} · ${SCRIPT.length} commands`,
    realized,
  };
}

/**
 * Fetches the module beside the page, instantiates it, and runs the script through it.
 *
 * The `.wasm` is a build artifact and is not committed — `task lab` builds it and puts it here —
 * so a missing module is an ordinary outcome and comes back as a failure that names the command
 * which fixes it, never as a page that quietly shows nothing.
 */
export async function loadRun(url: string): Promise<{run: Run; bytes: number}> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `no module at ${url} (${response.status}). Build it with \`task lab\` and reload.`,
    );
  }
  const bytes = await response.arrayBuffer();
  const driver = await open(bytes);
  return {run: buildRun(driver.request as Request, driver.realized), bytes: bytes.byteLength};
}
