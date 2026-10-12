// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import type { ObjectDto, ObjectKindDto } from '@openrows/ipc';
import { ObjectTree } from './ObjectTree';
import { SidebarFilters } from './SidebarFilters';
import { QuickOpen } from './CatalogPalettes';
import { objectRef } from './catalog';
import type { Catalog } from './useCatalog';
import type { IndexQuery } from './useCatalogIndex';
import type { Resource } from './useResources';
import {
  readSidebarObjects,
  recordRecent,
  toggleFavourite,
} from './sidebarStore';
import { writeSidebarPreferences } from './sidebarPreferences';

const ready = <T,>(data: T): Resource<T> => ({
  status: 'ready',
  data,
  refreshing: false,
});
const row = (
  name: string,
  kind: ObjectKindDto = 'table',
  schema = 'public',
): ObjectDto => ({
  name,
  kind,
  schema,
  arguments: null,
  rowEstimate: null,
  comment: null,
});
const saved = (object: ObjectDto) => ({
  ...objectRef('db', object),
  profileId: 'p',
});

function fixture(
  objects: ObjectDto[],
  kinds: ObjectKindDto[] = ['table', 'view', 'function'],
): Catalog {
  return {
    profileId: 'p',
    engine: 'postgres',
    database: 'db',
    schema: 'public',
    offline: null,
    opened: ['db'],
    openDatabase: vi.fn(),
    closeDatabase: vi.fn(),
    setSchema: vi.fn(),
    systemSchemas: false,
    setSystemSchemas: vi.fn(),
    databases: ready([{ name: 'db', system: false, comment: null }]),
    schemas: ready(
      ['public', 'archive', 'pg_catalog'].map((name) => ({
        name,
        system: name === 'pg_catalog',
        comment: null,
      })),
    ),
    objects: (group, _limit, schema = 'public') =>
      ready({
        objects: objects.filter(
          (object) =>
            object.schema === schema && group.kinds.includes(object.kind),
        ),
        total: objects.filter(
          (object) =>
            object.schema === schema && group.kinds.includes(object.kind),
        ).length,
      }),
    loadObjects: vi.fn(),
    available: (object) =>
      objects.some(
        (item) =>
          item.schema === object.schema &&
          item.name === object.name &&
          item.kind === object.kind,
      ),
    inspection: {
      capabilities: ready({ kinds, extensions: false, roles: false }),
      extensions: { status: 'idle' },
      roles: { status: 'idle' },
      load: vi.fn(),
    },
    refreshing: false,
    revision: 0,
    refresh: vi.fn(),
    expanded: () => false,
    setExpanded: vi.fn(),
    columns: () => ({ status: 'idle' }),
    loadColumns: vi.fn(),
    index: ready(objects.length),
    loadIndex: vi.fn(),
    searchIndex: vi.fn(({ text, schema }: IndexQuery) =>
      Promise.resolve(
        objects.filter(
          (object) =>
            (!schema || schema === object.schema) && object.name.includes(text),
        ),
      ),
    ),
  };
}
function mount(catalog: Catalog, search = '') {
  const onOpen = vi.fn();
  const view = render(
    <ThemeProvider>
      <SidebarFilters catalog={catalog} />
      <ObjectTree catalog={catalog} search={search} onOpen={onOpen} />
    </ThemeProvider>,
  );
  return { ...view, onOpen, user: userEvent.setup() };
}

it('keeps a favourite across tree refresh, marks a dropped object missing, and removes it', async () => {
  const object = row('orders');
  toggleFavourite(saved(object));
  const catalog = fixture([object]);
  const view = mount(catalog);
  expect(
    within(screen.getByRole('tree', { name: 'Favourites' })).getByRole(
      'treeitem',
      { name: 'public.orders' },
    ),
  ).toBeVisible();
  view.rerender(
    <ThemeProvider>
      <ObjectTree catalog={fixture([])} search="" onOpen={view.onOpen} />
    </ThemeProvider>,
  );
  const missing = screen.getByRole('treeitem', {
    name: 'public.orders (missing)',
  });
  expect(missing).toBeVisible();
  fireEvent.keyDown(missing, { key: 'Enter' });
  expect(view.onOpen).not.toHaveBeenCalled();
  await view.user.click(
    screen.getByRole('button', { name: 'Remove public.orders' }),
  );
  expect(readSidebarObjects('p', 'db').favourites).toEqual([]);
  expect(catalog.loadObjects).not.toHaveBeenCalled();
});

it('shows the last 15 recent objects and Clear empties the section', async () => {
  const objects = Array.from({ length: 20 }, (_, index) =>
    row(`object${String(index)}`),
  );
  for (const object of objects) recordRecent(saved(object));
  const { user } = mount(fixture(objects));
  const tree = screen.getByRole('tree', { name: 'Recent' });
  expect(within(tree).getAllByRole('treeitem')).toHaveLength(15);
  expect(within(tree).getAllByRole('treeitem')[0]).toHaveAccessibleName(
    'public.object19',
  );
  await user.click(
    screen.getByRole('button', { name: 'Clear recent objects' }),
  );
  expect(within(tree).queryAllByRole('treeitem')).toHaveLength(0);
});

