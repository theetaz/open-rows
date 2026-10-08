// SPDX-License-Identifier: Apache-2.0
import { Switch } from '@openrows/ui';
import styles from './ConnectionSheet.module.css';

/** A small switch with a label and a one-line explanation, aligned with the fields. */
export function SwitchRow(props: {
  label: string;
  detail: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <div className={styles.switchRow}>
      <Switch
        aria-label={props.label}
        checked={props.checked}
        onCheckedChange={props.onChange}
      />
      <span>
        {props.label}
        <small>{props.detail}</small>
      </span>
    </div>
  );
}
