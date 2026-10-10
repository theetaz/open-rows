// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import type { Connections } from '../connections';
import type { QueryTabs } from '../query';
import { acceptNativeFile, localFileScope } from './useNativeFiles';

it('opens SQL into a new editor tab without any execution or connection request', () => {
  const open = vi.fn();
  const connect = vi.fn();
  const setSheet = vi.fn();
  const tabs = { open } as unknown as QueryTabs;
  const connections = { connect, setSheet } as unknown as Connections;
  const file = {
    kind: 'sql' as const,
    opened: {
      file: { path: '/query.sql', name: 'query.sql', modifiedMs: null },
      sql: 'DROP TABLE example;',
    },
  };
  expect(acceptNativeFile(file, { tabs, connections })).toBeNull();
  expect(open.mock.calls[0]?.[0]).toEqual(localFileScope);
  expect(open.mock.calls[0]?.[1] as unknown).toMatchObject({
    sql: 'DROP TABLE example;',
    file: { saved: 'DROP TABLE example;' },
  });
  expect(connect).not.toHaveBeenCalled();
  expect(setSheet).not.toHaveBeenCalled();
  acceptNativeFile(
    { kind: 'database', path: '/example.sqlite' },
    { tabs, connections },
  );
  expect(setSheet).toHaveBeenCalledWith({
    kind: 'new-file',
    path: '/example.sqlite',
  });
  expect(
    acceptNativeFile(
      { kind: 'rejected', message: 'Binary rejected' },
      { tabs, connections },
    ),
  ).toBe('Binary rejected');
  expect(open).toHaveBeenCalledTimes(1);
});
