// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { ConnectionSheet } from './ConnectionSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('offers explicit HTTP/TLS modes and changes only default ports', async () => {
  render(
    <ThemeProvider>
      <ConnectionSheet
        engine="clickhouse"
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
  const user = userEvent.setup();
  expect(
    screen.getByRole('heading', { name: 'New ClickHouse connection' }),
  ).toBeVisible();
  const port = screen.getByRole('textbox', { name: 'Port' });
  expect(port).toHaveValue('8123');
  await user.click(screen.getByRole('button', { name: /^TLS/ }));
  const mode = screen.getByRole('combobox', { name: 'TLS mode' });
  expect(mode).toHaveValue('disable');
  expect(screen.queryByRole('option', { name: /Prefer/ })).toBeNull();
  await user.selectOptions(mode, 'verify-full');
  expect(port).toHaveValue('8443');
  await user.selectOptions(mode, 'disable');
  expect(port).toHaveValue('8123');
  await user.clear(port);
  await user.type(port, '9443');
  await user.selectOptions(mode, 'require');
  expect(port).toHaveValue('9443');
});
