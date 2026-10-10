// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { createProfile } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile } from '../../test/ipcStub';
import { ConnectionSheet } from './ConnectionSheet';
import { PasswordPromptSheet } from './PasswordPromptSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('saves a remote endpoint with its token policy and no user or database', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <ConnectionSheet
        engine="libsql"
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(screen.queryByLabelText('User')).not.toBeInTheDocument();
  expect(screen.queryByLabelText('Database')).not.toBeInTheDocument();
  const token = screen.getByLabelText('Auth token');
  expect(token).toHaveAttribute('type', 'password');
  await user.type(screen.getByLabelText('Name'), 'remote');
  await user.clear(screen.getByLabelText('Host'));
  await user.type(screen.getByLabelText('Host'), 'db.test');
  await user.type(token, 'test-only');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(createProfile).toHaveBeenCalledWith(
    expect.objectContaining({
      engine: 'libsql',
      host: 'db.test',
      port: 443,
      passwordStorage: 'keychain',
      password: { kind: 'set', value: 'test-only' },
      tlsMode: 'verify-full',
      user: null,
      database: null,
    }),
  );
});

it('asks for an auth token using the existing ephemeral credential prompt', async () => {
  const user = userEvent.setup();
  const submit = vi.fn();
  render(
    <ThemeProvider>
      <PasswordPromptSheet
        request={{
          profile: profile('remote', {
            engine: 'libsql',
            passwordStorage: 'ask',
            user: '',
          }),
          submit,
          cancel: vi.fn(),
        }}
      />
    </ThemeProvider>,
  );
  await user.type(screen.getByLabelText('Auth token'), 'test-only');
  await user.click(screen.getByRole('button', { name: 'Connect' }));
  expect(submit).toHaveBeenCalledWith('test-only');
});
