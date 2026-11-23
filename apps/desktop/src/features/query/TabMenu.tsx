// SPDX-License-Identifier: Apache-2.0
import { Icon, Menu } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';
import type { WindowEntryDto } from '@openrows/ipc';
import { useOtherWindows, windowName } from '../windows/useOtherWindows';
import { addToGroup, liveGroups, removeFromGroup } from './tabGroups';
import { duplicateTab, moveToEdge, otherTabs, tabsRightOf } from './tabOps';
import type { QueryTab } from './tabs';
import type { QueryTabs } from './useQueryTabs';
import { t } from './tabMessages';

/** Move a tab to window `target`, or to a new window when `null`. */
export type MoveTab = (id: string, target: string | null) => void;

/** Where the tab menu opened, and for which tab. */
export type TabMenuTarget = { id: string; x: number; y: number };

/** What the menu asks of the strip beyond the tab set itself. */
export type TabMenuActions = {
  /** A new tab right after this one, in its group. */
  newRight: (id: string) => void;
  /** Put the tab in a new group and open the group's editor. */
  newGroup: (id: string) => void;
  rename: (id: string) => void;
  reload: (id: string) => void;
  copyPath: (id: string) => void;
  /** Move the tab to another window; no window items without it. */
  move?: MoveTab | undefined;
};

const item = (
  label: string,
  run: () => void,
  extra: Partial<Extract<MenuEntry, { kind: 'item' }>> = {},
): MenuEntry => ({ kind: 'item', label, run, ...extra });
const separator: MenuEntry = { kind: 'separator' };

/** Add Tab to Group ▸: New Group, then every group with its colour. */
function groupEntries(
  tabs: QueryTabs,
  tab: QueryTab,
  actions: TabMenuActions,
): MenuEntry[] {
  const groups = liveGroups(tabs.state).map((group) =>
    item(
      group.name || t('unnamedGroup'),
      () => tabs.apply?.((state) => addToGroup(state, tab.id, group.id)),
      {
        swatch: `var(--or-group-${group.colour})`,
        disabled: tab.group === group.id,
      },
    ),
  );
  return [
    item(
      t('newGroup'),
      () => {
        actions.newGroup(tab.id);
      },
      { icon: <Icon name="plus" /> },
    ),
    ...(groups.length ? [separator, ...groups] : []),
  ];
}

/** Move Tab ▸: to the start or end of the strip, or to another window. */
function moveEntries(
  tabs: QueryTabs,
  tab: QueryTab,
  actions: TabMenuActions,
  windows: readonly WindowEntryDto[],
): MenuEntry[] {
  const index = tabs.state.tabs.findIndex((item) => item.id === tab.id);
  const edge = (where: 'start' | 'end') => () =>
    tabs.apply?.((state) => moveToEdge(state, tab.id, where));
  const { move } = actions;
  const windowEntries: MenuEntry[] = move
    ? [
        separator,
        item(t('moveNewWindow'), () => {
          move(tab.id, null);
        }),
        ...windows.map((entry) =>
          item(t('moveEntry', { entry: windowName(entry) }), () => {
            move(tab.id, entry.label);
          }),
        ),
      ]
    : [];
  return [
    item(t('toStart'), edge('start'), { disabled: index === 0 }),
    item(t('toEnd'), edge('end'), {
      disabled: index === tabs.state.tabs.length - 1,
    }),
    ...windowEntries,
  ];
}

/** Close, Close Others and Close to the Right; pinned tabs stay. */
function closeEntries(tabs: QueryTabs, id: string): MenuEntry[] {
  const closeAll = (ids: readonly string[]): void => {
    if (tabs.closeMany) tabs.closeMany(ids);
    else
      ids.forEach((other) => {
        tabs.close(other);
      });
  };
  const others = otherTabs(tabs.state, id);
  const right = tabsRightOf(tabs.state, id);
  return [
    item(
      t('closeTab'),
      () => {
        tabs.close(id);
      },
      {
        icon: <Icon name="close" />,
        shortcut: '⌘W',
      },
    ),
    item(
      t('closeOtherTabs'),
      () => {
        closeAll(others);
      },
      {
        disabled: !others.length,
        shortcut: '⌥⌘W',
      },
    ),
    item(
      t('closeTabsRight'),
      () => {
        closeAll(right);
      },
      {
        disabled: !right.length,
      },
    ),
  ];
}

