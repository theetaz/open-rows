// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { emptyDraft } from './form';
import { PASSWORD_MASK, composeUrl, splitUrl } from './urlText';

vi.mock('@openrows/ipc', () => ({ DEFAULT_CUSTOM_ENV_COLOR: '#7c3aed' }));

it('masks a userinfo password for display and removes it before parsing', () => {
  const split = splitUrl('postgres://app:p%40ss-synthetic@db.test:6543/orders');
  expect(split.masked).toBe(
    `postgres://app:${PASSWORD_MASK}@db.test:6543/orders`,
  );
  expect(split.stripped).toBe('postgres://app@db.test:6543/orders');
  expect(split.password).toEqual({ kind: 'typed', value: 'p@ss-synthetic' });
});

it('masks a password query parameter and keeps the query well formed', () => {
  expect(splitUrl('postgres://db?password=x&sslmode=require')).toMatchObject({
    masked: `postgres://db?password=${PASSWORD_MASK}&sslmode=require`,
    stripped: 'postgres://db?sslmode=require',
  });
  expect(splitUrl('postgres://db?sslmode=disable&password=x').stripped).toBe(
    'postgres://db?sslmode=disable',
  );
});

it('keeps the current password when the mask is still in place', () => {
  expect(splitUrl(`postgres://app:${PASSWORD_MASK}@other/db`).password).toEqual(
    {
      kind: 'masked',
    },
  );
  expect(splitUrl('postgres://app@db/orders').password).toEqual({
    kind: 'none',
  });
  expect(splitUrl('postgres://app:%E0%A4%A@db').password).toEqual({
    kind: 'invalid',
  });
});

it('composes the URL from fields, masking typed and saved passwords', () => {
  const draft = { ...emptyDraft(), user: 'app', database: 'orders' };
  expect(composeUrl(draft)).toBe('postgres://app@localhost:5432/orders');
  expect(composeUrl({ ...draft, password: 'secret-synthetic' })).toBe(
    `postgres://app:${PASSWORD_MASK}@localhost:5432/orders`,
  );
  expect(composeUrl({ ...draft, hasStoredPassword: true })).toContain(
    PASSWORD_MASK,
  );
  expect(
    composeUrl({ ...draft, host: 'db.test', tlsMode: 'require', user: '' }),
  ).toBe('postgres://db.test:5432/orders?sslmode=require');
  expect(
    composeUrl({ ...draft, host: 'db.test', tlsMode: 'verify-full' }),
  ).toBe('postgres://app@db.test:5432/orders');
});
