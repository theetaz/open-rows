// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ArrowWorker } from './workerClient';
import type { DecodedTile, WorkerReply } from './types';

vi.mock('./arrow.worker?worker&inline', () => ({
  default: function (): Worker {
    return new Worker('inline-test-worker');
  },
}));

class FakeWorker {
  static readonly instances: FakeWorker[] = [];
  onmessage: ((event: { data: WorkerReply }) => void) | undefined;
  onerror: ((event: { message: string }) => void) | undefined;
  onmessageerror: (() => void) | undefined;
  postMessage = vi.fn();
  terminate = vi.fn();
  constructor() {
    FakeWorker.instances.push(this);
  }
  reply(data: WorkerReply): void {
    this.onmessage?.({ data });
  }
}
function native(): FakeWorker {
  const latest = FakeWorker.instances.at(-1);
  if (!latest) throw new Error('Worker was not created');
  return latest;
}
beforeEach(() => {
  vi.stubGlobal('Worker', FakeWorker);
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  FakeWorker.instances.length = 0;
});

it('waits for lazy decoder readiness, transfers bytes and correlates replies', async () => {
  const worker = new ArrowWorker();
  const initialized = worker.initialize();
  native().reply({ kind: 'ready' });
  await initialized;
  const bytes = new ArrayBuffer(16);
  const pending = worker.decode(bytes);
  await Promise.resolve();
  expect(native().postMessage).toHaveBeenCalledWith({ id: 1, bytes }, [bytes]);
  const result: DecodedTile = { tile: { rows: 0, columns: [] }, decodeMs: 0.1 };
  native().reply({ kind: 'decoded', id: 999, result });
  native().reply({ kind: 'decoded', id: 1, result });
  await expect(pending).resolves.toMatchObject(result);
  worker.stop();
  expect(native().terminate).toHaveBeenCalledOnce();
});

it('rejects decode failures and worker crashes without leaving promises pending', async () => {
  const worker = new ArrowWorker();
  native().reply({ kind: 'ready' });
  const first = worker.decode(new ArrayBuffer(8));
  await Promise.resolve();
  native().reply({ kind: 'error', id: 1, message: 'bad stream' });
  await expect(first).rejects.toThrow('bad stream');
  const second = worker.decode(new ArrayBuffer(8));
  await Promise.resolve();
  native().onerror?.({ message: 'crashed' });
  await expect(second).rejects.toThrow('crashed');
  await expect(worker.decode(new ArrayBuffer(8))).rejects.toThrow('crashed');
});

it('rejects initialization on shutdown and invalid messages', async () => {
  const worker = new ArrowWorker();
  const pending = worker.initialize();
  worker.stop();
  await expect(pending).rejects.toThrow('stopped');
  const other = new ArrowWorker();
  const initialization = other.initialize();
  native().onmessageerror?.();
  await expect(initialization).rejects.toThrow('Invalid Arrow worker message');
});

it('settles a failed postMessage and allows the next decode to complete', async () => {
  const worker = new ArrowWorker();
  native().reply({ kind: 'ready' });
  native().postMessage.mockImplementationOnce(() => {
    throw new Error('transfer failed');
  });
  await expect(worker.decode(new ArrayBuffer(8))).rejects.toThrow(
    'transfer failed',
  );
  const next = worker.decode(new ArrayBuffer(8));
  await Promise.resolve();
  const result: DecodedTile = { tile: { rows: 0, columns: [] }, decodeMs: 0 };
  native().reply({ kind: 'decoded', id: 2, result });
  await expect(next).resolves.toMatchObject(result);
  worker.stop();
});

it('measures post-to-reply transfer on the page clock independently of worker decode', async () => {
  const worker = new ArrowWorker();
  native().reply({ kind: 'ready' });
  await worker.initialize();
  const clock = vi.spyOn(performance, 'now').mockReturnValue(100);
  native().postMessage.mockImplementation(() => {
    clock.mockReturnValue(100.75);
  });
  const pending = worker.decode(new ArrayBuffer(16));
  await Promise.resolve();
  native().reply({
    kind: 'decoded',
    id: 1,
    result: { tile: { rows: 0, columns: [] }, decodeMs: 0.125 },
  });
  await expect(pending).resolves.toMatchObject({
    transferMs: 0.75,
    decodeMs: 0.125,
  });
  worker.stop();
});

it('requests ready-to-paint display cells from the worker with the logical window coordinates', async () => {
  const worker = new ArrowWorker();
  native().reply({ kind: 'ready' });
  const bytes = new ArrayBuffer(16);
  const display = {
    resultId: 'spike',
    rowStart: 512,
    rowCount: 256,
    colStart: 40,
    colCount: 20,
  };
  const pending = worker.display(bytes, display);
  await Promise.resolve();
  expect(native().postMessage).toHaveBeenCalledWith({ id: 1, bytes, display }, [
    bytes,
  ]);
  const result = {
    cells: [{ text: '9007199254740993', state: 'normal' as const }],
    columns: 1,
    convertMs: 2,
  };
  native().reply({ kind: 'display', id: 1, result });
  await expect(pending).resolves.toBe(result);
  worker.stop();
});
