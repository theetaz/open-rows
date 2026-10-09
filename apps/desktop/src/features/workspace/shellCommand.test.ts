// SPDX-License-Identifier: Apache-2.0
import { renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { useTableCommands } from '../commands';
import { shellCommand } from './shellCommand';

function shell(): {
  run: (id: string) => void;
  input: Parameters<typeof shellCommand>[0];
} {
  const input = {
    catalog: {},
    connections: { setSheet: vi.fn() },
    inspector: { open: true, toggle: vi.fn() },
    toggleSidebar: vi.fn(),
    setSidebar: vi.fn(),
    setMode: vi.fn(),
    addQuery: vi.fn(),
    closeTab: vi.fn(),
    onExportTable: vi.fn(),
    onImport: vi.fn(),
    openFile: vi.fn(),
    refresh: vi.fn(),
    toggleSafe: vi.fn(),
    onGallery: vi.fn(),
    onSettings: vi.fn(),
    setTheme: vi.fn(),
  } as unknown as Parameters<typeof shellCommand>[0];
  return { run: shellCommand(input), input };
}

it('sends table commands to the active table tab and Refresh to both', () => {
  const handle = vi.fn();
  const view = renderHook(() => {
    useTableCommands(handle);
  });
  const { run, input } = shell();
  run('insert-row');
  run('commit-changes');
  expect(handle.mock.calls).toEqual([['insert-row'], ['commit-changes']]);
  run('refresh');
  expect(input.refresh).toHaveBeenCalledOnce();
  expect(handle).toHaveBeenLastCalledWith('refresh');
  view.unmount();
  run('delete-row');
  run('refresh');
  expect(handle).toHaveBeenCalledTimes(3);
  expect(input.refresh).toHaveBeenCalledTimes(2);
});

it('exports the table without a query tab, and Help search opens the palette', () => {
  const { run, input } = shell();
  run('export-results');
  expect(input.onExportTable).toHaveBeenCalledOnce();
  run('help-search');
  expect(input.setMode).toHaveBeenCalledWith('commands');
});

it('opens the import sheet from File ▸ Import… and the palette', () => {
  const { run, input } = shell();
  run('import-data');
  expect(input.onImport).toHaveBeenCalledOnce();
});
