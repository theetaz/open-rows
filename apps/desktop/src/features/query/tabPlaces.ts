// SPDX-License-Identifier: Apache-2.0
import { closeTab, scopeKey } from './tabs';
import type { QueryTab, TabPlace, TabsState } from './tabs';

/** Places kept in each direction of a tab's history. */
const HISTORY_LIMIT = 50;

/** Where a table tab is, for its history. */
function placeOf(tab: QueryTab): TabPlace | undefined {
  if (tab.kind !== 'table' || !tab.table) return undefined;
  return {
    connectionId: tab.connectionId,
    database: tab.database,
    table: tab.table,
  };
}

/** The same relation in the same database of the same connection. */
function samePlace(a: TabPlace, b: TabPlace): boolean {
  return (
    a.connectionId === b.connectionId &&
    a.database === b.database &&
    a.table.schema === b.table.schema &&
    a.table.name === b.table.name
  );
}

/** Where a history step goes, and the lists left behind. */
type Step = {
  place: TabPlace;
  back: readonly TabPlace[];
  forward: readonly TabPlace[];
};

/** The step `direction` takes from `tab`; `null` at the end of its history. */
export function historyStep(
  tab: QueryTab,
  direction: 'back' | 'forward',
): Step | null {
  const here = placeOf(tab);
  const list = (direction === 'back' ? tab.back : tab.forward) ?? [];
  const place = list.at(-1);
  if (!here || !place) return null;
  const rest = list.slice(0, -1);
  const other = [
    ...((direction === 'back' ? tab.forward : tab.back) ?? []),
    here,
  ].slice(-HISTORY_LIMIT);
  return direction === 'back'
    ? { place, back: rest, forward: other }
    : { place, back: other, forward: rest };
}

/** The step to a new place: the current one goes back, forward clears. */
export function visitStep(tab: QueryTab, place: TabPlace): Step | null {
  const here = placeOf(tab);
  if (!here || samePlace(here, place)) return null;
  return {
    place,
    back: [...(tab.back ?? []), here].slice(-HISTORY_LIMIT),
    forward: [],
  };
}

/**
 * Show `step.place` in tab `id`'s slot: a new tab (new id, so the view
 * remounts) keeps the strip position, pin, group and history, and becomes
 * the active tab of its scope.
 */
export function replaceWithPlace(
  state: TabsState,
  id: string,
  step: Step,
): TabsState {
  const index = state.tabs.findIndex((tab) => tab.id === id);
  const old = state.tabs[index];
  if (!old) return state;
  const { place } = step;
  const tab: QueryTab = {
    ...old,
    id: `table-${String(state.next)}`,
    connectionId: place.connectionId,
    database: place.database,
    title: place.table.name,
    sql: '',
    objectPath: [place.table.schema, place.table.name],
    table: place.table,
    back: step.back,
    forward: step.forward,
    preview: false,
  };
  const closed = closeTab(state, id);
  const tabs = [...closed.tabs];
  tabs.splice(index, 0, tab);
  return {
    ...closed,
    tabs,
    next: state.next + 1,
    active: { ...closed.active, [scopeKey(tab)]: tab.id },
  };
}
