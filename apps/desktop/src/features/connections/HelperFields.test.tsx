// SPDX-License-Identifier: Apache-2.0
import { screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { createProfile, dumpTools, pickConnectionFile } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { continueButton, mountFlow, saveFlow } from './flow/flowTestKit';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
async function mount() {
  const { user } = await mountFlow({ engine: 'postgres' });
  return user;
}
/** Continue to Security and choose a route card and its hop. */
async function route(
  user: Awaited<ReturnType<typeof mount>>,
  card: RegExp,
  hop: string,
): Promise<void> {
  await user.type(screen.getByRole('textbox', { name: 'User' }), 'app');
  await user.click(continueButton());
  await user.click(screen.getByRole('radio', { name: card }));
  await user.selectOptions(screen.getByRole('combobox', { name: 'Hop' }), hop);
}
it('saves a structured kubectl route and shows missing tool advice inline', async () => {
  vi.mocked(dumpTools).mockResolvedValue([
    {
      tool: 'kubectl',
      name: 'kubectl',
      path: null,
      version: null,
      fromSettings: false,
      warning: null,
      error: 'kubectl was not found. Check Settings ▸ Tools.',
      others: [],
    },
  ]);
  vi.mocked(createProfile).mockResolvedValue(profile('routed'));
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'routed');
  await route(user, /^Cloud provider/, 'kubectl');
  await user.type(screen.getByRole('textbox', { name: 'Context' }), 'test');
  await user.type(
    screen.getByRole('textbox', {
      name: 'Resource (pod/name, svc/name, deployment/name)',
    }),
    'svc/db',
  );
  expect(
    await screen.findByText('kubectl was not found. Check Settings ▸ Tools.'),
  ).toBeVisible();
  await saveFlow(user);
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      helper: {
        kind: 'kubectl',
        context: 'test',
        namespace: 'default',
        resource: 'svc/db',
        remotePort: 5432,
        kubeconfig: null,
      },
    }),
  );
});
it('defaults Google proxy database TLS to off with an encryption hint', async () => {
  const user = await mount();
  await route(user, /^Cloud provider/, 'cloud');
  expect(screen.getByText(/auth proxy encrypts traffic/)).toBeVisible();
  expect(screen.getByRole('combobox', { name: 'TLS mode' })).toHaveValue(
    'disable',
  );
});

it('picks a custom program and adds, reorders, removes and saves literal argument rows', async () => {
  vi.mocked(pickConnectionFile).mockResolvedValue('/tools/hop');
  vi.mocked(createProfile).mockResolvedValue(profile('custom'));
  const user = await mount();
  await user.type(screen.getByRole('textbox', { name: 'Name' }), 'custom');
  await route(user, /^Zero-trust/, 'custom');
  await user.click(screen.getByRole('button', { name: 'Choose program' }));
  expect(await screen.findByRole('textbox', { name: 'Program' })).toHaveValue(
    '/tools/hop',
  );
  await user.click(screen.getByRole('button', { name: 'Add argument' }));
  await user.type(
    screen.getByRole('textbox', { name: 'Argument 2' }),
    'a;b $(x)',
  );
  await user.click(screen.getByRole('button', { name: 'Move argument 2 up' }));
  expect(screen.getByRole('textbox', { name: 'Argument 1' })).toHaveValue(
    'a;b $(x)',
  );
  await user.click(
    screen.getByRole('button', { name: 'Move argument 1 down' }),
  );
  await user.click(screen.getByRole('button', { name: 'Add argument' }));
  await user.click(screen.getByRole('button', { name: 'Remove argument 3' }));
  await user.type(
    screen.getByRole('spinbutton', { name: 'Readiness timeout (seconds)' }),
    '45',
  );
  await saveFlow(user);
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      helper: {
        kind: 'custom',
        program: '/tools/hop',
        args: ['{local_port}', 'a;b $(x)'],
        readyTimeoutSeconds: 45,
      },
    }),
  );
});
