// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { FilterBar } from '../FilterBar';
import { filterRow, initialView, tableQuery } from '../model';
import type { TableView } from '../model';
import { withSavedFilter } from '../presentation';
import type { TableActions } from '../useTableTab';
import { filterActions } from './filterActions';

vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);

function Harness(): React.JSX.Element {
  const [view, changeView] = useState<TableView>({
    ...initialView,
    rows: [filterRow(1, 'id')],
    nextRow: 2,
  });
  const actions = {
    ...filterActions(changeView, ['id']),
    applySaved: (saved: Parameters<TableActions['applySaved']>[0]) => {
      changeView((current) => withSavedFilter(current, saved));
    },
  } as TableActions;
  const query = tableQuery(
    { profileId: 'local', database: 'shop', schema: 'a', table: 'items' },
    view,
    ['id'],
  );
  return (
    <>
      <FilterBar
        view={view}
        actions={actions}
        columns={['id']}
        types={new Map([['id', 'integer']])}
        engine="postgres"
        query={query}
        storageKey="group-ui"
      />
      <output aria-label="Applied groups">
        {JSON.stringify(query.groups ?? [])}
      </output>
    </>
  );
}

it('edits, applies and reloads named groups with raw SQL rows', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Harness />
    </ThemeProvider>,
  );
  await user.type(screen.getByLabelText('Value of filter 1'), '7');
  await user.selectOptions(
    screen.getByLabelText('Combine rows in group 1'),
    'or',
  );
  await user.click(screen.getByRole('button', { name: 'Add group' }));
  await user.selectOptions(screen.getByLabelText('Column of filter 2'), '');
  await user.type(screen.getByLabelText('SQL of filter 2'), 'id > 0');
  await user.selectOptions(screen.getByLabelText('Combine groups'), 'and');
  await user.click(screen.getByRole('button', { name: 'Apply All' }));
  expect(
    JSON.parse(screen.getByLabelText('Applied groups').textContent),
  ).toEqual([
    {
      combinator: 'or',
      filters: [{ column: 'id', operator: 'equal', value: '7', second: '' }],
    },
    {
      combinator: 'and',
      filters: [{ column: '', operator: 'raw', value: 'id > 0', second: '' }],
    },
  ]);
  await user.click(screen.getByRole('button', { name: 'Save…' }));
  await user.type(screen.getByLabelText('Saved filter name'), 'Grouped{Enter}');
  await user.click(screen.getByRole('button', { name: 'Clear' }));
  await user.selectOptions(screen.getByLabelText('Saved filters'), '');
  await user.selectOptions(screen.getByLabelText('Saved filters'), 'Grouped');
  expect(screen.getByLabelText('Combine rows in group 1')).toHaveValue('or');
  expect(screen.getByLabelText('SQL of filter 2')).toHaveValue('id > 0');
  expect(screen.getByLabelText('Applied groups')).toHaveTextContent('id > 0');
});
