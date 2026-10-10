// SPDX-License-Identifier: Apache-2.0
import type {
  FilterDto,
  SchemaDiagramDto,
  TableDetailDto,
} from '@openrows/ipc';

/**
 * One foreign key seen from the browsed table (FR-GRID-07): `columns` are
 * this table's columns and `targetColumns` the matching columns of `table`.
 * Outgoing keys open the referenced row; incoming keys list the rows that
 * reference this one.
 */
export type KeyLink = {
  /** Constraint name. */
  name: string;
  schema: string;
  sourceSchema?: string;
  /** The other table. */
  table: string;
  columns: readonly string[];
  targetColumns: readonly string[];
};

/** The browsed table's foreign keys in both directions. */
export type TableLinks = {
  outgoing: readonly KeyLink[];
  incoming: readonly KeyLink[];
};

export const noLinks: TableLinks = { outgoing: [], incoming: [] };

/** A field value as the jump reads it: display text, or `null` for SQL NULL. */
export type KeyValue = string | null;

/** Primary-key columns of `table`, for keys that leave the target implicit (SQLite). */
function primaryKey(diagram: SchemaDiagramDto, table: string): string[] {
  const found = diagram.tables.find((item) => item.name === table);
  return (
    found?.columns
      .filter((column) => column.primaryKey)
      .map((column) => column.name) ?? []
  );
}

/** Referenced columns, resolving an implicit target to its primary key. */
function referenced(
  diagram: SchemaDiagramDto,
  key: SchemaDiagramDto['foreignKeys'][number],
): string[] {
  if (key.targetColumns.length === key.columns.length) return key.targetColumns;
  return primaryKey(diagram, key.targetTable);
}

/** Outgoing and incoming foreign keys of `table` from the schema's ER data. */
export function tableLinks(
  diagram: SchemaDiagramDto,
  schema: string,
  table: string,
): TableLinks {
  const outgoing: KeyLink[] = [];
  const incoming: KeyLink[] = [];
  for (const key of diagram.foreignKeys) {
    const target = referenced(diagram, key);
    if (target.length !== key.columns.length) continue;
    if (key.table === table)
      outgoing.push({
        name: key.name,
        schema,
        table: key.targetTable,
        columns: key.columns,
        targetColumns: target,
      });
    if (key.targetTable === table)
      incoming.push({
        name: key.name,
        schema,
        table: key.table,
        columns: target,
        targetColumns: key.columns,
      });
  }
  return { outgoing, incoming };
}

/** Columns of this table that hold an outgoing key (they show a jump arrow). */
export function linkedColumns(links: TableLinks): ReadonlySet<string> {
  return new Set(links.outgoing.flatMap((link) => link.columns));
}

/** The first outgoing key that uses `column`. */
export function linkOf(links: TableLinks, column: string): KeyLink | undefined {
  return links.outgoing.find((link) => link.columns.includes(column));
}

/**
 * Equality filters on the other table that select the rows `link` pairs
 * with `values` (this table's fields by name); `null` when a key column is
 * NULL or missing, so there is nothing to open.
 */
export function jumpFilters(
  link: KeyLink,
  values: ReadonlyMap<string, KeyValue>,
): FilterDto[] | null {
  const filters: FilterDto[] = [];
  for (const [index, column] of link.columns.entries()) {
    const value = values.get(column);
    const target = link.targetColumns[index];
    if (value === undefined || value === null || target === undefined)
      return null;
    filters.push({ column: target, operator: 'equal', value, second: '' });
  }
  return filters;
}

/** Menu label of a key: the other table and its key columns, `orders (customer_id)`. */
export function linkLabel(link: KeyLink): string {
  return `${link.sourceSchema && link.sourceSchema !== link.schema ? `${link.schema}.` : ''}${link.table} (${link.targetColumns.join(', ')})`;
}

/** Outgoing keys from table detail retain referenced schemas and all composite columns. */
export function detailLinks(
  detail: TableDetailDto,
  fallback: TableLinks,
  schema: string,
): TableLinks {
  const outgoing = detail.constraints.flatMap((constraint): KeyLink[] => {
    const target = constraint.references;
    if (constraint.kind !== 'foreign' || !target) return [];
    const columns =
      target.columns.length === constraint.columns.length
        ? target.columns
        : fallback.outgoing.find((link) => link.name === constraint.name)
            ?.targetColumns;
    if (!columns || columns.length !== constraint.columns.length) return [];
    return [
      {
        name: constraint.name,
        schema: target.schema || schema,
        ...(target.schema && target.schema !== schema
          ? { sourceSchema: schema }
          : {}),
        table: target.table,
        columns: constraint.columns,
        targetColumns: columns,
      },
    ];
  });
  return { outgoing, incoming: fallback.incoming };
}
