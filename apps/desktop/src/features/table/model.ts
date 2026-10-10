// SPDX-License-Identifier: Apache-2.0
import type {
  CombinatorDto,
  FilterDto,
  FilterGroupDto,
  FilterOperatorDto,
  SortDto,
  TableQueryDto,
} from '@openrows/ipc';

/** Rows per page until page size becomes a setting (FR-GRID-08). */
const PAGE_SIZE = 300;

/** One row of the filter bar; only enabled rows are applied. */
export type FilterRow = {
  id: number;
  /** Group identity; legacy rows belong to group 1. */
  group?: number;
  enabled: boolean;
  column: string;
  operator: FilterOperatorDto;
  value: string;
  /** BETWEEN's upper bound. */
  second: string;
};

/** The join used by each parenthesised filter group. */
export type FilterGroup = { id: number; combinator: CombinatorDto };

export type ViewMode = 'data' | 'structure' | 'indexes';

/** A table tab's view settings; row data stays in the core's result store. */
export type TableView = {
  mode: ViewMode;
  offset: number;
  limit: number;
  sort: SortDto | null;
  /** Filter bar rows being edited. */
  rows: readonly FilterRow[];
  /** Filters in effect for the shown page. */
  applied: readonly FilterDto[];
  combinator: CombinatorDto;
  groups?: readonly FilterGroup[] | undefined;
  appliedGroups?: readonly FilterGroupDto[] | undefined;
  /** Hidden column names; the page selects only the visible ones. */
  hidden: readonly string[];
  filtersOpen: boolean;
  columnsOpen: boolean;
  nextRow: number;
  rowNumbers: boolean;
  striped: boolean;
  revision: number;
  /** The remembered presentation (FR-GRID-12) has been applied. */
  restored: boolean;
  /** Row form view instead of the grid (FR-GRID-09). */
  form: boolean;
};

export const initialView: TableView = {
  mode: 'data',
  offset: 0,
  limit: PAGE_SIZE,
  sort: null,
  rows: [],
  applied: [],
  combinator: 'and',
  hidden: [],
  filtersOpen: false,
  columnsOpen: false,
  nextRow: 1,
  rowNumbers: true,
  striped: true,
  revision: 0,
  restored: false,
  form: false,
};

export type OperatorInfo = {
  value: FilterOperatorDto;
  label: string;
  /** Number of value fields: 0 for NULL checks, 2 for BETWEEN. */
  arity: 0 | 1 | 2;
};

/** Filter operators in menu order (FR-GRID-11 subset). */
export const operators: readonly OperatorInfo[] = [
  { value: 'equal', label: '=', arity: 1 },
  { value: 'notEqual', label: '≠', arity: 1 },
  { value: 'less', label: '<', arity: 1 },
  { value: 'greater', label: '>', arity: 1 },
  { value: 'lessOrEqual', label: '≤', arity: 1 },
  { value: 'greaterOrEqual', label: '≥', arity: 1 },
  { value: 'contains', label: 'contains', arity: 1 },
  { value: 'notContains', label: 'not contains', arity: 1 },
  { value: 'startsWith', label: 'starts with', arity: 1 },
  { value: 'endsWith', label: 'ends with', arity: 1 },
  { value: 'in', label: 'IN', arity: 1 },
  { value: 'isNull', label: 'IS NULL', arity: 0 },
  { value: 'isNotNull', label: 'IS NOT NULL', arity: 0 },
  { value: 'between', label: 'BETWEEN', arity: 2 },
  { value: 'notBetween', label: 'NOT BETWEEN', arity: 2 },
  { value: 'notIn', label: 'NOT IN', arity: 1 },
  { value: 'like', label: 'LIKE pattern', arity: 1 },
  { value: 'notLike', label: 'NOT LIKE pattern', arity: 1 },
  { value: 'regex', label: 'Regex', arity: 1 },
  { value: 'notRegex', label: 'Not regex', arity: 1 },
  { value: 'isEmpty', label: 'IS EMPTY', arity: 0 },
  { value: 'isNotEmpty', label: 'IS NOT EMPTY', arity: 0 },
  { value: 'raw', label: 'Raw SQL', arity: 1 },
];

