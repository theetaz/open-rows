// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import { AgentActivityButton } from './AgentActivityButton';
import { setAgentActivityForTest } from './activityStore';

vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
  listAgentActivity: vi.fn(() => Promise.resolve([])),
  onAgentActivity: vi.fn(() => Promise.resolve(() => undefined)),
}));

beforeEach(() => {
  setAgentActivityForTest([]);
});

it('appears only while an external agent is recently active', () => {
  vi.useFakeTimers();
  const { container } = render(<AgentActivityButton />);
  expect(container).toBeEmptyDOMElement();
  act(() => {
    setAgentActivityForTest([
      {
        at: Math.floor(Date.now() / 1000),
        token: 'token',
        app: 'claude',
        tool: 'list_schemas',
        connection: 'orders',
        outcome: 'ok',
      },
    ]);
  });
  expect(
    screen.getByRole('button', { name: 'Claude Code is using OpenRows' }),
  ).toBeInTheDocument();
  act(() => {
    vi.advanceTimersByTime(121_000);
  });
  expect(container).toBeEmptyDOMElement();
  vi.useRealTimers();
});
