// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { Connections } from '../connections';
import type { QueryTabs, TabScope } from '../query';
import { WINDOW_SCOPE } from '../query/tabs';
import type { ConnectionFlowTarget, QueryTab } from '../query/tabs';
import { duplicateTab, openBeside, otherTabs } from '../query/tabOps';
import { stripSlot } from '../query/TabStrip';
import type { StripPane, StripSlot } from '../query/TabStrip';
import { changeGroup } from '../query/tabGroups';
import {
  outsideScope,
  shownTab,
  stepFrom,
  stripTabs,
  tabAtKey,
} from './windowTabs';
import { t } from './navMessages';

/** The window's tabs across connections, and the commands that walk them. */
export type WindowTabs = {
  /** The tab the window shows. */
  shown: QueryTab | undefined;
  /** Every tab of the strip, in order. */
  list: QueryTab[];
  /** Focus a tab, switching the workspace to its connection and database. */
  select: (id: string) => void;
  /** A new query tab right after `id`. */
  addRight: (id: string) => void;
  /** Open (or refocus) the connection flow as a tab. */
  openFlow: (flow: ConnectionFlowTarget) => void;
  /** The flow saved `profileId`: close its tab and open the connection. */
  finishFlow: (id: string, profileId: string) => void;
  /** Run a tab or history command; `false` when `id` is not one. */
  command: (id: string) => boolean;
  /** Where the active pane hands its strip actions (split panes, moves). */
  host: StripSlot;
};

type Input = {
  connections: Connections;
  tabs: QueryTabs;
  scope: TabScope | undefined;
  /** ⌘L: open the path bar's last segment. */
  onLocation: () => void;
};

/** Focus `tab`; a tab of another connection or database switches the workspace. */
function focus(input: Input, host: StripPane | null, tab: QueryTab): void {
  const { tabs, connections, scope } = input;
  const owner = tab.group ? tabs.state.groups?.[tab.group] : undefined;
  if (owner?.collapsed)
    tabs.apply?.((state) => changeGroup(state, owner.id, { collapsed: false }));
  if (!outsideScope(tab, scope) && host && tab.kind !== 'connection') {
    host.tabs.select(tab.id);
    return;
  }
  tabs.select(tab.id);
  const known = connections.profiles.some(
    (item) => item.id === tab.connectionId,
  );
  if (outsideScope(tab, scope) && known)
    connections.select(tab.connectionId, tab.database);
}

/** The tab and history commands, by id. */
function commands(
  input: Input,
  shown: QueryTab | undefined,
  go: (tab: QueryTab | undefined) => void,
): Readonly<Record<string, () => void>> {
  const { tabs } = input;
  const state = tabs.state;
  const id = shown?.id;
  return {
    'next-tab': () => {
      go(stepFrom(state, id, 1));
    },
    'previous-tab': () => {
      go(stepFrom(state, id, -1));
    },
    'go-back': () => id && tabs.navigate?.(id, 'back'),
    'go-forward': () => id && tabs.navigate?.(id, 'forward'),
    'open-location': input.onLocation,
    'duplicate-tab': () => {
      if (!shown || shown.kind === 'connection') return;
      const copy = `${shown.kind}-${String(state.next)}`;
      tabs.apply?.((current) => duplicateTab(current, shown.id));
      tabs.select(copy);
    },
    'pin-tab': () => {
      if (shown) tabs.pin(shown.id, !shown.pinned);
    },
    'close-other-tabs': () => {
      if (!id) return;
      const ids = otherTabs(state, id);
      if (tabs.closeMany) tabs.closeMany(ids);
      else
        ids.forEach((other) => {
          tabs.close(other);
        });
    },
  };
}

/**
 * One strip for the whole window (DESIGN.md §7): tabs of every connection
 * and database, the connection flow as a tab, ⌘1–9, ⌃Tab, ⌘[ and ⌘].
 */
export function useWindowTabs(input: Input): WindowTabs {
  const [host] = useState(stripSlot);
  const { tabs, connections } = input;
  const shown = shownTab(tabs.state, input.scope);
  const select = (id: string): void => {
    const tab = tabs.state.tabs.find((item) => item.id === id);
    if (tab) focus(input, host.get(), tab);
  };
  const go = (tab: QueryTab | undefined): void => {
    if (tab) select(tab.id);
  };
  return {
    shown,
    list: stripTabs(tabs.state),
    select,
    addRight: (id) => {
      const tab = tabs.state.tabs.find((item) => item.id === id);
      if (!tab || tab.kind === 'connection') return;
      tabs.apply?.((state) => openBeside(state, id));
      const known = connections.profiles.some(
        (item) => item.id === tab.connectionId,
      );
      if (outsideScope(tab, input.scope) && known)
        connections.select(tab.connectionId, tab.database);
    },
    openFlow: (flow) => {
      const profile = connections.profiles.find(
        (item) => item.id === flow.profileId,
      );
      const title =
        flow.mode === 'edit' && profile
          ? t('editNamed', { name: profile.name })
          : t('newConnectionTab');
      tabs.open(WINDOW_SCOPE, { flow, title });
    },
    finishFlow: (id, profileId) => {
      tabs.close(id, { discard: true });
      connections.reload();
      connections.select(profileId);
    },
    command: (id) => {
      const key = /^select-tab-(\d)$/.exec(id)?.[1];
      if (key) {
        go(tabAtKey(tabs.state, Number(key)));
        return true;
      }
      if (id === 'close-tab' && shown?.kind === 'connection') {
        tabs.close(shown.id);
        return true;
      }
      const run = commands(input, shown, go)[id];
      run?.();
      return Boolean(run);
    },
    host,
  };
}
