// The boundary smoke test: load the realized module outside a browser and drive it.
//
// It is a *boundary* test, not a suite. The billing system's twenty-seven scenarios are the
// committed conformance suite's, run natively against the same realization by
// `cargo xtask synth`; what nothing else covers is the crossing this target adds — JSON in over
// linear memory, JSON out, a catalogue built from the model, and a page that calls three exports.
// So this asserts exactly that crossing, end to end, once.
//
// It imports the page's own `bridge.js` rather than reimplementing the protocol, which is what
// makes it a test of the glue a browser will run and not of a second implementation that happens
// to agree.
//
// Usage: node smoke.mjs <bridge.js> <module.wasm>

import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const [glue, wasm] = process.argv.slice(2);
if (!glue || !wasm) {
  console.error("usage: node smoke.mjs <bridge.js> <module.wasm>");
  process.exit(2);
}

const failures = [];

/** Records a failure rather than throwing, so one run reports everything wrong (invariant 3). */
function check(claim, held, detail) {
  if (!held) failures.push(`${claim}${detail === undefined ? "" : ` — ${detail}`}`);
}

const { open } = await import(pathToFileURL(glue).href);
const system = await open(await readFile(wasm));

check("the module carries a realization", system.realized === true);

// ---- the catalogue is the model, not a copy of it ------------------------------------------------

const catalog = system.request({ request: "catalog" }).catalog;
check("the catalogue names the system", catalog.system === "billing", catalog.system);
check(
  "the catalogue lists every declared command",
  catalog.commands.length === 5,
  `${catalog.commands.length} command(s)`,
);
const create = catalog.commands.find((command) => command.name === "billing.invoice.CreateInvoice");
check("CreateInvoice is in the catalogue", create !== undefined);
check(
  "its input is typed from the model",
  create && create.input.map((field) => field.name).join(",") === "customer_email,amount",
  create && JSON.stringify(create.input.map((field) => field.name)),
);
check(
  "its declared refusal travels with it",
  create && create.outcomes.some((outcome) => outcome.error === "billing.invoice.InvalidAmount"),
);

// ---- one accepted command, and everything it set off ---------------------------------------------

const accepted = system.request({
  request: "command",
  command: "billing.invoice.CreateInvoice",
  input: { customer_email: "smoke@example.invalid", amount: { amount: "10.50", currency: "EUR" } },
});
check("the command was served", accepted.ok === true, JSON.stringify(accepted.error));
check(
  "it took the declared `accepted` outcome",
  accepted.ok && accepted.outcome.outcome === "accepted",
  accepted.ok && accepted.outcome.outcome,
);
check(
  "it published the declared event",
  accepted.ok && accepted.outcome.published.length === 1
    && accepted.outcome.published[0].event === "billing.invoice.InvoiceCreated",
);
check(
  "the amount crossed the boundary exactly",
  accepted.ok && accepted.outcome.published[0].payload.amount.amount === "10.50",
  accepted.ok && JSON.stringify(accepted.outcome.published[0].payload.amount),
);
check(
  "the transport delivered it to the binding, which published in turn",
  accepted.ok
    && accepted.log.map((entry) => entry.event).join(",")
      === "billing.invoice.InvoiceCreated,billing.email.EmailSent",
  accepted.ok && JSON.stringify(accepted.log.map((entry) => entry.event)),
);
check(
  "the binding's invocation is on the record, with what it filled",
  accepted.ok && accepted.invocations.length === 1
    && accepted.invocations[0].binding === "notify-on-invoice-created"
    && accepted.invocations[0].input.recipient === "smoke@example.invalid",
  accepted.ok && JSON.stringify(accepted.invocations),
);
check(
  "the declared view now holds the instance",
  accepted.ok && accepted.views["billing.invoice.InvoiceById"].rows.length === 1,
  accepted.ok && JSON.stringify(accepted.views["billing.invoice.InvoiceById"]),
);

// ---- a declared refusal is an outcome, not an error ----------------------------------------------

const refused = system.request({
  request: "command",
  command: "billing.invoice.CreateInvoice",
  input: { customer_email: "smoke@example.invalid", amount: { amount: "0", currency: "EUR" } },
});
check("a refusal is still a served request", refused.ok === true, JSON.stringify(refused.error));
check(
  "it took the declared `rejected` outcome, with the declared error",
  refused.ok && refused.outcome.outcome === "rejected"
    && refused.outcome.refusal.error === "billing.invoice.InvalidAmount",
  refused.ok && JSON.stringify(refused.outcome),
);
check(
  "and nothing was published for it",
  refused.ok && refused.log.length === 2,
  refused.ok && refused.log.length,
);

// ---- at least once means the duplicate is reachable, on purpose ------------------------------------

const again = system.request({ request: "redeliver", occurrence: 0 });
check("redelivering an occurrence is served", again.ok === true, JSON.stringify(again.error));
check(
  "the occurrence is not published a second time",
  again.ok && again.log.length === 3,
  again.ok && JSON.stringify(again.log.map((entry) => entry.event)),
);
check(
  "but the binding reacted again, which is the duplicate the guarantee permits",
  again.ok && again.invocations.length === 2,
  again.ok && again.invocations.length,
);

// ---- every refusal is a value, and none of them traps -----------------------------------------------

const unknown = system.request({ request: "command", command: "billing.invoice.Nonexistent", input: {} });
check(
  "an undeclared command is refused by kind",
  unknown.ok === false && unknown.error.kind === "unknown-command",
  JSON.stringify(unknown),
);
const mistyped = system.request({
  request: "command",
  command: "billing.invoice.CreateInvoice",
  input: { customer_email: 7, amount: { amount: "1", currency: "EUR" } },
});
check(
  "an input that does not match its declared type is refused with the path",
  mistyped.ok === false && mistyped.error.kind === "undecodable"
    && mistyped.error.at === "input.customer_email",
  JSON.stringify(mistyped),
);

if (failures.length) {
  console.error(`the browser boundary smoke test failed ${failures.length} claim(s):`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("browser boundary: 17 claims held — catalogue, dispatch, transport, view, refusal, redelivery");
