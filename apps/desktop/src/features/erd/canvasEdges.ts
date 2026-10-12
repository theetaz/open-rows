// SPDX-License-Identifier: Apache-2.0
import type { DiagramEdge, DiagramLayout, Point } from './geometry';

function endpoint(
  context: CanvasRenderingContext2D,
  points: readonly Point[],
  many: boolean,
): void {
  const at = many ? points.at(-1) : points[0];
  const from = many ? points.at(-2) : points[1];
  if (!at || !from) return;
  context.save();
  context.translate(at.x, at.y);
  context.rotate(Math.atan2(at.y - from.y, at.x - from.x));
  context.beginPath();
  if (many) {
    context.moveTo(-12, 0);
    context.lineTo(0, -6);
    context.moveTo(-12, 0);
    context.lineTo(0, 6);
    context.moveTo(-12, 0);
    context.lineTo(0, 0);
  } else {
    context.moveTo(-5, -6);
    context.lineTo(-5, 6);
  }
  context.stroke();
  context.restore();
}

function draw(context: CanvasRenderingContext2D, edge: DiagramEdge): void {
  context.beginPath();
  edge.points.forEach((point, index) => {
    if (index) context.lineTo(point.x, point.y);
    else context.moveTo(point.x, point.y);
  });
  context.stroke();
  endpoint(context, edge.points, false);
  endpoint(context, edge.points, true);
}

/** Draw routed relationships with the same one/many marks as SVG. */
export function drawEdges(
  context: CanvasRenderingContext2D,
  layout: DiagramLayout,
): void {
  for (const edge of layout.edges) draw(context, edge);
}

/** FK column marks shared by each visible canvas table. */
export function foreignColumns(
  layout: DiagramLayout,
): ReadonlyMap<string, ReadonlySet<string>> {
  const foreign = new Map<string, Set<string>>();
  for (const { key } of layout.edges) {
    const columns = foreign.get(key.table) ?? new Set<string>();
    key.columns.forEach((column) => {
      columns.add(column);
    });
    foreign.set(key.table, columns);
  }
  return foreign;
}
