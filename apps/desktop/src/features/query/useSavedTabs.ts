// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { loadWorkspaceState } from '../settings/workspaceState';
import { generalSettings } from '../settings/general';
import { restoreTabs } from './tabStorage';
import { emptyTabs } from './tabs';
import type { TabsState } from './tabs';
/** Hydrate disconnected descriptors; keep the launch snapshot for explicit reopening. */
export function useSavedTabs(
  setState: (change: (current: TabsState) => TabsState) => void,
): boolean {
  const [loaded, setLoaded] = useState(false);
  const previous = useRef(emptyTabs);
  useEffect(() => {
    let live = true;
    loadWorkspaceState('queryTabs').then(
      (text) => {
        if (!live) return;
        previous.current = restoreTabs(text);
        if (generalSettings.get().restoreSession)
          setState((current) =>
            current === emptyTabs ? previous.current : current,
          );
        setLoaded(true);
      },
      () => {
        if (live) setLoaded(true);
      },
    );
    const reopen = (): void => {
      setState((current) => mergeSessionTabs(current, previous.current));
    };
    window.addEventListener('openrows:reopen-session', reopen);
    return () => {
      live = false;
      window.removeEventListener('openrows:reopen-session', reopen);
    };
  }, [setState]);
  return loaded;
}
/** Reopen launch descriptors without replacing SQL typed into current tabs. */
export function mergeSessionTabs(
  current: TabsState,
  saved: TabsState,
): TabsState {
  let next = current.next;
  const remap = new Map<string, string>();
  const added = saved.tabs.flatMap((tab) => {
    const same = current.tabs.find(
      (item) =>
        item.connectionId === tab.connectionId &&
        item.database === tab.database &&
        item.kind === tab.kind &&
        item.title === tab.title &&
        item.sql === tab.sql &&
        item.file?.path === tab.file?.path &&
        JSON.stringify(item.table) === JSON.stringify(tab.table) &&
        JSON.stringify(item.inspection) === JSON.stringify(tab.inspection),
    );
    const id = same?.id ?? `${tab.kind}-${String(next++)}`;
    remap.set(tab.id, id);
    return same ? [] : [{ ...tab, id }];
  });
  const active = Object.fromEntries(
    Object.entries(saved.active).map(([key, id]) => [
      key,
      id ? remap.get(id) : undefined,
    ]),
  );
  const splits = Object.fromEntries(
    Object.entries(saved.splits ?? {}).map(([key, split]) => [
      key,
      { ...split, pinned: remap.get(split.pinned) ?? split.pinned },
    ]),
  );
  return {
    tabs: [...current.tabs, ...added],
    active: { ...current.active, ...active },
    next,
    splits: { ...current.splits, ...splits },
  };
}
