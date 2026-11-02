// SPDX-License-Identifier: Apache-2.0
import type { WindowEntryDto } from '@openrows/ipc';
import { PointMenu } from '../../shell/PointMenu';
import { useOtherWindows, windowName } from '../windows/useOtherWindows';
import { otherTabs, tabsRightOf } from './tabOps';
import type { QueryTabs } from './useQueryTabs';

/** Move a tab to window `target`, or to a new window when `null`. */
export type MoveTab = (id: string, target: string | null) => void;

/** Where the tab menu opened, and for which tab. */
export type TabMenuTarget = { id: string; x: number; y: number };

type Item = {
  label: string;
  run: () => void;
  disabled?: boolean;
  /** Draw a separator before this item. */
  group?: boolean;
};

/** Move to New Window, then Move to each other open window (FR-APP-03). */
function moveItems(
  id: string,
  move: MoveTab,
  windows: readonly WindowEntryDto[],
): Item[] {
  return [
    {
      label: 'Move to New Window',
      run: () => {
        move(id, null);
      },
      group: true,
    },
    ...windows.map((entry) => ({
      label: `Move to ${windowName(entry)}`,
      run: () => {
        move(id, entry.label);
      },
    })),
  ];
}

/** Close, Close Others, Close Tabs to the Right, Rename, Pin/Unpin and Keep Open. */
function items(
  tabs: QueryTabs,
  id: string,
  rename: (id: string) => void,
): Item[] {
  const tab = tabs.state.tabs.find((item) => item.id === id);
  const others = otherTabs(tabs.state, id);
  const right = tabsRightOf(tabs.state, id);
  const closeAll = (ids: readonly string[]): void => {
    if (tabs.closeMany) {
      tabs.closeMany(ids);
      return;
    }
    ids.forEach((other) => {
      tabs.close(other);
    });
  };
  return [
    {
      label: 'Close Tab',
      run: () => {
        tabs.close(id);
      },
    },
    {
      label: 'Close Other Tabs',
      run: () => {
        closeAll(others);
      },
      disabled: !others.length,
    },
    {
      label: 'Close Tabs to the Right',
      run: () => {
        closeAll(right);
      },
      disabled: !right.length,
    },
    {
      label: 'Rename Tab…',
      run: () => {
        rename(id);
      },
      group: true,
    },
    {
      label: tab?.pinned ? 'Unpin Tab' : 'Pin Tab',
      run: () => {
        tabs.pin(id, !tab?.pinned);
      },
    },
    {
      label: 'Keep Open',
      run: () => {
        tabs.keep(id);
      },
      disabled: !tab?.preview,
    },
  ];
}

/** The tab strip's context menu at the pointer. */
export function TabMenu(props: {
  tabs: QueryTabs;
  target: TabMenuTarget;
  /** Start editing the tab's title in place. */
  onRename: (id: string) => void;
  /** Move the tab to another window; no move items without it. */
  onMove?: MoveTab | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const { target, onClose, onMove } = props;
  const windows = useOtherWindows();
  const all = [
    ...items(props.tabs, target.id, props.onRename),
    ...(onMove ? moveItems(target.id, onMove, windows) : []),
  ];
  return (
    <PointMenu x={target.x} y={target.y} label="Tab actions" onClose={onClose}>
      {all.map((item) => (
        <MenuItem key={item.label} item={item} onClose={onClose} />
      ))}
    </PointMenu>
  );
}

function MenuItem(props: {
  item: Item;
  onClose: () => void;
}): React.JSX.Element {
  const { item } = props;
  return (
    <>
      {item.group && <hr />}
      <button
        type="button"
        role="menuitem"
        disabled={item.disabled}
        onClick={() => {
          props.onClose();
          item.run();
        }}
      >
        {item.label}
      </button>
    </>
  );
}
