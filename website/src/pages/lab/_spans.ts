/**
 * Where each declaration sits in the specification, read out of the specification.
 *
 * The lab highlights lines while a run steps, and the numbers it highlights are not written down
 * anywhere: they are computed here, from [`INVOICE_YAML_LINES`], by walking the file's own
 * indentation. That is the difference between a highlight that is true and one that was true when
 * somebody typed it — refresh the copy in `_source.ts` and every range in the lab moves with it.
 *
 * WHAT THIS IS NOT: a YAML parser. It reads structure — which line a block starts on and which
 * line it ends on — and never a value, except the one key [`PRINCIPAL_DOMAIN`] needs. Anchors,
 * flow mappings, multi-line scalars and quoted keys are all things `invoice.yaml` does not
 * contain, and a parser that handled them would be a dependency this page does not need.
 *
 * The one convention it relies on is the one the specification format already fixes: a
 * declaration is a list item whose first key is `name:`, so `- name: billing.invoice.Money` is
 * addressable as `span('types', 'billing.invoice.Money')`.
 */

import {INVOICE_YAML_LINES} from './_source.ts';

/** An inclusive, 1-based range of lines in the file `_source.ts` holds. */
export type LineRange = [number, number];

/**
 * One block of the file: a mapping key, or a list item addressed by its `name:`.
 *
 * `from` is the line the block opens on and `to` the last line that carries content — trailing
 * blank lines and trailing comments belong to whatever comes next, which is how this
 * specification is written and why a block's end is trimmed rather than taken as "the line before
 * the next one".
 */
type Block = {
  /** The key, or the list item's `name:` value, or the item's scalar. */
  label: string;
  from: number;
  to: number;
  /** Columns the label starts at, used only while the file is being walked. */
  indent: number;
  children: Block[];
};

/** A mapping key: `types:`, `naming:`, `input:`. */
const KEY = /^(\s*)([A-Za-z_][\w.$-]*):(?:\s.*)?$/;

/** A list item: `- name: x`, `- billing.invoice.InvoiceCreated`, `- amount >= 0`. */
const ITEM = /^(\s*)-\s+(.*)$/;

/** The first key of a list item written on the item's own line. */
const ITEM_KEY = /^([A-Za-z_][\w.$-]*):(?:\s+(.*))?$/;

/**
 * Walks the file once and answers its block tree.
 *
 * Comments and blank lines take no part in the structure — a comment is not indented meaningfully
 * in this file, and treating one as a block would end the block above it at the wrong line.
 */
function walk(lines: string[]): Block {
  const root: Block = {label: '', from: 1, to: lines.length, indent: -1, children: []};
  const open: Block[] = [root];
  let last = 1;

  for (let at = 0; at < lines.length; at += 1) {
    const line = lines[at];
    const trimmed = line.trim();
    if (trimmed === '' || trimmed.startsWith('#')) {
      continue;
    }

    let indent: number;
    let label: string;
    const item = ITEM.exec(line);
    if (item) {
      indent = item[1].length;
      const inner = ITEM_KEY.exec(item[2]);
      label = inner && inner[1] === 'name' && inner[2] ? inner[2].trim() : item[2].trim();
    } else {
      const key = KEY.exec(line);
      if (!key) {
        continue;
      }
      indent = key[1].length;
      label = key[2];
    }

    while (open.length > 1 && open[open.length - 1].indent >= indent) {
      open.pop()!.to = last;
    }
    const block: Block = {label, from: at + 1, to: at + 1, indent, children: []};
    open[open.length - 1].children.push(block);
    open.push(block);
    last = at + 1;
  }

  while (open.length > 1) {
    open.pop()!.to = last;
  }
  return root;
}

const ROOT = walk(INVOICE_YAML_LINES);

/** Follows a path of labels from the file's root, or answers nothing. */
function at(path: string[]): Block | undefined {
  let here: Block | undefined = ROOT;
  for (const step of path) {
    here = here?.children.find((child) => child.label === step);
    if (!here) {
      return undefined;
    }
  }
  return here;
}

/**
 * The lines one declaration occupies, or nothing if this file does not declare it.
 *
 * Nothing is the honest answer and not a failure: the run crosses into `billing.email`, whose
 * declarations live in a file the lab does not show, and a step that highlights no line is how
 * the panel says so.
 */
export function span(...path: string[]): LineRange | undefined {
  const block = at(path);
  return block ? [block.from, block.to] : undefined;
}

/** The one line a declaration opens on — what a step points at when it names several at once. */
export function head(...path: string[]): LineRange | undefined {
  const block = at(path);
  return block ? [block.from, block.from] : undefined;
}

/** Every range in `ranges` that exists, in the order given. */
export function ranges(...maybe: (LineRange | undefined)[]): LineRange[] {
  return maybe.filter((range): range is LineRange => range !== undefined);
}

/**
 * The domain this file declares, read from its own first key.
 *
 * The lab orders the intermediate layer around it: a system has several domains and this panel
 * stands beside one file, so the domain that file declares is the one a reader is looking at.
 */
export const PRINCIPAL_DOMAIN: string = (() => {
  const line = INVOICE_YAML_LINES.find((text) => text.startsWith('domain:'));
  return line ? line.slice('domain:'.length).trim() : '';
})();