/** How many value fields an operator takes. */
export function arity(operator: FilterOperatorDto): 0 | 1 | 2 {
  return operators.find((item) => item.value === operator)?.arity ?? 1;
}

/** Header click cycles ascending → descending → unsorted; another column starts ascending. */
export function nextSort(sort: SortDto | null, column: string): SortDto | null {
  if (sort?.column !== column) return { column, descending: false };
  return sort.descending ? null : { column, descending: true };
}

/** A new enabled filter row on `column` with `=`. */
export function filterRow(id: number, column: string): FilterRow {
  return {
    id,
    enabled: true,
    column,
    operator: 'equal',
    value: '',
    second: '',
  };
}

/** The filters a row list applies; `only` applies a single row (its Apply button). */
export function toFilters(
  rows: readonly FilterRow[],
  only?: number,
): FilterDto[] {
  return rows
    .filter((row) => (only === undefined ? row.enabled : row.id === only))
    .map(({ column, operator, value, second }) => ({
      column,
      operator,
      value,
      second,
    }));
}

/** The relation a table tab reads. */
export type TableTargetRef = {
  profileId: string;
  database: string;
  schema: string;
  table: string;
};

/** The relation a table tab on `profileId` reads. */
export function tableTarget(
  table: { database: string; schema: string; name: string },
  profileId: string,
): TableTargetRef {
  return {
    profileId,
    database: table.database,
    schema: table.schema,
    table: table.name,
  };
}

/**
 * The request model of the view; every column when none is hidden. `rowId`
 * (a keyless table's rowid) is selected last, for row identity only.
 */
export function tableQuery(
  target: TableTargetRef,
  view: TableView,
  columns: readonly string[],
  rowId: string | null = null,
): TableQueryDto {
  const visible = view.hidden.length
    ? columns.filter((column) => !view.hidden.includes(column))
    : [];
  return {
    target,
    columns: visible,
    filters: view.appliedGroups ? [] : [...view.applied],
    ...(view.appliedGroups ? { groups: [...view.appliedGroups] } : {}),
    combinator: view.combinator,
    sort: view.sort,
    ...(rowId ? { rowId } : {}),
  };
}

/**
 * Select the shown columns in the user's arranged order (dragged headers,
 * pinned first) when it differs from the catalog's, so result column
 * indexes always match what the grid shows.
 */
export function arrangedQuery(
  query: TableQueryDto,
  catalog: readonly string[],
  arranged: readonly string[],
  hidden: readonly string[],
): TableQueryDto {
  if (arranged.every((name, index) => name === catalog[index])) return query;
  return {
    ...query,
    columns: arranged.filter((name) => !hidden.includes(name)),
  };
}

/** Identity of a page request: a different key means the page must reload. */
export function pageKey(
  query: TableQueryDto,
  view: TableView,
  sessionId: string,
): string {
  return JSON.stringify([
    sessionId,
    query,
    view.offset,
    view.limit,
    view.revision,
  ]);
}

/** Identity of the rows a count covers (filters only). */
export function countKey(query: TableQueryDto): string {
  return JSON.stringify([
    query.target,
    query.filters,
    query.groups,
    query.combinator,
  ]);
}

const number = (value: number): string => value.toLocaleString('en-US');

/** What the row total is known as: an exact count or the planner's estimate. */
export type Total = { exact: number | null; estimate: number | null };

/** Footer text such as `1–300 of ~1,811 rows`. */
export function rangeLabel(offset: number, rows: number, total: Total): string {
  if (rows === 0)
    return offset === 0 ? 'No rows' : `No rows after ${number(offset)}`;
  const range = `${number(offset + 1)}–${number(offset + rows)}`;
  if (total.exact !== null) return `${range} of ${number(total.exact)} rows`;
  if (total.estimate !== null)
    return `${range} of ~${number(Math.max(total.estimate, offset + rows))} rows`;
  return `${range} rows`;
}
