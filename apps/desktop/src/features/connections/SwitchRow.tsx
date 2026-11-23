// SPDX-License-Identifier: Apache-2.0
import { Switch } from '@openrows/ui';
import styles from './flow/Form.module.css';

/** A row of an inset group: label and one-line explanation, the small switch trailing. */
export function SwitchRow(props: {
  label: string;
  detail?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <div className={styles.switchRow}>
      <span className={styles.stack}>
        <span>{props.label}</span>
        {props.detail && <small>{props.detail}</small>}
      </span>
      <Switch
        aria-label={props.label}
        checked={props.checked}
        onCheckedChange={props.onChange}
      />
    </div>
  );
}
