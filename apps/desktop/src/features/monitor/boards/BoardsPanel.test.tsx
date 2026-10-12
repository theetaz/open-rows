// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import * as ipc from '@openrows/ipc';
import type { Board, BoardDto } from '@openrows/ipc';
import { BoardsPanel } from './BoardsPanel';

vi.mock('@openrows/ipc', () => ({
  listBoards: vi.fn(),
  saveBoard: vi.fn(),
  importBoard: vi.fn(),
  exportBoard: vi.fn(),
  approveBoard: vi.fn(),
  deleteBoard: vi.fn(),
  runBoardWidget: vi.fn(),
  confirmDestructive: vi.fn(() => Promise.resolve(null)),
}));

const target = { profileId: 'p', database: 'shop' };
const connection = {
  name: 'Local',
  engine: 'postgres' as const,
  database: 'shop',
};
const definition: Board = {
  version: 1,
  name: 'Sales',
  params: [
    { name: 'region', kind: 'enum', default: 'eu', options: ['eu', 'us'] },
  ],
  widgets: [
    {
      id: 'w1',
      title: 'Orders',
      kind: 'stat',
      sql: 'SELECT count(*) FROM orders WHERE region = :region',
      chart: null,
      limits: { maxRows: 1_000, timeoutMs: 15_000, maxBytes: 1_048_576 },
    },
  ],
  refreshSecs: 60,
};
const dto = (reviewed: boolean, board = definition): BoardDto => ({
  id: 'b1',
  reviewed,
  board,
  updatedAtMs: 1,
});

beforeEach(() => {
  vi.mocked(ipc.runBoardWidget).mockReset();
  vi.mocked(ipc.approveBoard).mockReset();
  vi.mocked(ipc.runBoardWidget).mockResolvedValue({
    result: {
      columns: [{ name: 'count', kind: 'number' }],
      rows: [['42']],
      truncated: null,
    },
    readOnly: 'enforced',
    elapsedMs: 3,
  });
});

it('shows an imported board paused with its queries and never runs it', async () => {
  vi.mocked(ipc.listBoards).mockResolvedValue([dto(false)]);
  vi.mocked(ipc.approveBoard).mockResolvedValue();
  render(<BoardsPanel target={target} connection={connection} active />);
  const review = await screen.findByRole('region', {
    name: 'Review imported board',
  });
  expect(
    within(review).getByText(definition.widgets[0]?.sql ?? ''),
  ).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Run' })).toHaveProperty(
    'disabled',
    true,
  );
  vi.mocked(ipc.listBoards).mockResolvedValue([dto(true)]);
  await userEvent.click(
    within(review).getByRole('button', { name: 'Approve and enable' }),
  );
  expect(ipc.approveBoard).toHaveBeenCalledWith('p', 'b1');
  await screen.findByRole('button', { name: 'Run' });
  expect(ipc.runBoardWidget).not.toHaveBeenCalled();
});

it('runs only on an explicit Run, binding typed values', async () => {
  vi.mocked(ipc.listBoards).mockResolvedValue([dto(true)]);
  render(<BoardsPanel target={target} connection={connection} active />);
  const run = await screen.findByRole('button', { name: 'Run' });
  expect(ipc.runBoardWidget).not.toHaveBeenCalled();
  await userEvent.selectOptions(screen.getByLabelText('region'), 'us');
  await userEvent.click(run);
  expect(await screen.findByText('42')).toBeTruthy();
  expect(ipc.runBoardWidget).toHaveBeenCalledWith({
    target,
    boardId: 'b1',
    widgetId: 'w1',
    values: [{ name: 'region', value: 'us' }],
    automatic: false,
  });
  expect(screen.getByRole('button', { name: 'Stop refresh' })).toBeTruthy();
});

it('offers one clear Add widget action on an empty board', async () => {
  vi.mocked(ipc.listBoards).mockResolvedValue([
    dto(true, { ...definition, params: [], widgets: [] }),
  ]);
  render(<BoardsPanel target={target} connection={connection} active />);
  const add = await screen.findAllByRole('button', { name: 'Add widget' });
  expect(add).toHaveLength(1);
  await userEvent.click(add[0] as HTMLElement);
  expect(screen.getByRole('region', { name: 'Edit widget' })).toBeTruthy();
});
