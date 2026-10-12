// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { startLayout } from './layoutWorker';
import { shop } from './testDiagram';

const terminate = vi.fn();
let instance: {
  onmessage?: (event: { data: unknown }) => void;
  onerror?: (event: { message: string }) => void;
  postMessage: ReturnType<typeof vi.fn>;
};
function mockWorker(postMessage = vi.fn()): void {
  instance = { postMessage };
  vi.stubGlobal(
    'Worker',
    vi.fn(function WorkerStub() {
      return Object.assign(instance, { terminate });
    }),
  );
}
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

it('posts metadata to a worker and terminates after its layout reply', async () => {
  mockWorker();
  const job = startLayout(shop);
  expect(instance.postMessage).toHaveBeenCalledWith(shop);
  const layout = { nodes: [], edges: [], width: 10, height: 10 };
  instance.onmessage?.({ data: { layout } });
  expect(await job.result).toEqual(layout);
  expect(terminate).toHaveBeenCalledOnce();
});
it('terminates and rejects a cancelled layout without accepting stale work', async () => {
  mockWorker();
  const job = startLayout(shop);
  job.cancel();
  await expect(job.result).rejects.toThrow('cancelled');
  expect(terminate).toHaveBeenCalledOnce();
});
it('reports worker failures and releases the worker', async () => {
  mockWorker();
  const job = startLayout(shop);
  instance.onerror?.({ message: 'layout failed' });
  await expect(job.result).rejects.toThrow('layout failed');
  expect(terminate).toHaveBeenCalledOnce();
});

it('releases the worker when posting metadata fails', async () => {
  mockWorker(
    vi.fn(() => {
      throw new Error('Cannot clone metadata');
    }),
  );
  const job = startLayout(shop);
  await expect(job.result).rejects.toThrow('Cannot clone metadata');
  expect(terminate).toHaveBeenCalledOnce();
});
