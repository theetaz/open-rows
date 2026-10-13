// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { GridColumn, GridHandle } from '@openrows/grid';
import type { ColumnDto, SortDto } from '@openrows/ipc';
import { columnAlign } from '../../results/align';
import { pinnedCount, widthOf } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';
import type { ResultMeta } from '../query/run';
import type { EditedSource } from './EditedSource';
import { storeColumns } from './row';
import { resultColumnKey } from '../../results/projectedResult';

/** The server-side sort direction of a column, painted as a header triangle. */
function headerSort(
  name: string,
  sort: SortDto | null,
): Pick<GridColumn, 'sort'> {
  if (sort?.column !== name) return {};
  return { sort: sort.descending ? 'descending' : 'ascending' };
}

const noLinks: ReadonlySet<string> = new Set();

/** Grid columns of a page: titles with sort, layout widths and pins, key arrows. */
function columnsOf(
  columns: readonly ColumnDto[],
  sort: SortDto | null,
  layout: ColumnLayout,
  links: ReadonlySet<string> = noLinks,
): GridColumn[] {
  const pins = pinnedCount(
    layout,
    columns.map((column) => column.name),
  );
  return columns.map((column, index) => ({
    id: String(index),
    title: column.name,
    ...headerSort(column.name, sort),
    width: widthOf(
      layout,
      column.name,
      Math.min(
        320,
        Math.max(columnWidth(column.sourceType), column.name.length * 8 + 56),
      ),
    ),
    pinned: index < pins,
    align: columnAlign(column.sourceType),
    ...(column.sourceType === 'bool' ? { kind: 'boolean' as const } : {}),
    ...(links.has(column.name) ? { link: true } : {}),
  }));
}

function columnWidth(type: string | null): number {
  if (type && /text|char|json|uuid/.test(type)) return 180;
  if (type && /date|time|numeric|decimal/.test(type)) return 168;
  return 96;
}

/** Use stable layout identities while painting the original database labels. */
export function presentationColumns(
  result: ResultMeta,
  sort: SortDto | null,
  layout: ColumnLayout,
  links: ReadonlySet<string> = noLinks,
): GridColumn[] {
  const stored = storeColumns(result);
  return columnsOf(
    stored.map((column, index) => ({
      ...column,
      name: resultColumnKey(result, index),
    })),
    sort,
    layout,
    links,
  ).map((column, index) => ({ ...column, title: stored[index]?.name ?? '' }));
}

/**
 * Apply a changed layout (a pin that needs no reorder, a reset, a stored
 * width) or newly read key arrows to the mounted columns in place; reorders
 * reload the page instead.
 */
export function useLayoutSync({
  result,
  sort,
  layout,
  links,
  sourceRef,
  handle,
  fitted,
}: {
  result: ResultMeta;
  sort: SortDto | null;
  layout: ColumnLayout;
  /** Columns holding a foreign key (they show a jump arrow). */
  links?: ReadonlySet<string> | undefined;
  sourceRef: React.RefObject<EditedSource | undefined>;
  handle: React.RefObject<GridHandle | undefined>;
  /** Auto-fitted widths, kept when no width is stored. */
  fitted?: React.RefObject<Map<string, number>> | undefined;
}): void {
  useEffect(() => {
    const columns = sourceRef.current?.columns;
    if (!columns) return;
    const fresh = presentationColumns(result, sort, layout, links ?? noLinks);
    columns.forEach((column, index) => {
      const next = fresh[index];
      if (!next) return;
      const name = resultColumnKey(result, index);
      column.width =
        layout.widths[name] ?? fitted?.current.get(name) ?? next.width;
      column.pinned = next.pinned;
      column.link = next.link ?? false;
    });
    handle.current?.relayout?.();
  }, [result, sort, layout, links, sourceRef, handle, fitted]);
}
