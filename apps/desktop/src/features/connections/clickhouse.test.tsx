// SPDX-License-Identifier: Apache-2.0
import { screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { continueButton, mountFlow, openStep } from './flow/flowTestKit';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('offers explicit HTTP/TLS modes and changes only default ports', async () => {
  const { user } = await mountFlow({ engine: 'clickhouse' });
  expect(
    screen.getByRole('heading', { name: 'ClickHouse details' }),
  ).toBeVisible();
  const port = (): HTMLElement => screen.getByRole('textbox', { name: 'Port' });
  expect(port()).toHaveValue('8123');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'default');
  await user.click(continueButton());
  const mode = screen.getByRole('combobox', { name: 'TLS mode' });
  expect(mode).toHaveValue('disable');
  expect(screen.queryByRole('option', { name: /Prefer/ })).toBeNull();
  await user.selectOptions(mode, 'verify-full');
  expect(
    screen.getByRole('complementary', { name: 'Summary' }),
  ).toHaveTextContent(':8443');
  await user.selectOptions(mode, 'disable');
  await openStep(user, 'Details');
  expect(port()).toHaveValue('8123');
  await user.clear(port());
  await user.type(port(), '9443');
  await openStep(user, 'Security');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'TLS mode' }),
    'require',
  );
  await openStep(user, 'Details');
  expect(port()).toHaveValue('9443');
});
