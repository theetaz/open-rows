// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  cancelDump,
  dumpContext,
  pickBackupFile,
  previewBackup,
  startBackup,
} from '@openrows/ipc';
import type { DumpEventDto } from '@openrows/ipc';
import { dumpContextOf, tool } from '../../test/dumpStub';
import { JobsButton } from '../jobs';
import { DumpHost } from './DumpHost';
import { showDump } from './dumpStore';
import type { DumpTarget } from './dumpStore';
import { openBackup } from './openDump';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const target: DumpTarget = {
  sessionId: 'session-1',
  profileId: 'id-local',
  connection: 'Local',
  database: 'shop',
  serverProduct: 'PostgreSQL',
  serverVersion: '17.2',
};

let emit: (event: DumpEventDto) => void = () => undefined;

beforeEach(() => {
  vi.mocked(startBackup).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    return Promise.resolve({ jobId: 'dump-1' });
  });
});

async function open(): Promise<HTMLElement> {
  render(
    <ThemeProvider>
      <JobsButton />
      <DumpHost />
    </ThemeProvider>,
  );
  act(() => {
    openBackup(target);
  });
  return screen.findByRole('dialog', { name: 'Back Up shop' });
}

it('shows the tool version, the options and the exact command before the save panel', async () => {
  const user = userEvent.setup();
  const sheet = await open();
  expect(dumpContext).toHaveBeenCalledWith({
    sessionId: 'session-1',
    serverProduct: 'PostgreSQL',
    serverVersion: '17.2',
  });
  const tools = await within(sheet).findByRole('list', {
    name: 'Client tools',
  });
  expect(within(tools).getByText('PostgreSQL 17.4')).toBeInTheDocument();
  expect(await within(sheet).findByText('pg_dump shop')).toBeInTheDocument();
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'Format' }),
    'tar',
  );
  expect(
    within(sheet).getByRole('combobox', { name: 'Compression' }),
  ).toBeDisabled();
  await user.click(within(sheet).getByRole('checkbox', { name: /no-owner/ }));
  expect(previewBackup).toHaveBeenLastCalledWith({
    scope: expect.objectContaining({ sessionId: 'session-1' }) as unknown,
    options: expect.objectContaining({
      format: 'tar',
      noOwner: true,
      compression: null,
    }) as unknown,
    output: null,
  });
  act(() => {
    showDump(null);
  });
});

it('runs the backup as a job with its log, progress and cancel', async () => {
  const user = userEvent.setup();
  vi.mocked(pickBackupFile).mockResolvedValueOnce('/tmp/shop.dump');
  const sheet = await open();
  await within(sheet).findByText('pg_dump shop');
  await user.click(within(sheet).getByRole('button', { name: 'Back Up…' }));
  expect(pickBackupFile).toHaveBeenCalledWith({
    sessionId: 'session-1',
    format: 'custom',
  });
  expect(startBackup).toHaveBeenCalledWith(
    expect.objectContaining({ output: '/tmp/shop.dump' }),
    expect.any(Function),
  );
  const log = await screen.findByRole('dialog', { name: 'Back up shop' });
  act(() => {
    emit({ type: 'step', index: 0, command: 'pg_dump --verbose shop' });
    emit({ type: 'log', lines: ['pg_dump: dumping contents of table'] });
    emit({
      type: 'progress',
      objects: 3,
      total: null,
      bytes: 2048,
      totalBytes: null,
    });
  });
  expect(within(log).getByText('pg_dump --verbose shop')).toBeInTheDocument();
  expect(within(log).getByLabelText('Log')).toHaveTextContent(
    'dumping contents of table',
  );
  expect(within(log).getByText('3 objects · 2.0 KB')).toBeInTheDocument();
  await user.click(within(log).getByRole('button', { name: 'Cancel Backup' }));
  expect(cancelDump).toHaveBeenCalledWith('dump-1');
  act(() => {
    emit({
      type: 'finished',
      outcome: 'completed',
      exitCode: null,
      message: null,
      output: '/tmp/shop.dump',
      elapsedMs: 4000,
    });
  });
  expect(within(log).getByRole('status')).toHaveTextContent(
    'Backed up 3 objects · 2.0 KB in 0:04 · /tmp/shop.dump',
  );
  await user.click(screen.getByRole('button', { name: /^Background jobs/ }));
  expect(
    (await screen.findAllByRole('listitem', { name: 'Back up shop' })).length,
  ).toBeGreaterThan(0);
  act(() => {
    showDump(null);
  });
});

it('names a missing tool and does not offer the backup', async () => {
  vi.mocked(dumpContext).mockResolvedValueOnce(
    dumpContextOf({
      tools: [
        tool({ path: null, version: null, error: 'pg_dump was not found' }),
      ],
    }),
  );
  const sheet = await open();
  expect(await within(sheet).findByRole('alert')).toHaveTextContent(
    'pg_dump was not found',
  );
  expect(
    within(sheet).getByRole('button', { name: 'Back Up…' }),
  ).toBeDisabled();
  act(() => {
    showDump(null);
  });
});

it('warns when the installed tool is older than the server', async () => {
  vi.mocked(dumpContext).mockResolvedValueOnce(
    dumpContextOf({
      tools: [tool({ warning: 'pg_dump 14 cannot back up PostgreSQL 17' })],
    }),
  );
  const sheet = await open();
  expect(
    await within(sheet).findByText('pg_dump 14 cannot back up PostgreSQL 17'),
  ).toBeInTheDocument();
  act(() => {
    showDump(null);
  });
});
