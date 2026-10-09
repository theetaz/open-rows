// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { listObjects } from './catalog';
import { CommandError, deleteProfile, listProfiles } from './connections';

const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));

it('returns command data and calls the snake_case command', async () => {
  mockInvoke.mockResolvedValue({ profiles: [], unreadable: [] });
  expect(await listProfiles()).toEqual({ profiles: [], unreadable: [] });
  expect(mockInvoke).toHaveBeenCalledWith('profiles_list');
});

it('rejects with a typed CommandError carrying kind and SQLSTATE', async () => {
  mockInvoke.mockRejectedValue({
    kind: 'notFound',
    message: 'profile x not found',
    code: null,
  });
  const error = await deleteProfile('x').catch((failure: unknown) => failure);
  expect(error).toBeInstanceOf(CommandError);
  expect(error).toMatchObject({
    kind: 'notFound',
    message: 'profile x not found',
  });
});

it('wraps plain Tauri string rejections as internal errors', async () => {
  mockInvoke.mockRejectedValue('Command profiles_list not allowed');
  await expect(listProfiles()).rejects.toMatchObject({
    kind: 'internal',
    message: 'Command profiles_list not allowed',
  });
});

it('reads catalog objects with the profile, database, schema, kinds and freshness', async () => {
  mockInvoke.mockResolvedValue({ data: [], stale: true });
  const section = { schema: 'public', kinds: ['table' as const] };
  expect(await listObjects('p', 'orders', section, false)).toEqual({
    data: [],
    stale: true,
  });
  expect(mockInvoke).toHaveBeenCalledWith('catalog_objects', {
    profileId: 'p',
    database: 'orders',
    request: { schema: 'public', kinds: ['table'], fresh: false },
  });
});
