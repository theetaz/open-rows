// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, TextField } from '@openrows/ui';
import type { EnvKindDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { ConnectionForm } from './useConnectionForm';
import { SafetySelector } from '../security/SafetySelector';
import { SwitchRow } from './SwitchRow';
import styles from './ConnectionSheet.module.css';

const environments: readonly { value: EnvKindDto; label: string }[] = [
  { value: 'local', label: 'Local' },
  { value: 'dev', label: 'Dev' },
  { value: 'staging', label: 'Staging' },
  { value: 'prod', label: 'Prod' },
  { value: 'custom', label: 'Custom' },
];

/** Environment tag (custom label and colour) and the write-safety switches. */
export function EnvironmentFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  return (
    <>
      <div className={styles.row}>
        <span className={styles.label}>Environment</span>
        <SegmentedControl
          className={styles.segmented}
          aria-label="Environment"
          options={environments}
          value={draft.envKind}
          onValueChange={form.setEnvironment}
        />
      </div>
      {draft.envKind === 'custom' && (
        <div className={styles.pair}>
          <TextField
            className={styles.row}
            label="Label"
            value={draft.envLabel}
            {...fieldError(errors.envLabel)}
            onChange={(event) => {
              update({ envLabel: event.target.value });
            }}
          />
          <label className={`${styles.port ?? ''} ${styles.colour ?? ''}`}>
            <span>Colour</span>
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
        onChange={(safety) => {
          update({ safety });
        }}
      />
      <SwitchRow
        label="Record query history"
        detail="Keep executed statements in History. Off records nothing."
        checked={draft.recordHistory}
        onChange={(recordHistory) => {
          update({ recordHistory });
        }}
      />
      <SwitchRow
        label="Redact literals in history"
        detail="Production and read-only connections always redact strings and numbers."
        checked={draft.redactHistory}
        onChange={(redactHistory) => {
          update({ redactHistory });
        }}
      />
    </>
  );
}
