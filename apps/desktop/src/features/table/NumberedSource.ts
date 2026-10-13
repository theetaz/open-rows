// SPDX-License-Identifier: Apache-2.0
import type { CellRange, DisplayCell, GridDataSource } from '@openrows/grid';

/** Staged inserts after the page's rows have no number yet. */
const newRow: DisplayCell = { text: '+', state: 'inserted' };

/** A pinned, page-aware row gutter; result column indices are never sent to IPC shifted. */
export class NumberedSource implements GridDataSource {
  readonly columns: GridDataSource['columns'];
  private readonly numbers = new Map<number, DisplayCell>();
  private readonly page: number;
  constructor(
    private readonly source: GridDataSource,
    private readonly offset: number,
    /** Rows on the page; later rows are staged inserts. */
    page = source.rows,
  ) {
    this.page = page;
    this.prepare(0, Math.min(page, 512) - 1);
    this.columns = [
      {
        id: 'row-number',
        title: '↔',
        width: Math.max(48, String(offset + source.rows).length * 8 + 16),
        pinned: true,
        align: 'end',
      },
      ...source.columns,
    ];
  }
  get rows(): number {
    return this.source.rows;
  }
  get metrics(): GridDataSource['metrics'] {
    return this.source.metrics;
  }
  cell(row: number, column: number): DisplayCell | undefined {
    if (column !== 0) return this.source.cell(row, column - 1);
    return row < this.page ? this.numbers.get(row) : newRow;
  }
  async window(range: CellRange, direction: number): Promise<void> {
    this.prepare(range.r0, range.r1);
    await this.source.window(
      {
        ...range,
        c0: Math.max(0, range.c0 - 1),
        c1: Math.max(0, range.c1 - 1),
      },
      direction,
    );
  }
  /** Only viewport numbers are materialized, including million-row query results. */
  private prepare(first: number, last: number): void {
    if (this.numbers.size > 2048) this.numbers.clear();
    for (let row = first; row <= Math.min(last, this.page - 1); row++)
      if (!this.numbers.has(row))
        this.numbers.set(row, {
          text: String(this.offset + row + 1),
          state: 'normal',
        });
  }
  subscribe(listener: () => void): () => void {
    return this.source.subscribe(listener);
  }
}
