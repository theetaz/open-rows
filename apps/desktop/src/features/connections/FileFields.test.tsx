// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { createProfile, pickConnectionFile } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile } from '../../test/ipcStub';
import { ConnectionSheet } from './ConnectionSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

function mount(engine: 'sqlite' | 'mysql') {
  const onSaved = vi.fn();
  render(
    <ThemeProvider>
      <ConnectionSheet
        engine={engine}
        onClose={vi.fn()}
        onSaved={onSaved}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
  return { user: userEvent.setup(), onSaved };
}

it('a SQLite connection names a file instead of a server and route', async () => {
  vi.mocked(pickConnectionFile).mockResolvedValueOnce('/data/new.sqlite');
  vi.mocked(createProfile).mockResolvedValue(profile('new'));
  const { user, onSaved } = mount('sqlite');
  expect(
    screen.getByRole('dialog', { name: 'New SQLite connection' }),
  ).toBeInTheDocument();
  expect(screen.queryByRole('textbox', { name: 'Host' })).toBeNull();
  expect(screen.queryByText(/^SSH/)).toBeNull();
  expect(screen.queryByText(/^TLS/)).toBeNull();
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
  await user.click(screen.getByRole('button', { name: 'Save' }));
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
  expect(onSaved).toHaveBeenCalled();
});

it('the in-memory switch hides the file controls and syncs the URL', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('scratch'));
  const { user } = mount('sqlite');
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
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      file: { path: '', memory: true, readOnly: false, create: false },
    }),
  );
});

it('a MySQL connection starts on port 3306 with a mysql:// URL', () => {
  mount('mysql');
  expect(
    screen.getByRole('dialog', { name: 'New MySQL connection' }),
  ).toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: 'Port' })).toHaveValue('3306');
  expect(screen.getByRole('textbox', { name: 'URL' })).toHaveValue(
    'mysql://localhost:3306',
  );
});
