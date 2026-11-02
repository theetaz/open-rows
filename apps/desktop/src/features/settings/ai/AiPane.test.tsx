// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  cancelAiVerification,
  deleteAiKey,
  promptAiKey,
  verifyAiProvider,
  writeSettingsFile,
} from '@openrows/ipc';
import type { Verification } from '@openrows/ipc';
import AiPane from './AiPane';
import { aiSettings, parseAiSettings } from './aiSettings';
import { collectSettings } from '../settingsFile';
import { reloadSettings } from '../localSetting';
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../../test/ipcStub')).ipcStub(),
  cancelAiVerification: vi.fn().mockResolvedValue(undefined),
  deleteAiKey: vi.fn().mockResolvedValue(undefined),
  promptAiKey: vi.fn().mockResolvedValue('saved'),
  verifyAiProvider: vi.fn(),
}));
const result: Verification = {
  health: { state: 'verified' },
  usage: { used: 2, limit: 10, remaining: 8 },
  cli: null,
  models: [
    {
      id: 'model',
      name: 'Model',
      capabilities: {
        streaming: true,
        toolCalling: null,
        structuredOutput: null,
        vision: null,
        contextWindow: null,
        maxOutput: null,
      },
    },
  ],
};
const configured = {
  id: 'fixture',
  name: 'Fixture',
  kind: 'openAi',
  baseUrl: 'https://api.openai.com/v1/',
  allowInsecureRemote: false,
  secretRef: 'ai/fixture/api-key',
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(writeSettingsFile).mockResolvedValue(undefined);
  vi.mocked(deleteAiKey).mockResolvedValue(undefined);
  vi.mocked(promptAiKey).mockResolvedValue('saved');
  vi.mocked(verifyAiProvider).mockResolvedValue(result);
  aiSettings.set(parseAiSettings(null));
  reloadSettings();
});
async function addProvider(local = false): Promise<void> {
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Add provider' }));
  await user.type(screen.getByLabelText('Provider name'), 'My provider');
  if (local)
    await user.selectOptions(screen.getByLabelText('Provider type'), 'ollama');
  expect(document.querySelector('input[type="password"]')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Save provider' }));
  await screen.findByRole('region', { name: 'My provider' });
}
it('adds keychain references, explicitly enables, verifies, selects defaults, edits and removes', async () => {
  const user = userEvent.setup();
  render(<AiPane />);
  expect(screen.getByLabelText('AI features off')).toBeChecked();
  await addProvider();
  const id = aiSettings.get().providers[0]?.id;
  expect(promptAiKey).toHaveBeenCalledWith(id);
  expect(aiSettings.get().providers[0]?.secretRef).toBe(
    `ai/${String(id)}/api-key`,
  );
  expect(JSON.stringify(collectSettings())).not.toMatch(/"key"|apiKey/);
  expect(screen.getByRole('button', { name: 'Verify' })).toBeDisabled();
  expect(verifyAiProvider).not.toHaveBeenCalled();
  await user.click(screen.getByLabelText('AI features off'));
  await waitFor(() => {
    expect(screen.getByRole('button', { name: 'Verify' })).toBeEnabled();
  });
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  expect(await screen.findByText('Verified')).toBeVisible();
  expect(screen.getByText(/Tools: unknown/)).toBeVisible();
  expect(screen.getByText(/Remaining: 8/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Use as default' }));
  await waitFor(() => {
    expect(screen.getByLabelText('Default model')).toBeEnabled();
  });
  await user.type(screen.getByLabelText('Default model'), 'model');
  await user.tab();
  expect(aiSettings.get().defaultModel).toBe('model');
  await waitFor(() => {
    expect(screen.getByRole('button', { name: 'Edit' })).toBeEnabled();
  });
  await user.click(screen.getByRole('button', { name: 'Edit' }));
  await user.clear(screen.getByLabelText('Provider name'));
  await user.type(screen.getByLabelText('Provider name'), 'Edited');
  await user.click(screen.getByRole('button', { name: 'Save provider' }));
  await screen.findByRole('region', { name: 'Edited' });
  expect(promptAiKey).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole('button', { name: 'Remove' }));
  await waitFor(() => {
    expect(
      screen.queryByRole('region', { name: 'Edited' }),
    ).not.toBeInTheDocument();
  });
  expect(deleteAiKey).toHaveBeenCalled();
  expect(aiSettings.get().defaultProvider).toBe('');
  expect(aiSettings.get().defaultModel).toBe('');
});
it('supports keyless local servers and never probes on mount', async () => {
  render(<AiPane />);
  await addProvider(true);
  expect(promptAiKey).not.toHaveBeenCalled();
  expect(aiSettings.get().providers[0]?.secretRef).toBe('');
  expect(verifyAiProvider).not.toHaveBeenCalled();
});
it('shows configured, typed failure with retry-after, and unknown discovery states', async () => {
  const user = userEvent.setup();
  aiSettings.set(parseAiSettings({ enabled: true, providers: [configured] }));
  vi.mocked(verifyAiProvider)
    .mockResolvedValueOnce({
      ...result,
      health: { state: 'failed', reason: { kind: 'rateLimit', retryAfter: 7 } },
    })
    .mockResolvedValueOnce({
      health: { state: 'unknown' },
      models: [],
      usage: null,
      cli: null,
    });
  render(<AiPane />);
  expect(screen.getByText('Configured')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  expect(
    await screen.findByText('Failed: Rate limit reached (retry after 7s)'),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  expect(await screen.findByText('Unknown')).toBeVisible();
  expect(screen.getByText('Available models: unknown')).toBeVisible();
});
it('retains configuration when key deletion fails and rolls back failed permission writes', async () => {
  const user = userEvent.setup();
  aiSettings.set(parseAiSettings({ providers: [configured] }));
  vi.mocked(deleteAiKey).mockRejectedValueOnce(new Error('test-only'));
  render(<AiPane />);
  await user.click(screen.getByRole('button', { name: 'Remove' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Provider retained',
  );
  expect(screen.getByRole('region', { name: 'Fixture' })).toBeVisible();
  vi.mocked(writeSettingsFile).mockRejectedValueOnce(new Error('test-only'));
  await user.click(screen.getByLabelText('AI features off'));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'could not be saved',
  );
  expect(screen.getByLabelText('AI features off')).toBeChecked();
  expect(screen.queryByText('test-only')).not.toBeInTheDocument();
});
it('exposes a cancellation action while metadata verification is pending', async () => {
  const user = userEvent.setup();
  aiSettings.set(parseAiSettings({ enabled: true, providers: [configured] }));
  vi.mocked(verifyAiProvider).mockImplementationOnce(
    () => new Promise(() => {}),
  );
  render(<AiPane />);
  await user.click(screen.getByRole('button', { name: 'Verify' }));
  const provider = screen.getByRole('region', { name: 'Fixture' });
  expect(within(provider).getByRole('status')).toHaveTextContent('Verifying');
  await user.click(screen.getByRole('button', { name: 'Cancel verification' }));
  expect(cancelAiVerification).toHaveBeenCalledWith('fixture');
});
it('replaces a key only through the native dialog and reports its outcome', async () => {
  const user = userEvent.setup();
  aiSettings.set(parseAiSettings({ providers: [configured] }));
  render(<AiPane />);
  const row = screen.getByRole('region', { name: 'Fixture' });
  await user.click(within(row).getByRole('button', { name: 'Set API key…' }));
  expect(promptAiKey).toHaveBeenCalledWith('fixture');
  expect(
    await within(row).findByText('API key saved in the keychain.'),
  ).toBeVisible();
  vi.mocked(promptAiKey).mockResolvedValueOnce('cancelled');
  await user.click(within(row).getByRole('button', { name: 'Set API key…' }));
  expect(
    await within(row).findByText(/No key entered; the stored key is unchanged/),
  ).toBeVisible();
  vi.mocked(promptAiKey).mockRejectedValueOnce(new Error('test-only'));
  await user.click(within(row).getByRole('button', { name: 'Set API key…' }));
  expect(
    await within(row).findByText('The API key could not be stored.'),
  ).toBeVisible();
  expect(screen.queryByText('test-only')).not.toBeInTheDocument();
});
it('saves a bounded Codex turn deadline', async () => {
  const user = userEvent.setup();
  render(<AiPane />);
  const field = screen.getByLabelText('Codex turn deadline (seconds)');
  expect(field).toHaveValue(300);
  await user.clear(field);
  await user.type(field, '600');
  await user.tab();
  await waitFor(() => {
    expect(aiSettings.get().cliTurnSeconds).toBe(600);
  });
  expect(writeSettingsFile).toHaveBeenCalled();
  const next = screen.getByLabelText('Codex turn deadline (seconds)');
  await user.clear(next);
  await user.type(next, '5000');
  await user.tab();
  await waitFor(() => {
    expect(aiSettings.get().cliTurnSeconds).toBe(900);
  });
});
