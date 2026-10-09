// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import type { BoundFunctions, queries } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  invalidateCatalog,
  listObjects,
  listSchemas,
  onCatalogChanged,
  searchCatalog,
} from '@openrows/ipc';
import type { CatalogChanged, CatalogReply, SchemaDto } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from '../workspace/Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});

const sidebar = (): BoundFunctions<typeof queries> =>
  within(screen.getByRole('complementary', { name: 'Sidebar' }));

function mount(): void {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
}

const publicSchemas: SchemaDto[] = [
  { name: 'public', system: false, comment: null },
];

it('renders a snapshot at once, marks it refreshing and re-reads it fresh in the background', async () => {
  let release: (reply: CatalogReply<SchemaDto[]>) => void = () => undefined;
  vi.mocked(listSchemas)
    .mockResolvedValueOnce({ data: publicSchemas, stale: true })
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    );
  mount();
  await connectTo();
  expect(
    await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' }),
  ).toBeVisible();
  expect(sidebar().getByText('Refreshing…')).toBeVisible();
  expect(listSchemas).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders',
    true,
  );
  await act(async () => {
    release({ data: publicSchemas, stale: false });
    await Promise.resolve();
  });
  expect(sidebar().queryByText('Refreshing…')).toBeNull();
});

it('re-reads loaded sections fresh after DDL on the connection, and ⌘R marks the cache stale', async () => {
  mount();
  await connectTo();
  await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' });
  const listener = vi.mocked(onCatalogChanged).mock.calls.at(-1)?.[0] as
    ((event: CatalogChanged) => void) | undefined;
  await act(async () => {
    listener?.({ profileId: 'id-orders-local', database: 'orders' });
    await Promise.resolve();
  });
  expect(listObjects).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders',
    { schema: 'public', kinds: ['table', 'foreignTable'] },
    true,
  );
  fireEvent.keyDown(sidebar().getByRole('button', { name: /^Tables/ }), {
    key: 'r',
    metaKey: true,
  });
  expect(invalidateCatalog).toHaveBeenCalledWith('id-orders-local');
});

it('searches the selected user schema through the name index', async () => {
  mount();
  await connectTo();
  await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' });
  fireEvent.change(screen.getByRole('textbox', { name: 'Search items' }), {
    target: { value: 'cust' },
  });
  expect(
    await sidebar().findByRole('treeitem', { name: 'customers ~412K' }),
  ).toBeVisible();
  expect(
    await sidebar().findByRole('treeitem', { name: 'active_customers' }),
  ).toBeVisible();
  expect(searchCatalog).toHaveBeenLastCalledWith('id-orders-local', 'orders', {
    query: 'cust',
    schema: 'public',
    limit: 1000,
    fresh: false,
  });
});
