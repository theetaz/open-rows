// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  cancelImport,
  previewImport,
  startImport,
  tableEditInfo,
} from '@openrows/ipc';
import type { EditInfoDto, ImportEventDto } from '@openrows/ipc';
import { samplePreview } from '../../test/importStub';
import { ImportSheet } from './ImportSheet';
import type { ImportContext } from './model';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

let emit: (event: ImportEventDto) => void = () => undefined;

const users: EditInfoDto = {
  key: ['id'],
  protected: [],
  rowId: null,
  columns: [
    {
      name: 'id',
      dataType: 'integer',
      nullable: false,
      default: null,
      comment: null,
      primaryKey: true,
      enumLabels: null,
    },
    {
      name: 'name',
      dataType: 'text',
      nullable: true,
      default: null,
      comment: null,
      primaryKey: false,
      enumLabels: null,
    },
  ],
  readOnly: false,
  confirmWrites: false,
  production: false,
};

const context: ImportContext = {
  profileId: 'profile-1',
  sessionId: 'session-1',
  database: 'shop',
  schema: 'public',
  table: 'users',
  readOnly: false,
};

beforeEach(() => {
  vi.mocked(tableEditInfo).mockResolvedValue(users);
  vi.mocked(previewImport).mockImplementation((request) =>
    Promise.resolve({
      ...samplePreview,
      // The core flags `x2` once `ID` feeds the integer `id` column.
      invalid: request.types[0] === 'integer' ? [{ row: 1, column: 0 }] : [],
    }),
  );
  vi.mocked(startImport).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    return Promise.resolve({ importId: 'import-1' });
  });
});

function renderSheet(props: Partial<ImportContext> = {}): () => void {
  const onClose = vi.fn();
  render(
    <ThemeProvider>
      <ImportSheet context={{ ...context, ...props }} onClose={onClose} />
    </ThemeProvider>,
  );
  return onClose;
}

async function pickFile(
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await user.click(screen.getByRole('button', { name: 'Choose File…' }));
  await screen.findByRole('table', { name: 'File preview' });
}

it('maps by name, highlights rejected values and checks conflicts', async () => {
  const user = userEvent.setup();
  renderSheet();
  await pickFile(user);
  // The controls exist before the defaults effect applies the name mapping.
  await waitFor(() => {
    expect(screen.getByLabelText('Source of id')).toHaveValue('column:0');
    expect(screen.getByLabelText('Source of name')).toHaveValue('column:1');
    expect(screen.getByLabelText('id identifies rows')).toBeChecked();
  });
  const preview = screen.getByRole('table', { name: 'File preview' });
  const bad = await within(preview).findByText('x2');
  // The highlight arrives with the preview checked against the mapping.
  await waitFor(() => {
    expect(bad).toHaveAttribute('data-invalid', 'true');
  });
  expect(bad).toHaveAttribute('title', 'Not a valid integer');
  expect(within(preview).getByText('NULL')).toHaveAttribute(
    'data-null',
    'true',
  );
  await user.click(screen.getByRole('button', { name: 'Check Conflicts' }));
  expect(startImport).toHaveBeenCalledWith(
    expect.objectContaining({
      mode: 'check',
      key: ['id'],
      columns: [
        { target: 'id', source: { kind: 'column', index: 0 } },
        { target: 'name', source: { kind: 'column', index: 1 } },
      ],
    }),
    expect.any(Function),
  );
  act(() => {
    emit({
      type: 'finished',
      summary: {
        rows: 2,
        inserted: 0,
        skipped: 0,
        failed: 1,
        conflicts: 1,
        errors: [
          { line: 3, column: 'id', message: '"x2" is not a valid integer' },
        ],
      },
    });
  });
  expect(
    await screen.findByText(/Checked 2 rows: 1 with errors. 1 row collides/),
  ).toBeInTheDocument();
  expect(screen.getByRole('list', { name: 'Row errors' })).toHaveTextContent(
    'Line 3id"x2" is not a valid integer',
  );
});

it('confirms, imports with progress and reports the summary', async () => {
  const user = userEvent.setup();
  const onClose = renderSheet();
  await pickFile(user);
  await waitFor(() => {
    expect(screen.getByLabelText('Source of id')).toHaveValue('column:0');
  });
  await user.selectOptions(
    screen.getByLabelText('When a key already exists'),
    'update',
  );
  await user.click(screen.getByLabelText('Delete all rows of the table first'));
  await user.click(screen.getByRole('button', { name: 'Import…' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Every existing row of the table is deleted first.',
  );
  await user.click(screen.getByRole('button', { name: 'Import' }));
  expect(startImport).toHaveBeenLastCalledWith(
    expect.objectContaining({
      mode: 'import',
      conflict: 'update',
      truncate: true,
    }),
    expect.any(Function),
  );
  act(() => {
    emit({
      type: 'progress',
      rows: 1000,
      inserted: 990,
      failed: 10,
      fraction: 0.5,
      rowsPerSecond: 2000,
    });
  });
  expect(await screen.findByText(/50% · 1,000 rows read/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Cancel Import' }));
  expect(cancelImport).toHaveBeenCalledWith('import-1');
  act(() => {
    emit({
      type: 'finished',
      summary: {
        rows: 2,
        inserted: 2,
        skipped: 0,
        failed: 0,
        conflicts: null,
        errors: [],
      },
    });
  });
  expect(screen.getByText('Imported 2 rows into users.')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Done' }));
  expect(onClose).toHaveBeenCalled();
});

it('creates a new table from edited names and types', async () => {
  const user = userEvent.setup();
  renderSheet({ table: null });
  await pickFile(user);
  await user.type(screen.getByLabelText('New table in public'), 'people');
  const name = screen.getByLabelText('Name of ID');
  await user.clear(name);
  await user.type(name, 'id');
  const type = screen.getByLabelText('Type of ID');
  await user.clear(type);
  await user.type(type, 'bigint');
  await user.click(screen.getByLabelText('ID is in the primary key'));
  await user.click(screen.getByLabelText('Import name'));
  await user.click(screen.getByRole('button', { name: 'Import…' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'The table is created first.',
  );
  await user.click(screen.getByRole('button', { name: 'Import' }));
  expect(vi.mocked(startImport).mock.lastCall?.[0]).toMatchObject({
    target: { table: 'people' },
    columns: [{ target: 'id', source: { kind: 'column', index: 0 } }],
    create: { types: ['bigint'], primaryKey: ['id'] },
  });
});

it('blocks imports on read-only connections but still checks', async () => {
  const user = userEvent.setup();
  renderSheet({ readOnly: true });
  await pickFile(user);
  await waitFor(() => {
    expect(screen.getByLabelText('Source of id')).toHaveValue('column:0');
  });
  expect(screen.getByRole('button', { name: 'Import…' })).toBeDisabled();
  expect(
    screen.getByText('This connection is read-only: imports are disabled.'),
  ).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Check Conflicts' })).toBeEnabled();
});
