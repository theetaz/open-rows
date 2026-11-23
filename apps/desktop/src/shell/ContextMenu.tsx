// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { Menu } from '@openrows/ui';
import type { MenuProps } from '@openrows/ui';

/**
 * The shared `Menu`, ringing `target` (the row it acts on) while it is open
 * (DESIGN.md §8: the object under the menu keeps an accent ring).
 */
export function ContextMenu(
  props: MenuProps & { target?: Element | null | undefined },
): React.JSX.Element {
  const { target, ...menu } = props;
  useEffect(() => {
    if (!target) return undefined;
    target.setAttribute('data-menu-target', 'true');
    return () => {
      target.removeAttribute('data-menu-target');
    };
  }, [target]);
  return <Menu {...menu} />;
}

/** The visual row of a right-clicked element: a tree item's row, else itself. */
export function menuTarget(element: Element): Element {
  return element.getAttribute('role') === 'treeitem'
    ? (element.firstElementChild ?? element)
    : element;
}
