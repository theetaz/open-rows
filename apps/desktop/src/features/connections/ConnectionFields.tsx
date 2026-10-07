// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { TlsModeDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { Draft } from './form';
import { PasswordField } from './PasswordField';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

type TlsOption = { value: TlsModeDto; label: string };

const tlsModes: readonly TlsOption[] = [
  { value: 'prefer', label: 'Prefer (no TLS yet)' },
  { value: 'disable', label: 'Disable' },
  { value: 'require', label: 'Require' },
  { value: 'verify-full', label: 'Verify full' },
];

function tlsOptions(current: TlsModeDto): readonly TlsOption[] {
  return tlsModes.some((mode) => mode.value === current)
    ? tlsModes
    : [...tlsModes, { value: current, label: current }];
}

/** Name, group, address, credentials and TLS mode, labels leading as in AppKit forms. */
export function ConnectionFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  const text = (key: keyof Draft) => ({
    className: styles.row,
    value: String(draft[key]),
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      update({ [key]: event.target.value });
    },
  });
  return (
    <>
      <TextField label="Name" {...text('name')} {...fieldError(errors.name)} />
      <TextField label="Group" placeholder="Optional" {...text('group')} />
      <div className={styles.pair}>
        <TextField
          label="Host"
          {...text('host')}
          {...fieldError(errors.host)}
        />
        <TextField
          label="Port"
          inputMode="numeric"
          {...text('port')}
          className={styles.port}
          {...fieldError(errors.port)}
        />
      </div>
      <TextField label="User" {...text('user')} {...fieldError(errors.user)} />
      <PasswordField form={form} />
      <TextField
        label="Database"
        placeholder="Same as user"
        {...text('database')}
      />
      <SelectField
        className={styles.row}
        label="TLS"
        value={draft.tlsMode}
        options={tlsOptions(draft.tlsMode)}
        onValueChange={(tlsMode) => {
          update({ tlsMode });
        }}
      />
    </>
  );
}
