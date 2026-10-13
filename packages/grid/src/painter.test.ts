// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { CanvasPainter } from './painter';
import { props } from './testFixture';
import { shadowGrid } from './accessibility';

function canvasMock(): {
  fillText: ReturnType<typeof vi.fn>;
  measureText: ReturnType<typeof vi.fn>;
  strokeRect: ReturnType<typeof vi.fn>;
} {
  const fillText = vi.fn(),
    measureText = vi.fn((text: string) => ({ width: text.length * 7 })),
    strokeRect = vi.fn();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    function (this: HTMLCanvasElement) {
      return {
        canvas: this,
        fillText,
        measureText,
        strokeRect,
        clearRect: vi.fn(),
        setTransform: vi.fn(),
        save: vi.fn(),
        restore: vi.fn(),
        beginPath: vi.fn(),
        rect: vi.fn(),
        clip: vi.fn(),
        fillRect: vi.fn(),
        fill: vi.fn(),
        arc: vi.fn(),
      } as unknown as CanvasRenderingContext2D;
    },
  );
  return { fillText, measureText, strokeRect };
}
afterEach(() => vi.restoreAllMocks());
it.each(['light', 'dark'] as const)(
  'paints synthetic tiles in %s tokens with four layers, DPR and selection',
  async (mode) => {
    const context = canvasMock(),
      host = document.createElement('div'),
      model = props(mode);
    await model.source.window({ r0: 0, r1: 30, c0: 0, c1: 19 }, 1);
    vi.stubGlobal('devicePixelRatio', 2);
    const painter = new CanvasPainter(host, model);
    painter.resize(1040, 600);
    painter.prepare();
    context.measureText.mockClear();
    painter.paint();
    painter.paint();
    expect(host.querySelectorAll('canvas')).toHaveLength(4);
    expect(host.querySelector('canvas')?.width).toBe(2080);
    expect(context.fillText.mock.calls.some(([text]) => text === 'NULL')).toBe(
      true,
    );
    expect(context.strokeRect).toHaveBeenCalled();
    expect(context.measureText).not.toHaveBeenCalled();
    expect(painter.metrics.placeholders).toBe(0);
    expect(painter.metrics.paints).toBe(2);
    vi.unstubAllGlobals();
  },
);
it('records placeholders and exposes focused indices without materializing rows', () => {
  canvasMock();
  const host = document.createElement('div'),
    model = props();
  const painter = new CanvasPainter(host, model);
  painter.resize(1040, 600);
  painter.paint();
  expect(painter.metrics.placeholders).toBeGreaterThan(0);
  const shadow = shadowGrid(host, model),
    grid = host.querySelector<HTMLElement>('[role="grid"]');
  expect(grid?.getAttribute('aria-rowcount')).toBe('1000001');
  expect(
    host.querySelector('[role="row"]')?.getAttribute('aria-rowindex'),
  ).toBe('4');
  expect(
    host.querySelector('[role="gridcell"]')?.getAttribute('aria-colindex'),
  ).toBe('3');
  grid?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown' }));
  expect(
    host.querySelector('[role="row"]')?.getAttribute('aria-rowindex'),
  ).toBe('5');
  shadow.destroy();
  expect(host.querySelector('[role="grid"]')).toBeNull();
});

it('does not reassign backing dimensions at an unchanged size, and resizes only on geometry or DPR change', () => {
  canvasMock();
  vi.stubGlobal('devicePixelRatio', 1);
  const painter = new CanvasPainter(document.createElement('div'), props());
  painter.resize(1040, 600);
  expect(painter.metrics.canvasResizeEvents).toBe(8);
  painter.resize(1040, 600);
  painter.view.y = 13_000_000;
  painter.paint();
  expect(painter.metrics.canvasResizeEvents).toBe(8);
  vi.stubGlobal('devicePixelRatio', 2);
  painter.resize(1040, 600);
  expect(painter.metrics.canvasResizeEvents).toBe(16);
  painter.resize(1040, 700);
  expect(painter.metrics.canvasResizeEvents).toBe(20);
  vi.unstubAllGlobals();
});
