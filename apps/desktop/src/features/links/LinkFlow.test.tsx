// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { executeQuery, openConnection, takeNativeFiles } from '@openrows/ipc';
import type { NativeFileDto } from '@openrows/ipc';
import { Workspace } from '../workspace/Workspace';
import '../table/TableGrid';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const link: NativeFileDto = {
  kind: 'link',
  link: {
    kind: 'table',
    connection: { id: null, name: 'orders-local' },
    database: 'orders',
    schema: 'public',
    object: { name: 'customers', kind: 'table' },
    filters: [
      { column: 'status', operator: 'equal', value: 'paid', second: '' },
    ],
  },
};

async function receive(): Promise<ReturnType<typeof userEvent.setup>> {
  vi.mocked(takeNativeFiles).mockResolvedValueOnce([link]);
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  return userEvent.setup({ skipHover: true });
}

it('previews a link and connects nothing until the user confirms', async () => {
  const user = await receive();
  const sheet = await screen.findByRole('dialog', { name: 'Open link?' });
  const details = within(sheet).getByLabelText('Link details');
  expect(details).toHaveTextContent('orders-local');
  expect(details).toHaveTextContent('status = paid');
  expect(openConnection).not.toHaveBeenCalled();
  await user.click(within(sheet).getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog', { name: 'Open link?' })).toBeNull();
  expect(openConnection).not.toHaveBeenCalled();
});

it('opens the linked table after confirmation and never runs SQL', async () => {
  const user = await receive();
  const sheet = await screen.findByRole('dialog', { name: 'Open link?' });
  await user.click(
    within(sheet).getByRole('button', { name: 'Connect and Open' }),
  );
  expect(vi.mocked(openConnection).mock.calls[0]?.[0]).toBe('id-orders-local');
  expect(await screen.findByRole('tab', { name: /customers/ })).toBeVisible();
  expect(executeQuery).not.toHaveBeenCalled();
});
