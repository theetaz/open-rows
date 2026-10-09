// SPDX-License-Identifier: Apache-2.0
import type { QueryTab } from '../query/tabs';
import { toggleFavourite } from './sidebarStore';

/** Palette/shortcut target is the active object tab, including routine overloads. */
export function toggleActiveFavourite(tab: QueryTab | undefined): void {
  if (!tab) return;
  const scope = { profileId: tab.connectionId, database: tab.database };
  if (tab.table)
    toggleFavourite({
      ...scope,
      ...tab.table,
      kind: tab.table.kind ?? 'table',
      arguments: null,
    });
  else if (tab.inspection?.kind === 'object')
    toggleFavourite({ ...scope, ...tab.inspection.object });
}
