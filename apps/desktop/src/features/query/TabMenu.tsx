// SPDX-License-Identifier: Apache-2.0
import { PointMenu } from '../../shell/PointMenu';
import { otherTabs, tabsRightOf } from './tabOps';
import type { QueryTabs } from './useQueryTabs';

/** Where the tab menu opened, and for which tab. */
export type TabMenuTarget = { id: string; x: number; y: number };

type Item = {
  label: string;
  run: () => void;
  disabled?: boolean;
  /** Draw a separator before this item. */
  group?: boolean;
};

/** Close, Close Others, Close Tabs to the Right, Pin/Unpin and Keep Open. */
function items(tabs: QueryTabs, id: string): Item[] {
  const tab = tabs.state.tabs.find((item) => item.id === id);
  const others = otherTabs(tabs.state, id);
  const right = tabsRightOf(tabs.state, id);
  const closeAll = (ids: readonly string[]): void => {
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
      label: tab?.pinned ? 'Unpin Tab' : 'Pin Tab',
      run: () => {
        tabs.pin(id, !tab?.pinned);
      },
      group: true,
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
  onClose: () => void;
}): React.JSX.Element {
  const { target, onClose } = props;
  return (
    <PointMenu x={target.x} y={target.y} label="Tab actions" onClose={onClose}>
      {items(props.tabs, target.id).map((item) => (
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
