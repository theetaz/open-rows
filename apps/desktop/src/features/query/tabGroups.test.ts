// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { WINDOW_SCOPE, emptyTabs, openTab, selectTab } from './tabs';
import type { QueryTabInit, TabsState } from './tabs';
import {
  addToGroup,
  changeGroup,
  liveGroups,
  newGroup,
  removeFromGroup,
  ungroup,
} from './tabGroups';
import {
  duplicateTab,
  moveTab,
  moveToEdge,
  openBeside,
  pinTab,
} from './tabOps';
import { historyStep, replaceWithPlace, visitStep } from './tabPlaces';
import { restoreTabs, serializeTabs } from './tabStorage';

const scope = { connectionId: 'a', database: 'orders' };
const table = (name: string): QueryTabInit => ({
  title: name,
  table: { database: 'orders', schema: 'public', name },
});

/** Five table tabs t1…t5 (ids table-1…table-5). */
function five(): TabsState {
  let state = emptyTabs;
  for (const name of ['t1', 't2', 't3', 't4', 't5'])
    state = openTab(state, scope, table(name));
  return state;
}
const order = (state: TabsState): string[] =>
  state.tabs.map((tab) => `${tab.title}${tab.group ? `:${tab.group}` : ''}`);

it('groups gather beside their first tab, take a free colour and stay contiguous', () => {
  const [grouped, id] = newGroup(five(), ['table-2', 'table-4']);
  expect(order(grouped)).toEqual(['t1', `t2:${id}`, `t4:${id}`, 't3', 't5']);
  expect(grouped.groups?.[id]).toMatchObject({ name: '', colour: 'blue' });
  const [second] = newGroup(grouped, ['table-5']);
  expect(liveGroups(second).map((group) => group.colour)).toEqual([
    'blue',
    'green',
  ]);
  const added = addToGroup(grouped, 'table-1', id);
  expect(order(added)).toEqual([
    `t2:${id}`,
    `t4:${id}`,
    `t1:${id}`,
    't3',
    't5',
  ]);
  const removed = removeFromGroup(added, 'table-2');
  expect(order(removed)).toEqual([`t4:${id}`, `t1:${id}`, 't2', 't3', 't5']);
  expect(order(ungroup(removed, id))).toEqual(['t4', 't1', 't2', 't3', 't5']);
  expect(ungroup(removed, id).groups?.[id]).toBeUndefined();
});

it('dropping between two tabs of a group joins it; past its end leaves it', () => {
  const [grouped, id] = newGroup(five(), ['table-2', 'table-3']);
  const joined = moveTab(grouped, 'table-5', 2);
  expect(order(joined)).toEqual([
    't1',
    `t2:${id}`,
    `t5:${id}`,
    `t3:${id}`,
    't4',
  ]);
  const left = moveTab(joined, 'table-2', 0);
  expect(order(left)).toEqual(['t2', 't1', `t5:${id}`, `t3:${id}`, 't4']);
});

it('pinning leaves the group, and pinned tabs stay first', () => {
  const [grouped] = newGroup(five(), ['table-3']);
  const pinned = pinTab(grouped, 'table-3', true);
  expect(order(pinned)[0]).toBe('t3');
  expect(pinned.tabs[0]?.group).toBeUndefined();
  expect(order(moveToEdge(pinned, 'table-5', 'start'))[1]).toBe('t5');
  expect(order(moveTab(pinned, 'table-1', 0))[0]).toBe('t3');
});

it('duplicates beside the original and opens a new tab right after it in its group', () => {
  const [grouped, id] = newGroup(five(), ['table-2']);
  const copy = duplicateTab(grouped, 'table-2');
  expect(order(copy).slice(0, 3)).toEqual(['t1', `t2:${id}`, `t2:${id}`]);
  expect(copy.tabs[2]?.id).toBe('table-7');
  const beside = openBeside(copy, 'table-2');
  expect(beside.tabs[2]).toMatchObject({ kind: 'query', group: id });
  expect(beside.active['a\norders']).toBe(beside.tabs[2]?.id);
});

it('walks a table tab back and forward through the places it showed', () => {
  const state = five();
  const tab = state.tabs[0];
  if (!tab) throw new Error('no tab');
  const place = {
    ...scope,
    table: { database: 'orders', schema: 'x', name: 'y' },
  };
  const visit = visitStep(tab, place);
  expect(visit?.back).toHaveLength(1);
  const moved = replaceWithPlace(
    state,
    tab.id,
    visit ?? { place, back: [], forward: [] },
  );
  const shown = moved.tabs[0];
  expect(shown).toMatchObject({ title: 'y', objectPath: ['x', 'y'] });
  expect(shown?.id).not.toBe(tab.id);
  expect(moved.active['a\norders']).toBe(shown?.id);
  const back = shown ? historyStep(shown, 'back') : null;
  expect(back?.place.table.name).toBe('t1');
  expect(back?.forward).toHaveLength(1);
  expect(shown ? historyStep(shown, 'forward') : 'none').toBeNull();
  expect(
    visitStep(tab, { ...scope, table: tab.table ?? place.table }),
  ).toBeNull();
});

it('stores groups with their tabs and drops unknown or empty ones', () => {
  const [grouped, id] = newGroup(five(), ['table-1', 'table-2']);
  let state = grouped;
  state = changeGroup(state, id, {
    name: 'Geo',
    colour: 'pink',
    collapsed: true,
  });
  state = openTab(state, WINDOW_SCOPE, {
    flow: { mode: 'new' },
    title: 'New connection',
  });
  const restored = restoreTabs(serializeTabs(state));
  expect(restored.groups?.[id]).toEqual({
    id,
    name: 'Geo',
    colour: 'pink',
    collapsed: true,
  });
  expect(restored.tabs.filter((tab) => tab.group === id)).toHaveLength(2);
  expect(restored.tabs.some((tab) => tab.kind === 'connection')).toBe(false);
  const text = serializeTabs(ungroup(state, id));
  expect(restoreTabs(text).groups).toBeUndefined();
});

it('the connection flow is one window tab, current over the browsed scope', () => {
  let state = openTab(five(), WINDOW_SCOPE, {
    flow: { mode: 'new' },
    title: 'New connection',
  });
  const flow = state.tabs.at(-1);
  expect(flow).toMatchObject({ kind: 'connection', connectionId: '' });
  expect(state.current).toBe(flow?.id);
  expect(
    openTab(state, WINDOW_SCOPE, { flow: { mode: 'new' } }).tabs,
  ).toHaveLength(6);
  state = selectTab(state, 'table-2');
  expect(state.current).toBeUndefined();
  state = selectTab(state, flow?.id ?? '');
  expect(state.current).toBe(flow?.id);
});
