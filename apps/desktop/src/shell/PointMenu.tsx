// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { GlassPanel } from '@openrows/ui';
import { menuKey } from './SidebarMenu';
import styles from './SidebarMenu.module.css';

/**
 * Pointer-anchored menu, clamped to the window with keyboard navigation.
 * `above` grows the menu upwards from `y` (bottom-bar buttons); `aside`
 * renders next to the menu panel and counts as inside it (submenus).
 */
export function PointMenu(props: {
  x: number;
  y: number;
  label: string;
  onClose: () => void;
  above?: boolean;
  aside?: React.ReactNode;
  children: React.ReactNode;
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const { above = false } = props;
  useLayoutEffect(() => {
    const menu = ref.current;
    if (!menu) return;
    const top = above ? props.y - menu.offsetHeight : props.y;
    menu.style.left = `${String(Math.max(8, Math.min(props.x, innerWidth - menu.offsetWidth - 8)))}px`;
    menu.style.top = `${String(Math.max(8, Math.min(top, innerHeight - menu.offsetHeight - 8)))}px`;
  }, [props.x, props.y, above]);
  const { onClose } = props;
  useEffect(() => {
    const menu = ref.current;
    const opener = document.activeElement;
    menu
      ?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')
      ?.focus();
    const outside = (event: Event): void => {
      if (event.target instanceof Node && !menu?.contains(event.target))
        onClose();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('scroll', outside, true);
    document.addEventListener('focusin', outside);
    window.addEventListener('resize', onClose);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('scroll', outside, true);
      document.removeEventListener('focusin', outside);
      window.removeEventListener('resize', onClose);
      if (
        opener instanceof HTMLElement &&
        (menu?.contains(document.activeElement) ||
          document.activeElement === document.body)
      )
        opener.focus();
    };
  }, [onClose]);
  return createPortal(
    <div ref={ref} className={styles.anchor} style={{ width: 240 }}>
      <GlassPanel
        role="menu"
        aria-label={props.label}
        className={styles.menu}
        onKeyDown={(event) => {
          menuKey(event, props.onClose);
        }}
      >
        {props.children}
      </GlassPanel>
      {props.aside}
    </div>,
    document.body,
  );
}
