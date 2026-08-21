import {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';

import {INVOICE_YAML_LINES} from './_source';
import {EFFECT_GLYPH, IR_ROWS, STEPS} from './_run';
import type {Effect, IrRow, LineRange} from './_run';
import styles from './styles.module.css';

/**
 * The lab — a LAYOUT DRAFT.
 *
 * Three panels and a canned run: the specification on the left, what the compiler resolved it to in
 * the middle, and what a run of it did on the right. Stepping moves all three at once, which is the
 * whole point of the arrangement — a line of YAML, the IR node it became and the effect it caused
 * are the same fact seen three ways, and the layout's job is to let a reader hold them together.
 *
 * WHAT THIS IS NOT: there is no engine here. `_run.ts` is a hardcoded array of steps (the underscore keeps Docusaurus from making it a route). The names in
 * it are real and the mechanism is not, and the toolbar says so in as many words rather than
 * leaving a reader to find out. The footnote at the bottom says what the implemented version runs.
 *
 * THEME: dark only, deliberately. This follows the landing page's hero, which is also a literal
 * dark field under the light theme: the lab is an instrument panel rather than a page rendered in
 * the brand's colours, and a second tuned palette for the gutter, the highlight overlay and nine
 * ledger glyph colours would buy a reader nothing the dark one does not already give them. The
 * navbar above it still follows the site theme.
 */

const STEP_MS = 1200;
const LAST = STEPS.length - 1;

/* ------------------------------------------------------------------ *
 * The source panel
 * ------------------------------------------------------------------ */

type Tok = {c: string; s: string};

/**
 * Enough YAML to colour this file, and no more.
 *
 * A real editor is a real editor — the draft's brief bans a bundled one, so this is a `<pre>` of
 * rows with a gutter, and the colours are the site's Prism theme by hand: green for keys, blue for
 * values, grey italic for the comments this specification is mostly made of.
 */
function tokenize(line: string): Tok[] {
  if (line.trim() === '') {
    return [];
  }
  const comment = /^(\s*)(#.*)$/.exec(line);
  if (comment) {
    return [
      {c: '', s: comment[1]},
      {c: styles.tComment, s: comment[2]},
    ];
  }
  const pair = /^(\s*)((?:-\s)?)([A-Za-z_][\w.$-]*)(:)(\s*)(.*)$/.exec(line);
  if (pair) {
    const toks: Tok[] = [];
    if (pair[1]) toks.push({c: '', s: pair[1]});
    if (pair[2]) toks.push({c: styles.tPunct, s: pair[2]});
    toks.push({c: styles.tKey, s: pair[3]});
    toks.push({c: styles.tPunct, s: pair[4]});
    if (pair[5]) toks.push({c: '', s: pair[5]});
    pushValue(toks, pair[6]);
    return toks;
  }
  const item = /^(\s*)(-\s)(.*)$/.exec(line);
  if (item) {
    const toks: Tok[] = [{c: '', s: item[1]}, {c: styles.tPunct, s: item[2]}];
    pushValue(toks, item[3]);
    return toks;
  }
  return [{c: '', s: line}];
}

function pushValue(toks: Tok[], rest: string): void {
  if (!rest) {
    return;
  }
  const at = rest.indexOf('  #');
  if (at > 0) {
    toks.push({c: styles.tValue, s: rest.slice(0, at)});
    toks.push({c: styles.tComment, s: rest.slice(at)});
    return;
  }
  toks.push({c: styles.tValue, s: rest});
}

/** Is this 1-based line inside any of the step's ranges? */
function inRanges(line: number, ranges: LineRange[]): boolean {
  return ranges.some(([from, to]) => line >= from && line <= to);
}

function SourcePanel({ranges}: {ranges: LineRange[]}) {
  const body = useRef<HTMLDivElement>(null);
  const first = ranges.length > 0 ? Math.min(...ranges.map(([from]) => from)) : 0;

  useEffect(() => {
    const host = body.current;
    if (!host || first === 0) {
      return;
    }
    const row = host.querySelector<HTMLElement>(`[data-line="${first}"]`);
    if (row) {
      host.scrollTo({
        top: Math.max(0, row.offsetTop - host.clientHeight / 2.6),
        behavior: 'smooth',
      });
    }
  }, [first]);

  return (
    <Panel
      label="the source"
      title="examples/billing/domains/invoice.yaml"
      meta={`${INVOICE_YAML_LINES.length} lines`}
      bodyRef={body}
      bodyClassName={styles.sourceBody}>
      <pre className={styles.source}>
        {INVOICE_YAML_LINES.map((line, i) => {
          const n = i + 1;
          const on = inRanges(n, ranges);
          return (
            <div
              key={n}
              data-line={n}
              className={clsx(styles.row, on && styles.rowOn, on && n === first && styles.rowFirst)}>
              <span className={styles.gutter}>{n}</span>
              <code className={styles.code}>
                {tokenize(line).map((tok, j) => (
                  <span key={j} className={tok.c}>
                    {tok.s}
                  </span>
                ))}
                {line === '' ? ' ' : null}
              </code>
            </div>
          );
        })}
      </pre>
    </Panel>
  );
}

/* ------------------------------------------------------------------ *
 * The intermediate layer
 * ------------------------------------------------------------------ */

function IrPanel({active}: {active: string[]}) {
  const body = useRef<HTMLDivElement>(null);
  const first = active[0];

  useEffect(() => {
    const host = body.current;
    if (!host || !first) {
      return;
    }
    const row = host.querySelector<HTMLElement>(`[data-ir="${first}"]`);
    if (row) {
      host.scrollTo({
        top: Math.max(0, row.offsetTop - host.clientHeight / 2.6),
        behavior: 'smooth',
      });
    }
  }, [first]);

  return (
    <Panel
      label="the intermediate layer"
      title="protocol ess compile"
      bodyRef={body}
      bodyClassName={styles.irBody}>
      {IR_ROWS.map((row: IrRow) => (
        <div
          key={row.id}
          data-ir={row.id}
          className={clsx(styles.irRow, active.includes(row.id) && styles.irRowOn)}
          style={{paddingLeft: `${0.55 + row.depth * 0.85}rem`}}>
          <span className={clsx(styles.irKind, styles[`k_${row.kind}`])}>{row.kind}</span>
          <span className={styles.irLabel}>{row.label}</span>
          {row.detail ? <span className={styles.irDetail}>{row.detail}</span> : null}
        </div>
      ))}
    </Panel>
  );
}

/* ------------------------------------------------------------------ *
 * The run: state, then effects
 * ------------------------------------------------------------------ */

type Status = 'idle' | 'running' | 'halted' | 'done';

const STATUS_WORD: Record<Status, string> = {
  idle: 'idle',
  running: 'running',
  halted: 'halted',
  done: 'done',
};

type LedgerEntry = Effect & {key: string; step: number};

function RunPanel({
  status,
  index,
  entries,
}: {
  status: Status;
  index: number;
  entries: LedgerEntry[];
}) {
  const body = useRef<HTMLDivElement>(null);
  const step = index >= 0 ? STEPS[index] : undefined;
  const done = index + 1;

  useEffect(() => {
    const host = body.current;
    if (host) {
      host.scrollTo({top: host.scrollHeight, behavior: 'smooth'});
    }
  }, [entries.length]);

  return (
    <Panel
      label="the run"
      title="state and effects"
      meta={`step ${done} / ${STEPS.length}`}
      bodyRef={body}
      bodyClassName={styles.runBody}
      head={
        <div className={styles.state}>
          <div className={styles.stateTop}>
            <span className={clsx(styles.chip, styles[`s_${status}`])}>
              <span className={styles.chipDot} />
              {STATUS_WORD[status]}
            </span>
            <span className={styles.stateCount}>
              {done} / {STEPS.length}
            </span>
          </div>
          <div className={styles.stateName}>{step ? step.label : 'no run yet'}</div>
          <div className={styles.stateNote}>
            {step ? step.note : 'Press play, or step forward one at a time.'}
          </div>
          <div className={styles.progress}>
            <span
              className={styles.progressFill}
              style={{width: `${(done / STEPS.length) * 100}%`}}
            />
          </div>
        </div>
      }>
      {entries.length === 0 ? (
        <p className={styles.ledgerEmpty}>The ledger is empty. Nothing has been asked of the system yet.</p>
      ) : (
        <ol className={styles.ledger}>
          {entries.map((entry) => (
            <li
              key={entry.key}
              className={clsx(styles.entry, styles[`e_${entry.kind}`], entry.step === index && styles.entryNew)}>
              <span className={styles.entryGlyph}>{EFFECT_GLYPH[entry.kind]}</span>
              <span className={styles.entryText}>
                {entry.text}
                {entry.detail ? <span className={styles.entryDetail}>{entry.detail}</span> : null}
              </span>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}

/* ------------------------------------------------------------------ *
 * The panel shell, shared by all three
 * ------------------------------------------------------------------ */

function Panel({
  label,
  title,
  meta,
  head,
  children,
  bodyRef,
  bodyClassName,
}: {
  label: string;
  title: string;
  meta?: string;
  head?: ReactNode;
  children: ReactNode;
  bodyRef?: React.RefObject<HTMLDivElement | null>;
  bodyClassName?: string;
}) {
  return (
    <section className={styles.panel}>
      <header className={styles.panelHead}>
        <span className={styles.panelLabel}>{label}</span>
        <span className={styles.panelTitle}>{title}</span>
        {meta ? <span className={styles.panelMeta}>{meta}</span> : null}
      </header>
      {head}
      <div ref={bodyRef} className={clsx(styles.panelBody, bodyClassName)}>
        {children}
      </div>
    </section>
  );
}

/* ------------------------------------------------------------------ *
 * The lab
 * ------------------------------------------------------------------ */

export default function Lab(): ReactNode {
  const [index, setIndex] = useState(-1);
  const [playing, setPlaying] = useState(false);
  const [full, setFull] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  const status: Status = playing ? 'running' : index < 0 ? 'idle' : index >= LAST ? 'done' : 'halted';

  const step = index >= 0 ? STEPS[index] : undefined;
  const ranges = step ? step.source : [];
  const irActive = step ? step.ir : [];

  const entries = useMemo<LedgerEntry[]>(() => {
    const out: LedgerEntry[] = [];
    for (let s = 0; s <= index; s += 1) {
      STEPS[s].effects.forEach((effect, i) => {
        out.push({...effect, key: `${STEPS[s].id}-${i}`, step: s});
      });
    }
    return out;
  }, [index]);

  const forward = useCallback(() => {
    setIndex((i) => Math.min(i + 1, LAST));
  }, []);
  const back = useCallback(() => {
    setPlaying(false);
    setIndex((i) => Math.max(i - 1, -1));
  }, []);
  const reset = useCallback(() => {
    setPlaying(false);
    setIndex(-1);
  }, []);
  const toggle = useCallback(() => {
    setPlaying((p) => {
      if (p) {
        return false;
      }
      setIndex((i) => (i >= LAST ? -1 : i));
      return true;
    });
  }, []);

  // Auto-advance. One timer per step, cleared on every state change, so a click during a run
  // takes effect on the click rather than on the next tick.
  useEffect(() => {
    if (!playing) {
      return undefined;
    }
    if (index >= LAST) {
      setPlaying(false);
      return undefined;
    }
    const timer = window.setTimeout(forward, STEP_MS);
    return () => window.clearTimeout(timer);
  }, [playing, index, forward]);

  // Space plays and pauses, the arrows step. Ignored while a form control has focus, so the
  // shortcut never eats a keystroke someone meant for something else.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) {
        return;
      }
      if (event.key === ' ' || event.key === 'Spacebar') {
        event.preventDefault();
        toggle();
      } else if (event.key === 'ArrowRight') {
        event.preventDefault();
        setPlaying(false);
        forward();
      } else if (event.key === 'ArrowLeft') {
        event.preventDefault();
        back();
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [toggle, forward, back]);

  // The Fullscreen API, on the lab itself rather than the document: fullscreening the lab takes
  // the navbar with it, which is the only thing between the panels and the viewport.
  const toggleFullscreen = useCallback(() => {
    const node = root.current;
    if (!node) {
      return;
    }
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void node.requestFullscreen?.();
    }
  }, []);

  useEffect(() => {
    function onChange() {
      setFull(document.fullscreenElement === root.current);
    }
    document.addEventListener('fullscreenchange', onChange);
    return () => document.removeEventListener('fullscreenchange', onChange);
  }, []);

  // No page scroll: the lab is the viewport, and every panel scrolls inside itself. Wide viewports
  // only — below 997px the lab is a stack of panels and the page has to scroll like any other. The
  // previous value is restored on the way out, because this is a route and not the whole site.
  useEffect(() => {
    const wide = window.matchMedia('(min-width: 997px)');
    const previous = document.body.style.overflow;
    const apply = () => {
      document.body.style.overflow = wide.matches ? 'hidden' : previous;
    };
    apply();
    wide.addEventListener('change', apply);
    return () => {
      wide.removeEventListener('change', apply);
      document.body.style.overflow = previous;
    };
  }, []);

  return (
    <Layout
      noFooter
      title="The lab"
      description="A specification, the IR it compiles to, and a run of it — the three side by side, stepped together. A layout draft over a canned run.">
      <div ref={root} className={styles.lab}>
        <div className={styles.toolbar}>
          <div className={styles.transport}>
            <button
              type="button"
              className={clsx(styles.tbutton, styles.tprimary)}
              onClick={toggle}
              aria-label={playing ? 'Halt the run' : 'Play the run'}
              title={playing ? 'Halt (space)' : 'Play (space)'}>
              {playing ? '\u25AE\u25AE' : '\u25B8'}
              <span className={styles.tlabel}>{playing ? 'halt' : 'play'}</span>
            </button>
            <button
              type="button"
              className={styles.tbutton}
              onClick={back}
              disabled={index < 0}
              aria-label="Step back"
              title="Step back (←)">
              {'\u25C2\u25C2'}
            </button>
            <button
              type="button"
              className={styles.tbutton}
              onClick={() => {
                setPlaying(false);
                forward();
              }}
              disabled={index >= LAST}
              aria-label="Step forward"
              title="Step forward (→)">
              {'\u25B8\u25B8'}
            </button>
            <button
              type="button"
              className={styles.tbutton}
              onClick={reset}
              disabled={index < 0 && !playing}
              aria-label="Reset the run"
              title="Reset">
              {'\u21BA'}
            </button>
          </div>

          <div className={styles.toolbarMid}>
            <span className={styles.runTitle}>billing v3 · CreateInvoice</span>
            <span className={styles.draftTag} title="No engine runs here yet. The steps are hardcoded.">
              draft — canned run
            </span>
          </div>

          <div className={styles.toolbarEnd}>
            <span className={styles.keys}>
              <kbd>space</kbd> play · <kbd>←</kbd> <kbd>→</kbd> step
            </span>
            <Link className={styles.exit} to="/docs/examples/specification-to-contracts">
              the source page
            </Link>
            <button
              type="button"
              className={styles.tbutton}
              onClick={toggleFullscreen}
              aria-label={full ? 'Leave fullscreen' : 'Enter fullscreen'}
              title={full ? 'Leave fullscreen' : 'Fullscreen'}>
              {'\u26F6'}
            </button>
          </div>
        </div>

        <div className={styles.panels}>
          <SourcePanel ranges={ranges} />
          <IrPanel active={irActive} />
          <RunPanel status={status} index={index} entries={entries} />
        </div>

        <footer className={styles.footnote}>
          <span className={styles.footnoteLabel}>what the real version will do</span>
          <p>
            The material here is real — <code>invoice.yaml</code> verbatim, the middle panel{' '}
            <code>protocol ess compile</code> trimmed, the outcomes and errors the ones the
            specification declares — and the run through it is a hardcoded array of eleven steps.
            Nothing is executed. It need not stay that way: the repository already synthesizes a
            browser realization of this same specification to WebAssembly —{' '}
            <code>generated/web/billing/</code>, a bridge taking JSON in over linear memory and
            handing JSON back, with <code>examples/billing-web/</code> the hand-written host that
            links a realization into it. The implemented lab loads that module and steps a real run
            through these same three panels.
          </p>
        </footer>
      </div>
    </Layout>
  );
}
