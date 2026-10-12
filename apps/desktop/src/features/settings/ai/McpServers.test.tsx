// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  listMcpServers,
  saveMcpServers,
  discoverMcpTools,
  deleteMcpServer,
  setMcpServerSecrets,
} from '@openrows/ipc';
import { McpServers } from './McpServers';
vi.mock('@openrows/ipc', () => ({
  listMcpServers: vi.fn(),
  saveMcpServers: vi.fn().mockResolvedValue(undefined),
  discoverMcpTools: vi
    .fn()
    .mockResolvedValue([{ name: 'lookup', schema: '{"type":"object"}' }]),
  deleteMcpServer: vi.fn().mockResolvedValue(undefined),
  setMcpServerSecrets: vi.fn().mockResolvedValue(undefined),
}));
const server = {
  id: 'fixture',
  name: 'Fixture',
  endpoint: { kind: 'http' as const, url: 'http://localhost:1/mcp' },
  enabledTools: [],
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listMcpServers).mockResolvedValue([server]);
});
it('mounting never starts a user server; discovery and enablement are explicit', async () => {
  const user = userEvent.setup();
  render(<McpServers />);
  await screen.findByRole('button', { name: 'Discover Fixture tools' });
  expect(discoverMcpTools).not.toHaveBeenCalled();
  await user.click(
    screen.getByRole('button', { name: 'Discover Fixture tools' }),
  );
  const tool = await screen.findByRole('checkbox', { name: 'Fixture: lookup' });
  expect(tool).not.toBeChecked();
  await user.click(tool);
  await waitFor(() => {
    expect(saveMcpServers).toHaveBeenCalledWith([
      { ...server, enabledTools: ['lookup'] },
    ]);
  });
  await user.click(tool);
  await waitFor(() => {
    expect(saveMcpServers).toHaveBeenLastCalledWith([server]);
  });
  await user.click(screen.getByRole('button', { name: 'Remove Fixture' }));
  expect(deleteMcpServer).toHaveBeenCalledWith('fixture');
});
it('credentials go only to the separate keychain API, never public preferences', async () => {
  const user = userEvent.setup();
  render(<McpServers />);
  await screen.findByRole('button', { name: 'Discover Fixture tools' });
  await user.click(screen.getByRole('button', { name: 'Add MCP server' }));
  await user.type(screen.getByLabelText('MCP server name'), 'Private');
  await user.type(screen.getByLabelText('MCP executable'), '/usr/bin/node');
  const env = screen.getByLabelText('MCP environment (secret JSON object)');
  await user.clear(env);
  await user.paste('{"FIXTURE_KEY":"synthetic-only"}');
  await user.click(screen.getByRole('button', { name: 'Save MCP server' }));
  await waitFor(() => {
    expect(setMcpServerSecrets).toHaveBeenCalled();
  });
  expect(setMcpServerSecrets).toHaveBeenCalledWith(expect.any(String), {
    env: { FIXTURE_KEY: 'synthetic-only' },
    headers: {},
  });
  expect(JSON.stringify(vi.mocked(saveMcpServers).mock.calls)).not.toContain(
    'synthetic-only',
  );
  expect(
    screen.queryByLabelText('MCP environment (secret JSON object)'),
  ).toBeNull();
});

it('saved enabled tools can be disabled without contacting their server', async () => {
  vi.mocked(listMcpServers).mockResolvedValue([
    { ...server, enabledTools: ['lookup'] },
  ]);
  const user = userEvent.setup();
  render(<McpServers />);
  const checkbox = await screen.findByRole('checkbox', {
    name: 'Fixture: lookup',
  });
  expect(checkbox).toBeChecked();
  await user.click(checkbox);
  await waitFor(() => {
    expect(saveMcpServers).toHaveBeenCalledWith([server]);
  });
  expect(discoverMcpTools).not.toHaveBeenCalled();
});
