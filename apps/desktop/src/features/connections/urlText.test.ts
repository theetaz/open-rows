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

it('composes MySQL, MariaDB and SQLite URLs with their own schemes', () => {
  const mysql = { ...emptyDraft('mysql'), user: 'app', database: 'shop' };
  expect(composeUrl(mysql)).toBe('mysql://app@localhost:3306/shop');
  expect(composeUrl({ ...mysql, host: 'db.test', tlsMode: 'verify-ca' })).toBe(
    'mysql://app@db.test:3306/shop?ssl-mode=VERIFY_CA',
  );
  expect(composeUrl({ ...mysql, host: 'db.test', tlsMode: 'allow' })).toBe(
    'mysql://app@db.test:3306/shop?sslmode=allow',
  );
  const maria = { ...emptyDraft('mariadb'), user: 'root' };
  expect(composeUrl(maria)).toBe('mariadb://root@localhost:3306');
  const lite = { ...emptyDraft('sqlite'), filePath: '/data/my shop.sqlite' };
  expect(composeUrl(lite)).toBe('sqlite:/data/my%20shop.sqlite');
  expect(composeUrl({ ...lite, fileReadOnly: true })).toBe(
    'sqlite:/data/my%20shop.sqlite?mode=ro',
  );
  expect(composeUrl({ ...lite, fileMemory: true, fileReadOnly: true })).toBe(
    'sqlite::memory:',
  );
});

it('composes SQL Server URLs with encrypt options', () => {
  const server = { ...emptyDraft('sqlserver'), user: 'sa', database: 'shop' };
  expect(composeUrl(server)).toBe('sqlserver://sa@localhost:1433/shop');
  expect(composeUrl({ ...server, host: 'db.test', tlsMode: 'require' })).toBe(
    'sqlserver://sa@db.test:1433/shop?encrypt=true&trustServerCertificate=true',
  );
  expect(composeUrl({ ...server, host: 'db.test', tlsMode: 'allow' })).toBe(
    'sqlserver://sa@db.test:1433/shop?encrypt=false',
  );
  expect(composeUrl({ ...server, tlsMode: 'disable' })).toBe(
    'sqlserver://sa@localhost:1433/shop?sslmode=disable',
  );
});

it('masks a JDBC password property', () => {
  const split = splitUrl(
    'jdbc:sqlserver://db.test:1433;databaseName=shop;password=x;encrypt=true',
  );
  expect(split.password).toEqual({ kind: 'typed', value: 'x' });
  expect(split.masked).toContain(`password=${PASSWORD_MASK};`);
  expect(split.stripped).toBe(
    'jdbc:sqlserver://db.test:1433;databaseName=shop;encrypt=true',
  );
});
