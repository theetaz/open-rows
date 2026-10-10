// SPDX-License-Identifier: Apache-2.0
import type { SafetyDto } from '@openrows/ipc';
import { POLICY_DETAILS, POLICY_LABELS } from './policies';
import { useSecurity } from './useSecurity';
import styles from '../connections/ConnectionSheet.module.css';
/** Explicit safety selector; unsupported authentication never silently becomes unrestricted. */
export function SafetySelector(props: {
  value: SafetyDto;
  onChange: (value: SafetyDto) => void;
}): React.JSX.Element {
  const security = useSecurity();
  return (
    <div className={styles.row}>
      <label htmlFor="connection-safety" className={styles.label}>
        Safety policy
      </label>
      <div>
        <select
          id="connection-safety"
          aria-describedby="safety-description"
          value={props.value}
          onChange={(e) => {
            props.onChange(e.target.value as SafetyDto);
          }}
        >
          {Object.entries(POLICY_LABELS).map(([value, label]) => (
            <option
              key={value}
              value={value}
              disabled={value.startsWith('authenticate') && !security.available}
            >
              {label}
            </option>
          ))}
        </select>
        <p id="safety-description">{POLICY_DETAILS[props.value]}</p>
        {security.notice && (
          <p role="status">
            {security.notice} Authenticate policies use matching Confirm
            behaviour.
          </p>
        )}
      </div>
    </div>
  );
}
