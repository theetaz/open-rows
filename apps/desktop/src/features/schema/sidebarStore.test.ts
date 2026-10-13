// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { parseSidebarObject, sidebarObjectKey } from './sidebarIdentity';
import type { SidebarObject } from './sidebarIdentity';
import {
  clearRecent,
  isFavourite,
  moveFavourite,
  readSidebarObjects,
  recordRecent,
  toggleFavourite,
} from './sidebarStore';
import { objectAvailability } from './objectAvailability';
import { applySettings, collectSettings } from '../settings/settingsFile';
import {
  readSidebarPreferences,
  writeSidebarPreferences,
} from './sidebarPreferences';

const object = (name: string): SidebarObject => ({
  profileId: 'p',
  database: 'db',
  schema: 'public',
  kind: 'table',
  name,
  arguments: null,
});

it('persists favourites across a new store instance and isolates profile/database scopes', async () => {
  const first = object('orders');
  toggleFavourite(first);
  expect(isFavourite(first)).toBe(true);
  vi.resetModules();
  const restored = await import('./sidebarStore');
  expect(restored.readSidebarObjects('p', 'db').favourites).toEqual([first]);
  expect(restored.readSidebarObjects('other', 'db').favourites).toEqual([]);
  expect(restored.readSidebarObjects('p', 'other').favourites).toEqual([]);
  restored.toggleFavourite(first);
  expect(restored.readSidebarObjects('p', 'db').favourites).toEqual([]);
});

it('keeps overloaded functions and delimiter-containing scopes independently starred', () => {
  const first = {
    ...object('tax'),
    kind: 'function' as const,
    arguments: 'integer',
  };
  const second = { ...first, arguments: 'numeric' };
  toggleFavourite(first);
  toggleFavourite(second);
  expect(readSidebarObjects('p', 'db').favourites).toEqual([first, second]);
  expect(sidebarObjectKey(first)).not.toBe(sidebarObjectKey(second));
  expect(sidebarObjectKey({ ...first, profileId: 'p\ndb' })).not.toBe(
    sidebarObjectKey({ ...first, database: 'db\npublic' }),
  );
  toggleFavourite(first);
  expect(isFavourite(second)).toBe(true);
});

it('deduplicates recent objects with a 50 stored cap and clears only the chosen scope', () => {
  for (let index = 0; index < 55; index += 1)
    recordRecent(object(`table${String(index)}`));
  recordRecent(object('table20'));
  const recent = readSidebarObjects('p', 'db').recent;
  expect(recent).toHaveLength(50);
  expect(recent[0]?.name).toBe('table20');
  expect(new Set(recent.map(sidebarObjectKey)).size).toBe(50);
  recordRecent({ ...object('elsewhere'), database: 'other' });
  clearRecent('p', 'db');
  expect(readSidebarObjects('p', 'db').recent).toEqual([]);
  expect(readSidebarObjects('p', 'other').recent).toHaveLength(1);
});

it('persists favourite ordering and ignores moves of objects that are not starred', () => {
  for (const name of ['first', 'second', 'third'])
    toggleFavourite(object(name));
  moveFavourite(object('third'), 0);
  moveFavourite(object('missing'), 1);
  expect(
    readSidebarObjects('p', 'db').favourites.map((item) => item.name),
  ).toEqual(['third', 'first', 'second']);
});

it('marks absent objects missing only after a complete, settled normal section read', () => {
  const schemas = {
    status: 'ready' as const,
    data: [{ name: 'public', system: false, comment: null }],
    refreshing: false,
  };
  const page = {
    status: 'ready' as const,
    data: { objects: [], total: 0 },
    refreshing: false,
  };
  expect(
    objectAvailability(object('dropped'), {
      schemas,
      pages: [['public\ntables\n5000', page]],
    }),
  ).toBe(false);
  expect(
    objectAvailability(object('unread'), { schemas, pages: [] }),
  ).toBeNull();
  expect(
    objectAvailability(object('loading'), {
      schemas,
      pages: [['public\ntables\n5000', { ...page, refreshing: true }]],
    }),
  ).toBeNull();
  expect(
    objectAvailability(object('partial'), {
      schemas,
      pages: [
        [
          'public\ntables\n5000',
          { ...page, data: { objects: [], total: 7000 } },
        ],
      ],
    }),
  ).toBeNull();
});

it('restores profile-scoped filters and visibility through the settings file with defaults', () => {
  writeSidebarPreferences('p', {
    text: 'orders',
    kinds: ['views'],
    schemas: ['archive'],
    hidden: ['favourites'],
    systemObjects: true,
  });
  expect(readSidebarPreferences('other').hidden).toEqual([]);
  const file = collectSettings();
  localStorage.clear();
  applySettings(file);
  expect(readSidebarPreferences('p')).toEqual({
    text: 'orders',
    kinds: ['views'],
    schemas: ['archive'],
    hidden: ['favourites'],
    systemObjects: true,
  });
  expect(collectSettings().sidebar).toEqual(file.sidebar);
  expect(parseSidebarObject({ kind: 'invalid' })).toBeNull();
});
