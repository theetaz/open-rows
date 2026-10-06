// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { TileClient } from './tileClient';
import type { WindowRequest } from '@openrows/ipc';
import type { Tile } from './types';

const request: WindowRequest = {
  resultId: 'test',
  rowStart: 0,
  rowCount: 256,
  colStart: 0,
  colCount: 20,
};
const tile: Tile = { rows: 256, columns: [] };

it('deduplicates concurrent requests and caches completion', async () => {
  const load = vi.fn(() => Promise.resolve(tile));
  const client = new TileClient(load);
  const first = client.get(request);
  expect(client.get({ ...request })).toBe(first);
  expect(await first).toBe(tile);
  expect(await client.get(request)).toBe(tile);
  expect(load).toHaveBeenCalledTimes(1);
});

it('evicts the least recently used tile and includes all dimensions and result id in the key', async () => {
  const load = vi.fn(() => Promise.resolve(tile));
  const client = new TileClient(load, 2);
  await client.get(request);
  await client.get({ ...request, rowStart: 256 });
  await client.get(request);
  await client.get({ ...request, colStart: 1 });
  await client.get({ ...request, rowStart: 256 });
  expect(load).toHaveBeenCalledTimes(4);
  await client.get({ ...request, resultId: 'other' });
  await client.get({ ...request, rowCount: 1 });
  await client.get({ ...request, colCount: 1 });
  expect(load).toHaveBeenCalledTimes(7);
});

it('removes failed requests so callers can retry', async () => {
  const load = vi
    .fn()
    .mockRejectedValueOnce(new Error('unavailable'))
    .mockResolvedValue(tile);
  const client = new TileClient(load);
  await expect(client.get(request)).rejects.toThrow('unavailable');
  await expect(client.get(request)).resolves.toBe(tile);
  expect(load).toHaveBeenCalledTimes(2);
});

it('rejects invalid cache capacities', () => {
  for (const capacity of [0, -1, 0.5, NaN])
    expect(
      () => new TileClient(() => Promise.resolve(tile), capacity),
    ).toThrow();
});
