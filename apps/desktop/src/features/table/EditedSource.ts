// SPDX-License-Identifier: Apache-2.0
import type { CellRange, DisplayCell, GridDataSource } from '@openrows/grid';
import type { StagedEdit } from './editHistory';
import { stagedDisplay } from './cellValue';
import type { CellValue } from './cellValue';

/** Display state of a staged value: NULL, DEFAULT and EMPTY paint as muted markers. */
export function stagedCell(value: CellValue): DisplayCell {
  const shown = stagedDisplay(value);
  return shown.placeholder
    ? { text: shown.text, state: 'changed', placeholder: true }
    : { text: shown.text, state: 'changed' };
}
/** Build staged highlights on tile arrival, outside the canvas paint loop. */
export class EditedSource implements GridDataSource {
  private readonly overrides = new Map<number, DisplayCell>();
  private readonly listeners = new Set<() => void>();
  constructor(
    private readonly source: GridDataSource,
    private changes: readonly StagedEdit[],
    private identity: {
      keys: readonly string[];
      names: readonly string[];
    },
  ) {}
  /** Swap the staged overlay in place so the grid keeps its selection and scroll. */
  stage(changes: readonly StagedEdit[], keys: readonly string[]): void {
    if (changes === this.changes && keys === this.identity.keys) return;
    this.changes = changes;
    this.identity = { ...this.identity, keys };
    this.prepare();
    this.listeners.forEach((listener) => {
      listener();
    });
  }
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
    this.listeners.add(listener);
    const unsubscribe = this.source.subscribe(() => {
      this.prepare();
      listener();
    });
    return () => {
      this.listeners.delete(listener);
      unsubscribe();
    };
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
        const staged = Object.hasOwn(edit.values, name)
          ? edit.values[name]
          : undefined;
        const cell = this.source.cell(row, index);
        if (staged)
          this.overrides.set(
            row * this.columns.length + index,
            stagedCell(staged),
          );
        else if (edit.kind === 'delete' && cell)
          this.overrides.set(row * this.columns.length + index, {
            ...cell,
            state: 'deleted',
          });
      });
    }
  }
}
