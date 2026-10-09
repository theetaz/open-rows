// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Tabs } from '@openrows/ui';
import { PointMenu } from '../../shell/PointMenu';
import menuStyles from '../../shell/CascadeMenu.module.css';
import { TabMenu } from './TabMenu';
import type { TabMenuTarget } from './TabMenu';
import type { QueryTab as Tab } from './tabs';
import type { QueryTabs } from './useQueryTabs';

/** The tab title; a file-bound tab with unsaved changes gets a dot. */
function tabLabel(tab: Tab): string {
  return tab.file && tab.file.saved !== tab.sql ? `${tab.title} •` : tab.title;
}

/** Every tab of the strip, for when they do not all fit. */
function OverflowMenu(props: {
  list: readonly Tab[];
  active: string;
  at: { x: number; y: number };
  onSelect: (id: string) => void;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <PointMenu
      x={props.at.x}
      y={props.at.y}
      label="All tabs"
      onClose={props.onClose}
    >
      {props.list.map((tab) => (
        <button
          type="button"
          role="menuitemradio"
          aria-checked={tab.id === props.active}
          key={tab.id}
          onClick={() => {
            props.onClose();
            props.onSelect(tab.id);
          }}
        >
          <span className={menuStyles.check} aria-hidden="true">
            {tab.id === props.active ? '✓' : ''}
          </span>
          {tabLabel(tab)}
        </button>
      ))}
    </PointMenu>
  );
}

/**
 * Document tabs with drag reordering, the tab menu, preview tabs, in-place
 * rename (double-click or Rename Tab) and the overflow menu.
 */
export function TabStrip(props: {
  tabs: QueryTabs;
  list: readonly Tab[];
  active: string;
  connection: string;
  onAdd: () => void;
}): React.JSX.Element {
  const { tabs } = props;
  const [menu, setMenu] = useState<TabMenuTarget | null>(null);
  const [overflow, setOverflow] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [renaming, setRenaming] = useState<string | null>(null);
  return (
    <>
      <Tabs
        tabs={props.list.map((item) => ({
          id: item.id,
          label: tabLabel(item),
          name: item.title,
          kind: item.kind,
          title: `${props.connection} · ${item.title}`,
          pinned: item.pinned ?? false,
          preview: item.preview ?? false,
        }))}
        active={props.active}
        onSelect={tabs.select}
        onClose={tabs.close}
        onAdd={props.onAdd}
        onReorder={tabs.move}
        onKeep={tabs.keep}
        renaming={renaming}
        onRenameStart={setRenaming}
        onRename={(id, title) => {
          setRenaming(null);
          if (title !== null) tabs.rename(id, title);
        }}
        onMenu={(id, x, y) => {
          setMenu({ id, x, y });
        }}
        onOverflow={(x, y) => {
          setOverflow({ x, y });
        }}
      />
      {menu && (
        <TabMenu
          tabs={tabs}
          target={menu}
          onRename={setRenaming}
          onClose={() => {
            setMenu(null);
          }}
        />
      )}
      {overflow && (
        <OverflowMenu
          list={props.list}
          active={props.active}
          at={overflow}
          onSelect={tabs.select}
          onClose={() => {
            setOverflow(null);
          }}
        />
      )}
    </>
  );
}
