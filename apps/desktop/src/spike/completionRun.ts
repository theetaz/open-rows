// SPDX-License-Identifier: Apache-2.0
import {
  defaultPreferences,
  mountSqlEditor,
  parseFully,
} from '@openrows/editor';
import type { EditorView } from '@openrows/editor';
import type { CompletionRun, CompletionSample } from '@openrows/ipc';
import { CATALOG_OBJECTS, syntheticCatalog } from './completionCatalog';
import { browserFrames, refreshInterval, viewInput } from './editorFrames';
import type { SpikeSession } from './lifecycle';

/** Statement prefixes before the typed trigger character, at the end of the document. */
const TRIGGERS = [
  { kind: 'keyword', prefix: '', key: 'S' },
  { kind: 'table', prefix: 'SELECT * FROM ', key: 'o' },
  { kind: 'schemaObject', prefix: 'SELECT * FROM sales', key: '.' },
  {
    kind: 'column',
    prefix: 'SELECT o.id FROM public.orders_00010 AS o WHERE o',
    key: '.',
  },
] as const;
/** Popups measured per trigger kind. */
const REPEATS = 25;
/** A popup that never opens fails the run instead of hanging it. */
const POPUP_DEADLINE_MS = 1_000;
const POPUP = '.cm-tooltip-autocomplete';

type Trigger = (typeof TRIGGERS)[number];

/**
 * Completion popup latency (NFR-PERF-07, M1-21): the typed trigger character
 * to the first rendering update that shows the popup, on the 5 000-line
 * document with a warm 50 000-object catalog and the default editor
 * preferences. Includes the Settings ▸ Editor completion delay
 * (`completionDelay`, CodeMirror's activate-on-typing delay), as a user sees it.
 */
export async function runCompletion(
  host: HTMLElement,
  session: SpikeSession,
  doc: string,
): Promise<CompletionRun> {
  const editor = mountSqlEditor(host, {
    doc: `${doc}\n`,
    label: 'SQL completion benchmark editor',
    catalog: syntheticCatalog(),
  });
  try {
    const { view } = editor;
    if (!parseFully(view, 5_000)) throw new Error('Parsing took over 5000 ms');
    const refreshIntervalMs = await refreshInterval(browserFrames, 30);
    const samples: CompletionSample[] = [];
    for (let repeat = 0; repeat < REPEATS; repeat++)
      for (const trigger of TRIGGERS) {
        session.signal.throwIfAborted();
        samples.push(await measurePopup(view, trigger, samples.length));
      }
    return {
      objects: CATALOG_OBJECTS,
      activateDelayMs: defaultPreferences.completionDelay,
      refreshIntervalMs,
      samples,
    };
  } finally {
    editor.destroy();
  }
}

/** Reset the last line to the prefix with no popup, then type the trigger. */
async function measurePopup(
  view: EditorView,
  trigger: Trigger,
  index: number,
): Promise<CompletionSample> {
  await prepare(view, trigger.prefix);
  const appeared = popupAppearance(view);
  const dispatchAt = browserFrames.now();
  if (!viewInput(view).type(trigger.key))
    throw new Error('WebKit refused insertText for the completion editor');
  const appearedAt = await appeared;
  const paint = await browserFrames.nextPaint();
  const options = view.dom.querySelectorAll(`${POPUP} li`).length;
  if (options === 0) throw new Error(`Empty ${trigger.kind} completion popup`);
  return {
    index,
    kind: trigger.kind,
    key: trigger.key,
    options,
    openMs: appearedAt - dispatchAt,
    paintMs: paint.paintedAt - appearedAt,
    popupMs: paint.paintedAt - dispatchAt,
  };
}

/** Unsampled: last line holds the prefix, caret at its end, completion closed. */
async function prepare(view: EditorView, prefix: string): Promise<void> {
  const last = view.state.doc.line(view.state.doc.lines);
  view.dispatch({
    changes: { from: last.from, to: last.to, insert: prefix },
    selection: { anchor: last.from + prefix.length },
    scrollIntoView: true,
  });
  view.focus();
  for (let frame = 0; frame < 3; frame++) {
    if (view.dom.querySelector(POPUP)) viewInput(view).press('Escape');
    await browserFrames.nextPaint();
  }
  if (view.dom.querySelector(POPUP) || !view.hasFocus)
    throw new Error('Completion popup did not close, or the editor lost focus');
}

/** Page-clock time when the popup element is inserted into the editor. */
function popupAppearance(view: EditorView): Promise<number> {
  return new Promise((resolve, reject) => {
    const observer = new MutationObserver(() => {
      if (!view.dom.querySelector(POPUP)) return;
      const at = browserFrames.now();
      observer.disconnect();
      clearTimeout(timer);
      resolve(at);
    });
    const timer = setTimeout(() => {
      observer.disconnect();
      reject(
        new Error(`No completion popup within ${String(POPUP_DEADLINE_MS)} ms`),
      );
    }, POPUP_DEADLINE_MS);
    observer.observe(view.dom, { childList: true, subtree: true });
  });
}
