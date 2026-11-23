// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { Icon } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { TabItem } from './TabItem';
import { TabMenu } from './TabMenu';
import type { MoveTab, TabMenuTarget } from './TabMenu';
import { TabGroupEditor } from './TabGroupEditor';
import { TabListPicker } from './TabListPicker';
import { useStripActions } from './useStripActions';
import type { StripActions } from './useStripActions';
import type { QueryTab, TabsState } from './tabs';
import type { TabGroup } from './tabGroups';
import type { QueryTabs } from './useQueryTabs';
import styles from './TabStrip.module.css';
import { t } from './tabMessages';

export type WindowTabStripProps = {
  tabs: QueryTabs;
  /** The tabs the strip shows, in strip order. */
  list: readonly QueryTab[];
  /** The tab the window shows. */
  active: string | undefined;
  profiles: readonly ProfileDto[];
  /** Tooltip prefix for tabs of no saved connection (local files). */
  owner?: string | undefined;
  /** The sidebar's state; closed, the strip clears the traffic lights. */
  sidebar?: 'open' | 'closed' | undefined;
  onSelect: (id: string) => void;
  onAdd: () => void;
  /** A new tab right after `id`, in its group. */
  onAddRight?: ((id: string) => void) | undefined;
  /** Move a tab to another window; no window items without it. */
  onMove?: MoveTab | undefined;
  onReload?: ((id: string) => void) | undefined;
};

/** A chip or a tab, as the strip draws them. */
type Slot = { group: TabGroup; count: number } | { tab: QueryTab };

/** Chips before each group's first tab; a collapsed group hides its tabs. */
function slots(state: TabsState, list: readonly QueryTab[]): Slot[] {
  const out: Slot[] = [];
  let previous: string | undefined;
  for (const tab of list) {
    const group = tab.group ? state.groups?.[tab.group] : undefined;
    if (group && tab.group !== previous)
      out.push({
        group,
        count: list.filter((item) => item.group === group.id).length,
      });
    previous = group ? tab.group : undefined;
    if (!group?.collapsed) out.push({ tab });
  }
  return out;
}

/** The group's chip: click collapses, right-click or double-click edits. */
function GroupChip(props: {
  group: TabGroup;
  count: number;
  onToggle: (id: string) => void;
  onEdit: (id: string, anchor: Element) => void;
}): React.JSX.Element {
  const { group } = props;
  const name = group.name || t('unnamedGroup');
  return (
    <button
      type="button"
      className={styles.chip}
      data-chip={group.id}
      data-collapsed={group.collapsed}
      data-unnamed={!group.name}
      style={{ '--group': `var(--or-group-${group.colour})` } as CSSProperties}
      aria-expanded={!group.collapsed}
      aria-label={
        group.collapsed
          ? t('expandLabel', { name })
          : t('collapseLabel', { name })
      }
      onClick={() => {
        props.onToggle(group.id);
      }}
      onDoubleClick={(event) => {
        props.onEdit(group.id, event.currentTarget);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        props.onEdit(group.id, event.currentTarget);
      }}
    >
      {group.name && <span>{group.name}</span>}
      {group.collapsed && <span className={styles.count}>{props.count}</span>}
    </button>
  );
}

/** Keep the active tab in view when it changes. */
function useActiveInView(
  listRef: React.RefObject<HTMLDivElement | null>,
  active: string | undefined,
): void {
  useEffect(() => {
    const strip = listRef.current;
    const node = strip?.querySelector<HTMLElement>(
      `[data-tab-id="${CSS.escape(active ?? '')}"]`,
    );
    if (!strip || !node) return;
    const left = node.offsetLeft;
    const right = left + node.offsetWidth;
    if (left < strip.scrollLeft) strip.scrollLeft = left - 8;
    else if (right > strip.scrollLeft + strip.clientWidth)
      strip.scrollLeft = right - strip.clientWidth + 8;
  }, [listRef, active]);
}

/** Connection, environment, database and title, for a tab's tooltip. */
function tooltipOf(props: WindowTabStripProps, tab: QueryTab): string {
  const profile = props.profiles.find((item) => item.id === tab.connectionId);
  if (!profile)
    return props.owner ? `${props.owner} · ${tab.title}` : tab.title;
  const env = profile.env.label ?? profile.env.kind;
  return `${profile.name} (${env}) · ${tab.database} · ${tab.title}`;
}

