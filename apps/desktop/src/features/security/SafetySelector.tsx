// SPDX-License-Identifier: Apache-2.0
import type { SafetyDto } from '@openrows/ipc';
import { SelectField } from '@openrows/ui';
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
    <div className={`${styles.row ?? ''} ${styles.top ?? ''}`}>
      <label htmlFor="connection-safety" className={styles.label}>
        {t('safetyPolicy')}
      </label>
      <div className={styles.stack}>
        <SelectField
          id="connection-safety"
          aria-label={t('safetyPolicy')}
          data-initial-focus={props.initialFocus}
          aria-describedby="safety-description"
          value={props.value}
          options={Object.entries(POLICY_LABELS).map(([value, label]) => ({
            value: value as SafetyDto,
            label,
            disabled: value.startsWith('authenticate') && !security.available,
          }))}
          onValueChange={props.onChange}
        />
        <p id="safety-description" className={styles.help}>
          {POLICY_DETAILS[props.value]}
        </p>
        {security.notice && (
          <p role="status" className={styles.help}>
            {t('noticeAuthenticatePolicies', { notice: security.notice })}
          </p>
        )}
      </div>
    </div>
  );
}
