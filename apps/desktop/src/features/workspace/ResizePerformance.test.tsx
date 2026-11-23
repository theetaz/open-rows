// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { readFileSync, readdirSync } from 'node:fs';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import type { SelectedCell } from '../query';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const renders = vi.hoisted(() => ({ content: vi.fn() }));
vi.mock('../query', async () => {
  const actual = await vi.importActual<typeof import('../query')>('../query');
  return {
    ...actual,
    // A selected cell gives the inspector something to show, so it opens.
    CellDetails: () => null,
    QueryContent: function MeasuredContent(
      props: React.ComponentProps<typeof actual.QueryContent>,
    ) {
      renders.content();
      const { onCell } = props;
      const selected = useRef(false);
      useEffect(() => {
        if (selected.current) return;
        selected.current = true;
        onCell({
          row: 0,
          index: 0,
          column: 'id',
          type: 'int4',
          text: '1',
        } as unknown as SelectedCell);
      }, [onCell]);
      return <actual.QueryContent {...props} />;
    },
  };
});
beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
  vi.stubGlobal('PointerEvent', MouseEvent);
  HTMLElement.prototype.setPointerCapture = vi.fn();
});
it('PERF: 120 drag frames per panel produce zero query content renders', () => {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  renders.content.mockClear();
  for (const label of ['Resize sidebar', 'Resize inspector']) {
    const handle = screen.getByRole('separator', { name: label });
    fireEvent.pointerDown(handle, { clientX: 300, button: 0 });
    for (let frame = 0; frame < 120; frame++)
      fireEvent.pointerMove(handle, { clientX: 300 + frame });
    fireEvent.pointerUp(handle);
  }
  expect(renders.content).not.toHaveBeenCalled();
});
it('PERF: CSS animation frames perform no React work after the collapse event', () => {
  vi.useFakeTimers();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  fireEvent.click(screen.getByRole('button', { name: 'Toggle sidebar (⌃⌘S)' }));
  renders.content.mockClear();
  act(() => {
    vi.advanceTimersByTime(360);
  });
  expect(renders.content).not.toHaveBeenCalled();
  vi.useRealTimers();
});
it('layout CSS never wraps the titlebar and every colour/type variable exists', () => {
  const workspace = readFileSync(
    'apps/desktop/src/features/workspace/Workspace.module.css',
    'utf8',
  );
  expect(workspace).toMatch(/\.toolbar\s*\{[^}]*white-space: nowrap/);
  expect(workspace).toMatch(/\.toolbar\s*\{[^}]*grid-template-columns/);
  expect(workspace).toContain('text-overflow: ellipsis');
  const split = readFileSync(
    'packages/ui/src/components/split/SplitPanel.module.css',
    'utf8',
  );
  // The inspector is a column of its own: it never overlays the content.
  expect(split).not.toContain('data-overlay');
  const tokens = readFileSync('packages/ui/src/tokens/tokens.css', 'utf8');
  for (const folder of [
    'apps/desktop/src/features/workspace',
    'apps/desktop/src/features/query',
  ]) {
    for (const file of readdirSync(folder).filter((name) =>
      name.endsWith('.css'),
    )) {
      const css = readFileSync(`${folder}/${file}`, 'utf8');
      for (const match of css.matchAll(/var\((--or-[\w-]+)/g))
        expect(tokens, `${file}: ${match[1] ?? ''}`).toContain(
          `${match[1] ?? ''}:`,
        );
    }
  }
});

it('query toolbar and status never wrap', () => {
  const query = readFileSync(
    'apps/desktop/src/features/query/Query.module.css',
    'utf8',
  );
  const bar = readFileSync(
    'apps/desktop/src/features/query/EditorBar.module.css',
    'utf8',
  );
  expect(bar).toMatch(/\.bar\s*\{[^}]*white-space: nowrap/);
  expect(query).toMatch(/\.status\s*\{[^}]*text-overflow: ellipsis/);
});
