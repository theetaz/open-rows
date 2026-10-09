// SPDX-License-Identifier: Apache-2.0
import { graphlib, layout } from '@dagrejs/dagre';
import type { EdgeLabel, GraphLabel, NodeLabel } from '@dagrejs/dagre';
import type {
  DiagramForeignKeyDto,
  DiagramTableDto,
  SchemaDiagramDto,
} from '@openrows/ipc';

/** Geometry of a table box, in diagram units (CSS pixels at 100 %). */
export const HEADER_HEIGHT = 30;
export const ROW_HEIGHT = 20;
export const PADDING_X = 10;
/** Columns drawn per table; the rest collapse into a "+N more" row. */
export const MAX_COLUMNS = 40;
const CHAR_WIDTH = 6.6;
const MIN_WIDTH = 160;
const MAX_WIDTH = 360;
const GAP = 40;
const MARGIN = 24;

export type Point = { x: number; y: number };

/** A placed table; `x`/`y` is its top-left corner. */
export type DiagramNode = {
  table: DiagramTableDto;
  x: number;
  y: number;
  width: number;
  height: number;
  /** Columns not drawn (beyond {@link MAX_COLUMNS}). */
  hidden: number;
};

/** A routed foreign key from the referenced (parent) to the referencing table. */
export type DiagramEdge = {
  id: string;
  key: DiagramForeignKeyDto;
  points: Point[];
};

export type DiagramLayout = {
  nodes: DiagramNode[];
  edges: DiagramEdge[];
  width: number;
  height: number;
};

/** Box size from text lengths: names left, types right. */
export function tableSize(table: DiagramTableDto): {
  width: number;
  height: number;
  hidden: number;
} {
  const shown = table.columns.slice(0, MAX_COLUMNS);
  const hidden = table.columns.length - shown.length;
  const longest = Math.max(
    table.name.length + 4,
    ...shown.map((column) => column.name.length + column.dataType.length + 6),
  );
  const width = Math.min(
    MAX_WIDTH,
    Math.max(MIN_WIDTH, Math.ceil(longest * CHAR_WIDTH) + 2 * PADDING_X),
  );
  const rows = shown.length + (hidden > 0 ? 1 : 0);
  return {
    width,
    height: HEADER_HEIGHT + Math.max(rows, 1) * ROW_HEIGHT,
    hidden,
  };
}

const edgeId = (key: DiagramForeignKeyDto): string =>
  `${key.table}\n${key.name}`;

/** Lay out the related tables with dagre, left (parents) to right. */
function layoutRelated(
  tables: DiagramTableDto[],
  keys: DiagramForeignKeyDto[],
): DiagramLayout {
  const graph = new graphlib.Graph<GraphLabel, NodeLabel, EdgeLabel>({
    multigraph: true,
  });
  graph.setGraph({
    rankdir: 'LR',
    nodesep: GAP / 2,
    ranksep: GAP * 2,
    marginx: MARGIN,
    marginy: MARGIN,
  });
  graph.setDefaultEdgeLabel(() => ({}));
  for (const table of tables) graph.setNode(table.name, tableSize(table));
  for (const key of keys)
    graph.setEdge(key.targetTable, key.table, {}, edgeId(key));
  layout(graph);
  const nodes = tables.map((table) => {
    const placed = graph.node(table.name);
    const size = tableSize(table);
    return {
      table,
      ...size,
      x: (placed.x ?? size.width / 2) - size.width / 2,
      y: (placed.y ?? size.height / 2) - size.height / 2,
    };
  });
  const edges = keys.map((key) => ({
    id: edgeId(key),
    key,
    points: graph.edge(key.targetTable, key.table, edgeId(key)).points ?? [],
  }));
  const { width = 0, height = 0 } = graph.graph();
  return { nodes, edges, width, height };
}

/** Shelf-pack unrelated tables in rows below `top`, wrapping at `maxWidth`. */
function packUnrelated(
  tables: DiagramTableDto[],
  top: number,
  maxWidth: number,
): DiagramNode[] {
  const nodes: DiagramNode[] = [];
  let x = MARGIN;
  let y = top;
  let rowHeight = 0;
  for (const table of tables) {
    const size = tableSize(table);
    if (x > MARGIN && x + size.width > maxWidth) {
      x = MARGIN;
      y += rowHeight + GAP;
      rowHeight = 0;
    }
    nodes.push({ table, ...size, x, y });
    x += size.width + GAP;
    rowHeight = Math.max(rowHeight, size.height);
  }
  return nodes;
}

/**
 * Place every table: related tables (any foreign key, self-references
 * included) by dagre, then unrelated tables in a grid underneath, so a
 * large schema without keys does not become one endless column.
 */
export function layoutDiagram(diagram: SchemaDiagramDto): DiagramLayout {
  const related = new Set(
    diagram.foreignKeys.flatMap((key) => [key.table, key.targetTable]),
  );
  const linked = diagram.tables.filter((table) => related.has(table.name));
  const alone = diagram.tables.filter((table) => !related.has(table.name));
  const graph = linked.length
    ? layoutRelated(linked, diagram.foreignKeys)
    : { nodes: [], edges: [], width: 0, height: 0 };
  const area = alone.reduce((sum, table) => {
    const size = tableSize(table);
    return sum + (size.width + GAP) * (size.height + GAP);
  }, 0);
  const maxWidth = Math.max(graph.width, Math.sqrt(area) * 1.6, 600);
  const top = graph.height ? graph.height + GAP : MARGIN;
  const packed = packUnrelated(alone, top, maxWidth);
  const nodes = [...graph.nodes, ...packed];
  const width = Math.max(
    graph.width,
    ...nodes.map((node) => node.x + node.width + MARGIN),
  );
  const height = Math.max(
    graph.height,
    ...nodes.map((node) => node.y + node.height + MARGIN),
  );
  return { nodes, edges: graph.edges, width, height };
}

/** Tables whose name contains `query` (case-insensitive); none for a blank query. */
export function matchingTables(
  nodes: readonly DiagramNode[],
  query: string,
): Set<string> {
  const needle = query.trim().toLowerCase();
  if (!needle) return new Set();
  return new Set(
    nodes
      .filter((node) => node.table.name.toLowerCase().includes(needle))
      .map((node) => node.table.name),
  );
}

/** SVG path data through routed points. */
export function edgePath(points: readonly Point[]): string {
  return points
    .map(
      (point, index) =>
        `${index ? 'L' : 'M'}${String(point.x)} ${String(point.y)}`,
    )
    .join(' ');
}
