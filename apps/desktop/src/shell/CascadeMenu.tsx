// SPDX-License-Identifier: Apache-2.0
import { Menu } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';

/**
 * A context menu at a window point: a thin wrapper over the shared `Menu`
 * (icons, shortcuts, submenus with a safe triangle, keys, clamping).
 */
export function CascadeMenu(props: {
  x: number;
  y: number;
  label: string;
  entries: readonly MenuEntry[];
  onClose: () => void;
  /** Shown below the rows, e.g. why some commands are unavailable. */
  footer?: React.ReactNode;
}): React.JSX.Element {
  return (
    <Menu
      label={props.label}
      at={{ x: props.x, y: props.y }}
      entries={props.entries}
      onClose={props.onClose}
      footer={props.footer}
    />
  );
}
