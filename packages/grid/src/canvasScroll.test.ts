// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import userEvent from '@testing-library/user-event';
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
it('moves the selection with arrow keys straight after clicking a cell', async () => {
  mockCanvas();
  const user = userEvent.setup();
  const input = props();
  const host = document.createElement('div');
  document.body.append(host);
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  await user.pointer({
    keys: '[MouseLeft]',
    target: scroll,
    coords: { clientX: 300, clientY: 39 },
  });
  expect(input.selection).toMatchObject({ row: 0, column: 2 });
  expect(document.activeElement).toBe(host.querySelector('[role="grid"]'));
  await user.keyboard('{ArrowDown}{ArrowRight}');
  expect(input.selection).toMatchObject({ row: 1, column: 3 });
  handle.destroy();
  host.remove();
});
it('requests the final row when a five-million-row native scrollbar reaches its end', () => {
  mockCanvas();
  const input = props();
  Object.defineProperty(input.source, 'rows', { value: 5_000_000 });
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(800);
  const request = vi.spyOn(input.source, 'window');
  const host = document.createElement('div');
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  expect((scroll.firstElementChild as HTMLElement).style.height).toBe(
    '32000000px',
  );
  scroll.scrollTop = 32_000_000 - 800;
  scroll.dispatchEvent(new Event('scroll'));
  expect(handle.viewport.y).toBe((5_000_000 + 1) * input.theme.rowHeight - 800);
  expect(request).toHaveBeenLastCalledWith(
    expect.objectContaining({ r1: 4_999_999 }),
    1,
  );
  handle.select?.(4_999_999, 2);
  expect(scroll.scrollTop).toBeCloseTo(32_000_000 - 800);
  expect(handle.cellRect?.(4_999_999, 2)?.y).toBeCloseTo(
    800 - input.theme.rowHeight,
  );
  handle.destroy();
});
it('remaps a capped scroll offset when the viewport height changes', () => {
  mockCanvas();
  let resized: ResizeObserverCallback | undefined;
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(callback: ResizeObserverCallback) {
        resized = callback;
      }
      observe(): void {}
      disconnect(): void {}
    },
  );
  const height = vi
    .spyOn(HTMLElement.prototype, 'clientHeight', 'get')
    .mockReturnValue(800);
  const input = props();
  Object.defineProperty(input.source, 'rows', { value: 5_000_000 });
  const host = document.createElement('div');
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  scroll.scrollTop = 16_000_000;
  scroll.dispatchEvent(new Event('scroll'));
  height.mockReturnValue(400);
  resized?.(
    [{ contentRect: { width: 600, height: 400 } } as ResizeObserverEntry],
    {} as ResizeObserver,
  );
  expect(handle.viewport.y).toBeCloseTo(
    (16_000_000 * (130_000_026 - 400)) / (32_000_000 - 400),
  );
  handle.destroy();
});
