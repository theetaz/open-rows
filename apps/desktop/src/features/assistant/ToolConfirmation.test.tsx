// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { confirmMcpTool } from '@openrows/ipc';
import { ToolConfirmation } from './ToolConfirmation';
vi.mock('@openrows/ipc', () => ({
  confirmMcpTool: vi.fn().mockResolvedValue(undefined),
}));
it('only explicit approval answers a pending prompt and cannot approve twice', async () => {
  const user = userEvent.setup();
  render(
    <ToolConfirmation
      prompt={{
        type: 'confirmation',
        id: 'pending',
        server: 'Fixture',
        tool: 'lookup',
        arguments: '{"q":"‹redacted›"}',
      }}
      disabled={false}
    />,
  );
  expect(confirmMcpTool).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Allow tool' }));
  expect(confirmMcpTool).toHaveBeenCalledWith('pending', true);
  expect(screen.getByRole('button', { name: 'Allow tool' })).toBeDisabled();
});
it('denial is explicit and cancelled prompts cannot be approved', async () => {
  const user = userEvent.setup();
  const { rerender } = render(
    <ToolConfirmation
      prompt={{
        type: 'confirmation',
        id: 'deny',
        server: 'Fixture',
        tool: 'lookup',
        arguments: '{}',
      }}
      disabled={false}
    />,
  );
  await user.click(screen.getByRole('button', { name: 'Deny tool' }));
  expect(confirmMcpTool).toHaveBeenCalledWith('deny', false);
  rerender(
    <ToolConfirmation
      key="new"
      prompt={{
        type: 'confirmation',
        id: 'closed',
        server: 'Fixture',
        tool: 'lookup',
        arguments: '{}',
      }}
      disabled
    />,
  );
  expect(screen.getByRole('button', { name: 'Allow tool' })).toBeDisabled();
});
