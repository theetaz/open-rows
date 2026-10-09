// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { dumpTools, writeSettingsFile } from '@openrows/ipc';
import { tool } from '../../test/dumpStub';
import { SettingsSheet } from './SettingsSheet';
import { parseTools, toolSettings } from './toolSettings';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('shows each tool found with its version and saves a chosen path', async () => {
  vi.mocked(dumpTools).mockResolvedValue([
    tool({ others: ['/usr/local/bin/pg_dump (PostgreSQL 14.17)'] }),
    tool({
      tool: 'mysqldump',
      name: 'mysqldump',
      path: null,
      version: null,
      error: 'mysqldump was not found',
    }),
  ]);
  render(
    <ThemeProvider>
      <SettingsSheet initial="tools" onClose={vi.fn()} />
    </ThemeProvider>,
  );
  expect(
    await screen.findByText(
      'Found: /opt/pg/bin/pg_dump (PostgreSQL 17.4). Also installed: /usr/local/bin/pg_dump (PostgreSQL 14.17).',
    ),
  ).toBeInTheDocument();
  expect(screen.getByText('mysqldump was not found')).toBeInTheDocument();
  const user = userEvent.setup();
  await user.type(
    screen.getByRole('textbox', { name: 'mysqldump path' }),
    '/opt/mysql/bin/mysqldump{Enter}',
  );
  expect(toolSettings.get().mysqldump).toBe('/opt/mysql/bin/mysqldump');
  expect(writeSettingsFile).toHaveBeenCalledWith(
    expect.stringContaining('"mysqldump": "/opt/mysql/bin/mysqldump"'),
  );
  await vi.waitFor(() => {
    expect(dumpTools).toHaveBeenCalledTimes(2);
  });
});

it('keeps only string paths of known tools', () => {
  expect(
    parseTools({ pgDump: ' /bin/pg_dump ', psql: 3, other: '/x' }),
  ).toEqual({
    pgDump: '/bin/pg_dump',
    pgRestore: '',
    psql: '',
    mysqldump: '',
    mysql: '',
    aws: '',
    sessionManagerPlugin: '',
    kubectl: '',
    cloudSqlProxy: '',
    alloydbAuthProxy: '',
    cloudflared: '',
  });
  expect(parseTools(null).pgDump).toBe('');
  expect(parseTools({ mysql: 'x'.repeat(1025) }).mysql).toBe('');
});
