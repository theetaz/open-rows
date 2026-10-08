// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { CanvasPainter } from './painter';
import { props } from './testFixture';

/** Per-layer fill colours of every rectangle draw. */
function layerMock(): Map<string, string[]> {
  const fills = new Map<string, string[]>();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    function (this: HTMLCanvasElement) {
      const layer = this.dataset.layer ?? '';
      fills.set(layer, []);
      const noop = vi.fn();
      return {
        canvas: this,
        fillStyle: '',
        fillRect(this: { fillStyle: string }) {
          fills.get(layer)?.push(this.fillStyle);
        },
        measureText: (text: string) => ({ width: text.length * 7 }),
        ...Object.fromEntries(
          [
            'fillText',
            'strokeRect',
            'clearRect',
            'setTransform',
            'save',
            'restore',
            'beginPath',
            'rect',
            'clip',
            'fill',
          ].map((name) => [name, noop]),
        ),
      } as unknown as CanvasRenderingContext2D;
    },
  );
  return fills;
}
afterEach(() => vi.restoreAllMocks());

it('fills a selected row edge to edge with the accent, but only the cells of a column selection', async () => {
  const fills = layerMock(),
    model = props();
  await model.source.window({ r0: 0, r1: 30, c0: 0, c1: 19 }, 1);
  const accent = model.theme.colours.accent;
  model.selection = {
    row: 2,
    column: 3,
    ranges: [{ r0: 2, r1: 2, c0: 3, c1: 3 }],
  };
  const painter = new CanvasPainter(document.createElement('div'), model);
  painter.resize(1040, 600);
  painter.paint();
  const visibleColumns =
    painter.layout.pinned + painter.range.c1 - painter.range.c0 + 1;
  expect(
    fills.get('selection')?.filter((fill) => fill === accent),
  ).toHaveLength(visibleColumns);
  expect(fills.get('body')).not.toContain(accent);
  fills.set('selection', []);
  model.selection = {
    row: 0,
    column: 3,
    ranges: [{ r0: 0, r1: 999_999, c0: 3, c1: 3 }],
  };
  painter.paint();
  expect(
    fills.get('selection')?.filter((fill) => fill === accent),
  ).toHaveLength(painter.range.r1 - painter.range.r0 + 1);
});

it('paints no selection before the user selects anything', () => {
  const fills = layerMock(),
    model = props();
  model.selection = { row: 0, column: 0, ranges: [] };
  const painter = new CanvasPainter(document.createElement('div'), model);
  painter.resize(1040, 600);
  painter.paint();
  expect(fills.get('selection')).toEqual([]);
});
