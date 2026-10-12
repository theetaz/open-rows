// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import { fromNative, NATIVE_TYPE, toNative } from './temporal';
import type { TemporalKind } from './temporal';
import styles from './Inline.module.css';

type Props = {
  kind: TemporalKind;
  openerRef?: React.Ref<HTMLButtonElement> | undefined;
  /** The value being edited, as typed text. */
  value: string;
  /** Column name, for the accessible label. */
  name: string;
  /** The native picker opened; typing focus may leave until it closes. */
  onOpen?: (() => void) | undefined;
  /** The picker closed: `text` is the picked value, or `null` when unchanged. */
  onPick: (text: string | null) => void;
  className?: string | undefined;
  style?: React.CSSProperties | undefined;
};

function showPicker(input: HTMLInputElement): void {
  try {
    input.showPicker();
  } catch {
    input.focus();
    input.click();
  }
}

/**
 * A calendar button opening the system date/time picker for `value`
 * (FR-DATA-06); typing stays the primary entry, the picker only fills it.
 */
export function TemporalPicker({
  openerRef,
  ...props
}: Props): React.JSX.Element {
  const input = useRef<HTMLInputElement>(null);
  const { kind, name } = props;
  return (
    <span
      data-temporal-picker
      className={[styles.picker, props.className].filter(Boolean).join(' ')}
      style={props.style}
    >
      <IconButton
        ref={openerRef ?? null}
        size="sm"
        variant="ghost"
        aria-label={`Pick ${kind === 'time' ? 'time' : 'date'} for ${name}`}
        title={kind === 'time' ? 'Pick a time' : 'Pick a date'}
        onMouseDown={(event) => {
          // Keep focus (and any typing) in the value box.
          event.preventDefault();
        }}
        onClick={() => {
          const element = input.current;
          if (!element) return;
          element.value = toNative(props.value, kind);
          props.onOpen?.();
          element.focus({ preventScroll: true });
          showPicker(element);
        }}
      >
        <Icon name="calendar" />
      </IconButton>
      <input
        ref={input}
        type={NATIVE_TYPE[kind]}
        step={kind === 'date' ? undefined : 'any'}
        tabIndex={-1}
        aria-label={`Choose ${kind} for ${name}`}
        className={styles.native}
        onChange={(event) => {
          const text = fromNative(event.target.value, props.value, kind);
          props.onPick(text === props.value ? null : text);
        }}
        onBlur={() => {
          props.onPick(null);
        }}
      />
    </span>
  );
}
