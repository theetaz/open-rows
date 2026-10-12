// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { updateTableSettings } from '../settings/tableSettings';
import { emptyEntry } from './entries';
import { countKey, initialView } from './model';
import type { TableViews } from './useTableViews';
import { tableRequest, useExactCount } from './tableRequest';
import { emptyLayout } from '../../results/columnLayout';

const target = { profileId: 'p', database: 'db', schema: 'public', table: 't' };
const request = () =>
  tableRequest(
    target,
    { ...initialView, search: 'ann', hidden: ['name'] },
    ['id', 'name'],
    { rowId: null, layout: emptyLayout },
  );

it('keeps search independent of column visibility and preserves the table filter join', () => {
  expect(request()).toMatchObject({
    search: 'ann',
    columns: ['id'],
    combinator: 'and',
  });
});

it('counts only in exact mode with a ready structure and recounts on search or reload', () => {
  let revision = 0;
  const count = vi.fn();
  const views = {
    count,
    entryOf: () => ({ ...emptyEntry, view: { ...initialView, revision } }),
  } as unknown as TableViews;
  const initial = {
    query: request(),
    structure: 'loading' as 'loading' | 'ready',
  };
  const hook = renderHook(
    ({ query, structure }) => {
      useExactCount({ tabId: 't', views, sessionId: 's', query }, structure);
    },
    { initialProps: initial },
  );
  act(() => {
    updateTableSettings({ rowCount: 'exact' });
  });
  expect(count).not.toHaveBeenCalled();
  hook.rerender({ ...initial, structure: 'ready' });
  expect(count).toHaveBeenLastCalledWith('t', countKey(initial.query), {
    sessionId: 's',
    query: initial.query,
  });
  hook.rerender({ ...initial, structure: 'ready' });
  expect(count).toHaveBeenCalledOnce();
  revision = 1;
  hook.rerender({ ...initial, structure: 'ready' });
  expect(count).toHaveBeenCalledTimes(2);
  hook.rerender({
    query: { ...initial.query, search: 'bob' },
    structure: 'ready',
  });
  expect(count).toHaveBeenCalledTimes(3);
  act(() => {
    updateTableSettings({ rowCount: 'estimate' });
  });
  hook.rerender({ ...initial, structure: 'ready' });
  expect(count).toHaveBeenCalledTimes(3);
});
