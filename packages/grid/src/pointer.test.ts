// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { mountGrid } from './canvas';
import { props } from './testFixture';

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

function contextMenu(scroll: HTMLElement, x: number, y: number): void {
  scroll.dispatchEvent(
    new MouseEvent('contextmenu', { clientX: x, clientY: y, cancelable: true }),
  );
}

it('keeps a multi-cell selection when right-clicking inside it, and replaces it outside', () => {
  mockCanvas();
  const input = props();
  const onContextMenu = vi.fn();
  input.onContextMenu = onContextMenu;
  const host = document.createElement('div');
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  // The fixture pins columns 0 and 1 (120 px each), which jsdom's 0 px viewport still hits.
  const ranges = [{ r0: 0, r1: 2, c0: 0, c1: 0 }];
  input.selection = { row: 2, column: 0, ranges };
  contextMenu(scroll, 60, 39);
  expect(input.selection.ranges).toEqual(ranges);
  expect(input.selection).toMatchObject({ row: 0, column: 0 });
  expect(onContextMenu).toHaveBeenLastCalledWith(
    expect.objectContaining({ row: 0, column: 0, kind: 'cell' }),
  );
  // Column 1 lies outside: the selection collapses to the clicked cell.
  contextMenu(scroll, 180, 39);
  expect(input.selection.ranges).toEqual([{ r0: 0, r1: 0, c0: 1, c1: 1 }]);
  expect(onContextMenu).toHaveBeenLastCalledWith(
    expect.objectContaining({ row: 0, column: 1, kind: 'cell' }),
  );
  handle.destroy();
});
