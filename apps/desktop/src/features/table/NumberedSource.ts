// SPDX-License-Identifier: Apache-2.0
import type { CellRange, DisplayCell, GridDataSource } from '@openrows/grid';

/** A pinned, page-aware row gutter; result column indices are never sent to IPC shifted. */
export class NumberedSource implements GridDataSource {
  readonly columns;
  private readonly numbers: readonly DisplayCell[];
  constructor(
    private readonly source: GridDataSource,
    offset: number,
  ) {
    this.numbers = Array.from({ length: source.rows }, (_, row) => ({
      text: String(offset + row + 1),
      state: 'normal' as const,
    }));
    this.columns = [
      {
        id: 'row-number',
        title: '#',
        width: Math.max(48, String(offset + source.rows).length * 8 + 16),
        pinned: true,
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
    return column === 0 ? this.numbers[row] : this.source.cell(row, column - 1);
  }
  async window(range: CellRange, direction: number): Promise<void> {
    await this.source.window(
      {
        ...range,
        c0: Math.max(0, range.c0 - 1),
        c1: Math.max(0, range.c1 - 1),
      },
      direction,
    );
  }
  subscribe(listener: () => void): () => void {
    return this.source.subscribe(listener);
  }
}
