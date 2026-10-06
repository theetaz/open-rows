// SPDX-License-Identifier: Apache-2.0
import type { WindowRequest } from '@openrows/ipc';
import type { Tile } from './types';

/** Deduplicate concurrent requests and retain at most capacity tiles in LRU order. */
export class TileClient {
  private readonly cache = new Map<string, Tile>();
  private readonly pending = new Map<string, Promise<Tile>>();

  constructor(
    private readonly load: (request: WindowRequest) => Promise<Tile>,
    private readonly capacity = 64,
  ) {
    if (!Number.isInteger(capacity) || capacity < 1)
      throw new Error('Invalid tile cache capacity');
  }

  /** Fetch or reuse exactly the requested result and range. */
  get(request: WindowRequest): Promise<Tile> {
    const key = JSON.stringify([
      request.resultId,
      request.rowStart,
      request.rowCount,
      request.colStart,
      request.colCount,
    ]);
    const cached = this.cache.get(key);
    if (cached) {
      this.cache.delete(key);
      this.cache.set(key, cached);
      return Promise.resolve(cached);
    }
    const pending = this.pending.get(key);
    if (pending) return pending;
    const promise = Promise.resolve()
      .then(() => this.load(request))
      .then((tile) => {
        this.cache.set(key, tile);
        if (this.cache.size > this.capacity) {
          const oldest = this.cache.keys().next().value;
          if (oldest !== undefined) this.cache.delete(oldest);
        }
        return tile;
      })
      .finally(() => this.pending.delete(key));
    this.pending.set(key, promise);
    return promise;
  }
}
