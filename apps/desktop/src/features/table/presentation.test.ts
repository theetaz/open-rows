// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { initialView } from './model';
import {
  parsePresentation,
  presentationOf,
  restoredView,
  withSavedFilter,
} from './presentation';
import {
  readPresentation,
  useSavedFilters,
  writePresentation,
} from './presentationStore';

const KEY = '["table","p","db","public","users"]';

const row = {
  id: 4,
  enabled: true,
  column: 'name',
  operator: 'contains' as const,
  value: 'ann',
  second: '',
};

beforeEach(() => {
  localStorage.clear();
});

it('remembers hidden columns, sort and filters per table across sessions', () => {
  const view = {
    ...initialView,
    hidden: ['secret'],
    sort: { column: 'id', descending: true },
    rows: [row],
    applied: [
      {
        column: 'name',
        operator: 'contains' as const,
        value: 'ann',
        second: '',
      },
    ],
    combinator: 'or' as const,
  };
  writePresentation(KEY, presentationOf(view, []));
  const stored = readPresentation(KEY);
  const restored = restoredView(initialView, stored);
  expect(restored).toMatchObject({
    hidden: ['secret'],
    sort: { column: 'id', descending: true },
    rows: [row],
    combinator: 'or',
    filtersOpen: true,
    restored: true,
    nextRow: 5,
  });
  expect(readPresentation('["table","p","db","public","other"]').rows).toEqual(
    [],
  );
  writePresentation(KEY, presentationOf(initialView, []));
  expect(localStorage.getItem('openrows.tablePresentation.v1')).toBe('{}');
});

it('drops malformed stored values field by field', () => {
  expect(parsePresentation('x')).toBeNull();
  expect(
    parsePresentation({
      hidden: ['a', 3],
      sort: { column: 7 },
      rows: [row, { ...row, operator: 'drop table' }, { column: 'x' }],
      applied: [{ column: 'a', operator: 'raw', value: 'id > 1' }],
      combinator: 'xor',
      saved: [{ name: ' ' }, { name: 'Recent', rows: [row], combinator: 'or' }],
    }),
  ).toEqual({
    hidden: ['a'],
    sort: null,
    rows: [row],
    applied: [{ column: 'a', operator: 'raw', value: 'id > 1', second: '' }],
    combinator: 'and',
    saved: [{ name: 'Recent', rows: [row], combinator: 'or' }],
  });
});

it('saves, applies and forgets named filters per table', () => {
  const { result } = renderHook(() => useSavedFilters(KEY));
  act(() => {
    result.current.save({ name: 'Ann', rows: [row], combinator: 'or' });
    result.current.save({ name: 'All', rows: [], combinator: 'and' });
  });
  expect(result.current.saved.map((item) => item.name)).toEqual(['All', 'Ann']);
  const ann = result.current.saved[1];
  if (!ann) throw new Error('missing saved filter');
  const view = withSavedFilter({ ...initialView, nextRow: 9 }, ann);
  expect(view).toMatchObject({
    rows: [{ ...row, id: 9 }],
    applied: [
      { column: 'name', operator: 'contains', value: 'ann', second: '' },
    ],
    combinator: 'or',
    nextRow: 10,
    filtersOpen: true,
  });
  act(() => {
    result.current.remove('Ann');
  });
  expect(result.current.saved.map((item) => item.name)).toEqual(['All']);
});
