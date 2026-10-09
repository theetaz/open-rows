// SPDX-License-Identifier: Apache-2.0
import { convertDisplayTile, type DisplayTile } from './displayTile';
import type { WindowRequest } from '@openrows/ipc';
import { TileClient } from './tileClient';
import type { Tile } from './tiles';
import type {
  CellRange,
  DisplayCell,
  GridColumn,
  GridDataSource,
} from './model';

/** Tile geometry matches S1; column windows are twenty wide even in the 100-column variant. */
const TILE_ROWS = 256,
  TILE_COLS = 20;
/** Adapt S1's bounded deduplicating LRU to synchronous display lookups and directional prefetch. */
export class TileSource implements GridDataSource {
  readonly metrics = {
    requested: 0,
    hits: 0,
    tileApplyMs: 0,
    tileConvertMs: 0,
    workerConvertMs: 0,
    tileAllocations: 0,
    arrayAllocations: 0,
    cellAllocations: 0,
  };
  private readonly client: TileClient<DisplayTile>;
  private readonly listeners = new Set<() => void>();
  private readonly columnTiles: number;
  private previousColumn = 0;
  private windowKey = -1;
  private filling: Promise<void> = Promise.resolve();
  private count: number;
  constructor(
    private readonly resultId: string,
    rows: number,
    readonly columns: readonly GridColumn[],
    load: (request: WindowRequest) => Promise<Tile | DisplayTile>,
  ) {
    this.count = rows;
    this.columnTiles = Math.ceil(columns.length / TILE_COLS);
    this.client = new TileClient(
      async (request) => {
        this.metrics.requested++;
        const tile = await load(request);
        const started = performance.now();
        const display =
          'cells' in tile ? tile : convertDisplayTile(tile, request);
        this.metrics.tileConvertMs += performance.now() - started;
        this.metrics.workerConvertMs += 'cells' in tile ? display.convertMs : 0;
        this.metrics.tileAllocations++;
        this.metrics.arrayAllocations++;
        this.metrics.cellAllocations += display.cells.length;
        return display;
      },
      64,
      (request) => this.key(request.rowStart, request.colStart),
      (applyMs) => {
        this.metrics.tileApplyMs += applyMs;
        const started = performance.now();
        for (const listener of this.listeners) listener();
        this.metrics.tileApplyMs += performance.now() - started;
      },
    );
  }
  /** Rows the result holds so far. */
  get rows(): number {
    return this.count;
  }
  /**
   * A streaming result grew: drop the partial last tile row, which was cut
   * at the old count, and notify the grid of the new row count.
   */
  grow(rows: number): void {
    if (rows <= this.count) return;
    const tail = Math.floor(this.count / TILE_ROWS);
    if (this.count % TILE_ROWS)
      for (let col = 0; col < this.columnTiles; col++)
        this.client.evict(tail * this.columnTiles + col);
    this.count = rows;
    this.windowKey = -1;
    for (const listener of this.listeners) listener();
  }
  /** Allocation-free cached lookup; placeholders remain undefined until arrival. */
  cell(row: number, column: number): DisplayCell | undefined {
    const tile = this.client.peek(this.key(row, column));
    return tile?.cells[(row % TILE_ROWS) * tile.columns + (column % TILE_COLS)];
  }
  /** Request visible tiles first, including pins, then one tile in the scroll direction. */
  window(range: CellRange, direction: number): Promise<void> {
    const key =
      (((range.r0 >> 8) * 4096 + (range.r1 >> 8)) * this.columnTiles +
        Math.floor(range.c0 / 20)) *
        this.columnTiles *
        3 +
      Math.floor(range.c1 / 20) * 3 +
      direction +
      1;
    if (key === this.windowKey) return this.filling;
    this.windowKey = key;
    this.filling = this.fill(range, direction);
    return this.filling;
  }
  private async fill(range: CellRange, direction: number): Promise<void> {
    const requests: Promise<DisplayTile>[] = [];
    const first = Math.floor(range.r0 / TILE_ROWS),
      last = Math.floor(range.r1 / TILE_ROWS);
    for (let row = first; row <= last; row++)
      this.requestColumns(requests, row, range);
    const next = direction < 0 ? first - 1 : last + 1;
    if (direction !== 0 && next >= 0 && next * TILE_ROWS < this.rows)
      this.requestColumns(requests, next, range);
    this.prefetchColumns(requests, range);
    this.metrics.arrayAllocations += 2;
    await Promise.all(requests);
  }
  /** Data arrival invalidates the renderer; unsubscribe before disposing it. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  private key(row: number, column: number): number {
    return (
      Math.floor(row / TILE_ROWS) * this.columnTiles +
      Math.floor(column / TILE_COLS)
    );
  }
  private prefetchColumns(
    requests: Promise<DisplayTile>[],
    range: CellRange,
  ): void {
    const first = Math.floor(range.r0 / TILE_ROWS),
      last = Math.floor(range.r1 / TILE_ROWS);
    const horizontal = Math.sign(range.c0 - this.previousColumn);
    this.previousColumn = range.c0;
    const nextColumn =
      Math.floor((horizontal < 0 ? range.c0 : range.c1) / TILE_COLS) +
      horizontal;
    if (horizontal !== 0 && nextColumn >= 0 && nextColumn < this.columnTiles) {
      for (let row = first; row <= last; row++)
        requests.push(this.request(row, nextColumn));
    }
  }
  private requestColumns(
    requests: Promise<DisplayTile>[],
    row: number,
    range: CellRange,
  ): void {
    const start = Math.floor(range.c0 / TILE_COLS),
      end = Math.floor(range.c1 / TILE_COLS);
    // Read per window: a host may pin columns in place (see GridHandle.relayout).
    const pinned = this.columns.filter((column) => column.pinned).length;

    for (let col = 0; col <= end; col++) {
      if (col >= start || col * TILE_COLS < pinned)
        requests.push(this.request(row, col));
    }
  }
  private request(row: number, col: number): Promise<DisplayTile> {
    const rowStart = row * TILE_ROWS,
      colStart = col * TILE_COLS;
    if (this.client.peek(this.key(rowStart, colStart))) this.metrics.hits++;
    return this.client.get({
      resultId: this.resultId,
      rowStart,
      colStart,
      rowCount: Math.min(TILE_ROWS, this.rows - rowStart),
      colCount: Math.min(TILE_COLS, this.columns.length - colStart),
    });
  }
}
