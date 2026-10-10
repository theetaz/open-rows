// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';
import { useMemo } from 'react';
import { tableLayoutKey } from '../../../results/columnLayoutStore';
import type { ResultMeta } from '../../query/run';
import type { TableTargetRef } from '../model';
import { readRow, shownFields } from '../row';
import type { RowField } from '../row';
import { jumpFilters, linkedColumns, linkOf } from './foreignKeys';
import type { KeyLink, KeyValue, TableLinks } from './foreignKeys';
import { seedFilters } from './seeds';
import { useTableLinks } from './useTableLinks';

/** Foreign-key navigation of a table tab (FR-GRID-07). */
export type KeyJumps = {
  links: TableLinks;
  /** Columns whose cells show a jump arrow. */
  columns: ReadonlySet<string>;
  /** The arrow of a key cell: open the row it references. */
  openCell: (result: ResultMeta, cell: { row: number; column: number }) => void;
  /** Open the rows of `link.table` that pair with page row `row`. */
  open: (result: ResultMeta, row: number, link: KeyLink) => void;
};

type Args = {
  target: TableTargetRef;
  sessionId: string | null;
  engine: EngineDto;
  /** Open (or focus) another table's tab in this database. */
  onOpenTable: ((schema: string, table: string) => void) | undefined;
  /** Report why a jump is impossible (a NULL key, an unreadable row). */
  onError: (error: Error) => void;
};

/** A row's field values by name, as a jump compares them. */
function valuesOf(fields: readonly RowField[]): Map<string, KeyValue> {
  return new Map(
    fields.map((field) => [
      field.name,
      field.state === 'null' ? null : field.state === 'empty' ? '' : field.text,
    ]),
  );
}

/**
 * Read a row's key fields and open the other table filtered to the
 * matching rows, one parameterised equality filter per key column.
 */
export function useKeyJumps(args: Args): KeyJumps {
  const links = useTableLinks(args.target, args.sessionId, args.engine);
  const columns = useMemo(() => linkedColumns(links), [links]);
  const open = (result: ResultMeta, row: number, link: KeyLink): void => {
    readRow(result, row).then(
      (fields) => {
        const filters = jumpFilters(link, valuesOf(shownFields(fields)));
        if (!filters) {
          args.onError(new Error('The key is NULL, so no row is referenced.'));
          return;
        }
        seedFilters(
          tableLayoutKey({
            ...args.target,
            schema: link.schema,
            table: link.table,
          }),
          filters,
        );
        args.onOpenTable?.(link.schema, link.table);
      },
      (error: unknown) => {
        args.onError(error instanceof Error ? error : new Error(String(error)));
      },
    );
  };
  return {
    links,
    columns,
    open,
    openCell: (result, cell) => {
      const name = result.columns[cell.column]?.name;
      const link = name === undefined ? undefined : linkOf(links, name);
      if (link) open(result, cell.row, link);
    },
  };
}
