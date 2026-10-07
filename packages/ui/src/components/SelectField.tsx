// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import type { SelectHTMLAttributes } from 'react';
import { Icon } from '../icons/Icon';
import type { ControlSize } from './TextField';
import styles from './TextField.module.css';

type Option<Value extends string> = { value: Value; label: string };
type Props<Value extends string> = Omit<
  SelectHTMLAttributes<HTMLSelectElement>,
  'value' | 'onChange'
> & {
  label: string;
  value: Value;
  options: readonly Option<Value>[];
  onValueChange: (value: Value) => void;
  controlSize?: ControlSize;
};

/** Native pop-up button in the shared text-field frame: same height and radius. */
export function SelectField<Value extends string>({
  label,
  value,
  options,
  onValueChange,
  controlSize = 'regular',
  id,
  className = '',
  ...props
}: Props<Value>): React.JSX.Element {
  const generated = useId();
  const selectId = id ?? generated;
  return (
    <div className={`${styles.field ?? ''} ${className}`}>
      <label htmlFor={selectId}>{label}</label>
      <div className={styles.frame} data-size={controlSize}>
        <select
          {...props}
          id={selectId}
          value={value}
          onChange={(event) => {
            const next = options.find(
              (option) => option.value === event.target.value,
            );
            if (next) onValueChange(next.value);
          }}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <span aria-hidden="true" className={styles.chevron}>
          <Icon name="chevronUpDown" />
        </span>
      </div>
    </div>
  );
}
