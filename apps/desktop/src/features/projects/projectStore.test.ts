// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { chooseProject, loadProject } from '@openrows/ipc';
import type { ProjectDto } from '@openrows/ipc';
import {
  closeProject,
  openProject,
  openProjectDialog,
  parseStored,
  useProject,
  withRecent,
} from './projectStore';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const project = (root: string): ProjectDto => ({
  root,
  name: root.slice(1),
  defaultConnection: null,
  queryFolders: ['queries'],
  entries: [{ path: 'queries', kind: 'folder' }],
  truncated: false,
});

it('reads stored roots defensively and keeps recent projects unique', () => {
  expect(parseStored(null)).toEqual({ current: null, recent: [] });
  expect(parseStored('{ broken')).toEqual({ current: null, recent: [] });
  expect(parseStored('{"current":"/a","recent":["/a",3,"/b"]}')).toEqual({
    current: '/a',
    recent: ['/a', '/b'],
  });
  expect(withRecent(['/a', '/b', '/c'], '/b')).toEqual(['/b', '/a', '/c']);
  expect(
    withRecent(
      Array.from({ length: 9 }, (_, i) => `/${String(i)}`),
      '/x',
    ),
  ).toHaveLength(8);
});

it('opens, remembers and closes projects', async () => {
  vi.mocked(chooseProject).mockResolvedValueOnce(project('/one'));
  vi.mocked(loadProject).mockResolvedValueOnce(project('/two'));
  const { result } = renderHook(() => useProject());
  act(() => {
    openProjectDialog();
  });
  await waitFor(() => {
    expect(result.current.project?.root).toBe('/one');
  });
  act(() => {
    openProject('/two');
  });
  await waitFor(() => {
    expect(result.current.project?.root).toBe('/two');
  });
  expect(result.current.recent).toEqual(['/two', '/one']);
  expect(parseStored(localStorage.getItem('openrows.projects')).current).toBe(
    '/two',
  );
  act(() => {
    closeProject();
  });
  expect(result.current.project).toBeNull();
  vi.mocked(loadProject).mockRejectedValueOnce(new Error('gone'));
  act(() => {
    openProject('/three');
  });
  await waitFor(() => {
    expect(result.current.error).toBe('gone');
  });
});
