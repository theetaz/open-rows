// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  HEADER_HEIGHT,
  MAX_COLUMNS,
  ROW_HEIGHT,
  edgePath,
  layoutDiagram,
  matchingTables,
  tableSize,
} from './layout';
import { shop, table } from './testDiagram';
import { centreOn, fitView, zoomAt } from './viewport';

const at = (name: string, layout = layoutDiagram(shop)) => {
  const node = layout.nodes.find((item) => item.table.name === name);
  if (!node) throw new Error(`${name} was not placed`);
  return node;
};

it('places parents left of children and routes every foreign key', () => {
  const layout = layoutDiagram(shop);
  expect(layout.nodes).toHaveLength(5);
  expect(at('customers', layout).x).toBeLessThan(at('orders', layout).x);
  expect(at('orders', layout).x).toBeLessThan(at('lines', layout).x);
  expect(layout.edges.map((edge) => edge.id)).toEqual([
    'lines\nlines_order_fk',
    'orders\norders_customer_fk',
  ]);
  for (const edge of layout.edges)
    expect(edge.points.length).toBeGreaterThanOrEqual(2);
  for (const node of layout.nodes) {
    expect(node.x + node.width).toBeLessThanOrEqual(layout.width);
    expect(node.y + node.height).toBeLessThanOrEqual(layout.height);
  }
});

it('packs unrelated tables in a grid below the related ones', () => {
  const layout = layoutDiagram(shop);
  const related = Math.max(
    ...['customers', 'orders', 'lines'].map((name) => {
      const node = at(name, layout);
      return node.y + node.height;
    }),
  );
  expect(at('audit', layout).y).toBeGreaterThan(related);
  expect(at('settings', layout).y).toBe(at('audit', layout).y);
  const many = layoutDiagram({
    ...shop,
    foreignKeys: [],
    tables: Array.from({ length: 100 }, (_, index) =>
      table(`t${String(index)}`),
    ),
  });
  expect(many.width).toBeLessThan(many.height * 3);
  expect(many.height).toBeLessThan(many.width * 3);
});

it('sizes boxes from their text and folds very wide tables', () => {
  const small = tableSize(table('t'));
  expect(small).toEqual({
    width: 160,
    height: HEADER_HEIGHT + ROW_HEIGHT,
    hidden: 0,
  });
  const columns = Array.from({ length: 50 }, (_, index) => `c${String(index)}`);
  const wide = tableSize(table('wide', ...columns));
  expect(wide.hidden).toBe(51 - MAX_COLUMNS);
  expect(wide.height).toBe(HEADER_HEIGHT + (MAX_COLUMNS + 1) * ROW_HEIGHT);
  expect(tableSize(table('x'.repeat(200))).width).toBe(360);
});

it('finds tables by substring and draws routed paths', () => {
  const layout = layoutDiagram(shop);
  expect([...matchingTables(layout.nodes, 'OR')]).toEqual(['orders']);
  expect(matchingTables(layout.nodes, '  ').size).toBe(0);
  expect(
    edgePath([
      { x: 0, y: 1 },
      { x: 2, y: 3 },
    ]),
  ).toBe('M0 1 L2 3');
});

it('fits, zooms about a point and centres', () => {
  const fitted = fitView(
    { width: 2000, height: 500 },
    { width: 1032, height: 532 },
  );
  expect(fitted.scale).toBeCloseTo(0.5);
  expect(fitted.x).toBeCloseTo(16);
  expect(
    fitView({ width: 10, height: 10 }, { width: 100, height: 100 }).scale,
  ).toBe(1);
  const zoomed = zoomAt({ x: 0, y: 0, scale: 1 }, 2, { x: 100, y: 50 });
  expect(zoomed).toEqual({ x: -100, y: -50, scale: 2 });
  expect(zoomAt(zoomed, 1000, { x: 0, y: 0 }).scale).toBe(3);
  expect(
    centreOn(
      { x: 0, y: 0, scale: 2 },
      { x: 10, y: 10 },
      { width: 100, height: 60 },
    ),
  ).toEqual({ x: 30, y: 10, scale: 2 });
});
