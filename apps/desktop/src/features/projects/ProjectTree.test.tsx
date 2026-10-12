// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  chooseProject,
  confirmDestructive,
  createProjectFolder,
  deleteProjectItem,
  moveProjectItem,
  renameProjectItem,
  readSqlFile,
  revealProjectItem,
} from '@openrows/ipc';
import type { ProjectDto } from '@openrows/ipc';
import { ProjectSection } from './ProjectSection';
import { ProjectTree } from './ProjectTree';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const project: ProjectDto = {
  root: '/work/analytics',
  name: 'Analytics',
  defaultConnection: null,
  queryFolders: ['queries'],
  snippetFolders: [],
  connectionsFile: null,
  entries: [
    { path: 'queries', kind: 'folder' },
    { path: 'queries/reports', kind: 'folder' },
    { path: 'queries/reports/monthly.sql', kind: 'query' },
    { path: 'queries/top.sql', kind: 'query' },
  ],
  truncated: false,
};

function renderTree(search = ''): {
  onOpenQuery: ReturnType<typeof vi.fn>;
  onMoved: ReturnType<typeof vi.fn>;
} {
  const onOpenQuery = vi.fn();
  const onMoved = vi.fn();
  render(
    <ThemeProvider>
      <ProjectTree
        project={project}
        search={search}
        onOpenQuery={onOpenQuery}
        onMoved={onMoved}
      />
    </ThemeProvider>,
  );
  return { onOpenQuery, onMoved };
}

const row = (name: string): HTMLElement =>
  screen.getByRole('treeitem', { name });

/** A drag carrying data between the dragged row and the drop target. */
function dataTransfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    setData: (type: string, value: string) => data.set(type, value),
    getData: (type: string) => data.get(type) ?? '',
    get types() {
      return [...data.keys()];
    },
    effectAllowed: 'all',
    dropEffect: 'none',
  } as unknown as DataTransfer;
}

const rowBox = (name: string): HTMLElement =>
  row(name).querySelector<HTMLElement>('[data-level]') ?? row(name);

it('lists query folders as a tree and opens a query in a bound tab', async () => {
  const user = userEvent.setup();
  const { onOpenQuery } = renderTree();
  expect(screen.getByRole('tree', { name: 'Analytics queries' })).toBeVisible();
  expect(row('queries')).toHaveAttribute('aria-expanded', 'true');
  expect(row('reports')).toHaveAttribute('aria-expanded', 'false');
  await user.click(screen.getByText('top.sql'));
  expect(onOpenQuery).toHaveBeenCalledWith('/work/analytics/queries/top.sql');
  await user.click(
    row('reports').querySelector('[aria-hidden="true"]') ?? row('reports'),
  );
  expect(row('monthly.sql')).toBeVisible();
});

it('creates folders and renames queries from the row menu', async () => {
  const user = userEvent.setup();
  const { onMoved } = renderTree();
  fireEvent.contextMenu(rowBox('reports'));
  await user.click(screen.getByRole('menuitem', { name: 'New Folder…' }));
  await user.type(
    screen.getByRole('textbox', { name: 'Folder name' }),
    'daily',
  );
  await user.click(screen.getByRole('button', { name: 'Create' }));
  expect(createProjectFolder).toHaveBeenCalledWith(
    '/work/analytics',
    'queries/reports',
    'daily',
  );
  fireEvent.contextMenu(rowBox('top.sql'));
  await user.click(screen.getByRole('menuitem', { name: 'Rename…' }));
  const name = screen.getByRole('textbox', { name: 'Name' });
  expect(name).toHaveValue('top');
  await user.clear(name);
  await user.type(name, 'renamed{Enter}');
  await waitFor(() => {
    expect(renameProjectItem).toHaveBeenCalledWith(
      '/work/analytics',
      'queries/top.sql',
      'renamed',
    );
  });
  expect(onMoved).toHaveBeenCalledWith(
    '/work/analytics/queries/top.sql',
    '/work/analytics/queries/renamed.sql',
  );
});

it('deletes after confirmation, reveals and keeps query folders fixed', async () => {
  const user = userEvent.setup();
  renderTree();
  vi.mocked(confirmDestructive).mockResolvedValueOnce(true);
  fireEvent.contextMenu(rowBox('top.sql'));
  await user.click(screen.getByRole('menuitem', { name: 'Delete…' }));
  await waitFor(() => {
    expect(deleteProjectItem).toHaveBeenCalledWith(
      '/work/analytics',
      'queries/top.sql',
    );
  });
  fireEvent.contextMenu(rowBox('queries'));
  expect(screen.queryByRole('menuitem', { name: 'Rename…' })).toBeNull();
  await user.click(screen.getByRole('menuitem', { name: 'Reveal in Finder' }));
  expect(revealProjectItem).toHaveBeenCalledWith('/work/analytics', 'queries');
});

it('moves an item by dragging it onto a folder', () => {
  renderTree();
  const transfer = dataTransfer();
  fireEvent.dragStart(rowBox('top.sql'), { dataTransfer: transfer });
  fireEvent.dragOver(rowBox('reports'), { dataTransfer: transfer });
  expect(rowBox('reports')).toHaveAttribute('data-drop-target', 'true');
  fireEvent.drop(rowBox('reports'), { dataTransfer: transfer });
  expect(moveProjectItem).toHaveBeenCalledWith(
    '/work/analytics',
    'queries/top.sql',
    'queries/reports',
  );
  expect(rowBox('queries')).toHaveAttribute('draggable', 'false');
});

it('filters the tree by the sidebar search', () => {
  renderTree('month');
  expect(row('monthly.sql')).toBeVisible();
  expect(screen.queryByRole('treeitem', { name: 'top.sql' })).toBeNull();
});

it('offers Open Project… and recent projects without a project', async () => {
  const user = userEvent.setup();
  localStorage.setItem(
    'openrows.projects',
    JSON.stringify({ current: null, recent: ['/work/billing'] }),
  );
  render(
    <ThemeProvider>
      <ProjectSection
        search=""
        active={undefined}
        profiles={[]}
        onConnection={vi.fn()}
        onOpenQuery={vi.fn()}
        onMoved={vi.fn()}
      />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Open Project…' }));
  expect(chooseProject).toHaveBeenCalled();
});

it('copies a project query body rather than its path', async () => {
  const user = userEvent.setup();
  const writeText = vi.spyOn(navigator.clipboard, 'writeText');
  vi.mocked(readSqlFile).mockResolvedValueOnce({
    sql: "SELECT '${1:literal}', :id;",
    file: {
      path: '/work/analytics/queries/top.sql',
      name: 'top.sql',
      modifiedMs: null,
    },
  });
  renderTree();
  fireEvent.contextMenu(row('top.sql'));
  await user.click(screen.getByRole('menuitem', { name: 'Copy SQL' }));
  await waitFor(() => {
    expect(writeText).toHaveBeenCalledWith("SELECT '${1:literal}', :id;");
  });
});
