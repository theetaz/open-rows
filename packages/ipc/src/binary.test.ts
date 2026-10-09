// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { resultWindow } from './binary';

const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));
const request = {
  resultId: 'spike',
  rowStart: 0,
  rowCount: 256,
  colStart: 0,
  colCount: 20,
};

it('preserves a raw ArrayBuffer without copying', async () => {
  const bytes = new ArrayBuffer(16);
  mockInvoke.mockResolvedValue(bytes);
  expect(await resultWindow(request)).toBe(bytes);
  expect(mockInvoke).toHaveBeenCalledWith('result_window', { request });
});

it('normalizes the macOS postMessage byte-array fallback', async () => {
  mockInvoke.mockResolvedValue([0, 128, 255]);
  expect(new Uint8Array(await resultWindow(request))).toEqual(
    new Uint8Array([0, 128, 255]),
  );
});

it.each([{}, 'bytes', [256], [-1], [1.5], [NaN], ['1'], [null]])(
  'rejects malformed bytes %j',
  async (bytes) => {
    mockInvoke.mockResolvedValue(bytes);
    await expect(resultWindow(request)).rejects.toThrow('Expected raw bytes');
  },
);
