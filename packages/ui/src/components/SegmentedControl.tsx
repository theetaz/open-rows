// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';
import styles from './Controls.module.css';

type Option<Value extends string> = {
  value: Value;
  label: string;
  disabled?: boolean;
};
type Props<Value extends string> = {
  options: readonly Option<Value>[];
  value: Value;
  onValueChange: (value: Value) => void;
  'aria-label': string;
  disabled?: boolean;
  className?: string;
};
function nextIndex(
  key: string,
  index: number,
  count: number,
): number | undefined {
  if (key === 'Home') return 0;
  if (key === 'End') return count - 1;
  if (key === 'ArrowRight' || key === 'ArrowDown') return (index + 1) % count;
  if (key === 'ArrowLeft' || key === 'ArrowUp')
    return (index - 1 + count) % count;
  return undefined;
}
/** Single-select radio group with wrapping arrows, Home/End and roving focus. */
export function SegmentedControl<Value extends string>(
  props: Props<Value>,
): React.JSX.Element {
  const enabled = props.options.filter((option) => !option.disabled);
  const active = enabled.some((option) => option.value === props.value)
    ? props.value
    : enabled[0]?.value;
  return (
    <div
      className={[styles.segmented, props.className].filter(Boolean).join(' ')}
      role="radiogroup"
      aria-label={props['aria-label']}
      aria-disabled={props.disabled}
    >
      {props.options.map((option) => (
        <Segment
          key={option.value}
          option={option}
          selected={option.value === props.value}
          disabled={Boolean(props.disabled || option.disabled)}
          tabbable={!props.disabled && active === option.value}
          select={() => {
            props.onValueChange(option.value);
          }}
          navigate={(event) => {
            navigate(event, option.value, props);
          }}
        />
      ))}
    </div>
  );
}

type SegmentProps<Value extends string> = {
  option: Option<Value>;
  selected: boolean;
  disabled: boolean;
  tabbable: boolean;
  select: () => void;
  navigate: (event: KeyboardEvent<HTMLButtonElement>) => void;
};
function Segment<Value extends string>(
  props: SegmentProps<Value>,
): React.JSX.Element {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={props.selected}
      disabled={props.disabled}
      tabIndex={props.tabbable ? 0 : -1}
      className={styles.segment}
      onClick={props.select}
      onKeyDown={props.navigate}
    >
      {props.option.label}
    </button>
  );
}

function navigate<Value extends string>(
  event: KeyboardEvent<HTMLButtonElement>,
  value: Value,
  props: Props<Value>,
): void {
  const enabled = props.options.filter((option) => !option.disabled);
  const index = nextIndex(
    event.key,
    enabled.findIndex((item) => item.value === value),
    enabled.length,
  );
  const option = index === undefined ? undefined : enabled[index];
  if (!option || props.disabled || index === undefined) return;
  event.preventDefault();
  props.onValueChange(option.value);
  const peers =
    event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
      'button[role="radio"]:not(:disabled)',
    );
  peers?.[index]?.focus();
}
