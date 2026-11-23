// SPDX-License-Identifier: Apache-2.0
import type { QueryTab } from '../query/tabs';
import type { SidebarObject } from '../schema/sidebarIdentity';

/** The catalog object a table or inspection tab shows, for the context bar's star. */
export function tabFavourite(
  tab: QueryTab | undefined,
  profileId: string | undefined,
): SidebarObject | undefined {
  if (!tab || !profileId) return undefined;
  if (tab.kind === 'table' && tab.table)
    return {
      ...tab.table,
      profileId,
      kind: tab.table.kind ?? 'table',
      arguments: null,
    };
  const target = tab.kind === 'inspection' ? tab.inspection : undefined;
  return target?.kind === 'object'
    ? { ...target.object, profileId, database: tab.database }
    : undefined;
}
