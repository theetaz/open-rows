// SPDX-License-Identifier: Apache-2.0
import type { WindowRequest } from '@openrows/ipc';
import type { Tile } from './tiles';

/** Deduplicate concurrent requests and retain at most capacity tiles in LRU order. */
export class TileClient<T = Tile> {
  private readonly cache = new Map<string | number, T>();
  private readonly pending = new Map<string | number, Promise<T>>();

  constructor(
    private readonly load: (request: WindowRequest) => Promise<T>,
    private readonly capacity = 64,
    private readonly keyOf: (
      request: WindowRequest,
    ) => string | number = requestKey,
    private readonly onFill: (applyMs: number) => void = () => {},
  ) {
    if (!Number.isInteger(capacity) || capacity < 1)
      throw new Error('Invalid tile cache capacity');
  }

  /** Synchronous lookup without allocation or I/O; get promotes the window in LRU order. */
  peek(key: string | number): T | undefined {
    return this.cache.get(key);
  }

  /** Fetch or reuse exactly the requested result and range. */
  get(request: WindowRequest): Promise<T> {
    const key = this.keyOf(request);
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
        const started = performance.now();
        this.cache.set(key, tile);
        if (this.cache.size > this.capacity) {
          const oldest = this.cache.keys().next().value;
          if (oldest !== undefined) this.cache.delete(oldest);
        }
        this.onFill(performance.now() - started);
        return tile;
      })
      .finally(() => this.pending.delete(key));
    this.pending.set(key, promise);
    return promise;
  }
}

function requestKey(request: WindowRequest): string {
  return JSON.stringify([
    request.resultId,
    request.rowStart,
    request.rowCount,
    request.colStart,
    request.colCount,
  ]);
}
