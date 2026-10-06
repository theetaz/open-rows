// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { afterEach, expect, it, vi } from 'vitest';
import type { WorkerReply } from './types';
import type { WindowRequest } from '@openrows/ipc';
interface WorkerSurface {
  onmessage?: (event: {
    data: { id: number; bytes: ArrayBuffer; display?: WindowRequest };
  }) => void;
  postMessage: ReturnType<typeof vi.fn>;
}
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});
it('formats golden Arrow data into exact worker-ready cells, then reports malformed streams', async () => {
  const worker: WorkerSurface = { postMessage: vi.fn() };
  vi.stubGlobal('self', worker);
  await import('./arrow.worker');
  expect(worker.postMessage).toHaveBeenCalledWith({ kind: 'ready' });
  const bytes = readFileSync(
    `${process.cwd()}/apps/desktop/src/results/fixtures/mixed.arrow`,
  );
  worker.onmessage?.({
    data: {
      id: 1,
      bytes: bytes.buffer.slice(
        bytes.byteOffset,
        bytes.byteOffset + bytes.byteLength,
      ),
      display: {
        resultId: 'golden',
        rowStart: 3,
        rowCount: 36,
        colStart: 0,
        colCount: 20,
      },
    },
  });
  const reply = worker.postMessage.mock.calls.at(-1)?.[0] as WorkerReply;
  if (reply.kind !== 'display')
    throw new Error('Expected worker display reply');
  expect(reply.result.cells[21]?.text).toBe('9007199255399135');
  expect(reply.result.cells[27]?.text).toBe('1234567890123522.6032');
  expect(reply.result.cells[81]?.text).toBe('NULL');
  expect(worker.postMessage.mock.calls.at(-1)).toHaveLength(1);
  worker.onmessage?.({ data: { id: 2, bytes: new ArrayBuffer(0) } });
  expect(worker.postMessage).toHaveBeenLastCalledWith({
    kind: 'error',
    id: 2,
    message: 'Error: Truncated Arrow IPC stream',
  });
});
