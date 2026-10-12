// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { drawDiagram, tableAt } from './canvasDrawing';
import type { DiagramLayout } from './geometry';
import type { View } from './viewport';
import styles from './Erd.module.css';

/** Large diagrams keep column DOM out of the viewport; the selector provides keyboard access. */
export function DiagramCanvas(props: {
  layout: DiagramLayout;
  view: View;
  matches: ReadonlySet<string>;
  attach: (element: HTMLCanvasElement | null) => void;
  onPointerDown: (event: React.PointerEvent) => void;
  panned: boolean;
  onOpen: (table: string) => void;
}): React.JSX.Element {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const draw = (): void => {
      drawDiagram(canvas, props.layout, props.view, props.matches);
    };
    draw();
    const observer = new ResizeObserver(draw);
    observer.observe(canvas);
    return () => {
      observer.disconnect();
    };
  }, [props.layout, props.view, props.matches]);
  return (
    <div className={styles.largeCanvas}>
      <canvas
        className={styles.canvas}
        aria-label="ER diagram canvas"
        role="img"
        ref={(canvas) => {
          ref.current = canvas;
          props.attach(canvas);
        }}
        onPointerDown={props.onPointerDown}
        onClick={(event) => {
          if (props.panned) return;
          const rect = event.currentTarget.getBoundingClientRect();
          const name = tableAt(props.layout, {
            x: (event.clientX - rect.left - props.view.x) / props.view.scale,
            y: (event.clientY - rect.top - props.view.y) / props.view.scale,
          });
          if (name) props.onOpen(name);
        }}
      />
      <select
        aria-label="Open diagram table"
        value=""
        onChange={(event) => {
          props.onOpen(event.target.value);
        }}
      >
        <option value="" disabled>
          Open table…
        </option>
        {props.layout.nodes.map(({ table }) => (
          <option key={table.name} value={table.name}>
            {table.name}
          </option>
        ))}
      </select>
    </div>
  );
}
