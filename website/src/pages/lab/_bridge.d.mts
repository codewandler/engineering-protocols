/**
 * Types for `_bridge.mjs`, which is a byte-identical copy of a generated file.
 *
 * WHY THERE IS A COPY. `_bridge.mjs` is `generated/web/billing/bridge.js`, copied whole, for the
 * reason `_source.ts` is a copy of `invoice.yaml`: nothing in this build reads outside `website/`.
 * The alternative — importing the module at a URL the bundler is told to ignore — would leave the
 * page's only untyped edge exactly where the boundary is.
 *
 * The copy is not left to be noticed if it drifts: `task lab` compares the two byte for byte
 * before it builds anything, and refuses if they differ. To refresh it:
 *
 *   cp generated/web/billing/bridge.js website/src/pages/lab/_bridge.mjs
 *
 * The declarations below are this file's own — the copy is untouched, so that `cmp` holds.
 */

/** What the module answers a request with: JSON, always, refusal included. */
export type Answer = Record<string, unknown>;

/** An instantiated module, and the one function a caller needs. */
export type Driver = {
  /** Sends one JSON request over linear memory and answers what came back. */
  request: (body: unknown) => Answer;
  /** Whether the module carried a realization — `false` means every command refuses. */
  realized: boolean;
  /** The module's raw exports, for a caller that wants the boundary rather than the driver. */
  exports: Record<string, unknown>;
};

/**
 * Instantiates a module and answers a driver over it.
 *
 * `source` is anything `WebAssembly.instantiate` accepts: the bytes of a `.wasm`, or a compiled
 * `Module`.
 */
export function open(source: BufferSource | WebAssembly.Module): Promise<Driver>;

/** The three exports every module behind this boundary has. */
export const EXPORTS: string[];

/** The optional export a host installs its realization through. */
export const REALIZE: string;