/** The group colour token of a tab, if it is in a group. */
function groupColour(state: TabsState, tab: QueryTab): string | undefined {
  const group = tab.group ? state.groups?.[tab.group] : undefined;
  return group ? `var(--or-group-${group.colour})` : undefined;
}

/** The tab list itself: chips and tabs in strip order. */
function StripTabs(props: {
  strip: WindowTabStripProps;
  actions: StripActions;
  menu: string | undefined;
  listRef: React.RefObject<HTMLDivElement | null>;
}): React.JSX.Element {
  const { strip, actions, menu, listRef } = props;
  const prod = new Set(
    strip.profiles
      .filter((item) => item.env.kind === 'prod')
      .map((item) => item.id),
  );
  return (
    <div
      ref={listRef}
      role="tablist"
      aria-label={t('workspaceTabs')}
      className={styles.tabs}
      onWheel={(event) => {
        if (Math.abs(event.deltaY) > Math.abs(event.deltaX))
          event.currentTarget.scrollLeft += event.deltaY;
      }}
    >
      {slots(strip.tabs.state, strip.list).map((slot) =>
        'group' in slot ? (
          <GroupChip
            key={`group:${slot.group.id}`}
            group={slot.group}
            count={slot.count}
            onToggle={actions.toggleGroup}
            onEdit={actions.editGroup}
          />
        ) : (
          <TabItem
            key={slot.tab.id}
            tab={slot.tab}
            active={slot.tab.id === strip.active}
            menu={menu === slot.tab.id}
            renaming={actions.renaming === slot.tab.id}
            group={groupColour(strip.tabs.state, slot.tab)}
            prod={prod.has(slot.tab.connectionId)}
            tooltip={tooltipOf(strip, slot.tab)}
            actions={actions.item}
          />
        ),
      )}
    </div>
  );
}

/** The tab menu, the group editor and the list of all tabs, when open. */
function StripOverlays(props: {
  strip: WindowTabStripProps;
  actions: StripActions;
  menu: TabMenuTarget | null;
  overflow: Element | null;
  onMenuClosed: () => void;
  onListClosed: () => void;
}): React.JSX.Element {
  const { strip, actions, menu, overflow } = props;
  return (
    <>
      {menu && (
        <TabMenu
          tabs={strip.tabs}
          target={menu}
          actions={actions.menu}
          onClose={props.onMenuClosed}
        />
      )}
      {actions.editing && (
        <TabGroupEditor
          key={actions.editing.group.id}
          tabs={strip.tabs}
          group={actions.editing.group}
          anchor={actions.editing.anchor}
          actions={actions.groupActions}
          onClose={actions.closeEditor}
        />
      )}
      {overflow && (
        <TabListPicker
          anchor={overflow}
          state={strip.tabs.state}
          list={strip.list}
          active={strip.active}
          profiles={strip.profiles}
          onPick={actions.pick}
          onClose={props.onListClosed}
        />
      )}
    </>
  );
}

/**
 * The window's tab strip (DESIGN.md §7): Chrome's model with drag to
 * reorder (FLIP), pinned tabs, named colour groups that collapse, the tab
 * menu, middle-click close and a searchable list of every tab.
 */
export function WindowTabStrip(props: WindowTabStripProps): React.JSX.Element {
  const listRef = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<TabMenuTarget | null>(null);
  const [overflow, setOverflow] = useState<Element | null>(null);
  const actions = useStripActions(props, listRef, setMenu);
  useActiveInView(listRef, props.active);
  return (
    <div className={styles.strip} data-sidebar={props.sidebar}>
      <StripTabs
        strip={props}
        actions={actions}
        menu={menu?.id}
        listRef={listRef}
      />
      <button
        type="button"
        className={styles.button}
        aria-label={t('newTab')}
        title={t('newTabKey')}
        onClick={props.onAdd}
      >
        <Icon name="plus" />
      </button>
      <div className={styles.drag} data-tauri-drag-region />
      <button
        type="button"
        className={styles.button}
        aria-label={t('allTabs')}
        aria-haspopup="dialog"
        title={t('searchTabs')}
        onClick={(event) => {
          setOverflow(overflow ? null : event.currentTarget);
        }}
      >
        <Icon name="chevronDown" />
      </button>
      <StripOverlays
        strip={props}
        actions={actions}
        menu={menu}
        overflow={overflow}
        onMenuClosed={() => {
          setMenu(null);
        }}
        onListClosed={() => {
          setOverflow(null);
        }}
      />
    </div>
  );
}
