// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import type { GridHandle, GridProps } from '@openrows/grid';
import {
  tableEdit,
  tableEditInfo,
  tableEditRow,
  tableEditRows,
} from '@openrows/ipc';
import { displayWindow } from '../../results/displayWindow';
import { useTableViews } from '../table/useTableViews';
import { RowDetails } from '../table/RowDetails';
import type { SelectedRow } from '../table/row';
import { useState } from 'react';
import { ResultPane } from './ResultPane';
import { ResultWorkspaceProvider } from './ResultWorkspace';
import { idleRun } from './run';
import type { ResultMeta } from './run';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('@openrows/grid/canvas', () => ({ mountGrid: vi.fn() }));
vi.mock('../../results/displayWindow', () => ({ displayWindow: vi.fn() }));
const records = [
  { id: '3', name: 'Cherry', active: 'true' },
  { id: '1', name: 'Apple', active: 'false' },
];
const result: ResultMeta = {
  id: 'query-result',
  statement: 0,
  rows: 2,
  done: true,
  truncated: false,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'name', sourceType: 'text' },
    { name: 'active', sourceType: 'bool' },
  ],
};
const onExport = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  vi.mocked(tableEditInfo).mockResolvedValue({
    key: ['id'],
    protected: ['id'],
    rowId: null,
    readOnly: false,
    confirmWrites: false,
    production: false,
    columns: result.columns.map((column) => ({
      name: column.name,
      dataType: column.sourceType ?? 'text',
      nullable: false,
      default: null,
      primaryKey: column.name === 'id',
      comment: null,
      enumLabels: null,
    })),
  });
  vi.mocked(tableEditRow).mockImplementation((_target, keys) =>
    Promise.resolve(records.find((row) => row.id === keys.id) ?? {}),
  );
  vi.mocked(tableEditRows).mockImplementation((_target, keys) =>
    Promise.resolve(
      keys.map((key) => records.find((row) => row.id === key.id) ?? {}),
    ),
  );
  vi.mocked(displayWindow).mockImplementation((request) =>
    Promise.resolve({
      columns: request.colCount,
      convertMs: 0,
      cells: records
        .slice(request.rowStart, request.rowStart + request.rowCount)
        .flatMap((row) =>
          Object.values(row)
            .slice(request.colStart, request.colStart + request.colCount)
            .map((text) => ({ text, state: 'normal' as const })),
        ),
    }),
  );
  vi.mocked(mountGrid).mockImplementation((_host, props) => {
    void props.source.window(
      {
        r0: 0,
        r1: Math.max(0, props.source.rows - 1),
        c0: 0,
        c1: props.source.columns.length - 1,
      },
      0,
    );
    return {
      destroy: vi.fn(),
      viewport: { x: 0, y: 0, width: 800, height: 400 },
      scrollTo: vi.fn(),
      relayout: vi.fn(),
      refresh: vi.fn(),
      fitWidth: () => 140,
      cellRect: () => ({ x: 200, y: 28, width: 140, height: 28 }),
      focus: vi.fn(),
      announce: vi.fn(),
      select: (row: number, column: number) => {
        props.onSelection({
          row,
          column,
          ranges: [{ r0: row, r1: row, c0: column, c1: column }],
        });
      },
    } as unknown as GridHandle;
  });
});

function App({
  sql = 'SELECT * FROM public.users',
}: {
  sql?: string;
}): React.JSX.Element {
  const views = useTableViews();
  const [row, setRow] = useState<SelectedRow | null>(null);
  const run = {
    ...idleRun,
    status: 'completed' as const,
    queryId: 'q',
    results: [result],
    statements: [
      {
        index: 0,
        sql,
        kind: 'read' as const,
        dangerous: false,
        verdict: 'allow' as const,
      },
    ],
  };
  return (
    <ThemeProvider>
      <ResultWorkspaceProvider
        value={{
          tabId: 'tab',
          profileId: 'synthetic',
          database: 'db',
          sessionId: 'session',
          views,
          onRow: setRow,
          onOpenTable: vi.fn(),
        }}
      >
        <ResultPane
          run={run}
          view="tabs"
          runDefault="current"
          engine="postgres"
          onCell={vi.fn()}
          onExport={onExport}
        />
        {row && <RowDetails row={row} />}
        <output aria-label="Protected edits">
          {views.entryOf('tab').edits.current.length}
        </output>
      </ResultWorkspaceProvider>
    </ThemeProvider>
  );
}
function grid(): GridProps {
  return vi.mocked(mountGrid).mock.calls.at(-1)?.[1] as GridProps;
}
async function ready(): Promise<void> {
  await waitFor(() => {
    expect(screen.getByRole('button', { name: '+ Row' })).toBeEnabled();
  });
  await waitFor(() => {
    expect(grid().source.cell(0, 2)?.text).toBe('Cherry');
  });
}
async function stageName(value: string): Promise<void> {
  act(() => {
    grid().onEdit?.({ row: 0, column: 2 });
  });
  const input = await screen.findByRole('textbox', { name: 'Edit name' });
  await waitFor(() => {
    expect(input).not.toBeDisabled();
  });
  const user = userEvent.setup();
  await user.clear(input);
  await user.type(input, `${value}{Enter}`);
  await waitFor(() => {
    expect(screen.getByLabelText('Protected edits')).toHaveTextContent('1');
  });
}

