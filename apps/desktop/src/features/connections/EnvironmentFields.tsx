// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, Switch, TextField } from '@openrows/ui';
import type { EnvKindDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { ConnectionForm } from './useConnectionForm';
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
    <fieldset className={styles.section}>
      <legend>Environment and safety</legend>
      <SegmentedControl
        aria-label="Environment"
        options={environments}
        value={draft.envKind}
        onValueChange={form.setEnvironment}
      />
      {draft.envKind === 'custom' && (
        <div className={styles.custom}>
          <TextField
            label="Label"
            value={draft.envLabel}
            {...fieldError(errors.envLabel)}
            onChange={(event) => {
              update({ envLabel: event.target.value });
            }}
          />
          <label className={styles.colour}>
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
      <SafetySwitch
        label="Read-only"
        detail="Reject every write statement."
        checked={draft.readOnly}
        onChange={(readOnly) => {
          update({ readOnly });
        }}
      />
      <SafetySwitch
        label="Confirm writes"
        detail="Ask before running a write statement. On by default for prod."
        checked={draft.confirmWrites}
        onChange={(confirmWrites) => {
          update({ confirmWrites });
        }}
      />
    </fieldset>
  );
}

function SafetySwitch(props: {
  label: string;
  detail: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <div className={styles.switchRow}>
      <span>
        <strong>{props.label}</strong>
        <small>{props.detail}</small>
      </span>
      <Switch
        aria-label={props.label}
        checked={props.checked}
        onCheckedChange={props.onChange}
      />
    </div>
  );
}
