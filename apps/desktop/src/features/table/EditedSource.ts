// SPDX-License-Identifier: Apache-2.0
import type { CellRange, DisplayCell, GridDataSource } from '@openrows/grid';
import type { StagedEdit } from './editHistory';
/** Build staged highlights on tile arrival, outside the canvas paint loop. */
export class EditedSource implements GridDataSource {
  private readonly overrides = new Map<number, DisplayCell>();
  constructor(
    private readonly source: GridDataSource,
    private readonly changes: readonly StagedEdit[],
    private readonly identity: {
      keys: readonly string[];
      names: readonly string[];
    },
  ) {}
  get rows(): number {
    return this.source.rows;
  }
  get columns(): GridDataSource['columns'] {
    return this.source.columns;
  }
  get metrics(): GridDataSource['metrics'] {
    return this.source.metrics;
  }
  cell(row: number, column: number): DisplayCell | undefined {
    return (
      this.overrides.get(row * this.columns.length + column) ??
      this.source.cell(row, column)
    );
  }
  async window(range: CellRange, direction: number): Promise<void> {
    await this.source.window(range, direction);
    this.prepare();
  }
  subscribe(listener: () => void): () => void {
    return this.source.subscribe(() => {
      this.prepare();
      listener();
    });
  }
  private prepare(): void {
    this.overrides.clear();
    if (!this.identity.keys.length || !this.changes.length) return;
    for (let row = 0; row < this.rows; row++) {
      const edit = this.changes.find(
        (change) =>
          change.kind !== 'insert' &&
          this.identity.keys.every((name) => {
            const column = this.identity.names.indexOf(name);
            const cell =
              column >= 0 ? this.source.cell(row, column) : undefined;
            return (
              cell &&
              (cell.state === 'empty' ? '' : cell.text) ===
                change.original[name]
            );
          }),
      );
      if (!edit) continue;
      this.columns.forEach((_column, index) => {
        const name = this.identity.names[index] ?? '';
        const changed = Object.hasOwn(edit.values, name);
        const cell = this.source.cell(row, index);
        if (changed)
          this.overrides.set(row * this.columns.length + index, {
            text: edit.values[name] ?? 'NULL',
            state: 'changed',
          });
        else if (edit.kind === 'delete' && cell)
          this.overrides.set(row * this.columns.length + index, {
            ...cell,
            state: 'deleted',
          });
      });
    }
  }
}
