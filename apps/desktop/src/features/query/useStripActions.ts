// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { useReducedMotion } from '@openrows/ui';
import { changeGroup, newGroup } from './tabGroups';
import type { TabGroup } from './tabGroups';
import { moveTab } from './tabOps';
import type { QueryTab, TabsState } from './tabs';
import { startTabDrag } from './tabDrag';
import type { TabDrop } from './tabDrag';
import type { TabItemActions } from './TabItem';
import type { TabMenuActions, TabMenuTarget } from './TabMenu';
import type { GroupEditorActions } from './TabGroupEditor';
import type { WindowTabStripProps } from './WindowTabStrip';

/** The strip's latest props, read when a handler runs. */
type Latest = React.RefObject<WindowTabStripProps>;
type Props = WindowTabStripProps;

/** The model index a drop lands on, among the tabs other than the dragged one. */
function dropIndex(state: TabsState, drop: TabDrop): number {
  const rest = state.tabs.filter((tab) => tab.id !== drop.id);
  const before = drop.others[drop.index];
  if (before) return rest.findIndex((tab) => tab.id === before);
  const last = drop.others.at(-1);
  return last ? rest.findIndex((tab) => tab.id === last) + 1 : 0;
}

/** The nearest shown tab outside `group`, preferring the one after `from`. */
function outside(
  state: TabsState,
  list: readonly QueryTab[],
  group: string,
  from: string | undefined,
): QueryTab | undefined {
  const open = list.filter((tab) => {
    const owner = tab.group ? state.groups?.[tab.group] : undefined;
    return tab.group !== group && !owner?.collapsed;
  });
  const at = list.findIndex((tab) => tab.id === from);
  const after = open.find((tab) => list.indexOf(tab) > at);
  return after ?? open.at(-1);
}

/** ← → move focus and selection; with ⌥ the tab moves; Home and End jump. */
function keyAction(props: Props, event: React.KeyboardEvent, id: string): void {
  const { tabs, list, onSelect } = props;
  const index = list.findIndex((tab) => tab.id === id);
  const step = ({ ArrowLeft: -1, ArrowRight: 1 } as Record<string, number>)[
    event.key
  ];
  if (event.altKey && step) {
    event.preventDefault();
    const at = tabs.state.tabs.findIndex((tab) => tab.id === id);
    tabs.apply?.((state) => moveTab(state, id, Math.max(0, at + step)));
    return;
  }
  const targets: Record<string, number> = {
    ArrowRight: (index + 1) % list.length,
    ArrowLeft: (index - 1 + list.length) % list.length,
    Home: 0,
    End: list.length - 1,
  };
  const next = list[targets[event.key] ?? -1];
  if (!next) return;
  event.preventDefault();
  onSelect(next.id);
  const peers = event.currentTarget.closest('[role="tablist"]');
  requestAnimationFrame(() => {
    peers
      ?.querySelector<HTMLElement>(
        `[data-tab-id="${CSS.escape(next.id)}"] [role="tab"]`,
      )
      ?.focus();
  });
}

/** Collapse or expand a group; collapsing moves focus out of it first. */
function toggleGroup(props: Props, id: string): void {
  const { tabs, list, active, onSelect } = props;
  const group = tabs.state.groups?.[id];
  if (!group) return;
  const inside = list.find((tab) => tab.id === active)?.group === id;
  if (!group.collapsed && inside) {
    const next = outside(tabs.state, list, id, active);
    if (!next) return;
    onSelect(next.id);
  }
  tabs.apply?.((state) =>
    changeGroup(state, id, { collapsed: !group.collapsed }),
  );
}

/** Copy `connection/database/schema.table` of a table tab. */
function copyPath(props: Props, id: string): void {
  const { tabs, profiles } = props;
  const tab = tabs.state.tabs.find((item) => item.id === id);
  if (!tab?.table) return;
  const name =
    profiles.find((item) => item.id === tab.connectionId)?.name ?? '';
  const path = `${name}/${tab.database}/${tab.table.schema}.${tab.table.name}`;
  navigator.clipboard.writeText(path).catch(() => undefined);
}

/** What the strip's tabs, chips, menu, group editor and list do. */
export type StripActions = {
  item: TabItemActions;
  menu: TabMenuActions;
  groupActions: GroupEditorActions;
  editGroup: (id: string, anchor: Element) => void;
  toggleGroup: (id: string) => void;
  renaming: string | null;
  editing: { group: TabGroup; anchor: Element } | null;
  closeEditor: () => void;
  pick: (id: string) => void;
};

/** Reorder the model where a drag dropped, synchronously so the FLIP can measure. */
function drop(props: Props, target: TabDrop): void {
  const { tabs } = props;
  flushSync(() => {
    tabs.apply?.((state) =>
      moveTab(state, target.id, dropIndex(state, target)),
    );
  });
}

