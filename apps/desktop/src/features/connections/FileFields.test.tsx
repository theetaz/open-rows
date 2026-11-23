// SPDX-License-Identifier: Apache-2.0
import { screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { createProfile, pickConnectionFile } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { mountFlow, saveFlow } from './flow/flowTestKit';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('a SQLite connection names a file instead of a server and route', async () => {
  vi.mocked(pickConnectionFile).mockResolvedValueOnce('/data/new.sqlite');
  vi.mocked(createProfile).mockResolvedValue(profile('new'));
  const { user, onDone } = await mountFlow({ engine: 'sqlite' });
  expect(
    screen.getByRole('heading', { name: 'SQLite details' }),
  ).toBeInTheDocument();
  expect(screen.queryByRole('textbox', { name: 'Host' })).toBeNull();
  expect(screen.queryByRole('button', { name: /Security/ })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'New database file' }));
  expect(pickConnectionFile).toHaveBeenCalledWith('newSqliteDatabase');
  expect(
    await screen.findByDisplayValue('/data/new.sqlite'),
  ).toBeInTheDocument();
  expect(
    screen.getByText('The file is created when you connect.'),
  ).toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: 'URL' })).toHaveValue(
    'sqlite:/data/new.sqlite',
  );
  await user.click(screen.getByRole('switch', { name: 'Open read-only' }));
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'Local');
  await saveFlow(user);
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      engine: 'sqlite',
      file: {
        path: '/data/new.sqlite',
        memory: false,
        readOnly: true,
        create: true,
      },
      ssh: null,
    }),
  );
  expect(onDone).toHaveBeenCalled();
});

it('the in-memory switch hides the file controls and syncs the URL', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('scratch'));
  const { user } = await mountFlow({ engine: 'sqlite' });
  const url = screen.getByRole('textbox', { name: 'URL' });
  await user.click(screen.getByRole('switch', { name: 'In-memory' }));
  expect(screen.queryByRole('textbox', { name: 'Database file' })).toBeNull();
  expect(screen.queryByRole('switch', { name: 'Open read-only' })).toBeNull();
  expect(
    screen.queryByRole('button', { name: 'New database file' }),
  ).toBeNull();
  expect(url).toHaveValue('sqlite::memory:');
  await user.click(screen.getByRole('switch', { name: 'In-memory' }));
  expect(
    screen.getByRole('textbox', { name: 'Database file' }),
  ).toBeInTheDocument();
  expect(url).toHaveValue('sqlite:');
  await user.click(screen.getByRole('switch', { name: 'In-memory' }));
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'Scratch');
  await saveFlow(user);
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      file: { path: '', memory: true, readOnly: false, create: false },
    }),
  );
});

it('a file opened from Finder starts at its details, read-only and named after it', async () => {
  await mountFlow({ engine: 'sqlite', filePath: '/data/notes.sqlite' });
  expect(screen.getByText('Step 2 of 4')).toBeVisible();
  expect(screen.getByRole('textbox', { name: 'Database file' })).toHaveValue(
    '/data/notes.sqlite',
  );
  expect(screen.getByRole('switch', { name: 'Open read-only' })).toBeChecked();
  expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue(
    'notes.sqlite',
  );
});
