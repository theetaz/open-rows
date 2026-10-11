// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  createMcpToken,
  disconnectMcpClient,
  mcpStatus,
  revokeMcpToken,
  writeSettingsFile,
} from '@openrows/ipc';
import McpPane from './McpPane';
import { aiSettings } from './aiSettings';
import { collectSettings } from '../settingsFile';
import { mcpSettings, parseMcpSettings } from './mcpSettings';
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../../test/ipcStub')).ipcStub(),
  mcpStatus: vi.fn(),
  createMcpToken: vi.fn(),
  revokeMcpToken: vi.fn(),
  disconnectMcpClient: vi.fn(),
  listAiAudit: vi.fn().mockResolvedValue([]),
}));
vi.mock('../../connections/useProfiles', () => ({
  useProfiles: () => ({
    profiles: [{ id: 'connection', name: 'Fixture' }],
    problem: null,
  }),
}));
const scope = {
  provider: 'provider',
  connections: ['connection'],
  expires: 1234567890,
};
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  mcpSettings.set(parseMcpSettings(null));
  aiSettings.set({
    enabled: true,
    defaultProvider: '',
    defaultModel: '',
    providers: [
      {
        id: 'provider',
        name: 'Saved provider',
        kind: 'openAi',
        baseUrl: 'https://api.openai.com/v1/',
        secretRef: '',
        allowInsecureRemote: false,
      },
    ],
  });
  vi.mocked(mcpStatus).mockResolvedValue({
    port: null,
    httpEnabled: false,
    tokens: [],
    clients: [],
  });
});
it('defaults off, persists transport choices and never stores one-time tokens', async () => {
  const user = userEvent.setup();
  vi.mocked(createMcpToken).mockResolvedValue({
    info: { id: 'token', scope },
    value: 'test-only',
  });
  const view = render(<McpPane />);
  expect(
    screen.getByLabelText('Allow direct localhost HTTP'),
  ).not.toBeChecked();
  await user.click(screen.getByLabelText('Enable MCP stdio bridge'));
  await waitFor(() => {
    expect(writeSettingsFile).toHaveBeenCalled();
  });
  await user.selectOptions(
    screen.getByLabelText('MCP provider identity'),
    'provider',
  );
  await user.click(screen.getByLabelText('Expose Fixture'));
  await user.click(screen.getByRole('button', { name: 'Create token' }));
  expect(await screen.findByLabelText('New MCP token')).toHaveTextContent(
    'test-only',
  );
  expect(createMcpToken).toHaveBeenCalledWith(
    expect.objectContaining({
      provider: 'provider',
      connections: ['connection'],
    }),
  );
  expect(JSON.stringify(collectSettings())).not.toContain('test-only');
  await user.click(screen.getByRole('button', { name: 'Hide token' }));
  expect(screen.queryByLabelText('New MCP token')).toBeNull();
  view.unmount();
  render(<McpPane />);
  expect(screen.queryByLabelText('New MCP token')).toBeNull();
});
it('shows scrubbed metadata and exposes revoke/disconnect controls', async () => {
  vi.mocked(mcpStatus).mockResolvedValue({
    port: 1234,
    httpEnabled: true,
    tokens: [{ id: 'token', scope }],
    clients: [{ id: 'token', active: 1 }],
  });
  const user = userEvent.setup();
  render(<McpPane />);
  expect(
    await screen.findByText(/Listening on 127.0.0.1:1234/),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Revoke token' }));
  await waitFor(() => {
    expect(revokeMcpToken).toHaveBeenCalledWith('token');
  });
  await user.click(screen.getByRole('button', { name: 'Disconnect client' }));
  await waitFor(() => {
    expect(disconnectMcpClient).toHaveBeenCalledWith('token');
  });
});
it('discards pasted credentials from transport preferences', () => {
  expect(
    parseMcpSettings({ enabled: true, token: 'test-only', port: -1 }),
  ).toEqual({ enabled: true, httpEnabled: false, port: 0 });
});
