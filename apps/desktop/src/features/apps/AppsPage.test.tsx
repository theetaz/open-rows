// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  checkAgentApp,
  connectAgentApp,
  disconnectAgentApp,
  listAgentApps,
  planAgentApp,
  writeSettingsFile,
} from '@openrows/ipc';
import type { AppsOverview } from '@openrows/ipc';
import AppsPage from './AppsPage';
import { setAgentActivityForTest } from './activityStore';
import { aiSettings } from '../settings/ai/aiSettings';
import { mcpSettings, parseMcpSettings } from '../settings/ai/mcpSettings';

vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
  listAgentApps: vi.fn(),
  checkAgentApp: vi.fn(),
  planAgentApp: vi.fn(),
  connectAgentApp: vi.fn(() => Promise.resolve()),
  disconnectAgentApp: vi.fn(() => Promise.resolve(null)),
  renewAgentApp: vi.fn(() => Promise.resolve()),
  testAgentApp: vi.fn(() => Promise.resolve('[]')),
  listAgentActivity: vi.fn(() => Promise.resolve([])),
  onAgentActivity: vi.fn(() => Promise.resolve(() => undefined)),
  getAiSharing: vi.fn(() =>
    Promise.resolve({ level: 'schemaOnly', providers: ['claude-key'] }),
  ),
}));
vi.mock('../connections/useProfiles', () => ({
  useProfiles: () => ({
    profiles: [{ id: 'orders', name: 'Orders' }],
    problem: null,
  }),
}));

const LINE = '/app/openrows-mcp --port 32123 --client claude-0a1b2c3d4e5f';
const overview: AppsOverview = {
  bridge: '/app/openrows-mcp',
  bridgeProblem: null,
  port: 32123,
  apps: [
    {
      app: 'claude',
      install: { path: '/bin/claude', version: '2.1.330' },
      link: {
        client: 'claude-0a1b2c3d4e5f',
        token: 'token-claude',
        scope: {
          provider: 'claude-key',
          connections: ['orders'],
          expires: Math.floor(Date.now() / 1000) + 86_400 * 20,
        },
        usage: { token: 'token-claude', calls: 3, seen: null, client: null },
      },
    },
    {
      app: 'codex',
      install: { path: '/bin/codex', version: '0.171.0' },
      link: null,
    },
    { app: 'gemini', install: null, link: null },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  setAgentActivityForTest([]);
  mcpSettings.set(parseMcpSettings({ enabled: true, port: 32123 }));
  aiSettings.set({
    enabled: true,
    defaultProvider: '',
    defaultModel: '',
    cliTurnSeconds: 300,
    providers: [
      {
        id: 'claude-key',
        name: 'Anthropic',
        kind: 'anthropic',
        baseUrl: 'https://api.anthropic.com/v1/',
        secretRef: '',
        allowInsecureRemote: false,
      },
    ],
  });
  vi.mocked(listAgentApps).mockResolvedValue(structuredClone(overview));
  vi.mocked(checkAgentApp).mockResolvedValue({
    registration: { present: true, line: LINE, healthy: true },
    expected: LINE,
  });
});

it('shows each app’s state and only checks connected apps with their tool', async () => {
  render(<AppsPage onProviders={vi.fn()} />);
  const claude = await screen.findByRole('article', { name: 'Claude Code' });
  await within(claude).findByText('Connected');
  expect(within(claude).getByText(/3 tool calls/)).toBeInTheDocument();
  const codex = screen.getByRole('article', { name: 'Codex' });
  expect(within(codex).getByText('Installed · 0.171.0')).toBeInTheDocument();
  const gemini = screen.getByRole('article', { name: 'Gemini CLI' });
  expect(within(gemini).getByText('Not installed')).toBeInTheDocument();
  expect(checkAgentApp).toHaveBeenCalledTimes(1);
  expect(checkAgentApp).toHaveBeenCalledWith('claude');
  expect(screen.getByText('MCP server on 127.0.0.1:32123')).toBeInTheDocument();
});

it('shows the exact command and connects only after confirmation', async () => {
  const user = userEvent.setup();
  mcpSettings.set(parseMcpSettings({ enabled: false, port: 0 }));
  vi.mocked(planAgentApp).mockResolvedValue({
    client: 'codex-00112233aabb',
    port: 32124,
    replaces: false,
    commands: [
      'codex mcp add openrows -- /app/openrows-mcp --port 32124 --client codex-00112233aabb',
    ],
  });
  render(<AppsPage onProviders={vi.fn()} />);
  await user.click(
    await screen.findByRole('button', { name: 'Connect Codex' }),
  );
  const sheet = await screen.findByRole('dialog', { name: 'Connect Codex' });
  await within(sheet).findByText(/codex mcp add openrows --/);
  expect(
    within(sheet).getByText('Turns on the MCP server on 127.0.0.1:32124.'),
  ).toBeInTheDocument();
  expect(connectAgentApp).not.toHaveBeenCalled();
  await user.click(within(sheet).getByRole('button', { name: 'Connect' }));
  await waitFor(() => {
    expect(connectAgentApp).toHaveBeenCalledWith({
      app: 'codex',
      client: 'codex-00112233aabb',
      port: 32124,
      provider: 'claude-key',
      connections: ['orders'],
    });
  });
  expect(writeSettingsFile).toHaveBeenCalled();
  expect(mcpSettings.get()).toMatchObject({ enabled: true, port: 32124 });
});

it('flags a mismatched entry and disconnects after an inline confirmation', async () => {
  const user = userEvent.setup();
  vi.mocked(checkAgentApp).mockResolvedValue({
    registration: {
      present: true,
      line: '/old/openrows-mcp --port 1',
      healthy: false,
    },
    expected: LINE,
  });
  render(<AppsPage onProviders={vi.fn()} />);
  const claude = await screen.findByRole('article', { name: 'Claude Code' });
  await within(claude).findByText('Needs attention');
  await user.click(
    within(claude).getByRole('button', { name: 'Claude Code details' }),
  );
  const sheet = await screen.findByRole('dialog', { name: 'Claude Code' });
  expect(
    within(sheet).getByText(/different bridge or port/),
  ).toBeInTheDocument();
  await within(sheet).findByText('Schema only');
  await user.click(within(sheet).getByRole('button', { name: 'Disconnect…' }));
  expect(disconnectAgentApp).not.toHaveBeenCalled();
  await user.click(within(sheet).getByRole('button', { name: 'Disconnect' }));
  await waitFor(() => {
    expect(disconnectAgentApp).toHaveBeenCalledWith('claude');
  });
});
