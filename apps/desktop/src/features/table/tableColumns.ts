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

/** Header title with the server-side sort direction, e.g. `name ▲`. */
function headerTitle(name: string, sort: SortDto | null): string {
  if (sort?.column !== name) return name;
  return `${name} ${sort.descending ? '▼' : '▲'}`;
}

const noLinks: ReadonlySet<string> = new Set();

/** Grid columns of a page: titles with sort, layout widths and pins, key arrows. */
export function columnsOf(
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
    title: headerTitle(column.name, sort),
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
}: {
  result: ResultMeta;
  sort: SortDto | null;
  layout: ColumnLayout;
  /** Columns holding a foreign key (they show a jump arrow). */
  links?: ReadonlySet<string> | undefined;
  sourceRef: React.RefObject<EditedSource | undefined>;
  handle: React.RefObject<GridHandle | undefined>;
}): void {
  useEffect(() => {
    const columns = sourceRef.current?.columns;
    if (!columns) return;
    const fresh = columnsOf(
      storeColumns(result),
      sort,
      layout,
      links ?? noLinks,
    );
    columns.forEach((column, index) => {
      const next = fresh[index];
      if (!next) return;
      column.width = next.width;
      column.pinned = next.pinned;
      column.link = next.link ?? false;
    });
    handle.current?.relayout?.();
  }, [result, sort, layout, links, sourceRef, handle]);
}
