// SPDX-License-Identifier: Apache-2.0
import { resultWindow } from '@openrows/ipc';
import type { TileTiming, WindowRequest } from '@openrows/ipc';
import { ArrowWorker } from '../results/workerClient';
import { TileClient } from '../results/tileClient';
import { SpikeSession, withTimeout } from './lifecycle';

/** Opt-in release spike: 500 unique random offsets, no cache hits or warmup samples. */
export async function runIpcSpike(session = new SpikeSession()): Promise<void> {
  if (session.signal.aborted) return session.finish();
  let worker: ArrowWorker | undefined;
  let error: string | null = null;
  const stop = (): void => worker?.stop();
  try {
    session.progress('timer-resolution', JSON.stringify(session.timer));
    if (session.timer.resolutionMs === null)
      throw new Error('Page timer has no observable nonzero delta');
    worker = new ArrowWorker();
    session.signal.addEventListener('abort', stop, { once: true });
    session.progress('worker-created');
    await withTimeout(worker.initialize(), 10_000, 'Worker initialization');
    session.progress('worker-initialized');
    await sampleTiles(worker, session);
  } catch (caught) {
    error = String(caught);
  } finally {
    session.signal.removeEventListener('abort', stop);
    stop();
  }
  await session.finish(error);
}

async function sampleTiles(
  worker: ArrowWorker,
  session: SpikeSession,
): Promise<void> {
  let timing: Omit<TileTiming, 'rowStart' | 'totalMs'> | undefined;
  const client = new TileClient(async (request) => {
    const result = await loadTile({ worker, session, request });
    timing = result.timing;
    return result.tile;
  });
  session.progress('first-tile-requested');
  for (const rowStart of offsets()) {
    if (session.signal.aborted) return;
    const start = performance.now();
    const tile = await withTimeout(
      client.get(tileRequest(rowStart)),
      2_000,
      `Tile at row ${String(rowStart)}`,
    );
    session.signal.throwIfAborted();
    if (tile.rows !== 256 || tile.columns.length !== 20 || !timing)
      throw new Error('Invalid spike tile');
    session.samples.push({
      rowStart,
      ...timing,
      totalMs: performance.now() - start,
    });
    if (session.samples.length % 100 === 0)
      session.progress('samples', String(session.samples.length));
  }
}

async function loadTile({
  worker,
  session,
  request,
}: {
  worker: ArrowWorker;
  session: SpikeSession;
  request: WindowRequest;
}): Promise<{
  tile: Awaited<ReturnType<ArrowWorker['decode']>>['tile'];
  timing: Omit<TileTiming, 'rowStart' | 'totalMs'>;
}> {
  const start = performance.now();
  const bytes = await resultWindow(request);
  const received = performance.now();
  session.signal.throwIfAborted();
  if (!session.samples.length)
    session.progress(
      'first-bytes-received',
      `${String(bytes.byteLength)} bytes`,
    );
  const decoded = await worker.decode(bytes);
  const timing = {
    invokeMs: received - start,
    decodeMs: decoded.transferMs,
    workerDecodeMs: decoded.decodeMs,
  };
  if (!session.samples.length) session.progress('first-decode-done');
  return { tile: decoded.tile, timing };
}

function offsets(): number[] {
  let seed = 42;
  const unique = new Set<number>();
  while (unique.size < 500) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    unique.add(seed % (1_000_000 - 256 + 1));
  }
  return [...unique];
}

function tileRequest(rowStart: number): WindowRequest {
  return {
    resultId: 'spike',
    rowStart,
    rowCount: 256,
    colStart: 0,
    colCount: 20,
  };
}
