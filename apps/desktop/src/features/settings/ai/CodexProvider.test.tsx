// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  cancelAiVerification,
  promptAiKey,
  signInCodex,
  verifyAiProvider,
  writeSettingsFile,
} from '@openrows/ipc';
import type { CodexStatus, Verification } from '@openrows/ipc';
import AiPane from './AiPane';
import { aiSettings, parseAiSettings } from './aiSettings';
import { codexStatusText } from './codexStatus';
import { reloadSettings } from '../localSetting';
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../../test/ipcStub')).ipcStub(),
  cancelAiVerification: vi.fn().mockResolvedValue(undefined),
  promptAiKey: vi.fn().mockResolvedValue('saved'),
  signInCodex: vi.fn(),
  verifyAiProvider: vi.fn(),
}));
const codex = {
  id: 'codex',
  name: 'Codex',
  kind: 'codex' as const,
  baseUrl: '',
  allowInsecureRemote: false,
  secretRef: '',
};
const signedIn: CodexStatus = {
  state: 'signedIn',
  version: '0.166.0',
  newer: false,
  mode: 'chatgpt',
  plan: 'plus',
};
function probe(cli: CodexStatus): Verification {
  return {
    health:
      cli.state === 'signedIn'
        ? { state: 'verified' }
        : { state: 'failed', reason: { kind: 'auth' } },
    models: [],
    usage: null,
    cli,
  };
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(writeSettingsFile).mockResolvedValue(undefined);
  aiSettings.set(parseAiSettings(null));
  reloadSettings();
});
it('saves a Codex provider without a key or endpoint', async () => {
  const user = userEvent.setup();
  render(<AiPane />);
  await user.click(screen.getByRole('button', { name: 'Add provider' }));
  await user.type(screen.getByLabelText('Provider name'), 'Codex');
  await user.selectOptions(screen.getByLabelText('Provider type'), 'codex');
  expect(screen.queryByLabelText(/Enter an API key/)).not.toBeInTheDocument();
  expect(screen.queryByLabelText('Base URL')).not.toBeInTheDocument();
  expect(
    screen.getByText(/every approval Codex asks for is declined/),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Save provider' }));
  const row = await screen.findByRole('region', { name: 'Codex' });
  expect(within(row).getByText(/found on PATH/)).toBeVisible();
  expect(aiSettings.get().providers[0]).toMatchObject({
    kind: 'codex',
    baseUrl: '',
    secretRef: '',
  });
  expect(promptAiKey).not.toHaveBeenCalled();
});
it('names every discovery and account state explicitly', () => {
  expect(codexStatusText({ state: 'notInstalled' })).toMatch(/Not installed/);
  expect(
    codexStatusText({ state: 'signedOut', version: '0.166.0', newer: false }),
  ).toBe('Signed out · Codex 0.166.0');
  expect(codexStatusText(signedIn)).toBe(
    'Signed in · ChatGPT account · plus plan · Codex 0.166.0',
  );
  expect(
    codexStatusText({
      ...signedIn,
      plan: null,
      newer: true,
      version: '0.170.0',
    }),
  ).toBe('Signed in · ChatGPT account · Codex 0.170.0 (newer than tested)');
  expect(
    codexStatusText({ state: 'unsupported', version: null, tested: '0.166.0' }),
  ).toBe('Unsupported version unknown (tested with 0.166.0)');
  expect(codexStatusText({ state: 'error', reason: { kind: 'timeout' } })).toBe(
    'Error: Timed out',
  );
});
it('checks status, signs in through the CLI flow and re-checks', async () => {
  const user = userEvent.setup();
  aiSettings.set(parseAiSettings({ enabled: true, providers: [codex] }));
  vi.mocked(verifyAiProvider)
    .mockResolvedValueOnce(
      probe({ state: 'signedOut', version: '0.166.0', newer: false }),
    )
    .mockResolvedValueOnce(probe(signedIn));
  vi.mocked(signInCodex).mockImplementationOnce((_, onUrl) => {
    onUrl('https://auth.openai.com/oauth/authorize?state=fixture');
    return Promise.resolve(signedIn);
  });
  render(<AiPane />);
  const row = screen.getByRole('region', { name: 'Codex' });
  await user.click(within(row).getByRole('button', { name: 'Check status' }));
  expect(
    await within(row).findByText('Signed out · Codex 0.166.0'),
  ).toBeVisible();
  expect(within(row).getByText(/never API billing/)).toBeVisible();
  await user.click(
    within(row).getByRole('button', { name: 'Sign in with ChatGPT' }),
  );
  expect(signInCodex).toHaveBeenCalledWith('codex', expect.any(Function));
  expect(
    await within(row).findByText(/Signed in · ChatGPT account/),
  ).toBeVisible();
  expect(verifyAiProvider).toHaveBeenCalledTimes(2);
  expect(
    within(row).queryByRole('button', { name: 'Sign in with ChatGPT' }),
  ).not.toBeInTheDocument();
});
it('shows the sign-in page address and cancels a pending sign-in', async () => {
  const user = userEvent.setup();
  aiSettings.set(parseAiSettings({ enabled: true, providers: [codex] }));
  vi.mocked(verifyAiProvider).mockResolvedValue(
    probe({ state: 'signedOut', version: '0.166.0', newer: false }),
  );
  let fail: (reason: Error) => void = () => undefined;
  vi.mocked(signInCodex).mockImplementationOnce((_, onUrl) => {
    onUrl('https://auth.openai.com/oauth/authorize?state=fixture');
    return new Promise((_resolve, reject) => {
      fail = reject;
    });
  });
  render(<AiPane />);
  await user.click(screen.getByRole('button', { name: 'Check status' }));
  await user.click(
    await screen.findByRole('button', { name: 'Sign in with ChatGPT' }),
  );
  expect(
    await screen.findByText(/Finish signing in in your browser/),
  ).toBeVisible();
  expect(screen.getByText(/state=fixture/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Cancel sign-in' }));
  expect(cancelAiVerification).toHaveBeenCalledWith('codex');
  fail(new Error('cancelled'));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'cancelled, timed out or failed',
  );
});
