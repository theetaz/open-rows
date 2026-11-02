// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { mountGrid } from './canvas';
import { props } from './testFixture';
import type { GridHandle, GridProps } from './model';

const VIEWPORT = 800;
const SPACER = 32_000_000;

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

interface Mounted {
  input: GridProps;
  handle: GridHandle;
  host: HTMLElement;
  scroll: HTMLElement;
}
function mount(rows: number): Mounted {
  mockCanvas();
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(
    VIEWPORT,
  );
  const input = props();
  Object.defineProperty(input.source, 'rows', { value: rows });
  const host = document.createElement('div');
  document.body.append(host);
  const handle = mountGrid(host, input);
  const scroll = host.firstElementChild as HTMLElement;
  return { input, handle, host, scroll };
}
function unmount({ handle, host }: Mounted): void {
  handle.destroy();
  host.remove();
}
/** No scroll event follows: sub-pixel native moves are not reported by WebKit. */
function wheel(scroll: HTMLElement, deltaY: number): WheelEvent {
  const event = new WheelEvent('wheel', { deltaY, cancelable: true });
  scroll.dispatchEvent(event);
  return event;
}

it('requests the final row when a five-million-row native scrollbar reaches its end', () => {
  const grid = mount(5_000_000);
  const { input, handle, scroll } = grid;
  const request = vi.spyOn(input.source, 'window');
  expect((scroll.firstElementChild as HTMLElement).style.height).toBe(
    `${String(SPACER)}px`,
  );
  scroll.scrollTop = SPACER - VIEWPORT;
  scroll.dispatchEvent(new Event('scroll'));
  expect(handle.viewport.y).toBe((5_000_000 + 1) * input.theme.rowHeight - 800);
  expect(request).toHaveBeenLastCalledWith(
    expect.objectContaining({ r1: 4_999_999 }),
    1,
  );
  handle.select?.(4_999_999, 2);
  expect(scroll.scrollTop).toBeCloseTo(SPACER - VIEWPORT);
  expect(handle.cellRect?.(4_999_999, 2)?.y).toBeCloseTo(
    VIEWPORT - input.theme.rowHeight,
  );
  unmount(grid);
});
it('moves capped grids by exact logical pixels for each wheel step', () => {
  const grid = mount(10_000_000);
  const { handle, scroll } = grid;
  scroll.scrollTop = SPACER / 2;
  scroll.dispatchEvent(new Event('scroll'));
  const start = handle.viewport.y;
  expect(wheel(scroll, 3).defaultPrevented).toBe(true);
  expect(handle.viewport.y).toBeCloseTo(start + 3, 6);
  wheel(scroll, 40);
  wheel(scroll, -10);
  expect(handle.viewport.y).toBeCloseTo(start + 33, 6);
  wheel(scroll, -Number.MAX_SAFE_INTEGER);
  expect(handle.viewport.y).toBe(0);
  expect(scroll.scrollTop).toBe(0);
  unmount(grid);
});
it('leaves wheel input of uncapped grids to native scrolling', () => {
  const grid = mount(1_000);
  const { handle, scroll } = grid;
  expect(wheel(scroll, 40).defaultPrevented).toBe(false);
  scroll.scrollTop = 520;
  scroll.dispatchEvent(new Event('scroll'));
  expect(handle.viewport.y).toBe(520);
  unmount(grid);
});
it('jumps to the last and first rows with Cmd+arrows and hit-tests clicked cells', () => {
  const grid = mount(10_000_000);
  const { input, handle, host, scroll } = grid;
  const height = input.theme.rowHeight;
  handle.select?.(5_000_000, 2);
  scroll.dispatchEvent(new Event('scroll'));
  const focus = host.querySelector<HTMLElement>('[role="grid"]');
  focus?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowDown', metaKey: true }),
  );
  scroll.dispatchEvent(new Event('scroll'));
  expect(input.selection.row).toBe(9_999_999);
  expect(handle.cellRect?.(9_999_999, 2)?.y).toBeCloseTo(VIEWPORT - height);
  scroll.dispatchEvent(
    new MouseEvent('pointerdown', { clientX: 300, clientY: VIEWPORT - 1 }),
  );
  expect(input.selection.row).toBe(9_999_999);
  focus?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowUp', ctrlKey: true }),
  );
  scroll.dispatchEvent(new Event('scroll'));
  expect(input.selection.row).toBe(0);
  expect(handle.viewport.y).toBe(0);
  unmount(grid);
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
    .mockReturnValue(VIEWPORT);
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
    (16_000_000 * (130_000_026 - 400)) / (SPACER - 400),
  );
  handle.destroy();
});
