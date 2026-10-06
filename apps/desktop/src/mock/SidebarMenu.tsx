// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef } from 'react';
import type { ReactNode, RefObject } from 'react';
import { createPortal } from 'react-dom';
import { GlassPanel } from '@openrows/ui';
import styles from './SidebarHeader.module.css';
type Props = {
  id: string;
  label: string;
  opener: RefObject<HTMLButtonElement | null>;
  onClose: () => void;
  children: ReactNode;
};
/** Anchored glass menu with wrapping keyboard focus and opener restoration. */
export function SidebarMenu(props: Props): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const { opener, onClose } = props;
  useLayoutEffect(() => {
    const bounds = opener.current?.getBoundingClientRect();
    ref.current?.style.setProperty(
      'left',
      `${String(Math.max(8, Math.min(bounds?.left ?? 0, window.innerWidth - 248)))}px`,
    );
    ref.current?.style.setProperty('top', `${String(bounds?.bottom ?? 0)}px`);
  }, [opener]);
  useMenuFocus({ ref, opener, onClose });
  return createPortal(
    <div ref={ref} className={styles.anchor}>
      <GlassPanel
        id={props.id}
        role="menu"
        aria-label={props.label}
        className={styles.menu}
        onKeyDown={(event) => {
          menuKey(event, onClose);
        }}
      >
        {props.children}
      </GlassPanel>
    </div>,
    document.body,
  );
}
function menuKey(event: React.KeyboardEvent, close: () => void): void {
  if (event.nativeEvent.isComposing) return;
  if (event.metaKey) {
    close();
    return;
  }
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault();
    event.stopPropagation();
    close();
    return;
  }
  const items = Array.from(
    event.currentTarget.querySelectorAll<HTMLButtonElement>(
      '[role^="menuitem"]:not(:disabled)',
    ),
  );
  const index = items.indexOf(document.activeElement as HTMLButtonElement);
  const next = menuIndex(event.key, index, items.length);
  if (next === undefined) return;
  event.preventDefault();
  items[next]?.focus();
}
function menuIndex(
  key: string,
  index: number,
  length: number,
): number | undefined {
  if (key === 'Home') return 0;
  if (key === 'End') return length - 1;
  if (key === 'ArrowDown') return (index + 1) % length;
  if (key === 'ArrowUp') return (index - 1 + length) % length;
  return undefined;
}

function useMenuFocus({
  ref,
  opener,
  onClose,
}: {
  ref: RefObject<HTMLDivElement | null>;
  opener: Props['opener'];
  onClose: Props['onClose'];
}): void {
  useEffect(() => {
    const menu = ref.current;
    (
      menu?.querySelector<HTMLElement>('[aria-checked="true"]') ??
      menu?.querySelector<HTMLElement>('[role^="menuitem"]')
    )?.focus();
    function outside(event: PointerEvent): void {
      if (
        event.target instanceof Node &&
        !menu?.contains(event.target) &&
        !opener.current?.contains(event.target)
      )
        onClose();
    }
    function blur(event: FocusEvent): void {
      if (event.target instanceof Node && !menu?.contains(event.target))
        onClose();
    }
    document.addEventListener('pointerdown', outside);
    document.addEventListener('focusin', blur);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('focusin', blur);
    };
  }, [ref, opener, onClose]);
}
