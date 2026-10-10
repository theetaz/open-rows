// SPDX-License-Identifier: Apache-2.0
import { SecureField, SelectField } from '@openrows/ui';
import type { PasswordStorageDto } from '@openrows/ipc';
import { CredentialFields } from './CredentialFields';
import { SecretField } from './SecretField';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

const storageChoices: readonly { value: PasswordStorageDto; label: string }[] =
  [
    { value: 'keychain', label: 'Save in keychain' },
    { value: 'ask', label: 'Ask every time' },
    { value: 'none', label: 'No password' },
    { value: 'command', label: 'Run a command' },
  ];

/**
 * The database password and where it lives (TablePlus): a [`SecretField`]
 * saved in the Keychain, or asked for on every connect and never saved.
 */
export function PasswordField({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft } = form;
  return (
    <>
      {draft.passwordStorage === 'command' ? (
        <CredentialFields form={form} />
      ) : draft.passwordStorage === 'none' ? (
        <small className={styles.hint}>
          No database password is sent or requested. Use peer, trust, IAM or
          socket authentication.
        </small>
      ) : draft.passwordStorage === 'ask' ? (
        <AskedPassword form={form} />
      ) : (
        <SecretField
          label={draft.engine === 'libsql' ? 'Auth token' : 'Password'}
          noun={draft.engine === 'libsql' ? 'auth token' : 'password'}
          secret={{
            value: draft.password,
            mode: draft.passwordMode,
            stored: draft.hasStoredPassword,
          }}
          reveal={form.reveal('password')}
          onChange={(secret) => {
            if (secret.mode === 'clear')
              form.update({ passwordMode: 'clear', password: '' });
            else form.setPassword(secret.value);
          }}
        />
      )}
      <SelectField
        className={styles.row}
        label="Store"
        aria-label={
          draft.engine === 'libsql' ? 'Auth token storage' : 'Password storage'
        }
        value={draft.passwordStorage}
        options={storageChoices}
        onValueChange={(passwordStorage) => {
          form.update({ passwordStorage });
        }}
      />
    </>
  );
}

/** "Ask every time": a password typed here is only used by Test. */
function AskedPassword({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { draft } = form;
  return (
    <div className={styles.stack}>
      <SecureField
        className={styles.row}
        label={draft.engine === 'libsql' ? 'Auth token' : 'Password'}
        placeholder="Asked when connecting"
        value={draft.password}
        onChange={(event) => {
          form.setPassword(event.target.value);
        }}
      />
      <small className={styles.hint}>
        {draft.hasStoredPassword
          ? 'The saved password is removed from the Keychain on Save. You are asked for it each time you connect.'
          : 'Never saved. You are asked for it each time you connect; a password typed here is only used by Test.'}
      </small>
    </div>
  );
}
