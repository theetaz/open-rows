// SPDX-License-Identifier: Apache-2.0
import type {
  CombinatorDto,
  FilterDto,
  FilterGroupDto,
  FilterOperatorDto,
  SortDto,
} from '@openrows/ipc';
import { appliedGroups } from './filters/groups';
import type { FilterGroup } from './model';
import { operators } from './model';
import type { FilterRow, TableView } from './model';

/** A named set of filter rows, saved per table. */
export type SavedFilter = {
  name: string;
  rows: readonly FilterRow[];
  combinator: CombinatorDto;
  groups?: readonly FilterGroup[] | undefined;
};

/**
 * What a table tab remembers across sessions (FR-GRID-12): hidden columns,
 * sort and filters; widths, order and pins live in the column layout.
 */
export type TablePresentation = {
  version?: 2;
  groups?: readonly FilterGroup[] | undefined;
  appliedGroups?: readonly FilterGroupDto[] | undefined;
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
  return {
    ...filter,
    id: record.id,
    enabled: record.enabled !== false,
    ...(typeof record.group === 'number' &&
    Number.isSafeInteger(record.group) &&
    record.group > 0
      ? { group: record.group }
      : {}),
  };
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
    groups: groupsFrom(record),
    combinator: combinatorOf(record.combinator),
  };
}

/** Validate one stored presentation, dropping anything malformed. */
export function parsePresentation(value: unknown): TablePresentation | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  if (record.version !== undefined && record.version !== 2) return null;
  return {
    version: 2,
    groups: groupsFrom(record),
    appliedGroups: appliedFrom(record),
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
  view: Pick<
    TableView,
    | 'hidden'
    | 'sort'
    | 'rows'
    | 'applied'
    | 'combinator'
    | 'groups'
    | 'appliedGroups'
  >,
  saved: readonly SavedFilter[],
): TablePresentation {
  return {
    version: 2,
    groups: view.groups,
    appliedGroups: view.appliedGroups,
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
    groups: stored.groups,
    appliedGroups: stored.appliedGroups,
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
    groups: saved.groups,
    appliedGroups: saved.groups
      ? appliedGroups({ ...view, rows, groups: saved.groups })
      : undefined,
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

/** Version-one presentations and saved filters migrate to one group. */
function groupsFrom(record: Record<string, unknown>): FilterGroup[] {
  const groups = listOf(record.groups, (value): FilterGroup | null => {
    if (typeof value !== 'object' || value === null) return null;
    const group = value as Record<string, unknown>;
    return typeof group.id === 'number' &&
      Number.isSafeInteger(group.id) &&
      group.id > 0
      ? { id: group.id, combinator: combinatorOf(group.combinator) }
      : null;
  });
  return groups.length
    ? groups
    : [{ id: 1, combinator: combinatorOf(record.combinator) }];
}

function appliedFrom(record: Record<string, unknown>): FilterGroupDto[] {
  if (!Array.isArray(record.appliedGroups)) {
    const filters = listOf(record.applied, filterOf);
    return filters.length
      ? [{ filters, combinator: combinatorOf(record.combinator) }]
      : [];
  }
  return listOf(record.appliedGroups, (value): FilterGroupDto | null => {
    if (typeof value !== 'object' || value === null) return null;
    const group = value as Record<string, unknown>;
    return {
      filters: listOf(group.filters, filterOf),
      combinator: combinatorOf(group.combinator),
    };
  });
}
