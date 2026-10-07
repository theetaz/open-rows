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
it('writes native offsets once and waits for scroll delivery before reading, without resizing on scroll', () => {
  mockCanvas();
  const top = vi
    .spyOn(HTMLElement.prototype, 'scrollTop', 'get')
    .mockReturnValue(0);
  const left = vi
    .spyOn(HTMLElement.prototype, 'scrollLeft', 'get')
    .mockReturnValue(0);
  const host = document.createElement('div');
  const handle = mountGrid(host, props());
  const scroller = host.firstElementChild as HTMLElement;
  const write = vi.fn();
  scroller.scrollTo = write;
  top.mockClear();
  left.mockClear();
  const resizes = handle.metrics.canvasResizeEvents;
  handle.scrollTo(240, 13_000_000);
  expect(write).toHaveBeenCalledExactlyOnceWith(240, 13_000_000);
  expect(top).not.toHaveBeenCalled();
  expect(left).not.toHaveBeenCalled();
  top.mockReturnValue(13_000_000);
  left.mockReturnValue(240);
  scroller.dispatchEvent(new Event('scroll'));
  expect(top).toHaveBeenCalledTimes(1);
  expect(left).toHaveBeenCalledTimes(1);
  expect(handle.viewport).toMatchObject({ x: 240, y: 13_000_000 });
  expect(handle.metrics.canvasResizeEvents).toBe(resizes);
  handle.destroy();
});
it('right-clicks target rows and columns without sorting, and ignores blank space', () => {
  mockCanvas();
  const input = props();
  input.rowHeaderColumn = 0;
  const onContextMenu = vi.fn();
  const onHeader = vi.fn();
  input.onContextMenu = onContextMenu;
  input.onHeader = onHeader;
  const host = document.createElement('div');
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  scroll.dispatchEvent(
    new MouseEvent('pointerdown', { button: 2, clientX: 300, clientY: 10 }),
  );
  scroll.dispatchEvent(
    new MouseEvent('contextmenu', {
      clientX: 300,
      clientY: 10,
      cancelable: true,
    }),
  );
  expect(onHeader).not.toHaveBeenCalled();
  expect(onContextMenu).toHaveBeenLastCalledWith({
    row: 0,
    column: 2,
    kind: 'column',
    x: 300,
    y: 10,
  });
  scroll.dispatchEvent(
    new MouseEvent('contextmenu', {
      clientX: 10,
      clientY: 39,
      cancelable: true,
    }),
  );
  expect(onContextMenu).toHaveBeenLastCalledWith({
    row: 0,
    column: 1,
    kind: 'row',
    x: 10,
    y: 39,
  });
  expect(input.selection.ranges[0]).toEqual({ r0: 0, r1: 0, c0: 1, c1: 99 });
  scroll.dispatchEvent(
    new MouseEvent('contextmenu', {
      clientX: 15_000,
      clientY: 39,
      cancelable: true,
    }),
  );
  expect(onContextMenu).toHaveBeenCalledTimes(2);
  handle.destroy();
});
it('updates colors and fonts in place without losing selection, scroll or canvas layers', () => {
  mockCanvas();
  const input = props();
  const host = document.createElement('div');
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  scroll.scrollTop = 260;
  scroll.dispatchEvent(new Event('scroll'));
  const selection = { row: 12, column: 2, ranges: [] };
  input.selection = selection;
  const layers = Array.from(host.querySelectorAll('canvas'));
  const next = {
    ...input.theme,
    fontSize: 17,
    fontFamily: 'Menlo, monospace',
    rowHeight: 29,
    colours: { ...input.theme.colours, accent: '#7842C1' },
  };
  expect(typeof handle.setTheme).toBe('function');
  handle.setTheme?.(next);
  expect(input.theme).toEqual(next);
  expect(input.selection).toBe(selection);
  expect(handle.viewport.y).toBe(260);
  expect(Array.from(host.querySelectorAll('canvas'))).toEqual(layers);
  expect((scroll.firstElementChild as HTMLElement).style.height).toBe(
    `${String((input.source.rows + 1) * 29)}px`,
  );
  handle.destroy();
});
