// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { listSchemas, searchCatalog, tableStructure } from '@openrows/ipc';
import type { ObjectDto } from '@openrows/ipc';
import { completionCatalog, currentCatalog } from './completionCatalog';
import { cursorText } from './cursor';
import { defaultEditorSettings, parseEditorSettings } from './editorSettings';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const object = (name: string, kind: ObjectDto['kind']): ObjectDto => ({
  schema: 'public',
  name,
  kind,
  arguments: null,
  rowEstimate: null,
  comment: null,
});

it('reads each catalog part once per revision and maps object kinds', async () => {
  vi.mocked(searchCatalog).mockResolvedValue([
    object('users', 'table'),
    object('active_users', 'materializedView'),
    object('touch', 'procedure'),
    object('users_id_seq', 'sequence'),
  ]);
  const target = { profileId: 'p1', database: 'orders', revision: '1:0' };
  const catalog = completionCatalog(target);
  expect(completionCatalog(target)).toBe(catalog);
  expect(await catalog.objects()).toEqual([
    { schema: 'public', name: 'users', kind: 'table' },
    { schema: 'public', name: 'active_users', kind: 'view' },
    { schema: 'public', name: 'touch', kind: 'function' },
  ]);
  await catalog.objects();
  expect(searchCatalog).toHaveBeenCalledOnce();
  const refreshed = completionCatalog({ ...target, revision: '1:1' });
  expect(refreshed).not.toBe(catalog);
  await refreshed.objects();
  expect(searchCatalog).toHaveBeenCalledTimes(2);
});

it('keeps failed reads for the revision instead of retrying per keystroke', async () => {
  vi.mocked(tableStructure).mockRejectedValue(new Error('no such table'));
  const catalog = completionCatalog({
    profileId: 'p2',
    database: 'orders',
    revision: '1:0',
  });
  await expect(catalog.columns('public', 'nope')).rejects.toThrow();
  await expect(catalog.columns('public', 'nope')).rejects.toThrow();
  expect(tableStructure).toHaveBeenCalledOnce();
});

it('reads through the current catalog and offers nothing offline', async () => {
  vi.mocked(listSchemas).mockResolvedValue([
    { name: 'public', system: false, comment: null },
  ]);
  let online = false;
  const catalog = currentCatalog(() =>
    online
      ? completionCatalog({ profileId: 'p3', database: 'd', revision: '1' })
      : null,
  );
  expect(await catalog.schemas()).toEqual([]);
  online = true;
  expect(await catalog.schemas()).toEqual([{ name: 'public', system: false }]);
});

it('validates persisted editor settings field by field', () => {
  expect(parseEditorSettings(null)).toEqual(defaultEditorSettings);
  expect(parseEditorSettings('{oops')).toEqual(defaultEditorSettings);
  expect(
    parseEditorSettings(
      JSON.stringify({
        rowLimit: 1_000,
        fontSize: 7,
        keyBinding: 'vim',
        wrap: 'yes',
      }),
    ),
  ).toEqual({ ...defaultEditorSettings, rowLimit: 1_000, keyBinding: 'vim' });
});

it('describes the cursor like TablePlus', () => {
  expect(cursorText({ line: 3, column: 7, location: 42 })).toBe(
    'line 3, column 7, location 42',
  );
});
