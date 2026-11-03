// SPDX-License-Identifier: Apache-2.0
import type { SafetyDto } from '@openrows/ipc';
import { POLICY_DETAILS, POLICY_LABELS } from './policies';
import { useSecurity } from './useSecurity';
import styles from '../connections/ConnectionSheet.module.css';
import { t } from './messages';
/** Explicit safety selector; unsupported authentication never silently becomes unrestricted. */
export function SafetySelector(props: {
  value: SafetyDto;
  onChange: (value: SafetyDto) => void;
  /** Take the sheet's initial focus: it was opened to change this. */
  initialFocus?: boolean | undefined;
}): React.JSX.Element {
  const security = useSecurity();
  return (
    <div className={styles.row}>
      <label htmlFor="connection-safety" className={styles.label}>
        {t('safetyPolicy')}
      </label>
      <div>
        <select
          id="connection-safety"
          data-initial-focus={props.initialFocus}
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
            {t('noticeAuthenticatePolicies', { notice: security.notice })}
          </p>
        )}
      </div>
    </div>
  );
}
