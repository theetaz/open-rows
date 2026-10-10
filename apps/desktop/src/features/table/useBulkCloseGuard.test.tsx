// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { useBulkCloseGuard } from './useBulkCloseGuard';
import { useTableViews } from './useTableViews';
import { TabMenu } from '../query/TabMenu';
import { closeTab, emptyTabs, openTab } from '../query/tabs';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
const scope = { connectionId: 'a', database: 'orders' };
const initial = openTab(
  openTab(openTab(emptyTabs, scope, { title: 'Keep' }), scope, {
    title: 'First dirty',
    sql: 'SELECT 1',
  }),
  scope,
  { title: 'Second dirty', sql: 'SELECT 2' },
);

function Harness(): React.JSX.Element {
  const [state, setState] = useState(initial);
  const [menu, setMenu] = useState(false);
  const guard = useRef<((ids: readonly string[]) => boolean) | null>(null);
  const tables = useTableViews();
  const tabs = {
    state,
    installBulkCloseGuard: (next: (ids: readonly string[]) => boolean) => {
      guard.current = next;
      return () => {
        guard.current = null;
      };
    },
    closeMany: (ids: readonly string[], options?: { discard: boolean }) => {
      if (!options?.discard && guard.current && !guard.current(ids)) return;
      setState((current) =>
        ids.reduce((next, id) => closeTab(next, id), current),
      );
    },
  } as QueryTabs;
  const review = useBulkCloseGuard({
    tabs,
    tables,
    runs: { release: vi.fn() } as unknown as QueryRuns,
  });
  return (
    <>
      <output aria-label="Open tabs">
        {state.tabs.map((tab) => tab.title).join(', ')}
      </output>
      <button
        type="button"
        onClick={() => {
          setMenu(true);
        }}
      >
        Menu
      </button>
      <button
        type="button"
        onClick={() => {
          tables.setEdits('query-2', (history) => ({
            ...history,
            pending: true,
          }));
        }}
      >
        Start commit
      </button>
      {menu && (
        <TabMenu
          tabs={tabs}
          target={{ id: 'query-1', x: 0, y: 0 }}
          onRename={vi.fn()}
          onClose={() => {
            setMenu(false);
          }}
        />
      )}
      {review}
    </>
  );
}

it.each(['Close Other Tabs', 'Close Tabs to the Right'])(
  'reviews all dirty tabs once for %s, cancel preserves every tab',
  async (action) => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Menu' }));
    await user.click(screen.getByRole('menuitem', { name: action }));
    const review = screen.getByRole('dialog', { name: 'Review closing tabs' });
    expect(within(review).getAllByRole('listitem')).toHaveLength(2);
    expect(review).toHaveTextContent('First dirty: unsaved changes');
    expect(review).toHaveTextContent('Second dirty: unsaved changes');
    expect(screen.getByLabelText('Open tabs')).toHaveTextContent('First dirty');
    await user.click(screen.getByRole('button', { name: 'Keep tabs' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Menu' }));
    await user.click(screen.getByRole('menuitem', { name: action }));
    await user.click(
      screen.getByRole('button', { name: 'Discard and close tabs' }),
    );
    expect(screen.getByLabelText('Open tabs')).toHaveTextContent(/^Keep$/);
  },
);

it('blocks the whole group while any affected tab is committing', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole('button', { name: 'Start commit' }));
  await user.click(screen.getByRole('button', { name: 'Menu' }));
  await user.click(screen.getByRole('menuitem', { name: 'Close Other Tabs' }));
  expect(
    screen.getByRole('button', { name: 'Discard and close tabs' }),
  ).toBeDisabled();
  expect(screen.getByLabelText('Open tabs')).toHaveTextContent('Second dirty');
});
