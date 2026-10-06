// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  assertInitialModules,
  initialFiles,
} from '../../scripts/initial-bundle';
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

it('rejects denied modules in the static graph, including shared chunks', () => {
  const files = initialFiles({
    entry: { file: 'entry.js', isEntry: true, imports: ['shared'] },
    shared: { file: 'shared.js' },
    lazy: { file: 'lazy.js' },
  });
  expect(() => {
    assertInitialModules(
      files,
      {
        'entry.js': [],
        'shared.js': [
          '/repo/node_modules/.pnpm/apache-arrow@21/node_modules/apache-arrow/Arrow.js',
        ],
      },
      ['apache-arrow'],
    );
  }).toThrow('apache-arrow found in initial chunk shared.js');
});
it('allows denied modules in lazy and worker chunks and matches package boundaries', () => {
  expect(() => {
    assertInitialModules(
      ['entry.js'],
      {
        'entry.js': ['/repo/node_modules/apache-arrow-helper/index.js'],
        'lazy.js': ['/repo/node_modules/apache-arrow/Arrow.js'],
        'worker.js': ['/repo/node_modules/apache-arrow/Arrow.js'],
      },
      ['apache-arrow'],
    );
  }).not.toThrow();
});
it('supports configurable scoped packages and Windows module paths', () => {
  expect(() => {
    assertInitialModules(
      ['entry.js'],
      {
        'entry.js': ['C:\\repo\\node_modules\\@heavy\\decoder\\index.js'],
      },
      ['@heavy/decoder'],
    );
  }).toThrow('@heavy/decoder found in initial chunk');
});
it('fails closed if an initial chunk has no module metadata', () => {
  expect(() => {
    assertInitialModules(['missing.js'], {}, ['apache-arrow']);
  }).toThrow('Missing build module metadata: missing.js');
});
