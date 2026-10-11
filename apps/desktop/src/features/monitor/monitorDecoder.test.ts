// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { MonitorDecoder } from './monitorDecoder';
import { decodeTable } from './monitorArrow';
import { monitorBytes } from './monitorFixture';

const native = vi.hoisted(() => ({
  onmessage: undefined as ((event: { data: unknown }) => void) | undefined,
  onerror: undefined as (() => void) | undefined,
  onmessageerror: undefined as (() => void) | undefined,
  postMessage: vi.fn(),
  terminate: vi.fn(),
}));
vi.mock('./monitor.worker?worker&inline', () => ({
  default: function () {
    return native;
  },
}));
afterEach(() => {
  vi.clearAllMocks();
});

it('transfers one bounded page, preserves NULL validity, and ignores stale replies', async () => {
  const decoder = new MonitorDecoder();
  const bytes = monitorBytes('activity');
  const pending = decoder.decode(bytes);
  expect(native.postMessage).toHaveBeenCalledWith({ id: 1, bytes }, [bytes]);
  await expect(decoder.decode(bytes)).rejects.toThrow('busy');
  const table = decodeTable(bytes);
  native.onmessage?.({ data: { id: 99, table } });
  native.onmessage?.({ data: { id: 1, table } });
  await expect(pending).resolves.toEqual(table);
  decoder.stop();
  expect(native.terminate).toHaveBeenCalledOnce();
  await expect(decoder.decode(bytes)).rejects.toThrow('stopped');
});
it('rejects outstanding work on close and worker failures', async () => {
  for (const failure of ['close', 'crash', 'invalid']) {
    const decoder = new MonitorDecoder();
    const pending = decoder.decode(monitorBytes('activity'));
    if (failure === 'close') decoder.stop();
    if (failure === 'crash') native.onerror?.();
    if (failure === 'invalid') native.onmessageerror?.();
    await expect(pending).rejects.toThrow(/decoder/);
    await expect(decoder.decode(new ArrayBuffer(0))).rejects.toThrow(/decoder/);
  }
});
it('settles invalid streams and failed transfers without retaining a page', async () => {
  const decoder = new MonitorDecoder();
  const bytes = monitorBytes('activity');
  const pending = decoder.decode(bytes);
  native.onmessage?.({ data: { id: 1, error: 'Invalid monitor Arrow page' } });
  await expect(pending).rejects.toThrow('Invalid monitor Arrow page');
  native.postMessage.mockImplementationOnce(() => {
    throw new Error('test-only');
  });
  await expect(decoder.decode(bytes)).rejects.toThrow('transfer failed');
  decoder.stop();
});
