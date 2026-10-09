// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  bindFile,
  closeTab,
  editTab,
  emptyTabs,
  openTab,
  renameTableTab,
  scopeKey,
  selectTab,
  setTabParams,
  tabsOf,
} from './tabs';
import type { ParamValues } from './paramValues';
import type {
  QueryTab,
  QueryTabInit,
  TabFile,
  TabScope,
  TabsState,
} from './tabs';
import { keepTab, moveTab, pinTab, renameTab } from './tabOps';
import { useSavedTabs } from './useSavedTabs';
import { useEditorSettings } from './editorSettings';
import { generalSettings } from '../settings/general';
import { useTabPersistence } from './useTabPersistence';
import { useFileChanges } from './useFileChanges';

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
  /** Bind a query tab to a `.sql` file, or record a save of it. */
  bindFile: (id: string, file: TabFile) => void;
  /** Remember a query tab's last parameter values. */
  setParams: (id: string, params: ParamValues) => void;
  /** Move a tab to a position among its scope's tabs. */
  move: (id: string, index: number) => void;
  pin: (id: string, pinned: boolean) => void;
  /** Turn a preview tab into a normal tab. */
  keep: (id: string) => void;
  /** Give a tab a new title. */
  rename: (id: string, title: string) => void;
  /** The whole tab set, for tab menus and session restore. */
  state: TabsState;
  /** Persist the per-scope split layouts with tab descriptors. */
  setSplits?: (splits: NonNullable<TabsState['splits']>) => void;
};

/**
 * Query tabs of every connection and database; descriptors only, never result
 * rows. With session restore on, tabs reopen on relaunch (query text with
 * Auto Save).
 */
export function useQueryTabs(): QueryTabs {
  const guard = useRef<((id: string) => boolean) | null>(null);
  const installCloseGuard = useCallback((next: (id: string) => boolean) => {
    guard.current = next;
    return () => {
      if (guard.current === next) guard.current = null;
    };
  }, []);
  const [state, setState] = useState<TabsState>(emptyTabs);
  const loaded = useSavedTabs(setState);
  const [{ autoSave }] = useEditorSettings();
  const { restoreSession } = generalSettings.use();
  const file = useCallback((id: string, next: TabFile) => {
    setState((current) => bindFile(current, id, next));
  }, []);
  const persistence = { autoSave, restore: restoreSession, loaded };
  useTabPersistence(state, persistence, file);
  useFileChanges(state.tabs, loaded, file);
  const latest = useRef(state);
  useEffect(() => {
    latest.current = state;
  }, [state]);
  const open = useCallback<OpenQueryTab>((scope, init) => {
    const preview = init?.preview
      ? tabsOf(latest.current, scope).find((tab) => tab.preview)
      : undefined;
    // A preview tab being replaced is closed first, releasing its views.
    if (preview && guard.current && !guard.current(preview.id)) {
      setState((current) =>
        openTab(current, scope, { ...init, preview: false }),
      );
      return;
    }
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
    bindFile: file,
    setParams: (id, params) => {
      setState((current) => setTabParams(current, id, params));
    },
    move: (id, index) => {
      setState((current) => moveTab(current, id, index));
    },
    pin: (id, pinned) => {
      setState((current) => pinTab(current, id, pinned));
    },
    keep: (id) => {
      setState((current) => keepTab(current, id));
    },
    rename: (id, title) => {
      setState((current) => renameTab(current, id, title));
    },
    setSplits: (splits) => {
      setState((current) => ({ ...current, splits }));
    },
    state,
  };
}
