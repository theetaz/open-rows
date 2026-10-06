// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { initialFiles } from '../../scripts/initial-bundle';
it('counts all static initial chunks once, excluding deferred chunks', () => {
  expect(
    initialFiles({
      entry: { file: 'entry.js', isEntry: true, imports: ['shared', 'other'] },
      shared: { file: 'shared.js', imports: ['entry'] },
      other: { file: 'other.js', imports: ['shared'] },
      lazy: { file: 'deferred.js' },
    }),
  ).toEqual(['entry.js', 'shared.js', 'other.js']);
});
it('fails rather than reporting an incomplete initial bundle', () => {
  expect(() => initialFiles({})).toThrow('no initial entry');
  expect(() =>
    initialFiles({
      entry: { file: 'entry.js', isEntry: true, imports: ['missing'] },
    }),
  ).toThrow('Missing build manifest dependency');
});
