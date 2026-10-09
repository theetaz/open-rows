// SPDX-License-Identifier: Apache-2.0
import type {
  CellRange,
  DisplayCell,
  GridColumn,
  GridDataSource,
  TileSource,
} from '@openrows/grid';
import type { LocalRows } from './localRows';

/**
 * A query result as the grid shows it: columns in the user's order (pinned
 * first) and, while sorted or filtered, only the matching rows in order.
 * Unsorted rows load through result windows; sorted ones read loaded rows.
 */
export class ResultViewSource implements GridDataSource {
  columns: readonly GridColumn[];
  private order: Int32Array;
  private shown: { local: LocalRows; rows: Int32Array } | null = null;
  private readonly listeners = new Set<() => void>();
  constructor(private readonly base: TileSource) {
    this.columns = base.columns;
    this.order = Int32Array.from(base.columns, (_, index) => index);
    base.subscribe(this.notify);
  }
  get rows(): number {
    return this.shown ? this.shown.rows.length : this.base.rows;
  }
  get metrics(): GridDataSource['metrics'] {
    return this.base.metrics;
  }
  /** The result row shown at view row `row`. */
  rowOf(row: number): number {
    return this.shown ? (this.shown.rows[row] ?? row) : row;
  }
  /** The result column shown at view column `column`. */
  columnOf(column: number): number {
    return this.order[column] ?? column;
  }
  cell(row: number, column: number): DisplayCell | undefined {
    if (this.shown)
      return this.shown.local.cell(this.rowOf(row), this.columnOf(column));
    return this.base.cell(row, this.columnOf(column));
  }
  /** Fetch the result columns behind the visible and pinned view columns. */
  window(range: CellRange, direction: number): Promise<void> {
    if (this.shown) return Promise.resolve();
    let c0 = Number.MAX_SAFE_INTEGER,
      c1 = -1;
    const pinned = this.columns.filter((column) => column.pinned).length;
    for (let column = 0; column <= range.c1; column++) {
      if (column >= range.c0 || column < pinned) {
        const stored = this.columnOf(column);
        c0 = Math.min(c0, stored);
        c1 = Math.max(c1, stored);
      }
    }
    if (c1 < 0) return Promise.resolve();
    return this.base.window({ ...range, c0, c1 }, direction);
  }
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  /** New column order, widths or titles; the caller relayouts the grid. */
  arrange(columns: readonly GridColumn[], order: readonly number[]): void {
    this.columns = columns;
    this.order = Int32Array.from(order);
  }
  /** Show only `rows` (sorted or filtered) of `local`, or every row again. */
  show(view: { local: LocalRows; rows: Int32Array } | null): void {
    this.shown = view;
    this.notify();
  }
  /** More rows of a streaming result arrived. */
  grow(rows: number): void {
    this.base.grow(rows);
  }
  private readonly notify = (): void => {
    for (const listener of this.listeners) listener();
  };
}