it('reorders favourites with Alt+arrows, retains focus and opens with Enter', async () => {
  const objects = [row('first'), row('second')];
  objects.forEach((object) => {
    toggleFavourite(saved(object));
  });
  const { user, onOpen } = mount(fixture(objects));
  const tree = screen.getByRole('tree', { name: 'Favourites' });
  const second = within(tree).getByRole('treeitem', { name: 'public.second' });
  second.focus();
  await user.keyboard('{Alt>}{ArrowUp}{/Alt}');
  expect(within(tree).getAllByRole('treeitem')[0]).toBe(second);
  expect(second).toHaveFocus();
  await user.keyboard('{Enter}');
  expect(onOpen).toHaveBeenCalledWith(saved(objects[1] ?? row('second')));
});

it('combines PostgreSQL kind, multi-schema and text filters without filtering saved sections', async () => {
  const objects = [
    row('orders'),
    row('report', 'view'),
    row('report_old', 'view', 'archive'),
  ];
  toggleFavourite(saved(objects[0] ?? row('orders')));
  const { user } = mount(fixture(objects), 'report');
  const choose = async (submenu: string, item: string): Promise<void> => {
    await user.click(screen.getByRole('button', { name: 'Filter objects' }));
    await user.click(screen.getByRole('menuitem', { name: submenu }));
    await user.click(screen.getByRole('menuitemcheckbox', { name: item }));
  };
  await choose('Object kinds', 'Views');
  await choose('Schemas', 'public');
  await choose('Schemas', 'archive');
  expect(await screen.findByRole('treeitem', { name: 'report' })).toBeVisible();
  expect(
    await screen.findByRole('treeitem', { name: 'report_old' }),
  ).toBeVisible();
  expect(screen.queryByRole('tree', { name: 'Tables' })).toBeNull();
  expect(screen.getByRole('treeitem', { name: 'public.orders' })).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Filter objects' }));
  await user.click(screen.getByRole('menuitem', { name: 'Object kinds' }));
  expect(
    screen.getByRole('menuitemcheckbox', { name: 'Functions' }),
  ).toBeVisible();
});

it('offers only SQLite-supported kinds and hides sqlite_% objects until enabled', async () => {
  const catalog = {
    ...fixture(
      [row('orders', 'table', 'main'), row('sqlite_sequence', 'table', 'main')],
      ['table', 'view'],
    ),
    schema: 'main',
    schemas: ready([{ name: 'main', system: false, comment: null }]),
    engine: 'sqlite' as const,
  };
  const view = mount(catalog);
  await view.user.click(screen.getByRole('button', { name: 'Filter objects' }));
  await view.user.click(screen.getByRole('menuitem', { name: 'Object kinds' }));
  expect(
    screen.queryByRole('menuitemcheckbox', { name: 'Functions' }),
  ).toBeNull();
  await view.user.keyboard('{Escape}');
  expect(
    screen.queryByRole('treeitem', { name: 'sqlite_sequence' }),
  ).toBeNull();
  view.rerender(
    <ThemeProvider>
      <ObjectTree
        catalog={{ ...catalog, systemSchemas: true }}
        search=""
        onOpen={view.onOpen}
      />
    </ThemeProvider>,
  );
  expect(
    screen.getByRole('treeitem', { name: 'sqlite_sequence' }),
  ).toBeVisible();
});

it('hides PostgreSQL system sections by default and shows pg_catalog when enabled', () => {
  const catalog = {
    ...fixture([row('pg_class', 'table', 'pg_catalog')]),
    schema: 'pg_catalog',
  };
  const view = mount(catalog);
  expect(screen.queryByRole('treeitem', { name: 'pg_class' })).toBeNull();
  writeSidebarPreferences('p', { systemObjects: true });
  view.rerender(
    <ThemeProvider>
      <ObjectTree
        catalog={{ ...catalog, systemSchemas: true }}
        search=""
        onOpen={view.onOpen}
      />
    </ThemeProvider>,
  );
  expect(screen.getByRole('treeitem', { name: 'pg_class' })).toBeVisible();
});

it('persists section hiding from the header menu and keeps quick open independent', async () => {
  const catalog = fixture([row('orders')]);
  const { user, unmount } = mount(catalog);
  await user.click(screen.getByRole('button', { name: 'Filter objects' }));
  await user.click(screen.getByRole('menuitem', { name: 'Sections' }));
  await user.click(screen.getByRole('menuitemcheckbox', { name: 'Tables' }));
  expect(screen.queryByRole('tree', { name: 'Tables' })).toBeNull();
  unmount();
  mount(catalog);
  expect(screen.queryByRole('tree', { name: 'Tables' })).toBeNull();
  render(
    <ThemeProvider>
      <QuickOpen catalog={catalog} onChoose={vi.fn()} onClose={vi.fn()} />
    </ThemeProvider>,
  );
  expect(await screen.findByRole('option', { name: /orders/ })).toBeVisible();
});

it('keeps favourites removable after the last browsable schema is dropped', async () => {
  toggleFavourite(saved(row('orders')));
  const catalog = { ...fixture([]), schema: '', schemas: ready([]) };
  const { user } = mount(catalog);
  expect(
    screen.getByRole('treeitem', { name: 'public.orders (missing)' }),
  ).toBeVisible();
  await user.click(
    screen.getByRole('button', { name: 'Remove public.orders' }),
  );
  expect(readSidebarObjects('p', 'db').favourites).toEqual([]);
});
