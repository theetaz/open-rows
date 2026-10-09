// SPDX-License-Identifier: Apache-2.0
import type { Ref } from 'react';
import { DIAGRAM_STYLE } from './diagramStyle';
import { edgePath } from './layout';
import type { DiagramLayout } from './layout';
import { TableNode } from './TableNode';

/** Referencing columns per table, for the FK marks. */
function foreignColumns(layout: DiagramLayout): Map<string, Set<string>> {
  const columns = new Map<string, Set<string>>();
  for (const { key } of layout.edges) {
    const set = columns.get(key.table) ?? new Set<string>();
    for (const column of key.columns) set.add(column);
    columns.set(key.table, set);
  }
  return columns;
}

const NONE: ReadonlySet<string> = new Set();

/** Crow's foot at the referencing end, a bar at the referenced end. */
function Markers(): React.JSX.Element {
  return (
    <defs>
      <marker
        id="erd-many"
        viewBox="0 0 12 12"
        refX={12}
        refY={6}
        markerWidth={12}
        markerHeight={12}
        markerUnits="userSpaceOnUse"
        orient="auto"
      >
        <path className="erd-mark" d="M0 6L12 0M0 6L12 12M0 6H12" />
      </marker>
      <marker
        id="erd-one"
        viewBox="0 0 12 12"
        refX={0}
        refY={6}
        markerWidth={12}
        markerHeight={12}
        markerUnits="userSpaceOnUse"
        orient="auto"
      >
        <path className="erd-mark" d="M5 0V12" />
      </marker>
    </defs>
  );
}

/**
 * Everything drawn: styles, relationship lines and table boxes, in one group
 * the export copies. Pan and zoom transform its parent, so panning does not
 * re-render the tables.
 */
export function DiagramContent(props: {
  layout: DiagramLayout;
  matches: ReadonlySet<string>;
  onOpen: (name: string) => void;
  contentRef: Ref<SVGGElement>;
}): React.JSX.Element {
  const { layout, matches, contentRef } = props;
  const foreign = foreignColumns(layout);
  return (
    <g
      ref={contentRef}
      data-diagram=""
      data-searching={matches.size ? '' : undefined}
    >
      <style>{DIAGRAM_STYLE}</style>
      <Markers />
      {layout.edges.map((edge) => (
        <path
          key={edge.id}
          className="erd-edge"
          d={edgePath(edge.points)}
          markerStart="url(#erd-one)"
          markerEnd="url(#erd-many)"
        >
          <title>{`${edge.key.table}.${edge.key.columns.join(', ')} → ${edge.key.targetTable}`}</title>
        </path>
      ))}
      {layout.nodes.map((node) => (
        <TableNode
          key={node.table.name}
          node={node}
          foreign={foreign.get(node.table.name) ?? NONE}
          match={matches.has(node.table.name)}
          onOpen={props.onOpen}
        />
      ))}
    </g>
  );
}
