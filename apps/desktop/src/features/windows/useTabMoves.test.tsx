// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { transferTabs } from '@openrows/ipc';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';
import type { TableViews } from '../table/useTableViews';
import type { QueryTab } from '../query/tabs';
import { trackLiveText } from '../query/liveText';
import { moveText, useTabMoves } from './useTabMoves';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const tab: QueryTab = {
  id: 'table-1',
  kind: 'table',
  connectionId: 'id-a',
  database: 'app',
  title: 'orders',
  sql: '',
  table: { database: 'app', schema: 'public', name: 'orders' },
};

function setup(rows: number) {
  const close = vi.fn();
  const tabs = {
    state: { tabs: [tab], active: {}, next: 2 },
    close,
    activeFor: () => tab,
  } as unknown as QueryTabs;
  const release = vi.fn();
  const tables = {
    entryOf: () => ({
      edits: { current: Array.from({ length: rows }), pending: false },
      schema: { current: [], pending: false },
    }),
    release,
  } as unknown as TableViews;
  const runs = { release: vi.fn() } as unknown as QueryRuns;
  function Harness(): React.JSX.Element {
    const moves = useTabMoves({ tabs, tables, runs }, 'id-a', 'app');
    return (
      <ThemeProvider>
        <button
          type="button"
          onClick={() => {
            moves.move('table-1', 'workspace-2');
          }}
        >
          move
        </button>
        {moves.sheet}
      </ThemeProvider>
    );
  }
  render(<Harness />);
  return { close, release, user: userEvent.setup() };
}

it('moves a clean tab at once and closes it here', async () => {
  const { close, release, user } = setup(0);
  await user.click(screen.getByRole('button', { name: 'move' }));
  expect(vi.mocked(transferTabs).mock.calls[0]?.[0]).toBe('workspace-2');
  await vi.waitFor(() => {
    expect(close).toHaveBeenCalledWith('table-1', { discard: true });
  });
  expect(release).toHaveBeenCalledWith('table-1');
});

it('moves the editor text typed just before the move', async () => {
  vi.mocked(transferTabs).mockClear();
  trackLiveText('table-1', () => 'SELECT typed_now');
  const { user } = setup(0);
  await user.click(screen.getByRole('button', { name: 'move' }));
  expect(String(vi.mocked(transferTabs).mock.calls[0]?.[1])).toContain(
    'typed_now',
  );
  trackLiveText('table-1', null);
});

it('explains staged edits before moving', async () => {
  vi.mocked(transferTabs).mockClear();
  const { close, user } = setup(2);
  await user.click(screen.getByRole('button', { name: 'move' }));
  expect(screen.getByRole('dialog')).toHaveTextContent(
    '2 staged row edits stay in the local journal',
  );
  expect(transferTabs).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Move tab' }));
  await vi.waitFor(() => {
    expect(close).toHaveBeenCalled();
  });
});

it('describes a running commit as a reason to wait', () => {
  expect(moveText({ pending: true, journaled: 1, lost: 0 })).toContain(
    'until the commit finishes',
  );
});
