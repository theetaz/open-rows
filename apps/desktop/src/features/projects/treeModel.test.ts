// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { ProjectDto } from '@openrows/ipc';
import {
  absolutePath,
  canMove,
  filterTree,
  parentOf,
  projectTree,
  stem,
} from './treeModel';

const project: ProjectDto = {
  root: '/work/analytics',
  name: 'Analytics',
  defaultConnection: null,
  queryFolders: ['queries', '.'],
  entries: [
    { path: 'queries', kind: 'folder' },
    { path: 'queries/reports', kind: 'folder' },
    { path: 'queries/reports/monthly.sql', kind: 'query' },
    { path: 'queries/top.sql', kind: 'query' },
    { path: '.', kind: 'folder' },
    { path: 'scratch.sql', kind: 'query' },
  ],
  truncated: false,
};

it('builds one root per query folder with nested folders and queries', () => {
  const roots = projectTree(project);
  expect(roots.map((node) => node.name)).toEqual(['queries', 'Analytics']);
  const [queries, top] = roots;
  expect(queries?.children.map((node) => node.name)).toEqual([
    'reports',
    'top.sql',
  ]);
  expect(queries?.children[0]?.children[0]?.path).toBe(
    'queries/reports/monthly.sql',
  );
  expect(top?.children.map((node) => node.path)).toEqual(['scratch.sql']);
});

it('filters by name and keeps the ancestors of matches', () => {
  const shown = filterTree(projectTree(project), 'MONTH');
  expect(shown).toHaveLength(1);
  expect(shown[0]?.children[0]?.children[0]?.name).toBe('monthly.sql');
  expect(filterTree(projectTree(project), '  ')).toHaveLength(2);
  expect(filterTree(projectTree(project), 'zzz')).toEqual([]);
});

it('joins paths with the root separator and checks moves', () => {
  expect(absolutePath('/work/p/', 'queries/a.sql')).toBe(
    '/work/p/queries/a.sql',
  );
  expect(absolutePath('C:\\work\\p', 'queries/a.sql')).toBe(
    'C:\\work\\p\\queries\\a.sql',
  );
  expect(absolutePath('/work/p', '.')).toBe('/work/p');
  expect(parentOf('a.sql')).toBe('.');
  expect(parentOf('queries/a.sql')).toBe('queries');
  expect(stem('Top.SQL')).toBe('Top');
  expect(canMove('queries/a.sql', 'queries/reports')).toBe(true);
  expect(canMove('queries/a.sql', 'queries')).toBe(false);
  expect(canMove('queries/reports', 'queries/reports/inner')).toBe(false);
  expect(canMove('queries/reports', 'queries/reports')).toBe(false);
});
