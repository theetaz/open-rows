// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { SchemaDiagramDto } from '@openrows/ipc';
import { DiagramContent } from './DiagramContent';
import { DiagramToolbar } from './DiagramToolbar';
import { exportDiagram } from './exportImage';
import type { ExportFormat } from './exportImage';
import { matchingTables } from './layout';
import type { DiagramLayout, DiagramNode, Point } from './layout';
import { usePanZoom } from './usePanZoom';
import { centreOn, fitView } from './viewport';
import type { Size, View } from './viewport';
import styles from './Erd.module.css';

const middle = (node: DiagramNode): Point => ({
  x: node.x + node.width / 2,
  y: node.y + node.height / 2,
});

/** The focused table at 100 %, else the whole diagram fitted. */
function initialView(layout: DiagramLayout, focus: string | undefined) {
  const node = layout.nodes.find((item) => item.table.name === focus);
  return (viewport: Size): View =>
    node
      ? centreOn({ x: 0, y: 0, scale: 1 }, middle(node), viewport)
      : fitView(layout, viewport);
}

type Notice = { kind: 'saved' | 'error'; text: string } | null;

/** Export through the native panel, reporting where it went or why not. */
function useExport(
  name: string,
  drawing: () => { content: SVGGElement; size: Size } | null,
) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice>(null);
  const run = (format: ExportFormat): void => {
    const target = drawing();
    if (!target) return;
    setBusy(true);
    setNotice(null);
    exportDiagram(format, name, target)
      .then(
        (path) => {
          if (path) setNotice({ kind: 'saved', text: `Saved ${path}` });
        },
        (error: unknown) => {
          const text = error instanceof Error ? error.message : String(error);
          setNotice({ kind: 'error', text });
        },
      )
      .finally(() => {
        setBusy(false);
      });
  };
  return { busy, notice, run };
}

/** Toolbar, pannable canvas and status line of a loaded diagram. */
export function DiagramView(props: {
  schema: string;
  diagram: SchemaDiagramDto;
  layout: DiagramLayout;
  focus?: string | undefined;
  onOpen: (table: string) => void;
  onDone: () => void;
}): React.JSX.Element {
  const { layout } = props;
  const [query, setQuery] = useState('');
  const [cursor, setCursor] = useState(0);
  const content = useRef<SVGGElement>(null);
  const { attach, ...pan } = usePanZoom(initialView(layout, props.focus));
  const matches = matchingTables(layout.nodes, query);
  const found = layout.nodes.filter((node) => matches.has(node.table.name));
  const exporter = useExport(props.schema, () =>
    content.current ? { content: content.current, size: layout } : null,
  );
  const reveal = (nodes: DiagramNode[], index: number): void => {
    const node = nodes[index % Math.max(nodes.length, 1)];
    setCursor(index);
    if (node) pan.centre(middle(node));
  };
  return (
    <div className={styles.view}>
      <DiagramToolbar
        query={query}
        onQuery={(next) => {
          setQuery(next);
          const hits = matchingTables(layout.nodes, next);
          reveal(
            layout.nodes.filter((node) => hits.has(node.table.name)),
            0,
          );
        }}
        onNext={() => {
          reveal(found, cursor + 1);
        }}
        matches={found.length}
        scale={pan.view.scale}
        onZoom={pan.zoomBy}
        onFit={() => {
          pan.fit(layout);
        }}
        exporting={exporter.busy}
        onExport={exporter.run}
        onDone={props.onDone}
      />
      <svg
        ref={attach}
        className={styles.canvas}
        role="group"
        aria-label={`ER diagram of ${props.schema}`}
        onPointerDown={pan.onPointerDown}
      >
        <g
          transform={`translate(${String(pan.view.x)} ${String(pan.view.y)}) scale(${String(pan.view.scale)})`}
        >
          <DiagramContent
            layout={layout}
            matches={matches}
            contentRef={content}
            onOpen={(table) => {
              if (!pan.panned) props.onOpen(table);
            }}
          />
        </g>
      </svg>
      <Status diagram={props.diagram} notice={exporter.notice} />
    </div>
  );
}

function Status(props: {
  diagram: SchemaDiagramDto;
  notice: Notice;
}): React.JSX.Element {
  const { diagram, notice } = props;
  const tables = diagram.tables.length;
  const keys = diagram.foreignKeys.length;
  return (
    <footer className={styles.status}>
      <span>
        {`${String(tables)} ${tables === 1 ? 'table' : 'tables'} · ${String(keys)} ${keys === 1 ? 'relationship' : 'relationships'}`}
        {diagram.truncated &&
          ` · showing the first ${String(diagram.limit)} tables by name`}
      </span>
      {notice && (
        <span
          className={notice.kind === 'error' ? styles.error : undefined}
          role={notice.kind === 'error' ? 'alert' : 'status'}
        >
          {notice.text}
        </span>
      )}
    </footer>
  );
}
