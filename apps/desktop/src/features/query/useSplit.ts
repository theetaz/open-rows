// SPDX-License-Identifier: Apache-2.0
import type { SplitState, SplitStates } from './splitState';
import { useState } from 'react';
import { scopeKey } from './tabs';
import type { QueryTab, TabScope } from './tabs';
import type { QueryTabs } from './useQueryTabs';

/** Side by side (`right`) or stacked (`down`). */
export type Orientation = 'right' | 'down';

/** Which pane has focus: the pinned one or the one following the active tab. */
type PaneRole = 'pinned' | 'active';

/** What the split pane area renders. */
export type SplitLayout = {
  orientation: Orientation;
  ratio: number;
  pinned: QueryTab;
};

export type Split = {
  /** The split to render, or `null` for one pane. */
  layout: SplitLayout | null;
  /** Split the area (a new query tab beside the active one), or turn it. */
  split: (orientation: Orientation) => void;
  close: () => void;
  /** Move keyboard focus to the other pane. */
  focusOther: () => void;
  setRatio: (ratio: number) => void;
  /** Record which pane received focus, so the tab strip targets it. */
  focused: (role: PaneRole) => void;
  /** The tab set for the tab strip: choosing a tab fills the focused pane. */
  stripTabs: QueryTabs;
};

const clampRatio = (ratio: number): number =>
  Math.min(0.8, Math.max(0.2, ratio));

/** Focus the editor (else the grid, else the pane itself) of a pane. */
function focusPane(role: PaneRole): void {
  const pane = document.querySelector<HTMLElement>(`[data-pane="${role}"]`);
  const target =
    pane?.querySelector<HTMLElement>('.cm-content, textarea, [role="grid"]') ??
    pane;
  target?.focus();
}

/** The scope's split, if its pinned tab still exists beside another active tab. */
function layoutOf(
  state: SplitState | undefined,
  list: readonly QueryTab[],
  active: QueryTab | undefined,
): SplitLayout | null {
  const pinned = list.find((tab) => tab.id === state?.pinned);
  if (!state || !pinned || !active || pinned.id === active.id) return null;
  return { orientation: state.orientation, ratio: state.ratio, pinned };
}

/**
 * Split editor panes per tab scope (FR-EDIT-11): the first pane keeps the
 * tab that was active when splitting, the second follows the active tab.
 * Each pane keeps its own tab, connection scope, editor and results.
 */
export function useSplit(tabs: QueryTabs, scope: TabScope | undefined): Split {
  const [fallback, setFallback] = useState<SplitStates>({});
  const states = tabs.state.splits ?? fallback;
  const setStates = (change: (current: SplitStates) => SplitStates): void => {
    const next = change(states);
    if (tabs.setSplits) tabs.setSplits(next);
    else setFallback(next);
  };
  const key = scope ? scopeKey(scope) : '';
  const list = scope ? tabs.tabsFor(scope) : [];
  const active = scope ? tabs.activeFor(scope) : undefined;
  const state = states[key];
  const layout = layoutOf(state, list, active);
  const change = (next: Partial<SplitState>): void => {
    setStates((current) => {
      const previous = current[key];
      return previous
        ? { ...current, [key]: { ...previous, ...next } }
        : current;
    });
  };
  const choose = (id: string): void => {
    if (!layout || state?.focus !== 'pinned') {
      tabs.select(id);
      return;
    }
    // The pinned pane takes the chosen tab; choosing the other pane's tab swaps them.
    if (id === active?.id) tabs.select(layout.pinned.id);
    change({ pinned: id });
  };
  return {
    layout,
    split: (orientation) => {
      if (!scope || !active) return;
      if (layout) {
        change({ orientation });
        return;
      }
      setStates((current) => ({
        ...current,
        [key]: { orientation, pinned: active.id, ratio: 0.5, focus: 'active' },
      }));
      tabs.open(scope);
    },
    close: () => {
      setStates((current) =>
        Object.fromEntries(
          Object.entries(current).filter(([scoped]) => scoped !== key),
        ),
      );
    },
    focusOther: () => {
      if (!layout) return;
      const next = state?.focus === 'pinned' ? 'active' : 'pinned';
      change({ focus: next });
      focusPane(next);
    },
    setRatio: (ratio) => {
      change({ ratio: clampRatio(ratio) });
    },
    focused: (role) => {
      if (state && state.focus !== role) change({ focus: role });
    },
    stripTabs: { ...tabs, select: choose },
  };
}
