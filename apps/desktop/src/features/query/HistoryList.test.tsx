// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { clearHistory, deleteHistory, searchHistory } from '@openrows/ipc';
import type { HistoryItemDto } from '@openrows/ipc';
import { HistoryList } from './HistoryList';

type HistoryConnection = { id: string; name: string };

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const orders: HistoryConnection = { id: 'p1', name: 'orders-local' };
const staging: HistoryConnection = { id: 'p2', name: 'staging' };
const writeText = vi.fn<(text: string) => Promise<void>>(() =>
  Promise.resolve(),
);

function entry(
  id: number,
  statement: string,
  profileId = 'p1',
  extra: Partial<HistoryItemDto> = {},
): HistoryItemDto {
  return {
    id,
    profileId,
    database: 'shop',
    statement,
    startedAtMs: Date.now(),
    durationMs: 4,
    status: 'ok',
    rows: 2,
    errorMessage: null,
    ...extra,
  };
}

function renderList(
  props: Partial<React.ComponentProps<typeof HistoryList>> = {},
): {
  user: ReturnType<typeof userEvent.setup>;
  onOpen: ReturnType<typeof vi.fn>;
} {
  const onOpen = vi.fn();
  render(
    <ThemeProvider>
      <HistoryList
        active={orders}
        connections={[orders, staging]}
        text=""
        onOpen={onOpen}
        {...props}
      />
    </ThemeProvider>,
  );
  // userEvent.setup() installs its own clipboard stub, so the mock goes back on top.
  const user = userEvent.setup();
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText },
  });
  return { user, onOpen };
}

beforeEach(() => {
  vi.mocked(searchHistory).mockClear();
  vi.mocked(deleteHistory).mockClear();
  vi.mocked(clearHistory).mockClear();
  vi.mocked(searchHistory).mockResolvedValue([
    entry(1, 'SELECT 1'),
    entry(2, 'SELECT * FROM orders', 'p1', {
      status: 'error',
      errorMessage: 'relation "orders" does not exist',
    }),
  ]);
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText },
  });
});

it('lists the active connection by day and opens a statement in a tab', async () => {
  const { user, onOpen } = renderList();
  const list = await screen.findByRole('list', { name: 'Query history' });
  expect(searchHistory).toHaveBeenCalledWith({
    profileId: 'p1',
    text: '',
    limit: 300,
  });
  expect(within(list).getByText('Today')).toBeVisible();
  await user.click(
    within(list).getByRole('button', { name: /^SELECT \* FROM orders/ }),
  );
  expect(onOpen).toHaveBeenCalledWith('SELECT * FROM orders');
});

it('searches the typed text and widens to every connection, naming each one', async () => {
  vi.mocked(searchHistory).mockResolvedValue([entry(3, 'SELECT 3', 'p2')]);
  const { user } = renderList({ text: 'select' });
  await user.click(await screen.findByRole('radio', { name: 'All' }));
  await waitFor(() => {
    expect(searchHistory).toHaveBeenLastCalledWith({
      profileId: null,
      text: 'select',
      limit: 300,
    });
  });
  expect(await screen.findByText(/staging/)).toBeVisible();
});

it('copies a statement and deletes one, then refreshes the list', async () => {
  vi.mocked(deleteHistory).mockResolvedValue(true);
  const { user } = renderList();
  await user.click(
    await screen.findByRole('button', { name: 'Copy SELECT 1' }),
  );
  expect(writeText).toHaveBeenCalledWith('SELECT 1');
  const before = vi.mocked(searchHistory).mock.calls.length;
  await user.click(screen.getByRole('button', { name: 'Delete SELECT 1' }));
  await waitFor(() => {
    expect(deleteHistory).toHaveBeenCalledWith(1);
  });
  await waitFor(() => {
    expect(vi.mocked(searchHistory).mock.calls.length).toBeGreaterThan(before);
  });
});

it('clears this connection only after the confirmation', async () => {
  vi.mocked(clearHistory).mockResolvedValue(null);
  const { user } = renderList();
  await user.click(await screen.findByRole('button', { name: 'Clear' }));
  const confirm = screen.getByRole('group', { name: 'Clear history' });
  expect(clearHistory).not.toHaveBeenCalled();
  await user.click(
    within(confirm).getByRole('button', { name: 'Clear history' }),
  );
  await waitFor(() => {
    expect(clearHistory).toHaveBeenCalledWith('p1');
  });
});

it('asks for a connection rather than listing every one', () => {
  renderList({ active: undefined });
  expect(
    screen.getByText('Select a connection to see its history.'),
  ).toBeVisible();
  expect(searchHistory).not.toHaveBeenCalled();
});

it('explains an empty history', async () => {
  vi.mocked(searchHistory).mockResolvedValue([]);
  renderList();
  expect(await screen.findByText('No statements run yet')).toBeVisible();
});

it('explains a search with no match', async () => {
  vi.mocked(searchHistory).mockResolvedValue([]);
  renderList({ text: 'zzz' });
  expect(await screen.findByText('No matching statements')).toBeVisible();
});

it('reports a failed search instead of an empty list', async () => {
  vi.mocked(searchHistory).mockRejectedValueOnce(new Error('disk is full'));
  renderList();
  expect(await screen.findByRole('alert')).toHaveTextContent('disk is full');
});
