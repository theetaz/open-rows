// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen } from '@testing-library/react';
import { readFileSync, readdirSync } from 'node:fs';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);

const renders = vi.hoisted(() => ({ grid: vi.fn() }));
vi.mock('./DataGrid', async () => {
  const actual =
    await vi.importActual<typeof import('./DataGrid')>('./DataGrid');
  const { memo } = await import('react');
  return {
    DataGrid: memo(function MeasuredGrid(
      props: React.ComponentProps<typeof actual.DataGrid>,
    ) {
      renders.grid();
      return <actual.DataGrid {...props} />;
    }),
  };
});
beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
  vi.stubGlobal('PointerEvent', MouseEvent);
  HTMLElement.prototype.setPointerCapture = vi.fn();
});
it('PERF: 120 drag frames per panel and collapse/expand produce zero grid renders', () => {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  expect(renders.grid).toHaveBeenCalledOnce();
  renders.grid.mockClear();
  for (const label of ['Resize sidebar', 'Resize inspector']) {
    const handle = screen.getByRole('separator', { name: label });
    fireEvent.pointerDown(handle, { clientX: 300, button: 0 });
    for (let frame = 0; frame < 120; frame++)
      fireEvent.pointerMove(handle, { clientX: 300 + frame });
    fireEvent.pointerUp(handle);
  }
  for (const name of ['Toggle sidebar (⌃⌘S)', 'Toggle inspector (⌥⌘I)']) {
    fireEvent.click(screen.getByRole('button', { name }));
    fireEvent.click(screen.getByRole('button', { name }));
  }
  expect(renders.grid).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('row', { name: 'Order 10422' }));
  expect(renders.grid).toHaveBeenCalledOnce();
});
it('PERF: CSS animation frames perform no React work after the collapse event', () => {
  vi.useFakeTimers();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  fireEvent.click(screen.getByRole('button', { name: 'Toggle sidebar (⌃⌘S)' }));
  renders.grid.mockClear();
  act(() => {
    vi.advanceTimersByTime(360);
  });
  expect(renders.grid).not.toHaveBeenCalled();
  vi.useRealTimers();
});
it('layout CSS never wraps the titlebar and every mock colour/type variable exists', () => {
  const workspace = readFileSync(
    'apps/desktop/src/mock/Workspace.module.css',
    'utf8',
  );
  expect(workspace).toContain('flex-wrap: nowrap');
  expect(workspace).toContain('text-overflow: ellipsis');
  const split = readFileSync(
    'packages/ui/src/components/split/SplitPanel.module.css',
    'utf8',
  );
  expect(split).toContain('position: absolute');
  expect(split).toContain("[data-overlay='true']");
  const tokens = readFileSync('packages/ui/src/tokens/tokens.css', 'utf8');
  for (const file of readdirSync('apps/desktop/src/mock').filter((file) =>
    file.endsWith('.css'),
  )) {
    const css = readFileSync(`apps/desktop/src/mock/${file}`, 'utf8');
    for (const match of css.matchAll(/var\((--or-[\w-]+)/g))
      expect(tokens, `${file}: ${match[1] ?? ''}`).toContain(
        `${match[1] ?? ''}:`,
      );
  }
});

it('compact layouts preserve toolbar actions and reserve space below the final row', () => {
  const content = readFileSync(
    'apps/desktop/src/mock/Content.module.css',
    'utf8',
  );
  const grid = readFileSync('apps/desktop/src/mock/Grid.module.css', 'utf8');
  const inspector = readFileSync(
    'apps/desktop/src/mock/Inspector.module.css',
    'utf8',
  );
  expect(content).toMatch(/\.tableToolbar\s*\{[^}]*flex-wrap: nowrap/);
  expect(content).toMatch(
    /\.addRow\s*\{[^}]*flex-shrink: 0;[^}]*margin-inline-start: auto/,
  );
  expect(content).toMatch(
    /\.filterLabel,[^}]*overflow: hidden;[^}]*text-overflow: ellipsis/,
  );
  expect(grid).toContain(
    'padding-block-end: calc(var(--or-commit-height) + var(--or-space-24))',
  );
  expect(grid).toContain(
    'scroll-padding-block-end: calc(var(--or-commit-height) + var(--or-space-24))',
  );
  expect(inspector).toMatch(
    /\.sqlCard pre\s*\{[^}]*white-space: pre-wrap;[^}]*overflow-wrap: anywhere;[^}]*overflow: visible/,
  );
});