it('stages an inline query edit, highlights it, supports undo/redo and commits the reviewed SQL', async () => {
  render(<App />);
  await ready();
  const mounted = vi.mocked(mountGrid).mock.calls.length;
  await stageName('Edited');
  expect(tableEdit).not.toHaveBeenCalled();
  expect(grid().source.cell(0, 2)).toMatchObject({
    text: 'Edited',
    state: 'changed',
  });
  expect(mountGrid).toHaveBeenCalledTimes(mounted);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Undo' }));
  expect(grid().source.cell(0, 2)?.text).toBe('Cherry');
  await user.click(screen.getByRole('button', { name: 'Redo' }));
  expect(grid().source.cell(0, 2)?.text).toBe('Edited');
  vi.mocked(tableEdit).mockResolvedValueOnce({
    kind: 'prepared',
    token: 'review-token',
    statements: [
      {
        sql: 'UPDATE "public"."users" SET "name" = $1 WHERE "id" = $2',
        params: ['Edited', '3'],
      },
    ],
  });
  await user.click(screen.getByRole('button', { name: 'Review & commit' }));
  expect(await screen.findByLabelText('SQL to commit')).toHaveTextContent(
    'UPDATE "public"."users"',
  );
  expect(vi.mocked(tableEdit).mock.calls[0]?.[0]).toMatchObject({
    target: { schema: 'public', table: 'users' },
    confirmToken: null,
    changes: [
      {
        kind: 'update',
        original: records[0],
        values: { name: { kind: 'text', value: 'Edited' } },
      },
    ],
  });
  vi.mocked(tableEdit).mockResolvedValueOnce({ kind: 'committed' });
  await user.click(screen.getByRole('button', { name: 'Commit changes' }));
  await waitFor(() => {
    expect(screen.getByLabelText('Protected edits')).toHaveTextContent('0');
  });
  expect(vi.mocked(tableEdit).mock.calls[1]?.[0].confirmToken).toBe(
    'review-token',
  );
  expect(
    screen.getByRole('status', { name: 'Result status' }),
  ).toHaveTextContent('out of date');
});

it('edits the stored key after sorting and keeps a conflict staged', async () => {
  render(<App />);
  await ready();
  act(() => {
    grid().onHeader?.(1);
  });
  await waitFor(() => {
    expect(grid().source.cell(0, 2)?.text).toBe('Apple');
  });
  await stageName('Sorted edit');
  expect(tableEditRow).toHaveBeenLastCalledWith(expect.anything(), { id: '1' });
  expect(grid().source.cell(0, 2)).toMatchObject({
    state: 'changed',
    text: 'Sorted edit',
  });
  vi.mocked(tableEdit).mockRejectedValueOnce(
    new Error('Conflict; transaction rolled back'),
  );
  await userEvent
    .setup()
    .click(screen.getByRole('button', { name: 'Review & commit' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Conflict');
  expect(screen.getByLabelText('Protected edits')).toHaveTextContent('1');
});

it('shows row Details fields and places Export beside Filter rows with no extra result bar', async () => {
  const { container } = render(<App />);
  await ready();
  expect(
    await screen.findByRole('searchbox', { name: 'Search for field' }),
  ).toBeVisible();
  const toolbar = screen.getByRole('toolbar', { name: 'Result toolbar' });
  expect(
    within(toolbar).getByRole('textbox', { name: 'Filter rows' }),
  ).toBeVisible();
  await userEvent
    .setup()
    .click(within(toolbar).getByRole('button', { name: 'Export…' }));
  expect(onExport).toHaveBeenCalledWith(result);
  expect(container.querySelector('[class*="resultBar"]')).toBeNull();
  expect(grid().rowHeaderColumn).toBe(0);
  expect(grid().source.cell(1, 0)?.text).toBe('2');
  expect(grid().source.columns[3]?.kind).toBe('boolean');
  act(() => {
    grid().onColumnResize?.(2, 220, true);
  });
  expect(grid().source.columns[2]?.width).toBe(220);
});

it('explains unsafe results and does not ask the backend for editing metadata', async () => {
  render(<App sql="SELECT DISTINCT * FROM public.users" />);
  await waitFor(() => {
    expect(mountGrid).toHaveBeenCalled();
  });
  expect(
    screen.getByRole('status', { name: 'Result status' }),
  ).toHaveTextContent('DISTINCT');
  expect(screen.getByRole('button', { name: '+ Row' })).toBeDisabled();
  expect(tableEditInfo).not.toHaveBeenCalled();
  act(() => {
    grid().onEdit?.({ row: 0, column: 2 });
  });
  expect(screen.queryByRole('textbox', { name: 'Edit name' })).toBeNull();
});

it('keeps hidden keys available for editing and stages insert, duplicate and deletion without writes', async () => {
  render(<App />);
  await ready();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Columns' }));
  await user.click(screen.getByRole('checkbox', { name: 'id' }));
  await waitFor(() => {
    expect(grid().source.columns[1]?.title).toBe('name');
  });
  act(() => {
    grid().onContextMenu?.({ kind: 'row', row: 0, column: 0, x: 10, y: 10 });
  });
  await user.click(screen.getByRole('menuitem', { name: 'Duplicate row' }));
  await waitFor(() => {
    expect(grid().source.rows).toBe(3);
  });
  expect(grid().source.cell(2, 0)?.text).toBe('+');
  expect(grid().source.cell(2, 1)?.text).toBe('Cherry');
  act(() => {
    grid().onContextMenu?.({ kind: 'row', row: 1, column: 0, x: 10, y: 10 });
  });
  await user.click(screen.getByRole('menuitem', { name: 'Delete row' }));
  await waitFor(() => {
    expect(grid().source.cell(1, 1)?.state).toBe('deleted');
  });
  await user.click(screen.getByRole('button', { name: '+ Row' }));
  await waitFor(() => {
    expect(grid().source.rows).toBe(4);
  });
  expect(screen.getByLabelText('Protected edits')).toHaveTextContent('3');
  expect(tableEdit).not.toHaveBeenCalled();
});
