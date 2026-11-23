// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { WINDOW_SCOPE, emptyTabs, openTab } from '../query/tabs';
import type { TabsState } from '../query/tabs';
import { changeGroup, newGroup } from '../query/tabGroups';
import {
  outsideScope,
  shownTab,
  stepFrom,
  stripTabs,
  tabAtKey,
  visibleTabs,
} from './windowTabs';
import { shortcut } from './shortcuts';

const a = { connectionId: 'a', database: 'orders' };
const b = { connectionId: 'b', database: 'shop' };

/** Two tabs of `a`, one of `b`, one local file and one of `a` again. */
function mixed(): TabsState {
  let state = openTab(emptyTabs, a);
  state = openTab(state, a);
  state = openTab(state, b);
  state = openTab(state, { connectionId: 'local-files', database: 'Files' });
  return openTab(state, a);
}

it('the strip lists every scope but launch-time files, in window order', () => {
  expect(stripTabs(mixed()).map((tab) => tab.id)).toEqual([
    'query-1',
    'query-2',
    'query-3',
    'query-5',
  ]);
});

it('⌘1–8 count shown tabs, ⌘9 is the last, collapsed groups are skipped', () => {
  const [grouped, id] = newGroup(mixed(), ['query-2']);
  let state = grouped;
  expect(tabAtKey(state, 2)?.id).toBe('query-2');
  state = changeGroup(state, id, { collapsed: true });
  expect(visibleTabs(state).map((tab) => tab.id)).toEqual([
    'query-1',
    'query-3',
    'query-5',
  ]);
  expect(tabAtKey(state, 2)?.id).toBe('query-3');
  expect(tabAtKey(state, 9)?.id).toBe('query-5');
  expect(tabAtKey(state, 7)).toBeUndefined();
  expect(stepFrom(state, 'query-5', 1)?.id).toBe('query-1');
  expect(stepFrom(state, 'query-1', -1)?.id).toBe('query-5');
});

it('shows the connection flow over the scope, else the scope’s active tab', () => {
  let state = mixed();
  expect(shownTab(state, a)?.id).toBe('query-5');
  expect(shownTab(state, b)?.id).toBe('query-3');
  expect(shownTab(state, undefined)).toBeUndefined();
  state = openTab(state, WINDOW_SCOPE, { flow: { mode: 'new' } });
  const flow = shownTab(state, a);
  expect(flow?.kind).toBe('connection');
  expect(flow && outsideScope(flow, a)).toBe(false);
  expect(state.tabs[2] && outsideScope(state.tabs[2], a)).toBe(true);
});

it('⌘digits pick tabs, ⌥⌘digits connections and ⌃Tab walks', () => {
  const key = {
    key: '3',
    code: 'Digit3',
    metaKey: true,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    isComposing: false,
  };
  expect(shortcut(key)).toBe('select-tab-3');
  expect(shortcut({ ...key, altKey: true, key: '£' })).toBe(2);
  expect(
    shortcut({
      ...key,
      key: 'Tab',
      code: 'Tab',
      metaKey: false,
      ctrlKey: true,
    }),
  ).toBe('next-tab');
});
