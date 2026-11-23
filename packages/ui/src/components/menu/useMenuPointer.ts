// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef } from 'react';
import type { Dispatch, PointerEvent, RefObject, SetStateAction } from 'react';
import { aimsAtSubmenu } from './safeTriangle';
import { hover, levelEntries, openSubmenu } from './menuModel';
import type { MenuEntry, MenuState } from './menuModel';

/** Submenus open after this hover delay. */
const OPEN_DELAY = 90;
/** While the pointer aims at an open submenu, other rows wait this long. */
const AIM_GRACE = 280;

type Point = { x: number; y: number };
type Options = {
  root: readonly MenuEntry[];
  state: MenuState;
  setState: Dispatch<SetStateAction<MenuState>>;
  panels: RefObject<(HTMLElement | null)[]>;
};

/**
 * Pointer behaviour: hover highlights, submenus open after 90 ms, and the
 * safe triangle keeps an open submenu while the pointer travels towards it.
 */
export function useMenuPointer({ root, state, setState, panels }: Options): {
  onMove: (level: number, index: number | null, event: PointerEvent) => void;
  onLeave: (level: number) => void;
} {
  const last = useRef<Point | null>(null);
  const timer = useRef(0);
  const current = useRef(state);
  useEffect(() => {
    current.current = state;
  }, [state]);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );

  const apply = useCallback(
    (level: number, index: number | null) => {
      setState((previous) => hover(previous, level, index));
      const entry =
        index === null
          ? undefined
          : levelEntries(root, current.current.open)[level]?.[index];
      if (index === null || entry?.kind !== 'submenu') return;
      timer.current = window.setTimeout(() => {
        setState((previous) => openSubmenu(previous, level, index, null));
      }, OPEN_DELAY);
    },
    [root, setState],
  );

  const onMove = useCallback(
    (level: number, index: number | null, event: PointerEvent) => {
      window.clearTimeout(timer.current);
      const point = { x: event.clientX, y: event.clientY };
      const child = panels.current[level + 1];
      const parent = panels.current[level];
      const from = last.current;
      const leaving = current.current.open[level] !== index;
      if (child && parent && from && leaving) {
        const aiming = aimsAtSubmenu(
          from,
          point,
          child.getBoundingClientRect(),
          parent.getBoundingClientRect(),
        );
        if (aiming) {
          timer.current = window.setTimeout(() => {
            apply(level, index);
          }, AIM_GRACE);
          return;
        }
      }
      last.current = point;
      if (index === null && child) return;
      apply(level, index);
    },
    [apply, panels],
  );

  const onLeave = useCallback(
    (level: number) => {
      if (current.current.open.length > level) return;
      window.clearTimeout(timer.current);
      setState((previous) => hover(previous, level, null));
    },
    [setState],
  );
  return { onMove, onLeave };
}
