// SPDX-License-Identifier: Apache-2.0
import type { KeystrokeSample } from '@openrows/ipc';
import type { Keystroke } from './editorScript';

/** Input paths into the editor under test. */
export interface KeyInput {
  /** Insert text through the browser editing pipeline; false if refused. */
  type(text: string): boolean;
  /** Dispatch one keydown to the editor content. */
  press(key: string): void;
}

/** Page-clock boundaries of the first rendering opportunity after a call. */
export interface PaintTiming {
  /** The rAF frame timestamp (start of the rendering update). */
  frameAt: number;
  /** When the rAF callback itself ran. */
  callbackAt: number;
  /** A task posted from that rAF: after style, layout and paint, not GPU display. */
  paintedAt: number;
}

/** Page clock and frame boundaries; injected so tests need no real frames. */
export interface FrameProbe {
  now(): number;
  nextPaint(): Promise<PaintTiming>;
}

/** Page-clock time of the first document change after `arm()`. */
export class ChangeClock {
  private changedAt: number | null = null;
  constructor(private readonly now: () => number) {}
  arm(): void {
    this.changedAt = null;
  }
  record(): void {
    this.changedAt ??= this.now();
  }
  get first(): number | null {
    return this.changedAt;
  }
}

/** Everything one measurement touches. */
export interface Probe {
  input: KeyInput;
  frames: FrameProbe;
  changes: ChangeClock;
  docLength: () => number;
}

/** A sampled keystroke and when the script intended to send it. */
export interface Target {
  index: number;
  position: string;
  stroke: Exclude<Keystroke, { kind: 'mode' }>;
  scheduledAt: number;
}

/**
 * Keystroke-to-paint: scheduling delay + input handling up to CodeMirror's
 * document update + the rendering update of the first frame after it. Only the
 * idle wait from the update to that frame's start is excluded (`vsyncWaitMs`).
 */
export async function measureKeystroke(
  probe: Probe,
  target: Target,
): Promise<KeystrokeSample> {
  const { input, frames, changes } = probe;
  const { stroke } = target;
  const expected = probe.docLength() + (stroke.kind === 'insert' ? 1 : -1);
  changes.arm();
  const dispatchAt = frames.now();
  if (stroke.kind === 'insert') {
    if (!input.type(stroke.key))
      throw new Error('WebKit refused insertText for the editor');
  } else input.press(stroke.key);
  const paint = await frames.nextPaint();
  const updateAt = changes.first;
  if (updateAt === null)
    throw new Error(`Keystroke ${String(target.index)} missed its frame`);
  if (probe.docLength() !== expected)
    throw new Error(`Keystroke ${String(target.index)} changed the wrong text`);
  // An update inside the frame (after its start) has no idle wait to exclude.
  const frameStart = Math.max(
    updateAt,
    Math.min(paint.frameAt, paint.callbackAt),
  );
  const queueMs = Math.max(0, dispatchAt - target.scheduledAt);
  const inputMs = updateAt - dispatchAt;
  const frameMs = paint.paintedAt - frameStart;
  return {
    index: target.index,
    position: target.position,
    kind: stroke.kind,
    key: stroke.key,
    queueMs,
    inputMs,
    vsyncWaitMs: frameStart - updateAt,
    frameMs,
    wallMs: paint.paintedAt - dispatchAt,
    keystrokeToPaintMs: queueMs + inputMs + frameMs,
  };
}
