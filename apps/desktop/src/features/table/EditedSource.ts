// SPDX-License-Identifier: Apache-2.0
import type {
  CellRange,
  DisplayCell,
  GridColumn,
  GridDataSource,
} from '@openrows/grid';
import type { StagedEdit } from './editHistory';
import type { CellValue } from './cellValue';

/** How a staged value paints: NULL, EMPTY and DEFAULT (also an unset insert's column) are muted markers. */
export function stagedCell(
  value: CellValue | undefined,
  state: 'changed' | 'inserted',
): DisplayCell {
  if (value === undefined || value.kind === 'default')
    return { text: 'DEFAULT', state, marker: 'default' };
  if (value.kind === 'null') return { text: 'NULL', state, marker: 'null' };
  if (value.value === '') return { text: 'EMPTY', state, marker: 'empty' };
  return { text: value.value, state };
}

/** Staged inserts in the order they appear after the page's rows. */
export function stagedInserts(
  changes: readonly StagedEdit[],
): readonly StagedEdit[] {
  return changes.filter((change) => change.kind === 'insert');
}

/**
 * Build staged highlights on tile arrival, outside the canvas paint loop.
 * Source columns past `identity.shown` (a keyless table's rowid) are never
 * shown but always fetched, so staged rows still match by them.
 */
export class EditedSource implements GridDataSource {
  private readonly loaded = new Set<number>();
  private readonly overrides = new Map<number, DisplayCell>();
  private readonly listeners = new Set<() => void>();
  private inserted: readonly (readonly DisplayCell[])[] = [];
  private readonly shown: GridDataSource['columns'];
  constructor(
    private readonly source: GridDataSource,
    private changes: readonly StagedEdit[],
    private identity: {
      keys: readonly string[];
      names: readonly string[];
      /** Leading source columns the grid shows; default all. */
      shown?: number;
    },
  ) {
    this.shown =
      identity.shown === undefined
        ? source.columns
        : source.columns.slice(0, identity.shown);
    for (let row = 0; row < Math.min(source.rows, 256); row++)
      this.loaded.add(row);
    this.prepareInserts();
  }
  /** Swap the staged overlay in place so the grid keeps its selection and scroll. */
  stage(changes: readonly StagedEdit[], keys: readonly string[]): void {
    if (changes === this.changes && keys === this.identity.keys) return;
    this.changes = changes;
    this.identity = { ...this.identity, keys };
    this.prepareInserts();
    this.prepare();
    this.notify();
  }
  /** Set boolean/enum column kinds; true when any changed and the grid must refresh. */
  describe(kinds: readonly GridColumn['kind'][]): boolean {
    let changed = false;
    for (const [index, column] of this.shown.entries()) {
      const kind = kinds[index];
      if (column.kind === kind) continue;
      changed = true;
      if (kind) column.kind = kind;
      else delete column.kind;
    }
    return changed;
  }
  /** Page rows, then one row per staged insert. */
  get rows(): number {
    return this.source.rows + this.inserted.length;
  }
  get columns(): GridDataSource['columns'] {
    return this.shown;
  }
  get metrics(): GridDataSource['metrics'] {
    return this.source.metrics;
  }
  cell(row: number, column: number): DisplayCell | undefined {
    if (row >= this.source.rows)
      return this.inserted[row - this.source.rows]?.[column];
    return (
      this.overrides.get(row * this.columns.length + column) ??
      this.source.cell(row, column)
    );
  }
  /** Staged insert rows are local; only page rows are fetched, through any hidden columns. */
  async window(range: CellRange, direction: number): Promise<void> {
    const last = Math.min(range.r1, this.source.rows - 1);
    // Match only requested tiles, never scan a million-row result on each keystroke.
    if (this.loaded.size > 16_384) this.loaded.clear();
    for (let row = range.r0; row <= last; row++) this.loaded.add(row);
    const hidden = this.source.columns.length > this.shown.length;
    const c1 = hidden ? this.source.columns.length - 1 : range.c1;
    if (last >= 0)
      await this.source.window(
        { ...range, r0: Math.min(range.r0, last), r1: last, c1 },
        direction,
      );
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
  private notify(): void {
    this.listeners.forEach((listener) => {
      listener();
    });
  }
  private prepare(): void {
    this.overrides.clear();
    if (!this.identity.keys.length || !this.changes.length) return;
    for (const row of this.loaded) {
      const edit = this.changes.find(
        (change) => change.kind !== 'insert' && this.matches(row, change),
      );
      if (edit) this.override(row, edit);
    }
  }
  private prepareInserts(): void {
    const names = this.identity.names.slice(0, this.shown.length);
    this.inserted = stagedInserts(this.changes).map((edit) =>
      names.map((name) =>
        stagedCell(
          Object.hasOwn(edit.values, name) ? edit.values[name] : undefined,
          'inserted',
        ),
      ),
    );
  }
  private matches(row: number, change: StagedEdit): boolean {
    return this.identity.keys.every((name) => {
      const column = this.identity.names.indexOf(name);
      const cell = column >= 0 ? this.source.cell(row, column) : undefined;
      return (
        cell &&
        (cell.state === 'empty' ? '' : cell.text) === change.original[name]
      );
    });
  }
  private override(row: number, edit: StagedEdit): void {
    this.columns.forEach((_column, index) => {
      const name = this.identity.names[index] ?? '';
      const cell = this.source.cell(row, index);
      const at = row * this.columns.length + index;
      if (Object.hasOwn(edit.values, name))
        this.overrides.set(at, stagedCell(edit.values[name], 'changed'));
      else if (edit.kind === 'delete' && cell)
        this.overrides.set(at, { ...cell, state: 'deleted' });
    });
  }
}
