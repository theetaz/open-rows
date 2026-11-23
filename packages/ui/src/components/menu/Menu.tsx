// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, KeyboardEvent, ReactNode, SetStateAction } from 'react';
import { createPortal } from 'react-dom';
import { MenuPanel } from './MenuPanel';
import { menuKey } from './menuKeys';
import { levelEntries, openSubmenu, step } from './menuModel';
import type { MenuEntry, MenuState } from './menuModel';
import { belowAnchor, placeFloating } from './placement';
import { useMenuExit, useMenuDismiss, useTypeahead } from './useMenuLife';
import { useMenuPointer } from './useMenuPointer';
import styles from './Menu.module.css';

export type MenuProps = {
  /** Accessible name of the menu. */
  label: string;
  entries: readonly MenuEntry[];
  /** Called once the menu has closed (after its fade); unmount it then. */
  onClose: () => void;
  /** A context menu at the pointer (`clientX`, `clientY`)… */
  at?: { x: number; y: number };
  /** …or a pull-down under this element. */
  anchor?: Element | null;
  /** Align a pull-down to the anchor's leading (default) or trailing edge. */
  align?: 'start' | 'end';
  /** Opened from the keyboard: highlight the first row. */
  keyboard?: boolean;
  /** A note under the rows, e.g. why commands are unavailable. */
  footer?: ReactNode;
};
type Panels = React.RefObject<(HTMLElement | null)[]>;

/** Where a submenu goes: overlapping its parent by 3 px, flipping left. */
function placeSubmenu(panel: HTMLElement, parent: HTMLElement | null): void {
  const row = parent?.querySelector<HTMLElement>('[aria-expanded="true"]');
  if (!parent || !row) return;
  const outer = parent.getBoundingClientRect();
  const box = row.getBoundingClientRect();
  placeFloating(panel, {
    x: outer.right - 3,
    y: box.top - 5,
    flipX: outer.left + 3,
    flip: box.bottom + 5,
  });
}

/** Places each level: the root at the pointer or under the anchor. */
function useMenuPlace(
  props: MenuProps,
  panels: Panels,
): (panel: HTMLElement, level: number) => void {
  const { anchor, align } = props;
  const x = props.at?.x;
  const y = props.at?.y;
  return useCallback(
    (panel: HTMLElement, level: number) => {
      if (level > 0) placeSubmenu(panel, panels.current[level - 1] ?? null);
      else if (x !== undefined && y !== undefined)
        placeFloating(panel, { x: x + 1, y: y + 1, flipX: x, flip: y });
      else if (anchor) {
        const width = panel.offsetWidth;
        placeFloating(
          panel,
          belowAnchor(anchor.getBoundingClientRect(), { align, width }),
        );
      }
    },
    [x, y, anchor, align, panels],
  );
}

/** Real focus follows the deepest highlighted row (or its panel). */
function useActiveFocus(state: MenuState, panels: Panels): void {
  useEffect(() => {
    const level = state.active.length - 1;
    const panel = panels.current[level];
    const index = state.active[level];
    const row =
      index === null || index === undefined
        ? null
        : panel?.querySelector<HTMLElement>(`[data-index="${String(index)}"]`);
    (row ?? panel)?.focus();
  }, [state, panels]);
}

/** Choosing a row: a submenu opens on its first row, a command runs. */
function useChoose(
  entries: readonly MenuEntry[],
  state: MenuState,
  setState: Dispatch<SetStateAction<MenuState>>,
  run: (level: number, index: number, command: () => void) => void,
): (level: number, index: number) => void {
  return (level, index) => {
    const entry = levelEntries(entries, state.open)[level]?.[index];
    if (entry?.kind === 'submenu' && !entry.disabled) {
      const first = step(entry.entries, null, 1);
      setState((previous) => openSubmenu(previous, level, index, first));
    } else if (entry?.kind === 'item' && !entry.disabled)
      run(level, index, entry.run);
  };
}

/**
 * A macOS menu in the webview: AppKit metrics, submenus that open after 90 ms
 * with a safe triangle, ↑↓ → ← ↩ Esc and type-ahead, clamped 6 px inside the
 * window, a blink on choose and a quick fade. Mount it while open:
 *
 * ```tsx
 * {at && (
 *   <Menu label="Table" at={at} onClose={() => setAt(null)} entries={[
 *     { kind: 'item', label: 'Open', run: open, icon: <Icon name="table" /> },
 *     { kind: 'submenu', label: 'Copy As', entries: copyEntries },
 *     { kind: 'separator' },
 *     { kind: 'item', label: 'Drop Table…', run: drop, danger: true },
 *   ]} />
 * )}
 * ```
 */
export function Menu(props: MenuProps): React.JSX.Element {
  const { entries } = props;
  const [state, setState] = useState<MenuState>(() => ({
    open: [],
    active: [props.keyboard ? step(entries, null, 1) : null],
  }));
  const panels = useRef<(HTMLElement | null)[]>([]);
  const container = useRef<HTMLDivElement>(null);
  const exit = useMenuExit(props.onClose);
  useMenuDismiss(container, props.onClose);
  const typed = useTypeahead();
  const pointer = useMenuPointer({ root: entries, state, setState, panels });
  useActiveFocus(state, panels);
  const place = useMenuPlace(props, panels);
  const register = useCallback((level: number, panel: HTMLElement | null) => {
    panels.current[level] = panel;
  }, []);
  const choose = useChoose(entries, state, setState, exit.choose);
  const onKeyDown = (event: KeyboardEvent): void => {
    if (event.nativeEvent.isComposing) return;
    const result = menuKey(event.key, state, entries, typed(event));
    if (result.kind === 'none') return;
    event.preventDefault();
    event.stopPropagation();
    if (result.kind === 'state') setState(result.state);
    else if (result.kind === 'close') exit.close();
    else choose(result.level, result.index);
  };
  const levels = levelEntries(entries, state.open);
  const shared = { place, register, onChoose: choose, ...pointer };
  return createPortal(
    <div ref={container} className={styles.menu} onKeyDown={onKeyDown}>
      {levels.map((rows, level) => {
        const opener = state.open[level - 1] ?? -1;
        const parent = levels[level - 1]?.[opener];
        return (
          <MenuPanel
            key={level ? `${String(level)}:${String(opener)}` : 'root'}
            {...shared}
            level={level}
            label={parent && 'label' in parent ? parent.label : props.label}
            entries={rows}
            active={state.active[level] ?? null}
            open={state.open[level] ?? null}
            chosen={exit.chosen?.level === level ? exit.chosen.index : null}
            closing={exit.closing}
            footer={level ? undefined : props.footer}
          />
        );
      })}
    </div>,
    document.body,
  );
}
