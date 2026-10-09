// SPDX-License-Identifier: Apache-2.0
import {
  act,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  clearHistory,
  createSavedQuery,
  createProjectQuery,
  loadProject,
  deleteHistory,
  historyLiterals,
  searchHistory,
} from '@openrows/ipc';
import type { HistoryItemDto } from '@openrows/ipc';
import { HistoryList } from './HistoryList';
import { useHistoryInsert } from './historyInsert';
import type { EditorHandle } from './request';
import { closeProject, showProject } from '../projects/projectStore';

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
afterEach(() => {
  closeProject();
});

it('inserts from the history action into the focused editor cursor', async () => {
  const replace = vi.fn();
  const editor = {
    current: {
      text: () => 'before after',
      cursor: () => 7,
      replace,
      focus: vi.fn(),
    } as unknown as EditorHandle,
  };
  const focus = renderHook(() => useHistoryInsert('query-1', editor));
  act(() => {
    focus.result.current();
  });
  const { user } = renderList();
  await user.click(
    await screen.findByRole('button', { name: 'Insert SELECT 1 at cursor' }),
  );
  expect(replace).toHaveBeenCalledWith('before SELECT 1after');
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

it('reviews and highlights literals before saving a history statement locally', async () => {
  vi.mocked(historyLiterals).mockResolvedValue([{ from: 7, to: 8 }]);
  vi.mocked(createSavedQuery).mockClear();
  const { user } = renderList();
  await user.click(
    await screen.findByRole('button', { name: 'Save SELECT 1 as query' }),
  );
  const review = await screen.findByRole('dialog', {
    name: 'Review history query',
  });
  const preview = within(review).getByLabelText('Literal review');
  await waitFor(() => {
    expect(preview.querySelector('mark')).toHaveTextContent('1');
  });
  expect(createSavedQuery).not.toHaveBeenCalled();
  await user.click(
    within(review).getByRole('button', { name: 'Save reviewed query' }),
  );
  await waitFor(() => {
    expect(createSavedQuery).toHaveBeenCalledWith(
      'p1',
      'History query',
      'SELECT 1',
    );
  });
});

it('clears all history only after an explicit confirmation', async () => {
  const { user } = renderList();
  await screen.findByRole('list', { name: 'Query history' });
  await user.click(screen.getByRole('radio', { name: 'All' }));
  await user.click(screen.getByRole('button', { name: 'Clear' }));
  expect(clearHistory).not.toHaveBeenCalled();
  const confirm = screen.getByRole('group', { name: 'Clear history' });
  expect(confirm).toHaveTextContent('all connections');
  await user.click(
    within(confirm).getByRole('button', { name: 'Clear history' }),
  );
  await waitFor(() => {
    expect(clearHistory).toHaveBeenCalledWith(null);
  });
});

it('saves only the reviewed SQL into the active project', async () => {
  const project = {
    root: '/project',
    name: 'Test project',
    defaultConnection: null,
    queryFolders: ['queries'],
    entries: [],
    truncated: false,
  };
  vi.mocked(loadProject).mockResolvedValue(project);
  vi.mocked(createProjectQuery).mockClear();
  vi.mocked(historyLiterals).mockResolvedValue([{ from: 7, to: 8 }]);
  act(() => {
    showProject(project);
  });
  const { user } = renderList();
  await user.click(
    await screen.findByRole('button', { name: 'Save SELECT 1 as query' }),
  );
  const dialog = await screen.findByRole('dialog', {
    name: 'Review history query',
  });
  const sql = within(dialog).getByRole('textbox', { name: 'SQL to save' });
  await user.clear(sql);
  await user.type(sql, 'SELECT 2');
  const save = within(dialog).getByRole('button', {
    name: 'Save reviewed query',
  });
  await waitFor(() => {
    expect(save).toBeEnabled();
  });
  expect(createProjectQuery).not.toHaveBeenCalled();
  await user.click(save);
  await waitFor(() => {
    expect(createProjectQuery).toHaveBeenCalledWith(
      '/project',
      'queries',
      'History query',
      'SELECT 2',
    );
  });
});
