// SPDX-License-Identifier: Apache-2.0
import { mountSqlEditor, parseFully } from '@openrows/editor';
import type { EditorView } from '@openrows/editor';
import type { EditorRun, KeystrokeSample } from '@openrows/ipc';
import {
  browserFrames,
  delay,
  refreshInterval,
  viewInput,
} from './editorFrames';
import { ChangeClock, measureKeystroke } from './editorSampler';
import type { Probe } from './editorSampler';
import { KEY_INTERVAL_MS, POSITIONS, keystrokes } from './editorScript';
import type { EditorConfig, Keystroke } from './editorScript';
import type { SpikeSession } from './lifecycle';

/** The lazily loaded `@openrows/editor/vim` entry. */
export type VimModule = typeof import('@openrows/editor/vim');

/** One configuration on a fresh editor; Vim is present only for `vim`. */
export interface ConfigContext {
  host: HTMLElement;
  session: SpikeSession;
  config: EditorConfig;
  doc: string;
  vim: VimModule | null;
}

const PARSE_BUDGET_MS = 5_000;
/** ≈ 5 s at 60 Hz. */
const FOCUS_FRAMES = 300;

/** Mount, parse fully, then type at every position; mounting and parsing are unsampled. */
export async function runConfig(context: ConfigContext): Promise<EditorRun> {
  const { host, config, doc, vim } = context;
  const changes = new ChangeClock(() => browserFrames.now());
  const editor = mountSqlEditor(host, {
    doc,
    label: 'SQL benchmark editor',
    onDocChange: () => {
      changes.record();
    },
    ...(vim ? { keyMode: vim.vimMode() } : {}),
  });
  try {
    const { view } = editor;
    const lines = view.state.doc.lines;
    const probe: Probe = {
      input: viewInput(view),
      frames: browserFrames,
      changes,
      docLength: () => view.state.doc.length,
    };
    const refreshIntervalMs = await refreshInterval(browserFrames, 30);
    const started = browserFrames.now();
    const samples: KeystrokeSample[] = [];
    for (const position of POSITIONS) {
      context.session.signal.throwIfAborted();
      context.session.progress('editor-position', `${config}/${position.name}`);
      await place(view, position.line);
      for (const stroke of keystrokes(config))
        await runStroke(
          { ...context, view, probe, samples, stroke },
          position.name,
        );
    }
    const durationMs = browserFrames.now() - started;
    return {
      config,
      lines,
      documentChars: doc.length,
      refreshIntervalMs,
      durationMs,
      samples,
    };
  } finally {
    editor.destroy();
  }
}

interface StrokeContext extends ConfigContext {
  view: EditorView;
  probe: Probe;
  samples: KeystrokeSample[];
  stroke: Keystroke;
}

async function runStroke(
  context: StrokeContext,
  position: string,
): Promise<void> {
  const { stroke, samples } = context;
  if (stroke.kind === 'mode') {
    await switchMode(context, stroke);
    return;
  }
  const scheduledAt = browserFrames.now() + KEY_INTERVAL_MS;
  await delay(KEY_INTERVAL_MS);
  samples.push(
    await measureKeystroke(context.probe, {
      index: samples.length,
      position,
      stroke,
      scheduledAt,
    }),
  );
}

/** Caret at the end of `line`, scrolled into view and fully parsed; unsampled. */
async function place(view: EditorView, line: number): Promise<void> {
  view.dispatch({
    selection: { anchor: view.state.doc.line(line).to },
    scrollIntoView: true,
  });
  if (!parseFully(view, PARSE_BUDGET_MS))
    throw new Error(`Parsing took over ${String(PARSE_BUDGET_MS)} ms`);
  await awaitFocus(view);
  for (let frame = 0; frame < 3; frame++) await browserFrames.nextPaint();
}

/**
 * WebKit refuses `insertText` until the native window is key, which can lag the
 * show/focus handshake; retry focus for up to {@link FOCUS_FRAMES} frames.
 */
async function awaitFocus(view: EditorView): Promise<void> {
  for (let frame = 0; frame < FOCUS_FRAMES; frame++) {
    view.focus();
    if (document.hasFocus() && view.hasFocus) return;
    await browserFrames.nextPaint();
  }
  throw new Error(
    'Editor never gained keyboard focus; keep its window frontmost',
  );
}

/** Unsampled mode switch; a second Escape may be needed to close completion first. */
async function switchMode(
  context: StrokeContext,
  stroke: Extract<Keystroke, { kind: 'mode' }>,
): Promise<void> {
  const { vim, view, probe } = context;
  if (!vim) throw new Error('Vim keystroke without Vim mode');
  for (let attempt = 0; attempt < 3; attempt++) {
    if (vim.vimModeOf(view) === stroke.mode) break;
    probe.input.press(stroke.key);
  }
  if (vim.vimModeOf(view) !== stroke.mode)
    throw new Error(`Vim did not enter ${stroke.mode} mode`);
  for (let frame = 0; frame < 2; frame++) await browserFrames.nextPaint();
}
