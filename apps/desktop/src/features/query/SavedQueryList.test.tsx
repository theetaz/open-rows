// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  deleteSavedQuery,
  listSavedQueries,
  renameSavedQuery,
} from '@openrows/ipc';
import { SavedQueryList } from './SavedQueryList';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

function renderList(): {
  user: ReturnType<typeof userEvent.setup>;
  onOpen: ReturnType<typeof vi.fn>;
} {
  const onOpen = vi.fn();
  render(<SavedQueryList profileId="id-local" search="" onOpen={onOpen} />);
  return { user: userEvent.setup(), onOpen };
}

it('lists the connection’s saved queries and opens one on click', async () => {
  const { user, onOpen } = renderList();
  await user.click(
    await screen.findByRole('button', { name: 'Top customers' }),
  );
  expect(listSavedQueries).toHaveBeenCalledWith('id-local');
  expect(onOpen).toHaveBeenCalledWith(
    expect.objectContaining({ id: 'saved-1', name: 'Top customers' }),
  );
});

it('renames from the context menu and reloads the list', async () => {
  const { user } = renderList();
  const row = await screen.findByRole('button', { name: 'Top customers' });
  fireEvent.contextMenu(row);
  await user.click(screen.getByRole('menuitem', { name: /Rename/ }));
  const name = screen.getByRole('textbox', { name: 'Name' });
  await user.clear(name);
  await user.type(name, 'Best customers{Enter}');
  expect(renameSavedQuery).toHaveBeenCalledWith('saved-1', 'Best customers');
  expect(listSavedQueries).toHaveBeenCalledTimes(2);
});

it('deletes after confirmation in the in-app alert', async () => {
  const { user } = renderList();
  fireEvent.contextMenu(
    await screen.findByRole('button', { name: 'Top customers' }),
  );
  await user.click(screen.getByRole('menuitem', { name: /Delete/ }));
  const alert = await screen.findByRole('dialog', {
    name: 'Delete “Top customers”?',
  });
  await user.click(within(alert).getByRole('button', { name: 'Delete' }));
  expect(deleteSavedQuery).toHaveBeenCalledWith('saved-1');
});
