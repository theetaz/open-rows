// SPDX-License-Identifier: Apache-2.0
import type { EditorView } from '@openrows/editor';
import type { FrameProbe, KeyInput, PaintTiming } from './editorSampler';

/** rAF, then a message task posted from it: runs after that frame's rendering update. */
export const browserFrames: FrameProbe = {
  now: () => performance.now(),
  nextPaint: () =>
    new Promise<PaintTiming>((resolve) => {
      requestAnimationFrame((frameAt) => {
        const callbackAt = performance.now();
        const channel = new MessageChannel();
        channel.port1.onmessage = () => {
          const paintedAt = performance.now();
          channel.port1.close();
          channel.port2.close();
          resolve({ frameAt, callbackAt, paintedAt });
        };
        channel.port2.postMessage(null);
      });
    }),
};

/** Typing goes through WebKit's native editing path, keys through keydown. */
export function viewInput(view: EditorView): KeyInput {
  return {
    type: (text) =>
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- the only scriptable path through WebKit's native text input (beforeinput + DOM mutation), as typed text takes.
      document.execCommand('insertText', false, text),
    press: (key) => {
      view.contentDOM.dispatchEvent(
        new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }),
      );
    },
  };
}

/** Resolve after `milliseconds` on the page's timer queue. */
export function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

/** Median rAF interval over `frames` unsampled frames (display refresh). */
export async function refreshInterval(
  probe: FrameProbe,
  frames: number,
): Promise<number> {
  const intervals: number[] = [];
  let previous = (await probe.nextPaint()).frameAt;
  for (let frame = 0; frame < frames; frame++) {
    const { frameAt } = await probe.nextPaint();
    intervals.push(frameAt - previous);
    previous = frameAt;
  }
  intervals.sort((a, b) => a - b);
  return intervals[Math.floor(intervals.length / 2)] ?? 0;
}