/** New Tab to the Right, Duplicate, Pin, Rename and Keep Open. */
function tabEntries(
  tabs: QueryTabs,
  tab: QueryTab,
  actions: TabMenuActions,
): MenuEntry[] {
  const flow = tab.kind === 'connection';
  return [
    item(
      t('newTabRight'),
      () => {
        actions.newRight(tab.id);
      },
      { icon: <Icon name="plus" />, disabled: flow },
    ),
    item(t('duplicate'), () => tabs.apply?.((s) => duplicateTab(s, tab.id)), {
      icon: <Icon name="copy" />,
      shortcut: '⇧⌘K',
      disabled: flow,
    }),
    item(
      tab.pinned ? t('unpinTab') : t('pinTab'),
      () => {
        tabs.pin(tab.id, !tab.pinned);
      },
      { icon: <Icon name="pin" /> },
    ),
    item(t('renameTab'), () => {
      actions.rename(tab.id);
    }),
    item(
      t('keepOpen'),
      () => {
        tabs.keep(tab.id);
      },
      { disabled: !tab.preview },
    ),
  ];
}

/** Add Tab to Group ▸, Remove from Group and Move Tab ▸. */
function placeEntries(
  tabs: QueryTabs,
  tab: QueryTab,
  actions: TabMenuActions,
  windows: readonly WindowEntryDto[],
): MenuEntry[] {
  return [
    {
      kind: 'submenu',
      label: t('addToGroup'),
      icon: <Icon name="list" />,
      disabled: Boolean(tab.pinned),
      entries: groupEntries(tabs, tab, actions),
    },
    item(
      t('removeFromGroup'),
      () => tabs.apply?.((state) => removeFromGroup(state, tab.id)),
      { disabled: !tab.group },
    ),
    {
      kind: 'submenu',
      label: t('moveTab'),
      icon: <Icon name="chevronUpDown" />,
      entries: moveEntries(tabs, tab, actions, windows),
    },
  ];
}

/** Reload and Copy Path. */
function dataEntries(tab: QueryTab, actions: TabMenuActions): MenuEntry[] {
  return [
    item(
      t('reload'),
      () => {
        actions.reload(tab.id);
      },
      {
        icon: <Icon name="refresh" />,
        shortcut: '⌘R',
        disabled: tab.kind !== 'table' && tab.kind !== 'query',
      },
    ),
    item(
      t('copyPath'),
      () => {
        actions.copyPath(tab.id);
      },
      { icon: <Icon name="copy" />, disabled: tab.kind !== 'table' },
    ),
  ];
}

/** The whole tab menu, top to bottom (DESIGN.md §7). */
function entries(
  tabs: QueryTabs,
  tab: QueryTab,
  actions: TabMenuActions,
  windows: readonly WindowEntryDto[],
): MenuEntry[] {
  return [
    ...tabEntries(tabs, tab, actions),
    separator,
    ...placeEntries(tabs, tab, actions, windows),
    separator,
    ...dataEntries(tab, actions),
    separator,
    ...closeEntries(tabs, tab.id),
  ];
}

/** The tab strip's context menu at the pointer (DESIGN.md §7). */
export function TabMenu(props: {
  tabs: QueryTabs;
  target: TabMenuTarget;
  actions: TabMenuActions;
  onClose: () => void;
}): React.JSX.Element | null {
  const windows = useOtherWindows();
  const tab = props.tabs.state.tabs.find((item) => item.id === props.target.id);
  if (!tab) return null;
  return (
    <Menu
      label={t('tabActions')}
      at={{ x: props.target.x, y: props.target.y }}
      entries={entries(props.tabs, tab, props.actions, windows)}
      onClose={props.onClose}
    />
  );
}
