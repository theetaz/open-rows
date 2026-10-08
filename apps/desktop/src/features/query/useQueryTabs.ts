// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import {
  closeTab,
  editTab,
  emptyTabs,
  openTab,
  renameTableTab,
  scopeKey,
  selectTab,
  tabsOf,
} from './tabs';
import type { QueryTab, QueryTabInit, TabScope, TabsState } from './tabs';

/**
 * Open a query tab in a connection and database scope, e.g. from the schema
 * tree: `open(scope, { title: 'orders', objectPath: ['public', 'orders'], sql })`.
 */
type OpenQueryTab = (scope: TabScope, init?: QueryTabInit) => void;

export type QueryTabs = {
  tabsFor: (scope: TabScope) => readonly QueryTab[];
  activeFor: (scope: TabScope) => QueryTab | undefined;
  open: OpenQueryTab;
  close: (id: string, options?: { discard: boolean }) => void;
  installCloseGuard?: (guard: (id: string) => boolean) => () => void;
  select: (id: string) => void;
  edit: (id: string, sql: string) => void;
  /** Point a table tab at its table's new name after a committed rename. */
  renameTable?: (id: string, name: string) => void;
};

/** Query tabs of every connection and database; descriptors only, never result rows. */
export function useQueryTabs(): QueryTabs {
  const guard = useRef<((id: string) => boolean) | null>(null);
  const installCloseGuard = useCallback((next: (id: string) => boolean) => {
    guard.current = next;
    return () => {
      if (guard.current === next) guard.current = null;
    };
  }, []);
  const [state, setState] = useState<TabsState>(emptyTabs);
  const open = useCallback<OpenQueryTab>((scope, init) => {
    setState((current) => openTab(current, scope, init));
  }, []);
  const edit = useCallback((id: string, sql: string) => {
    setState((current) => editTab(current, id, sql));
  }, []);
  return {
    tabsFor: (scope) => tabsOf(state, scope),
    activeFor: (scope) => {
      const id = state.active[scopeKey(scope)];
      return state.tabs.find((tab) => tab.id === id);
    },
    open,
    installCloseGuard,
    close: (id, options) => {
      if (!options?.discard && guard.current && !guard.current(id)) return;
      setState((current) => closeTab(current, id));
    },
    select: (id) => {
      setState((current) => selectTab(current, id));
    },
    edit,
    renameTable: (id, name) => {
      setState((current) => renameTableTab(current, id, name));
    },
  };
}