/** Pick from the list of all tabs: a collapsed group opens first. */
function pickTab(props: Props, id: string): void {
  const { tabs, onSelect } = props;
  const owner = tabs.state.tabs.find((tab) => tab.id === id)?.group;
  if (owner && tabs.state.groups?.[owner]?.collapsed)
    tabs.apply?.((state) => changeGroup(state, owner, { collapsed: false }));
  onSelect(id);
}

/** Start a new group with the tab and open its editor under the tab. */
function startGroup(
  props: Props,
  id: string,
  edit: (group: string, anchor: Element) => void,
  list: HTMLDivElement | null,
): void {
  const { tabs } = props;
  tabs.apply?.((state) => newGroup(state, [id])[0]);
  // The new group takes the next id.
  const node = list?.querySelector(`[data-tab-id="${CSS.escape(id)}"]`);
  if (node) edit(`group-${String(tabs.state.next)}`, node);
}

/** The id of a group's last tab. */
function lastOf(props: Props, group: string): string | undefined {
  return props.tabs.state.tabs.filter((tab) => tab.group === group).at(-1)?.id;
}

/** Handlers of each tab, stable for the memoised tabs. */
function useItemActions(
  latest: Latest,
  setMenu: (target: TabMenuTarget | null) => void,
  setRenaming: (id: string | null) => void,
): TabItemActions {
  const reduced = useReducedMotion();
  return useMemo<TabItemActions>(
    () => ({
      select: (id) => {
        latest.current.onSelect(id);
      },
      close: (id) => {
        latest.current.tabs.close(id);
      },
      menu: (id, x, y) => {
        setMenu({ id, x, y });
      },
      rename: (id) => {
        latest.current.tabs.keep(id);
        setRenaming(id);
      },
      renamed: (id, title) => {
        setRenaming(null);
        if (title !== null) latest.current.tabs.rename(id, title);
      },
      pointerDown: (event) => {
        startTabDrag(event, {
          reduced,
          onDrop: (target) => {
            drop(latest.current, target);
          },
        });
      },
      key: (event, id) => {
        keyAction(latest.current, event, id);
      },
    }),
    [latest, reduced, setMenu, setRenaming],
  );
}

/** Handlers of the tab menu and the group editor. */
function useMenuActions(
  latest: Latest,
  listRef: React.RefObject<HTMLDivElement | null>,
  setRenaming: (id: string | null) => void,
  setEditing: (editing: { id: string; anchor: Element } | null) => void,
): { menu: TabMenuActions; groups: GroupEditorActions } {
  return useMemo(
    () => ({
      menu: {
        newRight: (id) => {
          latest.current.onAddRight?.(id);
        },
        newGroup: (id) => {
          startGroup(
            latest.current,
            id,
            (group, anchor) => {
              setEditing({ id: group, anchor });
            },
            listRef.current,
          );
        },
        rename: (id) => {
          setRenaming(id);
        },
        reload: (id) => {
          latest.current.onReload?.(id);
        },
        copyPath: (id) => {
          copyPath(latest.current, id);
        },
      },
      groups: {
        newTabIn: (group) => {
          const last = lastOf(latest.current, group);
          if (last) latest.current.onAddRight?.(last);
        },
        toggle: (group) => {
          toggleGroup(latest.current, group);
        },
      },
    }),
    [latest, listRef, setRenaming, setEditing],
  );
}

/**
 * Everything the strip's tabs, chips, menu and group editor do, as stable
 * handlers over the latest props (so memoised tabs never re-render for them).
 */
export function useStripActions(
  props: WindowTabStripProps,
  listRef: React.RefObject<HTMLDivElement | null>,
  setMenu: (target: TabMenuTarget | null) => void,
): StripActions {
  const latest = useRef(props);
  useEffect(() => {
    latest.current = props;
  });
  const [renaming, setRenaming] = useState<string | null>(null);
  const [editing, setEditing] = useState<{
    id: string;
    anchor: Element;
  } | null>(null);
  const item = useItemActions(latest, setMenu, setRenaming);
  const { menu, groups } = useMenuActions(
    latest,
    listRef,
    setRenaming,
    setEditing,
  );
  const group = editing ? props.tabs.state.groups?.[editing.id] : undefined;
  return {
    item,
    menu: { ...menu, move: props.onMove },
    groupActions: groups,
    editGroup: (id, anchor) => {
      setEditing({ id, anchor });
    },
    toggleGroup: groups.toggle,
    renaming,
    editing: editing && group ? { group, anchor: editing.anchor } : null,
    closeEditor: () => {
      setEditing(null);
    },
    pick: (id) => {
      pickTab(props, id);
    },
  };
}
