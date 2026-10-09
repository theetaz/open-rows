// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  checkRestore,
  pickRestoreFile,
  restoreContents,
  startRestore,
} from '@openrows/ipc';
import type { PickedDumpDto, RestoreCheckDto } from '@openrows/ipc';
import { DumpHost } from './DumpHost';
import { showDump } from './dumpStore';
import type { DumpTarget } from './dumpStore';
import { pickRestoreFor } from './openDump';

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

const archive: PickedDumpDto = {
  path: '/tmp/shop.dump',
  name: 'shop.dump',
  kind: 'pgCustom',
  bytes: 4096,
};

function check(change: Partial<RestoreCheckDto> = {}): RestoreCheckDto {
  return {
    commands: ['pg_restore --dbname shop /tmp/shop.dump'],
    warnings: [],
    blocked: null,
    database: 'shop',
    typed: false,
    confirmToken: 'token-1',
    ...change,
  };
}

beforeEach(() => {
  vi.mocked(startRestore).mockResolvedValue({ jobId: 'dump-2' });
});

async function open(
  result: RestoreCheckDto,
  file: PickedDumpDto = archive,
): Promise<HTMLElement> {
  vi.mocked(checkRestore).mockResolvedValue(result);
  vi.mocked(pickRestoreFile).mockResolvedValueOnce(file);
  render(
    <ThemeProvider>
      <DumpHost />
    </ThemeProvider>,
  );
  await act(async () => {
    await pickRestoreFor(target);
  });
  return screen.findByRole('dialog', { name: `Restore ${file.name}` });
}

it('lists the archive, restores the chosen entries with the confirmed token', async () => {
  const user = userEvent.setup();
  vi.mocked(restoreContents).mockResolvedValueOnce([
    { id: 1, label: 'TABLE public items app', data: false },
    { id: 2, label: 'VIEW public cheap app', data: false },
  ]);
  const sheet = await open(check());
  expect(restoreContents).toHaveBeenCalledWith({
    scope: expect.objectContaining({ sessionId: 'session-1' }) as unknown,
    path: '/tmp/shop.dump',
  });
  await user.click(
    await within(sheet).findByRole('checkbox', {
      name: 'VIEW public cheap app',
    }),
  );
  expect(checkRestore).toHaveBeenLastCalledWith(
    expect.objectContaining({
      options: expect.objectContaining({ selected: [1] }) as unknown,
    }),
  );
  expect(
    await within(sheet).findByText('pg_restore --dbname shop /tmp/shop.dump'),
  ).toBeInTheDocument();
  await user.click(within(sheet).getByRole('button', { name: 'Restore' }));
  expect(startRestore).toHaveBeenCalledWith(
    expect.objectContaining({
      path: '/tmp/shop.dump',
      target: { kind: 'existing' },
      confirmToken: 'token-1',
      typedName: null,
    }),
    expect.any(Function),
  );
  expect(
    await screen.findByRole('dialog', { name: 'Restore shop' }),
  ).toBeInTheDocument();
  act(() => {
    showDump(null);
  });
});

it('needs the database name typed on production or destructive restores', async () => {
  const user = userEvent.setup();
  const sheet = await open(check({ typed: true }));
  const restore = within(sheet).getByRole('button', { name: 'Restore' });
  const field = await within(sheet).findByRole('textbox', {
    name: 'Type shop to confirm the restore',
  });
  expect(restore).toBeDisabled();
  await user.type(field, 'sho');
  expect(restore).toBeDisabled();
  await user.type(field, 'p');
  await user.click(restore);
  expect(startRestore).toHaveBeenCalledWith(
    expect.objectContaining({ confirmToken: 'token-1', typedName: 'shop' }),
    expect.any(Function),
  );
  act(() => {
    showDump(null);
  });
});

it('never restores on a read-only connection', async () => {
  const sheet = await open(
    check({
      blocked: 'This connection is read-only',
      confirmToken: null,
    }),
    { ...archive, kind: 'plainSql', name: 'shop.sql', path: '/tmp/shop.sql' },
  );
  expect(await within(sheet).findByRole('alert')).toHaveTextContent(
    'read-only',
  );
  expect(within(sheet).getByRole('button', { name: 'Restore' })).toBeDisabled();
  expect(restoreContents).not.toHaveBeenCalled();
  act(() => {
    showDump(null);
  });
});

it('creates a new database first when asked', async () => {
  const user = userEvent.setup();
  const sheet = await open(check());
  await user.click(
    await within(sheet).findByRole('radio', { name: 'A new database' }),
  );
  await user.type(
    within(sheet).getByRole('textbox', { name: 'New database name' }),
    'copy',
  );
  expect(checkRestore).toHaveBeenLastCalledWith(
    expect.objectContaining({ target: { kind: 'create', name: 'copy' } }),
  );
  act(() => {
    showDump(null);
  });
});
