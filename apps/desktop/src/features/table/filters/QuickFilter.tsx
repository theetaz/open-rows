// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Button, SelectField, TextField } from '@openrows/ui';
import styles from './QuickFilter.module.css';

/** An operator the quick filter offers; `arity` 0 takes no value (IS NULL). */
export type QuickOperator<T extends string> = {
  value: T;
  label: string;
  arity: 0 | 1;
};

type Props<T extends string> = {
  /** Column being filtered. */
  name: string;
  /** Window point under the header that opened it. */
  x: number;
  y: number;
  operators: readonly QuickOperator<T>[];
  onApply: (operator: T, value: string) => void;
  onClose: () => void;
};

/** Close on a pointer press outside `panel`, Escape or a resize. */
function useDismiss(
  panel: React.RefObject<HTMLElement | null>,
  onClose: () => void,
): void {
  useEffect(() => {
    const outside = (event: Event): void => {
      if (
        event.target instanceof Node &&
        !panel.current?.contains(event.target)
      )
        onClose();
    };
    document.addEventListener('pointerdown', outside);
    window.addEventListener('resize', onClose);
    return () => {
      document.removeEventListener('pointerdown', outside);
      window.removeEventListener('resize', onClose);
    };
  }, [panel, onClose]);
}

/**
 * A per-column filter opened from a column header (FR-GRID-11): one row
 * with an operator, a value and Filter; Return applies, Escape closes.
 */
export function QuickFilter<T extends string>(
  props: Props<T>,
): React.JSX.Element | null {
  const panel = useRef<HTMLDivElement>(null);
  const first = props.operators[0];
  const [operator, setOperator] = useState<T | undefined>(first?.value);
  const [value, setValue] = useState('');
  useDismiss(panel, props.onClose);
  useLayoutEffect(() => {
    const element = panel.current;
    if (!element) return;
    const left = Math.min(props.x, innerWidth - element.offsetWidth - 8);
    element.style.left = `${String(Math.max(8, left))}px`;
    element.style.top = `${String(Math.max(8, Math.min(props.y, innerHeight - element.offsetHeight - 8)))}px`;
  }, [props.x, props.y]);
  if (operator === undefined) return null;
  const arity = props.operators.find((item) => item.value === operator)?.arity;
  const apply = (): void => {
    props.onApply(operator, value);
    props.onClose();
  };
  return createPortal(
    <div
      ref={panel}
      role="dialog"
      aria-label={`Filter ${props.name}`}
      className={styles.popover}
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        if (event.key === 'Escape') {
          event.stopPropagation();
          props.onClose();
        } else if (event.key === 'Enter') {
          event.preventDefault();
          apply();
        }
      }}
    >
      <SelectField<T>
        aria-label="Operator"
        controlSize="compact"
        className={styles.operator}
        value={operator}
        options={props.operators}
        onValueChange={setOperator}
      />
      {arity !== 0 && (
        <TextField
          aria-label={`Value of ${props.name}`}
          controlSize="compact"
          className={styles.value}
          placeholder="Value"
          autoFocus
          value={value}
          onChange={(event) => {
            setValue(event.target.value);
          }}
        />
      )}
      <Button size="sm" variant="primary" onClick={apply}>
        Filter
      </Button>
    </div>,
    document.body,
  );
}
