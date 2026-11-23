// SPDX-License-Identifier: Apache-2.0
import { createContext, useContext, useEffect } from 'react';
import type { EnvDto } from '@openrows/ipc';
import { WindowTabStrip } from './WindowTabStrip';
import type { MoveTab } from './TabMenu';
import type { QueryTab as Tab } from './tabs';
import type { QueryTabs } from './useQueryTabs';

/** What a pane hands to the window's tab strip. */
export type StripPane = {
  /** The pane's tab set: choosing a tab of its scope fills the focused split pane. */
  tabs: QueryTabs;
  /** Move a tab to another window. */
  onMove?: MoveTab | undefined;
};

/** Holds the active pane's strip actions for the window's strip. */
export type StripSlot = {
  set: (pane: StripPane | null) => void;
  get: () => StripPane | null;
};

/** An empty slot; the workspace keeps one per window. */
export function stripSlot(): StripSlot {
  let pane: StripPane | null = null;
  return {
    set: (next) => {
      pane = next;
    },
    get: () => pane,
  };
}

/**
 * Where a pane's strip goes: the workspace draws one strip for the whole
 * window above its toolbar, and panes hand it their actions.
 */
export const TabStripHost = createContext<StripSlot | null>(null);

/**
 * A pane's document tabs. Inside the workspace they join the window's strip
 * above the toolbar (this renders nothing); on its own (local `.sql` files,
 * tests) the pane draws its own strip.
 */
export function TabStrip(props: {
  tabs: QueryTabs;
  list: readonly Tab[];
  active: string;
  /** The tabs' owner, named in each tab's tooltip with its environment. */
  connection: { name: string; env?: EnvDto };
  onAdd: () => void;
  /** Move a tab to another window (the tab menu's Move items). */
  onMove?: MoveTab | undefined;
  /** Draw the strip here even inside the workspace. */
  inline?: boolean;
}): React.JSX.Element | null {
  const host = useContext(TabStripHost);
  const joined = Boolean(host) && !props.inline;
  const { tabs, onMove } = props;
  useEffect(() => {
    if (!joined || !host) return;
    host.set({ tabs, onMove });
    return () => {
      host.set(null);
    };
  }, [joined, host, tabs, onMove]);
  if (joined) return null;
  const { name, env } = props.connection;
  return (
    <WindowTabStrip
      tabs={tabs}
      list={props.list}
      active={props.active}
      profiles={[]}
      owner={env ? `${name} (${env.label ?? env.kind})` : name}
      onSelect={tabs.select}
      onAdd={props.onAdd}
      onMove={onMove}
    />
  );
}
