// SPDX-License-Identifier: Apache-2.0
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { answerSshPrompt, onSshPrompt } from '@openrows/ipc';
import type { SshPromptChanged } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { SshPrompts } from './SshPromptSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

async function mount(): Promise<(event: SshPromptChanged) => void> {
  let emit: ((event: SshPromptChanged) => void) | undefined;
  vi.mocked(onSshPrompt).mockImplementation((listener) => {
    emit = listener;
    return Promise.resolve(() => {});
  });
  render(
    <ThemeProvider>
      <SshPrompts />
    </ThemeProvider>,
  );
  await act(() => Promise.resolve());
  return (event) => {
    act(() => {
      emit?.(event);
    });
  };
}

it('asks to trust an unknown host key, showing its fingerprint', async () => {
  const emit = await mount();
  emit({
    id: 'ssh-1',
    prompt: {
      kind: 'hostKey',
      host: 'bastion',
      port: 2222,
      algorithm: 'ssh-ed25519',
      fingerprint: 'SHA256:synthetic-fingerprint',
      otherAlgorithms: ['ssh-rsa'],
    },
  });
  expect(screen.getByText('SHA256:synthetic-fingerprint')).toBeVisible();
  expect(screen.getByText(/bastion:2222/)).toBeVisible();
  expect(screen.getByRole('alert')).toHaveTextContent('ssh-rsa');
  await userEvent.click(
    screen.getByRole('button', { name: 'Trust and connect' }),
  );
  expect(answerSshPrompt).toHaveBeenCalledWith('ssh-1', { kind: 'trust' });
  expect(screen.queryByText('SHA256:synthetic-fingerprint')).toBeNull();
});

it('cancels on Escape and closes when the core abandons the prompt', async () => {
  const emit = await mount();
  const prompt = {
    kind: 'hostKey' as const,
    host: 'bastion',
    port: 22,
    algorithm: 'ssh-ed25519',
    fingerprint: 'SHA256:abc',
    otherAlgorithms: [],
  };
  emit({ id: 'ssh-2', prompt });
  await userEvent.keyboard('{Escape}');
  expect(answerSshPrompt).toHaveBeenCalledWith('ssh-2', { kind: 'cancel' });
  emit({ id: 'ssh-3', prompt });
  emit({ id: 'ssh-3', prompt: null });
  expect(screen.queryByText('SHA256:abc')).toBeNull();
});

it('collects keyboard-interactive answers, masking secret prompts', async () => {
  const emit = await mount();
  emit({
    id: 'ssh-4',
    prompt: {
      kind: 'challenge',
      host: 'bastion',
      user: 'ops',
      name: '',
      instructions: 'Enter your code',
      prompts: [
        { text: 'Password: ', echo: false },
        { text: 'Verification code:', echo: true },
      ],
    },
  });
  const password = screen.getByLabelText('Password');
  expect(password).toHaveAttribute('type', 'password');
  await userEvent.type(password, 'pw-synthetic');
  await userEvent.type(
    screen.getByRole('textbox', { name: 'Verification code' }),
    '123456{Enter}',
  );
  expect(answerSshPrompt).toHaveBeenCalledWith('ssh-4', {
    kind: 'answers',
    values: ['pw-synthetic', '123456'],
  });
});
