// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { drawDiagram, tableAt } from './canvasDrawing';
import type { DiagramLayout } from './geometry';

const layout: DiagramLayout = {
  width: 2000,
  height: 100,
  nodes: [
    {
      table: {
        name: 'visible',
        columns: [
          { name: 'id', dataType: 'int', nullable: false, primaryKey: true },
        ],
      },
      x: 10,
      y: 10,
      width: 160,
      height: 50,
      hidden: 0,
    },
    {
      table: { name: 'offscreen', columns: [] },
      x: 1000,
      y: 10,
      width: 160,
      height: 50,
      hidden: 0,
    },
  ],
  edges: [],
};
afterEach(() => {
  vi.restoreAllMocks();
});
it('draws visible columns and culls offscreen table boxes', () => {
  const context = {
    scale: vi.fn(),
    translate: vi.fn(),
    fillRect: vi.fn(),
    strokeRect: vi.fn(),
    fillText: vi.fn(),
  };
  const canvas = document.createElement('canvas');
  vi.spyOn(canvas, 'getContext').mockReturnValue(
    context as unknown as CanvasRenderingContext2D,
  );
  vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
    width: 500,
    height: 400,
  } as DOMRect);
  drawDiagram(canvas, layout, { x: 0, y: 0, scale: 1 }, new Set());
  expect(context.strokeRect).toHaveBeenCalledTimes(1);
  expect(context.fillText).toHaveBeenCalledWith('visible', 20, 30, 140);
  expect(context.fillText).toHaveBeenCalledWith('PK id  int', 20, 54, 140);
});
it('maps table hit targets using diagram coordinates', () => {
  expect(tableAt(layout, { x: 20, y: 20 })).toBe('visible');
  expect(tableAt(layout, { x: 500, y: 20 })).toBeUndefined();
});
