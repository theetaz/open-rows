// SPDX-License-Identifier: Apache-2.0
import { screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { fakeRuns, renderTab } from './testQueryTab';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => {
  const { TextareaEditor } = await import('./testEditor');
  return { QueryEditor: TextareaEditor };
});

const changed = {
  path: '/work/q.sql',
  name: 'q.sql',
  saved: 'select 1',
  modifiedMs: 2,
  external: 'select 2',
};

it('reloads the text another program saved into the editor and the tab', async () => {
  const { user, tabs } = renderTab(fakeRuns(), { tab: { file: changed } });
  expect(
    screen.getByText('q.sql was changed by another program.'),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Reload' }));
  expect(screen.getByRole('textbox', { name: 'SQL editor' })).toHaveValue(
    'select 2',
  );
  expect(tabs.edit).toHaveBeenCalledWith('query-1', 'select 2');
  expect(tabs.bindFile).toHaveBeenLastCalledWith('query-1', {
    path: '/work/q.sql',
    name: 'q.sql',
    saved: 'select 2',
    modifiedMs: 2,
  });
});

it('keeps the editor text and treats the file text as saved', async () => {
  const { user, tabs } = renderTab(fakeRuns(), { tab: { file: changed } });
  await user.click(screen.getByRole('button', { name: 'Keep Mine' }));
  expect(screen.getByRole('textbox', { name: 'SQL editor' })).toHaveValue(
    'select 1',
  );
  expect(tabs.edit).not.toHaveBeenCalled();
  expect(tabs.bindFile).toHaveBeenLastCalledWith('query-1', {
    path: '/work/q.sql',
    name: 'q.sql',
    saved: 'select 2',
    modifiedMs: 2,
  });
});
