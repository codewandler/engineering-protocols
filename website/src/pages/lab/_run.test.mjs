// The lab's engine, held to the module it runs.
//
// Not a browser test and not a second copy of the boundary smoke test under
// `examples/billing-web/` — that one proves the crossing (JSON in over linear memory, JSON out).
// This one proves the *narration*: that the steps the three panels render are derived from what
// the module answered, that the derivation is deterministic, and that every line range it points
// at exists in the file the left panel shows.
//
// It loads the same `.wasm` the page fetches, through the same glue the page imports, and calls
// the same `buildRun` the page calls. There is no stub anywhere in it.
//
// Usage, from `website/`:
//
//   npm run test:lab
//
// It needs two things. The module: `task lab` from the repository root builds it and puts it in
// `static/lab/`. And Node 22.18 or newer, because it imports the engine as TypeScript and lets
// Node strip the types — the alternative was a build step between the page's source and its test,
// which is exactly the gap a test like this exists to close.

import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

import {open} from './_bridge.mjs';
import {buildRun} from './_run.ts';
import {INVOICE_YAML_LINES} from './_source.ts';

const MODULE = fileURLToPath(new URL('../../../static/lab/billing_web_realized.wasm', import.meta.url));

const failures = [];

/** Records a failure rather than throwing, so one run reports everything wrong. */
function check(claim, held, detail) {
  if (!held) failures.push(`${claim}${detail === undefined ? '' : ` — ${detail}`}`);
}

let bytes;
try {
  bytes = await readFile(MODULE);
} catch {
  console.error(`the module is not built: ${MODULE}\nbuild it with \`task lab\` from the repository root.`);
  process.exit(2);
}

const first = await open(bytes);
check('the module carries a realization', first.realized === true);
const run = buildRun(first.request, first.realized);

// ---- the stream is what the script produced, and all of it ---------------------------------------

check('the run has steps', run.steps.length > 0, run.steps.length);
check(
  'it opens by loading the module',
  run.steps[0].id === 'load' && run.steps[0].effects[0].text === 'billing v3 compiled',
  JSON.stringify(run.steps[0].effects[0]),
);
check(
  'the load step says a realization is installed',
  run.steps[0].effects.some((effect) => effect.kind === 'accepted' && effect.text === 'a realization is installed'),
  JSON.stringify(run.steps[0].effects),
);
check(
  'the first command is the one the script sends',
  run.steps[1].id === '0.call' && run.steps[1].effects[0].text === 'CreateInvoice',
  JSON.stringify(run.steps[1]),
);
check(
  'the identity in the run came from the module, not from the page',
  run.steps.some((step) =>
    step.effects.some((effect) => effect.detail?.includes('00000000-0000-4000-8000-000000000001')),
  ),
);
check(
  'the invoice reaches Paid through the declared transitions',
  run.steps.some((step) => step.effects.some((effect) => effect.detail === 'Draft → Issued via issue')) &&
    run.steps.some((step) => step.effects.some((effect) => effect.detail === 'Issued → Paid via settle')),
  JSON.stringify(run.steps.filter((step) => step.id.endsWith('.subject')).map((step) => step.effects[0])),
);
check(
  'cancelling a Paid invoice is refused with the state it is actually in',
  run.steps.some((step) =>
    step.effects.some(
      (effect) =>
        effect.kind === 'rejected' &&
        effect.text.includes('billing.invoice.InvoiceStateConflict') &&
        effect.detail === '{state: "Paid"}',
    ),
  ),
);
const last = run.steps[run.steps.length - 1];
check(
  'it ends on the refusal changing nothing',
  last.id === '4.nothing' && last.effects[0].kind === 'nothing',
  JSON.stringify(last),
);
check(
  'and that claim was checked against the record, not asserted',
  last.effects[0].detail === 'the record still holds 4 occurrence(s), and no view changed',
  last.effects[0].detail,
);

// ---- the transport is on the record ---------------------------------------------------------------

const transport = run.steps.find((step) => step.id.endsWith('.transport'));
check('the binding reacted, and the step says what it filled', transport !== undefined);
check(
  'the address the invoice was created with is the address the email went to',
  transport &&
    transport.effects.some((effect) => effect.detail?.includes('recipient: "ada@example.com"')) &&
    transport.effects.some((effect) => effect.text === 'event EmailSent'),
  transport && JSON.stringify(transport.effects),
);

// ---- every highlight points at a line the left panel has -------------------------------------------

for (const step of run.steps) {
  for (const [from, to] of step.source) {
    check(
      `step ${step.id} highlights lines that exist`,
      from >= 1 && to >= from && to <= INVOICE_YAML_LINES.length,
      `${from}-${to} of ${INVOICE_YAML_LINES.length}`,
    );
  }
  for (const id of step.ir) {
    check(`step ${step.id} stands on rows the middle panel has`, run.ir.some((row) => row.id === id), id);
  }
}
check(
  'the highlight for the accepted outcome is the accepted outcome',
  INVOICE_YAML_LINES[run.steps[3].source[0][0] - 1].trim() === '- name: accepted',
  INVOICE_YAML_LINES[run.steps[3].source[0][0] - 1],
);

// ---- the intermediate layer is the catalogue, not a drawing of it ------------------------------------

check('every declaration has a row', run.ir.length > 40, run.ir.length);
check('the system row is first', run.ir[0].kind === 'system' && run.ir[0].label === 'billing', run.ir[0].label);
check(
  'the domain the source file declares comes first',
  run.ir[1].kind === 'domain' && run.ir[1].label === 'billing.invoice',
  run.ir[1].label,
);
check(
  'the derived state enum says it is derived',
  run.ir.some((row) => row.id === 'ty.billing.invoice.Invoice.State' && row.detail.includes('derived from the lifecycle')),
);
check(
  'the binding is a row of its own',
  run.ir.some((row) => row.kind === 'binding' && row.label === 'notify-on-invoice-created'),
);

// ---- the same module answers the same run, every time ------------------------------------------------

const second = await open(bytes);
const again = buildRun(second.request, second.realized);
check(
  'a second instantiation produces a byte-identical stream',
  JSON.stringify(again) === JSON.stringify(run),
  'the run is not deterministic',
);

if (failures.length) {
  console.error(`the lab's run failed ${failures.length} claim(s):`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  `the lab's run: ${run.steps.length} steps over ${run.ir.length} rows, deterministic, every highlight in range`,
);
