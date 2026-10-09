// SPDX-License-Identifier: Apache-2.0
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { cancelQuery, startExport } from '@openrows/ipc';
import type { ExportEventDto } from '@openrows/ipc';
import { ExportSheet } from './ExportSheet';
import { resultExport, tableExport } from './sources';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

let emit: (event: ExportEventDto) => void = () => undefined;

beforeEach(() => {
  vi.mocked(startExport).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    return Promise.resolve({
      status: 'started',
      exportId: 'export-1',
      fileName: 'users.sql',
    });
  });
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
      <ExportSheet {...props} onClose={onClose} />
    </ThemeProvider>,
  );
  return onClose;
}

it('exports a table as SQL INSERTs with progress and a row-count notice', async () => {
  const user = userEvent.setup();
  const onClose = renderSheet();
  await user.click(screen.getByRole('radio', { name: 'SQL' }));
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
      source: users.source,
      format: { kind: 'sql', table: 'users', rowsPerStatement: 250 },
      fileName: 'users',
    },
    expect.any(Function),
  );
  act(() => {
    emit({ type: 'progress', rows: 4000 });
  });
  expect(await screen.findByText('4,000 rows written')).toBeInTheDocument();
  expect(screen.getByText('Exporting to users.sql…')).toBeInTheDocument();
  act(() => {
    emit({ type: 'finished', rows: 10_000, path: '/tmp/users.sql' });
  });
  expect(
    screen.getByText('Exported 10,000 rows to users.sql.'),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Done' }));
  expect(onClose).toHaveBeenCalled();
});

it('cancels a running export and says nothing was written', async () => {
  const user = userEvent.setup();
  renderSheet();
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenLastCalledWith(
    expect.objectContaining({
      format: { kind: 'csv', delimiter: 'comma', header: true },
    }),
    expect.any(Function),
  );
  await user.click(
    await screen.findByRole('button', { name: 'Cancel export' }),
  );
  expect(cancelQuery).toHaveBeenCalledWith('export-1');
  act(() => {
    emit({ type: 'cancelled' });
  });
  expect(
    screen.getByText('Export cancelled. No file was written.'),
  ).toBeInTheDocument();
});

it('returns to the options when the save panel is dismissed or the export fails', async () => {
  const user = userEvent.setup();
  vi.mocked(startExport).mockResolvedValueOnce({ status: 'dismissed' });
  renderSheet();
  await user.click(screen.getByRole('radio', { name: 'JSON' }));
  await user.selectOptions(
    screen.getByLabelText('Layout'),
    'One object per line (NDJSON)',
  );
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenLastCalledWith(
    expect.objectContaining({ format: { kind: 'json', lines: true } }),
    expect.any(Function),
  );
  expect(await screen.findByRole('button', { name: 'Export…' })).toBeEnabled();
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  act(() => {
    emit({
      type: 'failed',
      error: { kind: 'storage', message: 'The disk is full', code: null },
    });
  });
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'The disk is full',
  );
});

it('offers every row of a result the row limit cut short', async () => {
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
    }),
  );
  expect(
    screen.getByRole('radio', { name: 'The 1,000 rows received (row limit)' }),
  ).toBeChecked();
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
