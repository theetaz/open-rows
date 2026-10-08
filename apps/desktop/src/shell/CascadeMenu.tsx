// SPDX-License-Identifier: Apache-2.0
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { GlassPanel, Icon } from '@openrows/ui';
import { PointMenu } from './PointMenu';
import { menuKey } from './SidebarMenu';
import menuStyles from './SidebarMenu.module.css';
import styles from './CascadeMenu.module.css';

/** One menu row: a command, a submenu (▸) or a separator. */
export type MenuEntry =
  | {
      kind: 'item';
      label: string;
      run: () => void;
      /** Shortcut shown right-aligned, e.g. `⌘I`. */
      shortcut?: string;
      /** A check mark; with `radio` the row is one choice of a group. */
      checked?: boolean;
      radio?: boolean;
      disabled?: boolean;
    }
  | { kind: 'submenu'; label: string; entries: readonly MenuEntry[] }
  | { kind: 'separator' };

type Item = Extract<MenuEntry, { kind: 'item' }>;
type Parent = Extract<MenuEntry, { kind: 'submenu' }>;
type Expanded = { index: number; item: HTMLElement; focus: boolean };

function role(entry: Item): string {
  if (entry.checked === undefined) return 'menuitem';
  return entry.radio ? 'menuitemradio' : 'menuitemcheckbox';
}

/** A command row; choosing it closes the whole menu, then runs it. */
function ItemRow(props: {
  entry: Item;
  onHover: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const { entry } = props;
  return (
    <button
      type="button"
      role={role(entry)}
      aria-checked={entry.checked}
      disabled={entry.disabled}
      onPointerEnter={props.onHover}
      onClick={() => {
        props.onClose();
        entry.run();
      }}
    >
      <span className={styles.check} aria-hidden="true">
        {entry.checked ? '✓' : ''}
      </span>
      {entry.label}
      {entry.shortcut && <kbd>{entry.shortcut}</kbd>}
    </button>
  );
}

/** A `▸` row that opens its submenu on hover, click or →. */
function ParentRow(props: {
  entry: Parent;
  expanded: boolean;
  onOpen: (item: HTMLElement, focus: boolean) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      role="menuitem"
      aria-haspopup="menu"
      aria-expanded={props.expanded}
      className={styles.parent}
      onPointerEnter={(event) => {
        props.onOpen(event.currentTarget, false);
      }}
      onClick={(event) => {
        props.onOpen(event.currentTarget, true);
      }}
      onKeyDown={(event) => {
        if (event.key !== 'ArrowRight') return;
        event.preventDefault();
        props.onOpen(event.currentTarget, true);
      }}
    >
      <span className={styles.check} aria-hidden="true" />
      {props.entry.label}
      <span className={styles.chevron} aria-hidden="true">
        <Icon name="chevronRight" />
      </span>
    </button>
  );
}

function rows(props: {
  entries: readonly MenuEntry[];
  expanded: number | null;
  onOpen: (index: number, item: HTMLElement, focus: boolean) => void;
  onHover: () => void;
  onClose: () => void;
}): React.JSX.Element[] {
  return props.entries.map((entry, index) => {
    const key = `${String(index)}:${entry.kind === 'separator' ? '' : entry.label}`;
    if (entry.kind === 'separator') return <hr key={key} />;
    if (entry.kind === 'submenu')
      return (
        <ParentRow
          key={key}
          entry={entry}
          expanded={props.expanded === index}
          onOpen={(item, focus) => {
            props.onOpen(index, item, focus);
          }}
        />
      );
    return (
      <ItemRow
        key={key}
        entry={entry}
        onHover={props.onHover}
        onClose={props.onClose}
      />
    );
  });
}

/** Place the submenu beside its row, flipping left near the window edge. */
function placeSubmenu(panel: HTMLElement, opener: HTMLElement): void {
  const anchor = panel.parentElement?.getBoundingClientRect();
  if (!anchor) return;
  const row = opener.getBoundingClientRect();
  const fits = anchor.right + panel.offsetWidth <= innerWidth - 8;
  panel.style.left = fits ? '100%' : '';
  panel.style.right = fits ? '' : '100%';
  const lowest = innerHeight - 8 - panel.offsetHeight - anchor.top;
  panel.style.top = `${String(Math.min(row.top - anchor.top, lowest))}px`;
}

function Submenu(props: {
  entry: Parent;
  expanded: Expanded;
  onBack: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const { item, focus } = props.expanded;
  useLayoutEffect(() => {
    if (ref.current) placeSubmenu(ref.current, item);
  }, [item]);
  useEffect(() => {
    if (focus)
      ref.current
        ?.querySelector<HTMLElement>('[role^="menuitem"]:not(:disabled)')
        ?.focus();
  }, [focus]);
  return (
    <div ref={ref} className={styles.submenu}>
      <GlassPanel
        role="menu"
        aria-label={props.entry.label}
        className={menuStyles.menu}
        onKeyDown={(event) => {
          if (event.key === 'ArrowLeft' || event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            props.onBack();
            return;
          }
          menuKey(event, props.onClose);
        }}
      >
        {rows({
          entries: props.entry.entries,
          expanded: null,
          onOpen: () => undefined,
          onHover: () => undefined,
          onClose: props.onClose,
        })}
      </GlassPanel>
    </div>
  );
}

/**
 * A native-style menu with check marks, shortcuts and one level of `▸`
 * submenus (opened by hover, click or →; ← or Escape goes back).
 */
export function CascadeMenu(props: {
  x: number;
  y: number;
  label: string;
  entries: readonly MenuEntry[];
  onClose: () => void;
  above?: boolean;
}): React.JSX.Element {
  const [expanded, setExpanded] = useState<Expanded | null>(null);
  const parent = expanded ? props.entries[expanded.index] : undefined;
  const aside = expanded && parent?.kind === 'submenu' && (
    <Submenu
      entry={parent}
      expanded={expanded}
      onBack={() => {
        expanded.item.focus();
        setExpanded(null);
      }}
      onClose={props.onClose}
    />
  );
  return (
    <PointMenu
      x={props.x}
      y={props.y}
      label={props.label}
      onClose={props.onClose}
      above={props.above ?? false}
      aside={aside}
    >
      {rows({
        entries: props.entries,
        expanded: expanded?.index ?? null,
        onOpen: (index, item, focus) => {
          setExpanded({ index, item, focus });
        },
        onHover: () => {
          setExpanded(null);
        },
        onClose: props.onClose,
      })}
    </PointMenu>
  );
}

/** Where a button's menu opens (above the button) and how it closes. */
export function useMenuAnchor(): {
  at: { x: number; y: number } | null;
  openAt: (button: HTMLElement) => void;
  close: () => void;
} {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const close = useCallback(() => {
    setAt(null);
  }, []);
  const openAt = useCallback((button: HTMLElement) => {
    const box = button.getBoundingClientRect();
    setAt({ x: box.left, y: box.top - 4 });
  }, []);
  return { at, openAt, close };
}
