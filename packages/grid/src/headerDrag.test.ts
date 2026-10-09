// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { mountGrid } from './canvas';
import { props } from './testFixture';
import type { GridHandle, GridProps } from './model';

beforeEach(() => {
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
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function mount(): {
  input: GridProps;
  handle: GridHandle;
  host: HTMLElement;
  scroll: HTMLElement;
  onHeader: ReturnType<typeof vi.fn>;
  onColumnResize: ReturnType<typeof vi.fn>;
  onColumnMove: ReturnType<typeof vi.fn>;
} {
  const input = props();
  const onHeader = vi.fn(),
    onColumnResize = vi.fn(),
    onColumnMove = vi.fn();
  Object.assign(input, { onHeader, onColumnResize, onColumnMove });
  const host = document.createElement('div');
  const handle = mountGrid(host, input);
  return {
    input,
    handle,
    host,
    scroll: host.firstElementChild as HTMLElement,
    onHeader,
    onColumnResize,
    onColumnMove,
  };
}

function press(target: EventTarget, type: string, clientX: number): void {
  target.dispatchEvent(
    new MouseEvent(type, { button: 0, clientX, clientY: 10, bubbles: true }),
  );
}

it('resizes a column live from its header edge and reports the final width', () => {
  const { handle, scroll, onHeader, onColumnResize } = mount();
  press(scroll, 'pointerdown', 120);
  press(window, 'pointermove', 150);
  press(window, 'pointermove', 170);
  press(window, 'pointerup', 170);
  expect(onColumnResize).toHaveBeenNthCalledWith(1, 0, 150, false);
  expect(onColumnResize).toHaveBeenNthCalledWith(2, 0, 170, false);
  expect(onColumnResize).toHaveBeenLastCalledWith(0, 170, true);
  expect(onHeader).not.toHaveBeenCalled();
  handle.destroy();
});

it('moves a dragged header with a drop marker, and sorts on a plain click', () => {
  const { handle, host, scroll, onHeader, onColumnMove } = mount();
  press(scroll, 'pointerdown', 300);
  press(window, 'pointermove', 420);
  expect(host.querySelector('[data-drop-marker]')).not.toBeNull();
  press(window, 'pointerup', 490);
  expect(host.querySelector('[data-drop-marker]')).toBeNull();
  expect(onColumnMove).toHaveBeenCalledWith(2, 3);
  expect(onHeader).not.toHaveBeenCalled();
  press(scroll, 'pointerdown', 300);
  press(window, 'pointerup', 301);
  expect(onHeader).toHaveBeenCalledWith(2);
  handle.destroy();
});

it('auto-sizes a column on a header edge double-click and relayouts in place', () => {
  const { input, handle, scroll, onColumnResize } = mount();
  scroll.dispatchEvent(
    new MouseEvent('dblclick', { clientX: 240, clientY: 10, bubbles: true }),
  );
  expect(onColumnResize).toHaveBeenCalledWith(1, 40, true);
  const column = input.source.columns[1];
  if (column) column.width = 200;
  handle.relayout?.();
  expect(handle.cellRect?.(0, 2)).toMatchObject({ x: 320, width: 120 });
  expect(handle.fitWidth?.(3)).toBe(40);
  handle.destroy();
});

it('ignores a cancelled header press', () => {
  const { handle, scroll, onHeader, onColumnMove } = mount();
  press(scroll, 'pointerdown', 300);
  press(window, 'pointermove', 420);
  press(window, 'pointercancel', 420);
  press(scroll, 'pointerdown', 300);
  press(window, 'pointercancel', 300);
  expect(onHeader).not.toHaveBeenCalled();
  expect(onColumnMove).not.toHaveBeenCalled();
  expect(scroll.parentElement?.querySelector('[data-drop-marker]')).toBeNull();
  handle.destroy();
});
