// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { objectChange, routineDefinition } from '@openrows/ipc';
import type { ObjectRequest, ProfileDto } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { ObjectEditors } from './ObjectEditors';
import type { ObjectEditorRequest } from './requests';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('@openrows/editor', async () =>
  (await import('../../test/editorStub')).editorStub(),
);

const done = vi.fn();
const closed = vi.fn();

beforeEach(() => {
  done.mockReset();
  closed.mockReset();
  vi.mocked(objectChange).mockClear();
});

function show(
  request: ObjectEditorRequest,
  connection: ProfileDto = profile('orders-local'),
): void {
  render(
    <ThemeProvider>
      <ObjectEditors
        request={request}
        profile={connection}
        opened={['orders']}
        onCloseDatabase={vi.fn()}
        onClose={closed}
        onDone={done}
      />
    </ThemeProvider>,
  );
}

/** The last request sent for review (no token) or run (with one). */
const lastRequest = (): ObjectRequest | undefined =>
  vi.mocked(objectChange).mock.lastCall?.[0];

it('drops a table after showing exactly the statement and its warnings', async () => {
  const user = userEvent.setup();
  show({
    kind: 'drop',
    target: {
      object: 'table',
      database: 'orders',
      schema: 'public',
      name: 'orders',
    },
  });
  const sheet = screen.getByRole('dialog', { name: 'Drop table orders' });
  expect(
    await within(sheet).findByLabelText('Statements to run'),
  ).toHaveTextContent('DROP TABLE "public"."orders";');
  expect(within(sheet).getByLabelText('Warnings')).toHaveTextContent(
    'deleted permanently',
  );
  expect(lastRequest()).toMatchObject({
    profileId: 'id-orders-local',
    database: 'orders',
    change: {
      kind: 'dropTable',
      target: { schema: 'public', name: 'orders' },
      cascade: false,
    },
    confirmToken: null,
  });
  await user.click(within(sheet).getByRole('checkbox', { name: /CASCADE/ }));
  await waitFor(() => {
    expect(lastRequest()?.change).toMatchObject({ cascade: true });
  });
  const drop = within(sheet).getByRole('button', { name: 'Drop' });
  await waitFor(() => {
    expect(drop).toBeEnabled();
  });
  await user.click(drop);
  expect(lastRequest()).toMatchObject({
    confirmToken: 'object-review',
    confirmName: null,
  });
  await waitFor(() => {
    expect(done).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'drop' }),
      expect.objectContaining({ kind: 'dropTable', cascade: true }),
    );
  });
});

it('asks for the typed name before a guarded destructive change runs', async () => {
  const user = userEvent.setup();
  vi.mocked(objectChange).mockResolvedValueOnce({
    kind: 'prepared',
    token: 'guarded',
    statements: ['TRUNCATE TABLE "public"."orders"'],
    warnings: [
      'This is a production connection: check every statement before committing.',
    ],
    destructive: true,
    confirmName: 'orders',
    transactional: true,
  });
  show({
    kind: 'truncateTable',
    database: 'orders',
    schema: 'public',
    name: 'orders',
  });
  const sheet = screen.getByRole('dialog', { name: 'Truncate table orders' });
  const field = await within(sheet).findByRole('textbox', {
    name: /Type orders to continue/,
  });
  const truncate = within(sheet).getByRole('button', { name: 'Truncate' });
  expect(truncate).toBeDisabled();
  await user.type(field, 'orders');
  expect(truncate).toBeEnabled();
  vi.mocked(objectChange).mockResolvedValueOnce({ kind: 'committed' });
  await user.click(truncate);
  expect(lastRequest()).toMatchObject({
    confirmToken: 'guarded',
    confirmName: 'orders',
  });
});

