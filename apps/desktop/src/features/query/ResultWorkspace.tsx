// SPDX-License-Identifier: Apache-2.0
import { createContext, useContext } from 'react';
import type { TableViews } from '../table/useTableViews';
import type { ShowRow } from '../table/row';

/** Workspace lifetime and inspector/navigation context of SQL result edits. */
export type ResultWorkspace = {
  tabId: string;
  profileId: string;
  database: string;
  sessionId: string | null;
  views: TableViews;
  onRow: ShowRow;
  onOpenTable: (schema: string, table: string) => void;
};
const context = createContext<ResultWorkspace | null>(null);
export const ResultWorkspaceProvider = context.Provider;
/** Current query tab, absent in standalone read-only result previews. */
export function useResultWorkspace(): ResultWorkspace | null {
  return useContext(context);
}

/** Rerunning must not discard a result's staged or in-flight work. */
export function useResultEditNotice(tabId: string): string | null {
  const edits = useResultWorkspace()?.views.entryOf(tabId).edits;
  return edits?.current.length || edits?.pending || edits?.uncertain
    ? 'Commit or discard the staged result changes before running SQL again.'
    : null;
}
