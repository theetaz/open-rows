// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { cancelQuery, revealExport, startExport } from '@openrows/ipc';
import type { ExportEventDto } from '@openrows/ipc';
import { JobsButton } from '../jobs';
import { ExportSheet } from './ExportSheet';
import { exportSettings } from './exportSettings';
import { resultExport, tableExport } from './sources';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

let emit: (event: ExportEventDto) => void = () => undefined;

function started(fileName: string): void {
  vi.mocked(startExport).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    return Promise.resolve({
      status: 'started',
      exportId: 'export-1',
      fileName,
    });
  });
}

beforeEach(() => {
  started('users.sql');
});

const users = tableExport({
  sessionId: 'session-1',
  profileId: 'profile-1',
  database: 'shop',
  schema: 'public',
  name: 'users',
});

function renderSheet(props = users, onClose = vi.fn()): typeof onClose {
  render(
    <ThemeProvider>
      <JobsButton />
      <ExportSheet {...props} onClose={onClose} />
    </ThemeProvider>,
  );
  return onClose;
}

it('starts a table export as a background job that reports progress and its file', async () => {
  const user = userEvent.setup();
  const onClose = renderSheet();
  await user.selectOptions(screen.getByLabelText('Format'), 'SQL INSERT');
  expect(screen.getByLabelText('Table name')).toHaveValue('users');
  const rows = screen.getByLabelText('Rows per INSERT');
  await user.clear(rows);
  await user.type(rows, '0');
  expect(screen.getByRole('button', { name: 'Export…' })).toBeDisabled();
  await user.clear(rows);
  await user.type(rows, '250');
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenCalledWith(
    {
      source: users.scopes[0]?.source,
      format: { kind: 'sql', table: 'users', rowsPerStatement: 250 },
      fileName: 'users',
    },
    expect.any(Function),
  );
  expect(onClose).toHaveBeenCalled();
  const job = await screen.findByRole('listitem', { name: 'Export users.sql' });
  act(() => {
    emit({ type: 'progress', rows: 4000, detail: null });
  });
  expect(within(job).getByText('4,000 rows written')).toBeInTheDocument();
  act(() => {
    emit({ type: 'finished', rows: 10_000, path: '/tmp/users.sql' });
  });
  expect(within(job).getByText('10,000 rows exported')).toBeInTheDocument();
  await user.click(within(job).getByRole('button', { name: 'Show in Finder' }));
  expect(revealExport).toHaveBeenCalledWith('/tmp/users.sql');
});

it('cancels a running export from the jobs popover', async () => {
  started('users.csv');
  const user = userEvent.setup();
  renderSheet();
  await user.selectOptions(screen.getByLabelText('Format'), 'CSV');
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  const job = await screen.findByRole('listitem', { name: 'Export users.csv' });
  await user.click(within(job).getByRole('button', { name: 'Cancel' }));
  expect(cancelQuery).toHaveBeenCalledWith('export-1');
  act(() => {
    emit({ type: 'cancelled' });
  });
  expect(
    within(job).getByText('Cancelled. No file was written.'),
  ).toBeInTheDocument();
});

it('stays open when the save panel is dismissed and shows why an export could not start', async () => {
  const user = userEvent.setup();
  vi.mocked(startExport).mockResolvedValueOnce({ status: 'dismissed' });
  const onClose = renderSheet();
  await user.selectOptions(screen.getByLabelText('Format'), 'JSON');
  await user.selectOptions(
    screen.getByLabelText('Layout'),
    'Pretty (indented)',
  );
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenLastCalledWith(
    expect.objectContaining({ format: { kind: 'json', pretty: true } }),
    expect.any(Function),
  );
  expect(await screen.findByRole('button', { name: 'Export…' })).toBeEnabled();
  expect(onClose).not.toHaveBeenCalled();
  vi.mocked(startExport).mockRejectedValueOnce(new Error('The disk is full'));
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'The disk is full',
  );
});

it('remembers the last format and its options for the next export', async () => {
  const user = userEvent.setup();
  renderSheet();
  await user.selectOptions(screen.getByLabelText('Format'), 'TSV');
  await user.selectOptions(
    screen.getByLabelText('Line endings'),
    'LF (macOS, Linux)',
  );
  await user.type(screen.getByLabelText('NULL as'), 'NULL');
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenLastCalledWith(
    expect.objectContaining({
      format: {
        kind: 'tsv',
        options: expect.objectContaining({
          lineEnding: 'lf',
          nullText: 'NULL',
        }) as unknown,
      },
    }),
    expect.any(Function),
  );
  expect(exportSettings.get()).toMatchObject({
    format: 'tsv',
    tsv: { lineEnding: 'lf', nullText: 'NULL' },
  });
});

it('offers the selected rows and every row of a result the row limit cut short', async () => {
  const user = userEvent.setup();
  renderSheet(
    resultExport({
      sessionId: 'session-1',
      result: { id: 'result-1', rows: 1000, truncated: true },
      statement: {
        index: 0,
        sql: 'SELECT * FROM users',
        kind: 'read',
        dangerous: false,
        verdict: 'allow',
      },
      selected: [7, 3],
    }),
  );
  expect(
    screen.getByRole('radio', { name: 'The 2 rows selected' }),
  ).toBeChecked();
  expect(
    screen.getByRole('radio', { name: 'The 1,000 rows received (row limit)' }),
  ).not.toBeChecked();
  await user.click(
    screen.getByRole('radio', { name: 'Every row: run the statement again' }),
  );
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenLastCalledWith(
    expect.objectContaining({
      source: {
        kind: 'query',
        sessionId: 'session-1',
        sql: 'SELECT * FROM users',
      },
      fileName: 'result',
    }),
    expect.any(Function),
  );
});
