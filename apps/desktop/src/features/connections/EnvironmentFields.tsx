// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, TextField } from '@openrows/ui';
import type { EnvKindDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { ConnectionForm } from './useConnectionForm';
import { SwitchRow } from './SwitchRow';
import styles from './flow/Form.module.css';
import { t } from './messages/profile';
import { t as flow } from './flow/messages';

const environments: readonly { value: EnvKindDto; label: string }[] = [
  { value: 'local', label: t('local') },
  { value: 'dev', label: t('dev') },
  { value: 'staging', label: t('staging') },
  { value: 'prod', label: t('prod') },
  { value: 'custom', label: t('custom2') },
];

/** What each environment does to the window chrome. */
const effects: Record<EnvKindDto, string> = {
  local: flow('envLocal'),
  dev: flow('envDev'),
  staging: flow('envStaging'),
  prod: flow('envProd'),
  custom: flow('envCustom'),
};

/** Environment rows: the tag and what it does, and a custom label and colour. */
export function EnvironmentFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  return (
    <>
      <div className={styles.choiceRow}>
        <span className={styles.stack}>
          <span>{flow('colourAndLabel')}</span>
          <small>{effects[draft.envKind]}</small>
        </span>
        <SegmentedControl
          aria-label={t('environment')}
          options={environments}
          value={draft.envKind}
          onValueChange={form.setEnvironment}
        />
      </div>
      {draft.envKind === 'custom' && (
        <div className={styles.fileRow}>
          <TextField
            label={t('label')}
            value={draft.envLabel}
            {...fieldError(errors.envLabel)}
            onChange={(event) => {
              update({ envLabel: event.target.value });
            }}
          />
          <label className={styles.colour}>
            <input
              type="color"
              aria-label={t('colour')}
              value={draft.envColor}
              onChange={(event) => {
                update({ envColor: event.target.value });
              }}
            />
          </label>
        </div>
      )}
    </>
  );
}

/** History rows: record executed statements, and redact their literals. */
export function HistoryFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, update } = form;
  return (
    <>
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
