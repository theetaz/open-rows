// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { readWidth, writeWidth, boundedWidth } from './width';
import type { PanelRange } from './width';
import { ResizeHandle } from './ResizeHandle';
import styles from './SplitPanel.module.css';
import { usePanelPresence } from './usePanelPresence';

type Props = {
  label: string;
  storageKey: string;
  range: PanelRange;
  open: boolean;
  edge: 'left' | 'right';
  overlay?: boolean;
  children: ReactNode;
};
/** Collapsible split region; CSS handles animation and drag previews, never its sibling tree. */
export function SplitPanel(props: Props): React.JSX.Element | null {
  const present = usePanelPresence(props.open);
  const [width, setWidth] = useState(() =>
    readWidth(props.storageKey, props.range),
  );
  const ref = useRef<HTMLElement>(null);
  function preview(value: number): void {
    ref.current?.style.setProperty('--panel-width', `${String(value)}px`);
    ref.current?.setAttribute('data-resizing', 'true');
  }
  function commit(value: number): void {
    const next = boundedWidth(value, props.range);
    ref.current?.style.setProperty('--panel-width', `${String(next)}px`);
    ref.current?.removeAttribute('data-resizing');
    writeWidth(props.storageKey, next);
    setWidth(next);
  }
  if (!present) return null;
  return (
    <aside
      ref={ref}
      aria-label={props.label}
      aria-hidden={!props.open}
      inert={!props.open}
      className={styles.panel}
      data-open={props.open}
      data-overlay={props.overlay}
      data-edge={props.edge}
      style={{ '--panel-width': `${String(width)}px` } as CSSProperties}
    >
      <div className={styles.inner}>{props.children}</div>
      <ResizeHandle
        label={`Resize ${props.label.toLowerCase()}`}
        width={width}
        range={props.range}
        edge={props.edge}
        preview={preview}
        commit={commit}
      />
    </aside>
  );
}
