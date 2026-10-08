// SPDX-License-Identifier: Apache-2.0
import { PointMenu } from '../../../shell/PointMenu';

/** One menu command; `checked` makes it a radio choice. */
export type MenuItem = {
  label: string;
  run: () => void;
  checked?: boolean;
  danger?: boolean;
  disabled?: boolean;
  title?: string;
};

/** A menu at a point (picker or context menu). */
export type MenuState = {
  x: number;
  y: number;
  label: string;
  items: readonly MenuItem[];
};

/** Pointer-anchored menu of commands; choosing one runs it and closes. */
export function ItemMenu({
  menu,
  onClose,
}: {
  menu: MenuState;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <PointMenu x={menu.x} y={menu.y} label={menu.label} onClose={onClose}>
      {menu.items.map((item) => (
        <button
          key={item.label}
          type="button"
          role={item.checked === undefined ? 'menuitem' : 'menuitemradio'}
          aria-checked={item.checked}
          disabled={item.disabled}
          title={item.title}
          data-danger={item.danger}
          onClick={() => {
            onClose();
            item.run();
          }}
        >
          {item.label}
          {item.checked !== undefined && <kbd>{item.checked ? '✓' : ''}</kbd>}
        </button>
      ))}
    </PointMenu>
  );
}
