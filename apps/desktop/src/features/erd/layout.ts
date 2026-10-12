// SPDX-License-Identifier: Apache-2.0
import { graphlib, layout } from '@dagrejs/dagre';
import type { EdgeLabel, GraphLabel, NodeLabel } from '@dagrejs/dagre';
import type {
  DiagramForeignKeyDto,
  DiagramTableDto,
  SchemaDiagramDto,
} from '@openrows/ipc';

import { tableSize, GAP, MARGIN } from './geometry';
import type { DiagramLayout, DiagramNode } from './geometry';
export {
  HEADER_HEIGHT,
  ROW_HEIGHT,
  MAX_COLUMNS,
  tableSize,
  matchingTables,
  edgePath,
} from './geometry';

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
