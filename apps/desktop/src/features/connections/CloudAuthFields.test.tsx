// SPDX-License-Identifier: Apache-2.0
import { screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { createProfile } from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { mountFlow, saveFlow } from './flow/flowTestKit';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
async function mount(editing?: ProfileDto) {
  const { user } = await mountFlow(
    editing ? { profile: editing } : { engine: 'postgres' },
  );
  return user;
}
const storage = (): HTMLElement =>
  screen.getByRole('combobox', { name: 'Password storage' });
const choices = (select: HTMLElement): (string | null)[] =>
  Array.from(select.querySelectorAll('option')).map((o) => o.textContent);

it('saves only the provider settings and warns until TLS is verified', async () => {
  vi.mocked(createProfile).mockResolvedValue(profile('iam'));
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'iam');
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'iam_user');
  await user.selectOptions(storage(), 'cloud');
  expect(screen.getByRole('alert')).toHaveTextContent('verified TLS');
  expect(
    screen.queryByRole('textbox', { name: 'Password' }),
  ).not.toBeInTheDocument();
  await user.type(screen.getByRole('textbox', { name: 'Region' }), 'us-east-1');
  await user.type(screen.getByRole('textbox', { name: 'AWS profile' }), 'dev');
  await saveFlow(user);
  const [input] = vi.mocked(createProfile).mock.calls.at(-1) ?? [];
  expect(input?.passwordStorage).toBe('cloud');
  expect(input?.cloudAuth).toEqual({
    provider: 'aws',
    region: 'us-east-1',
    profile: 'dev',
  });
  expect(input?.password).toEqual({ kind: 'clear' });
});

it('swaps the settings with the provider and hides the warning when verified', async () => {
  const user = await mount(
    profile('saved', {
      engine: 'postgres',
      passwordStorage: 'cloud',
      tlsMode: 'verify-full',
      cloudAuth: { provider: 'azure', tenant: 'contoso.example' },
    }),
  );
  expect(screen.getByRole('textbox', { name: 'Tenant' })).toHaveValue(
    'contoso.example',
  );
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Cloud provider' }),
    'google',
  );
  expect(screen.getByRole('textbox', { name: 'Account' })).toHaveValue('');
  expect(
    screen.queryByRole('textbox', { name: 'Tenant' }),
  ).not.toBeInTheDocument();
});

it('offers cloud tokens only for engines that accept them', async () => {
  await mount(profile('m', { engine: 'mariadb' }));
  expect(choices(storage())).not.toContain('Cloud IAM token');
});

it('offers only Entra ID on SQL Server', async () => {
  await mount(
    profile('s', {
      engine: 'sqlserver',
      passwordStorage: 'cloud',
      tlsMode: 'verify-full',
      cloudAuth: { provider: 'azure', tenant: null },
    }),
  );
  expect(choices(storage())).toContain('Cloud IAM token');
  expect(
    choices(screen.getByRole('combobox', { name: 'Cloud provider' })),
  ).toEqual(['Microsoft Entra ID']);
});
