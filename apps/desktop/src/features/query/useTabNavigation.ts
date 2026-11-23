// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef } from 'react';
import { openTab } from './tabs';
import type { QueryTab, TabPlace, TabsState } from './tabs';
import { moveTab } from './tabOps';
import { historyStep, replaceWithPlace, visitStep } from './tabPlaces';

type Navigate = (id: string, to: TabPlace | 'back' | 'forward') => void;
type Guard = React.RefObject<
  ((id: string, preview?: boolean) => boolean) | null
>;

/** The step a navigation takes from `tab`, or `null` when there is none. */
function stepOf(tab: QueryTab, to: TabPlace | 'back' | 'forward') {
  return typeof to === 'string' ? historyStep(tab, to) : visitStep(tab, to);
}

/** Open `place` as a new table tab right after `id`. */
function besides(state: TabsState, id: string, place: TabPlace): TabsState {
  const opened = openTab(state, place, {
    title: place.table.name,
    objectPath: [place.table.schema, place.table.name],
    table: place.table,
  });
  const at = opened.tabs.findIndex((tab) => tab.id === id);
  const added = opened.tabs.at(-1);
  return added && at >= 0 ? moveTab(opened, added.id, at + 1) : opened;
}

/**
 * Per-tab navigation: the tab's view is released (the close guard says
 * whether it may be, without asking) and the place takes its slot. With
 * staged edits the place opens in a new tab beside it instead.
 */
export function useTabNavigation(
  state: TabsState,
  setState: React.Dispatch<React.SetStateAction<TabsState>>,
  guard: Guard,
): Navigate {
  const latest = useRef(state);
  useEffect(() => {
    latest.current = state;
  }, [state]);
  return useCallback<Navigate>(
    (id, to) => {
      const tab = latest.current.tabs.find((item) => item.id === id);
      const step = tab ? stepOf(tab, to) : null;
      if (!step) return;
      if (guard.current && !guard.current(id, true)) {
        setState((current) => besides(current, id, step.place));
        return;
      }
      setState((current) => replaceWithPlace(current, id, step));
    },
    [guard, setState],
  );
}
