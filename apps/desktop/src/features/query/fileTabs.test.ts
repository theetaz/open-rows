// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { movedPath } from './fileTabs';

it('maps files at or under a moved path to its new place', () => {
  expect(movedPath('/p/q/a.sql', '/p/q/a.sql', '/p/q/b.sql')).toBe(
    '/p/q/b.sql',
  );
  expect(movedPath('/p/q/r/a.sql', '/p/q/r', '/p/q/s')).toBe('/p/q/s/a.sql');
  expect(movedPath('/p/q/rx/a.sql', '/p/q/r', '/p/q/s')).toBeNull();
  expect(movedPath('C:\\p\\r\\a.sql', 'C:\\p\\r', 'C:\\p\\s')).toBe(
    'C:\\p\\s\\a.sql',
  );
});
