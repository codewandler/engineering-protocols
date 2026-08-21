import {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';

import {INVOICE_YAML_LINES} from './_source.ts';
import {EFFECT_GLYPH, loadRun} from './_run.ts';
import type {Effect, IrRow, LineRange, Run, Step} from './_run.ts';
import styles from './styles.module.css';

/**
 * The lab.
 *
 * Three panels over one system: the specification on the left, what the compiler resolved it to in
 * the middle, and what a run of it did on the right. Stepping moves all three at once, which is the
 * whole point of the arrangement — a line of YAML, the IR node it became and the effect it caused
 * are the same fact seen three ways, and the layout's job is to let a reader hold them together.
 *
 * WHAT RUNS. `billing_web_realized.wasm`, fetched beside this page: the browser realization the
 * repository synthesises from `examples/billing/` (`generated/web/billing/`), linked with the
 * hand-written behaviour under `examples/billing-realization/`. Five commands go in over the
 * module's own boundary and the outcomes, the published events, the binding invocations and the
 * view rows come back out; `_run.ts` turns that answer into the steps below and `_spans.ts` finds
 * the lines each step stands on in the file itself. Nothing on this page is a transcript of a run
 * that happened once on somebody's machine.
 *
 * The module is a build artifact and is not committed, so this page can be opened without one. It
 * says so when that happens, and names the command that builds it, rather than showing a run it
 * did not do.
 *
 * THEME: dark only, deliberately. This follows the landing page's hero, which is also a literal
 * dark field under the light theme: the lab is an instrument panel rather than a page rendered in
 * the brand's colours, and a second tuned palette for the gutter, the highlight overlay and nine
 * ledger glyph colours would buy a reader nothing the dark one does not already give them. The
 * navbar above it still follows the site theme.
 */

/** How long one step is held while the run plays itself. */
const STEP_MS = 1000;

/** The stream before the module has answered — a stable identity, so the memos below do not churn. */
const NO_STEPS: Step[] = [];

/** The same, for the middle panel. */
const NO_ROWS: IrRow[] = [];

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

function IrPanel({rows, active}: {rows: IrRow[]; active: string[]}) {
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
      title="request: catalog"
      meta={rows.length > 0 ? `${rows.length} rows` : undefined}
      bodyRef={body}
      bodyClassName={styles.irBody}>
      {rows.map((row: IrRow) => (
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

type Status = 'loading' | 'failed' | 'idle' | 'running' | 'halted' | 'done';

const STATUS_WORD: Record<Status, string> = {
  loading: 'loading',
  failed: 'no module',
  idle: 'idle',
  running: 'running',
  halted: 'halted',
  done: 'done',
};

type LedgerEntry = Effect & {key: string; step: number};

function RunPanel({
  status,
  index,
  steps,
  entries,
  failure,
}: {
  status: Status;
  index: number;
  steps: Step[];
  entries: LedgerEntry[];
  failure: string | null;
}) {
  const body = useRef<HTMLDivElement>(null);
  const step = index >= 0 ? steps[index] : undefined;
  const done = index + 1;
  const total = Math.max(steps.length, 1);

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
      meta={`step ${done} / ${steps.length}`}
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
              {done} / {steps.length}
            </span>
          </div>
          <div className={styles.stateName}>
            {step ? step.label : status === 'loading' ? 'instantiating the module' : 'no run yet'}
          </div>
          <div className={styles.stateNote}>
            {step
              ? step.note
              : status === 'failed'
                ? (failure ?? 'the module did not load')
                : status === 'loading'
                  ? 'Fetching billing_web_realized.wasm and asking it for its catalogue.'
                  : 'Press play, or step forward one at a time.'}
          </div>
          <div className={styles.progress}>
            <span className={styles.progressFill} style={{width: `${(done / total) * 100}%`}} />
          </div>
        </div>
      }>
      {entries.length === 0 ? (
        <p className={styles.ledgerEmpty}>
          {status === 'failed'
            ? 'Nothing ran: this page fetches a WebAssembly module that is not there. It is a build artifact and is not committed — `task lab` builds it and puts it beside the page.'
            : status === 'loading'
              ? 'Loading the module the run happens in.'
              : 'The ledger is empty. Nothing has been asked of the system yet.'}
        </p>
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
  const [run, setRun] = useState<Run | null>(null);
  const [bytes, setBytes] = useState(0);
  const [failure, setFailure] = useState<string | null>(null);
  const [index, setIndex] = useState(-1);
  const [playing, setPlaying] = useState(false);
  const [full, setFull] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const module = useBaseUrl('/lab/billing_web_realized.wasm');

  // The run, once — instantiated in the browser, because a module cannot be instantiated during
  // the static render and a run rendered into the HTML would be a transcript again.
  useEffect(() => {
    let live = true;
    loadRun(module).then(
      (loaded) => {
        if (live) {
          setRun(loaded.run);
          setBytes(loaded.bytes);
        }
      },
      (error: unknown) => {
        if (live) {
          setFailure(error instanceof Error ? error.message : String(error));
        }
      },
    );
    return () => {
      live = false;
    };
  }, [module]);

  const steps = run ? run.steps : NO_STEPS;
  const rows = run ? run.ir : NO_ROWS;
  const last = steps.length - 1;

  const status: Status = failure
    ? 'failed'
    : !run
      ? 'loading'
      : playing
        ? 'running'
        : index < 0
          ? 'idle'
          : index >= last
            ? 'done'
            : 'halted';

  const step = index >= 0 ? steps[index] : undefined;
  const ranges = step ? step.source : [];
  const irActive = step ? step.ir : [];

  const entries = useMemo<LedgerEntry[]>(() => {
    const out: LedgerEntry[] = [];
    for (let s = 0; s <= index && s < steps.length; s += 1) {
      steps[s].effects.forEach((effect, i) => {
        out.push({...effect, key: `${steps[s].id}-${i}`, step: s});
      });
    }
    return out;
  }, [index, steps]);

  const forward = useCallback(() => {
    setIndex((i) => Math.min(i + 1, last));
  }, [last]);
  const back = useCallback(() => {
    setPlaying(false);
    setIndex((i) => Math.max(i - 1, -1));
  }, []);
  const reset = useCallback(() => {
    setPlaying(false);
    setIndex(-1);
  }, []);
  const toggle = useCallback(() => {
    if (steps.length === 0) {
      return;
    }
    setPlaying((p) => {
      if (p) {
        return false;
      }
      setIndex((i) => (i >= last ? -1 : i));
      return true;
    });
  }, [last, steps.length]);

  // Auto-advance. One timer per step, cleared on every state change, so a click during a run
  // takes effect on the click rather than on the next tick.
  useEffect(() => {
    if (!playing) {
      return undefined;
    }
    if (index >= last) {
      setPlaying(false);
      return undefined;
    }
    const timer = window.setTimeout(forward, STEP_MS);
    return () => window.clearTimeout(timer);
  }, [playing, index, last, forward]);

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
      description="A specification, the IR it compiles to, and a real run of it in WebAssembly — the three side by side, stepped together.">
      <div ref={root} className={styles.lab}>
        <div className={styles.toolbar}>
          <div className={styles.transport}>
            <button
              type="button"
              className={clsx(styles.tbutton, styles.tprimary)}
              onClick={toggle}
              disabled={!run}
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
              disabled={!run || index >= last}
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
            <span className={styles.runTitle}>{run ? run.title : 'billing · loading'}</span>
            {run ? (
              <span
                className={styles.liveTag}
                title="The steps come from a WebAssembly module answering over its own boundary.">
                wasm · {Math.round(bytes / 1024)} KiB
              </span>
            ) : (
              <span className={styles.absentTag} title={failure ?? 'Fetching the module.'}>
                {failure ? 'no module' : 'loading'}
              </span>
            )}
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
          <IrPanel rows={rows} active={irActive} />
          <RunPanel status={status} index={index} steps={steps} entries={entries} failure={failure} />
        </div>

        <footer className={styles.footnote}>
          <span className={styles.footnoteLabel}>what is actually running</span>
          <p>
            The left panel is <code>invoice.yaml</code> verbatim. The middle panel is what{' '}
            <code>{'{"request":"catalog"}'}</code> answers — the compiler&rsquo;s own model, read out
            of the module rather than drawn beside it. The right panel is five commands sent over
            that module&rsquo;s boundary, and the outcomes, events, binding invocations and view rows
            it sent back. The module is{' '}
            <code>generated/web/billing/</code> — synthesised from this specification — linked with
            the hand-written realization in <code>examples/billing-realization/</code>, built for{' '}
            <code>wasm32-unknown-unknown</code>. Identifiers come from a counter rather than from
            randomness, so the same module answers the same run every time you load this page.
          </p>
        </footer>
      </div>
    </Layout>
  );
}
