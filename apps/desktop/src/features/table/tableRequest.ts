// SPDX-License-Identifier: Apache-2.0
import type { TableQueryDto } from '@openrows/ipc';
import { arrange } from '../../results/columnLayout';
import { tableSettings } from '../settings/tableSettings';
import type { TableEntry } from './entries';
import { arrangedQuery, countKey, tableQuery } from './model';
import type { TableTargetRef, TableView } from './model';
import type { TableLayout } from './useTableLayout';
import type { TableViews } from './useTableViews';
import { useTrigger } from './useTrigger';

/** The page request: filters, any-column search and the arranged column order. */
export function tableRequest(
  target: TableTargetRef,
  view: TableView,
  columns: readonly string[],
  extra: { rowId: string | null; layout: TableLayout['layout'] },
): TableQueryDto {
  const query = tableQuery(target, view, columns, extra.rowId);
  return arrangedQuery(
    query,
    columns,
    arrange(extra.layout, columns),
    view.hidden,
  );
}

/**
 * With Settings ▸ Tables ▸ Row count set to exact, count once per filter
 * set after the structure loads; Counting… Cancel stops it as usual.
 */
export function useExactCount(
  target: {
    tabId: string;
    views: TableViews;
    sessionId: string | null;
    query: TableQueryDto;
  },
  structure: TableEntry['structure']['status'],
): void {
  const { rowCount } = tableSettings.useValue();
  const { tabId, views, sessionId, query } = target;
  const key = countKey(query);
  const wanted = rowCount === 'exact' && sessionId && structure === 'ready';
  useTrigger(
    wanted
      ? `${sessionId}\u0000${key}\u0000${String(target.views.entryOf(tabId).view.revision)}`
      : null,
    () => {
      if (sessionId) views.count(tabId, key, { sessionId, query });
    },
  );
}
