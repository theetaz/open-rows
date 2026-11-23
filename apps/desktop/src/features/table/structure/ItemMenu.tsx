// SPDX-License-Identifier: Apache-2.0
import { Menu } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';

/** One menu command; `checked` makes it a radio choice. */
export type MenuItem = {
  label: string;
  run: () => void;
  checked?: boolean;
  danger?: boolean;
  disabled?: boolean;
  /** Why the command is unavailable; shown under the menu's rows. */
  title?: string;
};

/** A menu at a point (picker or context menu). */
export type MenuState = {
  x: number;
  y: number;
  label: string;
  items: readonly MenuItem[];
};

function entryOf(item: MenuItem): MenuEntry {
  return {
    kind: 'item',
    label: item.label,
    run: item.run,
    disabled: item.disabled ?? false,
    danger: item.danger ?? false,
    ...(item.checked === undefined
      ? {}
      : { checked: item.checked, radio: true }),
  };
}

/** Pointer-anchored menu of commands; choosing one runs it and closes. */
export function ItemMenu({
  menu,
  onClose,
}: {
  menu: MenuState;
  onClose: () => void;
}): React.JSX.Element {
  const notes = menu.items.flatMap((item) =>
    item.disabled && item.title ? [item.title] : [],
  );
  return (
    <Menu
      at={{ x: menu.x, y: menu.y }}
      label={menu.label}
      entries={menu.items.map(entryOf)}
      onClose={onClose}
      footer={notes.length ? <p>{notes.join(' ')}</p> : undefined}
    />
  );
}
