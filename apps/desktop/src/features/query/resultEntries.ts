// SPDX-License-Identifier: Apache-2.0
import { emptyEntry } from '../table/entries';
import type { TableEntry } from '../table/entries';
type Entries = Readonly<Record<string, TableEntry | undefined>>;

/** A result owns independent edits, while its query tab owns close/quit protection. */
export function resultEntryIds(entries: Entries, tabId: string): string[] {
  return [
    tabId,
    ...Object.keys(entries).filter((id) => id.startsWith(`${tabId}:result:`)),
  ];
}

/** Aggregate child result edits only for the parent tab's existing safety guards. */
export function resultEntry(
  entries: Entries,
  tabId: string,
): TableEntry | undefined {
  const ids = resultEntryIds(entries, tabId);
  if (ids.length === 1) return entries[tabId];
  const entry = entries[tabId] ?? emptyEntry;
  const edits = ids.flatMap((id) => (entries[id] ? [entries[id].edits] : []));
  return {
    ...entry,
    edits: {
      ...entry.edits,
      current: edits.flatMap((history) => history.current),
      pending: edits.some((history) => history.pending),
      uncertain: edits.some((history) => history.uncertain),
    },
  };
}
