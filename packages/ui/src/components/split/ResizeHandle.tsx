// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type { PointerEvent, KeyboardEvent } from 'react';
import type { PanelRange } from './width';
import { boundedWidth } from './width';
import styles from './SplitPanel.module.css';

type Props = {
  label: string;
  width: number;
  range: PanelRange;
  edge: 'left' | 'right';
  preview: (width: number) => void;
  commit: (width: number) => void;
};
/** Pointer previews bypass React; only completed gestures update persisted state. */
export function ResizeHandle(props: Props): React.JSX.Element {
  const drag = useRef<{ x: number; width: number; latest: number } | null>(
    null,
  );
  function start(event: PointerEvent<HTMLDivElement>): void {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      x: event.clientX,
      width: props.width,
      latest: props.width,
    };
  }
  function move(event: PointerEvent<HTMLDivElement>): void {
    const gesture = drag.current;
    if (!gesture) return;
    gesture.latest = boundedWidth(
      gesture.width +
        (event.clientX - gesture.x) * (props.edge === 'right' ? 1 : -1),
      props.range,
    );
    props.preview(gesture.latest);
    event.currentTarget.setAttribute('aria-valuenow', String(gesture.latest));
  }
  function finish(): void {
    if (!drag.current) return;
    props.commit(drag.current.latest);
    drag.current = null;
  }
  return (
    <div
      role="separator"
      tabIndex={0}
      aria-label={props.label}
      aria-orientation="vertical"
      aria-valuemin={props.range.min}
      aria-valuemax={props.range.max}
      aria-valuenow={props.width}
      className={styles.handle}
      data-edge={props.edge}
      onPointerDown={start}
      onPointerMove={move}
      onPointerUp={finish}
      onLostPointerCapture={finish}
      onPointerCancel={(event) => {
        drag.current = null;
        props.commit(props.width);
        event.currentTarget.setAttribute('aria-valuenow', String(props.width));
      }}
      onDoubleClick={() => {
        props.commit(props.range.initial);
      }}
      onKeyDown={(event) => {
        resizeKey(event, props);
      }}
    />
  );
}
function resizeKey(event: KeyboardEvent<HTMLDivElement>, props: Props): void {
  const direction = props.edge === 'right' ? 1 : -1;
  const values: Record<string, number> = {
    ArrowLeft: props.width - 10 * direction,
    ArrowRight: props.width + 10 * direction,
    Home: props.range.min,
    End: props.range.max,
    Enter: props.range.initial,
  };
  const value = values[event.key];
  if (value === undefined) return;
  event.preventDefault();
  props.commit(boundedWidth(value, props.range));
}
