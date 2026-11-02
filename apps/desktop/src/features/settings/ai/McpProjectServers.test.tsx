// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  discoverMcpTools,
  enableProjectMcpTools,
  listProjectMcpServers,
  promptProjectMcpSecret,
  revokeProjectMcpServer,
  trustProjectMcpServer,
} from '@openrows/ipc';
import type { ProjectServerView } from '@openrows/ipc';
import { McpProjectServers } from './McpProjectServers';
vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);
const untrusted: ProjectServerView = {
  root: '/work/app',
  project: 'App',
  key: 'p0123456789ab-search',
  digest: 'digest-1',
  trust: 'untrusted',
  enabledTools: [],
  tools: [],
  server: {
    id: 'search',
    name: 'Search',
    endpoint: {
      kind: 'stdio',
      command: '/usr/local/bin/search-mcp',
      args: ['--read-only'],
    },
    env: ['SEARCH_TOKEN'],
  },
};
const trusted: ProjectServerView = {
  ...untrusted,
  trust: 'trusted',
  tools: [{ name: 'lookup', schema: '{"type":"object"}' }],
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listProjectMcpServers).mockResolvedValue([untrusted]);
});

it('shows the exact definition and trusts only the reviewed digest', async () => {
  const user = userEvent.setup();
  render(<McpProjectServers />);
  const row = await screen.findByRole('region', { name: 'App: Search' });
  expect(
    within(row).getByText('Not trusted — it will not start'),
  ).toBeVisible();
  expect(within(row).getByText('/usr/local/bin/search-mcp')).toBeVisible();
  expect(within(row).getByText('["--read-only"]')).toBeVisible();
  expect(within(row).getByText('SEARCH_TOKEN')).toBeVisible();
  expect(
    within(row).queryByRole('button', { name: /Discover/ }),
  ).not.toBeInTheDocument();
  vi.mocked(listProjectMcpServers).mockResolvedValue([trusted]);
  await user.click(
    within(row).getByRole('button', { name: 'Trust App: Search' }),
  );
  expect(trustProjectMcpServer).toHaveBeenCalledWith(untrusted.key, 'digest-1');
  expect(await within(row).findByText('Trusted on this device')).toBeVisible();
});

it('refuses a definition that changed after it was shown', async () => {
  const user = userEvent.setup();
  vi.mocked(trustProjectMcpServer).mockRejectedValueOnce({ kind: 'invalid' });
  vi.mocked(listProjectMcpServers)
    .mockResolvedValueOnce([untrusted])
    .mockResolvedValue([
      { ...untrusted, trust: 'changed', digest: 'digest-2' },
    ]);
  render(<McpProjectServers />);
  const row = await screen.findByRole('region', { name: 'App: Search' });
  await user.click(
    within(row).getByRole('button', { name: 'Trust App: Search' }),
  );
  expect(await within(row).findByRole('alert')).toHaveTextContent(
    'changed after it was shown',
  );
  expect(
    within(row).getByText(/Definition changed — trust and stored values/),
  ).toBeVisible();
});

it('discovers, enables tools, enters values natively and revokes', async () => {
  const user = userEvent.setup();
  vi.mocked(listProjectMcpServers).mockResolvedValue([trusted]);
  render(<McpProjectServers />);
  const row = await screen.findByRole('region', { name: 'App: Search' });
  await user.click(
    within(row).getByRole('button', { name: 'Discover App: Search tools' }),
  );
  expect(discoverMcpTools).toHaveBeenCalledWith(trusted.key);
  await user.click(within(row).getByLabelText('App: Search: lookup'));
  expect(enableProjectMcpTools).toHaveBeenCalledWith(trusted.key, ['lookup']);
  await user.click(
    within(row).getByRole('button', { name: 'Set SEARCH_TOKEN…' }),
  );
  expect(promptProjectMcpSecret).toHaveBeenCalledWith(
    trusted.key,
    'SEARCH_TOKEN',
  );
  expect(
    await within(row).findByText('SEARCH_TOKEN: Saved in the keychain.'),
  ).toBeVisible();
  expect(document.querySelector('input[type="password"]')).toBeNull();
  await user.click(
    within(row).getByRole('button', { name: 'Revoke trust in App: Search' }),
  );
  expect(revokeProjectMcpServer).toHaveBeenCalledWith(trusted.key);
});

it('explains how project servers appear when none are loaded', async () => {
  vi.mocked(listProjectMcpServers).mockResolvedValue([]);
  render(<McpProjectServers />);
  expect(
    await screen.findByText(/Open a project that declares MCP servers/),
  ).toBeVisible();
});
