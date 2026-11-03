// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, TextField } from '@openrows/ui';
import type { EnvKindDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { ConnectionForm } from './useConnectionForm';
import { SafetySelector } from '../security/SafetySelector';
import { SwitchRow } from './SwitchRow';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';

const environments: readonly { value: EnvKindDto; label: string }[] = [
  { value: 'local', label: t('local') },
  { value: 'dev', label: t('dev') },
  { value: 'staging', label: t('staging') },
  { value: 'prod', label: t('prod') },
  { value: 'custom', label: t('custom2') },
];

/** Environment tag (custom label and colour) and the write-safety switches. */
export function EnvironmentFields({
  form,
  focusSafety,
}: {
  form: ConnectionForm;
  focusSafety?: boolean | undefined;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  return (
    <>
      <div className={styles.row}>
        <span className={styles.label}>{t('environment')}</span>
        <SegmentedControl
          className={styles.segmented}
          aria-label={t('environment')}
          options={environments}
          value={draft.envKind}
          onValueChange={form.setEnvironment}
        />
      </div>
      {draft.envKind === 'custom' && (
        <div className={styles.pair}>
          <TextField
            className={styles.row}
            label={t('label')}
            value={draft.envLabel}
            {...fieldError(errors.envLabel)}
            onChange={(event) => {
              update({ envLabel: event.target.value });
            }}
          />
          <label className={`${styles.port ?? ''} ${styles.colour ?? ''}`}>
            <span>{t('colour')}</span>
            <input
              type="color"
              value={draft.envColor}
              onChange={(event) => {
                update({ envColor: event.target.value });
              }}
            />
          </label>
        </div>
      )}
      <SafetySelector
        value={draft.safety}
        initialFocus={focusSafety}
        onChange={(safety) => {
          update({ safety });
        }}
      />
      <SwitchRow
        label={t('recordQueryHistory')}
        detail={t('keepExecutedStatements')}
        checked={draft.recordHistory}
        onChange={(recordHistory) => {
          update({ recordHistory });
        }}
      />
      <SwitchRow
        label={t('redactLiteralsHistory')}
        detail={t('productionReadOnly')}
        checked={draft.redactHistory}
        onChange={(redactHistory) => {
          update({ redactHistory });
        }}
      />
    </>
  );
}
