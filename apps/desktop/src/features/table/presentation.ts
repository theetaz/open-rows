// SPDX-License-Identifier: Apache-2.0
import type {
  CombinatorDto,
  FilterDto,
  FilterOperatorDto,
  SortDto,
} from '@openrows/ipc';
import { operators } from './model';
import type { FilterRow, TableView } from './model';

/** A named set of filter rows, saved per table. */
export type SavedFilter = {
  name: string;
  rows: readonly FilterRow[];
  combinator: CombinatorDto;
};

/**
 * What a table tab remembers across sessions (FR-GRID-12): hidden columns,
 * sort and filters; widths, order and pins live in the column layout.
 */
export type TablePresentation = {
  hidden: readonly string[];
  sort: SortDto | null;
  rows: readonly FilterRow[];
  applied: readonly FilterDto[];
  combinator: CombinatorDto;
  saved: readonly SavedFilter[];
};

export const emptyPresentation: TablePresentation = {
  hidden: [],
  sort: null,
  rows: [],
  applied: [],
  combinator: 'and',
  saved: [],
};

const known = new Set<string>(operators.map((item) => item.value));

const text = (value: unknown): value is string => typeof value === 'string';

const strings = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter(text) : [];

const combinatorOf = (value: unknown): CombinatorDto =>
  value === 'or' ? 'or' : 'and';

function filterOf(value: unknown): FilterDto | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  const { column, operator, second } = record;
  if (!text(column) || !text(operator) || !known.has(operator)) return null;
  return {
    column,
    operator: operator as FilterOperatorDto,
    value: text(record.value) ? record.value : '',
    second: text(second) ? second : '',
  };
}

function rowOf(value: unknown): FilterRow | null {
  const filter = filterOf(value);
  const record = value as Record<string, unknown> | null;
  if (!filter || typeof record?.id !== 'number') return null;
  return { ...filter, id: record.id, enabled: record.enabled !== false };
}

function listOf<T>(value: unknown, parse: (item: unknown) => T | null): T[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    const parsed = parse(item);
    return parsed === null ? [] : [parsed];
  });
}

function sortOf(value: unknown): SortDto | null {
  if (typeof value !== 'object' || value === null) return null;
  const { column, descending } = value as Record<string, unknown>;
  return text(column) ? { column, descending: descending === true } : null;
}

function savedOf(value: unknown): SavedFilter | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  if (!text(record.name) || !record.name.trim()) return null;
  return {
    name: record.name,
    rows: listOf(record.rows, rowOf),
    combinator: combinatorOf(record.combinator),
  };
}

/** Validate one stored presentation, dropping anything malformed. */
export function parsePresentation(value: unknown): TablePresentation | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  return {
    hidden: strings(record.hidden),
    sort: sortOf(record.sort),
    rows: listOf(record.rows, rowOf),
    applied: listOf(record.applied, filterOf),
    combinator: combinatorOf(record.combinator),
    saved: listOf(record.saved, savedOf),
  };
}

/** The remembered parts of a view, keeping the table's saved filters. */
export function presentationOf(
  view: Pick<TableView, 'hidden' | 'sort' | 'rows' | 'applied' | 'combinator'>,
  saved: readonly SavedFilter[],
): TablePresentation {
  return {
    hidden: view.hidden,
    sort: view.sort,
    rows: view.rows,
    applied: view.applied,
    combinator: view.combinator,
    saved,
  };
}

/** A fresh view with the remembered presentation applied. */
export function restoredView(
  view: TableView,
  stored: TablePresentation,
): TableView {
  const next = Math.max(view.nextRow, ...stored.rows.map((row) => row.id + 1));
  return {
    ...view,
    hidden: stored.hidden,
    sort: stored.sort,
    rows: stored.rows,
    applied: stored.applied,
    combinator: stored.combinator,
    nextRow: next,
    filtersOpen: stored.applied.length > 0,
    restored: true,
  };
}

/** Load a saved filter into the bar and apply its enabled rows. */
export function withSavedFilter(
  view: TableView,
  saved: SavedFilter,
): TableView {
  const rows = saved.rows.map((row, index) => ({
    ...row,
    id: view.nextRow + index,
  }));
  return {
    ...view,
    rows,
    combinator: saved.combinator,
    applied: rows
      .filter((row) => row.enabled)
      .map(({ column, operator, value, second }) => ({
        column,
        operator,
        value,
        second,
      })),
    nextRow: view.nextRow + rows.length,
    filtersOpen: true,
    offset: 0,
  };
}

/** Saved filters with `filter` added, replacing one of the same name. */
export function savedWith(
  saved: readonly SavedFilter[],
  filter: SavedFilter,
): SavedFilter[] {
  return [...saved.filter((item) => item.name !== filter.name), filter].sort(
    (a, b) => a.name.localeCompare(b.name),
  );
}
