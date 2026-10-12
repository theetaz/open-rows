// SPDX-License-Identifier: Apache-2.0
import { drawEdges, foreignColumns } from './canvasEdges';
import { HEADER_HEIGHT, MAX_COLUMNS, ROW_HEIGHT } from './geometry';
import type { DiagramLayout, DiagramNode } from './geometry';
import type { View } from './viewport';

type Paint = {
  page: string;
  box: string;
  head: string;
  text: string;
  line: string;
  accent: string;
};

function table(
  context: CanvasRenderingContext2D,
  node: DiagramNode,
  drawing: { paint: Paint; foreign: ReadonlySet<string> | undefined },
): void {
  const { paint, foreign } = drawing;
  const { x, y, width, height } = node;
  context.fillStyle = paint.box;
  context.fillRect(x, y, width, height);
  context.fillStyle = paint.head;
  context.fillRect(x, y, width, HEADER_HEIGHT);
  context.strokeRect(x, y, width, height);
  context.fillStyle = paint.text;
  context.font = 'bold 12px system-ui';
  context.fillText(node.table.name, x + 10, y + 20, width - 20);
  context.font = '12px system-ui';
  node.table.columns.slice(0, MAX_COLUMNS).forEach((column, index) => {
    const label = `${column.primaryKey ? 'PK ' : foreign?.has(column.name) ? 'FK ' : ''}${column.name}  ${column.dataType}`;
    context.fillText(
      label,
      x + 10,
      y + HEADER_HEIGHT + (index + 0.7) * ROW_HEIGHT,
      width - 20,
    );
  });
  if (node.hidden)
    context.fillText(`+${String(node.hidden)} more`, x + 10, y + height - 6);
}

function colors(canvas: HTMLCanvasElement): Paint {
  const css = getComputedStyle(canvas);
  const color = (token: string): string => css.getPropertyValue(token).trim();
  return {
    page: color('--or-page'),
    box: color('--or-window'),
    head: color('--or-control-fill'),
    text: color('--or-text'),
    line: color('--or-control-stroke'),
    accent: color('--or-accent'),
  };
}

/** Draw only visible table boxes; relationships use the worker's routed points. */
export function drawDiagram(
  canvas: HTMLCanvasElement,
  layout: DiagramLayout,
  view: View,
  matches: ReadonlySet<string>,
): void {
  const context = canvas.getContext('2d');
  if (!context) return;
  const rect = canvas.getBoundingClientRect();
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.round(rect.width * ratio));
  canvas.height = Math.max(1, Math.round(rect.height * ratio));
  const paint = colors(canvas);
  context.scale(ratio, ratio);
  context.fillStyle = paint.page;
  context.fillRect(0, 0, rect.width, rect.height);
  context.translate(view.x, view.y);
  context.scale(view.scale, view.scale);
  context.strokeStyle = paint.line;
  context.lineWidth = 1 / view.scale;
  drawEdges(context, layout);
  const foreign = foreignColumns(layout);
  for (const node of layout.nodes) {
    if (!visible(node, view, rect)) continue;
    context.globalAlpha =
      matches.size && !matches.has(node.table.name) ? 0.35 : 1;
    context.strokeStyle = matches.has(node.table.name)
      ? paint.accent
      : paint.line;
    table(context, node, { paint, foreign: foreign.get(node.table.name) });
  }
  context.globalAlpha = 1;
}

function visible(
  node: DiagramNode,
  view: View,
  size: { width: number; height: number },
): boolean {
  const left = node.x * view.scale + view.x;
  const top = node.y * view.scale + view.y;
  return (
    left < size.width &&
    top < size.height &&
    left + node.width * view.scale > 0 &&
    top + node.height * view.scale > 0
  );
}

/** Table under a pointer, in diagram coordinates; empty space pans. */
export function tableAt(
  layout: DiagramLayout,
  point: { x: number; y: number },
): string | undefined {
  return layout.nodes.find(
    (node) =>
      point.x >= node.x &&
      point.x <= node.x + node.width &&
      point.y >= node.y &&
      point.y <= node.y + node.height,
  )?.table.name;
}
