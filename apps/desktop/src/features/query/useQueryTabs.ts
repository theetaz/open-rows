// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import {
  closeTab,
  editTab,
  emptyTabs,
  openTab,
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
  close: (id: string) => void;
  select: (id: string) => void;
  edit: (id: string, sql: string) => void;
};

/** Query tabs of every connection and database; descriptors only, never result rows. */
export function useQueryTabs(): QueryTabs {
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
    close: (id) => {
      setState((current) => closeTab(current, id));
    },
    select: (id) => {
      setState((current) => selectTab(current, id));
    },
    edit,
  };
}
