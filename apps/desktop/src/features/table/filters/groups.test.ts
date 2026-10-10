// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { filterRow, initialView, tableQuery } from '../model';
import { filterActions } from './filterActions';
import { withQuickFilter } from './groups';
import type { TableView } from '../model';

it('applies inner and outer joins, disables rows and removes a group', () => {
  let view: TableView = {
    ...initialView,
    rows: [filterRow(1, 'id')],
    nextRow: 2,
  };
  const actions = filterActions(
    (update) => {
      view = update(view);
    },
    ['id'],
  );
  actions.editRow(1, { value: '1' });
  actions.addRow(undefined, 1);
  actions.editRow(2, { value: '2' });
  actions.combineGroup(1, 'or');
  actions.addGroup();
  actions.editRow(3, { column: '', operator: 'raw', value: 'id > 0' });
  actions.combine('and');
  actions.apply();
  expect(
    view.appliedGroups?.map((group) => [
      group.combinator,
      group.filters.length,
    ]),
  ).toEqual([
    ['or', 2],
    ['and', 1],
  ]);
  actions.editRow(2, { enabled: false });
  actions.apply();
  expect(view.appliedGroups?.[0]?.filters).toHaveLength(1);
  actions.removeGroup(1);
  actions.apply();
  expect(view.appliedGroups?.[0]?.filters[0]?.operator).toBe('raw');
});

it('quick filters preserve the applied groups and do not apply draft edits', () => {
  const applied = {
    column: 'id',
    operator: 'equal' as const,
    value: '1',
    second: '',
  };
  const view = withQuickFilter(
    {
      ...initialView,
      nextRow: 2,
      combinator: 'and',
      groups: [{ id: 1, combinator: 'or' }],
      rows: [{ ...filterRow(1, 'id'), value: 'draft' }],
      applied: [applied],
      appliedGroups: [{ combinator: 'or', filters: [applied] }],
    },
    { ...filterRow(2, 'name'), value: 'ann' },
  );
  const query = tableQuery(
    { profileId: 'local', database: 'shop', schema: 'public', table: 'items' },
    view,
    ['id', 'name'],
  );
  expect(query.groups?.[0]?.filters).toEqual([applied]);
  expect(query.groups?.[1]?.filters[0]?.value).toBe('ann');
  expect(query.filters).toEqual([]);
});
