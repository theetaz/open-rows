// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it } from 'vitest';
import {
  arrange,
  emptyLayout,
  moved,
  pinnedCount,
  togglePin,
  widthOf,
  withWidth,
} from './columnLayout';
import {
  readLayout,
  resetLayout,
  tableLayoutKey,
  togglePinned,
  writeLayout,
} from './columnLayoutStore';

const names = ['id', 'name', 'email', 'created'];

beforeEach(() => {
  localStorage.clear();
});

it('arranges pinned columns first, then the stored order, then new columns', () => {
  expect(arrange(emptyLayout, names)).toEqual(names);
  const layout = {
    widths: {},
    order: ['email', 'gone', 'id'],
    pinned: ['created'],
  };
  expect(arrange(layout, names)).toEqual(['created', 'email', 'id', 'name']);
  expect(pinnedCount(layout, arrange(layout, names))).toBe(1);
  // A pin that is not leading (an older page) is not a pinned prefix.
  expect(pinnedCount(layout, names)).toBe(0);
});

it('moves columns within their pinned or unpinned region', () => {
  const pinned = togglePin(emptyLayout, 'email');
  const shown = arrange(pinned, names);
  expect(shown).toEqual(['email', 'id', 'name', 'created']);
  const right = moved(pinned, shown, 3, 1);
  expect(arrange(right, names)).toEqual(['email', 'created', 'id', 'name']);
  // Dropping an unpinned column onto the pinned region keeps it unpinned.
  expect(arrange(moved(pinned, shown, 2, 0), names)).toEqual([
    'email',
    'name',
    'id',
    'created',
  ]);
  // A pinned column cannot leave the pinned region.
  expect(arrange(moved(pinned, shown, 0, 3), names)).toEqual(shown);
  expect(moved(pinned, shown, 9, 0)).toBe(pinned);
});

it('unpins to the front of the unpinned columns and stores widths', () => {
  const layout = togglePin(togglePin(emptyLayout, 'name'), 'created');
  expect(arrange(layout, names)).toEqual(['name', 'created', 'id', 'email']);
  const unpinned = togglePin(layout, 'name');
  expect(arrange(unpinned, names)).toEqual(['created', 'name', 'id', 'email']);
  const sized = withWidth(emptyLayout, 'id', 12.4);
  expect(widthOf(sized, 'id', 90)).toBe(40);
  expect(widthOf(withWidth(sized, 'id', 151.6), 'id', 90)).toBe(152);
  expect(widthOf(sized, 'name', 90)).toBe(90);
});

it('persists layouts per table and forgets reset ones', () => {
  const key = tableLayoutKey({
    profileId: 'p',
    database: 'd',
    schema: 's',
    table: 't',
  });
  expect(readLayout(key)).toBe(emptyLayout);
  writeLayout(key, withWidth(emptyLayout, 'id', 200));
  togglePinned(key, 'name');
  expect(readLayout(key)).toMatchObject({
    widths: { id: 200 },
    pinned: ['name'],
  });
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toContain(
    '"id":200',
  );
  resetLayout(key);
  expect(readLayout(key)).toBe(emptyLayout);
  localStorage.setItem(
    'openrows.columnLayouts.v1',
    '{"k":{"widths":{"a":-1,"b":80},"order":[1,"x"]}}',
  );
  expect(readLayout('k')).toMatchObject({
    widths: { b: 80 },
    order: ['x'],
    pinned: [],
  });
  localStorage.setItem('openrows.columnLayouts.v1', 'not json');
  expect(readLayout('k')).toBe(emptyLayout);
});
