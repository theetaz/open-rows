// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { emitTo } from '@tauri-apps/api/event';
import { reportUiEvent } from '@openrows/ipc';
import { act, renderHook } from '@testing-library/react';
import { scheduleHostPrewarm } from './hostPrewarm';
import { useWindowReady } from './useWindowReady';

vi.mock('@tauri-apps/api/event', () => ({ emitTo: vi.fn() }));
vi.mock('@openrows/ipc', () => ({ reportUiEvent: vi.fn() }));

const frames: FrameRequestCallback[] = [];
let idle: IdleRequestCallback | undefined;
const cancelIdle = vi.fn();

beforeEach(() => {
  frames.length = 0;
  idle = undefined;
  cancelIdle.mockReset();
  vi.mocked(emitTo).mockReset().mockResolvedValue();
  vi.mocked(reportUiEvent).mockReset().mockResolvedValue();
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frames.push(callback);
    return frames.length;
  });
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
  vi.stubGlobal('requestIdleCallback', (callback: IdleRequestCallback) => {
    idle = callback;
    return 1;
  });
  vi.stubGlobal('cancelIdleCallback', cancelIdle);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function paint(): void {
  frames.shift()?.(1);
  frames.shift()?.(2);
}

function runIdle(): void {
  idle?.({ didTimeout: false, timeRemaining: () => 50 });
}

it('does no launch work before two visible frames and idle', async () => {
  scheduleHostPrewarm();
  expect(emitTo).not.toHaveBeenCalled();
  frames.shift()?.(1);
  expect(idle).toBeUndefined();
  frames.shift()?.(2);
  expect(emitTo).not.toHaveBeenCalled();
  runIdle();
  await Promise.resolve();
  expect(emitTo).toHaveBeenCalledExactlyOnceWith(
    'main',
    'openrows:driver-host-idle',
  );
});

it('cancels idle work when the ready view unmounts', () => {
  const cancel = scheduleHostPrewarm();
  paint();
  cancel();
  expect(cancelIdle).toHaveBeenCalledWith(1);
  expect(emitTo).not.toHaveBeenCalled();
});

it('uses a deferred event-loop turn after paint without requestIdleCallback', () => {
  vi.stubGlobal('requestIdleCallback', undefined);
  vi.useFakeTimers();
  const cancel = scheduleHostPrewarm();
  paint();
  expect(emitTo).not.toHaveBeenCalled();
  vi.runAllTimers();
  expect(emitTo).toHaveBeenCalledOnce();
  cancel();
});

it('records idle signal failure without rejecting or changing startup state', async () => {
  vi.mocked(emitTo).mockRejectedValue(new Error('test-only'));
  scheduleHostPrewarm();
  paint();
  runIdle();
  await Promise.resolve();
  expect(reportUiEvent).toHaveBeenCalledWith({
    kind: 'error',
    name: 'driverHostPrewarm',
    message: 'Driver host idle signal failed',
    durationMs: null,
  });
});

it('keeps the window-ready state successful when the later idle signal fails', async () => {
  vi.mocked(emitTo)
    .mockResolvedValueOnce()
    .mockRejectedValueOnce(new Error('test-only'));
  const hook = renderHook(useWindowReady);
  await act(async () => {
    await Promise.resolve();
  });
  paint();
  runIdle();
  await act(async () => {
    await Promise.resolve();
  });
  expect(hook.result.current).toBe(false);
  expect(reportUiEvent).toHaveBeenCalledOnce();
  hook.unmount();
});