it('creates a table from the new-table editor with keys and an index', async () => {
  const user = userEvent.setup();
  show({ kind: 'newTable', database: 'orders', schema: 'public' });
  const sheet = screen.getByRole('dialog', { name: 'New table in public' });
  expect(within(sheet).getByText('Name the table.')).toBeVisible();
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Table name' }),
    'notes',
  );
  await user.click(within(sheet).getByRole('button', { name: 'Add column' }));
  const name = within(sheet).getByRole('textbox', { name: 'Name' });
  await user.clear(name);
  await user.type(name, 'body');
  await user.click(
    within(sheet).getByRole('button', { name: 'Add to Indexes' }),
  );
  await user.click(within(sheet).getByRole('checkbox', { name: /^body/ }));
  await user.click(within(sheet).getByRole('button', { name: 'Add' }));
  await waitFor(() => {
    expect(lastRequest()?.change).toMatchObject({
      kind: 'createTable',
      target: { schema: 'public', name: 'notes' },
      definition: {
        primaryKey: ['id'],
        indexes: [{ columns: ['body'] }],
      },
    });
  });
  const change = lastRequest()?.change;
  if (change?.kind !== 'createTable') throw new Error('no create');
  expect(change.definition.columns.map((column) => column.name)).toEqual([
    'id',
    'body',
  ]);
  expect(change.definition.columns[0]).toMatchObject({
    identity: 'always',
    dataType: 'bigint',
  });
});

it('edits a view query in the SQL editor', async () => {
  const user = userEvent.setup();
  show({
    kind: 'view',
    database: 'orders',
    schema: 'public',
    name: null,
    materialized: false,
  });
  const sheet = screen.getByRole('dialog', { name: 'New view' });
  await user.type(
    within(sheet).getByRole('textbox', { name: 'Name' }),
    'recent',
  );
  const query = await within(sheet).findByRole('textbox', {
    name: 'View query',
  });
  await user.type(query, 'orders');
  await user.click(
    within(sheet).getByRole('checkbox', { name: 'Materialized' }),
  );
  await waitFor(() => {
    expect(lastRequest()?.change).toEqual({
      kind: 'saveView',
      target: { schema: 'public', name: 'recent' },
      query: 'SELECT\n  *\nFROM\n  orders',
      materialized: true,
      replace: false,
    });
  });
  expect(
    within(sheet).getByRole('heading', { name: 'New materialized view' }),
  ).toBeVisible();
});

it('reads a routine definition and saves it as a replacement', async () => {
  show({
    kind: 'routine',
    database: 'orders',
    schema: 'public',
    routine: 'function',
    name: 'touch',
    arguments: '',
  });
  const sheet = screen.getByRole('dialog', { name: 'Edit function touch' });
  const definition = await within(sheet).findByRole('textbox', {
    name: 'Definition',
  });
  expect(definition).toHaveValue(
    'CREATE OR REPLACE FUNCTION public.touch()\n RETURNS trigger\n LANGUAGE plpgsql\nAS $function$ BEGIN RETURN NEW; END $function$\n',
  );
  expect(routineDefinition).toHaveBeenCalledWith({
    profileId: 'id-orders-local',
    database: 'orders',
    schema: 'public',
    name: 'touch',
    routine: 'function',
    arguments: '',
  });
  await waitFor(() => {
    expect(lastRequest()?.change).toMatchObject({
      kind: 'saveRoutine',
      target: { schema: 'public', name: 'touch' },
      replaces: '',
    });
  });
});

it('names MySQL character sets when creating a database', () => {
  const mysql = profile('shop', { engine: 'mysql' });
  show({ kind: 'database', database: 'orders', name: null }, mysql);
  expect(screen.getByLabelText('Character set')).toBeVisible();
  expect(screen.queryByLabelText('Encoding')).toBeNull();
});

it('says an open database must be closed before it is renamed', () => {
  show({ kind: 'database', database: 'orders', name: 'orders' });
  const sheet = screen.getByRole('dialog', { name: 'Rename database orders' });
  expect(within(sheet).getByText(/is open in OpenRows/)).toBeVisible();
  expect(
    within(sheet).getByRole('button', { name: 'Close orders' }),
  ).toBeVisible();
  expect(within(sheet).getByRole('button', { name: 'Rename' })).toBeDisabled();
});
