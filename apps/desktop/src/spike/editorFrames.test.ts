// SPDX-License-Identifier: Apache-2.0
import type { EditorView } from '@openrows/editor';
import { afterEach, expect, it, vi } from 'vitest';
import {
  browserFrames,
  delay,
  refreshInterval,
  viewInput,
} from './editorFrames';
import type { FrameProbe } from './editorSampler';

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it('takes the median of unsampled rAF intervals', async () => {
  const stamps = [0, 16, 33, 50, 90, 106];
  const probe: FrameProbe = {
    now: () => 0,
    nextPaint: () => {
      const frameAt = stamps.shift() ?? 0;
      return Promise.resolve({
        frameAt,
        callbackAt: frameAt,
        paintedAt: frameAt,
      });
    },
  };
  await expect(refreshInterval(probe, 5)).resolves.toBe(17);
});

it('resolves after the frame with ordered rAF, callback and paint times', async () => {
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    setTimeout(() => {
      callback(performance.now());
    }, 0);
    return 1;
  });
  const paint = await browserFrames.nextPaint();
  expect(paint.frameAt).toBeLessThanOrEqual(paint.callbackAt);
  expect(paint.callbackAt).toBeLessThanOrEqual(paint.paintedAt);
});

it('types through execCommand and presses keys on the content element', () => {
  const contentDOM = document.createElement('div');
  const keys: string[] = [];
  contentDOM.addEventListener('keydown', (event) => keys.push(event.key));
  const execute = vi.fn(() => true);
  Object.defineProperty(document, 'execCommand', {
    configurable: true,
    value: execute,
  });
  const input = viewInput({ contentDOM } as unknown as EditorView);
  expect(input.type('a')).toBe(true);
  expect(execute).toHaveBeenCalledWith('insertText', false, 'a');
  input.press('Escape');
  expect(keys).toEqual(['Escape']);
});

it('delays on the timer queue', async () => {
  vi.useFakeTimers();
  let done = false;
  void delay(50).then(() => (done = true));
  await vi.advanceTimersByTimeAsync(49);
  expect(done).toBe(false);
  await vi.advanceTimersByTimeAsync(1);
  expect(done).toBe(true);
});
