// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { mountGrid } from './canvas';
import { shadowGrid } from './accessibility';
import { props } from './testFixture';
import type { DisplayCell, GridDataSource } from './model';

function mockCanvas(): void {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    function (this: HTMLCanvasElement) {
      return new Proxy(
        { canvas: this },
        {
          get: (target, key) =>
            key === 'canvas' ? target.canvas : () => ({ width: 1 }),
        },
      ) as unknown as CanvasRenderingContext2D;
    },
  );
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn(() => 1),
  );
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
}
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
function key(grid: Element | null, init: KeyboardEventInit): void {
  grid?.dispatchEvent(new KeyboardEvent('keydown', init));
}

it('opens the editor on Enter and F2, and seeds it with a typed printable key', () => {
  const host = document.createElement('div'),
    input = props(),
    onEdit = vi.fn();
  input.onEdit = onEdit;
  const shadow = shadowGrid(host, input);
  const grid = host.querySelector('[role=grid]');
  key(grid, { key: 'F2' });
  expect(onEdit).toHaveBeenLastCalledWith({ row: 2, column: 2 });
  key(grid, { key: 'x' });
  expect(onEdit).toHaveBeenLastCalledWith({ row: 2, column: 2 }, 'x');
  onEdit.mockClear();
  key(grid, { key: 'c', metaKey: true });
  key(grid, { key: ' ' });
  expect(onEdit).not.toHaveBeenCalled();
  expect(input.selection.ranges[0]).toMatchObject({ r0: 2, r1: 2 });
  shadow.destroy();
});

it('opens value menus on stepper columns with Space or typing, and announces edit mode', () => {
  const host = document.createElement('div'),
    input = props(),
    onEdit = vi.fn();
  input.onEdit = onEdit;
  const column = input.source.columns[2];
  if (column) column.kind = 'boolean';
  const shadow = shadowGrid(host, input);
  const grid = host.querySelector('[role=grid]');
  key(grid, { key: ' ' });
  key(grid, { key: 't' });
  expect(onEdit.mock.calls).toEqual([
    [{ row: 2, column: 2 }],
    [{ row: 2, column: 2 }],
  ]);
  shadow.announce('Editing column_2');
  expect(host.querySelector('[role=status]')?.textContent).toBe(
    'Editing column_2',
  );
  shadow.destroy();
  expect(host.querySelector('[role=status]')).toBeNull();
});

/** A source whose row count grows like staged inserts. */
function growing(): GridDataSource & { add: () => void } {
  const listeners = new Set<() => void>();
  const cell: DisplayCell = { text: 'v', state: 'normal' };
  const source = {
    rows: 3,
    columns: [
      { id: 'a', title: 'a', width: 100, pinned: false },
      { id: 'b', title: 'b', width: 80, pinned: false, kind: 'enum' as const },
    ],
    metrics: props().source.metrics,
    cell: () => cell,
    window: () => Promise.resolve(),
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    add: () => {
      source.rows++;
      listeners.forEach((listener) => {
        listener();
      });
    },
  };
  return source;
}

it('grows with staged rows, reports cell geometry and selects cells for inline editing', () => {
  mockCanvas();
  const source = growing(),
    input = { ...props(), source, onSelection: vi.fn() },
    host = document.createElement('div');
  const handle = mountGrid(host, input);
  const spacer = host.firstElementChild?.firstElementChild as HTMLElement;
  const height = input.theme.rowHeight;
  expect(spacer.style.height).toBe(`${String(4 * height)}px`);
  source.add();
  expect(spacer.style.height).toBe(`${String(5 * height)}px`);
  expect(host.querySelector('[role=grid]')?.getAttribute('aria-rowcount')).toBe(
    '5',
  );
  expect(handle.cellRect?.(3, 1)).toEqual({
    x: 100,
    y: 4 * height,
    width: 80,
    height,
  });
  handle.select?.(3, 1);
  expect(input.onSelection).toHaveBeenLastCalledWith(
    expect.objectContaining({ row: 3, column: 1 }),
  );
  handle.destroy();
});

it('opens the value menu when the stepper glyph is clicked', () => {
  mockCanvas();
  const input = { ...props(), source: growing(), onEdit: vi.fn() },
    host = document.createElement('div');
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  const y = input.theme.rowHeight + 4;
  scroll.dispatchEvent(
    new MouseEvent('pointerdown', { button: 0, clientX: 120, clientY: y }),
  );
  expect(input.onEdit).not.toHaveBeenCalled();
  scroll.dispatchEvent(
    new MouseEvent('pointerdown', { button: 0, clientX: 172, clientY: y }),
  );
  expect(input.onEdit).toHaveBeenCalledExactlyOnceWith({ row: 0, column: 1 });
  handle.destroy();
});
