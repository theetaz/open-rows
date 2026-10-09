// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  attachDatabase,
  detachDatabase,
  pickConnectionFile,
} from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile } from '../../test/ipcStub';
import type { Connections } from '../connections';
import { AttachDatabaseTools } from './AttachDatabaseTools';
import type { Catalog } from './useCatalog';

vi.mock('@openrows/ipc', () => ({
  attachDatabase: vi.fn(),
  detachDatabase: vi.fn(),
  pickConnectionFile: vi.fn(),
}));

function sqlite(attached: { alias: string; path: string }[]): ProfileDto {
  return profile('lite', {
    engine: 'sqlite',
    file: {
      path: '/data/app.db',
      memory: false,
      readOnly: false,
      create: false,
      attached,
    },
  });
}

function mount(active: ProfileDto, schema = 'main') {
  const connections = { active, saved: vi.fn() };
  const catalog = {
    offline: null,
    database: 'main',
    schema,
    refresh: vi.fn(),
    setSchema: vi.fn(),
  };
  render(
    <ThemeProvider>
      <AttachDatabaseTools
        connections={connections as unknown as Connections}
        catalog={catalog as unknown as Catalog}
      />
    </ThemeProvider>,
  );
  return { user: userEvent.setup(), connections, catalog };
}

it('attaches a chosen file under an editable, prefilled schema name', async () => {
  const saved = sqlite([{ alias: 'audit', path: '/data/audit log.db' }]);
  vi.mocked(pickConnectionFile).mockResolvedValueOnce('/data/audit log.db');
  vi.mocked(attachDatabase).mockResolvedValueOnce({
    profile: saved,
    warnings: ['An open tab could not follow the change.'],
  });
  const { user, connections, catalog } = mount(sqlite([]));
  expect(screen.queryByRole('button', { name: /^Detach/ })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Attach Database…' }));
  expect(pickConnectionFile).toHaveBeenCalledWith('sqliteDatabase');
  const name = await screen.findByRole('textbox', { name: 'Schema name' });
  expect(name).toHaveValue('audit_log');
  await user.clear(name);
  await user.type(name, 'audit');
  await user.click(screen.getByRole('button', { name: 'Attach' }));
  expect(attachDatabase).toHaveBeenCalledWith(
    'id-lite',
    '/data/audit log.db',
    'audit',
  );
  expect(await screen.findByRole('status')).toHaveTextContent(
    'could not follow',
  );
  expect(connections.saved).toHaveBeenCalledWith(saved);
  expect(catalog.refresh).toHaveBeenCalled();
  expect(catalog.setSchema).toHaveBeenCalledWith('audit');
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('shows a refused schema name in the sheet and keeps it open', async () => {
  vi.mocked(pickConnectionFile).mockResolvedValueOnce('/data/main.db');
  vi.mocked(attachDatabase).mockRejectedValueOnce(
    new Error('Schema name cannot be main or temp'),
  );
  const { user } = mount(sqlite([]));
  await user.click(screen.getByRole('button', { name: 'Attach Database…' }));
  const name = await screen.findByRole('textbox', { name: 'Schema name' });
  expect(name).toHaveValue('main_2');
  await user.clear(name);
  await user.type(name, 'main');
  await user.click(screen.getByRole('button', { name: 'Attach' }));
  expect(
    await screen.findByText('Schema name cannot be main or temp'),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('detaches the selected attached schema but never main', async () => {
  const before = sqlite([{ alias: 'logs', path: '/data/logs.db' }]);
  vi.mocked(detachDatabase).mockResolvedValueOnce({
    profile: sqlite([]),
    warnings: [],
  });
  const { user, catalog } = mount(before, 'logs');
  await user.click(screen.getByRole('button', { name: 'Detach logs' }));
  expect(detachDatabase).toHaveBeenCalledWith('id-lite', 'logs');
  expect(catalog.refresh).toHaveBeenCalled();
});

it('offers nothing for other engines', () => {
  mount(profile('pg'));
  expect(screen.queryByRole('button', { name: 'Attach Database…' })).toBeNull();
});
