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
import { CommandError } from '@openrows/ipc';
import { McpServers } from './McpServers';
vi.mock('@openrows/ipc', async (original) => ({
  CommandError: (await original<typeof import('@openrows/ipc')>()).CommandError,
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
  vi.mocked(listMcpServers).mockResolvedValue({ servers: [server], tools: {} });
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
  vi.mocked(listMcpServers).mockResolvedValue({
    servers: [{ ...server, enabledTools: ['lookup'] }],
    tools: {},
  });
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

it('edits a server and keeps stored secrets unless new ones are typed', async () => {
  const user = userEvent.setup();
  render(<McpServers />);
  await user.click(await screen.findByRole('button', { name: 'Edit Fixture' }));
  const name = screen.getByLabelText('MCP server name');
  await user.clear(name);
  await user.type(name, 'Renamed');
  await user.click(screen.getByRole('button', { name: 'Save MCP server' }));
  await waitFor(() => {
    expect(saveMcpServers).toHaveBeenCalledWith([
      { ...server, name: 'Renamed' },
    ]);
  });
  expect(setMcpServerSecrets).not.toHaveBeenCalled();
  await user.click(await screen.findByRole('button', { name: 'Edit Renamed' }));
  await user.click(screen.getByLabelText('MCP headers (secret JSON object)'));
  await user.paste('{"Authorization":"Bearer synthetic"}');
  await user.click(screen.getByRole('button', { name: 'Save MCP server' }));
  await waitFor(() => {
    expect(setMcpServerSecrets).toHaveBeenCalledWith('fixture', {
      env: {},
      headers: { Authorization: 'Bearer synthetic' },
    });
  });
});

it('shows the last discovered catalogue and explains typed failures', async () => {
  vi.mocked(listMcpServers).mockResolvedValue({
    servers: [server],
    tools: { fixture: [{ name: 'lookup', schema: '{}' }] },
  });
  vi.mocked(discoverMcpTools).mockRejectedValueOnce(
    new CommandError({ kind: 'timeout', message: 'native', code: null }),
  );
  const user = userEvent.setup();
  render(<McpServers />);
  expect(
    await screen.findByRole('checkbox', { name: 'Fixture: lookup' }),
  ).not.toBeChecked();
  await user.click(
    screen.getByRole('button', { name: 'Discover Fixture tools' }),
  );
  expect((await screen.findByRole('alert')).textContent).toMatch(
    /Fixture did not answer within 5 seconds/,
  );
});

it('rejects malformed secret JSON before anything is saved', async () => {
  const user = userEvent.setup();
  render(<McpServers />);
  await user.click(
    await screen.findByRole('button', { name: 'Add MCP server' }),
  );
  await user.type(screen.getByLabelText('MCP server name'), 'Bad');
  await user.type(screen.getByLabelText('MCP executable'), '/bin/true');
  await user.click(
    screen.getByLabelText('MCP environment (secret JSON object)'),
  );
  await user.paste('["not","an","object"]');
  await user.click(screen.getByRole('button', { name: 'Save MCP server' }));
  expect((await screen.findByRole('alert')).textContent).toMatch(
    /JSON object of strings/,
  );
  expect(saveMcpServers).not.toHaveBeenCalled();
});
