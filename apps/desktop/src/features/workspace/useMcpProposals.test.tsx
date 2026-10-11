// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { onMcpProposals, takeMcpProposals } from '@openrows/ipc';
import type { QueryTabs } from '../query';
import { useMcpProposals } from './useMcpProposals';
vi.mock('@openrows/ipc', () => ({
  onMcpProposals: vi.fn().mockResolvedValue(() => undefined),
  takeMcpProposals: vi
    .fn()
    .mockResolvedValue([
      { connection: 'connection', database: 'main', sql: 'DROP TABLE items' },
    ]),
}));
it('routes writes to an editor without invoking any execution path', async () => {
  const open = vi.fn();
  renderHook(() => {
    useMcpProposals({ open } as unknown as QueryTabs, true);
  });
  await waitFor(() => {
    expect(open).toHaveBeenCalledWith(
      { connectionId: 'connection', database: 'main' },
      {
        title: 'SQL proposal — review before running',
        sql: 'DROP TABLE items',
      },
    );
  });
  expect(onMcpProposals).toHaveBeenCalledOnce();
  expect(takeMcpProposals).toHaveBeenCalledOnce();
});
